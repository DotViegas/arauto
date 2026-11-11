# Fatos conferidos no motor (WAHA)

**Estado: fatos conferidos.** O que o Arauto assume sobre o WAHA, o programa usado como motor.

**Conferido em 09/11/2026, na imagem `devlikeapro/waha:gows-2026.10.1`, com o mecanismo interno GOWS, na
edição CORE.** O custo e a licença do WAHA são com o autor dele; este arquivo não substitui a
documentação dele.

Marcação: **[código]** lido no programa instalado; **[observado]** visto em funcionamento.

## Aviso

O WAHA é um cliente **não oficial** do WhatsApp. O WhatsApp pode bloquear números que o usam. O
Arauto não embute o WAHA: quem instala traz o seu.

**Licença:** o `package.json` da imagem conferida declara `UNLICENSED`. Antes de qualquer
distribuição conjunta, conferir os termos publicados pelo autor.

## O que a API permite comandar

| Grupo | Operações | Origem |
|---|---|---|
| Sessões do motor | Listar, criar, ver, atualizar, apagar, ligar, desligar, reiniciar, sair. | [código] |
| Pareamento | QR e código por número de telefone. | [código] |
| Servidor | Versão, ambiente, estado, parar. Saúde e ping. | [código] |
| Limites do WhatsApp | Consulta de teto de mensagens e de bloqueio de primeiro contato, por sessão. | [código] |

Várias sessões do motor funcionam na edição CORE, uma por número. **[observado]**

## Configuração de uma sessão do motor

| Campo | Conteúdo |
|---|---|
| `webhooks` | Endereço, eventos, segredo de assinatura, política de reenvio, cabeçalhos. |
| `ignore` | Ignorar status, grupos, canais e listas de transmissão. |
| `gows.storage` | O que o motor guarda: mensagens, grupos, conversas, etiquetas, contatos. |
| `client` | O nome com que o dispositivo conectado aparece no celular. |
| `metadata`, `proxy`, `debug` | Dados livres, proxy de saída, depuração. |

## Armadilhas

| Armadilha | Consequência para o hub |
|---|---|
| Atualizar a sessão **substitui a configuração inteira** e reinicia a sessão. | O hub só aplica quando há diferença, e sempre envia a configuração completa. |
| O que está em `ignore` nem é guardado pelo motor. | Uma sessão criada com grupos ignorados perde o histórico de grupo do pareamento. O hub cria a sessão já configurada. |
| O histórico antigo só chega **no pareamento**. Não existe "carregar mais antigas". | O arquivo do hub é o lugar permanente. |
| **Sair apaga o histórico** que o motor guarda daquela sessão. | O hub importa antes de permitir sair. |
| O histórico do pareamento não vira evento: só sai pela consulta de mensagens. | O hub importa por consulta. |
| O reenvio de eventos desiste depois de um tempo (cerca de 2 minutos na configuração usada). | Na partida, o hub busca no motor o que chegou enquanto esteve fora. |
| A contagem de participantes de grupo vem sempre zero. | Conta-se a lista de participantes. |
| O campo de origem do evento é sempre a conversa, mesmo em mensagem enviada pelo próprio número. | O hub identifica o autor por outros campos. |
| O motor recusa endereço de eventos com sublinhado no nome da máquina. | O hub é alcançado por um nome sem sublinhado. |
| A API não tem contador de mensagens não lidas. | "Novidades" é calculado pelo hub. |
| Editar sem texto e apagar sem o identificador completo têm comportamento perigoso por padrão. | O hub valida antes de chamar. |
| A documentação interativa pede credencial própria. | Não serve de catálogo em tempo de execução. |

Todas as armadilhas acima foram **[observado]** ou **[código]** na versão conferida.

## O que o motor não faz

Ligações, arquivar, fixar, bloquear contato, apagar conversa e botões.

## Regra para versões novas

Antes de trocar a versão do motor, rodar à mão o teste de contrato do hub contra um número de teste:
ler, listar grupos, enviar, e conferir o formato do evento de mensagem. O hub confere a versão do motor na
partida e acusa quando não é uma versão testada.
