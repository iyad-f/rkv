// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server's shared state.

use crate::{config::Config, store::Store};

/// The shared state commands read and modify.
pub struct State {
    /// The key-value store.
    pub store: Store,

    /// The server configuration.
    pub config: Config,
}

impl State {
    /// Creates empty state with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            store: Store::new(),
            config,
        }
    }
}
