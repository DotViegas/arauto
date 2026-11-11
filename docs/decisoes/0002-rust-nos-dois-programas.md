# 0002 — Rust nos dois programas

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

O serviço de origem já é escrito em Rust e foi pensado para dezenas de agentes num servidor pequeno.

## Decisão

Hub e conector são escritos em Rust. O conector é um binário estático, sem dependência do sistema.

## Consequências

- O conector é um arquivo só, fácil de conferir por resumo SHA-256.
- O código de origem é reaproveitado, com os testes dele.
- Contribuir exige saber Rust.
