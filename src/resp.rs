// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Types and parsing for the RESP2 wire protocol.

/// The RESP line terminator.
const CRLF: &[u8] = b"\r\n";

/// A single value in the RESP2 protocol.
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

#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// Not enough data is available to parse a message.
    Incomplete,

    /// Invalid data inside a message.
    Invalid,
}

impl Value {
    pub fn parse(input: &[u8]) -> Result<(Value, usize), ParseError> {
        match input.first() {
            None => Err(ParseError::Incomplete),
            Some(b'+') => {
                let (s, consumed) = read_string(&input[1..])?;
                Ok((Value::Simple(s), 1 + consumed))
            }
            Some(b'-') => {
                let (s, consumed) = read_string(&input[1..])?;
                Ok((Value::Error(s), 1 + consumed))
            }
            Some(b':') => {
                let (n, consumed) = read_int(&input[1..])?;
                Ok((Value::Integer(n), 1 + consumed))
            }
            Some(b'$') => {
                let (payload_len, header_len) = read_int(&input[1..])?;
                if payload_len == -1 {
                    return Ok((Value::Null, 1 + header_len));
                }
                if payload_len < 0 {
                    return Err(ParseError::Invalid);
                }

                let payload_len = payload_len as usize;
                let start = 1 + header_len;
                let end = start + payload_len;

                // The payload and its trailing CRLF must both be present before
                // we can report them consumed.
                if input.len() < end + 2 {
                    return Err(ParseError::Incomplete);
                }
                Ok((Value::Bulk(input[start..end].to_vec()), end + 2))
            }
            Some(b'*') => {
                let (element_count, header_len) = read_int(&input[1..])?;
                if element_count == -1 {
                    return Ok((Value::Null, 1 + header_len));
                }
                if element_count < 0 {
                    return Err(ParseError::Invalid);
                }

                let mut offset = 1 + header_len;
                let mut items = Vec::with_capacity(element_count as usize);
                for _ in 0..element_count {
                    let (value, n) = Value::parse(&input[offset..])?;
                    items.push(value);
                    offset += n;
                }
                Ok((Value::Array(items), offset))
            }
            _ => Err(ParseError::Invalid),
        }
    }

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

/// Find the next `\r\n` in `input`, returning the bytes before it and the
/// number of bytes consumed (content length + 2 for the `\r\n`).
fn read_line(input: &[u8]) -> Result<(&[u8], usize), ParseError> {
    match input.windows(2).position(|w| w == CRLF) {
        Some(pos) => Ok((&input[..pos], pos + 2)),
        None => Err(ParseError::Incomplete),
    }
}

/// Read a `\r\n`-terminated line and decode it as a UTF-8 string.
/// Returns the string and the bytes consumed (including the `\r\n`).
fn read_string(input: &[u8]) -> Result<(String, usize), ParseError> {
    let (line, consumed) = read_line(input)?;
    let s = std::str::from_utf8(line)
        .map_err(|_| ParseError::Invalid)?
        .to_string();
    Ok((s, consumed))
}

/// Read a `\r\n`-terminated line and parse it as a signed integer.
/// Returns the integer and the bytes consumed (including the `\r\n`).
fn read_int(input: &[u8]) -> Result<(i64, usize), ParseError> {
    let (line, consumed) = read_line(input)?;
    let n = std::str::from_utf8(line)
        .map_err(|_| ParseError::Invalid)?
        .parse::<i64>()
        .map_err(|_| ParseError::Invalid)?;
    Ok((n, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_string() {
        assert_eq!(
            Value::parse(b"+OK\r\n").unwrap(),
            (Value::Simple("OK".to_string()), 5)
        );
    }

    #[test]
    fn parse_error() {
        assert_eq!(
            Value::parse(b"-ERR bad\r\n").unwrap(),
            (Value::Error("ERR bad".to_string()), 10)
        );
    }

    #[test]
    fn parse_integer() {
        assert_eq!(
            Value::parse(b":1000\r\n").unwrap(),
            (Value::Integer(1000), 7)
        );
        assert_eq!(Value::parse(b":0\r\n").unwrap(), (Value::Integer(0), 4));
        assert_eq!(
            Value::parse(b":-1000\r\n").unwrap(),
            (Value::Integer(-1000), 8)
        );
    }

    #[test]
    fn parse_bulk_string() {
        assert_eq!(
            Value::parse(b"$5\r\nhello\r\n").unwrap(),
            (Value::Bulk(b"hello".to_vec()), 11)
        );
        assert_eq!(
            Value::parse(b"$0\r\n\r\n").unwrap(),
            (Value::Bulk(b"".to_vec()), 6)
        );
        // Payload present but the trailing CRLF has not fully arrived.
        assert_eq!(Value::parse(b"$5\r\nhello"), Err(ParseError::Incomplete));
        assert_eq!(Value::parse(b"$5\r\nhello\r"), Err(ParseError::Incomplete));
    }

    #[test]
    fn parse_null_bulk_string() {
        assert_eq!(Value::parse(b"$-1\r\n").unwrap(), (Value::Null, 5));
    }

    #[test]
    fn parse_invalid_length_bulk_string() {
        assert_eq!(Value::parse(b"$-2\r\n"), Err(ParseError::Invalid));
    }

    #[test]
    fn parse_array() {
        assert_eq!(
            Value::parse(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n").unwrap(),
            (
                Value::Array(vec![
                    Value::Bulk(b"hello".to_vec()),
                    Value::Bulk(b"world".to_vec())
                ]),
                26
            )
        );
        assert_eq!(Value::parse(b"*0\r\n").unwrap(), (Value::Array(vec![]), 4));
    }

    #[test]
    fn parse_null_array() {
        assert_eq!(Value::parse(b"*-1\r\n").unwrap(), (Value::Null, 5));
    }

    #[test]
    fn parse_invalid_length_array() {
        assert_eq!(Value::parse(b"*-2\r\n"), Err(ParseError::Invalid));
    }

    #[test]
    fn encode_simple_string() {
        assert_eq!(
            Value::Simple("hello".to_string()).encode(),
            b"+hello\r\n".to_vec()
        );
    }

    #[test]
    fn round_trip_simple_string() {
        let value = Value::Simple("hello".to_string());
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
    }

    #[test]
    fn encode_error() {
        assert_eq!(
            Value::Error("ERR".to_string()).encode(),
            b"-ERR\r\n".to_vec()
        );
    }

    #[test]
    fn round_trip_error() {
        let value = Value::Error("ERR".to_string());
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
    }

    #[test]
    fn encode_integer() {
        assert_eq!(Value::Integer(1000).encode(), b":1000\r\n".to_vec());
        assert_eq!(Value::Integer(0).encode(), b":0\r\n".to_vec());
        assert_eq!(Value::Integer(-1000).encode(), b":-1000\r\n".to_vec());
    }

    #[test]
    fn round_trip_integer() {
        let value = Value::Integer(1000);
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
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
    fn round_trip_bulk_string() {
        let value = Value::Bulk(b"hello".to_vec());
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
    }

    #[test]
    fn encode_null_bulk_string() {
        assert_eq!(Value::Null.encode(), b"$-1\r\n".to_vec());
    }

    #[test]
    fn round_trip_null() {
        let value = Value::Null;
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
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

    #[test]
    fn round_trip_array() {
        let value = Value::Array(vec![Value::Bulk(b"hello".to_vec()), Value::Integer(42)]);
        assert_eq!(Value::parse(&value.encode()).unwrap().0, value);
    }
}
