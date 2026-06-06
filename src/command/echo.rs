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

fn echo(args: &[Value], _state: &mut State) -> Value {
    match args {
        [Value::Bulk(message)] => Value::Bulk(message.clone()),
        _ => super::wrong_args("echo"),
    }
}
