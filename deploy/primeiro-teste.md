# Walkthrough of the first test on a server

For whoever installs Arauto for the first time with a real Hermes and a real WhatsApp engine. Each
step says what to do and what has to happen. Where the result is another, **stop and write it
down**: what came out on the screen, the log of the hub and the time.

The installation guide is the [docker/README.md](docker/README.md); this walkthrough follows the
same order and adds the checks.

```
 PHASE 0  before starting          PHASE 5  the WhatsApp number
 PHASE 1  start the hub            PHASE 6  wake Hermes
 PHASE 2  the proxy                PHASE 7  notes between two Hermes
 PHASE 3  the first Hermes         PHASE 8  what cannot happen
 PHASE 4  the WhatsApp engine      PHASE 9  outages, backup and closing
```

In the commands, `dc` is `docker compose --env-file arauto.env`, and `adm` is
`dc exec hub arauto-hub admin`.

## What this test needs to answer

Five things never ran outside the automatic tests. They are the reason for the test, and each one
has a step marked with **[PROOF]**.

| # | What was never proven | Step |
|---|---|---|
| 1 | The image builds and the composition starts. | 1.2 to 1.5 |
| 2 | The connector for Linux runs on Hermes's machine and speaks `wss://` (TLS). | 3.2 and 3.3 |
| 3 | The `join` registers the connector in Hermes, and Hermes starts it. | 3.4 to 3.6 |
| 4 | The hub talks to the real WAHA: pair, receive and send. | 4 and 5 |
| 5 | The connector wakes Hermes through the local API. | 6 |

## Phase 0: before starting

- [ ] **0.1** Linux x86_64 server with Docker and Compose; a domain name pointing to it; a proxy
  with TLS and a certificate from a public authority (self-signed will not do: the connector
  refuses).
- [ ] **0.2** On Hermes's machine, as Hermes's user: `uname -m` answers `x86_64`;
  `ldd --version` shows glibc 2.36 or newer; `which hermes` answers a path. Write down the three.
  If `which hermes` does not answer, you will need `ARAUTO_HERMES_BIN` in step 3.3.
- [ ] **0.3** The clocks of the server, of Hermes's machine and of the engine are right (NTP). The
  hub refuses an event of the engine with a time outside the window.
- [ ] **0.4** A test WhatsApp number, which can be disconnected and reconnected, and a second
  phone to send messages to it. Never a number in real use.

## Phase 1: start the hub

- [ ] **1.1** `cp arauto.env.example arauto.env` and fill it in. Database password only in
  hexadecimal (`openssl rand -hex 32`): it goes inside an address, and `@`, `/`, `#` or `%` break
  the connection. `git status` does not show the `arauto.env`.
- [ ] **1.2 [PROOF]** `dc build` ends without error. Write down the time. It is the first time the
  image is built, and the first time the connector is compiled with TLS.
- [ ] **1.3** On purpose, `dc up -d` **before** the `init`. Expected: the hub keeps restarting, and
  `dc logs hub` says the database is not installed and says to run the `init`. Nothing is created.
- [ ] **1.4** `dc run --rm hub init`. Expected: "The hub is installed." and the public key of the
  hub. Running it again: "already installed", with the same key.
- [ ] **1.5 [PROOF]** `dc up -d` and `dc ps`. Expected: `hub` as `healthy` within one minute.
  `dc logs hub` shows "the hub is up" with the two addresses.
- [ ] **1.6** `adm status`. Expected: database current, mark consistent, restored mode off.
- [ ] **1.7** `dc exec hub ls /usr/share/arauto/download`. Expected: only `arauto-connector-linux-x86_64`.

## Phase 2: the proxy

- [ ] **2.1** Configure the proxy ([proxy/README.md](proxy/README.md)): only `/v1/channel` and
  `/v1/download/` reach the hub; websocket; timeout of 120 seconds; the header `X-Forwarded-For`
  **replaced** by the address of whoever arrived.
- [ ] **2.2** From outside the server: `curl -I https://YOUR-ADDRESS/` answers 404; ports 8443,
  8444 and 8445 do not answer directly; `curl -I https://YOUR-ADDRESS/sign-in` answers 404.
- [ ] **2.3** `curl -fsSLO https://YOUR-ADDRESS/v1/download/arauto-connector-linux-x86_64` downloads
  the file. Keep its `sha256sum` for step 3.1.
- [ ] **2.4** `dc logs hub` does **not** have the line "a connection that is not from a trusted
  proxy brought X-Forwarded-For". If it does, the address it shows is the one missing in
  `ARAUTO_TRUSTED_PROXIES`: fix it, `dc up -d` and repeat 2.3.

## Phase 3: the first Hermes

- [ ] **3.1** `adm hermes add atendimento` and `adm invite atendimento`. Expected: the block with
  the address of the hub (`wss://...`), the hub key, the invite, **one** line `connector:`, the
  `sha256:` equal to that of step 2.3, and one command only, chained. The invite is a secret: do
  not paste it in a report.
