# 0035 — Websocket stack and message format

**Status:** proposal · **Date:** 10/11/2026 · revised on the same day, after the review of the design, and
on 11/11/2026, with the code of the channel in the hub

## Context

The channel is a websocket with Noise inside ([0012](0012-canal-cifrado-por-dentro.md)). What was missing was choosing the pieces and the format of what passes through it, with performance in view.

## Decision

- Server: `axum` over `hyper`, on `tokio` with several cores. The loop that accepts connections belongs to the project, to have a deadline for headers, a ceiling on connections and cancellation per connection. Websocket only with binary messages, of up to 65,535 bytes, and with no compression. In the hub, the loop and the routes belong to the project, over the HTTP/1 of `hyper`; from `axum` come the websocket and the HTTP types.
- Each connection has one task that reads and another that writes. Two output queues, counted in frames: control and data. Whoever produces data waits for room; the connection is only closed when a frame goes 30 seconds without leaving or the control queue fills up. The `closing` goes through the data queue, after what was already in it; only the control frames go ahead.
- Noise is used in the mode in which the counter of each direction is passed by hand. Only the task that writes encrypts, after choosing the queue. The refusal of a repeated or out-of-order frame is code of the project, with a test.
- Messages in JSON, inside Noise, cut into pieces of up to 60,000 bytes with a stream number, with limits per phase ([14-protocolo-do-canal.md](../14-protocolo-do-canal.md)).
- Database through `sqlx`, with two fixed pools of connections: the one of the connectors and a small one, only for the sweep, so that revoking takes effect in under 5 seconds even with the first one busy. Authorization on each call by a query with an index, with no cache.
- Between processes, the admin command notifies the hub by `NOTIFY`. Since that notification can be lost, the hub also sweeps every 2 seconds.

## Consequences

- JSON costs more processor and more bytes than a binary format. It was chosen because the calls and answers of the tools are already JSON, because an unknown type can be ignored and because the cost that weighs is that of the cipher and that of the database. If the load test shows the opposite, the format is changed in a new version of the protocol.
- With no cache of accesses, removing an access takes effect on the next call without depending on invalidation. The cost is one query per call.
- The goal of a few thousand connectors per hub only becomes data when the load test of the repository runs. On 11/11/2026 only the smaller version ran, with 500 connectors in the same process, in a development build: the numbers are in [15-mvp.md](../15-mvp.md), section 3.
- The `keep_alive` of `hyper` turned off breaks the upgrade to websocket. That is why the HTTP connection stays open between requests, and the deadline of 5 seconds for the headers also holds between them.
- Alternatives considered: a leaner websocket library, which gains in the reading of the frames and not in what weighs here; and messages in a binary format, for the reasons above.
