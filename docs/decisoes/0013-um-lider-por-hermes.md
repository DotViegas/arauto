# 0013 — Um líder por Hermes

**Estado:** proposta, a validar no protótipo · **Data:** 09/11/2026

## Contexto

O Hermes pode iniciar mais de uma cópia do conector. Com uma credencial que troca a cada conexão, duas cópias disputando o canal trancariam uma à outra, e o hub veria conexões duplas de um Hermes saudável.

## Decisão

A primeira cópia a pegar uma trava de arquivo vira o líder: só ela abre o canal e troca a credencial. As outras são repassadoras: mandam as chamadas ao líder por um soquete local.

## Consequências

- Para o hub existe exatamente uma conexão por aparelho.
- Conexão dupla volta a significar cópia.
- Se o líder morre, o sistema solta a trava e outra cópia assume.
- Outro processo local do mesmo usuário consegue usar o soquete: é o limite do modo rápido.
- A pasta do conector não pode ser compartilhada entre dois Hermes, e o sistema de arquivos precisa suportar trava.
