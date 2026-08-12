//! Tiny config: the list of default zones shown by bare `tz` (interactive).
//!
//! Stored as one city per line at $XDG_CONFIG_HOME/tz/zones (or ~/.config/tz/
//! zones). No dependency, no format ceremony — blank lines and `#` comments are
//! ignored. The file is only ever created when the user runs `tz --save ...`.

use std::path::PathBuf;

/// The tz config/state directory (XDG_CONFIG_HOME/tz or ~/.config/tz).
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tz"))
}

fn config_file() -> Option<PathBuf> {
    Some(config_dir()?.join("zones"))
}

/// Load the user's saved default city tokens. Empty if no config exists.
pub fn load() -> Vec<String> {
    let Some(path) = config_file() else {
        return Vec::new();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Save default city tokens, creating the config dir if needed.
pub fn save(cities: &[String]) -> std::io::Result<PathBuf> {
    let path = config_file()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = format!(
        "# tz default zones — one city per line. Edit freely.\n{}\n",
        cities.join("\n")
    );
    std::fs::write(&path, body)?;
    Ok(path)
}
