# 0010 — Identidade criada no aparelho

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Se a credencial lacrada fosse a única coisa que identifica um Hermes, quem obtivesse a chave-mestra do hub numa cópia de segurança poderia criar credenciais e se passar por qualquer Hermes.

## Decisão

Além da credencial lacrada, o conector cria na própria máquina um par de chaves. A parte privada nunca sai dali. O hub exige as duas.

## Consequências

- Uma cópia roubada do hub não basta para se passar por um Hermes.
- O hub guarda só a parte pública.
- O caminho de quem usa não muda.
