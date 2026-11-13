# 0002 — Rust in both programs

**Status:** decided · **Date:** 09/11/2026

## Context

The source service is already written in Rust and was designed for dozens of agents on a small server.

## Decision

Hub and connector are written in Rust. The connector is a static binary, with no dependency on the system.

## Consequences

- The connector is a single file, easy to check by SHA-256 digest.
- The source code is reused, with its tests.
- Contributing requires knowing Rust.
