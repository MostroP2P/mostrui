use crate::SETTINGS;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub mostro_pubkey: String,
    pub relays: Vec<String>,
    pub log_level: String,
}

#[cfg(windows)]
fn has_trailing_slash(p: &Path) -> bool {
    let last = p.as_os_str().encode_wide().last();
    last == Some(b'\\' as u16) || last == Some(b'/' as u16)
}
#[cfg(unix)]
fn has_trailing_slash(p: &Path) -> bool {
    p.as_os_str().as_bytes().last() == Some(&b'/')
}

fn add_trailing_slash(p: &mut PathBuf) {
    let fname = p.file_name();
    let dirname = if let Some(fname) = fname {
        let mut s = OsString::with_capacity(fname.len() + 1);
        s.push(fname);
        if cfg!(windows) {
            s.push("\\");
        } else {
            s.push("/");
        }
        s
    } else {
        OsString::new()
    };

    if p.pop() {
        p.push(dirname);
    }
}
impl Settings {
    pub fn new(mut config_path: PathBuf) -> Result<Self, ConfigError> {
        let file_name = {
            if !has_trailing_slash(config_path.as_path()) {
                add_trailing_slash(&mut config_path);
                let tmp = format!("{}settings.toml", config_path.display());
                tmp
            } else {
                format!("{}settings.toml", config_path.display())
            }
        };
        if !Path::new(&file_name).exists() {
            println!("Settings file not found: {}", file_name);
            return Err(ConfigError::NotFound(file_name));
        }
        let s = Config::builder()
            .add_source(File::with_name(&file_name).required(true))
            // Add in settings from the environment (with a prefix of APP)
            // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(Environment::with_prefix("app"))
            .build()?;

        // You can deserialize the entire configuration as
        s.try_deserialize()
    }

    pub fn get() -> Self {
        SETTINGS.get().unwrap().clone()
    }
}

pub fn init_global_settings(settings: Settings) {
    SETTINGS.set(settings).unwrap()
}

pub fn get_settings_path() -> String {
    let home_dir = env::var("HOME").expect("Couldn't get HOME directory");
    let settings_path = format!("{}/.mostrui", home_dir);
    if !Path::new(&settings_path).exists() {
        fs::create_dir(&settings_path).expect("Couldn't create mostrui directory");
        println!("Directory {} created.", settings_path);
    }

    settings_path
}
