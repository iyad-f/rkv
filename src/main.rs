// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{Read, Write},
    net::TcpListener,
};
mod resp;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6380")?;
    loop {
        let (mut stream, _) = listener.accept()?;
        let mut buf: [u8; 512] = [0; 512];

        loop {
            let n = stream.read(&mut buf)?;
            if n == 0 {
                break;
            }

            let (value, _consumed) = match resp::Value::parse(&buf[..n]) {
                Ok(parsed) => parsed,
                Err(_) => continue, // TODO: handle incomplete/invalid.
            };

            if let resp::Value::Array(items) = value
                && let Some(resp::Value::Bulk(name)) = items.first()
            {
                match name.to_ascii_uppercase().as_slice() {
                    b"PING" => stream.write_all(b"+PONG\r\n")?,
                    _ => stream.write_all(b"-ERR unkown command\r\n")?,
                }
            }
        }
    }
}
