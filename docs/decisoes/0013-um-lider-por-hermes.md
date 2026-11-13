# 0013 — One leader per Hermes

**Status:** proposal, to be validated in the prototype · **Date:** 09/11/2026

## Context

Hermes may start more than one copy of the connector. With a credential that is swapped at each connection, two copies fighting over the channel would lock each other out, and the hub would see double connections from a healthy Hermes.

## Decision

The first copy to take a file lock becomes the leader: only it opens the channel and swaps the credential. The others are relays: they send the calls to the leader through a local socket.

## Consequences

- For the hub there is exactly one connection per device.
- A double connection means a copy again.
- If the leader dies, the system releases the lock and another copy takes over.
- Another local process of the same user manages to use the socket: it is the limit of quick mode.
- The connector's folder cannot be shared between two Hermes, and the file system needs to support locks.
