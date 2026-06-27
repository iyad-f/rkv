// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Command dispatch and the command table.
//!
//! Each command lives in its own submodule and exposes a [`Command`] descriptor.
//! [`dispatch`] looks the incoming command up in [`COMMANDS`], checks its arity,
//! and calls the handler.

mod connection;
mod errors;
mod generic;
mod list;
mod server;
mod string;

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::resp::Value;
use crate::server::State;
use crate::session::Session;

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

    /// Whether the command can modify the dataset.
    pub write: bool,

    /// Whether the command requires the connection to have authenticated.
    pub auth_required: bool,

    /// Runs the command.
    pub handler: fn(&mut Context, &mut State) -> Value,
}

/// The per-invocation context of a command.
pub struct Context<'a, 's> {
    /// The command being invoked.
    pub command: &'static Command,

    /// The arguments after the command name.
    pub args: &'a [Vec<u8>],

    /// The session of the connection that issued the command.
    pub session: &'s mut Session,

    /// A command to log in place of the running one, set by a handler that must
    /// persist a different, deterministic form of itself (e.g. `EXPIRE` as
    /// `PEXPIREAT`).
    pub rewrite: Option<Vec<Vec<u8>>>,
}

/// Every command the server knows.
const COMMANDS: &[Command] = &[
    // Connection
    connection::AUTH,
    connection::ECHO,
    connection::PING,
    connection::QUIT,
    connection::RESET,
    // String
    string::APPEND,
    string::DECR,
    string::DECRBY,
    string::GET,
    string::GETDEL,
    string::GETEX,
    string::GETRANGE,
    string::GETSET,
    string::INCR,
    string::INCRBY,
    string::INCRBYFLOAT,
    string::LCS,
    string::MGET,
    string::MSET,
    string::MSETNX,
    string::PSETEX,
    string::SET,
    string::SETEX,
    string::SETNX,
    string::SETRANGE,
    string::STRLEN,
    string::SUBSTR,
    // Generic
    generic::DEL,
    generic::EXISTS,
    generic::EXPIRE,
    generic::PERSIST,
    generic::PEXPIREAT,
    generic::TTL,
    // List
    list::LINDEX,
    list::LINSERT,
    list::LLEN,
    list::LMOVE,
    list::LMPOP,
    list::LPOP,
    list::LPOS,
    list::LPUSH,
    list::LPUSHX,
    list::LRANGE,
    list::LREM,
    list::LSET,
    list::LTRIM,
    list::RPOP,
    list::RPUSH,
    list::RPUSHX,
    // Server
    server::BGREWRITEAOF,
    server::CONFIG,
    server::DBSIZE,
];

/// Command name to command mapping.
static COMMAND_TABLE: LazyLock<HashMap<&'static [u8], &'static Command>> =
    LazyLock::new(|| COMMANDS.iter().map(|c| (c.name.as_bytes(), c)).collect());

/// Routes a parsed request to its command and returns the reply.
///
/// `argv` is the command name followed by its arguments, and is never empty.
pub fn dispatch(argv: &[Vec<u8>], state: &mut State, session: &mut Session) -> Value {
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

    // Reject commands until the connection authenticates, once a password is
    // configured. A few commands (e.g. AUTH, QUIT) are allowed through so a
    // client can authenticate or disconnect.
    if command.auth_required && state.config.password.is_some() && !session.is_authenticated() {
        return errors::noauth();
    }

    // Refuse writes while the append-only file is in a failed-write state, so
    // changes that cannot be persisted are not accepted.
    if command.write && !state.aof.write_ok() {
        return errors::misconf();
    }

    let mut ctx = Context {
        command,
        args: &argv[1..],
        session,
        rewrite: None,
    };
    let dirty = state.store.dirty();
    let reply = (command.handler)(&mut ctx, state);

    // Log only when the handler actually changed the keyspace. A handler may
    // rewrite what gets logged (e.g. EXPIRE -> PEXPIREAT) so a replay is
    // deterministic. The append is a no-op while logging is disabled.
    if command.write
        && state.store.dirty() != dirty
        && let Err(e) = state.aof.append(
            ctx.rewrite.as_deref().unwrap_or(argv),
            state.config.aof.fsync,
        )
    {
        eprintln!("aof append failed: {e}");
    }

    reply
}

/// Parses bytes as a signed 64-bit integer, or `None` if they are not one.
fn parse_i64(bytes: &[u8]) -> Option<i64> {
    std::str::from_utf8(bytes).ok().and_then(|s| s.parse().ok())
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

    /// Dispatches a command with a throwaway session, for tests that do not
    /// exercise per-connection state.
    pub fn dispatch(argv: &[Vec<u8>], state: &mut State) -> Value {
        super::dispatch(argv, state, &mut Session::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::{cmd, dispatch, state};

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
