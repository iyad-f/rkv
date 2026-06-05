// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use crate::resp::Value;

pub type Store = HashMap<Vec<u8>, Vec<u8>>;

/// Route a parsed request to its command and return the reply.
pub fn dispatch(request: Value, store: &mut Store) -> Value {
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
        b"SET" => set(args, store),
        b"GET" => get(args, store),
        _ => unknown(name, args),
    }
}

fn wrong_args(command: &str) -> Value {
    Value::Error(format!(
        "ERR wrong number of arguments for '{command}' command"
    ))
}

fn ping(args: &[Value]) -> Value {
    match args {
        [] => Value::Simple("PONG".to_string()),
        [Value::Bulk(msg)] => Value::Bulk(msg.clone()),
        _ => wrong_args("ping"),
    }
}

fn echo(args: &[Value]) -> Value {
    match args {
        [Value::Bulk(msg)] => Value::Bulk(msg.clone()),
        _ => wrong_args("echo"),
    }
}

fn set(args: &[Value], store: &mut Store) -> Value {
    match args {
        [Value::Bulk(key), Value::Bulk(value)] => {
            store.insert(key.clone(), value.clone());
            Value::Simple("OK".to_string())
        }
        _ => wrong_args("set"),
    }
}

fn get(args: &[Value], store: &Store) -> Value {
    match args {
        [Value::Bulk(key)] => match store.get(key) {
            Some(value) => Value::Bulk(value.clone()),
            None => Value::Null,
        },
        _ => wrong_args("get"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(parts: &[&str]) -> Value {
        Value::Array(
            parts
                .iter()
                .map(|p| Value::Bulk(p.as_bytes().to_vec()))
                .collect(),
        )
    }

    #[test]
    fn ping_no_arg() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["PING"]), &mut store),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn ping_with_message() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["PING", "hi"]), &mut store),
            Value::Bulk(b"hi".to_vec())
        );
    }

    #[test]
    fn ping_too_many_args() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["PING", "a", "b"]), &mut store),
            Value::Error("ERR wrong number of arguments for 'ping' command".to_string())
        );
    }

    #[test]
    fn echo_returns_argument() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["ECHO", "hello"]), &mut store),
            Value::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn echo_wrong_args() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["ECHO"]), &mut store),
            Value::Error("ERR wrong number of arguments for 'echo' command".to_string())
        );
    }

    #[test]
    fn set_then_get() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["SET", "foo", "bar"]), &mut store),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(cmd(&["GET", "foo"]), &mut store),
            Value::Bulk(b"bar".to_vec())
        );
    }

    #[test]
    fn set_overwrites_existing() {
        let mut store = Store::new();
        dispatch(cmd(&["SET", "k", "v1"]), &mut store);
        dispatch(cmd(&["SET", "k", "v2"]), &mut store);
        assert_eq!(
            dispatch(cmd(&["GET", "k"]), &mut store),
            Value::Bulk(b"v2".to_vec())
        );
    }

    #[test]
    fn set_wrong_args() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["SET", "k"]), &mut store),
            Value::Error("ERR wrong number of arguments for 'set' command".to_string())
        );
    }

    #[test]
    fn get_missing_is_null() {
        let mut store = Store::new();
        assert_eq!(dispatch(cmd(&["GET", "nope"]), &mut store), Value::Null);
    }

    #[test]
    fn get_wrong_args() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["GET"]), &mut store),
            Value::Error("ERR wrong number of arguments for 'get' command".to_string())
        );
    }

    #[test]
    fn command_name_is_case_insensitive() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["ping"]), &mut store),
            Value::Simple("PONG".to_string())
        );
    }

    #[test]
    fn unknown_command() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(cmd(&["FOOBAR", "x"]), &mut store),
            Value::Error(
                "ERR unknown command 'FOOBAR', with args beginning with: 'x' ".to_string()
            )
        );
    }

    #[test]
    fn non_array_request_is_protocol_error() {
        let mut store = Store::new();
        assert_eq!(
            dispatch(Value::Integer(1), &mut store),
            Value::Error("ERR Protocol error".to_string())
        );
    }
}
