// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;

/// `DECRBY key decrement` subtracts `decrement` from the integer at `key`,
/// replying with the new value.
pub const COMMAND: Command = Command {
    name: "DECRBY",
    arity: Arity::Exact(3),
    handler: decrby,
};

fn decrby(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key, decrement] = args else {
        return errors::wrong_args("decrby");
    };

    let Some(decrement) = super::parse_i64(decrement) else {
        return errors::not_integer();
    };

    let Some(delta) = decrement.checked_neg() else {
        return errors::decrement_overflow();
    };

    super::apply_delta(state, key, delta)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn subtracts_from_missing_key() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "5"]), &mut state()),
            Value::Integer(-5)
        );
    }

    #[test]
    fn subtracts_from_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "3"]), &mut state),
            Value::Integer(7)
        );
    }

    #[test]
    fn negative_decrement_increments() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "-5"]), &mut state),
            Value::Integer(15)
        );
    }

    #[test]
    fn non_integer_decrement_is_error() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "abc"]), &mut state()),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn unnegatable_decrement_is_error() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "-9223372036854775808"]), &mut state()),
            Value::Error("ERR decrement would overflow".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'decrby' command".to_string())
        );
    }
}
