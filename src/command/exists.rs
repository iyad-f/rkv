// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::{resp::Value, state::State};

use super::{Arity, Command};

/// `EXISTS key [key ...]` replies with how many of the given keys exist.
pub const COMMAND: Command = Command {
    name: "EXISTS",
    arity: Arity::Min(2),
    handler: exists,
};

fn exists(args: &[Vec<u8>], state: &mut State) -> Value {
    let mut count = 0;

    for key in args {
        if state.store.contains_key(key) {
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
    fn present_key_returns_one() {
        let mut state = state();
        dispatch(&cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "foo"]), &mut state),
            Value::Integer(1)
        );
    }

    #[test]
    fn missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "nope"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn duplicate_keys_count_twice() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k", "k"]), &mut state),
            Value::Integer(2)
        );
    }

    #[test]
    fn counts_only_present_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "a", "b", "c"]), &mut state),
            Value::Integer(1)
        );
    }
}
