// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command};
use crate::resp::Value;
use crate::state::State;

/// `GET key` returns the value stored at `key`, or nil if it is missing.
pub const COMMAND: Command = Command {
    name: "GET",
    arity: Arity::Exact(2),
    handler: get,
};

fn get(args: &[Value], state: &mut State) -> Value {
    match args {
        [Value::Bulk(key)] => match state.store.get(key) {
            Some(value) => Value::Bulk(value.clone()),
            None => Value::Null,
        },
        _ => super::wrong_args("get"),
    }
}
