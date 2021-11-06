use std::path::PathBuf;
use std::process::Command;

use relative_path::RelativePathBuf;

#[derive(Debug)]
pub struct FirejailLauncher {
    firejail_exec: PathBuf,
    whitelists: Vec<PathBuf>,
    blacklists: Vec<PathBuf>,

    pub profile: Option<PathBuf>,
    pub private_tmp: bool,
    pub private_cache: bool,

    pub no3d: bool,
    pub nodbus: bool,
    pub nodvd: bool,
    pub nogroups: bool,
    pub nonewprivs: bool,
    pub noroot: bool,
    pub nosound: bool,
    pub novideo: bool,
    pub nou2f: bool,

    pub seccomp: bool,

    pub x11: Option<X11Sandbox>,
}

#[allow(dead_code)]
impl FirejailLauncher {
    pub fn command(self) -> Command {
        let mut cmd = Command::new(self.firejail_exec);

        self.whitelists
            .into_iter()
            .map(|x| format!("--whitelist={}", x.display()))
            .for_each(|x| {
                cmd.arg(x);
            });

        self.blacklists
            .into_iter()
            .map(|x| format!("--blacklist={}", x.display()))
            .for_each(|x| {
                cmd.arg(x);
            });

        if let Some(profile) = self.profile {
            let profile_path = format!("--profile={}", profile.display());
            cmd.arg(profile_path);
        } else {
            cmd.arg("--noprofile");
        }

        if let Some(x11) = self.x11 {
            cmd.arg(x11.flag());
        }

        macro_rules! apply_bool {
            ($flag: ident) => {
                apply_bool!($flag, format!("--{}", stringify!($flag)));
            };

            ($property: ident, $flag: expr) => {
                if self.$property {
                    cmd.arg($flag);
                }
            };
        }

        apply_bool!(private_tmp, "--private-tmp");
        apply_bool!(private_cache, "--private-cache");

        apply_bool!(no3d);
        apply_bool!(nodbus);
        apply_bool!(nodvd);
        apply_bool!(nogroups);
        apply_bool!(nonewprivs);
        apply_bool!(noroot);
        apply_bool!(nosound);
        apply_bool!(novideo);
        apply_bool!(nou2f);

        apply_bool!(seccomp);

        cmd
    }

    pub fn whitelist(&mut self, path: PathBuf) -> &mut FirejailLauncher {
        self.whitelists.push(path);
        self
    }

    pub fn blacklist(&mut self, path: PathBuf) -> &mut FirejailLauncher {
        self.blacklists.push(path);
        self
    }

    pub fn profile(&mut self, profile: PathBuf) -> &mut FirejailLauncher {
        self.profile = Some(profile);
        self
    }

    pub fn no_profile(&mut self) -> &mut FirejailLauncher {
        self.profile = None;
        self
    }

    pub fn private_tmp(&mut self, private: bool) -> &mut FirejailLauncher {
        self.private_tmp = private;
        self
    }
}

impl Default for FirejailLauncher {
    fn default() -> FirejailLauncher {
        FirejailLauncher {
            firejail_exec: PathBuf::from("/usr/bin/firejail"),
            whitelists: Vec::new(),
            blacklists: Vec::new(),

            profile: None,
            private_tmp: true,
            private_cache: true,

            no3d: false,
            nodbus: true,
            nodvd: true,
            nogroups: true,
            nonewprivs: true,
            noroot: true,
            nosound: false,
            novideo: false,
            nou2f: true,

            seccomp: true,

            x11: None,
        }
    }
}

// TODO Add ability to change sandbox
#[allow(dead_code)]
#[derive(Debug)]
pub enum X11Sandbox {
    DEFAULT,
    XEPHYR,
    XORG,
    XPRA,
    XVFB,
}

impl X11Sandbox {
    pub const fn flag(&self) -> &'static str {
        match self {
            Self::DEFAULT => "--x11",
            Self::XEPHYR => "--x11=xephyr",
            Self::XORG => "--x11=xorg",
            Self::XPRA => "--x11-xpra",
            Self::XVFB => "--x11-xvfb",
        }
    }
}

impl Default for X11Sandbox {
    fn default() -> X11Sandbox {
        Self::DEFAULT
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum WineType {
    SystemDefault,
    Embedded(RelativePathBuf),
    System(PathBuf),
}
