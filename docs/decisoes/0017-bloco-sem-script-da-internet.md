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

## Change of 14/11/2026

The block is no longer what the panel shows first: it stays behind "Show the command instead" and with `--command` in the admin command, unchanged. The rule of this decision is kept in the steps of the instruction page: download to a file, an exact version, check the digest, and only then run; no script from the internet; the invite as an argument, never in an address. The maintainer asked for the link and the page and approved the design on 14/11/2026. Decision [0042](0042-frase-de-conexao-e-pagina-de-instrucoes.md). Nothing else in this decision is reopened. The design was not tried on a real Hermes yet.
