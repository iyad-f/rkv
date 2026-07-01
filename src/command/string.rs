// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use super::{Arity, Command, Context, errors};
use crate::object::Object;
use crate::resp::Response;
use crate::server::State;
use crate::store::Store;

/// The largest value a string command may produce, matching the default
/// `proto-max-bulk-len` of 512 MB.
const PROTO_MAX_BULK_LEN: u64 = 512 * 1024 * 1024;

/// `APPEND key value` appends to the value at `key`, replying with its new length.
pub const APPEND: Command = Command {
    name: "APPEND",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: append,
};

fn append(ctx: &mut Context, state: &mut State) -> Response {
    let [key, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let mut stored = match state.store.get(key) {
        Some(Object::String(bytes)) => bytes.clone(),
        Some(_) => return errors::wrong_type(),
        None => Vec::new(),
    };
    stored.extend_from_slice(value);
    let len = stored.len() as i64;
    state.store.update(key.clone(), Object::String(stored));
    Response::Integer(len)
}

/// `DECR key` decrements the integer at `key` by one, replying with the new value.
pub const DECR: Command = Command {
    name: "DECR",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: decr,
};

fn decr(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    apply_delta(state, key, -1)
}

/// `DECRBY key decrement` subtracts `decrement` from the integer at `key`,
/// replying with the new value.
pub const DECRBY: Command = Command {
    name: "DECRBY",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: decrby,
};

fn decrby(ctx: &mut Context, state: &mut State) -> Response {
    let [key, decrement] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(decrement) = super::parse_i64(decrement) else {
        return errors::not_integer();
    };

    let Some(delta) = decrement.checked_neg() else {
        return Response::Error("ERR decrement would overflow".to_string());
    };

    apply_delta(state, key, delta)
}

/// `GET key` returns the value stored at `key`, or nil if it is missing.
pub const GET: Command = Command {
    name: "GET",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: get,
};

fn get(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get(key) {
        Some(Object::String(bytes)) => Response::Bulk(bytes.clone()),
        Some(_) => errors::wrong_type(),
        None => Response::NullBulk,
    }
}

/// `GETDEL key` returns the value at `key` and deletes it, or nil if it is missing.
pub const GETDEL: Command = Command {
    name: "GETDEL",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: getdel,
};

fn getdel(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get(key) {
        Some(Object::String(bytes)) => {
            let value = bytes.clone();
            state.store.remove(key, false);
            Response::Bulk(value)
        }
        Some(_) => errors::wrong_type(),
        None => Response::NullBulk,
    }
}

/// `GETEX key [EX s | PX ms | EXAT ts | PXAT ts | PERSIST]` returns the value at
/// `key` and optionally changes its expiry. With no option the expiry is left
/// unchanged.
pub const GETEX: Command = Command {
    name: "GETEX",
    arity: Arity::Min(2),
    write: true,
    auth_required: true,
    handler: getex,
};

fn getex(ctx: &mut Context, state: &mut State) -> Response {
    let [key, options @ ..] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let expiry = match options {
        [] => Expiry::Keep,
        [option] => match option.to_ascii_uppercase().as_slice() {
            b"PERSIST" => Expiry::Clear,
            _ => return errors::syntax_error(),
        },
        [option, raw] => {
            let (seconds, absolute) = match option.to_ascii_uppercase().as_slice() {
                b"EX" => (true, false),
                b"PX" => (false, false),
                b"EXAT" => (true, true),
                b"PXAT" => (false, true),
                _ => return errors::syntax_error(),
            };
            match resolve_deadline(raw, seconds, absolute, ctx.command.name) {
                Ok(deadline) => Expiry::At(deadline),
                Err(reply) => return reply,
            }
        }
        _ => return errors::syntax_error(),
    };

    let value = match state.store.get(key) {
        Some(Object::String(bytes)) => bytes.clone(),
        Some(_) => return errors::wrong_type(),
        None => return Response::NullBulk,
    };

    match expiry {
        Expiry::Keep => {}
        Expiry::Clear => {
            if state.store.persist(key) {
                ctx.rewrite = Some(vec![b"PERSIST".to_vec(), key.clone()]);
            }
        }
        Expiry::At(deadline) => {
            if Store::is_expired(deadline) {
                state.store.remove_expired(key);
            } else {
                state.store.set_expiry(key, deadline);
            }
            ctx.rewrite = Some(vec![
                b"PEXPIREAT".to_vec(),
                key.clone(),
                deadline.to_string().into_bytes(),
            ]);
        }
    }

    Response::Bulk(value)
}

/// `GETRANGE key start end` returns the substring of the value at `key` between
/// the inclusive byte offsets `start` and `end`. Negative offsets count back from
/// the end.
pub const GETRANGE: Command = Command {
    name: "GETRANGE",
    arity: Arity::Exact(4),
    write: false,
    auth_required: true,
    handler: getrange,
};

fn getrange(ctx: &mut Context, state: &mut State) -> Response {
    let [key, start, end] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let (Some(start), Some(end)) = (super::parse_i64(start), super::parse_i64(end)) else {
        return errors::not_integer();
    };

    let bytes = match state.store.get(key) {
        Some(Object::String(bytes)) => bytes,
        Some(_) => return errors::wrong_type(),
        None => return Response::Bulk(Vec::new()),
    };

    let len = bytes.len() as i64;

    // Resolve negative indices against the length, then clamp both ends to the
    // valid range. With `end` capped at `len - 1`, `start > end` alone covers
    // both an inverted range and a `start` past the end.
    let start = (if start < 0 { start + len } else { start }).max(0);
    let end = (if end < 0 { end + len } else { end }).min(len - 1);

    if start > end {
        return Response::Bulk(Vec::new());
    }

    Response::Bulk(bytes[start as usize..=end as usize].to_vec())
}

/// `GETSET key value` sets `key` to `value` and returns its old value, or nil if
/// it had none. Any existing expiry is discarded.
pub const GETSET: Command = Command {
    name: "GETSET",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: getset,
};

fn getset(ctx: &mut Context, state: &mut State) -> Response {
    let [key, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let old = match state.store.get(key) {
        Some(Object::String(bytes)) => Response::Bulk(bytes.clone()),
        Some(_) => return errors::wrong_type(),
        None => Response::NullBulk,
    };

    state.store.set(key.clone(), Object::String(value.clone()));
    old
}

/// `INCR key` increments the integer at `key` by one, replying with the new value.
pub const INCR: Command = Command {
    name: "INCR",
    arity: Arity::Exact(2),
    write: true,
    auth_required: true,
    handler: incr,
};

fn incr(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    apply_delta(state, key, 1)
}

/// `INCRBY key increment` adds `increment` to the integer at `key`, replying with
/// the new value.
pub const INCRBY: Command = Command {
    name: "INCRBY",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: incrby,
};

fn incrby(ctx: &mut Context, state: &mut State) -> Response {
    let [key, increment] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(increment) = super::parse_i64(increment) else {
        return errors::not_integer();
    };

    apply_delta(state, key, increment)
}

/// `INCRBYFLOAT key increment` adds `increment` to the float at `key`, replying
/// with the new value. Treats a missing key as `0`.
pub const INCRBYFLOAT: Command = Command {
    name: "INCRBYFLOAT",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: incrbyfloat,
};

fn incrbyfloat(ctx: &mut Context, state: &mut State) -> Response {
    let [key, increment] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let current = match state.store.get(key) {
        Some(Object::String(bytes)) => match parse_float(bytes) {
            Some(current) => current,
            None => return errors::not_float(),
        },
        Some(_) => return errors::wrong_type(),
        None => 0.0,
    };

    let Some(increment) = parse_float(increment) else {
        return errors::not_float();
    };

    let next = current + increment;
    if !next.is_finite() {
        return errors::nan_or_infinity();
    }

    let formatted = format_float(next).into_bytes();
    state
        .store
        .update(key.clone(), Object::String(formatted.clone()));
    Response::Bulk(formatted)
}

/// `LCS key1 key2 [LEN] [IDX] [MINMATCHLEN min] [WITHMATCHLEN]` returns the longest
/// common subsequence of the two string values. `LEN` returns its length instead,
/// and `IDX` returns the ranges that make up the match in each string.
pub const LCS: Command = Command {
    name: "LCS",
    arity: Arity::Min(3),
    write: false,
    auth_required: true,
    handler: lcs,
};

fn lcs(ctx: &mut Context, state: &mut State) -> Response {
    let [first_key, second_key, args @ ..] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let first_value = match state.store.get(first_key) {
        Some(Object::String(value)) => value.clone(),
        Some(_) => {
            return Response::Error(
                "ERR The specified keys must contain string values".to_string(),
            );
        }
        None => Vec::new(),
    };
    let second_value = match state.store.get(second_key) {
        Some(Object::String(value)) => value.clone(),
        Some(_) => {
            return Response::Error(
                "ERR The specified keys must contain string values".to_string(),
            );
        }
        None => Vec::new(),
    };

    let (mut get_idx, mut get_len, mut with_match_len, mut min_match_len) =
        (false, false, false, 0);

    let mut i = 0;
    while i < args.len() {
        match args[i].to_ascii_uppercase().as_slice() {
            b"IDX" => get_idx = true,
            b"LEN" => get_len = true,
            b"WITHMATCHLEN" => with_match_len = true,
            b"MINMATCHLEN" => {
                let Some(next_arg) = args.get(i + 1) else {
                    return errors::syntax_error();
                };

                min_match_len = match super::parse_i64(next_arg) {
                    Some(val) => val.max(0),
                    None => return errors::not_integer(),
                };

                i += 1;
            }
            _ => return errors::syntax_error(),
        }

        i += 1;
    }

    if get_len && get_idx {
        return Response::Error(
            "ERR If you want both the length and indexes, please just use IDX.".to_string(),
        );
    }

    let first_len = first_value.len();
    let second_len = second_value.len();

    if (first_len + 1)
        .checked_mul(second_len + 1)
        .and_then(|cells| cells.checked_mul(4)) // 4 bytes per u32 cell
        .is_none_or(|bytes| bytes as u64 > PROTO_MAX_BULK_LEN)
    {
        return Response::Error(
            "ERR Insufficient memory, transient memory for LCS exceeds proto-max-bulk-len"
                .to_string(),
        );
    }

    let mut dp = vec![0u32; (first_len + 1) * (second_len + 1)];
    let at = |i: usize, j: usize| i * (second_len + 1) + j;
    for i in (0..first_len).rev() {
        for j in (0..second_len).rev() {
            dp[at(i, j)] = if first_value[i] == second_value[j] {
                dp[at(i + 1, j + 1)] + 1
            } else {
                dp[at(i + 1, j)].max(dp[at(i, j + 1)])
            };
        }
    }

    if get_len {
        return Response::Integer(dp[at(0, 0)] as i64);
    }

    let mut matching_pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < first_len && j < second_len {
        if first_value[i] == second_value[j] {
            matching_pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[at(i + 1, j)] >= dp[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }

    if !get_idx {
        let lcs = matching_pairs
            .iter()
            .map(|&(i, _)| first_value[i])
            .collect();
        return Response::Bulk(lcs);
    }

    let mut ranges = Vec::new();
    for &(first, second) in matching_pairs.iter().rev() {
        if let Some((first_start, _, second_start, _)) = ranges.last_mut()
            && first + 1 == *first_start
            && second + 1 == *second_start
        {
            *first_start = first;
            *second_start = second;
        } else {
            ranges.push((first, first, second, second));
        }
    }

    let mut entries = Vec::new();
    for (first_start, first_end, second_start, second_end) in ranges {
        let match_len = (first_end - first_start + 1) as i64;
        if min_match_len != 0 && match_len < min_match_len {
            continue;
        }

        let mut entry = vec![
            Response::Array(vec![
                Response::Integer(first_start as i64),
                Response::Integer(first_end as i64),
            ]),
            Response::Array(vec![
                Response::Integer(second_start as i64),
                Response::Integer(second_end as i64),
            ]),
        ];
        if with_match_len {
            entry.push(Response::Integer(match_len));
        }

        entries.push(Response::Array(entry));
    }

    Response::Array(vec![
        Response::Bulk(b"matches".to_vec()),
        Response::Array(entries),
        Response::Bulk(b"len".to_vec()),
        Response::Integer(dp[at(0, 0)] as i64),
    ])
}

/// `MGET key [key ...]` returns the values at the given keys, with nil for each
/// key that is missing or does not hold a string.
pub const MGET: Command = Command {
    name: "MGET",
    arity: Arity::Min(2),
    write: false,
    auth_required: true,
    handler: mget,
};

fn mget(ctx: &mut Context, state: &mut State) -> Response {
    let values = ctx
        .args
        .iter()
        .map(|key| match state.store.get(key) {
            Some(Object::String(bytes)) => Response::Bulk(bytes.clone()),
            _ => Response::NullBulk,
        })
        .collect();

    Response::Array(values)
}

/// `MSET key value [key value ...]` sets each key to its value, discarding any
/// existing expiry.
pub const MSET: Command = Command {
    name: "MSET",
    arity: Arity::Min(3),
    write: true,
    auth_required: true,
    handler: mset,
};

fn mset(ctx: &mut Context, state: &mut State) -> Response {
    if !ctx.args.len().is_multiple_of(2) {
        return errors::wrong_args(ctx.command.name);
    }

    for pair in ctx.args.chunks_exact(2) {
        state
            .store
            .set(pair[0].clone(), Object::String(pair[1].clone()));
    }

    Response::Simple("OK".to_string())
}

/// `MSETNX key value [key value ...]` sets each key to its value only if none of
/// the keys exist, replying with `1` if all were set and `0` if none were.
pub const MSETNX: Command = Command {
    name: "MSETNX",
    arity: Arity::Min(3),
    write: true,
    auth_required: true,
    handler: msetnx,
};

fn msetnx(ctx: &mut Context, state: &mut State) -> Response {
    if !ctx.args.len().is_multiple_of(2) {
        return errors::wrong_args(ctx.command.name);
    }

    if ctx
        .args
        .chunks_exact(2)
        .any(|pair| state.store.contains_key(&pair[0]))
    {
        return Response::Integer(0);
    }

    for pair in ctx.args.chunks_exact(2) {
        state
            .store
            .set(pair[0].clone(), Object::String(pair[1].clone()));
    }

    Response::Integer(1)
}

/// `PSETEX key milliseconds value` sets `key` to `value` with a TTL of
/// `milliseconds`.
pub const PSETEX: Command = Command {
    name: "PSETEX",
    arity: Arity::Exact(4),
    write: true,
    auth_required: true,
    handler: psetex,
};

fn psetex(ctx: &mut Context, state: &mut State) -> Response {
    set_with_ttl(ctx, state, false)
}

/// `SET key value` stores `value` at `key`.
pub const SET: Command = Command {
    name: "SET",
    arity: Arity::Min(3),
    write: true,
    auth_required: true,
    handler: set,
};

/// The presence condition from `NX` / `XX`.
#[derive(PartialEq)]
enum Condition {
    Exists,    // XX
    NotExists, // NX
}

/// How a write sets the key's expiry.
#[derive(Default, Clone, Copy)]
enum Expiry {
    #[default]
    Clear, // drop any TTL
    Keep,    // KEEPTTL
    At(i64), // EX/PX/EXAT/PXAT, resolved to an absolute ms deadline
}

/// Which option set the expiry, tracked only while parsing so the same option
/// may repeat while a different one conflicts.
#[derive(PartialEq)]
enum ExpiryKind {
    Ex,
    Px,
    Exat,
    Pxat,
    KeepTtl,
}

/// The parsed options of a `SET` command.
#[derive(Default)]
struct SetOptions {
    /// The presence condition the write is gated on, if any.
    condition: Option<Condition>,

    /// Whether to reply with the key's old value rather than `OK`.
    get: bool,

    /// How the write affects the key's expiry.
    expiry: Expiry,
}

impl SetOptions {
    fn parse(options: &[Vec<u8>], command: &str) -> Result<Self, Response> {
        let mut opts = SetOptions::default();
        let mut expiry_kind = None;
        let mut i = 0;

        while i < options.len() {
            let option = options[i].to_ascii_uppercase();
            match option.as_slice() {
                b"NX" => set_once(&mut opts.condition, Condition::NotExists)?,
                b"XX" => set_once(&mut opts.condition, Condition::Exists)?,
                b"GET" => opts.get = true,
                b"KEEPTTL" => {
                    set_once(&mut expiry_kind, ExpiryKind::KeepTtl)?;
                    opts.expiry = Expiry::Keep;
                }
                b"EX" | b"PX" | b"EXAT" | b"PXAT" => {
                    let (kind, seconds, absolute) = match option.as_slice() {
                        b"EX" => (ExpiryKind::Ex, true, false),
                        b"PX" => (ExpiryKind::Px, false, false),
                        b"EXAT" => (ExpiryKind::Exat, true, true),
                        _ => (ExpiryKind::Pxat, false, true),
                    };
                    set_once(&mut expiry_kind, kind)?;

                    let Some(raw) = options.get(i + 1) else {
                        return Err(errors::syntax_error());
                    };
                    opts.expiry = Expiry::At(resolve_deadline(raw, seconds, absolute, command)?);
                    i += 1;
                }
                _ => return Err(errors::syntax_error()),
            }
            i += 1;
        }

        Ok(opts)
    }
}

/// Sets `slot` to `value`, erroring when a different value was already set.
/// Repeating the same value is allowed.
fn set_once<T: PartialEq>(slot: &mut Option<T>, value: T) -> Result<(), Response> {
    if slot.as_ref().is_some_and(|existing| existing != &value) {
        return Err(errors::syntax_error());
    }
    *slot = Some(value);
    Ok(())
}

fn set(ctx: &mut Context, state: &mut State) -> Response {
    let [key, value, options @ ..] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let opts = match SetOptions::parse(options, ctx.command.name) {
        Ok(opts) => opts,
        Err(reply) => return reply,
    };

    // `GET` reports the old value, which must be a string. It is returned even
    // when an `NX`/`XX` condition aborts the write below.
    let old = if opts.get {
        match state.store.get(key) {
            Some(Object::String(bytes)) => Response::Bulk(bytes.clone()),
            Some(_) => return errors::wrong_type(),
            None => Response::NullBulk,
        }
    } else {
        Response::NullBulk
    };

    let exists = state.store.contains_key(key);
    let aborted = match &opts.condition {
        Some(Condition::Exists) => !exists,
        Some(Condition::NotExists) => exists,
        None => false,
    };
    if aborted {
        return if opts.get { old } else { Response::NullBulk };
    }

    store_string(state, key, value, opts.expiry);
    ctx.rewrite = Some(canonical_set(key, value, opts.expiry));

    if opts.get {
        old
    } else {
        Response::Simple("OK".to_string())
    }
}

/// `SETEX key seconds value` sets `key` to `value` with a TTL of `seconds`.
pub const SETEX: Command = Command {
    name: "SETEX",
    arity: Arity::Exact(4),
    write: true,
    auth_required: true,
    handler: setex,
};

fn setex(ctx: &mut Context, state: &mut State) -> Response {
    set_with_ttl(ctx, state, true)
}

/// `SETNX key value` sets `key` to `value` only if it does not exist, replying
/// with `1` if it was set and `0` otherwise.
pub const SETNX: Command = Command {
    name: "SETNX",
    arity: Arity::Exact(3),
    write: true,
    auth_required: true,
    handler: setnx,
};

fn setnx(ctx: &mut Context, state: &mut State) -> Response {
    let [key, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    if state.store.contains_key(key) {
        return Response::Integer(0);
    }

    state.store.set(key.clone(), Object::String(value.clone()));
    Response::Integer(1)
}

/// `SETRANGE key offset value` overwrites the value at `key` from `offset`,
/// zero-padding any gap, and replies with the new length.
pub const SETRANGE: Command = Command {
    name: "SETRANGE",
    arity: Arity::Exact(4),
    write: true,
    auth_required: true,
    handler: setrange,
};

fn setrange(ctx: &mut Context, state: &mut State) -> Response {
    let [key, offset, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let Some(offset) = super::parse_i64(offset) else {
        return errors::not_integer();
    };
    if offset < 0 {
        return Response::Error("ERR offset is out of range".to_string());
    }

    if value.is_empty() {
        return match state.store.get(key) {
            Some(Object::String(bytes)) => Response::Integer(bytes.len() as i64),
            Some(_) => errors::wrong_type(),
            None => Response::Integer(0),
        };
    }

    let mut stored = match state.store.get(key) {
        Some(Object::String(bytes)) => bytes.clone(),
        Some(_) => return errors::wrong_type(),
        None => Vec::new(),
    };

    let end = offset as u64 + value.len() as u64;
    if end > PROTO_MAX_BULK_LEN {
        return Response::Error(
            "ERR string exceeds maximum allowed size (proto-max-bulk-len)".to_string(),
        );
    }
    let end = end as usize;

    if stored.len() < end {
        stored.resize(end, 0);
    }
    stored[offset as usize..end].copy_from_slice(value);

    let len = stored.len() as i64;
    state.store.update(key.clone(), Object::String(stored));
    Response::Integer(len)
}

/// `STRLEN key` returns the length of the string at `key`, or `0` if it is missing.
pub const STRLEN: Command = Command {
    name: "STRLEN",
    arity: Arity::Exact(2),
    write: false,
    auth_required: true,
    handler: strlen,
};

fn strlen(ctx: &mut Context, state: &mut State) -> Response {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get(key) {
        Some(Object::String(bytes)) => Response::Integer(bytes.len() as i64),
        Some(_) => errors::wrong_type(),
        None => Response::Integer(0),
    }
}

/// `SUBSTR key start end` is a deprecated alias of `GETRANGE`.
pub const SUBSTR: Command = Command {
    name: "SUBSTR",
    arity: Arity::Exact(4),
    write: false,
    auth_required: true,
    handler: getrange,
};

// Helpers

/// Adds `delta` to the integer stored at `key`, treating a missing key as 0,
/// and replies with the new value.
fn apply_delta(state: &mut State, key: &[u8], delta: i64) -> Response {
    let current = match state.store.get(key) {
        Some(Object::String(bytes)) => match super::parse_i64(bytes) {
            Some(current) => current,
            None => return errors::not_integer(),
        },
        Some(_) => return errors::wrong_type(),
        None => 0,
    };

    let Some(next) = current.checked_add(delta) else {
        return errors::overflow();
    };

    state
        .store
        .update(key.to_vec(), Object::String(next.to_string().into_bytes()));
    Response::Integer(next)
}

/// Parses `bytes` as a float. NaN is rejected. An infinite result is accepted
/// only when the input spells out infinity, not when a finite magnitude overflows.
fn parse_float(bytes: &[u8]) -> Option<f64> {
    let text = std::str::from_utf8(bytes).ok()?;
    let value: f64 = text.parse().ok()?;

    if value.is_nan() {
        return None;
    }
    if value.is_infinite()
        && !matches!(
            text.to_ascii_lowercase().as_str(),
            "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity"
        )
    {
        return None;
    }

    Some(value)
}

/// Formats a float with 17 digits after the decimal point, then removes trailing
/// zeros and any trailing decimal point.
fn format_float(value: f64) -> String {
    let formatted = format!("{value:.17}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

/// Resolves an expiry option value to an absolute ms deadline. `seconds` selects
/// the unit and `absolute` whether the value is already an epoch time.
fn resolve_deadline(
    raw: &[u8],
    seconds: bool,
    absolute: bool,
    command: &str,
) -> Result<i64, Response> {
    let Some(value) = super::parse_i64(raw) else {
        return Err(errors::not_integer());
    };

    if value <= 0 {
        return Err(errors::invalid_expire_time(command));
    }

    let invalid = || errors::invalid_expire_time(command);

    let ms = if seconds {
        value.checked_mul(1000).ok_or_else(invalid)?
    } else {
        value
    };

    if absolute {
        Ok(ms)
    } else {
        ms.checked_add(Store::now()).ok_or_else(invalid)
    }
}

/// Stores `value` at `key`, applying `expiry` to its TTL. A deadline already in
/// the past removes the key.
fn store_string(state: &mut State, key: &[u8], value: &[u8], expiry: Expiry) {
    if let Expiry::At(deadline) = expiry
        && Store::is_expired(deadline)
    {
        state.store.remove_server(key);
        return;
    }

    let object = Object::String(value.to_vec());
    match expiry {
        Expiry::Keep => state.store.update(key.to_vec(), object),
        Expiry::Clear => state.store.set(key.to_vec(), object),
        Expiry::At(deadline) => {
            state.store.set(key.to_vec(), object);
            state.store.set_expiry(key, deadline);
        }
    }
}

/// Builds the replay-safe form of a `SET`, resolving a relative expiry to an
/// absolute `PXAT` so a replay does not recompute it against a later clock.
fn canonical_set(key: &[u8], value: &[u8], expiry: Expiry) -> Vec<Vec<u8>> {
    let mut argv = vec![b"SET".to_vec(), key.to_vec(), value.to_vec()];
    match expiry {
        Expiry::At(deadline) => {
            argv.push(b"PXAT".to_vec());
            argv.push(deadline.to_string().into_bytes());
        }
        Expiry::Keep => argv.push(b"KEEPTTL".to_vec()),
        Expiry::Clear => {}
    }
    argv
}

/// Stores `key value` with a required TTL taken from the second argument, in seconds or milliseconds.
fn set_with_ttl(ctx: &mut Context, state: &mut State, seconds: bool) -> Response {
    let [key, ttl, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let deadline = match resolve_deadline(ttl, seconds, false, ctx.command.name) {
        Ok(deadline) => deadline,
        Err(reply) => return reply,
    };

    let expiry = Expiry::At(deadline);
    store_string(state, key, value, expiry);
    ctx.rewrite = Some(canonical_set(key, value, expiry));
    Response::Simple("OK".to_string())
}

#[cfg(test)]
mod tests {
    use crate::command::test_utils::{cmd, dispatch, state};
    use crate::resp::Response;

    // APPEND

    #[test]
    fn creates_missing_key() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "hello"]), &mut state),
            Response::Integer(5)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"hello".to_vec())
        );
    }

    #[test]
    fn appends_to_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "hello"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", " world"]), &mut state),
            Response::Integer(11)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"hello world".to_vec())
        );
    }

    #[test]
    fn returns_new_length() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "ab"]), &mut state),
            Response::Integer(2)
        );
        assert_eq!(
            dispatch(&cmd(&["APPEND", "k", "cde"]), &mut state),
            Response::Integer(5)
        );
    }

    // DECR

    #[test]
    fn missing_key_starts_at_minus_one() {
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state()),
            Response::Integer(-1)
        );
    }

    #[test]
    fn decrements_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Response::Integer(4)
        );
    }

    #[test]
    fn decr_non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn underflow_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "-9223372036854775808"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECR", "n"]), &mut state),
            Response::Error("ERR increment or decrement would overflow".to_string())
        );
    }

    #[test]
    fn decr_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECR"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'decr' command".to_string())
        );
    }

    // DECRBY

    #[test]
    fn subtracts_from_missing_key() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "5"]), &mut state()),
            Response::Integer(-5)
        );
    }

    #[test]
    fn subtracts_from_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "3"]), &mut state),
            Response::Integer(7)
        );
    }

    #[test]
    fn negative_decrement_increments() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "-5"]), &mut state),
            Response::Integer(15)
        );
    }

    #[test]
    fn non_integer_decrement_is_error() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "abc"]), &mut state()),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn unnegatable_decrement_is_error() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n", "-9223372036854775808"]), &mut state()),
            Response::Error("ERR decrement would overflow".to_string())
        );
    }

    #[test]
    fn decrby_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["DECRBY", "n"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'decrby' command".to_string())
        );
    }

    // GET

    #[test]
    fn missing_key_is_null() {
        assert_eq!(
            dispatch(&cmd(&["GET", "nope"]), &mut state()),
            Response::NullBulk
        );
    }

    #[test]
    fn wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GET"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'get' command".to_string())
        );
    }

    // GETDEL

    #[test]
    fn getdel_returns_and_deletes() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETDEL", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::NullBulk
        );
    }

    #[test]
    fn getdel_missing_is_null() {
        assert_eq!(
            dispatch(&cmd(&["GETDEL", "nope"]), &mut state()),
            Response::NullBulk
        );
    }

    #[test]
    fn getdel_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETDEL", "k"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn getdel_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GETDEL"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'getdel' command".to_string())
        );
    }

    // GETEX

    #[test]
    fn getex_returns_value_and_keeps_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn getex_persist_removes_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "k", "PERSIST"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn getex_sets_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "k", "EX", "50"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(50)
        );
    }

    #[test]
    fn getex_past_deadline_expires_key() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "k", "EXAT", "1"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn getex_missing_is_null() {
        assert_eq!(
            dispatch(&cmd(&["GETEX", "nope"]), &mut state()),
            Response::NullBulk
        );
    }

    #[test]
    fn getex_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "l"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn getex_invalid_expire_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETEX", "k", "EX", "0"]), &mut state),
            Response::Error("ERR invalid expire time in 'getex' command".to_string())
        );
    }

    #[test]
    fn getex_conflicting_options_are_syntax_errors() {
        let mut state = state();
        for opts in [
            vec!["GETEX", "k", "FOO"],
            vec!["GETEX", "k", "PERSIST", "EX", "5"],
        ] {
            assert_eq!(
                dispatch(&cmd(&opts), &mut state),
                Response::Error("ERR syntax error".to_string()),
                "{opts:?}"
            );
        }
    }

    // GETRANGE

    #[test]
    fn getrange_returns_substring() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "0", "4"]), &mut state),
            Response::Bulk(b"Hello".to_vec())
        );
    }

    #[test]
    fn getrange_resolves_negative_offsets() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "-5", "-1"]), &mut state),
            Response::Bulk(b"World".to_vec())
        );
    }

    #[test]
    fn getrange_clamps_out_of_range_bounds() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "0", "100"]), &mut state),
            Response::Bulk(b"Hello World".to_vec())
        );
    }

    #[test]
    fn getrange_inverted_range_is_empty() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "5", "2"]), &mut state),
            Response::Bulk(Vec::new())
        );
    }

    #[test]
    fn getrange_start_past_end_is_empty() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "100", "200"]), &mut state),
            Response::Bulk(Vec::new())
        );
    }

    #[test]
    fn getrange_missing_key_is_empty() {
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "nope", "0", "-1"]), &mut state()),
            Response::Bulk(Vec::new())
        );
    }

    #[test]
    fn getrange_non_integer_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "a", "2"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn getrange_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "l", "0", "-1"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn getrange_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GETRANGE", "k", "0"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'getrange' command".to_string())
        );
    }

    // GETSET

    #[test]
    fn getset_returns_old_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETSET", "k", "v2"]), &mut state),
            Response::Bulk(b"v1".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v2".to_vec())
        );
    }

    #[test]
    fn getset_missing_returns_null() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["GETSET", "k", "v"]), &mut state),
            Response::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
    }

    #[test]
    fn getset_clears_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        dispatch(&cmd(&["GETSET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn getset_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GETSET", "k", "v"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn getset_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["GETSET", "k"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'getset' command".to_string())
        );
    }

    // INCR

    #[test]
    fn missing_key_starts_at_one() {
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state()),
            Response::Integer(1)
        );
    }

    #[test]
    fn increments_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Response::Integer(6)
        );
    }

    #[test]
    fn non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn overflow_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "9223372036854775807"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCR", "n"]), &mut state),
            Response::Error("ERR increment or decrement would overflow".to_string())
        );
    }

    #[test]
    fn incr_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCR"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'incr' command".to_string())
        );
    }

    // INCRBY

    #[test]
    fn adds_to_missing_key() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state()),
            Response::Integer(5)
        );
    }

    #[test]
    fn adds_to_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state),
            Response::Integer(15)
        );
    }

    #[test]
    fn negative_increment_decrements() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "-3"]), &mut state),
            Response::Integer(7)
        );
    }

    #[test]
    fn non_integer_increment_is_error() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "abc"]), &mut state()),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn incrby_non_integer_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n", "5"]), &mut state),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn incrby_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCRBY", "n"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'incrby' command".to_string())
        );
    }

    // INCRBYFLOAT

    #[test]
    fn incrbyfloat_adds_to_missing_key() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "0.1"]), &mut state()),
            Response::Bulk(b"0.10000000000000001".to_vec())
        );
    }

    #[test]
    fn incrbyfloat_adds_to_existing_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "10.5"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "0.1"]), &mut state),
            Response::Bulk(b"10.59999999999999964".to_vec())
        );
    }

    #[test]
    fn incrbyfloat_trims_to_integer() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "3"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "1.0"]), &mut state),
            Response::Bulk(b"4".to_vec())
        );
    }

    #[test]
    fn incrbyfloat_accepts_scientific_notation() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "5.0e3"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "200"]), &mut state),
            Response::Bulk(b"5200".to_vec())
        );
    }

    #[test]
    fn incrbyfloat_preserves_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "1.0"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "n", "100"]), &mut state);
        dispatch(&cmd(&["INCRBYFLOAT", "n", "1.0"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "n"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn incrbyfloat_non_float_value_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "1.0"]), &mut state),
            Response::Error("ERR value is not a valid float".to_string())
        );
    }

    #[test]
    fn incrbyfloat_non_float_increment_is_error() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "abc"]), &mut state()),
            Response::Error("ERR value is not a valid float".to_string())
        );
    }

    #[test]
    fn incrbyfloat_nan_increment_is_error() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "nan"]), &mut state()),
            Response::Error("ERR value is not a valid float".to_string())
        );
    }

    #[test]
    fn incrbyfloat_infinite_result_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "n", "1e308"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "1e308"]), &mut state),
            Response::Error("ERR increment would produce NaN or Infinity".to_string())
        );
    }

    #[test]
    fn incrbyfloat_infinity_literal_is_accepted_then_errors() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "inf"]), &mut state()),
            Response::Error("ERR increment would produce NaN or Infinity".to_string())
        );
    }

    #[test]
    fn incrbyfloat_overflowing_magnitude_is_invalid() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n", "1e400"]), &mut state()),
            Response::Error("ERR value is not a valid float".to_string())
        );
    }

    #[test]
    fn incrbyfloat_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "l", "1.0"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn incrbyfloat_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["INCRBYFLOAT", "n"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'incrbyfloat' command".to_string())
        );
    }

    // LCS

    #[test]
    fn lcs_returns_subsequence() {
        let mut state = state();
        dispatch(
            &cmd(&["MSET", "k1", "ohmytext", "k2", "mynewtext"]),
            &mut state,
        );
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1", "k2"]), &mut state),
            Response::Bulk(b"mytext".to_vec())
        );
    }

    #[test]
    fn lcs_len_returns_length() {
        let mut state = state();
        dispatch(
            &cmd(&["MSET", "k1", "ohmytext", "k2", "mynewtext"]),
            &mut state,
        );
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1", "k2", "LEN"]), &mut state),
            Response::Integer(6)
        );
    }

    #[test]
    fn lcs_no_common_subsequence_is_empty() {
        let mut state = state();
        dispatch(&cmd(&["MSET", "a", "abc", "b", "xyz"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LCS", "a", "b"]), &mut state),
            Response::Bulk(Vec::new())
        );
    }

    #[test]
    fn lcs_missing_keys_treated_as_empty() {
        assert_eq!(
            dispatch(&cmd(&["LCS", "none1", "none2"]), &mut state()),
            Response::Bulk(Vec::new())
        );
    }

    #[test]
    fn lcs_idx_returns_ranges() {
        let mut state = state();
        dispatch(
            &cmd(&["MSET", "k1", "ohmytext", "k2", "mynewtext"]),
            &mut state,
        );

        let range = |a: i64, b: i64, c: i64, d: i64| {
            Response::Array(vec![
                Response::Array(vec![Response::Integer(a), Response::Integer(b)]),
                Response::Array(vec![Response::Integer(c), Response::Integer(d)]),
            ])
        };
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1", "k2", "IDX"]), &mut state),
            Response::Array(vec![
                Response::Bulk(b"matches".to_vec()),
                Response::Array(vec![range(4, 7, 5, 8), range(2, 3, 0, 1)]),
                Response::Bulk(b"len".to_vec()),
                Response::Integer(6),
            ])
        );
    }

    #[test]
    fn lcs_idx_minmatchlen_filters_short_ranges() {
        let mut state = state();
        dispatch(
            &cmd(&["MSET", "k1", "ohmytext", "k2", "mynewtext"]),
            &mut state,
        );
        assert_eq!(
            dispatch(
                &cmd(&["LCS", "k1", "k2", "IDX", "MINMATCHLEN", "4"]),
                &mut state
            ),
            Response::Array(vec![
                Response::Bulk(b"matches".to_vec()),
                Response::Array(vec![Response::Array(vec![
                    Response::Array(vec![Response::Integer(4), Response::Integer(7)]),
                    Response::Array(vec![Response::Integer(5), Response::Integer(8)]),
                ])]),
                Response::Bulk(b"len".to_vec()),
                Response::Integer(6),
            ])
        );
    }

    #[test]
    fn lcs_idx_withmatchlen_appends_lengths() {
        let mut state = state();
        dispatch(
            &cmd(&["MSET", "k1", "ohmytext", "k2", "mynewtext"]),
            &mut state,
        );

        let range = |a: i64, b: i64, c: i64, d: i64, len: i64| {
            Response::Array(vec![
                Response::Array(vec![Response::Integer(a), Response::Integer(b)]),
                Response::Array(vec![Response::Integer(c), Response::Integer(d)]),
                Response::Integer(len),
            ])
        };
        assert_eq!(
            dispatch(
                &cmd(&["LCS", "k1", "k2", "IDX", "WITHMATCHLEN"]),
                &mut state
            ),
            Response::Array(vec![
                Response::Bulk(b"matches".to_vec()),
                Response::Array(vec![range(4, 7, 5, 8, 4), range(2, 3, 0, 1, 2)]),
                Response::Bulk(b"len".to_vec()),
                Response::Integer(6),
            ])
        );
    }

    #[test]
    fn lcs_len_with_idx_is_error() {
        let mut state = state();
        dispatch(&cmd(&["MSET", "k1", "a", "k2", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1", "k2", "LEN", "IDX"]), &mut state),
            Response::Error(
                "ERR If you want both the length and indexes, please just use IDX.".to_string()
            )
        );
    }

    #[test]
    fn lcs_wrong_type_is_error() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k1", "a"]), &mut state);
        dispatch(&cmd(&["RPUSH", "k2", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1", "k2"]), &mut state),
            Response::Error("ERR The specified keys must contain string values".to_string())
        );
    }

    #[test]
    fn lcs_non_integer_minmatchlen_is_error() {
        assert_eq!(
            dispatch(
                &cmd(&["LCS", "a", "b", "IDX", "MINMATCHLEN", "x"]),
                &mut state()
            ),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn lcs_unknown_option_is_syntax_error() {
        assert_eq!(
            dispatch(&cmd(&["LCS", "a", "b", "FOO"]), &mut state()),
            Response::Error("ERR syntax error".to_string())
        );
    }

    #[test]
    fn lcs_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["LCS", "k1"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'lcs' command".to_string())
        );
    }

    // MGET

    #[test]
    fn mget_returns_values_and_nils() {
        let mut state = state();
        dispatch(&cmd(&["SET", "a", "1"]), &mut state);
        dispatch(&cmd(&["SET", "c", "3"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["MGET", "a", "b", "c"]), &mut state),
            Response::Array(vec![
                Response::Bulk(b"1".to_vec()),
                Response::NullBulk,
                Response::Bulk(b"3".to_vec()),
            ])
        );
    }

    #[test]
    fn mget_wrong_type_is_nil() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        dispatch(&cmd(&["RPUSH", "l", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["MGET", "s", "l"]), &mut state),
            Response::Array(vec![Response::Bulk(b"v".to_vec()), Response::NullBulk])
        );
    }

    #[test]
    fn mget_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["MGET"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'mget' command".to_string())
        );
    }

    // MSET

    #[test]
    fn mset_sets_all_pairs() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["MSET", "a", "1", "b", "2"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "a"]), &mut state),
            Response::Bulk(b"1".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "b"]), &mut state),
            Response::Bulk(b"2".to_vec())
        );
    }

    #[test]
    fn mset_discards_existing_expiry() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        dispatch(&cmd(&["EXPIRE", "k", "100"]), &mut state);
        dispatch(&cmd(&["MSET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn mset_odd_arguments_is_error() {
        assert_eq!(
            dispatch(&cmd(&["MSET", "a", "1", "b"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'mset' command".to_string())
        );
    }

    // MSETNX

    #[test]
    fn msetnx_sets_when_all_absent() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["MSETNX", "a", "1", "b", "2"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "b"]), &mut state),
            Response::Bulk(b"2".to_vec())
        );
    }

    #[test]
    fn msetnx_sets_none_when_any_present() {
        let mut state = state();
        dispatch(&cmd(&["SET", "b", "old"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["MSETNX", "a", "1", "b", "2"]), &mut state),
            Response::Integer(0)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "a"]), &mut state),
            Response::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "b"]), &mut state),
            Response::Bulk(b"old".to_vec())
        );
    }

    #[test]
    fn msetnx_odd_arguments_is_error() {
        assert_eq!(
            dispatch(&cmd(&["MSETNX", "a", "1", "b"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'msetnx' command".to_string())
        );
    }

    // PSETEX

    #[test]
    fn psetex_sets_value_and_ttl() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["PSETEX", "k", "100000", "v"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn psetex_zero_is_error() {
        assert_eq!(
            dispatch(&cmd(&["PSETEX", "k", "0", "v"]), &mut state()),
            Response::Error("ERR invalid expire time in 'psetex' command".to_string())
        );
    }

    // SET

    #[test]
    fn stored_value_is_readable() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "foo", "bar"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "foo"]), &mut state),
            Response::Bulk(b"bar".to_vec())
        );
    }

    #[test]
    fn overwrites_existing() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v1"]), &mut state);
        dispatch(&cmd(&["SET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v2".to_vec())
        );
    }

    #[test]
    fn set_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SET", "k"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'set' command".to_string())
        );
    }

    #[test]
    fn set_ex_sets_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v", "EX", "100"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn set_keepttl_preserves_ttl() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v", "EX", "100"]), &mut state);
        dispatch(&cmd(&["SET", "k", "v2", "KEEPTTL"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn set_clears_ttl_by_default() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v", "EX", "100"]), &mut state);
        dispatch(&cmd(&["SET", "k", "v2"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(-1)
        );
    }

    #[test]
    fn set_exat_in_the_past_expires_the_key() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "EXAT", "1"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn set_nx_only_when_absent() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "NX"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v2", "NX"]), &mut state),
            Response::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
    }

    #[test]
    fn set_xx_only_when_present() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "XX"]), &mut state),
            Response::NullBulk
        );
        dispatch(&cmd(&["SET", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v2", "XX"]), &mut state),
            Response::Simple("OK".to_string())
        );
    }

    #[test]
    fn set_get_returns_old_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "old"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "new", "GET"]), &mut state),
            Response::Bulk(b"old".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"new".to_vec())
        );
    }

    #[test]
    fn set_get_on_missing_is_null() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "GET"]), &mut state),
            Response::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
    }

    #[test]
    fn set_get_on_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "GET"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn set_nx_get_aborts_but_returns_old_value() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "old"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "new", "NX", "GET"]), &mut state),
            Response::Bulk(b"old".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"old".to_vec())
        );
    }

    #[test]
    fn set_duplicate_expiry_option_is_allowed() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "EX", "1", "EX", "100"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn set_invalid_expiry_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "EX", "0"]), &mut state()),
            Response::Error("ERR invalid expire time in 'set' command".to_string())
        );
    }

    #[test]
    fn set_non_integer_expiry_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SET", "k", "v", "EX", "abc"]), &mut state()),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn set_conflicting_options_are_syntax_errors() {
        let mut state = state();
        for opts in [
            vec!["SET", "k", "v", "NX", "XX"],
            vec!["SET", "k", "v", "EX", "1", "PX", "1"],
            vec!["SET", "k", "v", "EX", "1", "KEEPTTL"],
            vec!["SET", "k", "v", "FOO"],
            vec!["SET", "k", "v", "EX"],
        ] {
            assert_eq!(
                dispatch(&cmd(&opts), &mut state),
                Response::Error("ERR syntax error".to_string()),
                "{opts:?}"
            );
        }
    }

    // SETEX

    #[test]
    fn setex_sets_value_and_ttl() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SETEX", "k", "100", "v"]), &mut state),
            Response::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["TTL", "k"]), &mut state),
            Response::Integer(100)
        );
    }

    #[test]
    fn setex_zero_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SETEX", "k", "0", "v"]), &mut state()),
            Response::Error("ERR invalid expire time in 'setex' command".to_string())
        );
    }

    #[test]
    fn setex_non_integer_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SETEX", "k", "abc", "v"]), &mut state()),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn setex_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SETEX", "k", "100"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'setex' command".to_string())
        );
    }

    // SETNX

    #[test]
    fn setnx_sets_when_absent() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SETNX", "k", "v"]), &mut state),
            Response::Integer(1)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v".to_vec())
        );
    }

    #[test]
    fn setnx_does_not_overwrite() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "v1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SETNX", "k", "v2"]), &mut state),
            Response::Integer(0)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "k"]), &mut state),
            Response::Bulk(b"v1".to_vec())
        );
    }

    #[test]
    fn setnx_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SETNX", "k"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'setnx' command".to_string())
        );
    }

    // SETRANGE

    #[test]
    fn setrange_overwrites() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "s", "6", "Redis"]), &mut state),
            Response::Integer(11)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "s"]), &mut state),
            Response::Bulk(b"Hello Redis".to_vec())
        );
    }

    #[test]
    fn setrange_extends_with_zero_padding() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "pad", "5", "xyz"]), &mut state),
            Response::Integer(8)
        );
        assert_eq!(
            dispatch(&cmd(&["GET", "pad"]), &mut state),
            Response::Bulk(vec![0, 0, 0, 0, 0, b'x', b'y', b'z'])
        );
    }

    #[test]
    fn setrange_empty_value_on_missing_key_is_zero() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "e", "0", ""]), &mut state),
            Response::Integer(0)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "e"]), &mut state),
            Response::Integer(0)
        );
    }

    #[test]
    fn setrange_empty_value_returns_existing_length() {
        let mut state = state();
        dispatch(&cmd(&["SET", "e", "abc"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "e", "0", ""]), &mut state),
            Response::Integer(3)
        );
    }

    #[test]
    fn setrange_negative_offset_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "s", "-1", "x"]), &mut state()),
            Response::Error("ERR offset is out of range".to_string())
        );
    }

    #[test]
    fn setrange_non_integer_offset_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "s", "a", "x"]), &mut state()),
            Response::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn setrange_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "x"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "l", "0", "x"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn setrange_exceeding_max_size_is_error() {
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "big", "536870912", "x"]), &mut state()),
            Response::Error(
                "ERR string exceeds maximum allowed size (proto-max-bulk-len)".to_string()
            )
        );
    }

    #[test]
    fn setrange_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["SETRANGE", "k", "0"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'setrange' command".to_string())
        );
    }

    // STRLEN

    #[test]
    fn strlen_returns_length() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "hello"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["STRLEN", "k"]), &mut state),
            Response::Integer(5)
        );
    }

    #[test]
    fn strlen_missing_is_zero() {
        assert_eq!(
            dispatch(&cmd(&["STRLEN", "nope"]), &mut state()),
            Response::Integer(0)
        );
    }

    #[test]
    fn strlen_wrong_type() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["STRLEN", "k"]), &mut state),
            Response::Error(
                "WRONGTYPE Operation against a key holding the wrong kind of value".to_string()
            )
        );
    }

    #[test]
    fn strlen_wrong_args() {
        assert_eq!(
            dispatch(&cmd(&["STRLEN"]), &mut state()),
            Response::Error("ERR wrong number of arguments for 'strlen' command".to_string())
        );
    }

    // SUBSTR

    #[test]
    fn substr_aliases_getrange() {
        let mut state = state();
        dispatch(&cmd(&["SET", "k", "Hello World"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["SUBSTR", "k", "0", "4"]), &mut state),
            Response::Bulk(b"Hello".to_vec())
        );
        assert_eq!(
            dispatch(&cmd(&["SUBSTR", "k", "-5", "-1"]), &mut state),
            Response::Bulk(b"World".to_vec())
        );
    }
}
