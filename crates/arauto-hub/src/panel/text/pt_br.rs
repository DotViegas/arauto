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
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "Nesta versão o painel conecta e revoga Hermes e altera os acessos de cada número. Parear números e escolher conversas ainda se faz com o comando {command}, no servidor do hub.",
    ),
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
    ("Can be woken", "Pode ser acordado"),
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
        "No number yet. Add one with {command}.",
        "Nenhum número ainda. Adicione um com {command}.",
    ),
    ("command", "comando"),
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
    ("Block", "Bloco"),
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
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "Letras minúsculas, dígitos, hífen e sublinhado, até 32 caracteres, começando por letra ou dígito. O nome aparece no bloco e não muda depois.",
    ),
    (
        "Preparing a secure connection",
        "Preparando uma conexão segura",
    ),
    (
        "The hub drew the invite of {name}.",
        "O hub sorteou o convite de {name}.",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "Convite para {name} · vale por mais {time} · uso único",
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
        "Ele só aparece aqui, uma vez.",
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
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "O bloco manda o Hermes baixar o conector deste hub, e a pasta de downloads está vazia. Nada foi emitido. Emita o convite de {name} com o comando {command}, no servidor do hub.",
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
        "the hub gives notice when a message arrives (requires reading)",
        "o hub avisa quando chega uma mensagem (exige ler)",
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
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} está marcado como número de comando e há Hermes com permissão para enviar nele.",
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
        "The hub draws an invite: a block of text you paste in the command channel of that Hermes, and nowhere else.",
        "O hub gera um convite: um bloco de texto que você cola no canal de comando desse Hermes, e em nenhum outro lugar.",
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
];
