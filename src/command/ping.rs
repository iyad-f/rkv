// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command};
use crate::resp::Value;
use crate::state::State;

/// `PING [message]` replies with PONG, or echoes the optional message.
pub const COMMAND: Command = Command {
    name: "PING",
    arity: Arity::Min(1),
    handler: ping,
};

fn ping(args: &[Value], _state: &mut State) -> Value {
    match args {
        [] => Value::Simple("PONG".to_string()),
        [Value::Bulk(message)] => Value::Bulk(message.clone()),
        _ => super::wrong_args("ping"),
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
    fn no_arg_replies_pong() {
        assert_eq!(
            dispatch(cmd(&["PING"]), &mut state()),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn echoes_message() {
        assert_eq!(
            dispatch(cmd(&["PING", "hi"]), &mut state()),
            Value::Bulk(b"hi".to_vec())
        );
    }

    #[test]
    fn too_many_args() {
        assert_eq!(
            dispatch(cmd(&["PING", "a", "b"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'ping' command".to_string())
        );
    }
}