- [ ] **3.2 [PROOF]** On Hermes's machine, as Hermes's user, before pasting the block: download
  the file by hand and run `./arauto-connector-linux-x86_64 --version`. Expected: the version.
  A GLIBC error or "not found" means the program does not work on that machine: stop.
- [ ] **3.3 [PROOF]** Paste the chained command of the block (through Hermes's command channel, or
  in the terminal of its user). Expected: the file downloads, the digest matches, and the `join`
  shows "VERIFICATION CODE: NNN-NNN". It is the first real `wss://` connection. If `AR-601` comes
  out, see the table of the guide; the request remains open.
- [ ] **3.4 [PROOF]** `hermes mcp list` shows the entry `arauto`, with the program
  `~/.arauto/arauto-connector` and the argument `mcp`, with no secret at all. Write down the exact
  text of the entry.
- [ ] **3.5** On the server: `adm approve atendimento NNN-NNN`. Then `adm hermes list`. Expected:
  approved and, within one minute, active.
- [ ] **3.6 [PROOF]** On Hermes's machine: `~/.arauto/arauto-connector status`. Expected: channel in
  session, no secret on the screen. In Hermes, the tool `arauto_status` appears and answers.
- [ ] **3.7** `ls -la ~/.arauto ~/.arauto/devices/*`. Expected: folders of the owner only (700) and
  the files `identity.key`, `credential.seal` and `hub.json` of the owner only (600).
- [ ] **3.8** `adm audit --tail 20`. Expected: the connection of the Hermes appears with the
  **outside** address of its machine, never `172.x` or that of the proxy.

## Phase 4: the WhatsApp engine

- [ ] **4.1** Start WAHA on the network `arauto-engine`, **with no public port**. Write down the
  version.
- [ ] **4.2** `docker run --rm --network arauto-engine curlimages/curl -s http://hub:8444/health`
  answers `ok`. Without this the messages arrive minutes late and nothing reports it.
- [ ] **4.3** `adm engine set --url http://waha:3000 --events-url http://hub:8444`, then
  `dc exec -it hub arauto-hub admin engine set-key` and paste the API key of the engine.
- [ ] **4.4 [PROOF]** `adm engine health` and `adm engine version`. Expected: healthy, and the
  version. If the version is not one of the checked ones, the hub warns and creates an alert:
  going on is a known risk.
- [ ] **4.5** Send an event without a signature to the hub, from inside the network of the engine.
  Expected: refusal, and `dc logs hub` shows "an event of the engine was refused" with the reason.

## Phase 5: the WhatsApp number

- [ ] **5.1** `adm number add balcao +55DDDNUMERO`. Expected: added, not paired, with the session
  already created in the engine.
- [ ] **5.2 [PROOF]** Before pairing, keep the raw answer of `GET /api/sessions/NAME` of WAHA.
  Then, `dc exec -it hub arauto-hub admin number pair balcao` in a terminal with a dark background
  with 80 columns by 40 lines. Expected: the QR appears; on scanning, "is connected". If the QR
  does not read, repeat with `--code`.
- [ ] **5.3** Right after, `adm number list`. Expected: `connected`. If the screen says "another
  telephone" with the right phone, **do not run `number confirm`**: run `adm number unpair balcao --erase-engine-history`, keep the
  raw answer of the session (the field `me`) and stop this number. The hub expects `me.id` as
  `digits@c.us`.
- [ ] **5.4 [PROOF]** `adm wake atendimento on` and
  `adm access grant atendimento balcao --scope all --read --send --wake`. Through Hermes:
  `whatsapp_list_chats` and `whatsapp_read_chat`. Expected: the chats and the messages of the
  number.
- [ ] **5.5 [PROOF]** Through Hermes, `whatsapp_send_text` to the second phone. Expected: the
  message arrives **once**, and the answer brings `message_id`. Write down how long it took.
- [ ] **5.6 [PROOF]** From the second phone, send a message to the number. In the database, count
  the chats of that person (`SELECT id, hidden_id FROM chat`). Expected: **one** chat. If two
  appear, or one with `hidden_id` true, the engine names the same person in two ways: write it
  down. In that case an access that has chat lines does not read and is not woken by that chat
  until it gains a line of its own, and the log of the hub reports no error.

## Phase 6: wake Hermes

- [ ] **6.1** Without touching anything: `~/.arauto/arauto-connector status`. Expected: "Waking
  allowed by the admin: yes" and "Key of the local API of Hermes: not configured". Send a message
  from the second phone. Expected: Hermes does **not** wake. It is the baseline.
- [ ] **6.2 [PROOF]** Put in the entry `arauto` of Hermes the variables `ARAUTO_HERMES_API_KEY` and
  `ARAUTO_HERMES_API_URL` (with the number of the machine, like `http://127.0.0.1:8642/v1/runs`)
  and restart the tool server. `status` now says "configured". Write down **how** the key ended up
  in the entry: in the clear or by reference to a variable of Hermes.
