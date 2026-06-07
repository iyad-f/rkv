// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command};
use crate::resp::Value;
use crate::state::State;

/// `CONFIG GET key` reads a setting, `CONFIG SET key value` updates one.
pub const COMMAND: Command = Command {
    name: "CONFIG",
    arity: Arity::Min(2),
    handler: config,
};

fn config(args: &[Value], state: &mut State) -> Value {
    let subcommand = match args.first() {
        Some(Value::Bulk(subcommand)) => subcommand.to_ascii_uppercase(),
        _ => return super::wrong_args("config"),
    };

    match subcommand.as_slice() {
        b"GET" => config_get(&args[1..], state),
        b"SET" => config_set(&args[1..], state),
        _ => Value::Error(format!(
            "ERR Unknown CONFIG subcommand '{}'",
            String::from_utf8_lossy(&subcommand)
        )),
    }
}

/// Handles `CONFIG GET key`, replying with a `[key, value]` array, or an empty
/// array if the key is unknown.
fn config_get(args: &[Value], state: &mut State) -> Value {
    let key = match args {
        [Value::Bulk(key)] => String::from_utf8_lossy(key),
        _ => return super::wrong_args("config|get"),
    };

    match state.config.get(&key) {
        Some(value) => Value::Array(vec![
            Value::Bulk(key.into_owned().into_bytes()),
            Value::Bulk(value.into_bytes()),
        ]),
        None => Value::Array(Vec::new()),
    }
}

/// Handles `CONFIG SET key value`, updating the live configuration.
fn config_set(args: &[Value], state: &mut State) -> Value {
    let (key, value) = match args {
        [Value::Bulk(key), Value::Bulk(value)] => {
            (String::from_utf8_lossy(key), String::from_utf8_lossy(value))
        }
        _ => return super::wrong_args("config|set"),
    };

    match state.config.set(&key, &value) {
        Ok(()) => Value::Simple("OK".to_string()),
        Err(e) => Value::Error(format!("ERR CONFIG SET failed, {e}")),
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
    fn get_returns_value() {
        assert_eq!(
            dispatch(cmd(&["CONFIG", "GET", "maxclients"]), &mut state()),
            Value::Array(vec![
                Value::Bulk(b"maxclients".to_vec()),
                Value::Bulk(b"1024".to_vec()),
            ])
        );
    }

    #[test]
    fn get_unknown_key_is_empty() {
        assert_eq!(
            dispatch(cmd(&["CONFIG", "GET", "nope"]), &mut state()),
            Value::Array(vec![])
        );
    }

    #[test]
    fn set_updates_value() {
        let mut state = state();
        assert_eq!(
            dispatch(cmd(&["CONFIG", "SET", "maxclients", "50"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(cmd(&["CONFIG", "GET", "maxclients"]), &mut state),
            Value::Array(vec![
                Value::Bulk(b"maxclients".to_vec()),
                Value::Bulk(b"50".to_vec()),
            ])
        );
    }
}
