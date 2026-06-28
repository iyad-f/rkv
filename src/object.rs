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

impl Object {
    /// The name of this object's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::List(_) => "list",
        }
    }

    /// The effort required to drop this object.
    pub fn drop_effort(&self) -> usize {
        match self {
            Self::String(_) => 1,
            Self::List(list) => list.len(),
        }
    }
}
