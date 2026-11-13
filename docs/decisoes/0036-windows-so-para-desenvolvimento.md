# 0036 — Windows only for development; Linux in production

**Status:** proposal · **Date:** 10/11/2026

## Context

The connector was designed for Linux, which is where Hermes runs, with Windows for later. The maintainer of the project asked to run and test everything directly through `cargo`, with no container, and one of the development machines is Windows. Four pieces of the connector do not work on Windows the way the design described: the forced write of the folder, the replacement of the program in use by rename, the local socket and the file lock.

## Decision

The hub and the connector compile and pass the suite on Linux and on Windows. **Linux is the production platform. Windows is for development only.**

- What changes from one system to another stays in a single module (`arauto-platform`), with the same interface on both.
- On Windows: the durable write does not force the folder; the program in use is renamed before being replaced; the local socket is a named pipe, with the name derived from the folder, first instance and no remote client; the lock is the file lock, on a file only for that.
- The connector's folder can be changed by the variable `ARAUTO_HOME`, for two connectors on the same machine.

## Consequences

- On Windows the guarantee of durable write of the folder does not hold ([04](../04-seguranca-conexao.md), C7, rule 5), nor the one that only the owner talks to the named pipe. That is why it is not a production platform, and the documents say so.
- While development is only on Windows, the Linux-specific code is checked by the compiler and never executed. It needs to run on Linux, with the whole suite, before any real use ([15-mvp.md](../15-mvp.md), section 8).
- macOS and Linux arm64 remain for later.
