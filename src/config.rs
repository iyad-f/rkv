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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6380,
            max_clients: 1024,
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
            _ => None,
        }
    }

    /// Returns the `host:port` address the listener binds to.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
