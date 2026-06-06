// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Waiting on many file descriptors at once.
//!
//! The platform backend (`epoll` on Linux, `kqueue` on macOS) is selected at
//! compile time and exposed as the [`Poller`] alias behind the [`IoMultiplexer`]
//! trait.

mod types;

#[cfg(target_os = "linux")]
mod epoll;
#[cfg(target_os = "macos")]
mod kqueue;

use std::time::Duration;

pub use types::{Event, Operation};

#[cfg(target_os = "linux")]
pub use epoll::Epoll as Poller;
#[cfg(target_os = "macos")]
pub use kqueue::Kqueue as Poller;

/// A readiness-notification facility the event loop waits on.
pub trait IoMultiplexer {
    /// Registers interest in `event`, so [`poll`](Self::poll) reports its
    /// descriptor once it is ready.
    fn subscribe(&self, event: Event) -> std::io::Result<()>;

    /// Blocks until at least one registered descriptor is ready, replacing the
    /// contents of `events` with the ready ones.
    ///
    /// `None` blocks indefinitely, `Some(d)` waits up to `d`.
    fn poll(&mut self, timeout: Option<Duration>, events: &mut Vec<Event>) -> std::io::Result<()>;
}