- [ ] **6.3 [PROOF]** Send **one** message from the second phone. Expected: about 6 seconds later,
  Hermes starts a run and reads the message through the tool. If it does not wake, keep what the
  API of Hermes answered: the format of the call is the assumption this step proves.
- [ ] **6.4** Send 4 messages in a row, with 2 seconds between them. Expected: **one** run, with
  the four, about 6 seconds after the last.
- [ ] **6.5** Send a message **from the paired phone** (the number itself). Expected: it does not
  wake.
- [ ] **6.6** In a group the number is in: a message from a participant. Expected: it does not wake.
  Then `adm access chat atendimento balcao GROUP-ID --read --wake` and repeat: it wakes.
- [ ] **6.7** Restart the connector after a run. Expected: Hermes is not woken again by the same
  message.

## Phase 7: notes between two Hermes

- [ ] **7.1** A second Hermes, `financeiro`, through steps 3.1 to 3.6. Before any link, neither of
  the two has `hermes_` tools.
- [ ] **7.2** `adm link add atendimento financeiro --wake --both`. Expected: the risk warning on
  the screen, and the two Hermes now have the four tools without restarting anything.
- [ ] **7.3** `atendimento` calls `hermes_send_message` to `financeiro`. Expected: `financeiro`
  wakes (if it has the key), lists the thread with one unread, reads and answers in the same
  thread.
- [ ] **7.4** `adm link remove atendimento financeiro --both`, with a note still unread. Expected:
  `financeiro` no longer reads that note; `atendimento` receives `AR-707` on sending another.

## Phase 8: what cannot happen

- [ ] **8.1** Before step 7.4, a Hermes asks for a thread by an invented identifier. Expected:
  `AR-708`. With a third Hermes at hand, it asks for the thread of the other two by the right
  identifier: the same answer.
- [ ] **8.2** `adm access chat atendimento balcao +55...` without verbs (blocking a person) and that
  person sends a message. Expected: Hermes does not wake and the read of that chat answers
  `AR-705`. Then, `adm access chat-remove atendimento balcao +55...` undoes the block.
- [ ] **8.3** 21 `whatsapp_send_text` in one minute in the test chat. Expected: the 21st refused
  with `AR-704` and the waiting time.
- [ ] **8.4** `adm access remove atendimento balcao` with the Hermes in session. Expected: the next
  read call answers `AR-701`, and the list of tools loses the WhatsApp ones. Grant the access
  again (step 5.4) before going on.
- [ ] **8.5** `adm revoke financeiro` with that Hermes in session. Expected: its channel drops in
  less than 5 seconds, and the tools start answering the code of the closed device. The
  `atendimento` stays linked, for phase 9.
- [ ] **8.6** `dc logs hub` of the whole test: no line with the password of the database, the
  invite, the key of the engine or the text of a message.

## Phase 9: outages, backup and closing

- [ ] **9.1** `time dc stop hub` with a Hermes connected. Expected: less than 20 seconds, exit 0.
  `dc up -d`: the Hermes come back to active by themselves.
- [ ] **9.2 [PROOF]** Stop the hub for 6 minutes; in that time send 3 messages in one chat and 3
  in another; start the hub and send one more in the first right after. Expected: Hermes is woken
  by the two chats, with all the messages, within 6 minutes.
- [ ] **9.3** Restart the session of the number in WAHA and send 3 messages while it comes back.
  Expected: Hermes wakes once, with the three, when the number comes back to connected.
- [ ] **9.4** Stop WAHA right after a `whatsapp_send_text` goes out. Expected: Hermes receives
  `AR-713` (unknown result), never "engine unavailable", and does not send the same message twice.
- [ ] **9.5** Backup: `dc exec postgres pg_dump -U arauto -Fc arauto > backup.dump`, and the copy of
  the volume `arauto-data` to another place, separate.
- [ ] **9.6** Rehearsal of the wrong folder: stop the hub, start it with the volume `arauto-data`
  empty. Expected: the hub **does not start**, for lack of the master key, and `adm status` (with
  the right folder back) shows the restored mode **off**.
- [ ] **9.7** Restoration rehearsal, only if the rest passed: restore the copy of step 9.5 over the
  database. Expected: `adm status` shows the restored mode, the Hermes stop being served, and each
  one comes back with a new invite. Check the numbers and the accesses before issuing the invites.
- [ ] **9.8** Closing: `adm alerts` with no alert you cannot explain; `adm audit --tail 50` tells
  the story of the test.

## What to write down, even if everything passes

| What | What for |
|---|---|
| The version of WAHA and that of Hermes | The list of checked versions of the project. |
| The text of the entry `arauto` in Hermes, with and without the waking variables | The automatic registration of the variables does not exist yet. |
| What the local API of Hermes answered to the waking call | The format of the call is an assumption. |
| The raw answer of the WAHA session at the first "connected" (the field `me`) | The format of the phone in the pairing. |
| Whether the same person became two chats (step 5.6) | It decides the definitive fix of the hidden identifier. |
| The time of each send (text, image, document) | The send deadline of the hub is 15 seconds. |
| The time of the `dc build` and the memory used | The installation guide. |
