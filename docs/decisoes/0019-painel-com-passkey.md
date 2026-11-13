# 0019 — Painel com passkey em toda ação que muda algo

**Estado:** decidida · **Data:** 09/11/2026

## Contexto

A conta do admin é a raiz de tudo: quem entra nela cria convites, aprova aparelhos e concede acessos. Senha com código de seis dígitos cai em phishing em tempo real.

## Decisão

O login do painel é por passkey, e não há senha. Toda ação que muda algo pede a passkey de novo, naquele momento: com a sessão do painel, sozinha, só se vê. O primeiro admin só é criado com o código de instalação, que o hub mostra na primeira partida.

## Consequências

- Uma sessão do painel roubada não basta para agir.
- O banco guarda só chaves públicas dos admins.
- Quem perde a passkey recebe outra de um segundo admin; sem segundo admin, um comando no servidor emite novo código de instalação.
- O painel fica em endereço separado da porta dos conectores e, por padrão, só em rede privada ([0028](0028-painel-fechado-por-padrao.md)).
- Todo texto vindo de aparelho ou do WhatsApp é escapado na tela.
- O painel continua dependendo do TLS de quem instala.
- **Mudança de 12/11/2026, a pedido de quem mantém:** enquanto a passkey não existir, entra-se no painel com um código de entrada, e a sessão do painel aberta por ele pode agir (conectar Hermes, parear, dar acesso). Nesse período o painel fica fechado: a porta dele só é alcançada na própria máquina do hub, por túnel. A passkey, no login e em toda ação que muda algo, volta a valer antes de o painel ser aberto à internet. O que isso deixa sem proteção está na [0040](0040-painel-dentro-do-hub.md).
