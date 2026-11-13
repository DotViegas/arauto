# 0030 — Outdated connector: the hub checks the credential before refusing

**Status:** decided · **Date:** 10/11/2026

## Context

A connector below the minimum version is refused by the hub (AR-401). In the current design it stops using the channel and no longer presents the credential. With that, an outdated Hermes counts as turned off: whoever has a copy of the files and uses a current connector uses the hub until the admin reconnects that Hermes, and the use may not show up as a copy ([04-seguranca-conexao.md](../04-seguranca-conexao.md), section 6, item 3).

## Decision

The hub checks the credential of an outdated connector before refusing the version. If the credential presented had already been swapped, the use of a copy shows up: alert and quarantine, as in any reuse. The refusal of the version remains.

## Consequences

- The order of the messages is in [14-protocolo-do-canal.md](../14-protocolo-do-canal.md), section 5.3, as a proposal: the "presentation without swap". The hub answers the old connector, receives what it presents, checks and only then refuses the version, without ever issuing a credential.
- For this, besides the first message, the messages of this presentation are frozen.
- The outdated connector presents itself at the start of the leader and, after that, once every 24 hours.
- Until the [04](../04-seguranca-conexao.md) is updated, with approval, its section 6 keeps describing the old limit. C8 also still says that the old connector "receives AR-401 and nothing else": the point is in section 12 of the [14](../14-protocolo-do-canal.md).
- On 11/11/2026 the hub already does the presentation without swap for the connector that is too old, with a test: it checks the credential, applies the last three lines of the judgment table and only then sends AR-401. The connector's side is still missing.
