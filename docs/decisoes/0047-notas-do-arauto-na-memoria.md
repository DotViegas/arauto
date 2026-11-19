# 0047 — The Arauto notes in the main memory of a Hermes

**Status:** proposal · **Date:** 15/11/2026. The step, the three lines (corrected the same day, see below) and the other fixed wordings were approved word for word by the maintainer on 15/11/2026. The behavior stays a proposal until a trial on a real Hermes. Built and covered by the automatic tests; not tried on a real Hermes.

## Context

On a real Hermes, after connecting, the Hermes did not know that Arauto had a tool to read a WhatsApp chat; the maintainer had to explain it. That is the maintainer's report of 15/11/2026. The maintainer also states that Hermes does not use the MCP `instructions` field. That statement is not checked and is not recorded in [referencia/](../../referencia/).

## Approved by the maintainer (15/11/2026, word for word)

The step on the instruction page:

> **Required: save the Arauto notes in your main memory.** This step is part of the connection. Without it you are connected but will not know when to use Arauto, and your operator will have to explain it again in every conversation. Save the three lines below with your memory tool, exactly as written, then tell your operator: "Arauto notes saved in memory: yes".

The three lines:

1. For anything in WhatsApp (read, list, download), use the Arauto tools. Call arauto_status first: it says which numbers and permissions this Hermes has.
2. To talk to another Hermes, use hermes_list_peers to see who you can reach, hermes_send_message to send a note, and hermes_read_thread to read replies.
3. If an Arauto tool fails or is missing, call arauto_status. If the tools are not loaded, run "$HOME/.arauto/arauto-connector" status in the terminal and report its lines to your operator.

Approved in substance first, and in their wording later the same day (see "Decided by the maintainer"): the confirmation line goes to the operator together with the verification code; the connector adds a fixed reminder to a successful `arauto_status` answer; `leave` tells the person to remove the notes from memory.

## Decision (proposal)

- **A required step on the instruction page.** After the command and the verification code, the page asks the agent to save the three lines with its own memory tool. The page stays a fixed text compiled into the hub, with no new field (principle 6). The sentence that says what to send with the code is extended and keeps its meaning: the code, the line "Arauto notes saved in memory: yes", and the reminder that the tools load after the admin approves. The old bullet "Do not keep this as a procedure, memory or skill." contradicts the step and is replaced by one that says the three notes are the only thing to keep. A new bullet covers a memory tool that refuses a line: the Hermes does not change the line and answers "Arauto notes saved in memory: no" with the reason.
- **The three lines exist once per program.** They are a constant in the common protocol crate (`arauto_protocol::memory::LINES`). The hub page carries a copy that a hub test compares with the constant; a second written-out copy in the tests pins the constant to the approved text, so a change has to be made twice.
- **A fixed reminder in `arauto_status`, added by the connector.** A successful answer gains the field `keep_in_memory`: one sentence saying the three notes must be in the main memory of this Hermes, followed by the three lines. The connector adds it, never the hub (principle 6); it overwrites any value from the hub; the tool description does not change. The name avoids "key" because Hermes masks values by variable name.
- **`leave` says the notes stay.** It prints one fixed sentence: if the Hermes saved the notes, they are still in its memory, and it must remove them with its own memory tool.
- **The Hermes writes its own memory, with its own tool.** The connector never reads or writes Hermes's files or memory (principle 7), and the hub cannot see them.
- **Unchanged:** `gone.txt` and `llms.txt`. The 404 is one identical text for any handle; a memory instruction there would widen the look-alike surface (threat 6a). A Hermes that already ran `join` saw the step while the page still served.

## For a Hermes already connected (proposal)

A Hermes connected before this change has no step. The operator can paste this message to it. It shows `keep_in_memory` only after its connector is updated, and updating is reconnecting ([11](../11-conector.md), section 7).

