// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! A connected client and its buffered socket I/O.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};

/// How many bytes to read from a socket per `read` call.
const READ_CHUNK: usize = 16 * 1024;

/// A connected client.
pub struct Client {
    /// The client's TCP stream.
    stream: TcpStream,

    /// Bytes received but not yet consumed, which may arrive split across reads.
    in_buf: Vec<u8>,

    /// Bytes queued but not yet written to the socket, which may drain across
    /// several writes when the kernel send buffer fills.
    out_buf: Vec<u8>,

    /// Whether this client is currently registered for write readiness.
    write_registered: bool,
}

impl Client {
    /// Wraps an accepted `stream`, putting it in non-blocking mode.
    pub fn new(stream: TcpStream) -> std::io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            in_buf: Vec::new(),
            out_buf: Vec::new(),
            write_registered: false,
        })
    }

    /// The client's file descriptor.
    pub fn fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    /// Reads available bytes into the input buffer, returning whether the client
    /// is still open. A read of zero bytes or an error closes it.
    pub fn fill(&mut self) -> bool {
        let mut chunk = [0u8; READ_CHUNK];
        match self.stream.read(&mut chunk) {
            Ok(0) => false,
            Ok(n) => {
                self.in_buf.extend_from_slice(&chunk[..n]);
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    /// The received bytes not yet consumed.
    pub fn input(&self) -> &[u8] {
        &self.in_buf
    }

    /// Drops the first `n` consumed bytes from the input buffer.
    pub fn consume(&mut self, n: usize) {
        self.in_buf.drain(..n);
    }

    /// Queues `bytes` to be written to the socket.
    pub fn queue(&mut self, bytes: &[u8]) {
        self.out_buf.extend_from_slice(bytes);
    }

    /// Writes as much queued output as the socket accepts, returning whether the
    /// client is still open. A write error closes it.
    pub fn flush(&mut self) -> bool {
        while !self.out_buf.is_empty() {
            match self.stream.write(&self.out_buf) {
                Ok(0) => return false,
                Ok(n) => {
                    self.out_buf.drain(..n);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => return false,
            }
        }
        true
    }

    /// Reports a change in whether the socket should be watched for writability,
    /// returning the new state only when it differs from what is registered.
    pub fn write_interest_change(&mut self) -> Option<bool> {
        let want_write = !self.out_buf.is_empty();
        if want_write != self.write_registered {
            self.write_registered = want_write;
            Some(want_write)
        } else {
            None
        }
    }
}
