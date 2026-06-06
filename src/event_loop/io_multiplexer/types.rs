// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The `Event` and `Operation` vocabulary shared by the backends.

use std::os::fd::RawFd;

/// The kind of I/O readiness to watch for on a descriptor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    /// Readiness for reading.
    Read,

    /// Readiness for writing.
    Write,
}

/// A file descriptor paired with the [`Operation`] that applies to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// The file descriptor.
    pub fd: RawFd,

    /// The operation watched for, or reported as ready.
    pub op: Operation,
}
