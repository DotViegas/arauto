# 0017 — Block with no script from the internet

**Status:** decided · **Date:** 09/11/2026

## Context

The shortest way to install something is to send the content of an address straight to the terminal. It is also the gesture of a scam: whoever pastes does not see what runs, and whoever controls the address chooses what runs at each request.

## Decision

The block downloads the connector to a file, in an exact version, checks the SHA-256 digest written in the block itself and only then runs it. The invite goes as an argument of the connector, never in an address.

## Consequences

- Whoever pastes sees where the program comes from and which exact version. The digest fixes that version, the same for everyone.
- The digest comes from the authenticated panel and is the same one published in the versions of the project, to be checked from outside.
- The invite does not appear in a server log or in a preview.
- All the installation logic stays in the connector. What the LLM sees in the chat does not teach how to register tool servers.
- It is still a program from the internet running on Hermes. What changes is that the version is fixed and checkable.
