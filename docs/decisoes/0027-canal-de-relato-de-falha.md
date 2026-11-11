# 0027 — Canal para relato de falha

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Quem acha uma falha de segurança precisa de um caminho reservado para avisar. Uma issue pública entrega a falha a todos antes da correção.

## Decisão

O canal é o relato privado de falha do GitHub, na aba de segurança do repositório. Não há caixa de e-mail para isso.

## Consequências

- O repositório público fica no GitHub.
- O canal passa a existir quando o repositório for aberto ([roteiro](../12-roteiro.md), Fase 7). O [SECURITY.md](../../SECURITY.md) aponta para ele.
- Quem mantém precisa ligar o relato privado nas configurações do repositório e acompanhar os avisos.
