// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;

/// `APPEND key value` appends to the value at `key`, replying with its new length.
pub const COMMAND: Command = Command {
    name: "APPEND",
    arity: Arity::Exact(3),
    handler: append,
};

fn append(args: &[Vec<u8>], state: &mut State) -> Value {
    match args {
        [key, value] => {
            let stored = state.store.entry(key.clone()).or_default();
            stored.extend_from_slice(value);
            Value::Integer(stored.len() as i64)
        }
        _ => errors::wrong_args("append"),
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
    fn creates_missing_key() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "hello"]), &mut state),
            Value::Integer(5)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Value::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn appends_to_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "hello"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", " world"]), &mut state),
            Value::Integer(11)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Value::Bulk(b"hello world".to_vec())
        );
    }

    #[test]
    fn returns_new_length() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "ab"]), &mut state),
            Value::Integer(2)
        );
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "cde"]), &mut state),
            Value::Integer(5)
        );
    }
}
