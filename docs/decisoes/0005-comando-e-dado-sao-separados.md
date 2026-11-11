# 0005 — Comando e dado são separados

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Um agente que lê mensagens de terceiros pode ser instruído por elas. Se o mesmo meio servir para dar ordens e para receber conteúdo, a autoridade passa a depender de um rótulo dentro de um texto.

## Decisão

Ordem vem do canal de comando do Hermes, do operador, e esse canal fica fora do Arauto. Tudo o que entra pelos números do hub é dado, nunca ordem.

## Consequências

- O Arauto nunca é canal de comando.
- O aviso que acorda o Hermes diz que é dado e não leva conteúdo de terceiros ([0018](0018-aviso-e-ponteiro.md)).
- O bloco de conexão só vale quando chega pelo canal de comando, e diz isso.
- Uma ordem escrita num grupo de um número do hub não vale como ordem: chega só como leitura.
- Quem sustenta isso, no fim, é o Hermes. O hub nunca entrega texto de terceiros na posição de instrução, mas não garante que a LLM jamais obedeça a uma mensagem lida.
