// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::{resp::Value, state::State};

use super::{Arity, Command};

/// `DEL key [key ...]` removes the given keys, replying with the number removed.
pub const COMMAND: Command = Command {
    name: "DEL",
    arity: Arity::Min(2),
    handler: del,
};

fn del(args: &[Value], state: &mut State) -> Value {
    let mut count = 0;

    for arg in args {
        if let Value::Bulk(key) = arg
            && state.store.remove(key).is_some()
        {
            count += 1;
        }
    }

    Value::Integer(count)
}
