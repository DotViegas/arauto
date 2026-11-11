# Glossário

**Estado: proposta.** Um nome para cada coisa. Os outros documentos usam estes termos.

## As peças

| Termo | O que é |
|---|---|
| **Arauto** | O projeto. Tem dois programas, o hub e o conector, e usa um terceiro, o motor. |
| **Hermes** | Um agente de IA (Hermes Agent). No painel, é uma linha: a identidade lógica do agente, dona dos acessos. |
| **LLM do Hermes** | O modelo de linguagem que decide o que o Hermes faz e roda comandos no terminal dele. |
| **Hub** | O servidor do Arauto. Tem o painel, a porta dos conectores e a caixa preta, e fala com o motor. |
| **Painel** | As telas do hub, usadas por pessoas. |
| **Conector** | Programa pequeno que roda ao lado do Hermes. Para o Hermes, é um servidor de ferramentas local. |
| **Motor** | O programa que fala com o WhatsApp. Hoje, o WAHA de quem instala. |
| **Arquivo de mensagens** | As conversas guardadas no hub. |
| **MCP** | O protocolo padrão pelo qual um agente usa ferramentas de fora. |
| **stdio** | Forma de dois programas conversarem pela entrada e pela saída de texto, sem rede. É como o Hermes fala com o conector. |
| **API local do Hermes** | O endereço, na própria máquina do Hermes, por onde se inicia uma execução dele. |

## As pessoas

| Termo | O que é |
|---|---|
| **Admin** | Pessoa com login no painel. |
| **Operador** | Quem dá ordens ao Hermes pelo canal de comando. Pode ser o admin. |
| **Organização** | Dona de Hermes, números e admins. Uma instalação tem uma; o banco já separa por organização. |

## Comando e dado

| Termo | O que é |
|---|---|
| **Canal de comando** | Por onde o operador dá ordens ao Hermes. Existe fora do Arauto e não passa por ele. |
| **Meios de conversa do Hermes** | As plataformas em que o Hermes conversa: o canal de comando e as demais. |
| **Número** | Uma conta de WhatsApp pareada no motor. |
| **Apelido** | O nome que o admin dá a um número no painel. |
| **Parear** | Ligar um número ao motor, pelo celular. Vale só para números. |
| **Código de pareamento, QR** | O que o celular lê ou digita para parear. Segredo de curta duração. |
| **Dispositivo conectado** | Cada aparelho ou programa ligado a uma conta de WhatsApp, como aparece no celular. O motor é um deles. |
| **Conversa** | Um contato ou grupo dentro de um número. |

## A conexão de um Hermes

