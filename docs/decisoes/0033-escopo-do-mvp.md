# 0033 — Escopo do primeiro MVP: conversa e WhatsApp juntos

**Estado:** decidida · **Data:** 10/11/2026

## Contexto

O roteiro previa oito fases em sequência, com as provas antes de qualquer código e a conversa entre Hermes para depois da publicação. Quem mantém o projeto pediu para começar o primeiro MVP, com a conversa entre Hermes e o WhatsApp na mesma leva.

## Decisão

O primeiro MVP reúne a conexão, a conversa entre Hermes, o WhatsApp e o acordar, em cinco marcos ([15-mvp.md](../15-mvp.md)). Conversa e WhatsApp são construídos juntos, sobre a mesma conexão.

Três pontos são proposta de quem escreve, a confirmar por quem mantém:

- **Administração por comando no servidor**, sem endereço de administração na rede. O painel, com passkey ([0019](0019-painel-com-passkey.md)), vem depois do MVP.
- **Código escrito do zero.** O serviço de origem não é copiado enquanto a liberação por quem tem direito sobre ele não vier ([roteiro](../12-roteiro.md), Fase 2). As ideias que vêm dele já estão nestes documentos.
- **As provas "no canal" da Fase 1 são feitas dentro do MVP**, como testes com rodadas sorteadas. As provas "no Hermes" continuam pendentes e dependem de um Hermes de verdade.

## Consequências

- O roteiro é reescrito na ordem dos marcos. O critério de pronto de cada fase continua valendo.
- Ficam fora do MVP: painel, arquivo de mensagens, assinatura das versões, envio de arquivo por endereço e modo blindado. O modo restaurado entra, porque mexe em toda gravação de evento de segurança desde o primeiro marco.
- O desenho do MVP passou por uma revisão adversarial em 10/11/2026, antes do código. As correções estão nos documentos 13, 14 e 15.
- Enquanto não houver painel, a regra da passkey em toda ação que muda algo não tem onde ser aplicada: quem administra é quem tem acesso ao servidor.
- O MVP não é uma versão publicada. Nada é aberto ao público antes da Fase 7.
- **Mudança de 12/11/2026, a pedido de quem mantém:** o painel começou antes do fim do MVP, pelas
  telas de leitura e pelo guia de conectar Hermes. Como ele entra no hub, e o que ainda falta
  decidir, está na [0040](0040-painel-dentro-do-hub.md), que é proposta. Parear e dar acesso
  continuam pelo comando.
