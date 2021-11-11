use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// We write to Vec<u8>s a lot, and that trait likes to return an error. This type
/// is, sort of, a way to make that easier for us to handle since we can assume that
/// it is always okay.
type VecWrite = Result<Vec<u8>, std::io::Error>;

#[derive(Debug, Serialize, Deserialize)]
pub struct NSMount {
    r#type: NSMountType,
    readwrite: bool,
    mandatory: bool,
    noexec: bool,
}

impl NSMount {
    pub fn proc() -> NSMount {
        NSMount {
            r#type: NSMountType::ProcFs,
            readwrite: false,
            mandatory: true,
            noexec: true,
        }
    }

    /// Creates a new bindmount with `readwrite` set to `false`
    pub fn readonly<T: Into<PathBuf>>(src: T, dest: T) -> NSMount {
        let mut s = Self::bind(src, dest);
        s.readwrite = false;
        s
    }

    /// Creates a new bindmount that is both readwrite and mandatory
    pub fn bind<T: Into<PathBuf>, E: Into<PathBuf>>(src: T, dest: E) -> NSMount {
        NSMount {
            r#type: NSMountType::BindMount {
                src: src.into(),
                dest: dest.into(),
            },

            readwrite: true,
            mandatory: true,
            noexec: false,
        }
    }

    pub fn temp<T: Into<PathBuf>>(dest: T) -> NSMount {
        NSMount {
            r#type: NSMountType::TmpFs { dest: dest.into() },

            readwrite: true,
            mandatory: true,
            noexec: false,
        }
    }

    #[allow(dead_code)]
    pub fn make_readonly(&mut self) -> &mut NSMount {
        self.readwrite = false;
        self
    }

    #[allow(dead_code)]
    pub fn make_readwrite(&mut self) -> &mut NSMount {
        self.readwrite = true;
        self
    }

    #[allow(dead_code)]
    pub fn mandatory(&mut self) -> &mut NSMount {
        self.mandatory = true;
        self
    }

    #[allow(dead_code)]
    pub fn not_mandatory(&mut self) -> &mut NSMount {
        self.mandatory = false;
        self
    }

    fn into_proto_text(self) -> VecWrite {
        let mut f = Vec::new();
        write!(&mut f, "mount {{ ")?;
        write!(&mut f, "rw: {} ", self.readwrite)?;
        write!(&mut f, "mandatory: {} ", self.mandatory)?;
        write!(&mut f, "noexec: {} ", self.noexec)?;

        match self.r#type {
            NSMountType::BindMount { src, dest } => {
                write!(&mut f, "src: \"{}\" ", src.display())?;
                write!(&mut f, "dst: \"{}\" ", dest.display())?;
                write!(&mut f, "is_bind: true ")?;
            }
            NSMountType::TmpFs { dest } => {
                write!(&mut f, "dst: \"{}\" ", dest.display())?;
                write!(&mut f, "fstype: \"tmpfs\" ")?;
            }
            NSMountType::ProcFs => {
                write!(&mut f, "dst: \"/proc\"")?;
                write!(&mut f, "fstype: \"proc\"")?;
            }
        }

        write!(&mut f, "}}\n")?;

        Ok(f)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NSMountType {
    BindMount { src: PathBuf, dest: PathBuf },
    TmpFs { dest: PathBuf },
    ProcFs,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NSSymlink {
    src: PathBuf,
    dest: PathBuf,
}

impl NSSymlink {
    pub fn new<T: Into<PathBuf>>(src: T, dest: T) -> NSSymlink {
        NSSymlink {
            src: src.into(),
            dest: dest.into(),
        }
    }

    fn into_proto_text(self) -> VecWrite {
        let mut f = Vec::new();
        write!(&mut f, "mount {{ ")?;
        write!(&mut f, "src: \"{}\" ", self.src.display())?;
        write!(&mut f, "dst: \"{}\" ", self.dest.display())?;
        write!(&mut f, "is_symlink: true ")?;
        write!(&mut f, "}}\n")?;
        Ok(f)
    }
}

/// Takes the input as (src: PathBuf, dest: PathBuf)
impl Into<NSSymlink> for (PathBuf, PathBuf) {
    fn into(self) -> NSSymlink {
        NSSymlink::new(self.0, self.1)
    }
}

impl Into<NSSymlink> for (&'static str, &'static str) {
    fn into(self) -> NSSymlink {
        NSSymlink::new(PathBuf::from(self.0), PathBuf::from(self.1))
    }
}

pub struct NSJail {
    mounts: Vec<NSMount>,
    links: Vec<NSSymlink>,

    env: Vec<NSEnvVar>,
    user: u64,
    group: u64,
}

#[allow(dead_code)]
impl NSJail {
    pub fn command(self) -> Command {
        let mut cmd = Command::new("/usr/bin/nsjail");
        let mut f = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open("./jail.proto")
            .unwrap();

        cmd.arg("--config").arg("./jail.proto");
        cmd.arg("--user").arg(self.user.to_string());
        cmd.arg("--group").arg(self.group.to_string());

        self.mounts
            .into_iter()
            .map(NSMount::into_proto_text)
            .map(Result::unwrap)
            .for_each(|x| {
                f.write_all(&x).unwrap();
            });

        self.links
            .into_iter()
            .map(NSSymlink::into_proto_text)
            .map(Result::unwrap)
            .for_each(|x| {
                f.write_all(&x).unwrap();
            });

        self.env.into_iter().map(NSEnvVar::to_arg).for_each(|x| {
            cmd.arg(x.0).arg(x.1);
        });

        // TODO Make hostname support stuff work properly
        cmd.arg("--hostname").arg("vanilla");
        cmd.arg("--disable_rlimits");
        cmd.arg("--disable_no_new_privs");
        cmd.arg("--keep_caps");

        // Make sure that the caller can pass arguments without worry
        cmd.arg("--");
        cmd
    }

    pub fn env<T: Into<NSEnvVar>>(&mut self, var: T) -> &mut NSJail {
        self.env.push(var.into());
        self
    }

    pub fn mount(&mut self, mount: NSMount) -> &mut NSJail {
        self.mounts.push(mount);
        self
    }

    pub fn symlink<T: Into<NSSymlink>>(&mut self, link: T) -> &mut NSJail {
        self.links.push(link.into());
        self
    }
}

impl Default for NSJail {
    fn default() -> NSJail {
        NSJail {
            mounts: vec![NSMount::proc()],
            links: Vec::new(),

            env: Vec::new(),
            user: 1000,
            group: 984,
        }
    }
}

pub enum NSEnvVar {
    Set(String, String),
    Keep(String),
}

impl NSEnvVar {
    fn to_arg(self) -> (&'static str, String) {
        match self {
            NSEnvVar::Keep(s) => ("--env", s),
            NSEnvVar::Set(key, value) => ("--env", format!("{}={}", key, value)),
        }
    }
}

impl Into<NSEnvVar> for String {
    fn into(self) -> NSEnvVar {
        NSEnvVar::Keep(self)
    }
}

impl Into<NSEnvVar> for &'static str {
    fn into(self) -> NSEnvVar {
        self.to_string().into()
    }
}

impl<K, V> Into<NSEnvVar> for (K, V)
where
    K: Into<String> + Sized,
    V: Into<String> + Sized,
{
    fn into(self) -> NSEnvVar {
        NSEnvVar::Set(self.0.into(), self.1.into())
    }
}
