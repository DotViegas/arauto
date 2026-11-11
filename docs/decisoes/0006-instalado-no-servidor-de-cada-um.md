# 0006 — Instalado no servidor de cada um

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Um hub central guardaria as conversas e as sessões do motor de todos os que o usam e concentraria o risco num lugar só.

## Decisão

O Arauto é instalado no servidor de quem o usa, uma organização por instalação. O banco nasce separado por organização, para que um hub possa servir a várias sem reescrita.

## Consequências

- Conversas, sessões do motor e chaves ficam com quem instala.
- Toda tabela de negócio pertence a uma organização desde a primeira migração.
- Com várias organizações num hub, um motor para cada uma: nunca compartilhado.
- Não se constrói agora: cadastro aberto ao público e limites de uso por organização.
