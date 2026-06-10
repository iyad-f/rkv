// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;

/// `PERSIST key` removes the expiry from `key`, replying with `1` if a TTL was
/// removed and `0` otherwise.
pub const COMMAND: Command = Command {
    name: "PERSIST",
    arity: Arity::Exact(2),
    handler: persist,
};

fn persist(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key] = args else {
        return errors::wrong_args("persist");
    };

    Value::Integer(state.store.persist(key) as i64)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn removes_existing_expiry() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "k"]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Value::Integer(-1)
        );
    }

    #[test]
    fn key_without_expiry_returns_zero() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "k"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "nope"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["PERSIST"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'persist' command".to_string())
        );
    }
}
