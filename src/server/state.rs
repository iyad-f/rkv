// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server's shared state.

use std::hash::{BuildHasher, Hasher, RandomState};

use super::child::{self, Child};
use crate::{aof::Aof, config::Config, prng::Prng, store::Store};

/// The shared state commands read and modify.
pub struct State {
    /// The key-value store.
    pub store: Store,

    /// The server configuration.
    pub config: Config,

    /// The shared random number generator, used for key sampling.
    pub prng: Prng,

    /// The append-only file. Disabled unless persistence is enabled.
    pub aof: Aof,

    /// The background child process, if one is running.
    child: Option<Child>,
}

impl State {
    /// Creates empty state with the given configuration.
    pub fn new(config: Config) -> Self {
        let seed = RandomState::new().build_hasher().finish();
        let aof = Aof::disabled(config.aof_path());
        let store = Store::new(
            config.lazy_drop.expire.clone(),
            config.lazy_drop.server_del.clone(),
        );

        Self {
            store,
            config,
            prng: Prng::new(seed),
            aof,
            child: None,
        }
    }

    /// Whether an AOF rewrite is currently running.
    pub fn aof_rewrite_in_progress(&self) -> bool {
        matches!(
            self.child,
            Some(Child {
                kind: child::Kind::AofRewrite,
                ..
            })
        )
    }

    /// Starts a background AOF rewrite.
    pub fn start_aof_rewrite(&mut self) -> std::io::Result<()> {
        let pid = self.aof.background_rewrite(&self.store)?;
        self.child = Some(Child {
            pid,
            kind: child::Kind::AofRewrite,
        });
        Ok(())
    }

    /// Starts a background AOF rewrite if the file has grown past the auto-rewrite
    /// threshold and one is not already running.
    pub fn maybe_auto_rewrite(&mut self) -> std::io::Result<()> {
        if self.aof_rewrite_in_progress() {
            return Ok(());
        }

        let Some(percentage) = self.config.aof.auto_rewrite_percentage else {
            return Ok(());
        };

        if self
            .aof
            .should_rewrite(percentage, self.config.aof.auto_rewrite_min_size)?
        {
            self.start_aof_rewrite()?;
        }

        Ok(())
    }

    /// Reaps a finished background child, if one has completed, and completes the
    /// work it was doing. A no-op while no child runs or it is still working.
    pub fn reap_child(&mut self) -> std::io::Result<()> {
        let Some(child) = &self.child else {
            return Ok(());
        };
        let (pid, kind) = (child.pid, child.kind);

        let mut status: libc::c_int = 0;
        match unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } {
            0 => Ok(()), // still running
            -1 => {
                self.child = None;
                self.aof.abort_rewrite();
                Err(std::io::Error::last_os_error())
            }
            _ => {
                self.child = None;
                let ok = libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
                match kind {
                    child::Kind::AofRewrite if ok => self.aof.commit_rewrite(),
                    child::Kind::AofRewrite => {
                        self.aof.abort_rewrite();
                        Ok(())
                    }
                }
            }
        }
    }
}
