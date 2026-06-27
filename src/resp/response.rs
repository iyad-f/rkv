// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::CRLF;

/// A command response, encoded on the wire per the RESP2 protocol.
///
/// See the [RESP protocol specification][spec] for the full definitions.
///
/// [spec]: https://redis.io/docs/latest/develop/reference/protocol-spec/
#[derive(Debug, PartialEq)]
pub enum Response {
    /// A simple string.
    Simple(String),

    /// An error.
    Error(String),

    /// A signed 64-bit integer.
    Integer(i64),

    /// A bulk string, binary-safe.
    Bulk(Vec<u8>),

    /// An array of responses.
    Array(Vec<Response>),

    /// A null bulk string.
    NullBulk,

    /// A null array.
    NullArray,
}

impl Response {
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
            Self::NullBulk => {
                out.extend_from_slice(b"$-1\r\n");
                out
            }
            Self::NullArray => {
                out.extend_from_slice(b"*-1\r\n");
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
            Response::Simple("hello".to_string()).encode(),
            b"+hello\r\n".to_vec()
        );
    }

    #[test]
    fn encode_error() {
        assert_eq!(
            Response::Error("ERR".to_string()).encode(),
            b"-ERR\r\n".to_vec()
        );
    }

    #[test]
    fn encode_integer() {
        assert_eq!(Response::Integer(1000).encode(), b":1000\r\n".to_vec());
        assert_eq!(Response::Integer(0).encode(), b":0\r\n".to_vec());
        assert_eq!(Response::Integer(-1000).encode(), b":-1000\r\n".to_vec());
    }

    #[test]
    fn encode_bulk_string() {
        assert_eq!(
            Response::Bulk(b"hello world".to_vec()).encode(),
            b"$11\r\nhello world\r\n".to_vec()
        );
        assert_eq!(
            Response::Bulk(b"".to_vec()).encode(),
            b"$0\r\n\r\n".to_vec()
        );
    }

    #[test]
    fn encode_null_bulk_string() {
        assert_eq!(Response::NullBulk.encode(), b"$-1\r\n".to_vec());
    }

    #[test]
    fn encode_null_array() {
        assert_eq!(Response::NullArray.encode(), b"*-1\r\n".to_vec());
    }

    #[test]
    fn encode_array() {
        assert_eq!(
            Response::Array(vec![
                Response::Bulk(b"hello".to_vec()),
                Response::Bulk(b"world".to_vec())
            ])
            .encode(),
            b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n".to_vec()
        );
        assert_eq!(Response::Array(vec![]).encode(), b"*0\r\n".to_vec());
    }
}
