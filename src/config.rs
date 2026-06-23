use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub persist_rules_on_exit: bool,
    pub blocked_pops: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            persist_rules_on_exit: false,
            blocked_pops: Vec::new(),
        }
    }
}

pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
}

#[cfg(unix)]
fn get_user_home_by_uid(uid: libc::uid_t) -> Option<PathBuf> {
    unsafe {
        let pwd = libc::getpwuid(uid);
        if !pwd.is_null() {
            let home_ptr = (*pwd).pw_dir;
            if !home_ptr.is_null() {
                let home_str = std::ffi::CStr::from_ptr(home_ptr).to_string_lossy();
                return Some(PathBuf::from(home_str.into_owned()));
            }
        }
    }
    None
}

impl ConfigManager {
    pub fn new() -> Self {
        let mut config_dir = None;

        #[cfg(unix)]
        {
            if let Ok(uid_str) = std::env::var("SUDO_UID") {
                if let Ok(uid) = uid_str.parse::<libc::uid_t>() {
                    if let Some(home) = get_user_home_by_uid(uid) {
                        config_dir = Some(home.join(".config").join("valve-server-manager"));
                    }
                }
            }
        }

        let config_dir = config_dir.unwrap_or_else(|| {
            ProjectDirs::from("com", "valve-server-manager", "ValveServerManager")
                .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        });

        let config_file = config_dir.join("settings.toml");

        Self {
            config_dir,
            config_file,
        }
    }

    #[cfg(test)]
    pub fn with_path(config_file: PathBuf) -> Self {
        let config_dir = config_file.parent().unwrap_or(&config_file).to_path_buf();
        Self {
            config_dir,
            config_file,
        }
    }

    pub fn load(&self) -> Settings {
        if !self.config_file.exists() {
            return Settings::default();
        }

        let mut file = match File::open(&self.config_file) {
            Ok(f) => f,
            Err(_) => return Settings::default(),
        };

        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            return Settings::default();
        }

        toml::from_str(&content).unwrap_or_else(|_| Settings::default())
    }

    pub fn save(&self, settings: &Settings) -> Result<(), std::io::Error> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)?;

            #[cfg(unix)]
            {
                if let (Ok(uid_str), Ok(gid_str)) = (std::env::var("SUDO_UID"), std::env::var("SUDO_GID")) {
                    if let (Ok(uid), Ok(gid)) = (uid_str.parse::<u32>(), gid_str.parse::<u32>()) {
                        let _ = std::os::unix::fs::chown(&self.config_dir, Some(uid), Some(gid));
                    }
                }
            }
        }

        let content = toml::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut file = File::create(&self.config_file)?;
        file.write_all(content.as_bytes())?;

        #[cfg(unix)]
        {
            if let (Ok(uid_str), Ok(gid_str)) = (std::env::var("SUDO_UID"), std::env::var("SUDO_GID")) {
                if let (Ok(uid), Ok(gid)) = (uid_str.parse::<u32>(), gid_str.parse::<u32>()) {
                    let _ = std::os::unix::fs::chown(&self.config_file, Some(uid), Some(gid));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load_default_when_missing() {
        let temp_file = std::env::temp_dir().join("non_existent_config.toml");
        let _ = fs::remove_file(&temp_file);
        
        let manager = ConfigManager::with_path(temp_file);
        let settings = manager.load();
        
        assert_eq!(settings.persist_rules_on_exit, false);
        assert!(settings.blocked_pops.is_empty());
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_file = std::env::temp_dir().join("vsm_test_config.toml");
        let _ = fs::remove_file(&temp_file);
        
        let manager = ConfigManager::with_path(temp_file.clone());
        
        let mut settings = Settings::default();
        settings.persist_rules_on_exit = true;
        settings.blocked_pops = vec!["sea".to_string(), "fra".to_string()];
        
        let save_res = manager.save(&settings);
        assert!(save_res.is_ok());
        
        let loaded = manager.load();
        assert_eq!(loaded.persist_rules_on_exit, true);
        assert_eq!(loaded.blocked_pops, vec!["sea".to_string(), "fra".to_string()]);
        
        let _ = fs::remove_file(&temp_file);
    }
}
