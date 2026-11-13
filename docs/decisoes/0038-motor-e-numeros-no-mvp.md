# 0038 — The engine and the numbers in the first MVP

**Status:** proposal · **Date:** 11/11/2026, with the code of the engine and of the numbers in the
hub (M3, first part)

## Context

The [15-mvp.md](../15-mvp.md) asks, in M3, for the engine interface, the WAHA adapter, the fake
engine, the event inbox and the numbers. The [06-modelo-de-dados.md](../06-modelo-de-dados.md),
sections 4 and 6, the [10-maquinas-de-estado.md](../10-maquinas-de-estado.md), section 4, and the
pitfalls of [waha-fatos](../../referencia/waha-fatos.md) say what each piece does, but leave code
choices open. This decision records the choices and the readings made where the text is silent or
diverges. The formats of the WAHA API are in waha-fatos, section "The formats the hub uses", read
in the author's public documentation and not yet checked against a real engine.

## Decision

- **The engine client is the HTTP/1.1 of `hyper`**, one connection per call, with a deadline to
  open and a deadline for the whole call, a size ceiling checked on the declared size and again on
  the read, and the content type checked. The 15, section 4, said `reqwest`; it would bring new
  packages to the `Cargo.lock`. There is no TLS up to the engine: it only stays on the internal
  network ([0022](0022-motor-sem-porta-publica.md)).
- **One engine per organization** (`engine.organization_id` unique). The address and the address of
  the events stay in the clear; the API key and the events secret stay sealed by the vault, with
  the master key, in the columns `api_key_enc` and `events_secret_enc` (the 06 says `_cifrado`; the
  code already used `_enc` since 0001). Changing any of them increases `config_version`, and the
  hub rebuilds the client and reapplies the configuration of each engine session.
- **The events secret is drawn by the hub** (32 bytes) and never shown: the hub itself puts it in
  the configuration of each engine session, as the hexadecimal text. The signature is HMAC-SHA-512
  of the raw body, in hexadecimal, in `X-Webhook-Hmac`, checked in constant time. Changing the
  secret is `admin engine new-events-secret`; the supervision takes the new one to every session.
- **The inbox** is `POST /v1/engine/{engine}/events`, only on the internal port. The engine named
  in the address is the entry of the organization "by the events secret of the engine" (15,
  section 2, rule 2): nothing of the organization is used before the signature checks, and an
  unknown engine receives the same answer as a wrong signature. Order: declared size, content type,
  body read with a ceiling and a deadline, signature, strict format, 12-hour window, write. The
  event is written before the answer.
- **An event is a row.** A message is identified by its own identifier, not by that of the event:
  the same message coming through the inbox and through the startup fetch is one row only. The
  other types, by the identifier of the event. The key stays as a keyed digest with the key of the
  organization, and the body, encrypted with the key of the organization. The 12-hour window stays
  below the retention period of the handled event (1 day): an event written by someone does not
  become valid again after being purged.
- **Phones and chat identifiers are personal data of third parties.** The 06 lists `telefone` as a
  common column; the hub keeps it encrypted, with a keyed digest to find the number, and does the
  same with the chat identifiers (`chat_engine_id`) and with the displayed name of the chat.
- **The states of the number** are those of the 10, section 4. The reading of the code: an older
  state of the engine session that arrives after a newer one is ignored (`status_at`); a pairing
  has a deadline (`pairing_until`, 3 minutes); "pairing" with an account that is not the expected
  phone, or that the engine does not name, goes to "number swapped".
- **Removing and undoing the pairing without a message archive.** The pitfalls say to import the
  history before leaving WhatsApp, and the message archive is not in the MVP. That is why the
  command requires one of two words: `--keep-in-engine` (the session is stopped and stays in the
  engine, with the history; the number leaves the hub) or `--erase-engine-history` (it leaves
  WhatsApp and the engine erases the history). Undoing the pairing of a number swapped requires the
  second.
- **The security audit** gains, besides the list of the 15, section 2, rule 10: configuring the
  engine, changing the key, changing the events secret, adding, removing and confirming a number.
  They are what changes who can do what on WhatsApp. Asking for a pairing, the state change coming
  from the engine and the correction of a session by the supervision are not.
- **The supervision**, at startup and every 5 minutes: checks the version (alerts once when it
  changes to one that is not in the tested list, today only `2026.10.1` with GOWS), creates or
  corrects the session of each number (alerts when it corrects something put from outside),
  restarts the stopped session of a connected number, closes an expired pairing and, for each
  connected number, fetches from the engine the messages of the last 24 hours (at most 50 chats
  and 100 messages per chat) that the inbox did not receive.
- **The WhatsApp limit** (a 429 from the engine) is read as AR-704. The 08 reserves AR-704 for the
  send ceiling of the hub; the two situations ask the same thing of Hermes (to wait).
- **The pairing QR** is drawn in the terminal of the hub's machine by an encoder written in the
  project (`admin/qr.rs`), with no new dependency. The QR and the code appear once and are neither
  kept nor logged.

## Consequences

- The message notices are not born here: the rows of `engine_event` for message, reaction, edit
  and deletion stay with `processed_at` empty for the notice queue. Those of the session state are
  handled in the same transaction.
- Without the message archive, a number removed with `--keep-in-engine` leaves the session in the
  engine, with the history, until someone decides. The hub does not reuse it: the name of the
  session never comes back.
- The format of the WAHA API is that of the public documentation. Until the contract test runs
  against a real engine, each "to check" of waha-fatos is an assumption, written in the safest
  reading.
- Alternatives considered: `reqwest`, for the reason above; the signature checked by a fixed
  header instead of the HMAC, which the engine documents and which does not prove the body; and the
  events secret typed by the admin, which would make a long-lived secret pass through a person.
