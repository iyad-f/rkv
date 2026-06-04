# rkv

A key-value store written in Rust, wire-compatible with Redis, so it speaks the RESP protocol and works with existing Redis tooling like `redis-cli` and `redis-benchmark`.

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

### Commit messages

This project follows [Conventional Commits](https://www.conventionalcommits.org/), enforced by a `commit-msg` hook.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
