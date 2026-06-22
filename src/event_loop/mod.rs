// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! A single-threaded event loop.
//!
//! [`EventLoop`] waits for ready descriptors and dispatches each to an
//! [`EventHandler`]. The platform backend stays hidden in the private
//! [`io_multiplexer`] submodule, so handlers register interest through the loop,
//! not the backend.

mod io_multiplexer;

use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use crate::signal;

use io_multiplexer::{IoMultiplexer, Poller};

pub use io_multiplexer::{Event, Interest, Operation};

/// The application an [`EventLoop`] drives, through I/O events and a periodic tick.
pub trait EventHandler {
    /// Handles one ready I/O `event`. Returning an error stops the loop.
    fn on_io(&mut self, event: Event, event_loop: &mut EventLoop) -> std::io::Result<()>;

    /// Runs periodic background work, called on a roughly fixed interval.
    fn on_tick(&mut self) {}

    /// Runs once after a shutdown is requested, before the loop returns.
    fn on_shutdown(&mut self) {}
}

/// A single-threaded event loop driving an [`EventHandler`].
pub struct EventLoop {
    poller: Poller,

    /// The most descriptors a single poll may report, sizing the event buffer.
    max_events: usize,
}

impl EventLoop {
    /// Creates an event loop sized for up to `max_events` ready descriptors per poll.
    pub fn new(max_events: usize) -> std::io::Result<Self> {
        Ok(Self {
            poller: Poller::new(max_events)?,
            max_events,
        })
    }

    /// Registers `fd` for `interest` so future polls report it as ready.
    pub fn register(&mut self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.poller.register(fd, interest)
    }

    /// Replaces the interest of an already registered `fd`.
    pub fn reregister(&mut self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.poller.reregister(fd, interest)
    }

    /// Stops watching `fd`.
    pub fn deregister(&mut self, fd: RawFd) -> std::io::Result<()> {
        self.poller.deregister(fd)
    }

    /// Dispatches ready I/O events, firing the handler's periodic tick on a
    /// roughly fixed interval, until a shutdown is requested or it errors.
    pub fn run(&mut self, handler: &mut impl EventHandler) -> std::io::Result<()> {
        const TICK: Duration = Duration::from_millis(100);

        let mut ready = Vec::with_capacity(self.max_events);
        let mut next_tick = Instant::now() + TICK;
        while !signal::shutdown_requested() {
            // Cap the poll timeout at the next tick so one thread serves both I/O
            // and the timer. poll returns when I/O is ready or when the tick comes
            // due, whichever is first, and an overdue tick saturates to a zero
            // timeout so poll returns at once.
            let timeout = next_tick.saturating_duration_since(Instant::now());
            self.poller.poll(Some(timeout), &mut ready)?;

            // Handle ready I/O before the tick so client requests never wait
            // for background maintenance.
            for &event in &ready {
                handler.on_io(event, self)?;
            }

            if Instant::now() >= next_tick {
                handler.on_tick();
                next_tick = Instant::now() + TICK;
            }
        }

        handler.on_shutdown();
        Ok(())
    }
}
