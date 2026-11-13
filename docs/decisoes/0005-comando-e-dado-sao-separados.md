# 0005 — Command and data are separate

**Status:** decided · **Date:** 09/11/2026

## Context

An agent that reads messages from third parties can be instructed by them. If the same medium serves to give orders and to receive content, authority comes to depend on a label inside a text.

## Decision

An order comes from Hermes's command channel, from the operator, and that channel stays outside Arauto. Everything that comes in through the numbers of the hub is data, never an order.

## Consequences

- Arauto is never a command channel.
- The notice that wakes Hermes says it is data and carries no third-party content ([0018](0018-aviso-e-ponteiro.md)).
- The connection block is only valid when it arrives through the command channel, and says so.
- An order written in a group of a number of the hub is not valid as an order: it arrives only as reading.
- Who upholds this, in the end, is Hermes. The hub never delivers third-party text in the position of an instruction, but does not guarantee that the LLM will never obey a message it read.
