// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::os::fd::RawFd;
use std::time::Duration;

use super::{Event, IoMultiplexer, Operation};

/// An [`IoMultiplexer`] for Linux, backed by the `epoll` syscalls.
pub struct Epoll {
    /// The `epoll` instance file descriptor.
    fd: RawFd,

    /// Reused buffer the kernel fills with ready events during `epoll_wait`.
    native: Vec<libc::epoll_event>,
}

impl Epoll {
    /// Creates an [`Epoll`] sized for up to `max_events` ready descriptors per poll.
    pub fn new(max_events: usize) -> std::io::Result<Self> {
        let fd = unsafe { libc::epoll_create1(0) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            fd,
            native: vec![libc::epoll_event { events: 0, u64: 0 }; max_events],
        })
    }
}

impl IoMultiplexer for Epoll {
    fn subscribe(&self, event: Event) -> std::io::Result<()> {
        let mut native = libc::epoll_event {
            events: event.op.to_flags(),
            u64: event.fd as u64,
        };

        if unsafe { libc::epoll_ctl(self.fd, libc::EPOLL_CTL_ADD, event.fd, &mut native) } < 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }

    fn poll(&mut self, timeout: Option<Duration>, events: &mut Vec<Event>) -> std::io::Result<()> {
        // epoll_wait wants milliseconds as a C int, with -1 meaning forever.
        // Clamp so a large Duration cannot wrap into a negative value.
        let timeout = match timeout {
            Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
            None => -1,
        };

        let n = unsafe {
            libc::epoll_wait(
                self.fd,
                self.native.as_mut_ptr(),
                self.native.len() as i32,
                timeout,
            )
        };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }

        events.clear();
        for i in 0..n as usize {
            let native = self.native[i];
            events.push(Event {
                fd: native.u64 as RawFd,
                op: Operation::from_flags(native.events),
            });
        }

        Ok(())
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl Operation {
    /// Returns this operation's `epoll` event flags.
    fn to_flags(self) -> u32 {
        match self {
            Self::Read => libc::EPOLLIN as u32,
            Self::Write => libc::EPOLLOUT as u32,
        }
    }

    /// Reads the operation out of a set of `epoll` event flags.
    fn from_flags(flags: u32) -> Self {
        if flags & libc::EPOLLIN as u32 != 0 {
            Self::Read
        } else {
            Self::Write
        }
    }
}
