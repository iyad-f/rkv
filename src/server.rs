// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The server that accepts connections and serves commands.
//!
//! `Server` is the [`EventHandler`] driven by the [`EventLoop`]. It owns the
//! listener, the shared state, and the live connections.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};

use crate::command;
use crate::config::Config;
use crate::event_loop::{Event, EventHandler, EventLoop, Operation};
use crate::resp;
use crate::state::State;

/// How many bytes to read from a socket per `read` call.
const READ_CHUNK: usize = 16 * 1024;

/// A connected client and the bytes read from it but not yet parsed.
struct Connection {
    /// The client's TCP stream.
    stream: TcpStream,

    /// Bytes received but not yet consumed by a complete command, which may
    /// arrive split across reads.
    buffer: Vec<u8>,
}

/// The event-driven server, holding the listener, shared state, and connections.
pub struct Server {
    /// The listening socket new clients connect to.
    listener: TcpListener,

    /// The shared state every command operates on.
    state: State,

    /// Live connections, keyed by file descriptor.
    connections: HashMap<RawFd, Connection>,
}

impl Server {
    /// Binds the listening socket from `config` in non-blocking mode.
    pub fn bind(config: Config) -> std::io::Result<Self> {
        let listener = TcpListener::bind(config.addr())?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            listener,
            state: State::new(config),
            connections: HashMap::new(),
        })
    }

    /// Accepts every pending connection, registering each with the event loop.
    fn accept(&mut self, event_loop: &mut EventLoop) -> std::io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    stream.set_nodelay(true)?;

                    let fd = stream.as_raw_fd();
                    event_loop.subscribe(Event {
                        fd,
                        op: Operation::Read,
                    })?;

                    self.connections.insert(
                        fd,
                        Connection {
                            stream,
                            buffer: Vec::new(),
                        },
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Reads from `fd`, then parses and replies to each complete command, dropping
    /// the connection on end-of-stream, error, or invalid input.
    fn handle_client(&mut self, fd: RawFd) {
        let mut close = false;

        if let Some(conn) = self.connections.get_mut(&fd) {
            let mut chunk = [0u8; READ_CHUNK];
            match conn.stream.read(&mut chunk) {
                Ok(0) => close = true,
                Ok(n) => conn.buffer.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => close = true,
            }

            while !close {
                match resp::Value::parse(&conn.buffer) {
                    Ok((value, consumed)) => {
                        let reply = command::dispatch(value, &mut self.state);
                        if conn.stream.write_all(&reply.encode()).is_err() {
                            close = true;
                        } else {
                            conn.buffer.drain(..consumed);
                        }
                    }
                    Err(resp::ParseError::Incomplete) => break,
                    Err(resp::ParseError::Invalid) => close = true,
                }
            }
        }

        if close {
            self.connections.remove(&fd);
        }
    }
}

impl EventHandler for Server {
    fn register(&mut self, event_loop: &mut EventLoop) -> std::io::Result<()> {
        event_loop.subscribe(Event {
            fd: self.listener.as_raw_fd(),
            op: Operation::Read,
        })
    }

    fn handle(&mut self, event: Event, event_loop: &mut EventLoop) -> std::io::Result<()> {
        if event.fd == self.listener.as_raw_fd() {
            self.accept(event_loop)
        } else {
            self.handle_client(event.fd);
            Ok(())
        }
    }
}
