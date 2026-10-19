//! Where Relay keeps its files.
//!
//! | OS    | Directory                                                  |
//! | ----- | ---------------------------------------------------------- |
//! | any   | `$RELAY_HOME` when set                                     |
//! | macOS | `~/Library/Application Support/Relay`                      |
//! | Linux | `$XDG_CONFIG_HOME/relay`, else `~/.config/relay`           |
//!
//! The directory holds `config.toml`, `local-key` (mode 0600) and
//! `usage.db` (with its WAL files).

use std::io;
use std::path::{Path, PathBuf};

/// File locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Root directory.
    pub dir: PathBuf,
    /// `config.toml`.
    pub config_file: PathBuf,
    /// Generated local client key.
    pub local_key_file: PathBuf,
    /// Usage database.
    pub usage_db: PathBuf,
}

impl Paths {
    /// Everything inside `dir`.
    pub fn in_dir(dir: &Path) -> Paths {
        Paths {
            dir: dir.to_path_buf(),
            config_file: dir.join("config.toml"),
            local_key_file: dir.join("local-key"),
            usage_db: dir.join("usage.db"),
        }
    }

    /// The current user's default location (see module docs).
    pub fn for_user() -> io::Result<Paths> {
        if let Some(home) = std::env::var_os("RELAY_HOME").filter(|v| !v.is_empty()) {
            return Ok(Paths::in_dir(Path::new(&home)));
        }
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        let dir = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Relay")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("relay")
        };
        Ok(Paths::in_dir(&dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_inside_dir() {
        let p = Paths::in_dir(Path::new("/x/relay"));
        assert_eq!(p.config_file, Path::new("/x/relay/config.toml"));
        assert_eq!(p.local_key_file, Path::new("/x/relay/local-key"));
        assert_eq!(p.usage_db, Path::new("/x/relay/usage.db"));
    }
}
