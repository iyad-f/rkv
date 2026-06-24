// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::object::Object;
use crate::resp::Value;
use crate::server::State;

/// `GET key` returns the value stored at `key`, or nil if it is missing.
pub const GET: Command = Command {
    name: "GET",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: get,
};

fn get(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get(key) {
        Some(Object::String(bytes)) => Value::Bulk(bytes.clone()),
        Some(_) => errors::wrong_type(),
        None => Value::NullBulk,
    }
}

/// `SET key value` stores `value` at `key`.
pub const SET: Command = Command {
    name: "SET",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: set,
};

fn set(ctx: &mut Context, state: &mut State) -> Value {
    let [key, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    state.store.set(key.clone(), Object::String(value.clone()));
    Value::Simple("OK".to_string())
}

/// `APPEND key value` appends to the value at `key`, replying with its new length.
pub const APPEND: Command = Command {
    name: "APPEND",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: append,
};

fn append(ctx: &mut Context, state: &mut State) -> Value {
    let [key, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let mut stored = match state.store.get(key) {
        Some(Object::String(bytes)) => bytes.clone(),
        Some(_) => return errors::wrong_type(),
        None => Vec::new(),
    };
    stored.extend_from_slice(value);
    let len = stored.len() as i64;
    state.store.update(key.clone(), Object::String(stored));
    Value::Integer(len)
}

/// `INCR key` increments the integer at `key` by one, replying with the new value.
pub const INCR: Command = Command {
    name: "INCR",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: incr,
};

fn incr(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    apply_delta(state, key, 1)
}

/// `INCRBY key increment` adds `increment` to the integer at `key`, replying with
/// the new value.
pub const INCRBY: Command = Command {
    name: "INCRBY",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: incrby,
};

fn incrby(ctx: &mut Context, state: &mut State) -> Value {
    let [key, increment] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(increment) = super::parse_i64(increment) else {
        return errors::not_integer();
    };

    apply_delta(state, key, increment)
}

/// `DECR key` decrements the integer at `key` by one, replying with the new value.
pub const DECR: Command = Command {
    name: "DECR",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: decr,
};

fn decr(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    apply_delta(state, key, -1)
}

/// `DECRBY key decrement` subtracts `decrement` from the integer at `key`,
/// replying with the new value.
pub const DECRBY: Command = Command {
    name: "DECRBY",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: decrby,
};

fn decrby(ctx: &mut Context, state: &mut State) -> Value {
    let [key, decrement] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(decrement) = super::parse_i64(decrement) else {
        return errors::not_integer();
    };

    let Some(delta) = decrement.checked_neg() else {
        return errors::decrement_overflow();
    };

    apply_delta(state, key, delta)
}

/// Adds `delta` to the integer stored at `key`, treating a missing key as 0,
/// and replies with the new value.
fn apply_delta(state: &mut State, key: &[u8], delta: i64) -> Value {
    let current = match state.store.get(key) {
        Some(Object::String(bytes)) => match super::parse_i64(bytes) {
            Some(current) => current,
            None => return errors::not_integer(),
        },
        Some(_) => return errors::wrong_type(),
        None => 0,
    };

    let Some(next) = current.checked_add(delta) else {
        return errors::overflow();
    };

    state
        .store
        .update(key.to_vec(), Object::String(next.to_string().into_bytes()));
    Value::Integer(next)
}

#[cfg(test)]
mod tests {
    use crate::command::test_utils::{cmd, dispatch, state};
    use crate::resp::Value;

    // GET

    #[test]
    fn missing_key_is_null() {
        assert_eq!(
            dispatch(&cmd(&["GET", "nope"]), &mut state()),
            Value::NullBulk
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GET"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'get' command".to_string())
        );
    }

    // SET

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
    fn set_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SET", "k"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'set' command".to_string())
        );
    }

    // APPEND

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

    // INCR

    #[test]
    fn missing_key_starts_at_one() {
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state()),
            Value::Integer(1)
        );
    }

    #[test]
    fn increments_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Integer(6)
        );
    }

    #[test]
    fn non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn overflow_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "9223372036854775807"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Value::Error("ERR increment or decrement would overflow".to_string())
        );
    }

    #[test]
    fn incr_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCR"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'incr' command".to_string())
        );
    }

    // INCRBY

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
    fn incrby_non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn incrby_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'incrby' command".to_string())
        );
    }

    // DECR

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
    fn decr_non_integer_value_is_error() {
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
    fn decr_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECR"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'decr' command".to_string())
        );
    }

    // DECRBY

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
    fn decrby_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'decrby' command".to_string())
        );
    }
}
