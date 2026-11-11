# 0023 — Como o conector obtém a chave da API local do Hermes

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Para acordar o Hermes, o conector chama a API local dele, que exige uma chave. A chave fica no arquivo de segredos do Hermes, e o conector, como processo filho, não a recebe do ambiente.

## Decisão

Os caminhos são tentados nesta ordem. Primeiro, a entrada do servidor de ferramentas referencia a variável do Hermes, e é o próprio Hermes que entrega a chave ao conector. Se o teste mostrar que isso não funciona, o conector lê a chave do arquivo de segredos do Hermes, com aviso claro a quem instala. Sem chave, aquele Hermes fica sem acordar.

## Consequências

- O teste do primeiro caminho é da Fase 1 do [roteiro](../12-roteiro.md). Dele depende o verbo "ser acordado" (Fase 5).
- Se valer o segundo caminho, o texto do bloco passa a dizer que o conector lê esse arquivo.
- Em qualquer caminho, a chave nunca vai ao hub nem aparece em tela.
