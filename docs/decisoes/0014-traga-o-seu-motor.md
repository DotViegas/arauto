# 0014 — Traga o seu motor

**Estado:** proposta, a validar no protótipo · **Data:** 09/11/2026

## Contexto

O motor usado hoje é de terceiro, é um cliente não oficial do WhatsApp e a imagem conferida declara licença não definida.

## Decisão

O Arauto não embute nem redistribui o motor. Quem instala informa o endereço e a chave do próprio motor. O motor fica atrás de uma interface, para que outro, inclusive a API oficial, entre como adaptador.

## Consequências

- A licença, o custo e o risco do motor são de quem o instala, e o projeto diz isso com clareza.
- O hub confere a versão do motor na partida e acusa quando não é uma versão testada.
- Um segundo adaptador pode ser acrescentado sem tocar no núcleo.
