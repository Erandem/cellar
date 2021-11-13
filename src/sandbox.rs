pub mod firejail;

#[derive(Debug, Clone)]
pub enum EnvVar {
    /// Uses the env var in the environment when calling a command which would use it
    Pass(String),
    KeyValue(String, String),
}

impl EnvVar {
    pub fn to_key_value(self) -> (String, String) {
        match self {
            EnvVar::Pass(k) => {
                let val = std::env::var(&k).expect("failed to get env var");
                (k, val)
            }
            EnvVar::KeyValue(k, v) => (k, v),
        }
    }
}

impl Into<EnvVar> for &'static str {
    fn into(self) -> EnvVar {
        EnvVar::Pass(self.to_string())
    }
}

impl<K, V> Into<EnvVar> for (K, V)
where
    K: Sized + Into<String>,
    V: Sized + Into<String>,
{
    fn into(self) -> EnvVar {
        EnvVar::KeyValue(self.0.into(), self.1.into())
    }
use self::firejail::{FirejailLauncher, X11Sandbox};
