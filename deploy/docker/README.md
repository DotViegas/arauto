# Arauto with Docker Compose

The hub and the database in one composition. The proxy and the WhatsApp engine are yours and stay
outside it.

| File | What it is |
|---|---|
| [`Dockerfile`](Dockerfile) | The image of the hub. It also brings the connector for Linux x86_64, served for download. |
| [`compose.yaml`](compose.yaml) | The hub and PostgreSQL 16, with the volumes and the networks. |
| [`arauto.env.example`](arauto.env.example) | The variables you fill in. |

## What is left to prove

These files were written together with the code, but **the image has not been built yet and the
composition has not started on a server yet**: the development machine of the project has no
Docker. The first use on a server is the test. What to check in it is at the end of this page.

## 1. Prepare

You need a Linux x86_64 server with Docker and the Compose plugin, a domain name pointing to it,
and a proxy with TLS ([examples](../proxy/README.md)).

```
git clone <the repository> arauto
cd arauto/deploy/docker
cp arauto.env.example arauto.env
```

Fill in the `arauto.env`:

| Variable | What to put |
|---|---|
| `POSTGRES_PASSWORD` | A long, randomly drawn password. For example: `openssl rand -hex 32`. |
| `ARAUTO_PUBLIC_URL` | The outside address, with `https://`. Only the name: no path. |
| `ARAUTO_TRUSTED_PROXIES` | The address of your proxy **as the hub sees it**. See below. |

The `arauto.env` keeps a password. Do not put it in a repository.

**The address of the proxy, seen by the hub.** With the proxy on the server itself pointing to
`127.0.0.1:8443`, the hub does **not** see `127.0.0.1` or the address of the server: it sees the
gateway of a Docker network. The composition fixes these networks, and the example already brings
the two gateways (`172.30.250.1` and `172.30.251.1`). If you change the subnets, change the list
along with them.

A wrong list does not prevent the hub from starting. It ignores the header, treats all the
connectors as if they came from a single address, and warns **once** in the log: "a connection that
is not from a trusted proxy brought X-Forwarded-For". If that line appears, the address it shows is
the one missing from the list.

## 2. Build and start

```
docker compose --env-file arauto.env build
docker compose --env-file arauto.env up -d postgres
docker compose --env-file arauto.env run --rm hub init
docker compose --env-file arauto.env up -d
docker compose --env-file arauto.env ps
```

- `init` runs **once**. It creates the master key, the audit mark, the organization and the hub
  key. Running it again redoes nothing.
- After the `up`, the service `hub` needs to appear as `healthy`. The health check is the command
  `arauto-hub health`, which asks through the internal port.

From here on, every admin command runs inside the container:

```
docker compose --env-file arauto.env exec hub arauto-hub admin status
```

**The log of the hub** comes out in `docker compose --env-file arauto.env logs hub`. On starting it
says "the hub is up", with the two addresses (three, with the panel turned on). That is where the
warnings of the WhatsApp engine, the wrong list of proxies and the restored mode appear. For more
detail, put `RUST_LOG=debug` in the `arauto.env`.

## Open the panel (optional)

The panel shows the Hermes, the numbers and the accesses, connects and revokes a Hermes and changes
the access of a Hermes on a number. Pairing numbers and choosing the chats of an access continue
through the command. It comes off, and **has not been proven on a server yet**.

There is no passkey yet: whoever has the panel session open can connect, reconnect and revoke
Hermes and change accesses.
For that reason, leave the panel closed, as below, and sign out of it when you finish.

1. In the `arauto.env`, uncomment `ARAUTO_LISTEN_PANEL` and `ARAUTO_PANEL_URL`, and run
   `docker compose --env-file arauto.env up -d`.
2. From your computer, open a tunnel to the server: `ssh -L 8445:127.0.0.1:8445 YOUR-SERVER`.
3. Issue the entry code: `docker compose --env-file arauto.env exec hub arauto-hub admin panel code`.
   It appears once, is valid for 10 minutes and one entry. It is a secret: do not paste it in a
   chat or in a report.
4. Open `http://localhost:8445` in the browser and type the code.

The address in the browser has to be exactly that of `ARAUTO_PANEL_URL`: with another name, the hub
answers 421. The panel session lasts 12 hours, or 30 minutes without use, and ends when the hub
restarts.

The port of the panel is published only on `127.0.0.1` of the server. Inside Docker, whoever is on
a network of the hub (the WhatsApp engine, for example) reaches the entry screen; without the code,
they do not get past it. Opening the panel without a tunnel (an address of its own on your proxy,
with TLS, that is not that of the connectors) is possible, but **is not recommended while the
passkey does not exist**. The risk of opening it is yours.

## 3. Connect the first Hermes

```
docker compose --env-file arauto.env exec hub arauto-hub admin hermes add atendimento
docker compose --env-file arauto.env exec hub arauto-hub admin invite atendimento
```

The `invite` shows the block: the address of the hub, the hub key, the invite (valid for
15 minutes and one use), the address of the connector, the SHA-256 of the file and **one command
only**, chained: it downloads the connector to `~/.arauto`, checks the SHA-256 written in the block
and only then runs the `join`. If the file does not match, nothing runs.

