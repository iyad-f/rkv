// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::server::State;

/// `PING [message]` replies with PONG, or echoes the optional message.
pub const PING: Command = Command {
    name: "PING",
    arity: Arity::Min(1),
    write: false,
    handler: ping,
};

fn ping(ctx: &mut Context, _state: &mut State) -> Value {
    match ctx.args {
        [] => Value::Simple("PONG".to_string()),
        [message] => Value::Bulk(message.clone()),
        _ => errors::wrong_args(ctx.command.name),
    }
}

/// `ECHO message` replies with the message.
pub const ECHO: Command = Command {
    name: "ECHO",
    arity: Arity::Exact(2),
    write: false,
    handler: echo,
};

fn echo(ctx: &mut Context, _state: &mut State) -> Value {
    let [message] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    Value::Bulk(message.clone())
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    // PING

    #[test]
    fn no_arg_replies_pong() {
        assert_eq!(
            dispatch(&cmd(&["PING"]), &mut state()),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn echoes_message() {
        assert_eq!(
            dispatch(&cmd(&["PING", "hi"]), &mut state()),
            Value::Bulk(b"hi".to_vec())
        );
    }

    #[test]
    fn too_many_args() {
        assert_eq!(
            dispatch(&cmd(&["PING", "a", "b"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'ping' command".to_string())
        );
    }

    // ECHO

    #[test]
    fn returns_argument() {
        assert_eq!(
            dispatch(&cmd(&["ECHO", "hello"]), &mut state()),
            Value::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn echo_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["ECHO"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'echo' command".to_string())
        );
    }
}
