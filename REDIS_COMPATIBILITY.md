<!--
SPDX-FileCopyrightText: 2026 Iyad
SPDX-License-Identifier: Apache-2.0
-->

# Redis compatibility

rkv speaks RESP2 and aims to be byte-for-byte compatible with Redis for the
commands it implements. Every command marked **Yes** or **Partial** below has been
checked against Redis 8.2.2 by sending the same requests to both servers over a
raw socket and comparing the exact reply bytes.

This document tracks which commands are implemented.

## Legend

| Status  | Meaning                                                  |
| ------- | -------------------------------------------------------- |
| Yes     | Implemented and byte-for-byte compatible                 |
| Partial | Implemented, but some options or subcommands are missing |
| No      | Not implemented yet                                      |

## Connection

| Command | Status |
| ------- | ------ |
| AUTH    | Yes    |
| CLIENT  | No     |
| ECHO    | Yes    |
| HELLO   | No     |
| PING    | Yes    |
| QUIT    | Yes    |
| RESET   | Yes    |
| SELECT  | No     |

## String

| Command     | Status  |
| ----------- | ------- |
| APPEND      | Yes     |
| DECR        | Yes     |
| DECRBY      | Yes     |
| GET         | Yes     |
| GETDEL      | Yes     |
| GETEX       | Yes     |
| GETRANGE    | Yes     |
| GETSET      | Yes     |
| INCR        | Yes     |
| INCRBY      | Yes     |
| INCRBYFLOAT | Yes     |
| LCS         | Yes     |
| MGET        | Yes     |
| MSET        | Yes     |
| MSETNX      | Yes     |
| PSETEX      | Yes     |
| SET         | Yes     |
| SETEX       | Yes     |
| SETNX       | Yes     |
| SETRANGE    | Yes     |
| STRLEN      | Yes     |
| SUBSTR      | Yes     |

## Generic

| Command     | Status |
| ----------- | ------ |
| COPY        | No     |
| DEL         | Yes    |
| DUMP        | No     |
| EXISTS      | Yes    |
| EXPIRE      | Yes    |
| EXPIREAT    | No     |
| EXPIRETIME  | No     |
| KEYS        | No     |
| MIGRATE     | No     |
| MOVE        | No     |
| OBJECT      | No     |
| PERSIST     | Yes    |
| PEXPIRE     | No     |
| PEXPIREAT   | Yes    |
| PEXPIRETIME | No     |
| PTTL        | No     |
| RANDOMKEY   | No     |
| RENAME      | No     |
| RENAMENX    | No     |
| RESTORE     | No     |
| SCAN        | No     |
| SORT        | No     |
| SORT_RO     | No     |
| TOUCH       | No     |
| TTL         | Yes    |
| TYPE        | No     |
| UNLINK      | No     |
| WAIT        | No     |
| WAITAOF     | No     |

## List

| Command    | Status |
| ---------- | ------ |
| BLMOVE     | No     |
| BLMPOP     | No     |
| BLPOP      | No     |
| BRPOP      | No     |
| BRPOPLPUSH | No     |
| LINDEX     | Yes    |
| LINSERT    | Yes    |
| LLEN       | Yes    |
| LMOVE      | Yes    |
| LMPOP      | Yes    |
| LPOP       | Yes    |
| LPOS       | Yes    |
| LPUSH      | Yes    |
| LPUSHX     | Yes    |
| LRANGE     | Yes    |
| LREM       | Yes    |
| LSET       | Yes    |
| LTRIM      | Yes    |
| RPOP       | Yes    |
| RPOPLPUSH  | No     |
| RPUSH      | Yes    |
| RPUSHX     | Yes    |

## Server

| Command        | Status  |
| -------------- | ------- |
| ACL            | No      |
| BGREWRITEAOF   | Yes     |
| BGSAVE         | No      |
| COMMAND        | No      |
| CONFIG         | Partial |
| DBSIZE         | Yes     |
| DEBUG          | No      |
| FAILOVER       | No      |
| FLUSHALL       | No      |
| FLUSHDB        | No      |
| HOTKEYS        | No      |
| INFO           | No      |
| LASTSAVE       | No      |
| LATENCY        | No      |
| LOLWUT         | No      |
| MEMORY         | No      |
| MODULE         | No      |
| MONITOR        | No      |
| PSYNC          | No      |
| REPLCONF       | No      |
| REPLICAOF      | No      |
| RESTORE-ASKING | No      |
| ROLE           | No      |
| SAVE           | No      |
| SFLUSH         | No      |
| SHUTDOWN       | No      |
| SLAVEOF        | No      |
| SLOWLOG        | No      |
| SWAPDB         | No      |
| SYNC           | No      |
| TIME           | No      |
| TRIMSLOTS      | No      |

## Set

| Command     | Status |
| ----------- | ------ |
| SADD        | No     |
| SCARD       | No     |
| SDIFF       | No     |
| SDIFFSTORE  | No     |
| SINTER      | No     |
| SINTERCARD  | No     |
| SINTERSTORE | No     |
| SISMEMBER   | No     |
| SMEMBERS    | No     |
| SMISMEMBER  | No     |
| SMOVE       | No     |
| SPOP        | No     |
| SRANDMEMBER | No     |
| SREM        | No     |
| SSCAN       | No     |
| SUNION      | No     |
| SUNIONCARD  | No     |
| SUNIONSTORE | No     |

## Hash

