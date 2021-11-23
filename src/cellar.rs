use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use cellar_sandbox::{BubLauncher, BubMount, EnvVar, FirejailLauncher};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T, E = CellarError> = std::result::Result<T, E>;

pub const WINE_CELLAR_CONFIG: &str = "winecellar.json";
pub const REAPER_LOCAL_LOCATIONS: &str = ".:target/debug/:target/release";
pub const REAPER_BIN_NAME: &str = "cellar-reaper";

fn get_reaper_path() -> Result<PathBuf> {
    // First, check if the reaper binary can be found in the debug or release targets of cargo, or
    // if it can be found in the cwd
    which::which_in(REAPER_BIN_NAME, Some(REAPER_LOCAL_LOCATIONS), ".")
        // Otherwise, resolve it system-wide
        .or_else(|err| {
            debug!("failed to locate reaper locally due to {:?}", err);
            which::which(REAPER_BIN_NAME)
        })
        // If an error occurs, report that the reaper is missing
        .map_err(|_| CellarError::ReaperMissing)
}

#[derive(Debug, Error)]
pub enum CellarError {
    #[error(transparent)]
    ConfigError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error("unable to locate reaper")]
    ReaperMissing,
}

#[derive(Debug)]
pub struct WineCellar {
    path: PathBuf,
    pub config: CellarConfig,
}

impl WineCellar {
    pub fn open<T: AsRef<Path>>(path: T) -> Result<WineCellar> {
        let cellar_path = path.as_ref();
        let cfg_path = cellar_path.join(WINE_CELLAR_CONFIG);
        let file = File::open(&cfg_path)?;

        Ok(WineCellar {
            path: cellar_path.to_path_buf(),
            config: serde_json::from_reader(file)?,
        })
    }

    pub fn create<T: AsRef<Path>>(path: T) -> Result<WineCellar> {
        let cellar_path = path.as_ref();

        std::fs::create_dir_all(cellar_path)?;

        let cellar = WineCellar {
            path: cellar_path.to_path_buf(),
            config: CellarConfig::default(),
        };

        cellar.save_config()?;

        Ok(cellar)
    }

    pub fn save_config(&self) -> Result<()> {
        let cfg = File::create(self.config_path())?;
        serde_json::to_writer_pretty(cfg, &self.config)?;

        Ok(())
    }

    // Returns a `Command` that will start firejail with the proper profile and arguments
    // along with a wineserver with the current prefix. It is up to the caller to use proper
    // arguments or environmental modifications for the specified program.
    pub fn run(&self) -> Command {
        let mut launcher = FirejailLauncher::default();

        launcher.whitelist(std::fs::canonicalize(self.path.to_path_buf()).unwrap());

        let mut cmd = launcher.command();

        cmd.arg(self.wine_bin_path());
        cmd.env("WINEPREFIX", self.wine_prefix_path());
        cmd.envs(
            self.get_env_vars()
                .iter()
                .map(|x| x.clone())
                .map(|x| x.to_key_value())
                .collect::<Vec<(String, String)>>(),
        );

        match self.config.sync {
            WineSync::AUTO => cmd.env("WINEESYNC", "1").env("WINEFSYNC", "1"),
            WineSync::ESYNC => cmd.env("WINEESYNC", "1"),
            WineSync::FSYNC => cmd.env("WINEFSYNC", "1"),
            WineSync::WINESYNC => todo!("winesync"),
        };

        cmd
    }

