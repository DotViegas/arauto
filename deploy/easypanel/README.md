# Arauto on Easypanel

The hub and its database as one Compose service of Easypanel, set up on its screens: no terminal
on the server for the first access.

| File | What it is |
|---|---|
| [`compose.yaml`](compose.yaml) | The hub and PostgreSQL 16, with the volumes and the networks. |
| [`defaults.env`](defaults.env) | The values the recipe starts with. What you set in Environment wins. |

## What is left to prove

**Proposal.** This recipe was written with the code and has not been followed from start to end
on a server yet. What was observed on the screens of Easypanel 2.38.0 on 13/11/2026 is in
[`referencia/easypanel-fatos.md`](../../referencia/easypanel-fatos.md): the Logs lines go from the
oldest to the newest and the viewer has no search; the Open button points to the primary domain,
and a "Make primary" star exists; a Console button exists. In particular, nobody has seen:

- Easypanel build the image from the repository;
- a passkey registered by a real browser, or the hub replacing the password of the database on a
  server;
- the primary domain moved to another one, and the Open button following it;
- what the Console of a Compose service opens (the guide gives another way);
- whether the Logs screen colors lines;
- a template of Easypanel in JSON for this recipe.

The first use is the test.

## 1. Choose the two names

The hub answers on two addresses, and they have to be two different names:

- **The connectors' name** (`ARAUTO_PUBLIC_URL`): where the connectors arrive.
- **The panel's name** (`ARAUTO_PANEL_URL`): where you open the panel in a browser.

**For real use, give the panel a name of your own,** such as `https://arauto-panel.example.org`.
A passkey is bound to the address of the panel: it stops working if that address changes, and
moving to another name means registering another passkey, with a code from the command.

The domain that Easypanel creates by itself for the service works for a test, but it binds the
passkey to the DNS of the provider of Easypanel and to the names of the project and of the
service: renaming either one is the same as changing the address.

## 2. Create the service

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

   Nothing here is a secret.
4. In **Domains**, point each name to the service `hub`:

   | Name | Port of `hub` | What arrives |
   |---|---|---|
   | the one of `ARAUTO_PANEL_URL` | `8445` | the panel |
   | the one of `ARAUTO_PUBLIC_URL` | `8443` | the connectors and the download of the connector |

   Port `8444` gets no domain: it is the internal one, for the WhatsApp engine.

   **Observed on Easypanel 2.38.0:** the **Open** button points to the primary domain of the
   service (the one with the filled star), and with two domains it pointed to the first one
   created. Create the panel's domain (`hub:8445`) first, or press its star ("Make primary"), so
   the button lands on the panel (moving the primary was not tried yet). **If it does not,** open the address of the panel directly in the
   browser: it works the same. A connector's domain opened by mistake answers a short text that
   says it is for the connectors and that the panel has its own address. Until the change of that
   text is approved (decision 0040, pending decision 5), its last line says to point that domain to
   the port of the panel: **do not do it.** It takes the domain away from the connectors. Make the
   panel's domain the primary one, or open the panel's address directly.
5. **Deploy.**

## 3. First access

1. In the Overview of the service, choose `hub` in the logs (the **Logs** button in the header
   shows the same lines merged) and scroll to the bottom: Easypanel has no search in
   its Logs, and the latest block is the last one. Take the code of the block with the **latest
   time**. A new hub says "the database was empty: the hub installed
   itself" and then writes, and writes again every 10 minutes, a block like this one (the time
   is UTC):

   ```
   === ARAUTO FIRST ACCESS (YYYY-MM-DD HH:MM:SS UTC) ===
   Welcome! Your Arauto hub is installed and waiting for its first admin.
   INSTALLATION CODE: <the code>
   Open: https://arauto-panel.example.org
   Type the code there and create your passkey.
   The code has no time limit and works once.
   Each restart writes a new one: use the block with the latest time.
   === end of ARAUTO FIRST ACCESS ===
   ```

   The code has no time limit and works once: creating the first passkey spends it, and each
   restart of the hub writes a new one. It is a secret while it lasts: do not paste it in a chat
   or in a report.

   Cannot open the Logs? Run the same command (see "Lost the passkey?" below): its code cancels
   the one in the log.
2. **Open** the address of the panel. The page that comes is step 1 of 3: paste the code in the
   field and continue.
