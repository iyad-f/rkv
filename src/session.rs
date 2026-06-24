// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Per-connection state.

#[derive(Default)]
pub struct Session {
    /// Whether this connection has authenticated
    authenticated: bool,

    /// Whether to close the connection once the current reply is flushed.
    close_after_reply: bool,
}

impl Session {
    /// Marks the connection as having authenticated.
    pub fn authenticate(&mut self) {
        self.authenticated = true;
    }

    /// Whether the connection has authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Requests that the connection close once the current reply is flushed.
    pub fn request_close(&mut self) {
        self.close_after_reply = true;
    }

    /// Whether the connection should close after the current reply is flushed.
    pub fn should_close(&self) -> bool {
        self.close_after_reply
    }
}
