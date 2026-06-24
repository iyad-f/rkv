// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Server configuration.
//!
//! [`Config`] gathers every tunable setting in one place. Values start from
//! [`Config::default`], the built-in defaults, and are then overridden by a
//! config file and command-line arguments, in that order.

/// The server's runtime configuration.
pub struct Config {
    /// The host the listener binds to.
    pub host: String,

    /// The TCP port the server listens on.
    pub port: u16,

    /// The maximum number of clients.
    pub max_clients: usize,

    /// The working directory for persistence files.
    pub dir: String,

    /// The password clients must authenticate with, or `None` when no password
    /// is required.
    pub password: Option<String>,

    /// The append-only file settings.
    pub aof: AofConfig,
}

/// The append-only file settings.
pub struct AofConfig {
    /// Whether changes are logged to the append-only file.
    pub enabled: bool,

    /// The name of the append-only file within [`Config::dir`].
    pub file_name: String,

    /// How often the append-only file is flushed to disk.
    pub fsync: FsyncPolicy,

    /// The percentage the file may grow beyond its size after the last rewrite
    /// before an automatic rewrite triggers. `None` disables automatic rewrites.
    pub auto_rewrite_percentage: Option<u64>,

    /// The minimum file size, in bytes, before an automatic rewrite triggers.
    pub auto_rewrite_min_size: u64,
}

/// When the append-only file is flushed to disk.
#[derive(Clone, Copy)]
pub enum FsyncPolicy {
    /// Flush after every write.
    Always,

    /// Flush at most once per second.
    EverySec,

    /// Never flush explicitly, leaving it to the operating system.
    No,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6380,
            max_clients: 1024,
            dir: ".".to_string(),
            password: None,
            aof: AofConfig::default(),
        }
    }
}

impl Default for AofConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            file_name: "rkv.aof".to_string(),
            fsync: FsyncPolicy::EverySec,
            auto_rewrite_percentage: Some(100),
            auto_rewrite_min_size: 64 * 1024 * 1024,
        }
    }
}

/// An error encountered while loading a [`Config`].
#[derive(Debug)]
pub enum Error {
    /// A config file could not be read.
    Io(std::io::Error),

    /// A directive name was not recognized.
    UnknownKey(String),

    /// A directive was given without a value.
    MissingValue(String),

    /// A directive's value could not be parsed into its field.
    InvalidValue { key: String, value: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::UnknownKey(key) => write!(f, "unknown config key '{key}'"),
            Self::MissingValue(key) => write!(f, "missing value for config key '{key}'"),
            Self::InvalidValue { key, value } => {
                write!(f, "invalid value '{value}' for config key '{key}'")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl Config {
    /// Builds a [`Config`] from the defaults, then an optional config file, then
    /// command-line overrides, each layer winning over the last.
    ///
    /// The invocation is `rkv [configfile] [--key value ...]`.
    pub fn load() -> Result<Self, Error> {
        let mut config = Config::default();
        let mut args = std::env::args().skip(1).peekable();

        // An optional config-file path may come first, before any --flags.
        if let Some(first) = args.peek()
            && !first.starts_with("--")
        {
            let path = args.next().unwrap();
            config.apply_file(&path)?;
        }

        // --key value pairs override whatever the file set.
        while let Some(flag) = args.next() {
            let key = flag
                .strip_prefix("--")
                .ok_or_else(|| Error::UnknownKey(flag.clone()))?;
            let value = args
                .next()
                .ok_or_else(|| Error::MissingValue(key.to_string()))?;
            config.set(key, &value)?;
        }

        Ok(config)
    }

    /// Reads a config file and applies each `key value` directive, ignoring
    /// blank lines and `#` comments.
    fn apply_file(&mut self, path: &str) -> Result<(), Error> {
        let contents = std::fs::read_to_string(path)?;

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (key, value) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| Error::MissingValue(line.to_string()))?;
            self.set(key.trim(), value.trim())?;
        }

        Ok(())
    }

    /// Applies a single `key value` directive, parsing `value` into the field
    /// the key names.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Error> {
        match key {
            "bind" => self.host = value.to_string(),
            "port" => {
                self.port = value.parse().map_err(|_| Error::InvalidValue {
                    key: key.to_string(),
                    value: value.to_string(),
                })?;
            }
            "maxclients" => {
                self.max_clients = value.parse().map_err(|_| Error::InvalidValue {
                    key: key.to_string(),
                    value: value.to_string(),
                })?;
            }
            "appendonly" => {
                self.aof.enabled = match value {
                    "yes" => true,
                    "no" => false,
                    _ => {
                        return Err(Error::InvalidValue {
                            key: key.to_string(),
                            value: value.to_string(),
                        });
                    }
                };
            }
            "dir" => self.dir = value.to_string(),
            "requirepass" => self.password = (!value.is_empty()).then(|| value.to_string()),
            "appendfilename" => self.aof.file_name = value.to_string(),
            "appendfsync" => {
                self.aof.fsync = match value {
                    "always" => FsyncPolicy::Always,
                    "everysec" => FsyncPolicy::EverySec,
                    "no" => FsyncPolicy::No,
                    _ => {
                        return Err(Error::InvalidValue {
                            key: key.to_string(),
                            value: value.to_string(),
                        });
                    }
                };
            }
            "auto-aof-rewrite-percentage" => {
                let percentage: u64 = value.parse().map_err(|_| Error::InvalidValue {
                    key: key.to_string(),
                    value: value.to_string(),
                })?;
                self.aof.auto_rewrite_percentage = (percentage != 0).then_some(percentage);
            }
            "auto-aof-rewrite-min-size" => {
                self.aof.auto_rewrite_min_size =
                    value.parse().map_err(|_| Error::InvalidValue {
                        key: key.to_string(),
                        value: value.to_string(),
                    })?;
            }
            _ => return Err(Error::UnknownKey(key.to_string())),
        }

        Ok(())
    }

    /// Returns the value of directive `key`, or `None` if it is unknown.
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "bind" => Some(self.host.clone()),
            "port" => Some(self.port.to_string()),
            "maxclients" => Some(self.max_clients.to_string()),
            "appendonly" => Some(if self.aof.enabled { "yes" } else { "no" }.to_string()),
            "dir" => Some(self.dir.clone()),
            "requirepass" => Some(self.password.clone().unwrap_or_default()),
            "appendfilename" => Some(self.aof.file_name.clone()),
            "appendfsync" => Some(
                match self.aof.fsync {
                    FsyncPolicy::Always => "always",
                    FsyncPolicy::EverySec => "everysec",
                    FsyncPolicy::No => "no",
                }
                .to_string(),
            ),
            "auto-aof-rewrite-percentage" => {
                Some(self.aof.auto_rewrite_percentage.unwrap_or(0).to_string())
            }
            "auto-aof-rewrite-min-size" => Some(self.aof.auto_rewrite_min_size.to_string()),
            _ => None,
        }
    }

    /// Returns the `host:port` address the listener binds to.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Returns the full path to the append-only file.
    pub fn aof_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.dir).join(&self.aof.file_name)
    }
}