| Termo | O que é |
|---|---|
| **Conectar** | Ligar um Hermes ao hub. Vale só para Hermes. |
| **Bloco** | O texto que o admin copia do painel e cola no canal de comando do Hermes. |
| **Convite** | Segredo temporário que vai dentro do bloco. Vale 15 minutos e um uso. Só serve para pedir conexão. |
| **Inscrição** | O momento em que um conector apresenta um convite ao hub. |
| **Pedido de conexão** | O que nasce de uma inscrição. Não dá acesso a nada até o admin digitar o código. |
| **Código de conferência** | Seis dígitos que o conector mostra e o admin digita no painel. Não é segredo. |
| **Aparelho** | Uma instalação do conector, com a sua identidade. Um Hermes tem no máximo um aparelho ativo. |
| **Identidade do aparelho** | Par de chaves criado na máquina do Hermes. A parte privada nunca sai dali. |
| **Credencial lacrada** | Envelope emitido pela caixa preta, que o conector guarda e não consegue abrir. Para usar o hub é preciso trocá-la por outra. |
| **Lacre** | A cifra que fecha a credencial. Só o hub abre. |
| **Geração** | O número de ordem de cada credencial emitida para um aparelho. O hub compara também o resumo da credencial, porque depois de uma restauração um número pode se repetir. |
| **Geração zero** | Aparelho aprovado que ainda não confirmou a primeira credencial. |
| **Caixa preta** | A parte do hub que sorteia e lacra. |
| **Chave-mestra** | A chave da caixa preta. Fica fora do banco. |
| **Chave do hub** | Par de chaves do hub. A parte pública vai em todo bloco: é por ela que o conector reconhece o hub certo. |
| **Chave de versões** | A chave do projeto que assina cada versão publicada do conector. Não fica em hub nenhum. |
| **Canal** | A conexão de saída do conector para o hub, cifrada de ponta a ponta. |
| **Aperto de mãos** | A troca inicial em que conector e hub se reconhecem e combinam a chave da conversa. |
| **Sessão do canal** | Um período em que o canal está aberto e a credencial daquela sessão já foi trocada. |
| **Batimento** | Mensagem periódica que mostra que o outro lado continua ali. |
| **Líder** | A única cópia do conector que segura o canal num Hermes. |
| **Repassadora** | Cópia do conector que não é líder: repassa as chamadas ao líder. |
| **Reuso** | Apresentar uma credencial que já foi trocada, ou nenhuma quando deveria haver. Sinal de cópia. |
| **Conexão dupla** | Duas conexões vivas do mesmo aparelho ao mesmo tempo. Sinal de cópia. |
| **Dada por morta** | Conexão antiga que não respondeu ao teste do hub e foi fechada para outra entrar no lugar. |
| **Quarentena** | Aparelho parado porque o hub deixou de confiar na credencial dele: houve sinal de cópia, ou o arquivo se perdeu. Só volta com convite novo. |
| **Suspenso** | Aparelho parado porque o hub voltou de uma cópia de segurança. Só volta com convite novo. |
| **Modo restaurado** | O estado do hub depois de perceber que voltou de uma cópia de segurança antiga. Dura até não restar aparelho suspenso. |
| **Marca da auditoria** | O número do último evento de segurança, guardado fora do banco. Banco atrás da marca é banco restaurado. |
| **Revogar** | Encerrar um Hermes no hub: o aparelho, o pedido em andamento e o convite emitido. Os acessos ficam guardados. Quem revoga é o admin, no painel; um aparelho também pode pedir o próprio encerramento. |
| **Saída local** | Apagar os arquivos do conector sem falar com o hub. É para uma pessoa, quando o hub não existe mais ou trocou de chave. |
| **Reconectar** | Gerar convite novo para um Hermes que já existe. Os acessos são mantidos. |
| **Modo rápido** | O próprio Hermes instala o conector. A credencial fica onde a LLM alcança. |
| **Modo blindado** | Uma pessoa instala o conector separado do Hermes. A LLM não alcança a credencial. Previsto, não é da primeira versão. |
| **Resumo** | O resultado de uma função de resumo (SHA-256) sobre um dado. Serve para conferir sem guardar o dado. |
| **Vigente e pendente** | As duas credenciais que o hub acompanha por aparelho: a que vale, e a que ele acabou de emitir e ainda não foi confirmada. |
| **Troca** | O passo em que o conector apresenta a credencial que tem e recebe outra. "Gravei" é a confirmação do conector de que a nova está em disco. |
| **Em espera** | Conexão de canal que provou a identidade e aguarda: o pedido ainda não foi aprovado. |
| **Conexão estabelecida** | Conexão de canal em espera ou em sessão. É a que o hub testa antes de tratar outra como conexão dupla. |
| **Origem** | De onde vem uma conexão, para os tetos do hub: o endereço IPv4, ou os primeiros 64 bits do endereço IPv6. Não confundir com o serviço de origem. |
| **Varredura** | A tarefa do hub que, a cada 2 segundos, confere a marca, aplica os prazos e relê o estado dos aparelhos com conexão aberta. É ela que garante o revogar em menos de 5 segundos. |
| **Parada em ordem** | O jeito de o hub parar: deixa de aceitar conexões, deixa as chamadas em andamento responderem e manda `closing` a cada conexão antes de sair. |
| **Conexão curta** | Conexão do `join` ou do `leave`: prova a identidade, leva um pedido e fecha. Não é canal. |
| **Pedido em andamento** | Aparelho que ainda não confirmou a primeira credencial: pendente ou aprovado, mesmo que tenha ido à quarentena antes de confirmar. |
| **Aparelho em uso** | Aparelho que já confirmou credencial: ativo, suspenso ou em quarentena depois de ativo. |
| **Encerrado** | Aparelho em estado final: negado, vencido, substituído ou revogado. |
| **Alerta** | Aviso do painel para o admin. Não é o aviso que acorda o Hermes. |
| **Prova de restauração** | Sinal de que o banco do hub voltou no tempo: banco atrás da marca, ou lacre válido que o banco não explica. |
| **Linha do tempo perdida** | O que aconteceu entre a cópia de segurança e a restauração, e que o banco restaurado não conhece. |

