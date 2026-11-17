# 0044 — Numbers in the panel, on the engine, with a placeholder for a native one

**Status:** proposal · **Date:** 15/11/2026. It is built in the hub and covered by automatic tests against a fake engine. **It was not tried in a real browser, nor from the panel against a real WAHA.**

## Asked by the maintainer (15/11/2026)

To do everything about the numbers in the panel, on WAHA for now, without leaving Arauto to connect a WhatsApp. Two buttons on the WhatsApps tab: "Connect WhatsApp (WAHA)" and "Connect WhatsApp (native · in test)". In Portuguese: "Conectar WhatsApp (WAHA)" and "Conectar WhatsApp (nativo - Em teste)".

## Context

- [0003](0003-hub-e-motor-separados.md) (decided): hub and engine stay two processes.
- [0014](0014-traga-o-seu-motor.md) (proposal): Arauto does not embed or redistribute the engine; whoever installs brings their own.
- [0022](0022-motor-sem-porta-publica.md) (decided): the engine stays on the internal network and the hub watches its configuration.
- [0019](0019-painel-com-passkey.md) (decided): the passkey on every action. Not built; [0040](0040-painel-dentro-do-hub.md) records it as pending decision 2.
- `CLAUDE.md`: never embed the engine in the delivery.
- Until now, numbers were added, paired, restarted, disconnected and removed only by the command `arauto-hub admin number ...`; the tab only listed them. The admin had to use a terminal or WAHA's own screen to pair.

## Decision (proposal)

- **Routes and windows.** Everything is under `/whatsapps/`. Each step is a window over the list, as the access editor is: add (alias, number, the acceptance for the first number; the box "command" was removed on 17/11/2026, [0051](0051-quem-da-ordens-a-um-hermes-no-whatsapp.md)), the choice of QR or code, the wait, the decision after a swapped number, disconnect, remove and the native window. Restart is a one-click form on the line. The actions on each line follow the state of the number.
- **Same functions as the command.** Every write goes through the admin functions the command uses (authorization, audit rows, ceilings, state machine), with the actor "panel". Nothing is duplicated. Only a full panel session reaches these routes; every write is a `POST` with the panel's `Origin` check.
- **A read changes nothing.** The pairing window is polled with `GET`s that only read what the hub holds in memory. Starting, cancelling and the decisions are `POST`s.
- **The pairing runs in a task owned by the hub process.** It holds the QR or the code in memory only and the page reads it. The panel never holds a request open for minutes.
- **Ceilings.** One pairing per number, three at a time per organization.
- **Closing the window does not cancel.** The pairing goes on until its deadline (3 minutes) and the line offers "Continue pairing" to the session that started it. "Cancel the pairing" stops the engine session and puts the number back to unpaired. Another panel session never sees the QR or the code.
- **The QR** is drawn by the hub as inline SVG from the encoder that already existed (`admin/qr.rs`), with no new dependency, no inline script and no external request. The QR and the code are short-lived secrets: `no-store` on every answer, never logged, never audited, never written to the database, zeroized when the pairing ends.
- **The window's script.** A third file, `pair.js` (see the 15/11/2026 change in 0040), loaded only by the wait window. Without script, the window reloads every 3 seconds.
- **An end per number.** The end of a pairing (expiry, failure, cancel) unpairs only its own number, and only while the deadline it was started with still holds. Before, the end of one pairing unpaired every pairing of the organization started before it. An engine error in the middle of a pairing now also stops the engine session and puts the number back. These fixes are in the admin function, so the command gets them too.
- **The acceptance sentence.** The panel asks for it for the first number the organization ever had, and records `risk_accepted` on `number.added`. The command does not ask for it.
- **Disconnect and Remove** ask for the alias typed, and Remove for an explicit choice (keep the history in the engine, or erase it). They do not ask for the passkey again: the strictest check that needs no new mechanism.
- **No engine, or no answer.** The window says so plainly and points to `arauto-hub admin engine ...`. The engine key is a long-lived secret and is never typed on a screen.

## The native button is a placeholder

