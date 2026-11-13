# 0026 — Name of the project

**Status:** decided · **Date:** 10/11/2026

## Context

The working name, multi-hermes, contained the name of a third-party project, Hermes Agent. A name of its own avoids confusion with it and does not depend on the rules of use of that name.

## Decision

The project is now called **Arauto**. The "arauto" (herald) is the one who carries and announces the messages.

Checked on 10/11/2026: the name is free on crates.io and on PyPI and taken on npm; on GitHub there is a small project, unrelated to this one, with the same name. The trademark registration was not checked.

## Consequences

- **Blocks:** the opening of the repository to the public ([roadmap](../12-roteiro.md), Phase 7). The trademark registration needs to be checked before that.
- The change was applied to the documents on 10/11/2026, all at once, approved by the maintainer. It reached the name in the texts, the programs (`arauto-hub` and `arauto-connector`), the connector's folder (`~/.arauto`), the tool server (`arauto`) and the prefix of the error codes (`AR-`).
- The form of each of these names follows the language decision ([0016](0016-idioma.md)): the commands, the files of the connector, the modules of the hub and the tools got their English form in the same change.
- In the brand, the name is written Arauto[IA].
- The project is still for Hermes agents and still has no tie to the authors of Hermes Agent.
