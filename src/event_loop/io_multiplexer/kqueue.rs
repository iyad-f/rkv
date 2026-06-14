// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::os::fd::RawFd;

use super::{Event, Interest, IoMultiplexer, Operation};

/// An [`IoMultiplexer`] for macOS, backed by the `kqueue` and `kevent` syscalls.
pub struct Kqueue {
    /// The `kqueue` instance file descriptor.
    fd: RawFd,

    /// Reused buffer the kernel fills with ready events during `kevent`.
    native: Vec<libc::kevent>,
}

impl Kqueue {
    /// Creates a [`Kqueue`] sized for up to `max_events` ready descriptors per poll.
    pub fn new(max_events: usize) -> std::io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            fd,
            native: vec![
                libc::kevent {
                    ident: 0,
                    filter: 0,
                    flags: 0,
                    fflags: 0,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };
                max_events
            ],
        })
    }

    /// Adds or deletes a single filter for `fd`. Deleting a filter that was
    /// never added is treated as success.
    fn change(&self, fd: RawFd, filter: i16, add: bool) -> std::io::Result<()> {
        let change = libc::kevent {
            ident: fd as usize,
            filter,
            flags: if add { libc::EV_ADD } else { libc::EV_DELETE },
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let result = unsafe {
            libc::kevent(
                self.fd,
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if result < 0 {
            let err = std::io::Error::last_os_error();
            if !add && err.raw_os_error() == Some(libc::ENOENT) {
                return Ok(());
            }
            return Err(err);
        }

        Ok(())
    }

    /// Enables the filters `interest` asks for and deletes the rest.
    fn apply(&self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.change(fd, libc::EVFILT_READ, interest.read)?;
        self.change(fd, libc::EVFILT_WRITE, interest.write)
    }
}

impl IoMultiplexer for Kqueue {
    fn register(&self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.apply(fd, interest)
    }

    fn reregister(&self, fd: RawFd, interest: Interest) -> std::io::Result<()> {
        self.apply(fd, interest)
    }

    fn deregister(&self, fd: RawFd) -> std::io::Result<()> {
        self.apply(
            fd,
            Interest {
                read: false,
                write: false,
            },
        )
    }

    fn poll(
        &mut self,
        timeout: Option<std::time::Duration>,
        events: &mut Vec<Event>,
    ) -> std::io::Result<()> {
        // `ts` is bound before the match so the pointer it yields stays valid
        // through the kevent call below.
        let ts;
        let timeout_ptr = match timeout {
            Some(d) => {
                ts = libc::timespec {
                    tv_sec: d.as_secs().min(libc::time_t::MAX as u64) as libc::time_t,
                    tv_nsec: d.subsec_nanos() as libc::c_long,
                };
                &ts
            }
            None => std::ptr::null(),
        };

        let n = unsafe {
            libc::kevent(
                self.fd,
                std::ptr::null(),
                0,
                self.native.as_mut_ptr(),
                self.native.len() as libc::c_int,
                timeout_ptr,
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
            events.push(Event {
                fd: native.ident as RawFd,
                op: Operation::from_filter(native.filter),
            });
        }

        Ok(())
    }
}

impl Drop for Kqueue {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl Operation {
    /// Reads the operation out of a `kqueue` filter.
    fn from_filter(filter: i16) -> Self {
        if filter == libc::EVFILT_READ {
            Self::Read
        } else {
            Self::Write
        }
    }
}
