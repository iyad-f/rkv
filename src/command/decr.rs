// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;

/// `DECR key` decrements the integer at `key` by one, replying with the new value.
pub const COMMAND: Command = Command {
    name: "DECR",
    arity: Arity::Exact(2),
    handler: decr,
};

fn decr(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key] = args else {
        return errors::wrong_args("decr");
    };

    super::apply_delta(state, key, -1)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn missing_key_starts_at_minus_one() {
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state()),
            Value::Integer(-1)
        );
    }

    #[test]
    fn decrements_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Value::Integer(4)
        );
    }

    #[test]
    fn non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn underflow_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "-9223372036854775808"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Value::Error("ERR increment or decrement would overflow".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECR"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'decr' command".to_string())
        );
    }
}
