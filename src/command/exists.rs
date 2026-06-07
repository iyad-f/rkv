// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::{resp::Value, state::State};

use super::{Arity, Command};

/// `EXISTS key [key ...]` replies with how many of the given keys exist.
pub const COMMAND: Command = Command {
    name: "EXISTS",
    arity: Arity::Min(2),
    handler: exists,
};

fn exists(args: &[Value], state: &mut State) -> Value {
    let mut count = 0;

    for arg in args {
        if let Value::Bulk(key) = arg
            && state.store.contains_key(key)
        {
            count += 1;
        }
    }

    Value::Integer(count)
}
