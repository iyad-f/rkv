// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server that accepts connections and serves commands.
//!
//! `Server` is the [`EventHandler`] the [`EventLoop`] drives. It parses
//! requests, dispatches commands, and queues replies.

mod child;
mod state;

use std::collections::HashMap;
use std::net::TcpListener;
use std::os::fd::{AsRawFd, RawFd};

use crate::aof::Aof;
use crate::client::Client;
use crate::command;
use crate::config::Config;
use crate::event_loop::{Event, EventHandler, EventLoop, Interest, Operation};
use crate::resp;
use crate::session::Session;

pub use state::State;

/// The event-driven server, holding the listener, shared state, and clients.
pub struct Server {
    /// The listening socket new clients connect to.
    listener: TcpListener,

    /// The shared state every command operates on.
    state: State,

    /// Live clients, keyed by file descriptor.
    clients: HashMap<RawFd, Client>,
}

impl Server {
    /// Binds the listening socket from `config` in non-blocking mode.
    pub fn bind(config: Config) -> std::io::Result<Self> {
        let listener = TcpListener::bind(config.addr())?;
        listener.set_nonblocking(true)?;

        let mut server = Self {
            listener,
            state: State::new(config),
            clients: HashMap::new(),
        };
        server.load_aof()?;

        Ok(server)
    }

    /// Accepts every pending connection, registering each with the event loop.
    fn accept(&mut self, event_loop: &mut EventLoop) -> std::io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let client = Client::new(stream)?;
                    let fd = client.fd();
                    event_loop.register(fd, Interest::READABLE)?;
                    self.clients.insert(fd, client);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Serves a readable client by reading, parsing each complete command,
    /// dispatching it, and flushing the queued replies. Drops the client on
    /// end-of-stream, error, or invalid input.
    fn serve_client(&mut self, fd: RawFd, event_loop: &mut EventLoop) -> std::io::Result<()> {
        let Some(client) = self.clients.get_mut(&fd) else {
            return Ok(());
        };

        let mut close = false;
        if !client.fill() {
            close = true;
        }

        // A single fill may have delivered several pipelined commands, so parse
        // each complete one in turn. A trailing partial command stays buffered
        // for the next read.
        while !close {
            match resp::Request::parse(client.input()) {
                Ok(resp::Request::Command { argv, consumed }) => {
                    let reply = command::dispatch(&argv, &mut self.state, &mut client.session);
                    client.queue(&reply.encode());
                    client.consume(consumed);
                    if client.session.should_close() {
                        close = true;
                    }
                }
                Ok(resp::Request::Empty { consumed }) => client.consume(consumed),
                Ok(resp::Request::Incomplete) => break,
                Err(e) => {
                    client.queue(&resp::Value::Error(format!("ERR {e}")).encode());
                    close = true;
                }
            }
        }

        if close {
            // Best effort flush of any queued reply, e.g. a protocol error,
            // before dropping the client.
            client.flush();
            self.close_client(fd, event_loop);
            return Ok(());
        }
        self.flush_to_client(fd, event_loop)
    }

    /// Flushes a writable client's queued output and updates its write interest,
    /// closing it on a write error.
    fn flush_to_client(&mut self, fd: RawFd, event_loop: &mut EventLoop) -> std::io::Result<()> {
        let Some(client) = self.clients.get_mut(&fd) else {
            return Ok(());
        };

        let close = if !client.flush() {
            true
        } else {
            Self::apply_write_interest(client, fd, event_loop)?;
            false
        };

        if close {
            self.close_client(fd, event_loop);
        }
        Ok(())
    }

    /// Reregisters interest when a client starts or stops having queued output,
    /// so writability is watched only while there is something to send and a
    /// level triggered poll does not wake us in a busy loop.
    fn apply_write_interest(
        client: &mut Client,
        fd: RawFd,
        event_loop: &mut EventLoop,
    ) -> std::io::Result<()> {
        if let Some(want_write) = client.write_interest_change() {
            let interest = if want_write {
                Interest::READABLE_WRITABLE
            } else {
                Interest::READABLE
            };
            event_loop.reregister(fd, interest)?;
        }
        Ok(())
    }

    /// Stops watching `fd` and drops its client.
    fn close_client(&mut self, fd: RawFd, event_loop: &mut EventLoop) {
        // Dropping the client below also removes the descriptor from the poller,
        // but deregistering first keeps its set tidy.
        let _ = event_loop.deregister(fd);
        self.clients.remove(&fd);
    }

    /// The listener's file descriptor.
    pub fn listener_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }

    /// Replays the append-only file into the store, then opens it for appending.
    ///
    /// Does nothing when persistence is disabled. Replay runs before the append
    /// handle is opened, so the commands it dispatches are not re-logged.
    fn load_aof(&mut self) -> std::io::Result<()> {
        if !self.state.config.aof.enabled {
            return Ok(());
        }

        let path = self.state.config.aof_path();
        if path.exists() {
            let bytes = std::fs::read(&path)?;
            self.replay(&bytes);
        }

        self.state.aof = Aof::open(path)?;
        Ok(())
    }

    /// Dispatches every complete command in `bytes`, stopping at the first
    /// incomplete or malformed one.
    fn replay(&mut self, mut bytes: &[u8]) {
        // Replay is a trusted internal operation, so its session is already
        // authenticated and never blocked by a configured password.
        let mut session = Session::default();
        session.authenticate();

        loop {
            match resp::Request::parse(bytes) {
                Ok(resp::Request::Command { argv, consumed }) => {
                    command::dispatch(&argv, &mut self.state, &mut session);
                    bytes = &bytes[consumed..];
                }
                Ok(resp::Request::Empty { consumed }) => bytes = &bytes[consumed..],
                Ok(resp::Request::Incomplete) => break,
                Err(e) => {
                    eprintln!("aof replay stopped on malformed command: {e}");
                    break;
                }
            }
        }
    }
}

impl EventHandler for Server {
    fn on_io(&mut self, event: Event, event_loop: &mut EventLoop) -> std::io::Result<()> {
        if event.fd == self.listener.as_raw_fd() {
            return self.accept(event_loop);
        }

        match event.op {
            Operation::Read => self.serve_client(event.fd, event_loop),
            Operation::Write => self.flush_to_client(event.fd, event_loop),
        }
    }

    fn on_tick(&mut self) {
        self.state.store.expire_cycle(&mut self.state.prng);

        if let Err(e) = self.state.aof.sync_if_due(self.state.config.aof.fsync) {
            eprintln!("aof sync failed: {e}");
        }

        if let Err(e) = self.state.reap_child() {
            eprintln!("aof rewrite failed: {e}");
        }

        if let Err(e) = self.state.maybe_auto_rewrite() {
            eprintln!("aof auto-rewrite failed: {e}");
        }
    }

    fn on_shutdown(&mut self) {
        if let Err(e) = self.state.aof.sync() {
            eprintln!("aof shutdown sync failed: {e}");
        }
    }
}