3. **Step 2: create your passkey.** The page shows the address the passkey will work at; the
   device asks for the fingerprint, the face or the PIN, or your password manager does. If you
   leave the page, type the same code again: it is only spent when the passkey is created.
4. **Step 3: check the installation.** The panel opens a window that shows how the hub saw its
   own installation, and changes nothing. The same rows come with `arauto-hub admin check`.

At the creation of the first passkey the hub also replaces the password of its database with one
nobody sees, kept in its data folder.

**Lost the passkey?** Run `arauto-hub admin panel code` on the server of the hub: it issues a
code valid for 10 minutes, which opens the registration of another passkey. It also cancels the
code that is in the log. Where to run it:

- **In the console of the service `hub`, if Easypanel offers one for a Compose service**
  (not checked);
- or **over SSH on the server**: find the container with `docker ps --filter name=hub` and run
  `docker exec -it <that container> arauto-hub admin panel code`.

## What to check

The window of step 3 (`/installation` in the panel) and the command `arauto-hub admin check`
show most of this by themselves: the addresses, the password of the database, the connector, the
data folder and the engine. What they cannot see from inside:

| What | How |
|---|---|
| The connectors do not reach the panel | `https://` + the name of the connectors + `/sign-in` answers 404. |
| The panel does not answer on another name | The panel opened by any other address answers 421. |
| The address of each connector is the real one | The window of step 3 shows the address the hub sees you at. If it is not yours, or the log of the hub has the line "a connection that is not from a trusted proxy brought X-Forwarded-For", put the address it shows in `ARAUTO_TRUSTED_PROXIES`, in Environment, and deploy again. |
| The database has no port outside | Nothing in Domains points to `postgres`. |

## What is not guaranteed

- **Proposal, not checked on Easypanel:** moving the primary domain, the console, the templates and
  the colors of the Logs screen (see "What is left to prove"; the rest is in
  [`referencia/easypanel-fatos.md`](../../referencia/easypanel-fatos.md)).
- Until the first passkey, the database has the password written in `compose.yaml`, the same for
  every installation. Only what is on the network of this composition reaches the database in
  that interval.
- Before the first passkey, the installation code has no time limit: whoever reads the output of
  the start of the hub, in any copy (the log storage, a log shipper, a picture, a pasted excerpt),
  can register the first passkey until you do. Whoever can read those logs inside Easypanel
  usually controls the service already; a copy taken outside does not.
- The hub writes the code again every 10 minutes, so the log storage keeps many copies. They all
  die together at the first passkey or at the next start of the hub.
- Between typing the code and creating the passkey the code still serves: whoever sees it in that
  interval can open a registration too, and the first passkey registered wins.
- The entry page tells an anonymous visitor whether the hub already had its first access.
- A restart cancels the code in force; a code copied before it is refused, and the refusal does
  not say why.
- The ceilings of attempts are per address (30 a minute) plus a larger one for everyone together
  (600 for codes, 120 for passkey challenges), tested on 13/11/2026 against a test hub. Behind a
  proxy that is not trusted every request carries the address of the proxy and the ceiling is
  shared again; and enough addresses together can still fill the ceiling for everyone: a flood
  can delay the entry while it lasts.
- **Two versions of the hub must not serve the same database at once.** To upgrade, stop the old
  hub and then start the new one. The mark of the first access is written by the newer version; if
  an older one kept a passkey meanwhile, the start of the newer one repairs the mark by itself, but
  an overlap is still not supported.
- The first access ends with a person: creating a passkey needs a phone or a computer with a fingerprint, a
  face or a PIN.
- The panel is open to the internet at its address. It is entered with a passkey, but the passkey
  is asked at the entry and not again on each action: whoever has the panel session open acts
  while it lasts ([decision 0040](../../docs/decisoes/0040-painel-dentro-do-hub.md)).
- The data folder now holds the password of the database beside the master key. Losing that
  volume loses both; the backup of the database still does not carry them.
- A migration is never applied by the start. To update a hub that has data: back up the
  database, and run `arauto-hub migrate --i-have-a-backup` in the console of the service (not
  checked for Compose) or with `docker exec` over SSH, before the new version serves.
- While it is valid, the code lives in the memory of the hub process.
- A passkey is bound to `ARAUTO_PANEL_URL`: on a name in the Easypanel zone it depends on the DNS
  of the provider of Easypanel and on the names of the project and of the service; moving to
  another name means registering another passkey with a code from the command. For real use, a
  name of your own.
