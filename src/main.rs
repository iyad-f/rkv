// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{Read, Write},
    net::TcpListener,
};

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
            stream.write_all(&buf[..n])?;
        }
    }
}
