// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::server::State;

/// `INCR key` increments the integer at `key` by one, replying with the new value.
pub const COMMAND: Command = Command {
    name: "INCR",
    arity: Arity::Exact(2),
    write: true,
    handler: incr,
};

fn incr(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args("incr");
    };

    super::apply_delta(state, key, 1)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn missing_key_starts_at_one() {
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state()),
            Value::Integer(1)
        );
    }

    #[test]
    fn increments_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Integer(6)
        );
    }

    #[test]
    fn non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn overflow_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "9223372036854775807"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Error("ERR increment or decrement would overflow".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCR"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'incr' command".to_string())
        );
    }
}
