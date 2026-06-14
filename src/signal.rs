// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Catches SIGINT and SIGTERM to request a graceful shutdown.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set when a SIGINT or SIGTERM is received.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Sets the shutdown flag.
extern "C" fn shutdown_handler(_signal: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

/// Installs the signal handlers.
pub fn install() -> std::io::Result<()> {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = shutdown_handler as *const () as libc::sighandler_t;
    action.sa_flags = 0;
    unsafe { libc::sigemptyset(&mut action.sa_mask) };

    for signal in [libc::SIGINT, libc::SIGTERM] {
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
    }

    Ok(())
}

/// Whether a shutdown signal has been received.
pub fn shutdown_requested() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}