## O uso

| Termo | O que é |
|---|---|
| **Acesso** | O que um Hermes pode num número. |
| **Verbo** | Cada coisa que um acesso libera: ler, enviar, modificar, ser acordado. |
| **Escopo** | Se o acesso vale para todas as conversas do número ou só para as escolhidas. |
| **Linha de conversa** | Exceção de acesso para uma conversa. |
| **Matriz** | A tela que cruza Hermes e números e mostra os acessos. |
| **Aviso** | Sinal do hub de que há mensagem nova numa conversa, ou recado novo de outro Hermes. É um ponteiro: não leva o conteúdo. |
| **Frase fixa** | O texto, guardado no conector, que acorda o Hermes. |
| **Acordar** | O conector inicia uma execução no Hermes a partir de um aviso. |
| **Auditoria** | O registro, que só cresce, do que cada um fez no hub. Sem conteúdo de mensagem. |
| **Registro do sistema** | O diário técnico do hub. |
| **Expurgo** | Apagar o que passou do prazo de guarda. |
| **Passkey** | A chave de acesso do celular ou do computador, usada no lugar da senha. |
| **Código de instalação** | Código de uso único que o hub mostra na primeira partida, para criar o primeiro admin. |

## A conversa entre Hermes

| Termo | O que é |
|---|---|
| **Recado** | Mensagem de texto de um Hermes para outro, entregue pelo hub. É dado, nunca ordem. |
| **Ligação** | Uma entrada da lista de pares: de um Hermes para outro, num sentido só. |
| **Lista de pares** | O conjunto das ligações de uma organização. Só o admin a muda. |
| **Par** | Para um Hermes, outro Hermes com o qual ele tem ligação. |
| **Fio** | Sequência de recados entre os mesmos dois Hermes, sobre o mesmo assunto. |
| **Teto** | Limite fixo de quantidade, que o hub aplica sem olhar o conteúdo. |

## Sessões

A palavra "sessão" sempre vem com o dono:

| Termo | O que é |
|---|---|
| **Sessão do canal** | Ver acima. |
| **Sessão do motor** | A ligação do motor com uma conta de WhatsApp. Quem a copia tem o número. |
| **Sessão do painel** | O login de um admin no navegador. |

## Origem

| Termo | O que é |
|---|---|
| **Serviço de origem** | O programa particular de onde o projeto nasceu. Ligava agentes Hermes ao WhatsApp por configuração manual. |
| **WAHA** | O programa de terceiro usado hoje como motor. É um cliente não oficial do WhatsApp. |
| **Noise** | Uma família conhecida de apertos de mãos para canais cifrados. |

## Palavras que não usamos

| Não usar | Usar | Motivo |
|---|---|---|
| bilhete, token, chave do Hermes | credencial lacrada | "Bilhete" já foi confundido com o convite. |
| link de conexão | convite, bloco | O convite não é um endereço: vai como argumento do conector. |
| parear um Hermes | conectar um Hermes | Parear é para números. |
| inquilino, vaga | Hermes, organização | Eram nomes do serviço de origem. |
| aparelho, para o WhatsApp | dispositivo conectado | Aparelho é a instalação do conector. |
| canal, para Telegram ou e-mail | meio de conversa | Canal é a conexão do conector com o hub. |
| cliente, no sentido de quem usa o projeto | quem instala, organização | O projeto é gratuito. O sentido técnico ("cliente não oficial", "cliente MCP") continua valendo. |
