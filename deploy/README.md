# How to install Arauto

Each folder here is a way to install the hub, with the files and a guide of its own. What applies
to all of them is on this page.

| Folder | For whom | Status |
|---|---|---|
| [`docker/`](docker/README.md) | A server with Docker and Docker Compose. It is the recommended path. | Written. **Not yet built or tested on a server**: see "What is left to prove", in the guide. |
| [`proxy/`](proxy/README.md) | Whoever needs an example of a proxy in front of the hub (Caddy or NGINX). | Examples written, not tested. |
| [`kubernetes/`](kubernetes/README.md) | Whoever runs on Kubernetes. | Only the list of what the manifests need to guarantee. The manifests have not been written. |

The connector is not installed from here: it is downloaded from the hub itself, through the block
that the command `admin invite` shows ([docs/02-como-funciona.md](../docs/02-como-funciona.md)).

## The parts

```
                 internet                         internal network of the server
 CONNECTOR ─wss──► PROXY ──http──► HUB :8443 ─────► POSTGRESQL
 (beside each               (TLS)   │
  Hermes)                           │ :8444 (internal, never published)
                                    │ :8445 (panel, off by default)
                                    ◄──── events ───── WHATSAPP ENGINE
                                    ────► calls ──────► (of whoever installs)
```

| Part | Whose it is | What it needs |
|---|---|---|
| **Hub** | The project's: the image. | Two ports: that of the connectors (published behind the proxy) and the internal one (health and events of the engine). And a third, that of the panel, only when you turn it on. |
| **PostgreSQL 16** | It comes in the composition. | Forced write to disk turned on (`fsync`). The hub refuses to start without it. |
| **Proxy** | Of whoever installs. | Terminate the TLS, pass on the switch to websocket, idle timeout above 90 seconds. |
| **WhatsApp engine** | Of whoever installs. The project does not embed it. | To be on a network that reaches the internal port of the hub, and never have a public port. |

## What every installation needs to guarantee

1. **Two separate volumes.** The database in one, the data folder of the hub (the master key and
   the audit mark) in another. The backup of the database does **not** carry the data folder:
   whoever steals the copy of the database does not take the key.
2. **Only the port of the connectors is published**, and behind a proxy with TLS. The internal port
   and the engine never have a public port. The port of the panel comes off; turned on, it stays
   only on the server itself, reached by tunnel, and only goes outside by your choice, with TLS and
   on an address that is not that of the connectors.
3. **The address of each connector.** Behind a proxy, the hub reads the address from the header
   `X-Forwarded-For`, and only believes it when the connection comes from a proxy of the list
   `ARAUTO_TRUSTED_PROXIES`. A wrong list lets anyone forge their own address.
4. **Backup of the database**, continuous or, at least, daily. Restoring a copy turns on the
   restored mode: every Hermes needs to reconnect
   ([docs/04-seguranca-conexao.md](../docs/04-seguranca-conexao.md), C11).
5. **Limit of open files** high for the hub: each connector keeps one connection.
6. **Stop timeout** of at least 20 seconds: the hub lets the calls in progress answer before
   leaving.

## The variables of the hub

| Variable | What for |
|---|---|
| `ARAUTO_DATABASE_URL` | The PostgreSQL database. It carries the password: it never appears in a log. |
| `ARAUTO_DATA_DIR` | The folder of the master key and of the audit mark. |
| `ARAUTO_LISTEN` | The address of the port of the connectors. |
| `ARAUTO_LISTEN_INTERNAL` | The address of the internal port. |
| `ARAUTO_LISTEN_PANEL` | The address of the port of the panel. Without it, the panel stays off. |
| `ARAUTO_PANEL_URL` | The address the admin opens in the browser: `https://` and only the name (and the port), or `http://localhost` with the port, by tunnel. It comes together with the one above. |
| `ARAUTO_PUBLIC_URL` | The address that goes in the block: `https://` and only the name (and the port). |
| `ARAUTO_CLIENT_IP_SOURCE` | Where the address of a connector comes from: `socket` or `forwarded`. |
| `ARAUTO_TRUSTED_PROXIES` | With `forwarded`: the proxies the hub believes. |
| `ARAUTO_DOWNLOAD_DIR` | The folder with the connectors served at `/v1/download/`. The image already brings it. |

## The commands, in any installation

```
arauto-hub init                 creates the master key, the mark, the organization and the hub key
arauto-hub serve                runs the hub
arauto-hub health               asks the hub of this machine whether it is up (0 = it is)
arauto-hub migrate --i-have-a-backup   applies migrations to a database that already has data
arauto-hub admin ...            the administration, by command
arauto-hub admin panel code     issues the entry code of the panel (10 minutes, one entry)
```

The sequence of a first use is in the guide of each folder. The [walkthrough of the first test](primeiro-teste.md) brings, step by step, what to check in a new installation.
