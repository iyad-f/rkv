// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Command dispatch and the command table.
//!
//! Each command lives in its own submodule and exposes a [`Command`] descriptor.
//! [`dispatch`] looks the incoming command up in [`COMMANDS`], checks its arity,
//! and calls the handler.

mod append;
mod config;
mod decr;
mod decrby;
mod del;
mod echo;
mod errors;
mod exists;
mod expire;
mod get;
mod incr;
mod incrby;
mod persist;
mod ping;
mod set;
mod ttl;

use std::collections::HashMap;
use std::sync::LazyLock;

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
    pub handler: fn(&[Vec<u8>], &mut State) -> Value,
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
    incr::COMMAND,
    decr::COMMAND,
    incrby::COMMAND,
    decrby::COMMAND,
    expire::COMMAND,
    ttl::COMMAND,
    persist::COMMAND,
];

/// Command name to command mapping.
static COMMAND_TABLE: LazyLock<HashMap<&'static [u8], &'static Command>> =
    LazyLock::new(|| COMMANDS.iter().map(|c| (c.name.as_bytes(), c)).collect());

/// Routes a parsed request to its command and returns the reply.
///
/// `argv` is the command name followed by its arguments, and is never empty.
pub fn dispatch(argv: &[Vec<u8>], state: &mut State) -> Value {
    let name = &argv[0];

    let upper = name.to_ascii_uppercase();
    let command = match COMMAND_TABLE.get(upper.as_slice()).copied() {
        Some(command) => command,
        None => return errors::unknown_command(name, &argv[1..]),
    };

    let arity_ok = match command.arity {
        Arity::Exact(n) => argv.len() == n,
        Arity::Min(n) => argv.len() >= n,
    };
    if !arity_ok {
        return errors::wrong_args(command.name);
    }

    (command.handler)(&argv[1..], state)
}

/// Parses bytes as a signed 64-bit integer, or `None` if they are not one.
fn parse_i64(bytes: &[u8]) -> Option<i64> {
    std::str::from_utf8(bytes).ok().and_then(|s| s.parse().ok())
}

/// Adds `delta` to the integer stored at `key`, treating a missing key as 0,
/// and replies with the new value.
fn apply_delta(state: &mut State, key: &[u8], delta: i64) -> Value {
    let current = match state.store.get(key) {
        Some(value) => match parse_i64(value) {
            Some(current) => current,
            None => return errors::not_integer(),
        },
        None => 0,
    };

    let Some(next) = current.checked_add(delta) else {
        return errors::overflow();
    };

    state
        .store
        .update(key.to_vec(), next.to_string().into_bytes());
    Value::Integer(next)
}

#[cfg(test)]
mod test_utils {
    use super::*;
    use crate::config::Config;

    /// Builds a command's argument vector from its parts.
    pub fn cmd(parts: &[&str]) -> Vec<Vec<u8>> {
        parts.iter().map(|p| p.as_bytes().to_vec()).collect()
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
            dispatch(&cmd(&["ping"]), &mut state()),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn unknown_command_reports_args() {
        assert_eq!(
            dispatch(&cmd(&["FOOBAR", "x"]), &mut state()),
            Value::Error(
                "ERR unknown command 'FOOBAR', with args beginning with: 'x' ".to_string()
            )
        );
    }
}