This command runs **on Hermes's machine, with the same user as Hermes** (preferably by Hermes
itself, pasting the block into its command channel). Do not run it as another user or through
`docker exec` as root: the `join` registers the connector in Hermes by calling the program
`hermes`, and needs to find it.

The `join` answers a six-digit code, in the format `182-239`. You type it here, with or without the
hyphen:

```
docker compose --env-file arauto.env exec hub arauto-hub admin approve atendimento 182-239
```

The Hermes is born connected and with access to nothing.

| If the `join` says | What to do |
|---|---|
| `AR-601` (the registration in Hermes failed) | The program `hermes` was not found, or refused. Say where it is with the variable `ARAUTO_HERMES_BIN` and repeat the same `join`; or repeat with `--no-register` and register by hand. The request remains open in the hub. |
| `AR-101` or `AR-102` | The invite expired or was already used. Issue another with `admin invite`. |

A **reconnect** (another invite for a Hermes that already has a device) does not register again in
Hermes: the entry that already exists remains valid.

## 4. Link the WhatsApp engine

The engine is yours. Put it on the network `arauto-engine` (the name is in
`ARAUTO_ENGINE_NETWORK`), **with no public port**. On that network the hub reaches the engine, and
the engine reaches the hub at `http://hub:8444`.

```
docker compose --env-file arauto.env exec hub arauto-hub admin engine set --url http://waha:3000 --events-url http://hub:8444
docker compose --env-file arauto.env exec -it hub arauto-hub admin engine set-key
docker compose --env-file arauto.env exec hub arauto-hub admin engine health
```

The `set-key` asks for the API key of the engine in the terminal, so that it does not stay in the
history.

**Check both directions.** The `engine health` only proves that the hub reaches the engine. The
way back, from the engine to the hub, is where the messages arrive on time:

```
docker run --rm --network arauto-engine curlimages/curl -s http://hub:8444/health
```

It has to answer `ok`. If that way is wrong, nothing reports it: the hub still fetches the messages
by itself every 5 minutes, and everything seems to work, only late. After pairing, send a message
to the number: it has to appear in seconds, not in minutes.

Then, the number and the accesses:

```
... admin number add balcao +5565900000000
... admin number pair balcao
... admin wake atendimento on
... admin access grant atendimento balcao --scope all --read --send --wake
```

**For Hermes to be woken** one thing is missing that no command of the hub does: the connector
needs the key of Hermes's local API. It arrives through two variables in the entry `arauto` of the
tool server of Hermes: `ARAUTO_HERMES_API_KEY` (the key) and `ARAUTO_HERMES_API_URL` (the address,
with the number of the machine and the port, like `http://127.0.0.1:8642/v1/runs`; `localhost` is
not accepted). Without the key, the tools work and nobody is woken. Check on Hermes's machine:

```
~/.arauto/arauto-connector-linux-x86_64 status
```

It shows two lines: `Waking allowed by the admin` (what you released in the hub) and `Key of the
local API of Hermes` (whether the connector has the key). Hermes only wakes with both in order. The
exact format of this call has not been proven against a real Hermes yet: it is one of the checks of
the first use.

An access to the whole number wakes in a chat with a person. A **group** only wakes the Hermes that
has a line of its own for that group (`admin access chat ... --read --wake`). And the notice waits
for the chat to stay quiet for 6 seconds before waking; change it with `admin access wake-wait`.

## 5. Let two Hermes talk

```
... admin link add atendimento financeiro --wake --both
```

## Backup

- **Of the database:** the volume `arauto-postgres`, by `pg_dump` or by archiving the transaction
  log. Daily, at least.
- **Of the data folder** (volume `arauto-data`): keep it **separate** from the backup of the
  database, in a more protected place. It has the master key. Without it, the restored database
  opens no credential.
- Restoring the database from a copy turns on the restored mode: the Hermes stop being served
  until they reconnect, each one with a new invite. It is on purpose: an old copy brings old
  credentials.

## Update

```
git pull
docker compose --env-file arauto.env build
docker compose --env-file arauto.env run --rm hub migrate --i-have-a-backup
docker compose --env-file arauto.env up -d
```

Make the backup of the database before the `migrate`: the name of the argument is the reminder.

## What to check on the first use on a server

| Check | How |
|---|---|
| The image builds | `docker compose build` ends without error. |
| The hub passes the health check | `docker compose ps` shows `healthy`. |
| The database forces the write | The hub starts without asking for `ARAUTO_ALLOW_UNSAFE_DB`. |
| The internal port is not published | From outside the server, port 8444 does not answer. |
| The port of the panel is not published | From outside the server, port 8445 does not answer, with the panel on or off. |
| The panel does not exist on the address of the connectors | `curl -I https://YOUR-ADDRESS/sign-in` answers 404. |
| The download works | `curl -O https://YOUR-ADDRESS/v1/download/arauto-connector-linux-x86_64` and the `sha256sum` matches that of the block. |
| The address of the connectors | In `admin audit`, the connection of a Hermes shows its address, not that of the proxy. |
| The orderly stop | `docker compose stop hub` ends in less than 20 seconds, without being forced. |