    pub fn bwrap_run(&self) -> Command {
        let mut l = BubLauncher::default();

        l.mount(BubMount::tmpfs("/tmp"))
            .mount(BubMount::dev_bind(self.wine_prefix_path(), "/wineprefix"))
            .mount(BubMount::dev_bind("/run", "/run"))
            .mount(BubMount::tmpfs("/home"))
            .mount(BubMount::proc("/proc"))
            .mount(BubMount::dev_bind(
                "/run/user/1000/pulse/native",
                "/run/user/1000/pulse/native",
            ))
            .mount(BubMount::dev_bind("/dev", "/dev"))
            .mount(BubMount::bind_ro("/usr", "/usr"))
            .mount(BubMount::symlink("/usr/bin", "/bin"))
            .mount(BubMount::symlink("/usr/bin", "/sbin"))
            .mount(BubMount::symlink("/usr/lib", "/lib"))
            .mount(BubMount::symlink("/usr/lib32", "/lib32"))
            .mount(BubMount::symlink("/usr/lib64", "/lib64"));

        let reaper_path = get_reaper_path().expect("failed to find reaper");
        l.mount(BubMount::bind_ro(reaper_path, "/tmp/reaper"));

        l.env(("HOME", "/home"))
            .env(("WINEPREFIX", "/wineprefix"))
            .env(("DISPLAY", ":0"))
            .env(("XDG_RUNTIME_DIR", "/run/user/1000"))
            .env(("XAUTHORITY", "/tmp/xauthority"))
            .env(("LANG", "en_US.UTF-8"))
            .mount(BubMount::bind_ro("/etc/fonts", "/etc/fonts"))
            .mount(BubMount::dev_bind("/tmp/.X11-unix", "/tmp/.X11-unix"))
            .mount(BubMount::bind_ro("/home/me/.Xauthority", "/tmp/xauthority"));
        //.mount(BubMount::bind_rw(self.wine_prefix_path(), "/home/wine"));

        match self.config.sync {
            WineSync::AUTO => l.env(("WINEESYNC", "1")).env(("WINEFSYNC", "1")),
            WineSync::ESYNC => l.env(("WINEESYNC", "1")),
            WineSync::FSYNC => l.env(("WINEFSYNC", "1")),
            WineSync::WINESYNC => todo!("winesync"),
        };

        let mut cmd = l.command();
        cmd.arg("--");
        cmd
    }

    pub fn bwrap_wine(&self) -> Command {
        let mut cmd = self.bwrap_run();
        cmd.arg("/usr/bin/wine");
        cmd
    }

    pub fn kill(&self) {
        Command::new("wineserver")
            .arg("-k")
            .arg("-w") // wait for wineserver to terminate
            .env("WINEPREFIX", self.wine_prefix_path())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    pub fn set_env_var<T: Into<EnvVar>>(&mut self, env: T) {
        self.config.extra_env.push(env.into());
    }

    pub fn get_env_vars(&self) -> &Vec<EnvVar> {
        &self.config.extra_env
    }

    #[allow(dead_code)]
    pub fn get_env_var<T: AsRef<str>>(&self, var: T) -> Option<&EnvVar> {
        self.get_env_vars().iter().find(|x| x.key() == var.as_ref())
    }

    #[allow(dead_code)]
    pub fn config_path(&self) -> PathBuf {
        self.path.join(WINE_CELLAR_CONFIG)
    }

    #[allow(dead_code)]
    pub fn wine_bin_path(&self) -> PathBuf {
        PathBuf::from("wine")
    }

    #[allow(dead_code)]
    pub fn wine_prefix_path(&self) -> PathBuf {
        let prefix_rel = self.path.to_path_buf();
        let abs_prefix = std::fs::canonicalize(prefix_rel).unwrap();

        abs_prefix
    }

    #[allow(dead_code)]
    pub fn wine_path(&self) -> PathBuf {
        self.path.to_path_buf()
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AsRef<CellarConfig> for WineCellar {
    fn as_ref(&self) -> &CellarConfig {
        &self.config
    }
}

impl AsMut<CellarConfig> for WineCellar {
    fn as_mut(&mut self) -> &mut CellarConfig {
        &mut self.config
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CellarConfig {
    pub sandbox: bool,
    pub sync: WineSync,
    extra_env: Vec<EnvVar>,
}

impl Default for CellarConfig {
    fn default() -> CellarConfig {
        CellarConfig {
            sandbox: true,
            sync: WineSync::default(),
            extra_env: Vec::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WineSync {
    /// Enables both ESYNC and FSYNC for fallback
    AUTO,
    ESYNC,
    FSYNC,
    WINESYNC,
}

impl Default for WineSync {
    fn default() -> WineSync {
        WineSync::AUTO
    }
}

// TODO Proper error type
impl FromStr for WineSync {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_ref() {
            "AUTO" => Ok(WineSync::AUTO),
            "ESYNC" => Ok(WineSync::ESYNC),
            "FSYNC" => Ok(WineSync::FSYNC),
            "WINESYNC" => Ok(WineSync::WINESYNC),
            _ => Err(format!("Unknown sync type \"{}\"", s)),
        }
    }
}
