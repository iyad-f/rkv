// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::os::fd::RawFd;

use super::{Event, IoMultiplexer, Operation};

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
}

impl IoMultiplexer for Kqueue {
    fn subscribe(&self, event: Event) -> std::io::Result<()> {
        let change = libc::kevent {
            ident: event.fd as usize,
            filter: event.op.to_filter(),
            flags: libc::EV_ADD,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        if unsafe {
            libc::kevent(
                self.fd,
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
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
            return Err(std::io::Error::last_os_error());
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
    /// Returns this operation's `kqueue` filter.
    fn to_filter(self) -> i16 {
        match self {
            Self::Read => libc::EVFILT_READ,
            Self::Write => libc::EVFILT_WRITE,
        }
    }

    /// Reads the operation out of a `kqueue` filter.
    fn from_filter(filter: i16) -> Self {
        if filter == libc::EVFILT_READ {
            Self::Read
        } else {
            Self::Write
        }
    }
}
