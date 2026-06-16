// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The append-only file.
//!
//! Logs every write command as RESP so the dataset can be rebuilt by replaying
//! the file on startup.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::FsyncPolicy;
use crate::resp::Value;

/// How often [`FsyncPolicy::EverySec`] flushes.
const SYNC_INTERVAL: Duration = Duration::from_secs(1);

/// Appends write commands to a file on disk.
pub struct Aof {
    /// The open append-only file.
    file: File,

    /// When the file was last flushed to disk.
    last_sync: Instant,
}

impl Aof {
    /// Opens the append-only file at `path`, creating it if it does not exist.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file,
            last_sync: Instant::now(),
        })
    }

    /// Appends `argv` to the file as a RESP array, flushing it immediately under
    /// [`FsyncPolicy::Always`].
    pub fn append(&mut self, argv: &[Vec<u8>], policy: FsyncPolicy) -> std::io::Result<()> {
        let command = Value::Array(argv.iter().map(|arg| Value::Bulk(arg.clone())).collect());
        self.file.write_all(&command.encode())?;

        if matches!(policy, FsyncPolicy::Always) {
            self.sync()?;
        }

        Ok(())
    }

    /// Flushes the file to disk if [`FsyncPolicy::EverySec`] is due, called on the
    /// periodic tick.
    pub fn sync_if_due(&mut self, policy: FsyncPolicy) -> std::io::Result<()> {
        if matches!(policy, FsyncPolicy::EverySec) && self.last_sync.elapsed() >= SYNC_INTERVAL {
            self.sync()?;
        }

        Ok(())
    }

    /// Flushes the file's data to disk and records the time.
    fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync_data()?;
        self.last_sync = Instant::now();
        Ok(())
    }
}
