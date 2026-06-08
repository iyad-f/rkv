// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Types and parsing for the RESP2 wire protocol.

mod request;
mod value;

pub use request::Request;
pub use value::Value;

/// The RESP line terminator.
pub(super) const CRLF: &[u8] = b"\r\n";
