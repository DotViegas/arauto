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

## Change of 14/11/2026

The command still comes only from the person, in the command channel. What changes is how: the panel now writes a short connection sentence, in the language of the panel, that points to an instruction page served by the hub, and the command block becomes the fallback. The page is data that explains; it says it is not an order, and the order is the operator's own sentence. The maintainer asked for the link and the page and approved the design on 14/11/2026. Decision [0042](0042-frase-de-conexao-e-pagina-de-instrucoes.md). Nothing else in this decision is reopened. The design was not tried on a real Hermes yet.
