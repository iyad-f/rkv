// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Command dispatch and the command table.
//!
//! Each command lives in its own submodule and exposes a [`Command`] descriptor.
//! [`dispatch`] looks the incoming command up in [`COMMANDS`], checks its arity,
//! and calls the handler.

mod config;
mod del;
mod echo;
mod get;
mod ping;
mod set;

use crate::resp::Value;
use crate::state::State;

/// How many elements a command expects, counting the command name itself.
pub enum Arity {
    /// Exactly this many elements, for example `GET key` is `Exact(2)`.
    Exact(usize),

    /// At least this many elements, for example `PING [message]` is `Min(1)`.
    Min(usize),
}

/// A command's metadata together with the function that runs it.
pub struct Command {
    /// The command name, uppercase.
    pub name: &'static str,

    /// How many elements the command expects.
    pub arity: Arity,

    /// Runs the command, given the arguments after the name.
    pub handler: fn(&[Value], &mut State) -> Value,
}

/// Every command the server knows.
const COMMANDS: &[Command] = &[
    ping::COMMAND,
    echo::COMMAND,
    get::COMMAND,
    set::COMMAND,
    config::COMMAND,
    del::COMMAND,
];

/// Routes a parsed request to its command and returns the reply.
pub fn dispatch(request: Value, state: &mut State) -> Value {
    let items = match request {
        Value::Array(items) => items,
        _ => return Value::Error("ERR Protocol error".to_string()),
    };

    let name = match items.first() {
        Some(Value::Bulk(name)) => name,
        _ => return Value::Error("ERR Protocol error".to_string()),
    };

    let upper = name.to_ascii_uppercase();
    let command = match COMMANDS
        .iter()
        .find(|c| c.name.as_bytes() == upper.as_slice())
    {
        Some(command) => command,
        None => return unknown_command(name, &items[1..]),
    };

    let arity_ok = match command.arity {
        Arity::Exact(n) => items.len() == n,
        Arity::Min(n) => items.len() >= n,
    };
    if !arity_ok {
        return wrong_args(command.name);
    }

    (command.handler)(&items[1..], state)
}

/// Builds the standard reply for a command called with the wrong argument count.
fn wrong_args(command: &str) -> Value {
    Value::Error(format!(
        "ERR wrong number of arguments for '{}' command",
        command.to_ascii_lowercase()
    ))
}

/// Builds the standard reply for an unrecognized command.
fn unknown_command(name: &[u8], args: &[Value]) -> Value {
    let name = String::from_utf8_lossy(name);

    let mut list = String::new();
    for arg in args {
        if let Value::Bulk(arg) = arg {
            list.push_str(&format!("'{}' ", String::from_utf8_lossy(arg)));
        }
    }

    Value::Error(format!(
        "ERR unknown command '{name}', with args beginning with: {list}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cmd(parts: &[&str]) -> Value {
        Value::Array(
            parts
                .iter()
                .map(|p| Value::Bulk(p.as_bytes().to_vec()))
                .collect(),
        )
    }

    fn state() -> State {
        State::new(Config::default())
    }

    #[test]
    fn ping_no_arg() {
        assert_eq!(
            dispatch(cmd(&["PING"]), &mut state()),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn ping_with_message() {
        assert_eq!(
            dispatch(cmd(&["PING", "hi"]), &mut state()),
            Value::Bulk(b"hi".to_vec())
        );
    }

    #[test]
    fn ping_too_many_args() {
        assert_eq!(
            dispatch(cmd(&["PING", "a", "b"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'ping' command".to_string())
        );
    }

    #[test]
    fn echo_returns_argument() {
        assert_eq!(
            dispatch(cmd(&["ECHO", "hello"]), &mut state()),
            Value::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn echo_wrong_args() {
        assert_eq!(
            dispatch(cmd(&["ECHO"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'echo' command".to_string())
        );
    }

    #[test]
    fn set_then_get() {
        let mut state = state();
        assert_eq!(
            dispatch(cmd(&["SET", "foo", "bar"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(cmd(&["GET", "foo"]), &mut state),
            Value::Bulk(b"bar".to_vec())
        );
    }

    #[test]
    fn set_overwrites_existing() {
        let mut state = state();
        dispatch(cmd(&["SET", "k", "v1"]), &mut state);
        dispatch(cmd(&["SET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(cmd(&["GET", "k"]), &mut state),
            Value::Bulk(b"v2".to_vec())
        );
    }

    #[test]
    fn set_wrong_args() {
        assert_eq!(
            dispatch(cmd(&["SET", "k"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'set' command".to_string())
        );
    }

    #[test]
    fn get_missing_is_null() {
        assert_eq!(dispatch(cmd(&["GET", "nope"]), &mut state()), Value::Null);
    }

    #[test]
    fn get_wrong_args() {
        assert_eq!(
            dispatch(cmd(&["GET"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'get' command".to_string())
        );
    }

    #[test]
    fn del_removes_existing_key() {
        let mut state = state();
        dispatch(cmd(&["SET", "foo", "bar"]), &mut state);
        assert_eq!(
            dispatch(cmd(&["DEL", "foo"]), &mut state),
            Value::Integer(1)
        );
        assert_eq!(dispatch(cmd(&["GET", "foo"]), &mut state), Value::Null);
    }

    #[test]
    fn del_missing_key_returns_zero() {
        assert_eq!(
            dispatch(cmd(&["DEL", "missing"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn del_counts_only_present_keys() {
        let mut state = state();
        dispatch(cmd(&["SET", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(cmd(&["DEL", "a", "b", "c"]), &mut state),
            Value::Integer(1)
        );
    }

    #[test]
    fn del_does_not_double_count_duplicates() {
        let mut state = state();
        dispatch(cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(cmd(&["DEL", "k", "k"]), &mut state),
            Value::Integer(1)
        );
    }

    #[test]
    fn command_name_is_case_insensitive() {
        assert_eq!(
            dispatch(cmd(&["ping"]), &mut state()),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn unknown_command_reports_args() {
        assert_eq!(
            dispatch(cmd(&["FOOBAR", "x"]), &mut state()),
            Value::Error(
                "ERR unknown command 'FOOBAR', with args beginning with: 'x' ".to_string()
            )
        );
    }

    #[test]
    fn non_array_request_is_protocol_error() {
        assert_eq!(
            dispatch(Value::Integer(1), &mut state()),
            Value::Error("ERR Protocol error".to_string())
        );
    }

    #[test]
    fn config_get_returns_value() {
        assert_eq!(
            dispatch(cmd(&["CONFIG", "GET", "maxclients"]), &mut state()),
            Value::Array(vec![
                Value::Bulk(b"maxclients".to_vec()),
                Value::Bulk(b"1024".to_vec()),
            ])
        );
    }

    #[test]
    fn config_get_unknown_is_empty() {
        assert_eq!(
            dispatch(cmd(&["CONFIG", "GET", "nope"]), &mut state()),
            Value::Array(vec![])
        );
    }

    #[test]
    fn config_set_updates_value() {
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
