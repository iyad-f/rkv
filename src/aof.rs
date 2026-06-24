// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The append-only file.
//!
//! Logs every write command as RESP so the dataset can be rebuilt by replaying
//! the file on startup.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::FsyncPolicy;
use crate::object::Object;
use crate::resp::Value;
use crate::store::Store;

/// How often [`FsyncPolicy::EverySec`] syncs.
const SYNC_INTERVAL: Duration = Duration::from_secs(1);

/// The append-only file.
pub struct Aof {
    /// The path the file lives at.
    path: PathBuf,

    /// The logging state of the file.
    state: State,
}

/// Whether the append-only file is logging, and what an active log needs.
enum State {
    /// Logging is off. No file is open.
    Disabled,

    /// Logging is on, appending to an open file.
    Enabled {
        /// The open file.
        file: File,

        /// When the file was last synced to disk.
        last_sync: Instant,

        /// Writes captured while a rewrite child runs, `Some` while capturing.
        rewrite_buffer: Option<Vec<u8>>,

        /// The file's size, in bytes, after the last rewrite, against which
        /// growth is measured for automatic rewrites.
        base_size: u64,

        /// Whether the last write reached the file. Set false on a failed write
        /// or sync, and cleared once a later sync reaches disk.
        last_write_ok: bool,
    },
}

impl Aof {
    /// Opens the file at `path` for logging, creating it if it does not exist.
    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let file = open_file(&path)?;
        let base_size = file.metadata()?.len();
        Ok(Self {
            path,
            state: State::Enabled {
                file,
                last_sync: Instant::now(),
                rewrite_buffer: None,
                base_size,
                last_write_ok: true,
            },
        })
    }

    /// Creates a non-logging file at `path`. It opens no file but can still be
    /// rewritten on demand.
    pub fn disabled(path: PathBuf) -> Self {
        Self {
            path,
            state: State::Disabled,
        }
    }

    /// Whether the file is healthy to write to. True while logging is disabled,
    /// false once a write or sync has failed until a later sync reaches disk.
    pub fn write_ok(&self) -> bool {
        match &self.state {
            State::Disabled => true,
            State::Enabled { last_write_ok, .. } => *last_write_ok,
        }
    }

    /// Appends `argv` to the file as a RESP array, syncing it immediately under
    /// [`FsyncPolicy::Always`]. Does nothing while logging is disabled.
    pub fn append(&mut self, argv: &[Vec<u8>], policy: FsyncPolicy) -> std::io::Result<()> {
        let State::Enabled {
            file,
            last_sync,
            rewrite_buffer,
            last_write_ok,
            ..
        } = &mut self.state
        else {
            return Ok(());
        };

        let encoded = encode_command(argv);

        // Assume the write fails until it fully lands, so any early return below
        // leaves the file marked unhealthy until a later sync reaches disk.
        *last_write_ok = false;

        file.write_all(&encoded)?;

        // While a rewrite is in progress, capture the command so it can be
        // merged into the new file once the child finishes.
        if let Some(buffer) = rewrite_buffer {
            buffer.extend_from_slice(&encoded);
        }

        if matches!(policy, FsyncPolicy::Always) {
            file.sync_data()?;
            *last_sync = Instant::now();
        }

        *last_write_ok = true;
        Ok(())
    }

    /// Syncs the file to disk if [`FsyncPolicy::EverySec`] is due, or if a prior
    /// write failed, to retry reaching disk. Does nothing while logging is
    /// disabled.
    pub fn sync_if_due(&mut self, policy: FsyncPolicy) -> std::io::Result<()> {
        let State::Enabled {
            file,
            last_sync,
            last_write_ok,
            ..
        } = &mut self.state
        else {
            return Ok(());
        };

        let due = matches!(policy, FsyncPolicy::EverySec) && last_sync.elapsed() >= SYNC_INTERVAL;
        if due || !*last_write_ok {
            *last_write_ok = false;
            file.sync_data()?;
            *last_write_ok = true;
            *last_sync = Instant::now();
        }

        Ok(())
    }

    /// Syncs the file to disk unconditionally, for a clean shutdown. Does nothing
    /// while logging is disabled.
    pub fn sync(&mut self) -> std::io::Result<()> {
        let State::Enabled {
            file,
            last_sync,
            last_write_ok,
            ..
        } = &mut self.state
        else {
            return Ok(());
        };

        *last_write_ok = false;
        file.sync_data()?;
        *last_write_ok = true;
        *last_sync = Instant::now();

        Ok(())
    }

    /// Forks a child that writes the snapshot to a temporary file without blocking
    /// the server, returning the new child's pid. The parent begins capturing the
    /// writes made during the rewrite so they can be merged in on completion.
    pub fn background_rewrite(&mut self, store: &Store) -> std::io::Result<libc::pid_t> {
        let tmp = self.temp_path();

        match unsafe { libc::fork() } {
            // Fork failed
            -1 => Err(std::io::Error::last_os_error()),

            // After this point a child has been created and both the child and the parent
            // will run concurrently, resuming execution from the exact same instruction,
            // so fork will return twice, once for the parent and once for the child.

            // Getting 0 means we are inside the child process.
            0 => {
                let code = match write_snapshot(&snapshot_commands(store), &tmp) {
                    Ok(()) => 0,
                    Err(_) => 1,
                };
                unsafe { libc::_exit(code) }
            }

            // Getting > 0 means we are in the parent process and have received the pid
            // of the newly created child process.
            pid => {
                if let State::Enabled { rewrite_buffer, .. } = &mut self.state {
                    *rewrite_buffer = Some(Vec::new());
                }
                Ok(pid)
            }
        }
    }

    /// Installs a finished rewrite. While logging, it merges the captured diff
    /// into the child's snapshot and reopens the append handle on it. Otherwise
    /// it just renames the snapshot into place.
    pub fn commit_rewrite(&mut self) -> std::io::Result<()> {
        let tmp = self.temp_path();

        match &mut self.state {
            State::Enabled {
                file,
                last_sync,
                rewrite_buffer,
                base_size,
                last_write_ok,
            } => {
                let buffer = rewrite_buffer.take().unwrap_or_default();
                let mut snapshot = OpenOptions::new().append(true).open(&tmp)?;
                snapshot.write_all(&buffer)?;
                snapshot.sync_data()?;

                std::fs::rename(&tmp, &self.path)?;
                *file = open_file(&self.path)?;
                *base_size = file.metadata()?.len();
                *last_sync = Instant::now();
                *last_write_ok = true;
                Ok(())
            }
            State::Disabled => std::fs::rename(&tmp, &self.path),
        }
    }

    /// Whether the file has grown past the threshold for an automatic rewrite,
    /// having grown at least `percentage` over its size after the last rewrite
    /// and exceeded `min_size` bytes. Always false while logging is disabled.
    pub fn should_rewrite(&self, percentage: u64, min_size: u64) -> std::io::Result<bool> {
        let State::Enabled {
            file, base_size, ..
        } = &self.state
        else {
            return Ok(false);
        };

        let size = file.metadata()?.len();
        if size <= min_size {
            return Ok(false);
        }

        let base = (*base_size).max(1);
        Ok(size * 100 >= base * (100 + percentage))
    }

    /// Discards an in-progress rewrite's captured diff, keeping the live file.
    pub fn abort_rewrite(&mut self) {
        if let State::Enabled { rewrite_buffer, .. } = &mut self.state {
            *rewrite_buffer = None;
        }
    }

    /// The path of the temporary file a rewrite writes before renaming it in.
    fn temp_path(&self) -> PathBuf {
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        PathBuf::from(tmp)
    }
}

