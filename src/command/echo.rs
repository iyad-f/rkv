// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command};
use crate::resp::Value;
use crate::state::State;

/// `ECHO message` replies with the message.
pub const COMMAND: Command = Command {
    name: "ECHO",
    arity: Arity::Exact(2),
    handler: echo,
};

fn echo(args: &[Vec<u8>], _state: &mut State) -> Value {
    match args {
        [message] => Value::Bulk(message.clone()),
        _ => super::wrong_args("echo"),
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