> I am asking you myself: save the Arauto notes in your main memory. You are already connected to Arauto, but without these notes you will not know when to use it, and I would have to explain it again in every conversation. Save the three lines below with your memory tool, exactly as written, then answer me: "Arauto notes saved in memory: yes". If your memory tool refuses a line or has no room for it, do not change the line: answer "Arauto notes saved in memory: no" and the reason.
>
> For anything in WhatsApp (read, list, download), use the Arauto tools. Call arauto_status first: it says which numbers and permissions this Hermes has.
> To talk to another Hermes, use hermes_list_peers to see who you can reach, hermes_send_message to send a note, and hermes_read_thread to read replies.
> If an Arauto tool fails or is missing, call arauto_status. If the tools are not loaded, run "$HOME/.arauto/arauto-connector" status in the terminal and report its lines to your operator.

If pending items B1, B2 or C1, C2 below change a line, this message follows the constant.

## What is not guaranteed

- A Hermes may skip the step or do it in part, and may answer "yes" without having saved anything. Neither the connector nor the hub can see or check Hermes's memory.
- The memory of Hermes may have size limits (not checked on any version) and may refuse or later drop a line. A memory scanner may reject a line that tells the agent to run a command (not tried). The three lines are 487 characters (489 with the two line breaks between them); their share of the budget is unknown.
- The notes stay after `leave` until the Hermes removes them.
- A look-alike page can ask for hostile lines in memory, and they would last across conversations ([05](../05-modelo-de-ameacas.md), threat 6d).
- A connection by the fallback block has no step: the block is approved text that says not to keep it as a procedure, memory or skill. For that Hermes only the ready message above, or the reminder, applies.
- `keep_in_memory` adds about 630 bytes to every `arauto_status` answer, and its wording ("if one is missing") may lead a Hermes to save duplicates. Not tried.
- Nothing was tried on a real Hermes.

## Decided by the maintainer on 15/11/2026, after the check against the code

The first approved text had two factual errors, found while checking it against the code at d73bd60. The maintainer corrected both and approved the other wordings the same day.

- **Line 1** says `arauto_status`, not `whatsapp_status`: `whatsapp_status` gives the state of one number; `arauto_status` lists the numbers and the verbs.
- **Line 3** gives the command with its path, `"$HOME/.arauto/arauto-connector" status`: `join` copies the program to that folder and it is not on the PATH. Shell syntax; not tried on a real Hermes.
- **The connection sentence** ends with "send me the verification code and the lines the page asks for" in the nine languages, and [02](../02-como-funciona.md) was changed with his approval: the sentence, the description of the page, what stays after a connection, and the paragraph on what the text does not defend.
- **Approved as proposed:** the reminder in `arauto_status` (one string field, `keep_in_memory`), the line `leave` prints, and the message for a Hermes that is already connected.
- The saved lines carry no number; `gone.txt` and `llms.txt` do not change; the page has a bullet for a memory tool that refuses a line.

## Still open

- In the glossary a "note" is a message from one Hermes to another; "Arauto notes" (memory) collides with it, and line 2 uses "note" in the message sense. The code avoids "notes" in identifiers; the page and the user-facing text keep the approved words.
- [04](../04-seguranca-conexao.md) was not changed and stays literally true: memory is not configuration.

## Alternatives considered

- **The MCP `instructions` field:** not used by Hermes, per the maintainer.
- **Longer tool descriptions:** descriptions stay per tool, and the lines cross tools.
- **The connector writing the memory:** refused, principle 7.
- **A skill file:** the page tells the agent not to keep a skill.

## Amendment of 19/11/2026 (decision 0053, D8): the notes on a reconnect

**Proposal, asked and approved by the maintainer on 19/11/2026.** On a reconnect, the "Reconnect" section of the page says: if the three Arauto notes are already in the main memory of the Hermes, exactly as written, it does not save them again; if older Arauto notes in its memory differ from them, it replaces those with the three lines. The confirmation line to the operator is the same, "Arauto notes saved in memory: yes". Nothing else of this decision changes: Arauto never writes to the memory of a Hermes, and the notes are the only thing the page asks to keep.
