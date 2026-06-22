// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::server::State;

/// `PEXPIREAT key ms-timestamp` sets `key` to expire at an absolute time in
/// milliseconds since the Unix epoch, replying with `1` if the expiry was set
/// and `0` if the key does not exist.
pub const COMMAND: Command = Command {
    name: "PEXPIREAT",
    arity: Arity::Exact(3),
    write: true,
    handler: pexpireat,
};

fn pexpireat(ctx: &mut Context, state: &mut State) -> Value {
    let [key, when] = ctx.args else {
        return errors::wrong_args("pexpireat");
    };

    let Some(when) = super::parse_i64(when) else {
        return errors::not_integer();
    };

    super::set_expiry_at(state, key, when)
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;
    use crate::store::Store;

    #[test]
    fn sets_future_expiry() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        let when = (Store::now() + 100_000).to_string();
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", &when]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Value::Integer(100)
        );
    }

    #[test]
    fn past_timestamp_deletes_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "1"]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn missing_key_returns_zero() {
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "99999999999999"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn non_integer_timestamp_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k", "abc"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["PEXPIREAT", "k"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'pexpireat' command".to_string())
        );
    }
}