/// Opens the file at `path` for appending, creating it if it does not exist.
fn open_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Encodes `argv` as a RESP array, the on-disk form of a command.
fn encode_command(argv: &[Vec<u8>]) -> Vec<u8> {
    Value::Array(argv.iter().map(|arg| Value::Bulk(arg.clone())).collect()).encode()
}

/// Builds the minimal set of commands that recreates the current dataset.
fn snapshot_commands(store: &Store) -> Vec<Vec<Vec<u8>>> {
    let mut commands = Vec::new();

    for (key, value, deadline) in store.iter() {
        match value {
            Object::String(bytes) => {
                commands.push(vec![b"SET".to_vec(), key.to_vec(), bytes.clone()]);
            }
            Object::List(items) => {
                let mut argv = vec![b"RPUSH".to_vec(), key.to_vec()];
                argv.extend(items.iter().cloned());
                commands.push(argv);
            }
        }

        if let Some(deadline) = deadline {
            commands.push(vec![
                b"PEXPIREAT".to_vec(),
                key.to_vec(),
                deadline.to_string().into_bytes(),
            ]);
        }
    }

    commands
}

/// Writes `commands` as RESP to `path`, replacing it, and syncs to disk.
fn write_snapshot(commands: &[Vec<Vec<u8>>], path: &Path) -> std::io::Result<()> {
    let mut buf = Vec::new();
    for argv in commands {
        buf.extend_from_slice(&encode_command(argv));
    }

    let mut file = File::create(path)?;
    file.write_all(&buf)?;
    file.sync_data()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_emits_a_set_per_string_key() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), Object::String(b"v".to_vec()));

        assert_eq!(
            snapshot_commands(&store),
            vec![vec![b"SET".to_vec(), b"k".to_vec(), b"v".to_vec()]],
        );
    }

    #[test]
    fn snapshot_records_expiry_as_pexpireat() {
        let mut store = Store::new();
        store.set(b"k".to_vec(), Object::String(b"v".to_vec()));
        store.set_expiry(b"k", 9_999_999_999_999);

        assert_eq!(
            snapshot_commands(&store),
            vec![
                vec![b"SET".to_vec(), b"k".to_vec(), b"v".to_vec()],
                vec![
                    b"PEXPIREAT".to_vec(),
                    b"k".to_vec(),
                    b"9999999999999".to_vec()
                ],
            ],
        );
    }
}
