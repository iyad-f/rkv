// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::object::Object;
use crate::resp::Response;
use crate::server::State;
use crate::store::{Expiry, Store};

/// `DEL key [key ...]` removes the given keys, replying with the number removed.
pub const DEL: Command = Command {
    name: "DEL",
    arity: Arity::Min(2),
    write: true,
    auth_required: true,
    handler: del,
};

fn del(ctx: &mut Context, state: &mut State) -> Response {
    let mut count = 0;

    for key in ctx.args {
        if state.store.remove(key, state.config.lazy_drop.user_del) {
            count += 1;
        }
    }

    Response::Integer(count)
}

/// `EXISTS key [key ...]` replies with how many of the given keys exist.
pub const EXISTS: Command = Command {
    name: "EXISTS",
    arity: Arity::Min(2),
    write: false,
    auth_required: true,
    handler: exists,
};

fn exists(ctx: &mut Context, state: &mut State) -> Response {
    let mut count = 0;

    for key in ctx.args {
        if state.store.contains_key(key) {
            count += 1;
        }
    }

    Response::Integer(count)
}

/// `EXPIRE key seconds` sets `key` to expire after `seconds`, replying with `1`
/// if the expiry was set and `0` if the key does not exist.
pub const EXPIRE: Command = Command {
    name: "EXPIRE",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: expire,
};

fn expire(ctx: &mut Context, state: &mut State) -> Response {
    let [key, seconds] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(seconds) = super::parse_i64(seconds) else {
        return errors::not_integer();
    };

    let Some(when) = seconds
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(Store::now()))
    else {
        return errors::invalid_expire_time(ctx.command.name);
    };

    let reply = set_expiry_at(state, key, when);

    // Log the absolute deadline so a replay does not re-derive it from a later
    // clock.
    if matches!(reply, Response::Integer(1)) {
        ctx.rewrite = Some(vec![
            b"PEXPIREAT".to_vec(),
            key.clone(),
            when.to_string().into_bytes(),
        ]);
    }

    reply
}

/// `PERSIST key` removes the expiry from `key`, replying with `1` if a TTL was
/// removed and `0` otherwise.
pub const PERSIST: Command = Command {
    name: "PERSIST",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: persist,
};

fn persist(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    Response::Integer(state.store.persist(key) as i64)
}

/// `PEXPIREAT key ms-timestamp` sets `key` to expire at an absolute time in
/// milliseconds since the Unix epoch, replying with `1` if the expiry was set
/// and `0` if the key does not exist.
pub const PEXPIREAT: Command = Command {
    name: "PEXPIREAT",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: pexpireat,
};

fn pexpireat(ctx: &mut Context, state: &mut State) -> Response {
    let [key, when] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(when) = super::parse_i64(when) else {
        return errors::not_integer();
    };

    set_expiry_at(state, key, when)
}

/// `TTL key` returns the remaining time to live of `key` in seconds, `-2` if
/// the key does not exist, or `-1` if it exists but has no expiry.
pub const TTL: Command = Command {
    name: "TTL",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: ttl,
};

fn ttl(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.expiry(key) {
        Expiry::Missing => Response::Integer(-2),
        Expiry::Never => Response::Integer(-1),
        Expiry::At(deadline) => {
            let remaining = (deadline - Store::now()).max(0);
            Response::Integer((remaining + 500) / 1000)
        }
    }
}

/// `TYPE key` returns the type of the value held at `key`, or `none` if the key
/// does not exist.
pub const TYPE: Command = Command {
    name: "TYPE",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: r#type,
};

fn r#type(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let type_name = state.store.get(key).map_or("none", Object::type_name);
    Response::Simple(type_name.to_string())
}

/// `UNLINK key [key ...]` removes the given keys, reclaiming large values on a
/// background thread rather than inline. Replies with the number removed.
pub const UNLINK: Command = Command {
    name: "UNLINK",
    arity: Arity::Min(2),
    write: true,
    auth_required: true,
    handler: unlink,
};

fn unlink(ctx: &mut Context, state: &mut State) -> Response {
    let mut count = 0;

    for key in ctx.args {
        if state.store.remove(key, true) {
            count += 1;
        }
    }

    Response::Integer(count)
}

