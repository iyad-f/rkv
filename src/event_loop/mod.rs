// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! A single-threaded event loop.
//!
//! [`EventLoop`] waits for ready descriptors and dispatches each to an
//! [`EventHandler`]. The platform backend stays hidden in the private
//! [`io_multiplexer`] submodule, so handlers register interest through the loop,
//! not the backend.

mod io_multiplexer;

use io_multiplexer::{IoMultiplexer, Poller};

pub use io_multiplexer::{Event, Operation};

/// Reacts to the descriptors an [`EventLoop`] reports as ready.
pub trait EventHandler {
    /// Registers the handler's initial interests before the loop starts.
    fn register(&mut self, event_loop: &mut EventLoop) -> std::io::Result<()>;

    /// Handles one ready `event`. Returning an error stops the loop.
    fn handle(&mut self, event: Event, event_loop: &mut EventLoop) -> std::io::Result<()>;
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

    /// Registers interest in `event` so future polls report it as ready.
    pub fn subscribe(&mut self, event: Event) -> std::io::Result<()> {
        self.poller.subscribe(event)
    }

    /// Registers the handler, then dispatches ready descriptors until it errors.
    pub fn run(&mut self, handler: &mut impl EventHandler) -> std::io::Result<()> {
        handler.register(self)?;

        let mut ready = Vec::with_capacity(self.max_events);
        loop {
            self.poller.poll(None, &mut ready)?;
            for &event in &ready {
                handler.handle(event, self)?;
            }
        }
    }
}
