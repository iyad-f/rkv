// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::{resp::Value, server::State};

use super::{Arity, Command, Context};

/// `DBSIZE` replies with the number of keys in the database.
pub const COMMAND: Command = Command {
    name: "DBSIZE",
    arity: Arity::Exact(1),
    write: false,
    handler: dbsize,
};

fn dbsize(_ctx: &mut Context, state: &mut State) -> Value {
    Value::Integer(state.store.len() as i64)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn empty_store_is_zero() {
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state()), Value::Integer(0));
    }

    #[test]
    fn counts_stored_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        dispatch(&cmd(&["SET", "b", "2"]), &mut state);
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state), Value::Integer(2));
    }

    #[test]
    fn overwriting_a_key_does_not_double_count() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        dispatch(&cmd(&["SET", "a", "2"]), &mut state);
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state), Value::Integer(1));
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DBSIZE", "x"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'dbsize' command".to_string())
        );
    }
}
