// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::resp::Value;
use crate::state::State;

/// `ECHO message` replies with the message.
pub const COMMAND: Command = Command {
    name: "ECHO",
    arity: Arity::Exact(2),
    write: false,
    handler: echo,
};

fn echo(ctx: &mut Context, _state: &mut State) -> Value {
    match ctx.args {
        [message] => Value::Bulk(message.clone()),
        _ => errors::wrong_args("echo"),
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
    fn returns_argument() {
        assert_eq!(
            dispatch(&cmd(&["ECHO", "hello"]), &mut state()),
            Value::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["ECHO"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'echo' command".to_string())
        );
    }
}
