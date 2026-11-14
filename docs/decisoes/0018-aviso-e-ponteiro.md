# 0018 — The notice is a pointer, with a fixed sentence in the connector

**Status:** decided · **Date:** 09/11/2026

## Context

To wake a Hermes it is necessary to deliver a text to it. If that text came from the hub, an invaded hub would dictate what the LLM of each installation reads. If it carried the content of the message, a contact would write straight into the start of a run.

## Decision

The hub sends only a pointer: number, chat and count. The sentence that wakes Hermes is fixed and is compiled into the connector, as are the descriptions of the tools.

## Consequences

- The content only arrives as a tool result, marked as third-party.
- The hub cannot change a text the LLM reads.
- The notice carries no message text, no contact name and no group subject.

## Change of 14/11/2026

Principle 6 of the `CLAUDE.md` gains a sentence: the hub may serve a fixed page compiled into it, whose only fields have a strict format (host, Hermes name, UTC time, version, file name). The notice stays a pointer, and the fixed sentence and the descriptions of the tools stay in the connector. The page is not a notice and holds no free text from a device or from WhatsApp. The maintainer asked for the link and the page and approved the design on 14/11/2026. Decision [0042](0042-frase-de-conexao-e-pagina-de-instrucoes.md). Nothing else in this decision is reopened. The design was not tried on a real Hermes yet.
