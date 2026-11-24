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

## Change of 16/11/2026 (reopened by the maintainer)

The notice of a note may carry one more field of strict format, the boolean `follow`, and the connector then wakes Hermes with a second fixed sentence compiled into it ([0050](0050-etapa-b-marca-de-seguir.md)). The hub still sends no text, but with this boolean it chooses which of two fixed sentences the LLM reads: "the hub cannot change a text the LLM reads" holds with this one exception. An invaded hub can make the connector use the second sentence for any note notice. The descriptions of the tools do not change.

## Change of 24/11/2026 (decision 0054)

Changed at the maintainer's request of 24/11/2026 ("Começe implementando tudo. Você pode decidir como implementar para que tudo funcione."). Wording by his assistant, not reviewed word for word by him.

The notice of a WhatsApp chat may carry one more field of strict format, `session`: the key of the Hermes session of that contact, `arauto-wa1-`, `arauto-wg1-` or `arauto-wh1-` followed by 64 lowercase hexadecimal characters ([0054](0054-uma-sessao-do-hermes-por-contato.md)). It is not text and never enters the sentence. It carries no telephone, no name and no message text. The sentence that "the hub cannot change a text the LLM reads" still holds for the sentence and the tool descriptions, but the key chooses which earlier turns of the conversation the LLM reads: an invaded hub can make a run load the history of another contact of the same Hermes. Nothing else in this decision is reopened.
