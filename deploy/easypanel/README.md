# Arauto on Easypanel

The hub and its database as one Compose service of Easypanel, set up on its screens: no terminal
on the server.

| File | What it is |
|---|---|
| [`compose.yaml`](compose.yaml) | The hub and PostgreSQL 16, with the volumes and the networks. |
| [`defaults.env`](defaults.env) | The values the recipe starts with. What you set in Environment wins. |

## What is left to prove

**Proposal.** This recipe was written with the code and has not been followed from start to end
on a server yet. In particular, nobody has seen Easypanel build the image from the repository, a
passkey registered by a real browser, or the hub replacing the password of the database on a
server. The first use is the test.

## 1. Create the service

1. In a project of Easypanel, create a service of the type **Compose**.
2. In **Source**, choose **Git**: the address of the repository, the branch, and
   `deploy/easypanel/compose.yaml` as the path of the Compose file. Easypanel builds the image of
   the hub from the repository (a few minutes the first time).
3. In **Environment**, turn on **Create .env file** and set the two addresses, with `https://`
   and only the name:

   ```
   ARAUTO_PUBLIC_URL=https://arauto.example.org
   ARAUTO_PANEL_URL=https://arauto-panel.example.org
   ```

   They are two different names. The first is where the connectors arrive; the second is where
   you open the panel. Nothing here is a secret.
4. In **Domains**, point each name to the service `hub`:

   | Name | Port of `hub` | What arrives |
   |---|---|---|
   | the one of `ARAUTO_PUBLIC_URL` | `8443` | the connectors and the download of the connector |
   | the one of `ARAUTO_PANEL_URL` | `8445` | the panel |

   Port `8444` gets no domain: it is the internal one, for the WhatsApp engine.
5. **Deploy.**

## 2. First access

1. Open the **logs** of the service. A new hub says "the database was empty: the hub installed
   itself" and then writes:

   ```
   No admin of this hub has a passkey yet.
   INSTALLATION CODE: ....-....-....
   ```

   The code is valid for 30 minutes and for one use. If it passed, restart the service: the hub
   writes another. It is a secret while it lasts: do not paste it in a chat or in a report.
2. Open the address of the panel, type the code and **register a passkey**: the fingerprint, the
   face or the PIN of your device, or your password manager.
3. From then on the panel is entered with the passkey. At that moment the hub also replaces the
   password of its database with one nobody sees, kept in its data folder.

Lost the passkey? `arauto-hub admin panel code`, in the console of the service `hub`, issues an
installation code that opens the registration of another one.

## What to check

| What | How |
|---|---|
| The connectors do not reach the panel | `https://` + the name of the connectors + `/sign-in` answers 404. |
| The panel does not answer on another name | The panel opened by any other address answers 421. |
| The address of each connector is the real one | The log of the hub does **not** have the line "a connection that is not from a trusted proxy brought X-Forwarded-For". If it does, put the address it shows in `ARAUTO_TRUSTED_PROXIES`, in Environment, and deploy again. |
| The database has no port outside | Nothing in Domains points to `postgres`. |
| The password of the database was replaced | After the first passkey, the log of the hub says so, once. |

## What is not guaranteed

- Between the first start and the first access, the database has the password written in
  `compose.yaml`, the same for every installation. Only what is on the network of this
  composition reaches the database in that interval.
- The panel is open to the internet at its address. It is entered with a passkey, but the passkey
  is asked at the entry and not again on each action: whoever has the panel session open acts
  while it lasts ([decision 0040](../../docs/decisoes/0040-painel-dentro-do-hub.md)).
- Whoever can read the logs of the service in the first 30 minutes of a new hub can register the
  first passkey. Whoever can do that in Easypanel already controls the service.
- The data folder now holds the password of the database beside the master key. Losing that
  volume loses both; the backup of the database still does not carry them.
- A migration is never applied by the start. To update a hub that has data: back up the
  database, and run `arauto-hub migrate --i-have-a-backup` in the console of the service before
  the new version serves.
