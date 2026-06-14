// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::os::fd::RawFd;
use std::time::Duration;

use super::{Event, Interest, IoMultiplexer, Operation};

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

    /// Issues an `epoll_ctl` call applying `interest`'s flags to `fd`.
    fn ctl(&self, op: libc::c_int, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        let mut native = libc::epoll_event {
            events: interest.to_flags(),
            u64: fd as u64,
        };

        if unsafe { libc::epoll_ctl(self.fd, op, fd, &mut native) } < 0 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }
}

impl IoMultiplexer for Epoll {
    fn register(&self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.ctl(libc::EPOLL_CTL_ADD, fd, interest)
    }

    fn reregister(&self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.ctl(libc::EPOLL_CTL_MOD, fd, interest)
    }

    fn deregister(&self, fd: RawFd) -> std::io::Result<()> {
        // The event argument is ignored for EPOLL_CTL_DEL, but kernels before 2.6.9
        // reject a null pointer, so pass a throwaway.
        let mut native = libc::epoll_event { events: 0, u64: 0 };
        if unsafe { libc::epoll_ctl(self.fd, libc::EPOLL_CTL_DEL, fd, &mut native) } < 0 {
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
            let err = std::io::Error::last_os_error();
            // A signal interrupting the wait is not a failure, so report no events.
            if err.kind() == std::io::ErrorKind::Interrupted {
                events.clear();
                return Ok(());
            }
            return Err(err);
        }

        events.clear();
        for i in 0..n as usize {
            let native = self.native[i];
            let fd = native.u64 as RawFd;
            let flags = native.events;

            for op in [Operation::Read, Operation::Write] {
                if op.is_ready(flags) {
                    events.push(Event { fd, op });
                }
            }
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

impl Interest {
    /// Returns the `epoll` event flags for this interest.
    fn to_flags(self) -> u32 {
        let mut flags = 0;
        if self.read {
            flags |= libc::EPOLLIN as u32;
        }
        if self.write {
            flags |= libc::EPOLLOUT as u32;
        }
        flags
    }
}

impl Operation {
    /// Returns whether epoll `flags` report this operation ready. A hangup or
    /// error counts as readiness for both operations, so whichever handler is
    /// registered runs, sees end-of-stream or the error, and closes.
    fn is_ready(self, flags: u32) -> bool {
        let hup_or_err = flags & (libc::EPOLLHUP | libc::EPOLLERR) as u32 != 0;
        let bit = match self {
            Self::Read => libc::EPOLLIN,
            Self::Write => libc::EPOLLOUT,
        };
        flags & bit as u32 != 0 || hup_or_err
    }
}
