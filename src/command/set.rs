// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;

/// `SET key value` stores `value` at `key`.
pub const COMMAND: Command = Command {
    name: "SET",
    arity: Arity::Exact(3),
    handler: set,
};

fn set(args: &[Vec<u8>], state: &mut State) -> Value {
    match args {
        [key, value] => {
            state.store.set(key.clone(), value.clone());
            Value::Simple("OK".to_string())
        }
        _ => errors::wrong_args("set"),
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
    fn stored_value_is_readable() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "foo", "bar"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "foo"]), &mut state),
            Value::Bulk(b"bar".to_vec())
        );
    }

    #[test]
    fn overwrites_existing() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v1"]), &mut state);
        dispatch(&cmd(&["SET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Value::Bulk(b"v2".to_vec())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SET", "k"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'set' command".to_string())
        );
    }
}
