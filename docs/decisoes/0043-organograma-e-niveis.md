# 0043 — The org chart of the Hermes: levels, places and links on a map (stage A)

**Status:** proposal · **Date:** 14/11/2026. Stage A is built in the hub; **it was tried only in a desktop Chrome without a window.** Stage B is **not designed** and is a pending decision of the maintainer.

## Asked by the maintainer (14/11/2026)

The purpose of the project is many Hermes working together, each aware of its place in a hierarchy. He asked for a tab where he creates two or more levels and says which Hermes may talk to which, seen as a map: the Hermes are moved and positioned, and arrows mark who has contact with whom; the higher a Hermes, the nearer the top. The hierarchy is fully customizable. Only Hermes are on the chart, no people. He approved a clickable prototype and asked for it to be built.

## Context

Decision [0005](0005-comando-e-dado-sao-separados.md) (decided) and [0032](0032-conversa-entre-hermes-no-mvp.md) say that Arauto is never a command channel and that there is no note with authority. The maintainer also stated that an order from above is to be accepted automatically. The two cannot both hold, so the work is cut in two stages.

## Decision, stage A (as built, proposal)

- **Data.** Two tables, `org_level` (ordered, named in the Hermes-name format, at most 12) and `org_member` (one row per placed Hermes: level, horizontal place from 0 to 4000, an optional role text of at most 280 characters, for the admin's own reading). A Hermes is in one level only. A live Hermes with no row is "not placed yet". They are described in [06](../06-modelo-de-dados.md), section 11.
- **The tab.** A fourth tab, "Org chart": a map (bands, cards, arrows) and a list view with the same data. The admin edits a **draft** in the page, with undo; nothing is written until **Publish**.
- **Publish** sends the whole wanted state in one request and applies it in **one transaction**, after a summary of the changes. It carries the version of the chart the draft started from; if the chart changed meanwhile (another publish, a link changed by the command, a Hermes created or removed) the hub answers "stale", writes nothing and the page keeps the draft. The links are changed by the same functions as `arauto-hub admin link add|remove`: same events, same discarded notices, and the same signal to the hub so the connections of both Hermes read their state again.
- **The kind of an arrow** (from a higher level to a lower one: instruction; lower to higher: request; same level: note) is computed by the panel from the levels and **only shown to the admin**. It is not stored and gives no permission. Here "request" is not a connection request.
- **Nothing reaches the channel.** No notice, tool, tool description or connector changes; no Hermes is told its level, its role or the kind of any arrow. A note is still data with no authority. The role text and the level names never reach the channel, a notice, a tool answer or the log, and the audit holds neither (only ids, flags and counts).
- **On the screen** a sentence says that in this version the levels, the roles and the kinds of arrow are only shown there.
- **Card.** It shows the numbers of the Hermes and whether it may send or only read, from the existing access data, and points to the Accesses tab. It does not edit them. "Can be woken" is shown, not edited; the command stays.
  **Changed on 15/11/2026 ([0045](0045-a-marca-da-ligacao-basta-para-acordar.md)):** the drawer no longer shows "Can be woken" nor the command; it shows "Woken by WhatsApp" in its numbers section, and the window of an arrow says that the wake mark is enough.
  **Changed on 16/11/2026 by decision 0049 ([0049](0049-acordar-pelo-whatsapp-so-pelo-verbo.md)):** the drawer no longer shows "Woken by WhatsApp" either; the switch no longer exists. The window of an arrow is unchanged.
- **Command twin.** `arauto-hub admin org show` and `arauto-hub admin org publish FILE --version HEX`, by the same functions.
- **Audit.** `org_level.changed`, `org_member.changed` and `org_chart.published` are ordinary events, not security events; the link events inside a publish are the existing security ones.
- **The page's script.** A second file, `org.js`, loaded only by this tab, writes no markup and calls nothing on the network; the live refresh of the tab is paused while a draft exists ([0040](0040-painel-dentro-do-hub.md), change of 14/11/2026).
- **Out of stage A:** people on the chart; a Hermes in more than one level; editing accesses from the card; any permission derived from a level or a kind.

## Stage B: pending decision of the maintainer

He stated that an order from above is to be accepted automatically. That would make a note from a higher level an order, which 0005 and 0032 forbid. Doing it needs, from him:

1. decision 0005 **reopened by him**; nothing here reopens it;
2. new fixed sentences (the notice of a note from a higher level, the descriptions of the tools), approved by him;
3. the levels sent to the connector;
4. a decision on whether a kind of arrow becomes a permission.

Not designed here.

## What is not guaranteed

- In stage A no Hermes knows its level, so the chart changes no behaviour of any Hermes.
- In stage B, an instruction accepted automatically would be obeyed whoever forged it: an invaded hub, or whoever holds the database and the key, writes a note as any Hermes ([13](../13-conversa-entre-hermes.md), section 9, limit 1), and a Hermes on top, deceived by a text, passes the order down.
- A level is a label the admin gives, not a proof.
- The map was tried on 15/11/2026 in Chrome 157 for Windows, without a window and driven by a script with the mouse, against a test hub with six Hermes and the content policy in force: adding levels, moving a card to another level, pulling four links, the window of an arrow, the drawer of a card, the list view, Publish and the refresh after it. That trial found and fixed three defects (an empty area above the map, a tall card cut by its band, Escape not closing the window of an arrow pulled with the mouse). Not tried: touch screens, other browsers, a narrow screen beyond one look at the list view, a hub behind a proxy. The size ceiling of a publish (1 MiB, estimated for about 200 Hermes) is not proved. The check of control characters in the database beyond ASCII depends on the locale; the check in the program is the authority.
- While a publish runs it holds the rows of every live Hermes of the organization until its durable commit, so a link command, a note sent by a Hermes or a device call that needs one of those rows waits for it. A wait longer than the database's lock timeout (5 seconds) ends in an error and the caller tries again. The publish reads the chart once and writes without asking again, to keep that time short, but it grows with the number of links changed and with the speed of the disk; it was not measured on a large organization.
- The draft is lost when the panel session ends before Publish.
- Publishing is not confirmed by the passkey again ([0019](0019-painel-com-passkey.md) is not built), so a stolen panel session can also change links.
- Open for the maintainer: the colour of the "request" arrow (amber dashed in the prototype, while the brand keeps amber for state), and the drawer below the map against his rule that configuration is always in a modal.

## Consequences

The documents [06](../06-modelo-de-dados.md), [07](../07-telas.md), [08](../08-permissoes.md), [13](../13-conversa-entre-hermes.md), [15](../15-mvp.md), the [glossary](../glossario.md) and decision [0040](0040-painel-dentro-do-hub.md) say it.
