// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use super::CRLF;

/// A protocol violation detected while parsing a request.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The multibulk count was not a valid length.
    InvalidMultibulkLength,

    /// A bulk length was not a valid length.
    InvalidBulkLength,

    /// An element did not begin with the bulk-string marker, holding the byte
    /// found in its place.
    ExpectedBulk(u8),

    /// An inline request left a quote unclosed.
    UnbalancedQuotes,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidMultibulkLength => write!(f, "Protocol error: invalid multibulk length"),
            Self::InvalidBulkLength => write!(f, "Protocol error: invalid bulk length"),
            Self::ExpectedBulk(byte) => {
                write!(f, "Protocol error: expected '$', got '{}'", *byte as char)
            }
            Self::UnbalancedQuotes => write!(f, "Protocol error: unbalanced quotes in request"),
        }
    }
}

impl std::error::Error for Error {}

/// A request parsed from a connection buffer.
#[derive(Debug, PartialEq)]
pub enum Request {
    /// A complete command, its argument vector (name followed by arguments) and
    /// the number of bytes it consumed.
    Command { argv: Vec<Vec<u8>>, consumed: usize },

    /// A well-formed empty request (`*0`, a negative count, or a blank inline
    /// line), consuming `consumed` bytes with no command to run.
    Empty { consumed: usize },

    /// Not enough bytes have arrived yet to parse a whole request.
    Incomplete,
}

impl Request {
    /// Parses one request from the front of `input`.
    ///
    /// A request starting with `*` is a RESP multi-bulk array, an array whose
    /// every element is a bulk string. Anything else is an inline command, a
    /// single line of whitespace-separated arguments. In both the first
    /// argument is the command name, the rest are its arguments.
    pub fn parse(input: &[u8]) -> Result<Self, Error> {
        match input.first() {
            Some(b'*') => Self::parse_multibulk(input),
            Some(_) => Self::parse_inline(input),
            None => Ok(Self::Incomplete),
        }
    }

    /// Parses a multi-bulk request, whose first byte is known to be `*`.
    fn parse_multibulk(input: &[u8]) -> Result<Self, Error> {
        let (line, header_len) = match read_line(&input[1..]) {
            Some(parsed) => parsed,
            None => return Ok(Self::Incomplete),
        };
        let count = match parse_len(line) {
            Some(count) => count,
            None => return Err(Error::InvalidMultibulkLength),
        };

        if count > i32::MAX as i64 {
            return Err(Error::InvalidMultibulkLength);
        }
        if count <= 0 {
            return Ok(Self::Empty {
                consumed: 1 + header_len,
            });
        }

        let count = count as usize;
        let mut offset = 1 + header_len;
        let mut argv = Vec::with_capacity(count);

        for _ in 0..count {
            match input.get(offset) {
                Some(b'$') => {}
                Some(&byte) => return Err(Error::ExpectedBulk(byte)),
                None => return Ok(Self::Incomplete),
            }

            let (line, header_len) = match read_line(&input[offset + 1..]) {
                Some(parsed) => parsed,
                None => return Ok(Self::Incomplete),
            };
            let len = match parse_len(line) {
                Some(len) => len,
                None => return Err(Error::InvalidBulkLength),
            };

            if len < 0 {
                return Err(Error::InvalidBulkLength);
            }

            let len = len as usize;
            let start = offset + 1 + header_len;
            let end = start + len;

            // The payload and its trailing CRLF must both be present.
            if input.len() < end + 2 {
                return Ok(Self::Incomplete);
            }

            argv.push(input[start..end].to_vec());
            offset = end + 2;
        }

        Ok(Self::Command {
            argv,
            consumed: offset,
        })
    }

