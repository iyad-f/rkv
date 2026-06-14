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

/// A file descriptor paired with the [`Operation`] reported as ready.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// The file descriptor.
    pub fd: RawFd,

    /// The operation reported as ready.
    pub op: Operation,
}

/// The set of I/O readiness a descriptor is registered to watch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interest {
    /// Watch for read readiness.
    pub read: bool,

    /// Watch for write readiness.
    pub write: bool,
}

impl Interest {
    /// Watch for read readiness only.
    pub const READABLE: Self = Self {
        read: true,
        write: false,
    };

    /// Watch for both read and write readiness.
    pub const READABLE_WRITABLE: Self = Self {
        read: true,
        write: true,
    };
}
