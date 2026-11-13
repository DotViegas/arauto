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
    ("Sign in to the panel", "Entrar no painel"),
    ("Entry code", "Código de entrada"),
    (
        "Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in.",
        "Emita o código no servidor do hub, com {command}. Ele vale por 10 minutos e para uma entrada.",
    ),
    ("Sign in", "Entrar"),
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
    ("Continue", "Continuar"),
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
        "Invalid or expired code. Issue another one on the hub's server.",
        "Código inválido ou expirado. Emita outro no servidor do hub.",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Tentativas demais agora. Espere um minuto e tente de novo.",
    ),
];
