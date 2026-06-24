// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! The typed value a key holds.

use std::collections::VecDeque;

/// A value stored at a key.
#[derive(Debug, PartialEq)]
pub enum Object {
    /// A byte string.
    String(Vec<u8>),

    /// A list of byte strings.
    List(VecDeque<Vec<u8>>),
}
