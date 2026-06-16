// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::state::State;
use crate::store::{Expiry, Store};

/// `TTL key` returns the remaining time to live of `key` in seconds, `-2` if
/// the key does not exist, or `-1` if it exists but has no expiry.
pub const COMMAND: Command = Command {
    name: "TTL",
    arity: Arity::Exact(2),
    write: false,
    handler: ttl,
};

fn ttl(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args("ttl");
    };

    match state.store.expiry(key) {
        Expiry::Missing => Value::Integer(-2),
        Expiry::Never => Value::Integer(-1),
        Expiry::At(deadline) => {
            let remaining = (deadline - Store::now()).max(0);
            Value::Integer((remaining + 500) / 1000)
        }
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
    fn missing_key_is_negative_two() {
        assert_eq!(
            dispatch(&cmd(&["TTL", "nope"]), &mut state()),
            Value::Integer(-2)
        );
    }

    #[test]
    fn key_without_expiry_is_negative_one() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Value::Integer(-1)
        );
    }

    #[test]
    fn reports_remaining_seconds() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Value::Integer(100)
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["TTL"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'ttl' command".to_string())
        );
    }
}