"Connect WhatsApp (native · in test)" opens a window with a fixed text and no form. It says that an engine built into Arauto is being studied, that it is not part of this version, and that for now numbers connect through WAHA. It does nothing else, and it is marked as in test. The wording "being studied" is on purpose: nothing in the repository tries such an engine.

## What is NOT decided

A built-in engine. It would need the maintainer to reopen 0003 and 0014, and the rule "never embed the engine", with its license and its risk. Nothing in this delivery builds or tries one.

## What is not guaranteed

- **Not tried:** no real browser and no real WAHA from the panel. The automatic tests use a fake engine. Believed but not tried: inline SVG with presentation attributes under the content policy, `<noscript>` with a refresh in the head, and how a browser's back/forward cache treats `no-store` pages.
- **Copies outside the hub's control:** the HTTP stack, a proxy and the browser keep copies of the page that the hub cannot wipe. Without script, every reload sends the current QR again. "Shown once" is read as: in one window of one panel session, for the life of the pairing; with script each value travels once per window.
- **A stolen panel session** (12 hours, or 30 minutes without use) can start a pairing, scan it with another phone and press "Keep this one", which puts the Hermes with access on that account; it can also disconnect or remove numbers. The passkey on every action is not built.
- **Lost or stuck pairings.** After a hub restart a running pairing is lost; the row stays "pairing" for up to about 8 minutes and the engine session keeps waiting for a QR; a late scan connects WAHA while the hub says not paired, until "Pair again". A cancel that does not end within 10 seconds is aborted and the row waits for the supervision.
- **The command and the panel pairing the same number** at the same time stop each other's engine session. There is no lock across the two processes.
- **Engine failures on disconnect or remove** leave the engine session as it was, and nothing retries it. The command's sentence "the hub tries again at its next check" is not backed by the supervision for these two.
- **No sentence for a limit on linked devices:** the engine does not report it, and WAHA's behaviour when WhatsApp refuses one more is not known.
- **Labels and validity:** the WhatsApp menu names in each language were not checked on a phone, and the validity of the code on WhatsApp's side is unknown; the window shows the hub's deadline.
- **PASSKEY_REQUIRED** from WAHA during a pairing is not explained in the window.
- **Translations:** the eight were written by a model, with no native review.
- **Not audited:** the end of a pairing, as today in the command.

## Pending decisions, for the maintainer

1. **The two labels.** The glossary says "connect" is for Hermes and "pair" for numbers. The two buttons keep "Connect" by his request, and everything inside says "pair". He confirms the exception or chooses other labels.
2. **The wording of the native window** ("being studied"), and whether a built-in engine is ever to be tried (it would reopen 0003, 0014 and the rule "never embed the engine").
3. **The passkey on every action** (0019, 0040 pending decision 2). A step-up ceremony bound to the action is a new mechanism.
4. **The threat model** ([05](../05-modelo-de-ameacas.md)) should get the line about a stolen panel session, above. It is not changed by this delivery.
5. **The acceptance sentence in the command** (for example a `--accept-risk` flag).
6. **"Mark as command" after the add**: moot since 17/11/2026: the mark was removed ([0051](0051-quem-da-ordens-a-um-hermes-no-whatsapp.md)).
7. **Auditing the end of a pairing** (for example a cancel by an admin).
8. **The command's text** about trying again at the next check: correct it, or extend the supervision to stop the session of a disconnected number and retry a failed logout.
9. **Orphan pairings at start**: stopping them would be a new start-up step, not designed here.
10. **Mismatch outside this scope:** 0038 says a pairing with an account the engine does not name goes to "number swapped", but the state machine keeps it in "pairing" until the account is named.

## Alternatives considered

- **A request held open for the whole pairing.** A proxy cuts it, and closing the tab would kill the pairing half way.
- **Short steps driven by the page, each `GET` calling the engine.** A read would drive the engine, against the rule that a read does not change state, and two windows would mean two reads of the engine.
- **A websocket or server-sent events.** 0040 forbids a permanent connection.
- **Storing the QR** (in the database or in a cookie). It is a short-lived secret and is kept in memory only.
- **Asking for a recent sign-in before a destructive action.** It sends the admin through the sign-in and loses the window, and it is a policy nobody chose.
