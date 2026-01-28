use std::fs;
use std::path::PathBuf;

pub fn colony_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Colony");
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local").join("share").join("colony");
        }
    }

    PathBuf::from(".")
}

pub fn colony_config_path() -> PathBuf {
    colony_data_dir().join("config").join("colony.toml")
}

pub fn legacy_config_path() -> PathBuf {
    PathBuf::from("config").join("colony.toml")
}

pub fn colony_cache_path() -> PathBuf {
    colony_data_dir()
        .join("cache")
        .join("github_etag_cache.json")
}

pub fn legacy_cache_path() -> PathBuf {
    PathBuf::from("config").join("github_etag_cache.json")
}

pub fn load_colony_config() -> Option<(PathBuf, String)> {
    let paths = [colony_config_path(), legacy_config_path()];
    for path in paths {
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                return Some((path, contents));
            }
        }
    }
    None
}
