# 0028 — Painel fechado por padrão; quem instala escolhe

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

A conta do admin é a raiz de tudo ([0019](0019-painel-com-passkey.md)). Um painel alcançável pela internet fica exposto a quem varre endereços. Mas há quem precise administrar de fora e não tenha rede privada.

## Decisão

Quem instala escolhe. A instalação padrão vem com o painel fechado: ele só responde na rede privada ou por VPN. O guia de instalação mostra como abri-lo à internet, por conta de quem instala. A porta dos conectores é a única que precisa ser alcançável de fora.

## Consequências

- Painel e conectores continuam em endereços separados ([03-arquitetura.md](../03-arquitetura.md), seção 4).
- Aberto ou fechado, o painel exige a passkey em toda ação que muda algo e depende do TLS de quem instala.
- O guia de instalação ([roteiro](../12-roteiro.md), Fase 7) diz o risco de abrir.
