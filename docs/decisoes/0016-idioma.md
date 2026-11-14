# 0016 — Language of the project

**Status:** decided · **Date:** 10/11/2026

## Context

The planning documents are in Portuguese. The source code also uses names in Portuguese. Those who use Hermes agents are all over the world.

## Decision

English in the names in the code, in the descriptions of the tools and in the error messages. The main README is in English (`README.md`), with translations in `docs/traducoes/`: Portuguese, Spanish, French, German, Russian, Hindi, Simplified Chinese and Japanese. The planning documents stay in Portuguese while the design matures.

## Consequences

- When the source code is brought in ([roadmap](../12-roteiro.md), Phase 2), the names are translated.
- The names of commands and of folders that appear in these documents in Portuguese are provisional: they get their English form in Phase 2.
- The names of the tools (`whatsapp_*`) come from the source service ([09-contrato-mcp.md](../09-contrato-mcp.md), section 6). Whether they also change is a point to confirm in Phase 2.
- The language of the fixed sentence and of the text of the block was not decided on 10/11/2026: it stays with the approval of the wording of each one.
- Translating the documents later costs little.
- **Change of 12/11/2026, at the maintainer's request:** the decision foresaw the README in English and in Portuguese, for when the repository was opened. The main README became the English one and got six translations, right now. On the same day Russian and Hindi came in: there are eight.
- The translations into Spanish, French, German, Russian, Hindi, Chinese and Japanese **were not reviewed by native speakers of each language**.
- When `README.md` changes: the eight translations change with it and, if the text is in an image (the badges and the drawing "Em uma tela"), the images of `docs/marca/` of each language.
- The translations point to the documents in Portuguese and say so to the reader.
- **Change of 13/11/2026, at the maintainer's request:** English is the main language of the project, in everything that is versioned: the code, the text of the screens of the panel, the commit messages and the documents (`docs/`, the decisions, the glossary and the guides of `deploy/`). The documents were translated on that day; the glossary keeps a table from the Portuguese terms to the English ones. Two documents only change with the maintainer's approval, [02](../02-como-funciona.md) and [04](../04-seguranca-conexao.md): their English versions wait for that approval, and until then they stay in Portuguese, with the text of the block. The names of the files and folders (`docs/decisoes/`, `docs/glossario.md` and the others) are still the Portuguese ones. The translations of the README now say that the documents are in English.
- **Change of 13/11/2026, approved by the maintainer:** the English versions of the [02](../02-como-funciona.md) and of the [04](../04-seguranca-conexao.md) were approved and took the place of the Portuguese ones, with the text of the block, which the panel now writes in English. What was approved is the wording in English: what each of the two documents proposes keeps the status it had, and the translation was made to change nothing in what they say (the numbers, the names and the structure of each were compared with the Portuguese version). No document is left in Portuguese.
- **Change of 13/11/2026, at the maintainer's request:** the screens of the panel speak the nine languages of the README. English is the source and the other eight are translations, kept in the code of the panel; they were not reviewed by native speakers of each language. How a language is chosen and what stays in English is in [0040](0040-painel-dentro-do-hub.md). When a sentence of a screen changes, its eight translations change with it: a test fails while one is missing.

## Change of 14/11/2026

The connection sentence is written in the language the panel is showing (the nine panel languages), because it is a message from the operator to the Hermes. The instruction page, `/llms.txt` and the block stay in English only. The maintainer asked for the link and the page and approved the design on 14/11/2026. Decision [0042](0042-frase-de-conexao-e-pagina-de-instrucoes.md). Nothing else in this decision is reopened. The admin command prints the English sentence; `--command` prints the block. The design was not tried on a real Hermes yet.
