use log::{error, info};
use serde::{Deserialize, Serialize};
use snafu::prelude::*;

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use crate::sandbox::FirejailLauncher;
use crate::sandbox::{BubLauncher, BubMount};

pub type Result<T, E = CellarError> = std::result::Result<T, E>;
pub type EnvVars = HashMap<String, String>;

pub const WINE_CELLAR_CONFIG: &str = "winecellar.json";

#[derive(Debug, Snafu)]
pub enum CellarError {
    MissingConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    ConfigWriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    SerializationError {
        source: serde_json::Error,
    },

    DeserializationError {
        source: serde_json::Error,
    },

    ChildExecError {
        source: std::io::Error,
    },
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
        let file = File::open(&cfg_path).context(MissingConfigSnafu {
            path: cfg_path.clone(),
        })?;

        Ok(WineCellar {
            path: cellar_path.to_path_buf(),
            config: serde_json::from_reader(file).context(DeserializationSnafu)?,
        })
    }

    pub fn create<T: AsRef<Path>>(path: T) -> Result<WineCellar> {
        let cellar_path = path.as_ref();
        let cfg_path = cellar_path.join(WINE_CELLAR_CONFIG);

        std::fs::create_dir_all(cellar_path).context(ConfigWriteSnafu {
            path: cfg_path.clone(),
        })?;

        let cellar = WineCellar {
            path: cellar_path.to_path_buf(),
            config: CellarConfig::default(),
        };

        cellar.save_config()?;

        Ok(cellar)
    }

    pub fn save_config(&self) -> Result<()> {
        let cfg = File::create(self.config_path()).context(ConfigWriteSnafu {
            path: self.config_path(),
        })?;

        serde_json::to_writer_pretty(cfg, &self.config).context(SerializationSnafu)?;
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
        cmd.envs(self.get_env_vars());

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

        l.env(("HOME", "/home"))
            .env(("WINEPREFIX", "/wineprefix"))
            .env(("DISPLAY", ":0"))
            .env(("XDG_RUNTIME_DIR", "/run/user/1000"))
            .env(("XAUTHORITY", "/tmp/xauthority"))
            .env(("DXVK_HUD", "devinfo,fps,api,gpuload,memory"))
            .env(("LANG", "en_US.UTF-8"))
            .mount(BubMount::bind_ro("/etc/fonts", "/etc/fonts"))
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

    pub fn set_env_var(&mut self, key: String, val: String) {
        self.config.extra_env.insert(key, val);
    }

    pub fn get_env_vars(&self) -> &EnvVars {
        &self.config.extra_env
    }

    pub fn get_c_drive_path(&self) -> PathBuf {
        let wineprefix = self.wine_prefix_path();
        wineprefix.join("drive_c")
    }

    #[allow(dead_code)]
    pub fn get_env_var<T: AsRef<str>>(&self, var: T) -> Option<&str> {
        self.get_env_vars().get(var.as_ref()).map(|x| &**x)
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
    extra_env: HashMap<String, String>,
}

impl Default for CellarConfig {
    fn default() -> CellarConfig {
        CellarConfig {
            sandbox: true,
            sync: WineSync::default(),
            extra_env: HashMap::new(),
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
