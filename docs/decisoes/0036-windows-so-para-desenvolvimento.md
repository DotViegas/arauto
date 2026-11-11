# 0036 — Windows só para desenvolvimento; Linux na produção

**Estado:** proposta · **Data:** 10/11/2026

## Contexto

O conector foi desenhado para Linux, que é onde o Hermes roda, com Windows para depois. Quem mantém o projeto pediu para rodar e testar tudo direto pelo `cargo`, sem container, e uma das máquinas de desenvolvimento é Windows. Quatro peças do conector não funcionam no Windows do jeito que o desenho descrevia: a gravação forçada da pasta, a troca do programa em uso por troca de nome, o soquete local e a trava de arquivo.

## Decisão

O hub e o conector compilam e passam na suíte em Linux e em Windows. **Linux é a plataforma de produção. Windows é só de desenvolvimento.**

- O que muda de um sistema para outro fica num módulo só (`arauto-platform`), com a mesma interface nos dois.
- No Windows: a gravação durável não força a pasta; o programa em uso é renomeado antes de ser trocado; o soquete local é um canal nomeado, com o nome derivado da pasta, primeira instância e sem cliente remoto; a trava é a de arquivo, num arquivo só para isso.
- A pasta do conector pode ser trocada pela variável `ARAUTO_HOME`, para dois conectores na mesma máquina.

## Consequências

- No Windows não vale a garantia de gravação durável da pasta ([04](../04-seguranca-conexao.md), C7, regra 5), nem a de que só o dono fala com o canal nomeado. Por isso ele não é plataforma de produção, e os documentos dizem isso.
- Enquanto o desenvolvimento for só em Windows, o código específico de Linux é conferido pelo compilador e nunca executado. Ele precisa rodar em Linux, com a suíte inteira, antes de qualquer uso de verdade ([15-mvp.md](../15-mvp.md), seção 8).
- macOS e Linux arm64 continuam para depois.
