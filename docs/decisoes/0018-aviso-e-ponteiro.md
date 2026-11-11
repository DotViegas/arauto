# 0018 — O aviso é um ponteiro, com frase fixa no conector

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

Para acordar um Hermes é preciso entregar a ele um texto. Se esse texto viesse do hub, um hub invadido ditaria o que a LLM de cada instalação lê. Se levasse o conteúdo da mensagem, um contato escreveria direto no início de uma execução.

## Decisão

O hub envia só um ponteiro: número, conversa e quantidade. A frase que acorda o Hermes é fixa e fica compilada no conector, assim como as descrições das ferramentas.

## Consequências

- O conteúdo só chega como resultado de ferramenta, marcado como de terceiros.
- O hub não consegue mudar um texto que a LLM lê.
- No aviso não entram texto de mensagem, nome de contato nem assunto de grupo.
