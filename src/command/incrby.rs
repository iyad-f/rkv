// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::server::State;

/// `INCRBY key increment` adds `increment` to the integer at `key`, replying with
/// the new value.
pub const COMMAND: Command = Command {
    name: "INCRBY",
    arity: Arity::Exact(3),
    write: true,
    handler: incrby,
};

fn incrby(ctx: &mut Context, state: &mut State) -> Value {
    let [key, increment] = ctx.args else {
        return errors::wrong_args("incrby");
    };

    let Some(increment) = super::parse_i64(increment) else {
        return errors::not_integer();
    };

    super::apply_delta(state, key, increment)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn adds_to_missing_key() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state()),
            Value::Integer(5)
        );
    }

    #[test]
    fn adds_to_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state),
            Value::Integer(15)
        );
    }

    #[test]
    fn negative_increment_decrements() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "-3"]), &mut state),
            Value::Integer(7)
        );
    }

    #[test]
    fn non_integer_increment_is_error() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "abc"]), &mut state()),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'incrby' command".to_string())
        );
    }
}
