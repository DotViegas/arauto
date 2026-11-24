# 0024 — Sending a file by address

**Status:** decided · **Date:** 10/11/2026

## Context

A tool that sends a file from an address makes the hub fetch something chosen by someone else. The source service solves this by downloading it itself, and only from a public address.

## Decision

Keep the sending by address, as in the source: who downloads is the hub, and only from a public address. The sending by content continues to exist beside it.

## Consequences

- The hub needs to refuse an address that leads to the internal network, also after a redirect and after the resolution of the name. The rule comes with a test, in Phase 2 of the [roadmap](../12-roteiro.md).
- The alternative considered, accepting only content in the first version, would eliminate this class of attack, at the cost of splitting large files into parts over the channel.

## Note of 24/11/2026

The sending by address, left for after the first MVP, is built by [decision 0055](0055-arquivos-por-endereco-nos-dois-sentidos.md) (proposal): the hub downloads only from a public `https://` address, resolves the name and refuses any non-public IP, connects to the IP it checked, follows at most 3 redirects each checked again, cuts at 10 MB and 20 seconds, and answers every refusal with AR-715. Changed at the maintainer's request of 24/11/2026 ("Começe implementando tudo. Você pode decidir como implementar para que tudo funcione."); wording by his assistant, not reviewed word for word by him. The decision above is not reopened. Not tried on a real Hermes.
