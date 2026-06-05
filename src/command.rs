// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use crate::resp::Value;

/// Route a parsed request to its command and return the reply.
pub fn dispatch(request: Value) -> Value {
    let items = match request {
        Value::Array(items) => items,
        _ => return Value::Error("ERR Protocol error".to_string()),
    };

    let (name, args) = match items.split_first() {
        Some((Value::Bulk(name), args)) => (name, args),
        _ => return Value::Error("ERR Protocol error".to_string()),
    };

    match name.to_ascii_uppercase().as_slice() {
        b"PING" => ping(args),
        b"ECHO" => echo(args),
        _ => unknown(name, args),
    }
}

fn ping(args: &[Value]) -> Value {
    match args {
        [] => Value::Simple("PONG".to_string()),
        [Value::Bulk(msg)] => Value::Bulk(msg.clone()),
        _ => Value::Error("ERR wrong number of arguments for 'ping' command".to_string()),
    }
}

fn echo(args: &[Value]) -> Value {
    match args {
        [Value::Bulk(msg)] => Value::Bulk(msg.clone()),
        _ => Value::Error("ERR wrong number of arguments for 'echo' command".to_string()),
    }
}

fn unknown(name: &[u8], args: &[Value]) -> Value {
    let cmd = String::from_utf8_lossy(name);

    let mut list = String::new();
    for arg in args {
        if let Value::Bulk(arg) = arg {
            list.push_str(&format!("'{}' ", String::from_utf8_lossy(arg)));
        }
    }

    Value::Error(format!(
        "ERR unknown command '{cmd}', with args beginning with: {list}"
    ))
}
