// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command};
use crate::resp::Value;
use crate::state::State;

/// `SET key value` stores `value` at `key`.
pub const COMMAND: Command = Command {
    name: "SET",
    arity: Arity::Exact(3),
    handler: set,
};

fn set(args: &[Value], state: &mut State) -> Value {
    match args {
        [Value::Bulk(key), Value::Bulk(value)] => {
            state.store.insert(key.clone(), value.clone());
            Value::Simple("OK".to_string())
        }
        _ => super::wrong_args("set"),
    }
}
