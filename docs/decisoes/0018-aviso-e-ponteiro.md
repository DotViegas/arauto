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
