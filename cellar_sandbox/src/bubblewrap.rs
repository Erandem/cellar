use std::path::PathBuf;
use std::process::Command;

use crate::EnvVar;

#[derive(Debug, Clone)]
pub enum BubMount {
    DevBind { src: PathBuf, dest: PathBuf },

    BindRO { src: PathBuf, dest: PathBuf },
    BindRW { src: PathBuf, dest: PathBuf },

    Symlink { src: PathBuf, dest: PathBuf },

    TmpFs { path: PathBuf },
    Proc { path: PathBuf },

    Dir { path: PathBuf },
    File { content: String, path: PathBuf },
}

#[allow(dead_code)]
impl BubMount {
    /// A dev_bind allows the sandbox to access the device, not just the filesystem.
    pub fn dev_bind<T: Into<PathBuf>, E: Into<PathBuf>>(src: T, dest: E) -> BubMount {
        BubMount::DevBind {
            src: src.into(),
            dest: dest.into(),
        }
    }

    pub fn bind_ro<T: Into<PathBuf>, E: Into<PathBuf>>(src: T, dest: E) -> BubMount {
        BubMount::BindRO {
            src: src.into(),
            dest: dest.into(),
        }
    }

    pub fn bind_rw<T: Into<PathBuf>, E: Into<PathBuf>>(src: T, dest: E) -> BubMount {
        BubMount::BindRW {
            src: src.into(),
            dest: dest.into(),
        }
    }

    pub fn tmpfs<T: Into<PathBuf>>(dest: T) -> BubMount {
        BubMount::TmpFs { path: dest.into() }
    }

    pub fn proc<T: Into<PathBuf>>(dest: T) -> BubMount {
        BubMount::Proc { path: dest.into() }
    }

    pub fn symlink<T: Into<PathBuf>, E: Into<PathBuf>>(src: T, dest: E) -> BubMount {
        BubMount::Symlink {
            src: src.into(),
            dest: dest.into(),
        }
    }

    pub fn dir<T: Into<PathBuf>>(path: T) -> BubMount {
        BubMount::Dir { path: path.into() }
    }

    pub fn file<T: Into<String>, P: Into<PathBuf>>(content: T, path: P) -> BubMount {
        BubMount::File {
            content: content.into(),
            path: path.into(),
        }
    }

    fn apply_arg(&self, cmd: &mut Command) {
        match self {
            BubMount::DevBind { src, dest } => cmd.arg("--dev-bind").arg(src).arg(dest),
            BubMount::BindRO { src, dest } => cmd.arg("--ro-bind").arg(src).arg(dest),
            BubMount::BindRW { src, dest } => cmd.arg("--bind").arg(src).arg(dest),
            BubMount::Symlink { src, dest } => cmd.arg("--symlink").arg(src).arg(dest),
            BubMount::TmpFs { path } => cmd.arg("--tmpfs").arg(path),
            BubMount::Proc { path } => cmd.arg("--proc").arg(path),

            BubMount::Dir { path } => cmd.arg("--dir").arg(path),
            BubMount::File { content, path } => cmd.arg("--file").arg(content).arg(path),
        };
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BubLauncher {
    mounts: Vec<BubMount>,
    env: Vec<EnvVar>,
    inherit_env: bool,

    unshare_user: bool,
    unshare_ipc: bool,
    unshare_pid: bool,
    unshare_net: bool,
    unshare_uts: bool,
    unshare_cgroups: bool,

    as_pid_1: bool,
    new_session: bool,
    die_with_parent: bool,

    hostname: Option<String>,
    uid: Option<usize>,
    gid: Option<usize>,
}

#[allow(dead_code)]
impl BubLauncher {
    pub fn command(self) -> Command {
        let mut cmd = Command::new("/usr/bin/bwrap");

        // This might be totally pointless, but I wanted to get more familiar with macros
        macro_rules! bool_opt {
            ($prop:expr, when false $arg_if_false:expr) => {
                if !$prop {
                    cmd.arg($arg_if_false);
                }
            };

            ($prop:expr, when true $arg_if_true:expr) => {
                if $prop {
                    cmd.arg($arg_if_true);
                }
            };
        }

        self.mounts.into_iter().for_each(|x| x.apply_arg(&mut cmd));

        // Since bwrap applies arguments in the order they're passed, we have to do this before we
        // set any other env vars otherwise they're also cleared
        bool_opt!(self.inherit_env, when false "--clearenv");

        self.env
            .into_iter()
            .map(EnvVar::to_key_value)
            .for_each(|x| {
                cmd.arg("--setenv").arg(x.0).arg(x.1);
            });

        bool_opt!(self.unshare_user, when true "--unshare-user");
        bool_opt!(self.unshare_ipc, when true "--unshare-ipc");
        bool_opt!(self.unshare_pid, when true "--unshare-pid");
        bool_opt!(self.unshare_net, when true "--unshare-net");
        bool_opt!(self.unshare_uts, when true "--unshare-uts");
        bool_opt!(self.unshare_cgroups, when true "--unshare-cgroup");

        bool_opt!(self.as_pid_1, when true "--as-pid-1");
        bool_opt!(self.new_session, when true "--new-session");
        bool_opt!(self.die_with_parent, when true "--die-with-parent");

        cmd
    }

    pub fn mount(&mut self, mount: BubMount) -> &mut BubLauncher {
        self.mounts.push(mount);
        self
    }

    /// # Order
    /// Environmental modifications are applied in the order that they're added onto the struct.
    /// This means that if you add the env var `derp` then call `env_unset` it will be removed.
    pub fn env<T: Into<EnvVar>>(&mut self, var: T) -> &mut BubLauncher {
        self.env.push(var.into());
        self
    }

    /// Inherits the environmental variables of the process
    /// Any envrironmental variables set will overwrite the calling environment
    pub fn inherit_env(&mut self, inherit: bool) -> &mut BubLauncher {
        self.inherit_env = inherit;
        self
    }

    pub fn hostname<T: Into<String>>(&mut self, hostname: T) -> &mut BubLauncher {
        self.hostname = Some(hostname.into());
        self
    }

    pub fn no_hostname<T: Into<String>>(&mut self) -> &mut BubLauncher {
        self.hostname = None;
        self
    }
}

impl Default for BubLauncher {
    fn default() -> BubLauncher {
        BubLauncher {
            mounts: Vec::new(),
            env: Vec::new(),
            inherit_env: false,

            unshare_user: false,
            unshare_ipc: false,
            unshare_pid: true,
            unshare_net: true,
            unshare_uts: true,
            unshare_cgroups: true,

            as_pid_1: true,
            new_session: true,
            die_with_parent: true,

            hostname: None,
            uid: None,
            gid: None,
        }
    }
}
