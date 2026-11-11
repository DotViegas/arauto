# 0024 — Envio de arquivo por endereço

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

Uma ferramenta que envia um arquivo a partir de um endereço faz o hub buscar algo escolhido por outro. O serviço de origem resolve isso baixando ele mesmo, e só de endereço público.

## Decisão

Manter o envio por endereço, como na origem: quem baixa é o hub, e só de endereço público. O envio por conteúdo continua existindo ao lado dele.

## Consequências

- O hub precisa recusar endereço que leve à rede interna, também depois de um redirecionamento e da resolução do nome. A regra vem com teste, na Fase 2 do [roteiro](../12-roteiro.md).
- A alternativa considerada, aceitar só conteúdo na primeira versão, eliminaria essa classe de ataque, ao custo de dividir arquivos grandes em partes pelo canal.
