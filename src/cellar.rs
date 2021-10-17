use serde::{Deserialize, Serialize};
use snafu::prelude::*;

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

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
    config: CellarConfig,
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

    pub fn exec(&self, exec: PathBuf) -> Result<Child> {
        Command::new(self.wine_bin_path())
            .arg(exec)
            .env("WINEPREFIX", self.wine_prefix_path())
            .spawn()
            .context(ChildExecSnafu)
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CellarConfig {
    extra_env: HashMap<String, String>,
}