//! Durable configuration. Every change is validated first and written
//! atomically (temp file + rename) with mode 0600. The file holds secret
//! *references* only.

use crate::paths::Paths;
use relay_core::config::{AccountConfig, Config, ConfigError};
use std::fmt;
use std::io::Write;
use std::path::Path;

/// Why loading or saving failed.
#[derive(Debug)]
pub enum StoreError {
    /// File system error.
    Io(std::io::Error),
    /// The file is not valid TOML for [`Config`].
    Parse(String),
    /// The configuration breaks an invariant.
    Invalid(Vec<ConfigError>),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::Parse(e) => write!(f, "config is not valid: {e}"),
            StoreError::Invalid(errs) => {
                let msgs: Vec<String> = errs.iter().map(|e| e.0.clone()).collect();
                write!(f, "{}", msgs.join("; "))
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// Reads a config file. A missing file is the default (empty) config.
pub fn load(path: &Path) -> Result<Config, StoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let cfg: Config =
                toml::from_str(&text).map_err(|e| StoreError::Parse(e.to_string()))?;
            cfg.validate().map_err(StoreError::Invalid)?;
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e.into()),
    }
}

/// Writes `cfg` atomically after validating it.
pub fn save(path: &Path, cfg: &Config) -> Result<(), StoreError> {
    cfg.validate().map_err(StoreError::Invalid)?;
    let text = toml::to_string_pretty(cfg).map_err(|e| StoreError::Parse(e.to_string()))?;
    write_private(path, text.as_bytes())?;
    Ok(())
}

/// Writes a file readable only by the owner, atomically.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// Config file plus its in-memory copy.
pub struct ConfigStore {
    path: std::path::PathBuf,
    config: Config,
}

impl ConfigStore {
    /// Loads (or starts empty).
    pub fn open(paths: &Paths) -> Result<ConfigStore, StoreError> {
        Ok(ConfigStore {
            path: paths.config_file.clone(),
            config: load(&paths.config_file)?,
        })
    }

    /// Current config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Applies `edit` to a copy; saves and keeps it only if valid.
    pub fn update(&mut self, edit: impl FnOnce(&mut Config)) -> Result<(), StoreError> {
        let mut next = self.config.clone();
        edit(&mut next);
        save(&self.path, &next)?;
        self.config = next;
        Ok(())
    }

    /// Adds an account. Fails on a duplicate id or any invalid field.
    pub fn add_account(&mut self, account: AccountConfig) -> Result<(), StoreError> {
        self.update(|c| c.accounts.push(account))
    }

    /// Replaces the account with the same id.
    pub fn replace_account(&mut self, account: AccountConfig) -> Result<(), StoreError> {
        if self.config.account(account.id.as_str()).is_none() {
            return Err(unknown(account.id.as_str()));
        }
        self.update(|c| {
            if let Some(a) = c.accounts.iter_mut().find(|a| a.id == account.id) {
                *a = account;
            }
        })
    }

    /// Removes an account.
    pub fn remove_account(&mut self, id: &str) -> Result<(), StoreError> {
        if self.config.account(id).is_none() {
            return Err(unknown(id));
        }
        self.update(|c| c.accounts.retain(|a| a.id.as_str() != id))
    }

    /// Pauses or resumes an account.
    pub fn set_paused(&mut self, id: &str, paused: bool) -> Result<(), StoreError> {
        if self.config.account(id).is_none() {
            return Err(unknown(id));
        }
        self.update(|c| {
            if let Some(a) = c.accounts.iter_mut().find(|a| a.id.as_str() == id) {
                a.paused = paused;
            }
        })
    }
}

fn unknown(id: &str) -> StoreError {
    StoreError::Invalid(vec![ConfigError(format!("no account with id {id:?}"))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::model::{AccountId, AccountMode, Provider};

    fn account(id: &str) -> AccountConfig {
        AccountConfig {
            id: AccountId(id.into()),
            provider: Provider::Anthropic,
            vendor: None,
            device_id: None,
            label: "Personal".into(),
            mode: AccountMode::Observed,
            plan: Some("Max 20×".into()),
            identifier: Some("d••@example.com".into()),
            credential_kind: None,
            upstream: None,
            credential: None,
            paused: false,
            max_in_flight: 8,
            models: vec![],
        }
    }

    #[test]
    fn round_trip_and_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        let mut s = ConfigStore::open(&paths).unwrap();
        assert!(s.config().accounts.is_empty());
        s.add_account(account("personal")).unwrap();
        s.set_paused("personal", true).unwrap();
        let reloaded = ConfigStore::open(&paths).unwrap();
        assert_eq!(reloaded.config().accounts.len(), 1);
        assert!(reloaded.config().accounts[0].paused);
        assert_eq!(
            reloaded.config().accounts[0].plan.as_deref(),
            Some("Max 20×")
        );
        s.remove_account("personal").unwrap();
        assert!(
            ConfigStore::open(&paths)
                .unwrap()
                .config()
                .accounts
                .is_empty()
        );
    }

    #[test]
    fn invalid_change_is_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        let mut s = ConfigStore::open(&paths).unwrap();
        s.add_account(account("a")).unwrap();
        assert!(s.add_account(account("a")).is_err());
        assert!(s.set_paused("missing", true).is_err());
        assert_eq!(
            ConfigStore::open(&paths).unwrap().config().accounts.len(),
            1
        );
        assert_eq!(s.config().accounts.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        ConfigStore::open(&paths)
            .unwrap()
            .add_account(account("a"))
            .unwrap();
        let mode = std::fs::metadata(&paths.config_file)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[cfg(test)]
mod example {
    /// The shipped example must stay loadable.
    #[test]
    fn config_example_is_valid() {
        let text = include_str!("../../../config.example.toml");
        let cfg: relay_core::config::Config = toml::from_str(text).unwrap();
        cfg.validate().unwrap();
        assert_eq!(cfg.accounts.len(), 2);
    }
}