/// Applies the absolute expiry `when` (milliseconds since the Unix epoch) to
/// `key`, deleting it if the deadline has already passed. Replies `1` if the key
/// exists and `0` if it does not.
fn set_expiry_at(state: &mut State, key: &[u8], when: i64) -> Response {
    if !state.store.contains_key(key) {
        return Response::Integer(0);
    }

    if Store::is_expired(when) {
        state.store.remove_expired(key);
    } else {
        state.store.set_expiry(key, when);
    }

    Response::Integer(1)
}

#[cfg(test)]
mod tests {
    use crate::command::test_utils::{cmd, dispatch, state};
    use crate::resp::Response;
    use crate::store::Store;

    // DEL

    #[test]
    fn removes_existing_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "foo"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "foo"]), &mut state),
            Response::NullBulk
        );
    }

    #[test]
    fn del_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["DEL", "missing"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn del_counts_only_present_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "a", "b", "c"]), &mut state),
            Response::Integer(1)
        );
    }

    #[test]
    fn duplicate_keys_count_once() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DEL", "k", "k"]), &mut state),
            Response::Integer(1)
        );
    }

    // EXISTS

    #[test]
    fn present_key_returns_one() {
        let mut state = state();
        dispatch(&cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "foo"]), &mut state),
            Response::Integer(1)
        );
    }

    #[test]
    fn exists_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "nope"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn duplicate_keys_count_twice() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k", "k"]), &mut state),
            Response::Integer(2)
        );
    }

    #[test]
    fn exists_counts_only_present_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "a", "b", "c"]), &mut state),
            Response::Integer(1)
        );
    }

    // EXPIRE

    #[test]
    fn sets_expiry_on_existing_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state),
            Response::Integer(1)
        );
    }

    #[test]
    fn expire_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn negative_seconds_deletes_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "-1"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn non_integer_seconds_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "abc"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn overflowing_seconds_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k", "9999999999999999"]), &mut state),
            Response::Error("ERR invalid expire time in 'expire' command".to_string())
        );
    }

    #[test]
    fn expire_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["EXPIRE", "k"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'expire' command".to_string())
        );
    }

    // PERSIST

    #[test]
    fn removes_existing_expiry() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "k"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn key_without_expiry_returns_zero() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "k"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn persist_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["PERSIST", "nope"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn persist_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["PERSIST"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'persist' command".to_string())
        );
    }

    // PEXPIREAT

    #[test]
    fn sets_future_expiry() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        let when = (Store::now() + 100_000).to_string();
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", &when]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn past_timestamp_deletes_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "1"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn pexpireat_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "99999999999999"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn non_integer_timestamp_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "abc"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn pexpireat_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'pexpireat' command".to_string())
        );
    }

    // TTL

    #[test]
    fn missing_key_is_negative_two() {
        assert_eq!(
            dispatch(&cmd(&["TTL", "nope"]), &mut state()),
            Response::Integer(-2)
        );
    }

    #[test]
    fn key_without_expiry_is_negative_one() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn reports_remaining_seconds() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn ttl_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["TTL"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'ttl' command".to_string())
        );
    }

    // TYPE

    #[test]
    fn type_of_string_is_string() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TYPE", "k"]), &mut state),
            Response::Simple("string".to_string())
        );
    }

    #[test]
    fn type_of_list_is_list() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TYPE", "k"]), &mut state),
            Response::Simple("list".to_string())
        );
    }

    #[test]
    fn type_missing_key_is_none() {
        assert_eq!(
            dispatch(&cmd(&["TYPE", "nope"]), &mut state()),
            Response::Simple("none".to_string())
        );
    }

    #[test]
    fn type_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["TYPE"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'type' command".to_string())
        );
    }

    // UNLINK

    #[test]
    fn unlinks_existing_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["UNLINK", "foo"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "foo"]), &mut state),
            Response::NullBulk
        );
    }

    #[test]
    fn unlink_missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["UNLINK", "missing"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn unlink_counts_only_present_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["UNLINK", "a", "b", "c"]), &mut state),
            Response::Integer(1)
        );
    }

    #[test]
    fn unlink_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["UNLINK"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'unlink' command".to_string())
        );
    }
}
