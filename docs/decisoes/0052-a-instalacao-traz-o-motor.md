# 0052 — The installation brings the engine

**Status:** proposal · **Date:** 17/11/2026. Asked by the maintainer. **Nothing here is built or
tested.** The direction, the form (a service of its own in the composition) and the new text of the
rule in `CLAUDE.md` were approved by him on 17/11/2026; the design of the bootstrap below was not
approved step by step and was only read against the code, never run.

## Asked by the maintainer (17/11/2026)

- Installing Arauto installs the engine too, in a practical way, ideally with nothing to configure,
  the way PostgreSQL already comes.
- If the project ever has an engine of its own, it comes by the same mechanism.

## Context

- Until now whoever installs brought their own engine and typed its address and its key
  ([0014](0014-traga-o-seu-motor.md)).
- PostgreSQL does not live in the container of the hub: it is a second service of the same
  composition, on a network without a way out, and the hub waits for it (fact, read on 17/11/2026
  in `deploy/docker/compose.yaml` and `deploy/easypanel/compose.yaml`).
- A study of 17/11/2026 set aside a port of the engine library to Rust and an engine of the
  project's own for now: months of work and a standing duty to follow every change of WhatsApp.

## Decision (proposal)

1. **The engine is a third service of the composition,** in both recipes (Docker Compose and
   Easypanel), named by the image of its author, pinned to a tested version. It is not built into
   the image of the hub and the hub does not start it as a child process:
   [0003](0003-hub-e-motor-separados.md) stays.
2. **The project does not publish a copy of the image of a third-party engine.** The Docker of
   whoever installs pulls it from its author.
3. **The engine has no published port** ([0022](0022-motor-sem-porta-publica.md) stays). It sits on
   a network of the composition that has no fixed name, so two installations on one machine never
   share it.
4. **Nobody types or sees the key of the engine.** The hub draws it from the system generator,
   keeps it sealed as it keeps an engine key today, and hands the engine only its digest, in a file
   on a small volume the engine reads and cannot write. The digest file is written only for a key
   the hub drew, on an engine row marked as brought by the installation, never for a key an admin
   typed.
5. **The engine does not start without that file.** An engine left without a key draws one and
   prints it in its log; the recipe also turns off the screens of the engine and the passwords it
   would print for them.
6. **The hub records the engine of the first organization by itself,** from the address the recipe
   gives, in the same write that draws the key. The address from the recipe only fills a missing
   row: it never replaces an address an admin set.
7. **What is left for a person:** pairing the number in the panel.
8. **Bring your own engine stays as the advanced path,** with the commands that exist today. It is
   still the only path for an organization after the first.
9. **Order of work:** the Easypanel recipe first, then Docker Compose.
10. **An engine of the project's own,** if one comes, is a third service with its own image, by this
    same mechanism.

## What is not guaranteed

- **The license.** The repository of the engine declares Apache-2.0 and the `package.json` inside
  the checked image declares `UNLICENSED` ([referencia/waha-fatos.md](../../referencia/waha-fatos.md)).
  That naming an image in a recipe is not redistributing it is the project's own reading, not a
  legal opinion, and its author was not asked.
- **The risk of the engine is now stated by the project,** not left to whoever installs: it is an
  unofficial WhatsApp client, WhatsApp may block a number, and a fix after a change of WhatsApp
  depends on the author of the engine.
- **A changed key reaches the engine only when the engine restarts.** After a restore or a
  replacement of the key, a person restarts the engine.
- **On Easypanel, Restart and Stop act on every container of the service,** so there the hub and
  the engine stop together (documentation of Easypanel, not tried).
- **A volume of engine sessions kept from an earlier installation** carries live sessions the new
  hub does not know.
- **Every installation now pays for the engine,** in download and in memory, even one that only
  uses notes. The arm64 image was not tested.
- Not checked by a run: that the published image starts through the entry script read in its
  repository; that Easypanel accepts one volume shared by two services and a replaced entry point.

## Consequences

- `CLAUDE.md`, "What never to do": "Embed the engine in the delivery of the project" becomes "Build
  the engine into the image of the hub, or publish a copy of the image of a third-party engine. The
  composition names the original image, pinned, as a service of its own."
- [0014](0014-traga-o-seu-motor.md) is reduced to the advanced path.
- [0003](0003-hub-e-motor-separados.md): the version of the engine is pinned by the recipe of the
  project; applying it remains an act of whoever installs.
- The image of the hub gains a folder for the digest file, owned by the user the hub runs as.
- An installation that already exists needs a path of its own: its engine row was typed by an
  admin and stays as it is. To be designed before the code.
- The pinned version becomes a recurring duty: a new version means the contract test by hand and a
  new release of the recipe. Old tags of the image are removed by its author after some months
  (documentation, not checked by a pull).
- Each of these is a new security rule and comes with a test that fails without it: the plain key
  never appears in a log, an audit row, the file or a tool response; the recipes never publish a
  port of the engine, never use an unpinned version and never give its network a fixed name.
- To update when it is built, not before: [15](../15-mvp.md) (section on delivery),
  [03](../03-arquitetura.md), [05](../05-modelo-de-ameacas.md) and, with the maintainer's approval,
  [04](../04-seguranca-conexao.md).
