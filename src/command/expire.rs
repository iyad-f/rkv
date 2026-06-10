// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, errors};
use crate::resp::Value;
use crate::state::State;
use crate::store::Store;

/// `EXPIRE key seconds` sets `key` to expire after `seconds`, replying with `1`
/// if the expiry was set and `0` if the key does not exist.
pub const COMMAND: Command = Command {
    name: "EXPIRE",
    arity: Arity::Exact(3),
    handler: expire,
};

fn expire(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key, seconds] = args else {
        return errors::wrong_args("expire");
    };

    let Some(seconds) = super::parse_i64(seconds) else {
        return errors::not_integer();
    };

    let now = Store::now();
    let Some(when) = seconds.checked_mul(1000).and_then(|ms| ms.checked_add(now)) else {
        return errors::invalid_expire_time("expire");
    };

    if !state.store.contains_key(key) {
        return Value::Integer(0);
    }

    if when <= now {
        state.store.remove(key);
        return Value::Integer(1);
    }

    state.store.set_expiry(key, when);
    Value::Integer(1)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    #[test]
    fn sets_expiry_on_existing_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state),
            Value::Integer(1)
        );
    }

    #[test]
    fn missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn negative_seconds_deletes_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "-1"]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn non_integer_seconds_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "abc"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn overflowing_seconds_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "9999999999999999"]), &mut state),
            Value::Error("ERR invalid expire time in 'expire' command".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'expire' command".to_string())
        );
    }
}
