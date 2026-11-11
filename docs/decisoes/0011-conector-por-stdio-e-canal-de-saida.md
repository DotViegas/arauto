# 0011 — Conector por stdio e canal de saída

**Estado:** proposta, a validar no protótipo · **Data:** 09/11/2026

## Contexto

O Hermes fala com servidores de ferramentas por stdio ou HTTP; não tem websocket. O serviço de origem acordava o agente chamando a API dele, o que exige alcançar o agente pela rede e guardar a chave da API de cada um.

## Decisão

O conector é um servidor de ferramentas local, iniciado pelo próprio Hermes por stdio. É ele que abre um websocket de saída para o hub. Para acordar o Hermes, chama a API local.

## Consequências

- Funciona atrás de NAT: o Hermes só abre conexão de saída.
- Nenhuma porta de rede nova escuta na máquina do Hermes.
- O hub deixa de guardar a chave da API de cada Hermes.
- O conector vive enquanto o Hermes vive, sem precisar registrar serviço.
- Depende de uma prova: como o conector obtém a chave da API local ([0023](0023-chave-da-api-local.md)).
