// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Command dispatch and the command table.
//!
//! Each command lives in its own submodule and exposes a [`Command`] descriptor.
//! [`dispatch`] looks the incoming command up in [`COMMANDS`], checks its arity,
//! and calls the handler.

mod append;
mod config;
mod del;
mod echo;
mod exists;
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
    exists::COMMAND,
    append::COMMAND,
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
mod test_utils {
    use super::*;
    use crate::config::Config;

    /// Builds an array request from its parts, each as a bulk string.
    pub fn cmd(parts: &[&str]) -> Value {
        Value::Array(
            parts
                .iter()
                .map(|p| Value::Bulk(p.as_bytes().to_vec()))
                .collect(),
        )
    }

    /// Creates empty state with the default configuration.
    pub fn state() -> State {
        State::new(Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::{cmd, state};

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
}
