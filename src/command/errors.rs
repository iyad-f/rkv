// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::resp::Value;

/// Builds the standard reply for a command called with the wrong argument count.
pub(super) fn wrong_args(command: &str) -> Value {
    Value::Error(format!(
        "ERR wrong number of arguments for '{}' command",
        command.to_ascii_lowercase()
    ))
}

/// Builds the standard reply for an unrecognized command.
pub(super) fn unknown_command(name: &[u8], args: &[Vec<u8>]) -> Value {
    let name = String::from_utf8_lossy(name);

    let mut list = String::new();
    for arg in args {
        list.push_str(&format!("'{}' ", String::from_utf8_lossy(arg)));
    }

    Value::Error(format!(
        "ERR unknown command '{name}', with args beginning with: {list}"
    ))
}

/// Builds the standard reply for a value that is not a valid integer.
pub(super) fn not_integer() -> Value {
    Value::Error("ERR value is not an integer or out of range".to_string())
}

/// Builds the standard reply for an integer operation that would overflow.
pub(super) fn overflow() -> Value {
    Value::Error("ERR increment or decrement would overflow".to_string())
}
