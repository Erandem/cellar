use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct NSMount {
    r#type: NSMountType,
    readwrite: bool,
    mandatory: bool,
}

#[allow(dead_code)]
impl NSMount {
    /// Creates a new bindmount with `readwrite` set to `false`
    pub fn readonly<T: Into<PathBuf>>(src: T, dest: T) -> NSMount {
        let mut s = Self::bind(src, dest);
        s.readwrite = false;
        s
    }

    /// Creates a new bindmount that is both readwrite and mandatory
    pub fn bind<T: Into<PathBuf>>(src: T, dest: T) -> NSMount {
        NSMount {
            r#type: NSMountType::BindMount {
                src: src.into(),
                dest: dest.into(),
            },

            readwrite: true,
            mandatory: true,
        }
    }

    pub fn temp<T: Into<PathBuf>>(dest: T) -> NSMount {
        NSMount {
            r#type: NSMountType::TmpFs { dest: dest.into() },

            readwrite: true,
            mandatory: true,
        }
    }

    pub fn make_readonly(&mut self) -> &mut NSMount {
        self.readwrite = false;
        self
    }

    pub fn make_readwrite(&mut self) -> &mut NSMount {
        self.readwrite = true;
        self
    }

    pub fn mandatory(&mut self) -> &mut NSMount {
        self.mandatory = true;
        self
    }

    pub fn not_mandatory(&mut self) -> &mut NSMount {
        self.mandatory = false;
        self
    }

    fn to_write_arg(&self) -> (&'static str, String) {
        match &self.r#type {
            NSMountType::BindMount { src, dest } => {
                let map = format!("{}:{}", src.display(), dest.display());
                let arg = if self.readwrite {
                    "--bindmount"
                } else {
                    "--bindmount_ro"
                };

                (arg, map)
            }
            NSMountType::TmpFs { dest } => ("-m", format!("none:{}:tmpfs:{}", dest.display(), "")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NSMountType {
    BindMount { src: PathBuf, dest: PathBuf },
    TmpFs { dest: PathBuf },
}

impl NSMountType {
    pub fn dest(&self) -> Option<&Path> {
        match &self {
            NSMountType::BindMount { dest, .. } => Some(dest),
            NSMountType::TmpFs { dest, .. } => Some(dest),
        }
    }
}

pub struct NSJail {
    mounts: Vec<NSMount>,
    env: HashMap<String, String>,
    user: u64,
    group: u64,
}

impl NSJail {
    pub fn command(self) -> Command {
        let mut cmd = Command::new("/usr/bin/nsjail");

        cmd.arg("--user").arg(self.user.to_string());
        cmd.arg("--group").arg(self.group.to_string());

        self.mounts
            .into_iter()
            .map(|x| x.to_write_arg())
            .for_each(|x| {
                cmd.arg(x.0).arg(x.1);
            });

        cmd.envs(self.env);

        // Make sure that the caller can pass arguments without worry
        cmd.arg("--");
        cmd
    }

    pub fn mount(&mut self, mount: NSMount) -> &mut NSJail {
        self.mounts.push(mount);
        self
    }
}

impl Default for NSJail {
    fn default() -> NSJail {
        NSJail {
            mounts: Vec::new(),
            env: HashMap::new(),
            user: 1000,
            group: 1000,
        }
    }
}
