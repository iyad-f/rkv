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
