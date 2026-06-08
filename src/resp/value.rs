// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::CRLF;

/// A single value in the RESP2 protocol, used to build replies.
///
/// See the [RESP protocol specification][spec] for the full definitions.
///
/// [spec]: https://redis.io/docs/latest/develop/reference/protocol-spec/
#[derive(Debug, PartialEq)]
pub enum Value {
    /// A simple string.
    Simple(String),

    /// An error.
    Error(String),

    /// A signed 64-bit integer.
    Integer(i64),

    /// A bulk string, binary-safe.
    Bulk(Vec<u8>),

    /// An array of values.
    Array(Vec<Value>),

    /// The null value.
    Null,
}

impl Value {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        match self {
            Self::Simple(s) => {
                out.push(b'+');
                out.extend_from_slice(s.as_bytes());
                out.extend_from_slice(CRLF);
                out
            }
            Self::Error(s) => {
                out.push(b'-');
                out.extend_from_slice(s.as_bytes());
                out.extend_from_slice(CRLF);
                out
            }
            Self::Integer(n) => {
                out.push(b':');
                out.extend_from_slice(n.to_string().as_bytes());
                out.extend_from_slice(CRLF);
                out
            }
            Self::Bulk(bytes) => {
                out.push(b'$');
                out.extend_from_slice(bytes.len().to_string().as_bytes());
                out.extend_from_slice(CRLF);
                out.extend_from_slice(bytes);
                out.extend_from_slice(CRLF);
                out
            }
            Self::Array(items) => {
                out.push(b'*');
                out.extend_from_slice(items.len().to_string().as_bytes());
                out.extend_from_slice(CRLF);
                for item in items {
                    out.extend_from_slice(&item.encode());
                }
                out
            }
            Self::Null => {
                out.extend_from_slice(b"$-1\r\n");
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_simple_string() {
        assert_eq!(
            Value::Simple("hello".to_string()).encode(),
            b"+hello\r\n".to_vec()
        );
    }

    #[test]
    fn encode_error() {
        assert_eq!(
            Value::Error("ERR".to_string()).encode(),
            b"-ERR\r\n".to_vec()
        );
    }

    #[test]
    fn encode_integer() {
        assert_eq!(Value::Integer(1000).encode(), b":1000\r\n".to_vec());
        assert_eq!(Value::Integer(0).encode(), b":0\r\n".to_vec());
        assert_eq!(Value::Integer(-1000).encode(), b":-1000\r\n".to_vec());
    }

    #[test]
    fn encode_bulk_string() {
        assert_eq!(
            Value::Bulk(b"hello world".to_vec()).encode(),
            b"$11\r\nhello world\r\n".to_vec()
        );
        assert_eq!(Value::Bulk(b"".to_vec()).encode(), b"$0\r\n\r\n".to_vec());
    }

    #[test]
    fn encode_null_bulk_string() {
        assert_eq!(Value::Null.encode(), b"$-1\r\n".to_vec());
    }

    #[test]
    fn encode_array() {
        assert_eq!(
            Value::Array(vec![
                Value::Bulk(b"hello".to_vec()),
                Value::Bulk(b"world".to_vec())
            ])
            .encode(),
            b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n".to_vec()
        );
        assert_eq!(Value::Array(vec![]).encode(), b"*0\r\n".to_vec());
    }
}
