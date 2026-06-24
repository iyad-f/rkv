// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use super::{Arity, Command, Context, errors};
use crate::object::Object;
use crate::resp::Value;
use crate::server::State;

/// `RPUSH key value [value ...]` appends each value to the tail of the list at
/// `key`, creating it if absent, and replies with the list's new length.
pub const RPUSH: Command = Command {
    name: "RPUSH",
    arity: Arity::Min(3),
    write: true,
    handler: rpush,
};

fn rpush(ctx: &mut Context, state: &mut State) -> Value {
    push(ctx, state, End::Tail, false)
}

/// `LPUSH key value [value ...]` prepends each value to the head of the list at
/// `key`, creating it if absent, and replies with the list's new length.
pub const LPUSH: Command = Command {
    name: "LPUSH",
    arity: Arity::Min(3),
    write: true,
    handler: lpush,
};

fn lpush(ctx: &mut Context, state: &mut State) -> Value {
    push(ctx, state, End::Head, false)
}

/// `RPUSHX key value [value ...]` appends to the tail of the list at `key` only
/// if it already exists, replying with the new length or `0` if it is missing.
pub const RPUSHX: Command = Command {
    name: "RPUSHX",
    arity: Arity::Min(3),
    write: true,
    handler: rpushx,
};

fn rpushx(ctx: &mut Context, state: &mut State) -> Value {
    push(ctx, state, End::Tail, true)
}

/// `LPUSHX key value [value ...]` prepends to the head of the list at `key` only
/// if it already exists, replying with the new length or `0` if it is missing.
pub const LPUSHX: Command = Command {
    name: "LPUSHX",
    arity: Arity::Min(3),
    write: true,
    handler: lpushx,
};

fn lpushx(ctx: &mut Context, state: &mut State) -> Value {
    push(ctx, state, End::Head, true)
}

/// `LLEN key` replies with the number of elements in the list at `key`.
pub const LLEN: Command = Command {
    name: "LLEN",
    arity: Arity::Exact(2),
    write: false,
    handler: llen,
};

fn llen(ctx: &mut Context, state: &mut State) -> Value {
    let [key] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get(key) {
        Some(Object::List(list)) => Value::Integer(list.len() as i64),
        Some(_) => errors::wrong_type(),
        None => Value::Integer(0),
    }
}

/// `LRANGE key start stop` replies with the elements of the list at `key` from
/// index `start` to `stop`, inclusive. Negative indices count back from the end.
pub const LRANGE: Command = Command {
    name: "LRANGE",
    arity: Arity::Exact(4),
    write: false,
    handler: lrange,
};

fn lrange(ctx: &mut Context, state: &mut State) -> Value {
    let [key, start, stop] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let (Some(start), Some(stop)) = (super::parse_i64(start), super::parse_i64(stop)) else {
        return errors::not_integer();
    };

    let list = match state.store.get(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => return Value::Array(Vec::new()),
    };
    let len = list.len() as i64;

    // Resolve negative indices against the length, then clamp both ends to the
    // valid range. With `stop` capped at `len - 1`, `start > stop` alone covers
    // both an inverted range and a `start` past the end.
    let start = (if start < 0 { start + len } else { start }).max(0);
    let stop = (if stop < 0 { stop + len } else { stop }).min(len - 1);

    if start > stop {
        return Value::Array(Vec::new());
    }

    let elements = list
        .range(start as usize..=stop as usize)
        .map(|value| Value::Bulk(value.clone()))
        .collect();
    Value::Array(elements)
}

/// `LPOP key [count]` removes and replies with the first element of the list at
/// `key`, or the first `count` elements as an array. Replies nil if the key does
/// not exist.
pub const LPOP: Command = Command {
    name: "LPOP",
    arity: Arity::Min(2),
    write: true,
    handler: lpop,
};

fn lpop(ctx: &mut Context, state: &mut State) -> Value {
    pop(ctx, state, End::Head)
}

/// `RPOP key [count]` removes and replies with the last element of the list at
/// `key`, or the last `count` elements as an array. Replies nil if the key does
/// not exist.
pub const RPOP: Command = Command {
    name: "RPOP",
    arity: Arity::Min(2),
    write: true,
    handler: rpop,
};

fn rpop(ctx: &mut Context, state: &mut State) -> Value {
    pop(ctx, state, End::Tail)
}

/// `LINDEX key index` replies with the element at `index` in the list at `key`,
/// or nil if the index is out of range or the key is missing. Negative indices
/// count back from the end.
pub const LINDEX: Command = Command {
    name: "LINDEX",
    arity: Arity::Exact(3),
    write: false,
    handler: lindex,
};

