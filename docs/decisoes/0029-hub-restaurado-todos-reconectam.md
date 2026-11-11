# 0029 — Hub restaurado de cópia: todos reconectam

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Um hub que volta de uma cópia de segurança antiga esquece o que foi revogado, trocado ou retirado depois dela. O que já tinha deixado de valer volta a valer no banco.

## Decisão

Ao perceber a restauração, o hub suspende todos os aparelhos ativos, e cada Hermes volta com convite novo. Não existe "reativar". A alternativa considerada, reativar cada aparelho com um clique, pouparia trabalho depois de um desastre, mas pediria que o admin julgasse aparelho por aparelho, e um erro devolveria o acesso a uma cópia.

## Consequências

- Depois de uma restauração, cada Hermes precisa de um convite novo. Os acessos voltam como estavam na cópia: o que mudou depois dela se perdeu e precisa ser conferido (C11).
- A regra completa está em [04-seguranca-conexao.md](../04-seguranca-conexao.md), C11. O modo como o hub percebe a restauração continua sendo proposta, a validar na Fase 1 do [roteiro](../12-roteiro.md).
- O que continua em aberto está no fim de C11.
