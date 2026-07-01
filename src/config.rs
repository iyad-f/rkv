// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Server configuration.
//!
//! [`Config`] gathers every tunable setting in one place. Values start from
//! [`Config::default`], the built-in defaults, and are then overridden by a
//! config file and command-line arguments, in that order.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::LazyLock;

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

    /// The lazy-drop settings.
    pub lazy_drop: LazyDropConfig,
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

/// The lazy-drop settings, controlling which deletions drop their values on the
/// background worker instead of inline.
pub struct LazyDropConfig {
    /// Whether `DEL` drops removed values lazily.
    pub user_del: bool,

    /// Whether expired keys are dropped lazily.
    pub expire: Rc<Cell<bool>>,

    /// Whether values a write replaces are dropped lazily.
    pub server_del: Rc<Cell<bool>>,
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

impl std::str::FromStr for FsyncPolicy {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "always" => Ok(Self::Always),
            "everysec" => Ok(Self::EverySec),
            "no" => Ok(Self::No),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for FsyncPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Always => "always",
            Self::EverySec => "everysec",
            Self::No => "no",
        })
    }
}

/// Parses a `yes`/`no` setting value into a boolean.
fn parse_bool(value: &str) -> Result<bool, ()> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(()),
    }
}

/// Formats a boolean as a `yes`/`no` setting value.
fn format_bool(value: bool) -> String {
    if value { "yes" } else { "no" }.to_string()
}

/// Parses a memory size into bytes.
fn parse_memory(value: &str) -> Result<u64, ()> {
    let split = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let (digits, unit) = value.split_at(split);

    let number: u64 = digits.parse().map_err(|_| ())?;
    let multiplier: u64 = match unit.to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" => 1_000,
        "kb" => 1_024,
        "m" => 1_000_000,
        "mb" => 1_048_576,
        "g" => 1_000_000_000,
        "gb" => 1_073_741_824,
        _ => return Err(()),
    };

    number.checked_mul(multiplier).ok_or(())
}

/// The kind of value a setting expects.
#[derive(Debug)]
pub enum ValueKind {
    /// An integer.
    Integer,

    /// A memory size, optionally suffixed with a `k`/`m`/`g` or `kb`/`mb`/`gb`
    /// unit.
    Memory,

    /// A `yes`/`no` boolean.
    Bool,

    /// One of the held set of values.
    Enum(&'static [&'static str]),
}

/// Describes one configurable setting.
struct Setting {
    /// The setting name.
    name: &'static str,

    /// Parses `value` into the field this setting controls, naming the kind of
    /// value expected on failure.
    set: fn(&mut Config, &str) -> Result<(), ValueKind>,

    /// Formats the field this setting controls.
    get: fn(&Config) -> String,
}

/// Every configurable setting.
const SETTINGS: &[Setting] = &[
    Setting {
        name: "bind",
        set: |config, value| {
            config.host = value.to_string();
            Ok(())
        },
        get: |config| config.host.clone(),
    },
    Setting {
        name: "port",
        set: |config, value| {
            config.port = value.parse().map_err(|_| ValueKind::Integer)?;
            Ok(())
        },
        get: |config| config.port.to_string(),
    },
    Setting {
        name: "maxclients",
        set: |config, value| {
            config.max_clients = value.parse().map_err(|_| ValueKind::Integer)?;
            Ok(())
        },
        get: |config| config.max_clients.to_string(),
    },
    Setting {
        name: "dir",
        set: |config, value| {
            config.dir = value.to_string();
            Ok(())
        },
        get: |config| config.dir.clone(),
    },
    Setting {
        name: "requirepass",
        set: |config, value| {
            config.password = (!value.is_empty()).then(|| value.to_string());
            Ok(())
        },
        get: |config| config.password.clone().unwrap_or_default(),
    },
    Setting {
        name: "appendonly",
        set: |config, value| {
            config.aof.enabled = parse_bool(value).map_err(|_| ValueKind::Bool)?;
            Ok(())
        },
        get: |config| format_bool(config.aof.enabled),
    },
    Setting {
        name: "appendfilename",
        set: |config, value| {
            config.aof.file_name = value.to_string();
            Ok(())
        },
        get: |config| config.aof.file_name.clone(),
    },
    Setting {
        name: "appendfsync",
        set: |config, value| {
            config.aof.fsync = value
                .parse()
                .map_err(|_| ValueKind::Enum(&["everysec", "always", "no"]))?;
            Ok(())
        },
        get: |config| config.aof.fsync.to_string(),
    },
    Setting {
        name: "auto-aof-rewrite-percentage",
        set: |config, value| {
            let percentage: u64 = value.parse().map_err(|_| ValueKind::Integer)?;
            config.aof.auto_rewrite_percentage = (percentage != 0).then_some(percentage);
            Ok(())
        },
        get: |config| config.aof.auto_rewrite_percentage.unwrap_or(0).to_string(),
    },
    Setting {
        name: "auto-aof-rewrite-min-size",
        set: |config, value| {
            config.aof.auto_rewrite_min_size =
                parse_memory(value).map_err(|_| ValueKind::Memory)?;
            Ok(())
        },
        get: |config| config.aof.auto_rewrite_min_size.to_string(),
    },
    Setting {
        name: "lazyfree-lazy-user-del",
        set: |config, value| {
            config.lazy_drop.user_del = parse_bool(value).map_err(|_| ValueKind::Bool)?;
            Ok(())
        },
        get: |config| format_bool(config.lazy_drop.user_del),
    },
    Setting {
        name: "lazyfree-lazy-expire",
        set: |config, value| {
            config
                .lazy_drop
                .expire
                .set(parse_bool(value).map_err(|_| ValueKind::Bool)?);
            Ok(())
        },
        get: |config| format_bool(config.lazy_drop.expire.get()),
    },
    Setting {
        name: "lazyfree-lazy-server-del",
        set: |config, value| {
            config
                .lazy_drop
                .server_del
                .set(parse_bool(value).map_err(|_| ValueKind::Bool)?);
            Ok(())
        },
        get: |config| format_bool(config.lazy_drop.server_del.get()),
    },
];

