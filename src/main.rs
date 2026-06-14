// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! rkv, an in-memory key-value store.

mod client;
mod command;
mod config;
mod dict;
mod event_loop;
mod prng;
mod resp;
mod server;
mod state;
mod store;

use config::Config;
use event_loop::{EventLoop, Interest};
use server::Server;

fn main() -> std::io::Result<()> {
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("config error: {e}");
            std::process::exit(1);
        }
    };

    let mut event_loop = EventLoop::new(config.max_clients)?;
    let mut server = Server::bind(config)?;

    event_loop.register(server.listener_fd(), Interest::READABLE)?;
    event_loop.run(&mut server)
}
