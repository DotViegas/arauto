# 0032 — Conversation between Hermes in the first MVP: notes with a list

**Status:** decided · **Date:** 10/11/2026

## Context

The conversation between Hermes was in the vision, with no design and for after the first version. The maintainer of the project defined it as the main function of Arauto.

## Decision

The conversation between Hermes goes into the first MVP, in the format of **notes with a list**: a Hermes sends a note to another through the hub; the note only passes if the peer is on the list the admin marks; whoever receives it is notified and decides what to do. The note is data, never an order. There are ceilings on quantity, and everything is recorded. The specification is in [13-conversa-entre-hermes.md](../13-conversa-entre-hermes.md).

## Consequences

- The separation between command and data holds ([0005](0005-comando-e-dado-sao-separados.md)): a Hermes does not give orders to another through Arauto. The alternative, a note with the authority of an order, was discarded.
- The hub still only isolates ([0004](0004-o-hub-so-isola.md)): it does not read the content, it does not choose who answers. The ceilings come in as isolation, like the send ceiling ([0020](0020-teto-de-envio.md)).
- Rooms with several Hermes and the observer of a pair are left for later.
- The ceilings are counted per link and per recipient, never per thread: opening a new thread resets none of them. The value is fixed in the first version and is calibrated in the MVP.
- There is no end-to-end proof of sender: whoever controls the hub writes as any Hermes. It is stated as a limit.
- The notice of a note follows the pointer rule ([0018](0018-aviso-e-ponteiro.md)). The wording of the fixed sentence depends on approval.