| Command      | Status |
| ------------ | ------ |
| HDEL         | No     |
| HEXISTS      | No     |
| HEXPIRE      | No     |
| HEXPIREAT    | No     |
| HEXPIRETIME  | No     |
| HGET         | No     |
| HGETALL      | No     |
| HGETDEL      | No     |
| HGETEX       | No     |
| HINCRBY      | No     |
| HINCRBYFLOAT | No     |
| HKEYS        | No     |
| HLEN         | No     |
| HMGET        | No     |
| HMSET        | No     |
| HPERSIST     | No     |
| HPEXPIRE     | No     |
| HPEXPIREAT   | No     |
| HPEXPIRETIME | No     |
| HPTTL        | No     |
| HRANDFIELD   | No     |
| HSCAN        | No     |
| HSET         | No     |
| HSETEX       | No     |
| HSETNX       | No     |
| HSTRLEN      | No     |
| HTTL         | No     |
| HVALS        | No     |

## Sorted set

| Command          | Status |
| ---------------- | ------ |
| BZMPOP           | No     |
| BZPOPMAX         | No     |
| BZPOPMIN         | No     |
| ZADD             | No     |
| ZCARD            | No     |
| ZCOUNT           | No     |
| ZDIFF            | No     |
| ZDIFFSTORE       | No     |
| ZINCRBY          | No     |
| ZINTER           | No     |
| ZINTERCARD       | No     |
| ZINTERSTORE      | No     |
| ZLEXCOUNT        | No     |
| ZMPOP            | No     |
| ZMSCORE          | No     |
| ZPOPMAX          | No     |
| ZPOPMIN          | No     |
| ZRANDMEMBER      | No     |
| ZRANGE           | No     |
| ZRANGEBYLEX      | No     |
| ZRANGEBYSCORE    | No     |
| ZRANGESTORE      | No     |
| ZRANK            | No     |
| ZREM             | No     |
| ZREMRANGEBYLEX   | No     |
| ZREMRANGEBYRANK  | No     |
| ZREMRANGEBYSCORE | No     |
| ZREVRANGE        | No     |
| ZREVRANGEBYLEX   | No     |
| ZREVRANGEBYSCORE | No     |
| ZREVRANK         | No     |
| ZSCAN            | No     |
| ZSCORE           | No     |
| ZUNION           | No     |
| ZUNIONSTORE      | No     |

## Pub/Sub

| Command      | Status |
| ------------ | ------ |
| PSUBSCRIBE   | No     |
| PUBLISH      | No     |
| PUBSUB       | No     |
| PUNSUBSCRIBE | No     |
| SPUBLISH     | No     |
| SSUBSCRIBE   | No     |
| SUBSCRIBE    | No     |
| SUNSUBSCRIBE | No     |
| UNSUBSCRIBE  | No     |

## Transactions

| Command | Status |
| ------- | ------ |
| DISCARD | No     |
| EXEC    | No     |
| MULTI   | No     |
| UNWATCH | No     |
| WATCH   | No     |

## Stream

| Command     | Status |
| ----------- | ------ |
| XACK        | No     |
| XACKDEL     | No     |
| XADD        | No     |
| XAUTOCLAIM  | No     |
| XCFGSET     | No     |
| XCLAIM      | No     |
| XDEL        | No     |
| XDELEX      | No     |
| XGROUP      | No     |
| XIDMPRECORD | No     |
| XINFO       | No     |
| XLEN        | No     |
| XNACK       | No     |
| XPENDING    | No     |
| XRANGE      | No     |
| XREAD       | No     |
| XREADGROUP  | No     |
| XREVRANGE   | No     |
| XSETID      | No     |
| XTRIM       | No     |

## Scripting

| Command    | Status |
| ---------- | ------ |
| EVAL       | No     |
| EVALSHA    | No     |
| EVALSHA_RO | No     |
| EVAL_RO    | No     |
| FCALL      | No     |
| FCALL_RO   | No     |
| FUNCTION   | No     |
| SCRIPT     | No     |

## Geo

| Command              | Status |
| -------------------- | ------ |
| GEOADD               | No     |
| GEODIST              | No     |
| GEOHASH              | No     |
| GEOPOS               | No     |
| GEORADIUS            | No     |
| GEORADIUSBYMEMBER    | No     |
| GEORADIUSBYMEMBER_RO | No     |
| GEORADIUS_RO         | No     |
| GEOSEARCH            | No     |
| GEOSEARCHSTORE       | No     |

## Bitmap

| Command     | Status |
| ----------- | ------ |
| BITCOUNT    | No     |
| BITFIELD    | No     |
| BITFIELD_RO | No     |
| BITOP       | No     |
| BITPOS      | No     |
| GETBIT      | No     |
| SETBIT      | No     |

## HyperLogLog

| Command    | Status |
| ---------- | ------ |
| PFADD      | No     |
| PFCOUNT    | No     |
| PFDEBUG    | No     |
| PFMERGE    | No     |
| PFSELFTEST | No     |

## Cluster

| Command   | Status |
| --------- | ------ |
| ASKING    | No     |
| CLUSTER   | No     |
| READONLY  | No     |
| READWRITE | No     |

## Summary

**Total Commands**: 262

- **Fully Implemented**: 51 commands
- **Partially Implemented**: 1 command
- **Not Implemented**: 210 commands

**Implementation Coverage**: ~19%

This compatibility matrix is updated as new commands are implemented in rkv.
