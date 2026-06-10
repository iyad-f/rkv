// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::{resp::Value, state::State};

use super::{Arity, Command};

/// `DEL key [key ...]` removes the given keys, replying with the number removed.
pub const COMMAND: Command = Command {
    name: "DEL",
    arity: Arity::Min(2),
    handler: del,
};

fn del(args: &[Vec<u8>], state: &mut State) -> Value {
    let mut count = 0;

    for key in args {
        if state.store.remove(key) {
            count += 1;
        }
    }

    Value::Integer(count)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn removes_existing_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "foo"]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(dispatch(&cmd(&["GET", "foo"]), &mut state), Value::Null);
    }

    #[test]
    fn missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["DEL", "missing"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn counts_only_present_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "a", "b", "c"]), &mut state),
            Value::Integer(1)
        );
    }

    #[test]
    fn duplicate_keys_count_once() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "k", "k"]), &mut state),
            Value::Integer(1)
        );
    }
}
