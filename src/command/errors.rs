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

/// Builds the reply for a `DECRBY` decrement that cannot be negated (`i64::MIN`).
pub(super) fn decrement_overflow() -> Value {
    Value::Error("ERR decrement would overflow".to_string())
}

/// Builds the reply for an expire command given a time that overflows.
pub(super) fn invalid_expire_time(command: &str) -> Value {
    Value::Error(format!(
        "ERR invalid expire time in '{}' command",
        command.to_ascii_lowercase()
    ))
}

/// Builds the reply for a write refused because the append-only file is in a
/// failed-write state.
pub(super) fn misconf() -> Value {
    Value::Error("MISCONF Errors writing to the append-only file".to_string())
}

/// Builds the reply for a command run against a key holding the wrong type.
pub(super) fn wrong_type() -> Value {
    Value::Error("WRONGTYPE Operation against a key holding the wrong kind of value".to_string())
}

/// Builds the reply for a count argument that must be non-negative but is not.
pub(super) fn out_of_range_positive() -> Value {
    Value::Error("ERR value is out of range, must be positive".to_string())
}

/// Builds the reply for an operation on a key that does not exist.
pub(super) fn no_such_key() -> Value {
    Value::Error("ERR no such key".to_string())
}

/// Builds the reply for an index argument that falls outside the value.
pub(super) fn index_out_of_range() -> Value {
    Value::Error("ERR index out of range".to_string())
}

/// Builds the reply for a command given malformed or unrecognized arguments.
pub(super) fn syntax_error() -> Value {
    Value::Error("ERR syntax error".to_string())
}
