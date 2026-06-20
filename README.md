<!--
SPDX-FileCopyrightText: 2026 Iyad
SPDX-License-Identifier: Apache-2.0
-->

# rkv

A key-value store written in Rust, wire-compatible with Redis, so it speaks the RESP protocol and works with existing Redis tooling like `redis-cli` and `redis-benchmark`.

## Why?

To learn Rust and internals of redis.

## Goal

I am not really sure how much of redis i want/will be able to replicate, both in terms of backend logic details and what the client sees.

## Running

```sh
cargo run  # listens on 127.0.0.1:6380
```

From another shell:

```sh
redis-cli -p 6380 ping          # PONG
redis-cli -p 6380 set foo bar   # OK
redis-cli -p 6380 get foo       # "bar"
```

### Configuration

Defaults are overridden by an optional config file, then by command-line flags:

```sh
cargo run -- rkv.conf       # a key/value config file
cargo run -- --port 6390    # flags
```

Settings are `bind`, `port`, and `maxclients`, also readable and writable at runtime with `CONFIG GET` and `CONFIG SET`.

Supported commands: `PING`, `ECHO`, `SET`, `GET`, `DEL`, `EXISTS`, `APPEND`, `INCR`, `DECR`, `INCRBY`, `DECRBY`, `EXPIRE`, `TTL`, `PERSIST`, `CONFIG`.

## Development

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable) via `rustup`, with the `rustfmt` and `clippy` components:
  ```sh
  rustup component add rustfmt clippy
  ```
- [pre-commit](https://pre-commit.com/) for the git hooks.

### Setup

Clone the repository:

```sh
git clone https://github.com/iyad-f/rkv.git
cd rkv
```

Install the git hooks:

```sh
pre-commit install
```

### Tests

```sh
cargo test
```

### Commit messages

This project follows [Conventional Commits](https://www.conventionalcommits.org/), enforced by a `commit-msg` hook.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
