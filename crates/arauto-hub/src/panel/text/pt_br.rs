//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "Painel"),
    ("Language", "Idioma"),
    ("Change", "Trocar"),
    ("R", "L"),
    ("S", "E"),
    ("M", "M"),
    ("W", "A"),
    ("Waiting for the device…", "Aguardando o aparelho…"),
    ("Sign in with a passkey", "Entrar com chave de acesso"),
    ("Installation code", "Código de instalação"),
    ("Continue", "Continuar"),
    ("Sign out", "Sair"),
    ("Alerts", "Alertas"),
    ("Unknown devices", "Aparelhos desconhecidos"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "Dispositivos que apresentaram uma credencial válida que o banco de dados não explica: {n}. O caminho de volta de cada um é um novo convite. Veja quais são com {command}.",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "Mais alertas: {n}. Veja todos com {command}.",
    ),
    ("Accesses", "Acessos"),
    ("none", "nenhum"),
    (
        "Your Hermes and their [numbers]",
        "Seus Hermes e os [números] deles",
    ),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "Conectados: {connected} · Aguardando: {waiting} · Em alerta: {stopped}",
    ),
    ("Connect Hermes", "Conectar Hermes"),
    ("State", "Estado"),
    ("Numbers", "Números"),
    ("Connector", "Conector"),
    ("Seen", "Visto"),
    ("Actions", "Ações"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Nenhum Hermes ainda. Conecte o primeiro com o botão \"Conectar Hermes\".",
    ),
    ("yes", "sim"),
    ("no", "não"),
    ("Connect", "Conectar"),
    ("Reconnect", "Reconectar"),
    ("Revoke", "Revogar"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "\"Números\" são os que o Hermes alcança. O conector é o que o aparelho declarou: não é verificado.",
    ),
    ("Engine", "Motor"),
    (
        "There is no engine yet. Set it with {command}.",
        "Ainda não há motor. Defina-o com {command}.",
    ),
    ("Alias", "Apelido"),
    ("Number", "Número"),
    ("Hermes with access", "Hermes com acesso"),
    ("Last message", "Última mensagem"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "Da última mensagem aparece só o horário, nunca o conteúdo. O painel lista o que passa pelo hub: outros dispositivos conectados do mesmo número só aparecem no celular.",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "Hermes nas linhas, números nas colunas. Clique em uma célula para alterá-la.",
    ),
    ("Connect a Hermes first.", "Conecte um Hermes primeiro."),
    ("Add a WhatsApp first.", "Adicione um WhatsApp primeiro."),
    ("(accesses kept)", "(acessos mantidos)"),
    ("no access", "sem acesso"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} ler · {s} enviar · {m} modificar · {w} ser acordado.",
    ),
    ("Saved: {what}", "Salvo: {what}"),
    (
        "Warnings that block nothing",
        "Avisos que não bloqueiam nada",
    ),
    // guide.rs
    ("Name", "Nome"),
    ("Code", "Código"),
    ("Done", "Pronto"),
    ("Invite issued", "Convite emitido"),
    ("A device used the invite", "Um aparelho usou o convite"),
    (
        "Code checked and request approved",
        "Código conferido e pedido aprovado",
    ),
    (
        "The connector confirmed its first credential",
        "O conector confirmou sua primeira credencial",
    ),
    (
        "A device used the invite, from {address}",
        "Um aparelho usou o convite, a partir de {address}",
    ),
    ("waiting…", "aguardando…"),
    (
        "What name will this Hermes have in the panel?",
        "Que nome este Hermes terá no painel?",
    ),
    ("Name of the Hermes", "Nome do Hermes"),
    (
        "Preparing a secure connection",
        "Preparando uma conexão segura",
    ),
    (
        "The hub drew the invite of {name}.",
        "O hub sorteou o convite de {name}.",
    ),
    (
        "Invite for {name} · valid for another {time} · until {deadline} · single use",
        "Convite para {name} · vale por mais {time} · até {deadline} · uso único",
    ),
    (
        "The Hermes reads the deadline in UTC: {utc}.",
        "O Hermes lê o prazo em UTC: {utc}.",
    ),
    ("Copied", "Copiado"),
    (
        "Click the block to copy it.",
        "Clique no bloco para copiá-lo.",
    ),
    (
        "Select the whole text and copy it.",
        "Selecione o texto inteiro e copie.",
    ),
    (
        "It only appears here, once.",
        "Ela só aparece aqui, uma vez.",
    ),
    (
        "The invite issued before was cancelled.",
        "O convite emitido antes foi cancelado.",
    ),
    (
        "The request that was in progress was closed.",
        "O pedido que estava em andamento foi encerrado.",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "O aparelho em uso continua funcionando até o novo confirmar sua primeira credencial.",
    ),
    (
        "This hub has no connector to serve",
        "Este hub não tem conector para entregar",
    ),
    (
        "Waiting for the Hermes {name}",
        "Aguardando o Hermes {name}",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "Cada passo acende quando o hub de fato o observa. Se o Hermes pedir aprovação para executar o comando, quem aprova é você, no canal de comando.",
    ),
    (
        "A device used the invite of {name}",
        "Um aparelho usou o convite de {name}",
    ),
    ("unknown", "desconhecido"),
    ("Seen by the hub", "Visto pelo hub"),
    ("address {address}, {when}", "endereço {address}, {when}"),
    ("Said by the device", "Dito pelo aparelho"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "máquina \"{host}\" · sistema \"{os}\" · Hermes \"{version}\" (não verificado)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "Último uso do aparelho atual: {when}, a partir de {address}. Você o reconhece?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "Último uso do aparelho atual: {when}. Você o reconhece?",
    ),
    ("I recognize it", "Reconheço"),
    ("I do not recognize it", "Não reconheço"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Se o Hermes estava desligado e houve uso, alguém usou uma cópia.",
    ),
    ("Verification code", "Código de conferência"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Digite o código que o Hermes respondeu no canal de comando. O painel nunca mostra esse código.",
    ),
    (
        "The request of {name} was denied",
        "O pedido de {name} foi negado",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "O código foi digitado errado pela quinta vez. Para conectar, emita outro convite.",
    ),
    (
        "The request of {name} expired",
        "O pedido de {name} expirou",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "Os 10 minutos para digitar o código passaram. Para conectar, emita outro convite.",
    ),
    (
        "The identity of {name} was copied",
        "A identidade de {name} foi copiada",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "Dois aparelhos apareceram com a mesma identidade antes da aprovação, e o pedido foi interrompido. Para conectar, emita outro convite.",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} não tem convite nem pedido aguardando",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "O convite expirou, foi cancelado, ou o pedido já foi resolvido. Se o Hermes não respondeu nenhum código, não aprove; se respondeu AR-102, outro aparelho usou o convite. Para conectar, emita outro convite.",
    ),
    ("{name} is connected", "{name} está conectado"),
    ("{name} was approved", "{name} foi aprovado"),
    (
        "What it may do on each number is in the Accesses tab.",
        "O que ele pode fazer em cada número está na aba Acessos.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "Assim que o conector confirmar, ele aparece como conectado. Os acessos que ele já tinha voltam a valer para este aparelho.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "Assim que o conector confirmar, ele aparece como conectado. Ele ainda não tem acesso a nenhum número.",
    ),
    (
        "The hub did not do what was asked",
        "O hub não fez o que foi pedido",
    ),
    ("Issue invite", "Emitir convite"),
    ("Cancel", "Cancelar"),
    ("Next step", "Próximo passo"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "Depois de copiar, cole no canal de comando do Hermes {name}. Em nenhum outro lugar.",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "O convite vale por mais {time}. Esta tela se atualiza sozinha.",
    ),
    ("Approve", "Aprovar"),
    ("Deny", "Negar"),
    ("Confirm within {time}.", "Confirme em até {time}."),
    ("Issue another invite", "Emitir outro convite"),
    ("Close", "Fechar"),
    ("See accesses", "Ver acessos"),
    ("Finish", "Concluir"),
    (
        "Copied to the clipboard.",
        "Copiado para a área de transferência.",
    ),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "O texto está selecionado. Copie com Ctrl+C.",
    ),
    ("Reconnect {name}", "Reconectar {name}"),
    ("Connect {name}", "Conectar {name}"),
    ("Steps", "Passos"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "Ser acordado exige ler. Nada foi salvo.",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "Iniciar conversas exige enviar e o escopo \"todas as conversas\". Nada foi salvo.",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "A espera antes de acordar vai de 0 a 60 segundos. Nada foi salvo.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "Algo falhou no hub. Veja o registro do hub e confira o acesso na matriz.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "O banco de dados voltou de uma cópia de segurança mais antiga e o hub ainda não tratou isso. Nada foi gravado. Reinicie o hub.",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "O Hermes {hermes} não tem acesso ao número {number}.",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "O Hermes {hermes} só alcança as conversas escolhidas do número {number}. Conversas com linha: {lines}.",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "O Hermes {hermes} no número {number}, em todas as conversas:",
    ),
    ("reads", "lê"),
    ("does not read", "não lê"),
    ("sends", "envia"),
    ("does not send", "não envia"),
    ("modifies", "modifica"),
    ("does not modify", "não modifica"),
    (
        "is woken after {seconds} s of silence",
        "é acordado após {seconds} s de silêncio",
    ),
    ("is not woken", "não é acordado"),
    ("may start chats", "pode iniciar conversas"),
    (
        "chats with their own rule: {lines}",
        "conversas com regra própria: {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} no número {number}",
    ),
    ("Where it applies", "Onde vale"),
    (
        "Every chat, including the ones that come later",
        "Todas as conversas, inclusive as que vierem depois",
    ),
    ("Only chosen chats", "Só conversas escolhidas"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "Com \"só conversas escolhidas\", só as conversas que têm linha própria existem para o Hermes, e o que ele pode fazer em cada uma vem da linha. As linhas ainda são criadas com o comando {command}. Hoje: {lines}.",
    ),
    ("What it may do", "O que ele pode fazer"),
    ("Read", "Ler"),
    (
        "see chats and messages, search, download files",
        "ver conversas e mensagens, buscar, baixar arquivos",
    ),
    ("Send", "Enviar"),
    (
        "write, react, mark as read",
        "escrever, reagir, marcar como lida",
    ),
    ("Modify", "Modificar"),
    (
        "edit and delete what already exists",
        "editar e apagar o que já existe",
    ),
    ("Be woken", "Ser acordado"),
    (
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "o hub avisa quando chega uma mensagem (exige ler). Desligar também desliga nos chats deste acesso que têm regra própria.",
    ),
    ("Wait", "Espera"),
    (
        "seconds of silence before waking",
        "segundos de silêncio antes de acordar",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "Pode iniciar uma conversa com quem nunca falou com o número (exige enviar e todas as conversas)",
    ),
    ("Today:", "Hoje:"),
    ("Save", "Salvar"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "Vale a partir da próxima chamada, sem nada para reiniciar. Sem nenhum verbo marcado, o acesso é removido.",
    ),
    ("Revoke {name}", "Revogar {name}"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "Revogar encerra o Hermes no hub: o aparelho em uso, o pedido em andamento e o convite emitido. O canal cai em segundos.",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "Os acessos são mantidos e voltam a valer quando um novo aparelho for conectado. O único caminho de volta é um novo convite.",
    ),
    // data.rs
    ("never", "nunca"),
    ("now", "agora"),
    ("{n} min ago", "há {n} min"),
    ("{n} h ago", "há {n} h"),
    ("{n} d ago", "há {n} d"),
    (
        "a credential that had already been swapped was presented",
        "foi apresentada uma credencial que já tinha sido trocada",
    ),
    (
        "two connections at the same time",
        "duas conexões ao mesmo tempo",
    ),
    (
        "the credential could not be read",
        "não foi possível ler a credencial",
    ),
    (
        "the last use was not recognized on the reconnect",
        "o último uso não foi reconhecido na reconexão",
    ),
    (
        "the identity was copied before the approval",
        "a identidade foi copiada antes da aprovação",
    ),
    (
        "the hub no longer trusts the credential",
        "o hub não confia mais na credencial",
    ),
    ("Waiting for your code", "Aguardando o seu código"),
    (
        "Approved, waiting for the connector",
        "Aprovado, aguardando o conector",
    ),
    ("Waiting for the Hermes", "Aguardando o Hermes"),
    ("Connected", "Conectado"),
    ("Disconnected", "Desconectado"),
    ("In quarantine", "Em quarentena"),
    ("Suspended", "Suspenso"),
    ("No device", "Sem aparelho"),
    ("a Hermes", "um Hermes"),
    ("Invite", "Convite"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "outro aparelho tentou usar o convite de {hermes}. O bloco foi parar em outro lugar. O primeiro pedido continua valendo.",
    ),
    ("Identity copied", "Identidade copiada"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "a identidade de {hermes} foi copiada antes da aprovação. O pedido foi negado.",
    ),
    ("Hermes in quarantine", "Hermes em quarentena"),
    ("{hermes}. Reason: {reason}.", "{hermes}. Motivo: {reason}."),
    ("Suspected copy", "Suspeita de cópia"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "o último uso de {hermes} não foi reconhecido na reconexão.",
    ),
    ("Connection taken", "Conexão tomada"),
    (
        "the connection of {hermes} was taken by another one.",
        "a conexão de {hermes} foi tomada por outra.",
    ),
    ("Closing", "Encerramento"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} pediu o próprio encerramento.",
    ),
    ("Reuse", "Reuso"),
    (
        "a copy of {hermes} was used before the closing.",
        "uma cópia de {hermes} foi usada antes do encerramento.",
    ),
    ("Unconfirmed swaps", "Trocas sem confirmação"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} recebeu credenciais e não as confirmou, três vezes em 24 horas. Pode ser uma rede ruim, ou alguém tentando usar uma cópia.",
    ),
    ("Audit mark", "Marca da auditoria"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "um evento de segurança foi gravado e a marca fora do banco de dados não acompanhou.",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "a versão do motor não é uma que o hub tenha conferido.",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "uma sessão do motor tinha outra configuração, e o hub aplicou a dele.",
    ),
    (
        "the engine refused the key of the hub. Restart the engine: it reads its key only when it starts.",
        "o motor recusou a chave do hub. Reinicie o motor: ele só lê a chave quando liga.",
    ),
    (
        "the engine refused the key of the hub. Set its key again with {command}.",
        "o motor recusou a chave do hub. Defina a chave dele de novo com {command}.",
    ),
    ("Number swapped", "Número trocado"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "o celular que pareou pertence a um número diferente do informado. Os acessos ficam parados até você decidir.",
    ),
    ("Address: {address}.", "Endereço: {address}."),
    ("an unknown date", "uma data desconhecida"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "O hub voltou de uma cópia de segurança de {date}. O que foi aprovado, revogado, reconectado ou alterado nos acessos depois disso se perdeu. Reconecte cada Hermes e confira os acessos.",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "O hub voltou de uma cópia de segurança. {name} está suspenso e só volta com um novo convite: reconecte ou revogue. Não existe \"reativar\".",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "O hub não confia mais na credencial de {name}. Motivo: {reason}. O acesso dele está parado. Saídas: reconectar ou revogar. Não existe \"liberar\".",
    ),
    ("Not paired", "Não pareado"),
    ("Pairing", "Pareando"),
    ("Reconnecting", "Reconectando"),
    ("Failed", "Falhou"),
    ("Removed", "Removido"),
    ("version {version}, checked", "versão {version}, conferida"),
    (
        "version {version}, not checked by the hub",
        "versão {version}, não conferida pelo hub",
    ),
    ("version not read yet", "versão ainda não lida"),
    ("last answer {when}", "última resposta {when}"),
    ("has not answered yet", "ainda não respondeu"),
    ("all", "todas"),
    ("all · own rules: {n}", "todas · regras próprias: {n}"),
    ("chosen chats: {n}", "conversas escolhidas: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: {n} Hermes são acordados nas mesmas conversas.",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: {n} Hermes enviam nas mesmas conversas.",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "Já existe um Hermes com esse nome. Para ligar outro aparelho a ele, use o botão na linha dele.",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "Esse nome pertenceu a um Hermes que foi removido, e um nome nunca é reutilizado. Escolha outro.",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "Antes do código, diga se você reconhece o último uso do aparelho atual. Nada foi contado.",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "O aparelho atual foi usado enquanto você respondia. Veja agora o último uso dele e responda de novo. Nada foi aprovado.",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "O código tem 6 dígitos, como 123-456. Nada foi contado.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "O banco de dados voltou de uma cópia de segurança mais antiga e o hub ainda não tratou isso. Nada foi gravado. Reinicie o hub: ele liga o modo restaurado, e depois cada Hermes é reconectado com um novo convite.",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "Algo falhou no hub e nada foi feito. Veja o registro do hub.",
    ),
    (
        "The name has from 1 to 32 characters.",
        "O nome tem de 1 a 32 caracteres.",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "O nome começa com uma letra minúscula ou um dígito.",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "Use apenas letras minúsculas, dígitos, hífen e sublinhado.",
    ),
    (
        "That name is reserved. Choose another.",
        "Esse nome é reservado. Escolha outro.",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "O código não confere. Tentativa {attempts} de {max}. Digite o último código que o Hermes respondeu.",
    ),
    // mod.rs
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Tentativas demais agora. Espere um minuto e tente de novo.",
    ),
    // first access, installation check, connect guide
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "Esse código não funcionou. Use o mais recente: o do último bloco do log, ou um emitido no servidor do hub.",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "O hub não aceitou essa chave de acesso. Tente de novo.",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "Sua sessão do painel terminou. Digite o código mais recente para continuar de onde parou.",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "Este navegador não consegue usar uma chave de acesso aqui. Abra o painel num navegador atualizado, direto no endereço dele (e não dentro de outro aplicativo).",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "A chave não foi criada: o pedido foi cancelado ou demorou demais. É só tentar de novo.",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "Nenhuma chave de acesso foi usada. Se ela não está neste aparelho, use o aparelho ou o gerenciador de senhas onde você a criou, ou abra “Perdeu a chave de acesso?”.",
    ),
    ("Passkey", "Chave de acesso"),
    ("Check", "Conferir"),
    (
        "Welcome to your Arauto hub",
        "Boas-vindas ao seu hub do Arauto",
    ),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "Vamos configurar o seu primeiro acesso. São três passos rápidos, e você só faz isso uma vez.",
    ),
    (
        "Step 1 · Confirm it is you",
        "Passo 1 · Confirme que é você",
    ),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "Quando o hub ligou, ele escreveu um código de instalação no próprio log. Só quem cuida do servidor vê esse log: é assim que o hub sabe que é você.",
    ),
    ("Where to find the code:", "Onde achar o código:"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "No Easypanel, abra o serviço do hub e vá em Logs. (No Docker: {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "Ache o bloco ARAUTO FIRST ACCESS: o mais recente é o último, no fim do log. Copie o código dele.",
    ),
    ("Paste the code here:", "Cole o código aqui:"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "O código não vence e funciona uma vez só. Se o hub reiniciar, ele escreve um novo: use sempre o mais recente.",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "Não consegue abrir o log? No servidor do hub, rode {command}. Atenção: esse código novo cancela o do log.",
    ),
    ("Good to have you back", "Que bom ter você de volta"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "Entre com a sua chave de acesso: a digital, o rosto ou o PIN deste aparelho, ou o seu gerenciador de senhas.",
    ),
    ("Lost your passkey?", "Perdeu a chave de acesso?"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "Sem problema. No servidor do hub, rode {command}. Ele mostra um código que vale por 10 minutos: digite aqui e crie uma chave nova.",
    ),
    (
        "Code accepted! Now, create your passkey",
        "Código aceito! Agora, crie a sua chave de acesso",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "É com ela que você vai entrar no painel daqui em diante, sem senha para lembrar.",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "A chave fica neste aparelho ou no seu gerenciador de senhas, e abre com a sua digital, o seu rosto ou o PIN do aparelho. O hub guarda só o que confere a chave: não existe senha para vazar.",
    ),
    (
        "It will work at this address:",
        "Ela vai funcionar neste endereço:",
    ),
    ("Create my passkey", "Criar minha chave de acesso"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "Se você usa um gerenciador de senhas, a chave pode ir junto para os seus outros aparelhos.",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "Saiu desta página sem querer? Digite o mesmo código de novo: ele só é gasto quando a chave é criada.",
    ),
    ("Check the installation", "Conferir a instalação"),
    ("How it works:", "Como funciona:"),
    (
        "You give the Hermes a name here.",
        "Você dá um nome para o Hermes aqui.",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "O Hermes baixa o conector deste hub e responde com um código de conferência de 6 dígitos.",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "Você digita o código aqui e aprova. Pronto: o Hermes aparece como conectado assim que o conector dele confirmar.",
    ),
    (
        "Let's connect your first Hermes",
        "Vamos conectar o seu primeiro Hermes",
    ),
    ("ok", "ok"),
    ("check", "confira"),
    ("attention", "atenção"),
    ("later", "depois"),
    ("failed", "falhou"),
    ("Panel address", "Endereço do painel"),
    ("Connectors address", "Endereço dos conectores"),
    ("Your address", "Seu endereço"),
    ("Database password", "Senha do banco"),
    ("Data folder", "Pasta de dados"),
    ("WhatsApp engine", "Motor do WhatsApp"),
    ("Your passkey works here.", "Sua chave funciona aqui."),
    (
        "Open it: it should say it is the address of the connectors.",
        "Abra: ele deve dizer que é o endereço dos conectores.",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "O hub vê você em {address}. É o seu? Se não for, o proxy não repassa o endereço real.",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Uma conexão de {address}, que não está em ARAUTO_TRUSTED_PROXIES, trouxe um endereço repassado. Acrescente esse endereço a ARAUTO_TRUSTED_PROXIES e implante de novo.",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "O hub vê você em {address}, um endereço de rede interna: o proxy ainda não é confiável. Acrescente esse endereço a ARAUTO_TRUSTED_PROXIES e implante de novo.",
    ),
    (
        "Replaced by one nobody saw.",
        "Trocada por uma que ninguém viu.",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "Não trocada pelo hub: esta instalação define a senha do banco por conta própria.",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "A troca falhou. Veja o log do hub e depois rode {command}.",
    ),
    ("Ready for your Hermes.", "Pronto para os seus Hermes."),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "A pasta de downloads está vazia: não dá para conectar um Hermes pelo painel. Reconstrua a imagem do hub.",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "Guarda a chave mestra e a senha do banco. Faça backup: sem ela, o hub não abre o que guardou cifrado.",
    ),
    ("Configured.", "Configurado."),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "Dá para conectar um Hermes sem ele; os números de WhatsApp precisam dele.",
    ),
    (
        "Step 3 · Let's check the installation",
        "Passo 3 · Vamos conferir a instalação",
    ),
    (
        "All set: your passkey was created!",
        "Tudo certo: sua chave de acesso foi criada!",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "O hub verificou algumas coisas sozinho. Nada aqui muda nada: é só para você saber como ficou.",
    ),
    (
        "The same by command: {command}",
        "O mesmo por comando: {command}",
    ),
    ("Connect my first Hermes", "Conectar meu primeiro Hermes"),
    ("Do it later", "Fazer isso depois"),
    // block.rs, the connection sentence
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "Eu mesmo estou pedindo: conecte este Hermes ao nosso hub Arauto como \"{name}\".",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "Leia {page} e siga as instruções. Se algo não ficar claro, me pergunte.",
    ),
    ("Hub:", "Hub:"),
    ("Hub key:", "Chave do hub:"),
    (
        "Connector sha256 ({system}):",
        "sha256 do conector ({system}):",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "Quando terminar, me envie o código de verificação e as linhas que a página pedir.",
    ),
    // guide.rs, the message and its fallback
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "Envie como uma mensagem nova, não como resposta a um dos avisos dele. Se ele pedir confirmação, responda com suas próprias palavras.",
    ),
    ("Show the command instead", "Mostrar o comando em vez disso"),
    (
        "Send one or the other, not both: they carry the same invite.",
        "Envie um ou outro, não os dois: os dois levam o mesmo convite.",
    ),
    (
        "Click the message to copy it.",
        "Clique na mensagem para copiá-la.",
    ),
    ("Message", "Mensagem"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "O hub gera um convite e escreve uma mensagem com ele, que você envia no canal de comando desse Hermes, e em nenhum outro lugar.",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "Letras minúsculas, dígitos, hífen e sublinhado, até 32 caracteres, começando por letra ou dígito. O nome aparece na mensagem e não muda depois.",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "A mensagem manda o Hermes baixar o conector deste hub, e a pasta de downloads está vazia. Nada foi emitido. Emita o convite de {name} com o comando {command}, no servidor do hub.",
    ),
    // org.rs
    ("Org chart", "Organograma"),
    ("Who talks to [whom]", "Quem fala com [quem]"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "Arraste um Hermes para mudar o nível. Puxe a bolinha roxa de um cartão até outro para criar uma ligação. Clique numa seta ou num cartão para ajustar.",
    ),
    ("View", "Visão"),
    ("Map", "Mapa"),
    ("List", "Lista"),
    ("+ Level", "+ Nível"),
    ("Undo", "Desfazer"),
    ("Publish", "Publicar"),
    ("Draft: 1 change", "Rascunho: 1 mudança"),
    ("Draft: {n} changes", "Rascunho: {n} mudanças"),
    (
        "From a higher level to a lower one: instruction",
        "De cima para baixo: instrução",
    ),
    (
        "From a lower level to a higher one: request",
        "De baixo para cima: pedido",
    ),
    ("Same level: note", "Mesmo nível: recado"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "Os níveis e os papéis só aparecem aqui: nenhum Hermes fica sabendo deles. Um recado é dado, exceto em uma seta de instrução com a marca de seguir: nesse caso o aviso diz ao Hermes de baixo que siga essas mensagens novas do de cima, naquela execução, abaixo das instruções do seu operador. Só uma ligação permite que um Hermes mande um recado a outro.",
    ),
    ("instruction", "instrução"),
    ("request", "pedido"),
    ("note", "recado"),
    ("Name of level {n}", "Nome do nível {n}"),
    ("Remove empty level", "Remover nível vazio"),
    ("Move up", "Subir"),
    ("Move down", "Descer"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "Um nome de nível tem de 1 a 32 letras minúsculas, dígitos, hífens ou sublinhados, começa com letra ou dígito e não é uma palavra reservada.",
    ),
    (
        "Another level already has this name.",
        "Outro nível já tem este nome.",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "O organograma tem de 1 a {max} níveis.",
    ),
    (
        "Drag to another Hermes to create a link",
        "Puxe até outro Hermes para criar uma ligação",
    ),
    ("{alias} · answers", "{alias} · atende"),
    ("{alias} · reads only", "{alias} · só lê"),
    ("no number", "sem número"),
    ("not placed yet", "ainda sem lugar"),
    (
        "{name}, level {level}, {state}",
        "{name}, nível {level}, {state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "Ainda sem lugar: {names}. Eles aparecem no nível mais baixo; publique para que fiquem lá.",
    ),
    ("Kind: {kind}", "Tipo: {kind}"),
    ("Wakes {name}", "Acorda {name}"),
    ("Create the way back", "Criar a volta"),
    ("Remove the way back", "Remover a volta"),
    ("Remove", "Remover"),
    ("This link already exists.", "Esta ligação já existe."),
    ("Role of this Hermes", "Papel deste Hermes"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "Para a sua leitura: nesta versão nenhum Hermes o vê. Até 280 caracteres, em uma linha.",
    ),
    ("Level", "Nível"),
    ("Move left", "Mover para a esquerda"),
    ("Move right", "Mover para a direita"),
    ("Change them in Accesses", "Altere em Acessos"),
    ("No access to any number.", "Sem acesso a nenhum número."),
    ("Link to", "Ligar a"),
    ("Add link", "Criar ligação"),
    ("Links of this Hermes", "Ligações deste Hermes"),
    ("Levels", "Níveis"),
    ("Links", "Ligações"),
    ("Role", "Papel"),
    ("From", "De"),
    ("To", "Para"),
    ("Kind", "Tipo"),
    ("Wakes", "Acorda"),
    ("nobody", "ninguém"),
    ("No link.", "Nenhuma ligação."),
    ("Publish these changes?", "Publicar estas mudanças?"),
    (
        "Links are security rules. They only apply after you publish.",
        "As ligações são regras de segurança. Elas só passam a valer depois de publicadas.",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "Uma ligação deixa um pedido atravessar números: o que chega a um Hermes por um número pode virar um recado para o outro, que age nos números dele.",
    ),
    ("Back", "Voltar"),
    (
        "New link: {from} → {to} ({kind})",
        "Ligação nova: {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "Ligação nova: {from} → {to} ({kind}), acorda {to}",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "Ligação {from} → {to}: passa a acordar {to}",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "Ligação {from} → {to}: deixa de acordar {to}",
    ),
    (
        "Link removed: {from} → {to}",
        "Ligação removida: {from} → {to}",
    ),
    (
        "{name} changes level: {before} → {after}",
        "{name} muda de nível: {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} fica no nível {after}",
    ),
    ("Role of {name} changed", "Papel de {name} alterado"),
    (
        "Places on the map changed: {n}",
        "Posições no mapa alteradas: {n}",
    ),
    ("New level: {name}", "Nível novo: {name}"),
    (
        "Level renamed: {before} → {after}",
        "Nível renomeado: {before} → {after}",
    ),
    ("Level removed: {name}", "Nível removido: {name}"),
    ("Order of the levels: {names}", "Ordem dos níveis: {names}"),
    ("Published.", "Publicado."),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "Alguém mudou o organograma enquanto você editava: outra publicação, uma ligação alterada pelo comando, ou um Hermes criado ou removido. Nada foi publicado. O seu rascunho foi mantido: confira as mudanças e publique de novo.",
    ),
    ("Nothing was published.", "Nada foi publicado."),
    (
        "Two levels have the same name.",
        "Dois níveis têm o mesmo nome.",
    ),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "Um papel tem no máximo 280 caracteres, em uma linha, sem caracteres de controle ou de direção.",
    ),
    (
        "A place on the map is out of range.",
        "Uma posição no mapa está fora do limite.",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "O rascunho cita um Hermes ou um nível que não existe, ou deixa um Hermes de fora.",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "Uma ligação une dois Hermes diferentes, uma vez em cada sentido.",
    ),
    (
        "The draft could not be read.",
        "O rascunho não pôde ser lido.",
    ),
    (
        "The draft is too large to publish.",
        "O rascunho é grande demais para publicar.",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "Algo falhou no hub. Nada foi publicado. Veja o log do hub.",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "Para editar o organograma é preciso o script do painel. Sem ele, este é o organograma como está publicado.",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "Nenhum Hermes ainda. Conecte o primeiro na aba Hermes.",
    ),
    // numbers.rs, pairing.rs
    ("Connect WhatsApp (WAHA)", "Conectar WhatsApp (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "Conectar WhatsApp (nativo - Em teste)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "Nenhum número ainda. Adicione o primeiro com o botão \"Conectar WhatsApp (WAHA)\".",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "Nesta versão o painel conecta e revoga Hermes, pareia e gerencia números de WhatsApp no WAHA, muda os acessos de cada número e edita o organograma com suas ligações. Escolher conversas ainda se faz com o comando {command}, no servidor do hub.",
    ),
    ("Pair", "Parear"),
    ("Pair again", "Parear de novo"),
    ("Continue pairing", "Continuar o pareamento"),
    ("See the pairing", "Ver o pareamento"),
    ("Disconnect", "Desconectar"),
    ("Restart", "Reiniciar"),
    ("Decide", "Decidir"),
    ("Number {alias}: {state}.", "Número {alias}: {state}."),
    (
        "Number {alias} was removed.",
        "O número {alias} foi removido.",
    ),
    ("Telephone given", "Telefone informado"),
    ("Telephone that paired", "Telefone que pareou"),
    ("Undo…", "Desfazer…"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "O hub mudou o número {alias}, mas o WAHA não respondeu: a sessão dele lá pode ter ficado como estava. Confira o WAHA com {command} no servidor do hub.",
    ),
    ("Method", "Método"),
    ("Phone", "Celular"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "Como o painel e os Hermes chamam este número: letras minúsculas, dígitos, hífen e sublinhado, até 32. Um apelido nunca é reusado.",
    ),
    (
        "Number with the country code",
        "Número com o código do país",
    ),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "Como +55 11 9XXXX-XXXX. Só dígitos, espaços, hífens e parênteses.",
    ),
    ("Before the first number", "Antes do primeiro número"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "O motor é um cliente não oficial do WhatsApp. O WhatsApp pode bloquear o número. Use números que você pode perder.",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "Entendo e aceito. O hub registra este aceite na auditoria.",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "O número é pareado pelo WAHA, o motor definido neste hub.",
    ),
    ("Add and pair", "Adicionar e parear"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "O WAHA ainda não respondeu ao hub. Você pode adicionar o número; parear precisa que o WAHA responda.",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "Ainda não há motor de WhatsApp neste hub. Defina o WAHA no servidor do hub com {command} e, depois, a chave dele com {key_command}. A chave nunca é digitada no painel.",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "O motor ainda não tem chave. Defina-a no servidor do hub com {command}. A chave nunca é digitada no painel.",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "O apelido tem de 1 a 32 caracteres.",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "O apelido começa com uma letra minúscula ou um dígito.",
    ),
    (
        "That alias is reserved. Choose another.",
        "Esse apelido é reservado. Escolha outro.",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "Esse apelido já está em uso, agora ou por um número que foi removido, e um apelido nunca é reusado. Escolha outro.",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "Digite o número com o código do país: de 8 a 15 dígitos, como +55 11 9XXXX-XXXX.",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "Esse número já é um WhatsApp deste hub.",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "Leia a frase sobre o risco e marque que você a aceita. Nada foi adicionado.",
    ),
    ("Pair {alias}", "Parear {alias}"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "Tenha à mão o celular de {phone}, com o WhatsApp aberto. O pareamento dura até {minutes} minutos.",
    ),
    ("How will the phone pair?", "Como o celular vai parear?"),
    ("QR code", "Código QR"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "escaneie com a câmera do celular, de dentro do WhatsApp",
    ),
    ("Pairing code", "Código de pareamento"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "digite um código no celular, para quando ele não consegue escanear esta tela",
    ),
    ("Start pairing", "Começar o pareamento"),
    (
        "{alias} is in the list now. Pair it now, or later from its line.",
        "{alias} já está na lista. Pareie agora ou depois, pela linha dele.",
    ),
    ("Pair later", "Parear depois"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "O WAHA ainda não criou a sessão deste número. Começar o pareamento tenta de novo.",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "Escolha código QR ou código de pareamento. Nada foi começado.",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "{n} números estão sendo pareados agora, o máximo ao mesmo tempo. Espere um terminar ou cancele um.",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "Isso não pode ser feito enquanto o número está no estado \"{state}\". Nada foi feito.",
    ),
    ("Starting the pairing…", "Começando o pareamento…"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "Escaneie com o celular: WhatsApp › Dispositivos conectados › Conectar dispositivo.",
    ),
    (
        "QR code to scan with the phone",
        "Código QR para escanear com o celular",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "O QR muda a cada poucos segundos: esta janela mostra o atual.",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "No celular: WhatsApp › Dispositivos conectados › Conectar dispositivo › Conectar com número de telefone, e digite este código:",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "O código vale enquanto esta janela conta o tempo: faltam {time}. O WhatsApp pode encerrá-lo antes.",
    ),
    ("Time left to scan: {time}.", "Tempo para escanear: {time}."),
    (
        "This window refreshes by itself.",
        "Esta janela se atualiza sozinha.",
    ),
    ("Refresh now", "Atualizar agora"),
    ("Cancel the pairing", "Cancelar o pareamento"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "O QR é longo demais para desenhar aqui. Cancele e pareie com um código de pareamento.",
    ),
    ("Paired: {phone}.", "Pareado: {phone}."),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "O número {alias} está conectado. Dê acesso a ele a um Hermes na aba Acessos.",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "Nada foi escaneado ou digitado a tempo. O número não está pareado; pareie de novo com o celular à mão.",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "O WAHA informou uma falha da sessão. O número não está pareado. Tente de novo; se falhar outra vez, confira o WAHA.",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "O pareamento foi cancelado. O número não está pareado.",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "Este número está sendo pareado em outra janela. Só aquela janela mostra o QR ou o código.",
    ),
    ("Cancel that pairing", "Cancelar aquele pareamento"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "Este pareamento não está mais rodando no hub: terminou, ou o hub reiniciou. O número está no estado \"{state}\".",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "O hub não alcançou o WAHA. Confira se ele está rodando e na rede do hub.",
    ),
    (
        "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
        "O WAHA recusou a chave que o hub tem. Se a instalação trouxe o motor, reinicie o motor: ele só lê a chave quando liga. Se você configurou o motor por conta própria, defina a chave certa no servidor do hub com {command}.",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "O WAHA respondeu com erro, ou fora do tempo. Tente de novo daqui a pouco; {command} no servidor do hub diz mais.",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "O WhatsApp está limitando este número. Tente de novo mais tarde.",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "O WAHA recusou o pedido. Veja o registro do hub.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "Algo falhou no hub. Veja o registro do hub e confira o número na lista.",
    ),
    (
        "{alias}: another phone paired",
        "{alias}: outro celular pareou",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "O celular que pareou é de um número diferente do informado.",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "Até você decidir, o número fica no estado \"Número trocado\" e os acessos dele ficam parados.",
    ),
    ("Keep this one", "Manter este"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "Desfazer tira o WAHA do WhatsApp daquele celular (ele sai dos dispositivos conectados) e apaga o histórico que o WAHA guarda deste número.",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "Entendo que o WAHA apaga o histórico dele deste número.",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "Marque que você entende o que desfazer apaga. Nada foi feito.",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "O celular que pareou já é outro número deste hub. Desfaça este pareamento, ou remova o outro número antes.",
    ),
    ("Disconnect {alias}", "Desconectar {alias}"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "Desconectar para a sessão deste número no WAHA. Não tira o WAHA do WhatsApp, e o WAHA guarda o histórico. Enquanto estiver desconectado, nenhum Hermes lê ou envia por ele. Para usar de novo, pareie de novo.",
    ),
    (
        "Type the alias {alias} to confirm.",
        "Digite o apelido {alias} para confirmar.",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "O apelido digitado não confere. Nada foi feito.",
    ),
    ("Remove {alias}", "Remover {alias}"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "Remover encerra agora todos os acessos a este número. O apelido dele nunca mais é usado.",
    ),
    (
        "What happens to its session in WAHA",
        "O que acontece com a sessão dele no WAHA",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "Parar e manter no WAHA, com o histórico que o WAHA guarda",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "Tirar o WAHA do WhatsApp e apagar: o WAHA apaga o histórico que guarda",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "O Arauto ainda não tem arquivo de mensagens: o histórico é só o que o WAHA guarda.",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "Escolha o que acontece com a sessão dele no WAHA. Nada foi feito.",
    ),
    ("In test", "Em teste"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Um motor de WhatsApp embutido no Arauto está em estudo. Ele não faz parte desta versão, e esta janela não faz nada.",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "Por enquanto, os números são pareados pelo WAHA, com o botão \"Conectar WhatsApp (WAHA)\".",
    ),
    ("Open the WAHA window", "Abrir a janela do WAHA"),
    ("Try again", "Tentar de novo"),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "Com esta marca, um recado nesta ligação acorda {name}, dentro dos limites de aviso.",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} segue as instruções de {from}",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "Com esta marca, o aviso de um recado diz a {to} que faça o que {from} pedir naquela execução, sem esperar o operador, a menos que o operador proíba. Só em uma seta de instrução, só com a marca de acordar; {to} precisa da ligação de volta para responder. É uma frase dirigida a uma IA, não um controle.",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "Ligação {from} → {to}: {to} passa a seguir as instruções de {from}",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "Ligação {from} → {to}: {to} deixa de seguir as instruções de {from}",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "Um Hermes que segue outro faz o que ele pede sem esperar o operador. Quem enganar o Hermes de cima, pelo WhatsApp ou por um recado, alcança também o de baixo; quem controla o hub pode escrever como qualquer Hermes; e quem tem uma sessão do painel pode definir estas marcas. Um conector de versão mais antiga continua tratando esses recados como dados.",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "Uma marca de seguir vai só de um nível mais alto para um mais baixo, em uma ligação que acorda.",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "Quem dá ordens a ele"),
    ("not recorded", "não registrado"),
    ("its own chat", "a própria conversa"),
    ("{n} telephones", "{n} telefones"),
    ("1 telephone", "1 telefone"),
    ("Orders", "Ordens"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "Quem dá ordens a {hermes} no WhatsApp",
    ),
    (
        "This is the number {alias} in Arauto.",
        "Este é o número {alias} no Arauto.",
    ),
    ("Orders come from", "As ordens vêm de"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "O próprio WhatsApp dele, na conversa consigo mesmo (modo self-chat)",
    ),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "O Arauto recusa que qualquer Hermes, inclusive {hermes}, escreva na conversa em que {hermes} recebe essas ordens. Quem lê segue os acessos.",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "Se nada for registrado aqui, o Arauto não bloqueia nada: um Hermes que consiga escrever na conversa em que {hermes} recebe ordens poderia se passar por você, ou por quem as dá.",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} já recebe ordens neste telefone.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "Algo falhou no hub. Veja o registro do hub e confira o que está gravado para o Hermes.",
    ),
    (
        "A Hermes could give {hermes} orders",
        "Um Hermes poderia dar ordens a {hermes}",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "Nada está bloqueado: qualquer Hermes que consiga escrever na conversa em que {hermes} recebe ordens pode se passar por quem as dá.",
    ),
    ("Go back", "Voltar"),
    ("gives orders to {hermes}", "dá ordens a {hermes}"),
    ("{hermes} takes orders here", "{hermes} recebe ordens aqui"),
    (
        "Orders on WhatsApp for {alias}",
        "Ordens no WhatsApp para {alias}",
    ),
    ("Its WhatsApp", "O WhatsApp dele"),
    (
        "The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.",
        "O telefone em que o próprio WhatsApp de {hermes} funciona: {hermes} recebe ordens ali.",
    ),
    ("It has no WhatsApp", "Ele não tem WhatsApp"),
    ("A number of Arauto", "Um número do Arauto"),
    ("Choose a number", "Escolha um número"),
    ("Another telephone", "Outro telefone"),
    (
        "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
        "Lido só quando Outro telefone está escolhido. Comece com + e o código do país, como +55 11 9XXXX-XXXX.",
    ),
    ("These telephones", "Estes telefones"),
    ("its WhatsApp", "o WhatsApp dele"),
    ("Other telephones", "Outros telefones"),
    (
        "Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.",
        "Telefones que não são números do Arauto, um por linha. No máximo 16 telefones no total.",
    ),
    (
        "{alias} is chosen. Nothing is saved until you save.",
        "{alias} está escolhido. Nada é salvo até você salvar.",
    ),
    (
        "{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.",
        "{alias} está escolhido como o WhatsApp dele, no lugar de {before}. Nada é salvo até você salvar.",
    ),
    (
        "Choose an option for its WhatsApp. Nothing was saved.",
        "Escolha uma opção para o WhatsApp dele. Nada foi salvo.",
    ),
    (
        "{alias} is not a number of Arauto now. Nothing was saved.",
        "{alias} não é mais um número do Arauto. Nada foi salvo.",
    ),
    (
        "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
        "Digite o outro telefone em que o WhatsApp dele funciona, ou escolha outra opção. Nada foi salvo.",
    ),
    (
        "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
        "O outro telefone do WhatsApp dele não é um telefone: de 8 a 15 dígitos, começando com + e o código do país. Nada foi salvo.",
    ),
    (
        "Line {n} of Other telephones is not a telephone. Nothing was saved.",
        "A linha {n} de Outros telefones não é um telefone. Nada foi salvo.",
    ),
    (
        "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
        "A conversa dele consigo mesmo e uma lista de telefones são duas respostas: limpe a lista ou escolha Estes telefones. Nada foi salvo.",
    ),
    (
        "Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.",
        "Escolha primeiro o WhatsApp de {hermes}: uma conversa tem duas pontas. Nada foi salvo.",
    ),
    (
        "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
        "O WhatsApp dele não pode também dar ordens a ele: essa conversa é o modo self-chat. Nada foi salvo.",
    ),
    (
        "At most 16 telephones can give it orders; these are {n}. Nothing was saved.",
        "No máximo 16 telefones podem dar ordens a ele; estes são {n}. Nada foi salvo.",
    ),
    (
        "1 typed telephone needs a look. Nothing was saved.",
        "1 telefone digitado precisa de uma conferida. Nada foi salvo.",
    ),
    (
        "{n} typed telephones need a look. Nothing was saved.",
        "{n} telefones digitados precisam de uma conferida. Nada foi salvo.",
    ),
    ("Line {n}", "Linha {n}"),
    (
        "Not a telephone: 8 to 15 digits, with + and the country code first.",
        "Não é um telefone: de 8 a 15 dígitos, começando com + e o código do país.",
    ),
    ("Not a number of Arauto.", "Não é um número do Arauto."),
    (
        "The same telephone as line {n}: it counts once.",
        "O mesmo telefone da linha {n}: conta uma vez só.",
    ),
    (
        "The same telephone as {alias}, chosen above: it counts once.",
        "O mesmo telefone de {alias}, escolhido acima: conta uma vez só.",
    ),
    (
        "This is its WhatsApp: it cannot also give it orders.",
        "Este é o WhatsApp dele: não pode também dar ordens a ele.",
    ),
    (
        "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
        "Digitado sem +, então os primeiros dígitos foram lidos como código do país. Se esse não é o país dele, digite primeiro + e o código do país. Salvar de novo mantém como foi lido.",
    ),
    (
        "{n} of 16 telephones, with the numbers chosen above.",
        "{n} de 16 telefones, com os números escolhidos acima.",
    ),
    (
        "These chats stop being protected",
        "Estas conversas deixam de ser protegidas",
    ),
    (
        "Between {bridge}, its WhatsApp until now, and:",
        "Entre {bridge}, o WhatsApp dele até agora, e:",
    ),
    ("itself (self-chat mode)", "ele mesmo (modo self-chat)"),
    (
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
        "O Arauto voltará a deixar todo Hermes escrever nestas conversas e se passar ali por quem dá ordens a {hermes}.",
    ),
    ("Remove the protection", "Remover a proteção"),
    ("Save without protection", "Salvar sem proteção"),
    ("What {alias} is for it", "O que {alias} é para ele"),
    (
        "A telephone that gives it orders",
        "Um telefone que dá ordens a ele",
    ),
    (
        "Its WhatsApp, where it takes orders",
        "O WhatsApp dele, onde ele recebe ordens",
    ),
    (
        "takes orders on {alias} now",
        "recebe ordens em {alias} agora",
    ),
    (
        "{alias} gives it orders now",
        "{alias} dá ordens a ele agora",
    ),
    (
        "Nothing is saved here. The window of the Hermes opens with {alias} chosen.",
        "Nada é salvo aqui. A janela do Hermes abre com {alias} escolhido.",
    ),
    (
        "Choose a Hermes and what {alias} is for it.",
        "Escolha um Hermes e o que {alias} é para ele.",
    ),
    (
        "{alias} is the WhatsApp of {hermes}: it cannot also give it orders.",
        "{alias} é o WhatsApp de {hermes}: não pode também dar ordens a ele.",
    ),
    ("Actions: {name}", "Ações: {name}"),
    (
        "Issues the first invite for {name}.",
        "Emite o primeiro convite para {name}.",
    ),
    (
        "Goes back to the connection of {name} that is in progress.",
        "Volta à conexão de {name} que está em andamento.",
    ),
    (
        "Issues an invite. When it is used, {name} moves to the new device.",
        "Emite um convite. Quando ele for usado, {name} passa para o novo dispositivo.",
    ),
    (
        "Which chat or telephones may give it orders on WhatsApp.",
        "Qual conversa ou quais telefones podem dar ordens a ele no WhatsApp.",
    ),
    (
        "Closes {name} in the hub. Asks first.",
        "Encerra {name} no hub. Pergunta antes.",
    ),
    (
        "Opens the window to pair {alias} with a QR code or a pairing code.",
        "Abre a janela para parear {alias} com um QR code ou um código de pareamento.",
    ),
    (
        "Goes back to the pairing of {alias} that is running.",
        "Volta ao pareamento de {alias} que está em andamento.",
    ),
    (
        "Shows the pairing of {alias} that another panel session started.",
        "Mostra o pareamento de {alias} que outra sessão do painel iniciou.",
    ),
    (
        "Stops {alias} for every Hermes. Asks for the alias.",
        "Para {alias} para todos os Hermes. Pede o apelido.",
    ),
    (
        "Starts the failed session of {alias} again.",
        "Inicia de novo a sessão de {alias} que falhou.",
    ),
    (
        "Another phone paired as {alias}: keep it or undo.",
        "Outro telefone pareou como {alias}: manter ou desfazer.",
    ),
    (
        "Which Hermes takes orders in the chats of this number.",
        "Qual Hermes recebe ordens nas conversas deste número.",
    ),
    (
        "Takes {alias} out of the hub. Asks for the alias.",
        "Tira {alias} do hub. Pede o apelido.",
    ),
    ("Dismiss", "Dispensar"),
    ("Dismiss: {title}", "Dispensar: {title}"),
    (
        "The message of this invite cannot be shown again. If it did not reach the Hermes, issue another invite: this one then stops working.",
        "A mensagem deste convite não pode ser mostrada de novo. Se ela não chegou ao Hermes, emita outro convite: este deixa de funcionar.",
    ),
    ("Nothing has arrived yet", "Ainda não chegou nada"),
    (
        "The Hermes may be waiting for your approval to run the command, in the command channel.",
        "O Hermes pode estar esperando a sua aprovação para executar o comando, no canal de comando.",
    ),
    (
        "The Hermes may not reach this hub: check that its machine reaches the address of the hub.",
        "O Hermes pode não alcançar este hub: confira se a máquina dele alcança o endereço do hub.",
    ),
    (
        "The message may have gone to another chat: it only works in the command channel of this Hermes.",
        "A mensagem pode ter ido para outra conversa: ela só funciona no canal de comando deste Hermes.",
    ),
    (
        "Type the name {name} to confirm.",
        "Digite o nome {name} para confirmar.",
    ),
    (
        "The name typed does not match. Nothing was done.",
        "O nome digitado não confere. Nada foi feito.",
    ),
    ("Hermes {name}: {state}.", "Hermes {name}: {state}."),
    (
        "Saved. Who gives {hermes} orders on WhatsApp: {orders}.",
        "Salvo. Quem dá ordens ao {hermes} no WhatsApp: {orders}.",
    ),
    (
        "Outside WhatsApp: another conversation platform, like Telegram",
        "Fora do WhatsApp: outra plataforma de conversa, como o Telegram",
    ),
    (
        "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead.",
        "O Arauto então não recusa nada para {hermes}. O hub não consegue conferir isso: se {hermes} também recebe ordens em um WhatsApp, registre esse WhatsApp no lugar.",
    ),
    (
        "Orders from outside WhatsApp take no telephone: choose It has no WhatsApp, and leave These telephones and Other telephones empty. Nothing was saved.",
        "Ordens de fora do WhatsApp não levam telefone: escolha Ele não tem WhatsApp e deixe Estes telefones e Outros telefones vazios. Nada foi salvo.",
    ),
    ("outside WhatsApp", "fora do WhatsApp"),
    (
        "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it.",
        "Salvo. {hermes} recebe suas ordens fora do WhatsApp: o Arauto não recusa nada para ele.",
    ),
    // decision 0053: the reason of an invite, the page of the actions, the standing
    (
        "I am asking you myself: reconnect this Hermes to our Arauto hub as \"{name}\". It has a connection to this hub; this replaces it.",
        "Eu mesmo estou pedindo: reconecte este Hermes ao nosso hub Arauto como \"{name}\". Ele tem uma conexão com este hub; isto a substitui.",
    ),
    (
        "I am asking you myself: reconnect this Hermes to our Arauto hub as \"{name}\". It was connected before; this brings it back.",
        "Eu mesmo estou pedindo: reconecte este Hermes ao nosso hub Arauto como \"{name}\". Ele já esteve conectado; isto o traz de volta.",
    ),
    ("Why:", "Por quê:"),
    (
        "the hub stopped trusting the credential of this Hermes (AR-501). Only a new connection brings it back.",
        "o hub deixou de confiar na credencial deste Hermes (AR-501). Só uma nova conexão o traz de volta.",
    ),
    (
        "this hub came back from a backup, and every Hermes comes back with a new connection.",
        "este hub voltou de uma cópia de segurança, e cada Hermes volta com uma nova conexão.",
    ),
    (
        "its previous request was approved, but its connector has not confirmed it yet; this new request replaces it.",
        "o pedido anterior dele foi aprovado, mas o conector ainda não o confirmou; este pedido novo o substitui.",
    ),
    (
        "its previous request was still waiting for the verification code in the panel; this new request replaces it.",
        "o pedido anterior dele ainda esperava o código de verificação no painel; este pedido novo o substitui.",
    ),
    (
        "its previous request was approved, but its connector did not confirm it within {hours} hours (AR-303). One cause is a gateway that was not restarted after the approval.",
        "o pedido anterior dele foi aprovado, mas o conector não o confirmou em {hours} horas (AR-303). Uma das causas é um gateway que não foi reiniciado depois da aprovação.",
    ),
    (
        "the verification code of its previous request was not typed within {minutes} minutes, and the request expired (AR-302).",
        "o código de verificação do pedido anterior dele não foi digitado em {minutes} minutos, e o pedido expirou (AR-302).",
    ),
    (
        "its previous request was closed before it connected.",
        "o pedido anterior dele foi encerrado antes de se conectar.",
    ),
    (
        "this Hermes did not reach the hub for {days} days, and its connection expired (AR-507).",
        "este Hermes ficou {days} dias sem falar com o hub, e a conexão dele expirou (AR-507).",
    ),
    (
        "the admin closed its previous connection in the hub (AR-502).",
        "o admin encerrou a conexão anterior dele no hub (AR-502).",
    ),
    (
        "its previous connection was closed at its own request.",
        "a conexão anterior dele foi encerrada a pedido dele mesmo.",
    ),
    (
        "the previous invite passed its {invite_minutes} minutes without being used.",
        "o convite anterior passou do prazo de {invite_minutes} minutos sem ser usado.",
    ),
    (
        "the admin asked for a new connection.",
        "o admin pediu uma nova conexão.",
    ),
    (
        "Its connector changes from version {old} to version {new}.",
        "O conector dele muda da versão {old} para a versão {new}.",
    ),
    (
        "Its connector is older than the one this hub serves; this replaces it.",
        "O conector dele é mais antigo que o que este hub serve; este pedido o substitui.",
    ),
    (
        "Its connector is not the one this hub serves; this replaces it.",
        "O conector dele não é o que este hub serve; este pedido o substitui.",
    ),
    (
        "Read {page} and follow it; its section \"Reconnect\" applies to you. Ask me if anything is unclear.",
        "Leia {page} e siga; a seção \"Reconnect\" dela vale para você. Pergunte se algo não estiver claro.",
    ),
    (
        "Invite (one use, valid for {minutes} minutes from when it was issued, until {deadline} UTC; the hub checks the time, not you):",
        "Convite (um uso, vale {minutes} minutos a partir da emissão, até {deadline} UTC; quem confere o horário é o hub, não você):",
    ),
    (
        "{name} runs an older connector{old}; this hub serves {new}. Reconnect it to update. Its Arauto tools keep working until its gateway restarts, and the new connector runs from that restart.",
        "{name} roda um conector mais antigo{old}; este hub serve {new}. Reconecte para atualizar. As ferramentas do Arauto dele continuam funcionando até o gateway reiniciar, e o conector novo roda a partir desse reinício.",
    ),
    (
        "{name} has the new connector installed, but its gateway has not restarted since: it still runs the older one. Ask its operator to restart the gateway once. A reconnect is not needed.",
        "{name} já tem o conector novo instalado, mas o gateway dele não reiniciou desde então: ele ainda roda o antigo. Peça ao operador dele para reiniciar o gateway uma vez. Não é preciso reconectar.",
    ),
    (
        "{name} declares a connector that is not one this hub serves.",
        "{name} declara um conector que não é um dos que este hub serve.",
    ),
    (
        "The connector confirms this approval when it runs. If the Hermes said its program was replaced or a key was made, its gateway must restart once. Issuing another invite cancels this approval.",
        "O conector confirma esta aprovação quando roda. Se o Hermes disse que o programa foi substituído ou que uma chave foi criada, o gateway dele precisa reiniciar uma vez. Emitir outro convite cancela esta aprovação.",
    ),
    (
        "{name} is connected to this hub, but its connector has not reached it since {ago}. Try a restart of its gateway first. Reconnect only when it moved to another machine or lost its .arauto folder.",
        "{name} está conectado a este hub, mas o conector dele não fala com o hub desde {ago}. Tente primeiro reiniciar o gateway dele. Reconecte só se ele mudou de máquina ou perdeu a pasta .arauto.",
    ),
    (
        "If you issue it now, the message tells the Hermes: Why: {reason}",
        "Se você emitir agora, a mensagem diz ao Hermes: Por quê: {reason}",
    ),
    (
        "The state changed after you opened this window. The message gives the reason as it is now: {reason}",
        "O estado mudou depois que você abriu esta janela. A mensagem dá o motivo como está agora: {reason}",
    ),
    ("Outdated: {n}", "Desatualizados: {n}"),
    ("Revoked", "Revogado"),
    ("Expired", "Expirado"),
    ("Left", "Saiu"),
    ("Restart pending", "Reinício pendente"),
];