/// Maps each setting name to its [`Setting`].
static SETTING_TABLE: LazyLock<HashMap<&'static str, &'static Setting>> = LazyLock::new(|| {
    SETTINGS
        .iter()
        .map(|setting| (setting.name, setting))
        .collect()
});

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6380,
            max_clients: 1024,
            dir: ".".to_string(),
            password: None,
            aof: AofConfig::default(),
            lazy_drop: LazyDropConfig::default(),
        }
    }
}

impl Default for LazyDropConfig {
    fn default() -> Self {
        Self {
            user_del: false,
            expire: Rc::new(Cell::new(false)),
            server_del: Rc::new(Cell::new(false)),
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

    /// A setting name was not recognized.
    UnknownKey(String),

    /// A setting was given without a value.
    MissingValue(String),

    /// A setting's value could not be parsed into its field.
    InvalidValue {
        key: String,
        value: String,
        kind: ValueKind,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::UnknownKey(key) => write!(f, "unknown setting '{key}'"),
            Self::MissingValue(key) => write!(f, "missing value for setting '{key}'"),
            Self::InvalidValue { key, value, .. } => {
                write!(f, "invalid value '{value}' for setting '{key}'")
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

    /// Reads a config file and applies each `key value` setting, ignoring
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

    /// Applies a single `key value` setting, parsing `value` into the field
    /// the key names.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Error> {
        let setting = SETTING_TABLE
            .get(key)
            .ok_or_else(|| Error::UnknownKey(key.to_string()))?;

        (setting.set)(self, value).map_err(|kind| Error::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
            kind,
        })
    }

    /// Returns the value of setting `key`, or `None` if it is unknown.
    pub fn get(&self, key: &str) -> Option<String> {
        SETTING_TABLE.get(key).map(|setting| (setting.get)(self))
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

#[cfg(test)]
mod tests {
    use super::parse_memory;

    #[test]
    fn parse_memory_units() {
        assert_eq!(parse_memory("100"), Ok(100));
        assert_eq!(parse_memory("0"), Ok(0));
        assert_eq!(parse_memory("1b"), Ok(1));
        assert_eq!(parse_memory("1k"), Ok(1_000));
        assert_eq!(parse_memory("1kb"), Ok(1_024));
        assert_eq!(parse_memory("1m"), Ok(1_000_000));
        assert_eq!(parse_memory("1mb"), Ok(1_048_576));
        assert_eq!(parse_memory("1g"), Ok(1_000_000_000));
        assert_eq!(parse_memory("1gb"), Ok(1_073_741_824));
    }

    #[test]
    fn parse_memory_is_case_insensitive() {
        assert_eq!(parse_memory("1K"), Ok(1_000));
        assert_eq!(parse_memory("1KB"), Ok(1_024));
        assert_eq!(parse_memory("1MB"), Ok(1_048_576));
    }

    #[test]
    fn parse_memory_rejects_invalid() {
        for value in ["1.5gb", "  100  ", "1t", "1tb", "-5", "abc", "100x", ""] {
            assert!(
                parse_memory(value).is_err(),
                "expected {value:?} to be rejected"
            );
        }
    }
}
