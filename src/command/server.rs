// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::server::State;

/// `CONFIG GET key` reads a setting, `CONFIG SET key value` updates one.
pub const CONFIG: Command = Command {
    name: "CONFIG",
    arity: Arity::Min(2),
    write: false,
    handler: config,
};

fn config(ctx: &mut Context, state: &mut State) -> Value {
    let Some(subcommand) = ctx.args.first() else {
        return errors::wrong_args(ctx.command.name);
    };

    match subcommand.to_ascii_uppercase().as_slice() {
        b"GET" => config_get(&ctx.args[1..], state),
        b"SET" => config_set(&ctx.args[1..], state),
        b"HELP" => config_help(),
        _ => Value::Error(format!(
            "ERR unknown subcommand '{}'. Try CONFIG HELP.",
            String::from_utf8_lossy(subcommand)
        )),
    }
}

/// Handles `CONFIG GET key`, replying with a `[key, value]` array, or an empty
/// array if the key is unknown.
fn config_get(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key] = args else {
        return errors::wrong_args("config|get");
    };
    let key = String::from_utf8_lossy(key);

    match state.config.get(&key) {
        Some(value) => Value::Array(vec![
            Value::Bulk(key.into_owned().into_bytes()),
            Value::Bulk(value.into_bytes()),
        ]),
        None => Value::Array(Vec::new()),
    }
}

/// Handles `CONFIG SET key value`, updating the live configuration.
fn config_set(args: &[Vec<u8>], state: &mut State) -> Value {
    let [key, value] = args else {
        return errors::wrong_args("config|set");
    };
    let key = String::from_utf8_lossy(key);
    let value = String::from_utf8_lossy(value);

    match state.config.set(&key, &value) {
        Ok(()) => Value::Simple("OK".to_string()),
        Err(e) => Value::Error(format!("ERR CONFIG SET failed, {e}")),
    }
}

/// Handles `CONFIG HELP`, listing the supported subcommands.
fn config_help() -> Value {
    const LINES: &[&str] = &[
        "CONFIG <subcommand> [<arg> ...]. Subcommands are:",
        "GET <parameter>",
        "    Return the value of a configuration parameter.",
        "SET <parameter> <value>",
        "    Set a configuration parameter to a value.",
        "HELP",
        "    Print this help.",
    ];

    Value::Array(
        LINES
            .iter()
            .map(|line| Value::Bulk(line.as_bytes().to_vec()))
            .collect(),
    )
}

/// `DBSIZE` replies with the number of keys in the database.
pub const DBSIZE: Command = Command {
    name: "DBSIZE",
    arity: Arity::Exact(1),
    write: false,
    handler: dbsize,
};

fn dbsize(_ctx: &mut Context, state: &mut State) -> Value {
    Value::Integer(state.store.len() as i64)
}

/// `BGREWRITEAOF` rewrites the append-only file in the background, replying once
/// the rewrite has been started.
pub const BGREWRITEAOF: Command = Command {
    name: "BGREWRITEAOF",
    arity: Arity::Exact(1),
    write: false,
    handler: bgrewriteaof,
};

fn bgrewriteaof(_ctx: &mut Context, state: &mut State) -> Value {
    if state.aof_rewrite_in_progress() {
        return Value::Error(
            "ERR Background append only file rewriting already in progress".to_string(),
        );
    }

    match state.start_aof_rewrite() {
        Ok(()) => Value::Simple("Background append only file rewriting started".to_string()),
        Err(e) => Value::Error(format!("ERR {e}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    // CONFIG

    #[test]
    fn get_returns_value() {
        assert_eq!(
            dispatch(&cmd(&["CONFIG", "GET", "maxclients"]), &mut state()),
            Value::Array(vec![
                Value::Bulk(b"maxclients".to_vec()),
                Value::Bulk(b"1024".to_vec()),
            ])
        );
    }

    #[test]
    fn unknown_subcommand_keeps_original_case() {
        assert_eq!(
            dispatch(&cmd(&["CONFIG", "BadSub"]), &mut state()),
            Value::Error("ERR unknown subcommand 'BadSub'. Try CONFIG HELP.".to_string())
        );
    }

    #[test]
    fn get_unknown_key_is_empty() {
        assert_eq!(
            dispatch(&cmd(&["CONFIG", "GET", "nope"]), &mut state()),
            Value::Array(vec![])
        );
    }

    #[test]
    fn set_updates_value() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["CONFIG", "SET", "maxclients", "50"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["CONFIG", "GET", "maxclients"]), &mut state),
            Value::Array(vec![
                Value::Bulk(b"maxclients".to_vec()),
                Value::Bulk(b"50".to_vec()),
            ])
        );
    }

    #[test]
    fn help_lists_subcommands() {
        match dispatch(&cmd(&["CONFIG", "HELP"]), &mut state()) {
            Value::Array(lines) => {
                assert_eq!(
                    lines.first(),
                    Some(&Value::Bulk(
                        b"CONFIG <subcommand> [<arg> ...]. Subcommands are:".to_vec()
                    ))
                );
            }
            other => panic!("expected an array, got {other:?}"),
        }
    }

    // DBSIZE

    #[test]
    fn empty_store_is_zero() {
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state()), Value::Integer(0));
    }

    #[test]
    fn counts_stored_keys() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        dispatch(&cmd(&["SET", "b", "2"]), &mut state);
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state), Value::Integer(2));
    }

    #[test]
    fn overwriting_a_key_does_not_double_count() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        dispatch(&cmd(&["SET", "a", "2"]), &mut state);
        assert_eq!(dispatch(&cmd(&["DBSIZE"]), &mut state), Value::Integer(1));
    }

    #[test]
    fn dbsize_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DBSIZE", "x"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'dbsize' command".to_string())
        );
    }

    // BGREWRITEAOF

    #[test]
    fn bgrewriteaof_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["BGREWRITEAOF", "x"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'bgrewriteaof' command".to_string())
        );
    }
}
