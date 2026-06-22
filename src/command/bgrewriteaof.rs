// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context};
use crate::resp::Value;
use crate::server::State;

pub const COMMAND: Command = Command {
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

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["BGREWRITEAOF", "x"]), &mut state()),
            Value::Error("ERR wrong number of arguments for 'bgrewriteaof' command".to_string())
        );
    }
}