fn lindex(ctx: &mut Context, state: &mut State) -> Value {
    let [key, index] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let Some(index) = super::parse_i64(index) else {
        return errors::not_integer();
    };

    match state.store.get(key) {
        Some(Object::List(list)) => match resolve_index(index, list.len() as i64) {
            Some(i) => Value::Bulk(list[i].clone()),
            None => Value::NullBulk,
        },
        Some(_) => errors::wrong_type(),
        None => Value::NullBulk,
    }
}

/// `LSET key index value` sets the element at `index` in the list at `key`, with
/// negative indices counting back from the end. Errors if the key is missing or
/// the index is out of range.
pub const LSET: Command = Command {
    name: "LSET",
    arity: Arity::Exact(4),
    write: true,
    handler: lset,
};

fn lset(ctx: &mut Context, state: &mut State) -> Value {
    let [key, index, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let Some(index) = super::parse_i64(index) else {
        return errors::not_integer();
    };

    let list = match state.store.get_mut(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => return errors::no_such_key(),
    };

    let Some(i) = resolve_index(index, list.len() as i64) else {
        return errors::index_out_of_range();
    };

    list[i] = value.clone();
    state.store.incr_dirty();
    Value::Simple("OK".to_string())
}

/// `LTRIM key start stop` keeps only the elements of the list at `key` in the
/// inclusive range `start` to `stop`, removing the rest. Negative indices count
/// back from the end. Deletes the key if nothing remains, and always replies OK.
pub const LTRIM: Command = Command {
    name: "LTRIM",
    arity: Arity::Exact(4),
    write: true,
    handler: ltrim,
};

fn ltrim(ctx: &mut Context, state: &mut State) -> Value {
    let [key, start, stop] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let (Some(start), Some(stop)) = (super::parse_i64(start), super::parse_i64(stop)) else {
        return errors::not_integer();
    };

    let list = match state.store.get_mut(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => return Value::Simple("OK".to_string()),
    };
    let len = list.len() as i64;

    let start = (if start < 0 { start + len } else { start }).max(0);
    let stop = (if stop < 0 { stop + len } else { stop }).min(len - 1);

    if start > stop {
        // Nothing is kept, so drop the key entirely.
        state.store.remove(key);
    } else {
        list.drain(stop as usize + 1..);
        list.drain(..start as usize);
        state.store.incr_dirty();
    }

    Value::Simple("OK".to_string())
}

/// `LINSERT key BEFORE|AFTER pivot value` inserts `value` before or after the
/// first occurrence of `pivot` in the list at `key`. Replies with the new length,
/// `-1` if `pivot` is absent, or `0` if the key does not exist.
pub const LINSERT: Command = Command {
    name: "LINSERT",
    arity: Arity::Exact(5),
    write: true,
    handler: linsert,
};

fn linsert(ctx: &mut Context, state: &mut State) -> Value {
    let [key, whence, pivot, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let before = match whence.to_ascii_uppercase().as_slice() {
        b"BEFORE" => true,
        b"AFTER" => false,
        _ => return errors::syntax_error(),
    };

    let list = match state.store.get_mut(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => return Value::Integer(0),
    };

    let Some(pos) = list.iter().position(|element| element == pivot) else {
        return Value::Integer(-1);
    };

    list.insert(if before { pos } else { pos + 1 }, value.clone());
    let len = list.len() as i64;
    state.store.incr_dirty();
    Value::Integer(len)
}

/// `LREM key count value` removes occurrences of `value` from the list at `key`.
/// A positive `count` removes from the head, a negative one from the tail, and
/// zero removes all. Replies with the number removed, deleting the key if the
/// list becomes empty.
pub const LREM: Command = Command {
    name: "LREM",
    arity: Arity::Exact(4),
    write: true,
    handler: lrem,
};

fn lrem(ctx: &mut Context, state: &mut State) -> Value {
    let [key, count, value] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let Some(count) = super::parse_i64(count) else {
        return errors::not_integer();
    };

    let list = match state.store.get_mut(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => return Value::Integer(0),
    };

    let mut removed: i64 = 0;
    let limit = count.unsigned_abs();
    if count >= 0 {
        // Walk head to tail, removing up to `limit` matches (0 means no limit).
        list.retain(|element| {
            if element == value && (limit == 0 || (removed as u64) < limit) {
                removed += 1;
                false
            } else {
                true
            }
        });
    } else {
        // Walk tail to head, keeping the rest, then restore the original order.
        let mut kept: Vec<Vec<u8>> = Vec::with_capacity(list.len());
        for element in list.iter().rev() {
            if element == value && (removed as u64) < limit {
                removed += 1;
            } else {
                kept.push(element.clone());
            }
        }
        kept.reverse();
        *list = kept.into();
    }

    if removed > 0 {
        if list.is_empty() {
            state.store.remove(key);
        } else {
            state.store.incr_dirty();
        }
    }

    Value::Integer(removed)
}

/// `LMOVE source destination LEFT|RIGHT LEFT|RIGHT` pops an element from one end
/// of `source` and pushes it onto an end of `destination`, replying with the
/// moved element or nil if `source` is missing. Deletes `source` if it empties.
pub const LMOVE: Command = Command {
    name: "LMOVE",
    arity: Arity::Exact(5),
    write: true,
    handler: lmove,
};

fn lmove(ctx: &mut Context, state: &mut State) -> Value {
    let [source, destination, from, to] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };
    let (Some(from), Some(to)) = (parse_side(from), parse_side(to)) else {
        return errors::syntax_error();
    };

    move_element(state, source, destination, from, to)
}

/// `LPOS key element [RANK rank] [COUNT count] [MAXLEN maxlen]` replies with the
/// index of `element` in the list at `key`. `RANK` selects which match to start
/// from (negative scans from the tail), `COUNT` returns up to that many matches
/// as an array (`0` means all), and `MAXLEN` limits how many elements are
/// scanned. Without `COUNT` replies a single index or nil; with it, an array.
pub const LPOS: Command = Command {
    name: "LPOS",
    arity: Arity::Min(3),
    write: false,
    handler: lpos,
};

fn lpos(ctx: &mut Context, state: &mut State) -> Value {
    let [key, element, options @ ..] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    let mut rank: i64 = 1;
    let mut count: Option<i64> = None;
    let mut maxlen: i64 = 0;

    let mut options = options.iter();
    while let Some(option) = options.next() {
        // Every option takes a value, so a trailing bare option is a syntax error.
        let Some(value) = options.next() else {
            return errors::syntax_error();
        };
        match option.to_ascii_uppercase().as_slice() {
            b"RANK" => {
                let Some(value) = super::parse_i64(value) else {
                    return errors::not_integer();
                };
                if value == 0 {
                    return Value::Error(
                        "ERR RANK can't be zero: use 1 to start from the first match, 2 from \
                         the second ... or use negative to start from the end of the list"
                            .to_string(),
                    );
                }
                rank = value;
            }
            b"COUNT" => {
                let Some(value) = super::parse_i64(value).filter(|value| *value >= 0) else {
                    return Value::Error("ERR COUNT can't be negative".to_string());
                };
                count = Some(value);
            }
            b"MAXLEN" => {
                let Some(value) = super::parse_i64(value).filter(|value| *value >= 0) else {
                    return Value::Error("ERR MAXLEN can't be negative".to_string());
                };
                maxlen = value;
            }
            _ => return errors::syntax_error(),
        }
    }

    // A negative rank scans from the tail toward the head.
    let backward = rank < 0;
    let rank = rank.unsigned_abs();

    let list = match state.store.get(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        None => {
            return if count.is_some() {
                Value::Array(Vec::new())
            } else {
                Value::NullBulk
            };
        }
    };

    let len = list.len();
    let examined = if maxlen == 0 {
        len
    } else {
        (maxlen as usize).min(len)
    };

    let mut matches: u64 = 0;
    let mut found = Vec::new();
    for offset in 0..examined {
        // Positions are always reported relative to head, whichever way we scan.
        let position = if backward { len - 1 - offset } else { offset };
        if &list[position] != element {
            continue;
        }

        matches += 1;
        if matches < rank {
            continue;
        }

        match count {
            None => return Value::Integer(position as i64),
            Some(count) => {
                found.push(Value::Integer(position as i64));
                if count != 0 && matches - rank + 1 >= count as u64 {
                    break;
                }
            }
        }
    }

    match count {
        Some(_) => Value::Array(found),
        None => Value::NullBulk,
    }
}

/// `LMPOP numkeys key [key ...] LEFT|RIGHT [COUNT count]` pops up to `count`
/// elements (default 1) from the given end of the first non-empty list among the
/// keys. Replies with `[key, [elements]]`, or a nil array if none have elements.
pub const LMPOP: Command = Command {
    name: "LMPOP",
    arity: Arity::Min(4),
    write: true,
    handler: lmpop,
};

fn lmpop(ctx: &mut Context, state: &mut State) -> Value {
    let args = ctx.args;

    let Some(numkeys) = args
        .first()
        .and_then(|n| super::parse_i64(n))
        .filter(|n| *n >= 1)
    else {
        return Value::Error("ERR numkeys should be greater than 0".to_string());
    };
    let numkeys = numkeys as usize;

    // After numkeys come its keys, then the LEFT/RIGHT side.
    let side_index = 1 + numkeys;
    if side_index >= args.len() {
        return errors::syntax_error();
    }
    let keys = &args[1..side_index];
    let Some(end) = parse_side(&args[side_index]) else {
        return errors::syntax_error();
    };

    // The only remaining argument is an optional COUNT.
    let count = match &args[side_index + 1..] {
        [] => 1,
        [option, value] if option.eq_ignore_ascii_case(b"COUNT") => {
            let Some(count) = super::parse_i64(value).filter(|count| *count >= 1) else {
                return Value::Error("ERR count should be greater than 0".to_string());
            };
            count as usize
        }
        _ => return errors::syntax_error(),
    };

    // Pop from the first key that holds a non-empty list.
    for key in keys {
        let list = match state.store.get_mut(key) {
            Some(Object::List(list)) => list,
            Some(_) => return errors::wrong_type(),
            None => continue,
        };

        let popped = std::iter::from_fn(|| pop_end(list, end))
            .take(count)
            .map(Value::Bulk)
            .collect();
        if list.is_empty() {
            state.store.remove(key);
        } else {
            state.store.incr_dirty();
        }
        return Value::Array(vec![Value::Bulk(key.to_vec()), Value::Array(popped)]);
    }

    Value::NullArray
}

/// Which end of a list a push or pop acts on.
#[derive(Clone, Copy)]
enum End {
    /// The tail, the right-hand end where `RPUSH` appends.
    Tail,

    /// The head, the left-hand end where `LPUSH` prepends.
    Head,
}

/// Pushes each of `values` onto the given `end` of the list at `key`, replying
/// with the list's new length. Creates the list when the key does not exist,
/// unless `xx` is set, in which case a missing key is left untouched and replies
/// `0`. Replies with a type error when the key holds a value that is not a list.
fn push(ctx: &mut Context, state: &mut State, end: End, xx: bool) -> Value {
    let [key, values @ ..] = ctx.args else {
        return errors::wrong_args(ctx.command.name);
    };

    match state.store.get_mut(key) {
        Some(Object::List(list)) => {
            push_values(list, values, end);
            let len = list.len() as i64;
            state.store.incr_dirty();
            Value::Integer(len)
        }
        Some(_) => errors::wrong_type(),
        None if xx => Value::Integer(0),
        None => {
            let mut list = VecDeque::new();
            push_values(&mut list, values, end);
            let len = list.len() as i64;
            state.store.set(key.clone(), Object::List(list));
            Value::Integer(len)
        }
    }
}

/// Pushes each of `values` onto the given `end` of `list`.
fn push_values(list: &mut VecDeque<Vec<u8>>, values: &[Vec<u8>], end: End) {
    let push_fn = match end {
        End::Tail => VecDeque::push_back,
        End::Head => VecDeque::push_front,
    };

    for value in values {
        push_fn(list, value.clone());
    }
}

/// Resolves an `index` (negatives count back from the end) to a valid position in
/// a list of length `len`, or `None` if it falls outside the list.
fn resolve_index(index: i64, len: i64) -> Option<usize> {
    let index = if index < 0 { index + len } else { index };
    (0..len).contains(&index).then_some(index as usize)
}

/// Parses a `LEFT`/`RIGHT` argument into the list end it names.
fn parse_side(arg: &[u8]) -> Option<End> {
    match arg.to_ascii_uppercase().as_slice() {
        b"LEFT" => Some(End::Head),
        b"RIGHT" => Some(End::Tail),
        _ => None,
    }
}

/// Pops one element from the given `end` of `list`.
fn pop_end(list: &mut VecDeque<Vec<u8>>, end: End) -> Option<Vec<u8>> {
    match end {
        End::Tail => list.pop_back(),
        End::Head => list.pop_front(),
    }
}

/// Pushes `value` onto the given `end` of `list`.
fn push_end(list: &mut VecDeque<Vec<u8>>, end: End, value: Vec<u8>) {
    match end {
        End::Tail => list.push_back(value),
        End::Head => list.push_front(value),
    }
}

/// Moves an element from the `from` end of `source` to the `to` end of
/// `destination`, replying with it. Replies nil when `source` is missing, a type
/// error when either key holds a non-list, creates `destination` if needed, and
/// deletes `source` if it empties. `source` and `destination` may be the same key.
fn move_element(state: &mut State, source: &[u8], destination: &[u8], from: End, to: End) -> Value {
    if let Some(object) = state.store.get(destination)
        && !matches!(object, Object::List(_))
    {
        return errors::wrong_type();
    }

    // The same key is a rotation within one list, which never empties it.
    if source == destination {
        let list = match state.store.get_mut(source) {
            Some(Object::List(list)) => list,
            Some(_) => return errors::wrong_type(),
            None => return Value::NullBulk,
        };
        let Some(value) = pop_end(list, from) else {
            return Value::NullBulk;
        };
        push_end(list, to, value.clone());
        state.store.incr_dirty();
        return Value::Bulk(value);
    }

    // Pop from the source, deleting it if it empties.
    let value = {
        let list = match state.store.get_mut(source) {
            Some(Object::List(list)) => list,
            Some(_) => return errors::wrong_type(),
            None => return Value::NullBulk,
        };
        let Some(value) = pop_end(list, from) else {
            return Value::NullBulk;
        };
        if list.is_empty() {
            state.store.remove(source);
        } else {
            state.store.incr_dirty();
        }
        value
    };

    // Push onto the destination, creating it if needed.
    match state.store.get_mut(destination) {
        Some(Object::List(list)) => push_end(list, to, value.clone()),
        Some(_) => return errors::wrong_type(),
        None => {
            let mut list = VecDeque::new();
            push_end(&mut list, to, value.clone());
            state.store.set(destination.to_vec(), Object::List(list));
        }
    }
    state.store.incr_dirty();
    Value::Bulk(value)
}

/// Removes elements from the given `end` of the list at `key`. Without a count it
/// removes one and replies with it as a bulk string, with a count it removes up
/// to that many and replies with them as an array. Deletes the key if the list
/// becomes empty. Replies nil for a missing key, and a type error when the value
/// is not a list.
fn pop(ctx: &mut Context, state: &mut State, end: End) -> Value {
    let (key, count) = match ctx.args {
        [key] => (key, None),
        [key, count] => {
            let Some(count) = super::parse_i64(count) else {
                return errors::not_integer();
            };
            if count < 0 {
                return errors::out_of_range_positive();
            }
            (key, Some(count as usize))
        }
        _ => return errors::wrong_args(ctx.command.name),
    };

    let list = match state.store.get_mut(key) {
        Some(Object::List(list)) => list,
        Some(_) => return errors::wrong_type(),
        // A missing key is a nil array with a count, a nil bulk string without.
        None => {
            return if count.is_some() {
                Value::NullArray
            } else {
                Value::NullBulk
            };
        }
    };

    // A count of zero pops nothing and leaves the list untouched.
    if count == Some(0) {
        return Value::Array(Vec::new());
    }

    let pop_fn = match end {
        End::Tail => VecDeque::pop_back,
        End::Head => VecDeque::pop_front,
    };

    let reply = match count {
        None => match pop_fn(list) {
            Some(value) => Value::Bulk(value),
            None => return Value::NullBulk,
        },
        Some(count) => {
            let popped = std::iter::from_fn(|| pop_fn(list))
                .take(count)
                .map(Value::Bulk)
                .collect();
            Value::Array(popped)
        }
    };

    // Drop the key once its last element is gone, rather than leave an empty list.
    if list.is_empty() {
        state.store.remove(key);
    } else {
        state.store.incr_dirty();
    }

    reply
}

#[cfg(test)]
mod tests {
    use crate::command::{
        dispatch,
        test_utils::{cmd, state},
    };
    use crate::resp::Value;

    /// A bulk-string reply value, for terse assertions.
    fn bulk(s: &str) -> Value {
        Value::Bulk(s.as_bytes().to_vec())
    }

    const WRONG_TYPE: &str = "WRONGTYPE Operation against a key holding the wrong kind of value";

    // RPUSH / LPUSH

    #[test]
    fn rpush_appends_in_order_and_returns_length() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state),
            Value::Integer(3)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b"), bulk("c")])
        );
    }

    #[test]
    fn lpush_prepends_so_order_reverses() {
        let mut state = state();
        dispatch(&cmd(&["LPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("c"), bulk("b"), bulk("a")])
        );
    }

    #[test]
    fn push_extends_an_existing_list() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["RPUSH", "l", "b", "c"]), &mut state),
            Value::Integer(3)
        );
    }

    #[test]
    fn push_against_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["RPUSH", "s", "x"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LLEN

    #[test]
    fn llen_reports_length() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LLEN", "l"]), &mut state),
            Value::Integer(3)
        );
    }

    #[test]
    fn llen_missing_key_is_zero() {
        assert_eq!(
            dispatch(&cmd(&["LLEN", "nope"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn llen_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LLEN", "s"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LRANGE

    #[test]
    fn lrange_returns_inclusive_subrange() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c", "d"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "1"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b")])
        );
    }

    #[test]
    fn lrange_resolves_negative_indices() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c", "d"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "-2", "-1"]), &mut state),
            Value::Array(vec![bulk("c"), bulk("d")])
        );
    }

    #[test]
    fn lrange_clamps_out_of_range_bounds() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c", "d"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "-100", "100"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b"), bulk("c"), bulk("d")])
        );
    }

    #[test]
    fn lrange_inverted_range_is_empty() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "2", "1"]), &mut state),
            Value::Array(vec![])
        );
    }

    #[test]
    fn lrange_missing_key_is_empty_array() {
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "nope", "0", "-1"]), &mut state()),
            Value::Array(vec![])
        );
    }

    #[test]
    fn lrange_non_integer_index_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "x", "1"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    // LPOP / RPOP

    #[test]
    fn lpop_removes_from_head() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(dispatch(&cmd(&["LPOP", "l"]), &mut state), bulk("a"));
    }

    #[test]
    fn rpop_removes_from_tail() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(dispatch(&cmd(&["RPOP", "l"]), &mut state), bulk("c"));
    }

    #[test]
    fn pop_missing_key_is_nil_bulk() {
        assert_eq!(
            dispatch(&cmd(&["LPOP", "nope"]), &mut state()),
            Value::NullBulk
        );
    }

    #[test]
    fn popping_the_last_element_deletes_the_key() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "only"]), &mut state);
        dispatch(&cmd(&["LPOP", "l"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "l"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn lpop_with_count_returns_an_array() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOP", "l", "2"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b")])
        );
    }

    #[test]
    fn rpop_with_count_returns_tail_first() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["RPOP", "l", "2"]), &mut state),
            Value::Array(vec![bulk("c"), bulk("b")])
        );
    }

    #[test]
    fn pop_count_zero_is_empty_array_and_no_change() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOP", "l", "0"]), &mut state),
            Value::Array(vec![])
        );
        assert_eq!(
            dispatch(&cmd(&["LLEN", "l"]), &mut state),
            Value::Integer(2)
        );
    }

    #[test]
    fn pop_count_over_length_returns_all_and_deletes() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOP", "l", "5"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b")])
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "l"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn pop_with_count_on_missing_key_is_nil_array() {
        assert_eq!(
            dispatch(&cmd(&["LPOP", "nope", "2"]), &mut state()),
            Value::NullArray
        );
    }

    #[test]
    fn pop_negative_count_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOP", "l", "-1"]), &mut state),
            Value::Error("ERR value is out of range, must be positive".to_string())
        );
    }

    #[test]
    fn pop_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOP", "s"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LINDEX

    #[test]
    fn lindex_returns_element_at_index() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(dispatch(&cmd(&["LINDEX", "l", "0"]), &mut state), bulk("a"));
        assert_eq!(dispatch(&cmd(&["LINDEX", "l", "2"]), &mut state), bulk("c"));
    }

    #[test]
    fn lindex_resolves_negative_index() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "l", "-1"]), &mut state),
            bulk("c")
        );
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "l", "-3"]), &mut state),
            bulk("a")
        );
    }

    #[test]
    fn lindex_out_of_range_is_nil() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "l", "5"]), &mut state),
            Value::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "l", "-9"]), &mut state),
            Value::NullBulk
        );
    }

    #[test]
    fn lindex_missing_key_is_nil() {
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "nope", "0"]), &mut state()),
            Value::NullBulk
        );
    }

    #[test]
    fn lindex_non_integer_index_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "l", "x"]), &mut state),
            Value::Error("ERR value is not an integer or out of range".to_string())
        );
    }

    #[test]
    fn lindex_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINDEX", "s", "0"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LSET

    #[test]
    fn lset_replaces_element() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LSET", "l", "1", "B"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("B"), bulk("c")])
        );
    }

    #[test]
    fn lset_resolves_negative_index() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        dispatch(&cmd(&["LSET", "l", "-1", "C"]), &mut state);
        assert_eq!(dispatch(&cmd(&["LINDEX", "l", "2"]), &mut state), bulk("C"));
    }

    #[test]
    fn lset_out_of_range_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LSET", "l", "5", "x"]), &mut state),
            Value::Error("ERR index out of range".to_string())
        );
    }

    #[test]
    fn lset_missing_key_errors() {
        assert_eq!(
            dispatch(&cmd(&["LSET", "nope", "0", "x"]), &mut state()),
            Value::Error("ERR no such key".to_string())
        );
    }

    #[test]
    fn lset_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LSET", "s", "0", "x"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LPUSHX / RPUSHX

    #[test]
    fn pushx_on_missing_key_does_nothing() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["RPUSHX", "nope", "a"]), &mut state),
            Value::Integer(0)
        );
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "nope"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn pushx_extends_an_existing_list() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["RPUSHX", "l", "b", "c"]), &mut state),
            Value::Integer(3)
        );
        assert_eq!(
            dispatch(&cmd(&["LPUSHX", "l", "x"]), &mut state),
            Value::Integer(4)
        );
    }

    #[test]
    fn pushx_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["RPUSHX", "s", "x"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LTRIM

    #[test]
    fn ltrim_keeps_the_inclusive_range() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c", "d", "e"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LTRIM", "l", "1", "3"]), &mut state),
            Value::Simple("OK".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("b"), bulk("c"), bulk("d")])
        );
    }

    #[test]
    fn ltrim_empty_range_deletes_the_key() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        dispatch(&cmd(&["LTRIM", "l", "2", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "l"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn ltrim_missing_key_is_ok() {
        assert_eq!(
            dispatch(&cmd(&["LTRIM", "nope", "0", "-1"]), &mut state()),
            Value::Simple("OK".to_string())
        );
    }

    // LINSERT

    #[test]
    fn linsert_before_and_after() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINSERT", "l", "BEFORE", "b", "X"]), &mut state),
            Value::Integer(4)
        );
        assert_eq!(
            dispatch(&cmd(&["LINSERT", "l", "AFTER", "b", "Y"]), &mut state),
            Value::Integer(5)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("X"), bulk("b"), bulk("Y"), bulk("c")])
        );
    }

    #[test]
    fn linsert_missing_pivot_is_minus_one() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINSERT", "l", "BEFORE", "z", "x"]), &mut state),
            Value::Integer(-1)
        );
    }

    #[test]
    fn linsert_missing_key_is_zero() {
        assert_eq!(
            dispatch(&cmd(&["LINSERT", "nope", "BEFORE", "a", "x"]), &mut state()),
            Value::Integer(0)
        );
    }

    #[test]
    fn linsert_bad_direction_is_syntax_error() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LINSERT", "l", "SIDE", "a", "x"]), &mut state),
            Value::Error("ERR syntax error".to_string())
        );
    }

    // LREM

    #[test]
    fn lrem_positive_count_removes_from_head() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a", "c", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LREM", "l", "2", "a"]), &mut state),
            Value::Integer(2)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("b"), bulk("c"), bulk("a")])
        );
    }

    #[test]
    fn lrem_negative_count_removes_from_tail() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a", "c", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LREM", "l", "-2", "a"]), &mut state),
            Value::Integer(2)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("a"), bulk("b"), bulk("c")])
        );
    }

    #[test]
    fn lrem_zero_count_removes_all() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a", "c", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LREM", "l", "0", "a"]), &mut state),
            Value::Integer(3)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "l", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("b"), bulk("c")])
        );
    }

    #[test]
    fn lrem_emptying_the_list_deletes_the_key() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "a"]), &mut state);
        dispatch(&cmd(&["LREM", "l", "0", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "l"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn lrem_missing_key_or_value_is_zero() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["LREM", "nope", "1", "a"]), &mut state),
            Value::Integer(0)
        );
        dispatch(&cmd(&["RPUSH", "l", "a", "b"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LREM", "l", "1", "zzz"]), &mut state),
            Value::Integer(0)
        );
    }

    // LMOVE

    #[test]
    fn lmove_moves_between_ends() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "a", "1", "2", "3"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "a", "b", "LEFT", "RIGHT"]), &mut state),
            bulk("1")
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "a", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("2"), bulk("3")])
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "b", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("1")])
        );
    }

    #[test]
    fn lmove_same_key_rotates() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "r", "x", "y", "z"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "r", "r", "LEFT", "RIGHT"]), &mut state),
            bulk("x")
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "r", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("y"), bulk("z"), bulk("x")])
        );
    }

    #[test]
    fn lmove_missing_source_is_nil() {
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "nope", "d", "LEFT", "RIGHT"]), &mut state()),
            Value::NullBulk
        );
    }

    #[test]
    fn lmove_emptying_source_deletes_it() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "s", "only"]), &mut state);
        dispatch(&cmd(&["LMOVE", "s", "d", "LEFT", "RIGHT"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "s"]), &mut state),
            Value::Integer(0)
        );
        assert_eq!(
            dispatch(&cmd(&["LRANGE", "d", "0", "-1"]), &mut state),
            Value::Array(vec![bulk("only")])
        );
    }

    #[test]
    fn lmove_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "str", "v"]), &mut state);
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "str", "d", "LEFT", "RIGHT"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "l", "str", "LEFT", "RIGHT"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    #[test]
    fn lmove_bad_side_is_syntax_error() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "a", "1"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMOVE", "a", "b", "UP", "DOWN"]), &mut state),
            Value::Error("ERR syntax error".to_string())
        );
    }

    // LPOS

    #[test]
    fn lpos_finds_first_match() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "c", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn lpos_rank_selects_the_match() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a", "b", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a", "RANK", "2"]), &mut state),
            Value::Integer(2)
        );
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a", "RANK", "-1"]), &mut state),
            Value::Integer(4)
        );
    }

    #[test]
    fn lpos_count_returns_an_array() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a", "b", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a", "COUNT", "2"]), &mut state),
            Value::Array(vec![Value::Integer(0), Value::Integer(2)])
        );
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a", "COUNT", "0"]), &mut state),
            Value::Array(vec![
                Value::Integer(0),
                Value::Integer(2),
                Value::Integer(4)
            ])
        );
    }

    #[test]
    fn lpos_maxlen_limits_the_scan() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b", "a"]), &mut state);
        assert_eq!(
            dispatch(
                &cmd(&["LPOS", "l", "a", "RANK", "2", "MAXLEN", "2"]),
                &mut state
            ),
            Value::NullBulk
        );
    }

    #[test]
    fn lpos_no_match_is_nil_or_empty_array() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a", "b"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "x"]), &mut state),
            Value::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "x", "COUNT", "2"]), &mut state),
            Value::Array(vec![])
        );
    }

    #[test]
    fn lpos_missing_key_is_nil_or_empty_array() {
        let mut state = state();
        assert_eq!(
            dispatch(&cmd(&["LPOS", "nope", "a"]), &mut state),
            Value::NullBulk
        );
        assert_eq!(
            dispatch(&cmd(&["LPOS", "nope", "a", "COUNT", "1"]), &mut state),
            Value::Array(vec![])
        );
    }

    #[test]
    fn lpos_rank_zero_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "l", "a", "RANK", "0"]), &mut state),
            Value::Error(
                "ERR RANK can't be zero: use 1 to start from the first match, 2 from the \
                 second ... or use negative to start from the end of the list"
                    .to_string()
            )
        );
    }

    #[test]
    fn lpos_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LPOS", "s", "a"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    // LMPOP

    #[test]
    fn lmpop_pops_from_first_non_empty() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "b", "x", "y", "z"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "2", "nokey", "b", "LEFT"]), &mut state),
            Value::Array(vec![bulk("b"), Value::Array(vec![bulk("x")])])
        );
    }

    #[test]
    fn lmpop_count_and_right_order() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "r", "1", "2", "3"]), &mut state);
        assert_eq!(
            dispatch(
                &cmd(&["LMPOP", "1", "r", "RIGHT", "COUNT", "2"]),
                &mut state
            ),
            Value::Array(vec![bulk("r"), Value::Array(vec![bulk("3"), bulk("2")])])
        );
    }

    #[test]
    fn lmpop_all_missing_is_nil_array() {
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "2", "p", "q", "LEFT"]), &mut state()),
            Value::NullArray
        );
    }

    #[test]
    fn lmpop_emptying_a_list_deletes_the_key() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "k", "only"]), &mut state);
        dispatch(&cmd(&["LMPOP", "1", "k", "LEFT"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["EXISTS", "k"]), &mut state),
            Value::Integer(0)
        );
    }

    #[test]
    fn lmpop_wrong_type_errors() {
        let mut state = state();
        dispatch(&cmd(&["SET", "s", "v"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "1", "s", "LEFT"]), &mut state),
            Value::Error(WRONG_TYPE.to_string())
        );
    }

    #[test]
    fn lmpop_bad_numkeys_and_count_errors() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "0", "l", "LEFT"]), &mut state),
            Value::Error("ERR numkeys should be greater than 0".to_string())
        );
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "1", "l", "LEFT", "COUNT", "0"]), &mut state),
            Value::Error("ERR count should be greater than 0".to_string())
        );
    }

    #[test]
    fn lmpop_bad_side_is_syntax_error() {
        let mut state = state();
        dispatch(&cmd(&["RPUSH", "l", "a"]), &mut state);
        assert_eq!(
            dispatch(&cmd(&["LMPOP", "1", "l", "UP"]), &mut state),
            Value::Error("ERR syntax error".to_string())
        );
    }
}