    /// Parses an inline request, a single newline-terminated line of
    /// whitespace-separated, optionally quoted arguments.
    fn parse_inline(input: &[u8]) -> Result<Self, Error> {
        let newline = match input.iter().position(|&byte| byte == b'\n') {
            Some(newline) => newline,
            None => return Ok(Self::Incomplete),
        };
        let consumed = newline + 1;

        // The line excludes the newline and a preceding carriage return.
        let mut end = newline;
        if end > 0 && input[end - 1] == b'\r' {
            end -= 1;
        }

        match split_args(&input[..end]) {
            Some(argv) if argv.is_empty() => Ok(Self::Empty { consumed }),
            Some(argv) => Ok(Self::Command { argv, consumed }),
            None => Err(Error::UnbalancedQuotes),
        }
    }
}

/// Find the next `\r\n` in `input`, returning the bytes before it and the number
/// of bytes consumed (content length + 2 for the `\r\n`), or `None` when no
/// complete line is present yet.
fn read_line(input: &[u8]) -> Option<(&[u8], usize)> {
    input
        .windows(2)
        .position(|w| w == CRLF)
        .map(|pos| (&input[..pos], pos + 2))
}

/// Parses a length line as a signed integer, or `None` if it is not one.
fn parse_len(line: &[u8]) -> Option<i64> {
    std::str::from_utf8(line).ok()?.parse().ok()
}

