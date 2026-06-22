// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! A background child process forked for persistence work.

/// A forked child process and the work it is doing.
pub struct Child {
    /// The child's process id.
    pub pid: libc::pid_t,

    /// What the child is doing.
    pub kind: Kind,
}

/// The kind of work a background child performs.
#[derive(Clone, Copy)]
pub enum Kind {
    /// Rewriting the append-only file.
    AofRewrite,
}
