// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server's shared state.

use std::hash::{BuildHasher, Hasher, RandomState};

use crate::{aof::Aof, config::Config, prng::Prng, store::Store};

/// The shared state commands read and modify.
pub struct State {
    /// The key-value store.
    pub store: Store,

    /// The server configuration.
    pub config: Config,

    /// The shared random number generator, used for key sampling.
    pub prng: Prng,

    /// The append-only file, None when persistence is disabled.
    pub aof: Option<Aof>,
}

impl State {
    /// Creates empty state with the given configuration.
    pub fn new(config: Config) -> Self {
        let seed = RandomState::new().build_hasher().finish();
        Self {
            store: Store::new(),
            config,
            prng: Prng::new(seed),
            aof: None,
        }
    }
}