/// Splits an inline line into arguments, honouring single and double quotes and
/// their escapes, or `None` if a quote is left unbalanced.
fn split_args(line: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut args = Vec::new();
    let mut i = 0;

    loop {
        // Skip any whitespace.
        while i < line.len() && line[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= line.len() {
            return Some(args);
        }

        let mut current = Vec::new();
        let mut in_double = false;
        let mut in_single = false;

        loop {
            if i >= line.len() {
                // A quote still open at the end of the line is unbalanced.
                if in_double || in_single {
                    return None;
                }
                break;
            }

            let byte = line[i];
            if in_double {
                if byte == b'\\' {
                    // Handle hex escaping.
                    if i + 3 < line.len()
                        && line[i + 1] == b'x'
                        && line[i + 2].is_ascii_hexdigit()
                        && line[i + 3].is_ascii_hexdigit()
                    {
                        let hi = (line[i + 2] as char).to_digit(16).unwrap() as u8;
                        let lo = (line[i + 3] as char).to_digit(16).unwrap() as u8;
                        current.push(hi * 16 + lo);
                        i += 3;
                    } else if i + 1 < line.len() {
                        // Handle a named control escape.
                        i += 1;
                        current.push(match line[i] {
                            b'n' => b'\n',
                            b'r' => b'\r',
                            b't' => b'\t',
                            b'b' => 0x08,
                            b'a' => 0x07,
                            other => other,
                        });
                    }
                } else if byte == b'"' {
                    // A closing quote must be followed by whitespace or the end.
                    if i + 1 < line.len() && !line[i + 1].is_ascii_whitespace() {
                        return None;
                    }
                    i += 1;
                    break;
                } else {
                    current.push(byte);
                }
            } else if in_single {
                // A single quote only escapes another single quote.
                if byte == b'\\' && i + 1 < line.len() && line[i + 1] == b'\'' {
                    i += 1;
                    current.push(b'\'');
                } else if byte == b'\'' {
                    if i + 1 < line.len() && !line[i + 1].is_ascii_whitespace() {
                        return None;
                    }
                    i += 1;
                    break;
                } else {
                    current.push(byte);
                }
            } else {
                match byte {
                    b' ' | b'\n' | b'\r' | b'\t' => {
                        i += 1;
                        break;
                    }
                    b'"' => in_double = true,
                    b'\'' => in_single = true,
                    _ => current.push(byte),
                }
            }
            i += 1;
        }

        args.push(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_command() {
        assert_eq!(
            Request::parse(b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n"),
            Ok(Request::Command {
                argv: vec![b"GET".to_vec(), b"foo".to_vec()],
                consumed: 22,
            })
        );
    }

    #[test]
    fn empty_multibulk_is_empty() {
        assert_eq!(
            Request::parse(b"*0\r\n"),
            Ok(Request::Empty { consumed: 4 })
        );
    }

    #[test]
    fn incomplete_count_line() {
        assert_eq!(Request::parse(b"*2"), Ok(Request::Incomplete));
    }

    #[test]
    fn incomplete_missing_elements() {
        assert_eq!(
            Request::parse(b"*2\r\n$3\r\nGET\r\n"),
            Ok(Request::Incomplete)
        );
    }

    #[test]
    fn incomplete_partial_payload() {
        assert_eq!(Request::parse(b"*1\r\n$3\r\nfo"), Ok(Request::Incomplete));
    }

    #[test]
    fn non_bulk_element_is_protocol_error() {
        assert_eq!(
            Request::parse(b"*1\r\n:5\r\n"),
            Err(Error::ExpectedBulk(b':'))
        );
    }

    #[test]
    fn invalid_multibulk_length() {
        assert_eq!(
            Request::parse(b"*x\r\n"),
            Err(Error::InvalidMultibulkLength)
        );
    }

    #[test]
    fn invalid_bulk_length() {
        assert_eq!(
            Request::parse(b"*1\r\n$x\r\n"),
            Err(Error::InvalidBulkLength)
        );
    }

    #[test]
    fn negative_bulk_length_is_invalid() {
        assert_eq!(
            Request::parse(b"*1\r\n$-1\r\n"),
            Err(Error::InvalidBulkLength)
        );
    }

    #[test]
    fn parses_inline_command() {
        assert_eq!(
            Request::parse(b"PING\r\n"),
            Ok(Request::Command {
                argv: vec![b"PING".to_vec()],
                consumed: 6,
            })
        );
    }

    #[test]
    fn parses_inline_without_carriage_return() {
        assert_eq!(
            Request::parse(b"PING\n"),
            Ok(Request::Command {
                argv: vec![b"PING".to_vec()],
                consumed: 5,
            })
        );
    }

    #[test]
    fn parses_inline_with_arguments() {
        assert_eq!(
            Request::parse(b"SET foo bar\r\n"),
            Ok(Request::Command {
                argv: vec![b"SET".to_vec(), b"foo".to_vec(), b"bar".to_vec()],
                consumed: 13,
            })
        );
    }

    #[test]
    fn parses_inline_double_quoted_argument() {
        assert_eq!(
            Request::parse(b"SET foo \"bar baz\"\r\n"),
            Ok(Request::Command {
                argv: vec![b"SET".to_vec(), b"foo".to_vec(), b"bar baz".to_vec()],
                consumed: 19,
            })
        );
    }

    #[test]
    fn parses_inline_single_quoted_argument() {
        assert_eq!(
            Request::parse(b"ECHO 'a b'\n"),
            Ok(Request::Command {
                argv: vec![b"ECHO".to_vec(), b"a b".to_vec()],
                consumed: 11,
            })
        );
    }

    #[test]
    fn parses_inline_escape_in_double_quotes() {
        assert_eq!(
            Request::parse(b"ECHO \"a\\tb\"\n"),
            Ok(Request::Command {
                argv: vec![b"ECHO".to_vec(), b"a\tb".to_vec()],
                consumed: 12,
            })
        );
    }

    #[test]
    fn parses_inline_hex_escape() {
        assert_eq!(
            Request::parse(b"ECHO \"\\x41\"\n"),
            Ok(Request::Command {
                argv: vec![b"ECHO".to_vec(), b"A".to_vec()],
                consumed: 12,
            })
        );
    }

    #[test]
    fn blank_inline_line_is_empty() {
        assert_eq!(Request::parse(b"\r\n"), Ok(Request::Empty { consumed: 2 }));
    }

    #[test]
    fn incomplete_inline_without_newline() {
        assert_eq!(Request::parse(b"PING"), Ok(Request::Incomplete));
    }

    #[test]
    fn unbalanced_quote_is_protocol_error() {
        assert_eq!(Request::parse(b"SET \"foo\n"), Err(Error::UnbalancedQuotes));
    }
}
