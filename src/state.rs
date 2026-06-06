// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server's shared state.

use std::collections::HashMap;

use crate::config::Config;

/// The key-value store.
pub type Store = HashMap<Vec<u8>, Vec<u8>>;

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
