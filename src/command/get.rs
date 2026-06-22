// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::object::Object;
use crate::resp::Value;
use crate::server::State;

/// `GET key` returns the value stored at `key`, or nil if it is missing.
pub const COMMAND: Command = Command {
    name: "GET",
    arity: Arity::Exact(2),
    write: false,
    handler: get,
};

fn get(ctx: &mut Context, state: &mut State) -> Value {
    match ctx.args {
        [key] => match state.store.get(key) {
            Some(Object::String(bytes)) => Value::Bulk(bytes.clone()),
            None => Value::Null,
        },
        _ => errors::wrong_args("get"),
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn missing_key_is_null() {
        assert_eq!(dispatch(&cmd(&["GET", "nope"]), &mut state()), Value::Null);
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GET"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'get' command".to_string())
        );
    }
}
