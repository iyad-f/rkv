// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! rkv, an in-memory key-value store.

mod command;
mod config;
mod event_loop;
mod resp;
mod server;
mod state;

use config::Config;
use event_loop::EventLoop;
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
    event_loop.run(&mut server)
}
