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

use std::os::fd::RawFd;
use std::time::Duration;

pub use types::{Event, Interest, Operation};

#[cfg(target_os = "linux")]
pub use epoll::Epoll as Poller;
#[cfg(target_os = "macos")]
pub use kqueue::Kqueue as Poller;

/// A readiness notification facility the event loop waits on.
pub trait IoMultiplexer {
    /// Registers `fd` for the given `interest`, so [`poll`](Self::poll) reports
    /// it once ready. The descriptor must not already be registered.
    fn register(&self, fd: RawFd, interest: Interest) -> std::io::Result<()>;

    /// Replaces the interest of an already registered `fd`.
    fn reregister(&self, fd: RawFd, interest: Interest) -> std::io::Result<()>;

    /// Stops watching `fd` entirely.
    fn deregister(&self, fd: RawFd) -> std::io::Result<()>;

    /// Blocks until at least one registered descriptor is ready, replacing the
    /// contents of `events` with the ready ones. A descriptor ready for both
    /// reading and writing yields one [`Event`] per readiness.
    ///
    /// `None` blocks indefinitely, `Some(d)` waits up to `d`.
    fn poll(&mut self, timeout: Option<Duration>, events: &mut Vec<Event>) -> std::io::Result<()>;
}
