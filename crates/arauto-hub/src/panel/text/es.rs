//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "Panel"),
    ("Language", "Idioma"),
    ("Change", "Cambiar"),
    ("R", "L"),
    ("S", "E"),
    ("M", "M"),
    ("W", "D"),
    ("Sign in to the panel", "Entrar al panel"),
    ("Entry code", "Código de entrada"),
    (
        "Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in.",
        "Emite el código en el servidor del hub, con {command}. Es válido durante 10 minutos y para una sola entrada.",
    ),
    ("Sign in", "Entrar"),
    ("Sign out", "Salir"),
    ("Alerts", "Alertas"),
    ("Unknown devices", "Dispositivos desconocidos"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "Dispositivos que presentaron una credencial válida que la base de datos no explica: {n}. El camino de vuelta para cada uno es una invitación nueva. Míralos con {command}.",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "Más alertas: {n}. Míralas todas con {command}.",
    ),
    ("Accesses", "Accesos"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "En esta versión el panel conecta y revoca Hermes y cambia los accesos de cada número. Vincular números y elegir chats todavía se hace con el comando {command}, en el servidor del hub.",
    ),
    ("none", "ninguno"),
    (
        "Your Hermes and their [numbers]",
        "Tus Hermes y sus [números]",
    ),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "Conectados: {connected} · En espera: {waiting} · En alerta: {stopped}",
    ),
    ("Connect Hermes", "Conectar Hermes"),
    ("State", "Estado"),
    ("Numbers", "Números"),
    ("Can be woken", "Puede ser despertado"),
    ("Connector", "Conector"),
    ("Seen", "Visto"),
    ("Actions", "Acciones"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Todavía no hay ningún Hermes. Conecta el primero con el botón \"Conectar Hermes\".",
    ),
    ("yes", "sí"),
    ("no", "no"),
    ("Continue", "Continuar"),
    ("Connect", "Conectar"),
    ("Reconnect", "Reconectar"),
    ("Revoke", "Revocar"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "\"Números\" son los que el Hermes alcanza. El conector es lo que el dispositivo declaró: no está verificado.",
    ),
    ("Engine", "Motor"),
    (
        "There is no engine yet. Set it with {command}.",
        "Todavía no hay motor. Configúralo con {command}.",
    ),
    ("Alias", "Alias"),
    ("Number", "Número"),
    ("Hermes with access", "Hermes con acceso"),
    ("Last message", "Último mensaje"),
    (
        "No number yet. Add one with {command}.",
        "Todavía no hay ningún número. Añade uno con {command}.",
    ),
    ("command", "órdenes"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "Del último mensaje solo se muestra la hora, nunca el contenido. El panel lista lo que pasa por el hub: los demás dispositivos vinculados al mismo número solo aparecen en el teléfono.",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "Hermes en las filas, números en las columnas. Haz clic en una celda para cambiarla.",
    ),
    ("Connect a Hermes first.", "Primero conecta un Hermes."),
    ("Add a WhatsApp first.", "Primero añade un WhatsApp."),
    ("(accesses kept)", "(accesos conservados)"),
    ("no access", "sin acceso"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} leer · {s} enviar · {m} modificar · {w} ser despertado.",
    ),
    ("Saved: {what}", "Guardado: {what}"),
    ("Warnings that block nothing", "Avisos que no bloquean nada"),
    // guide.rs
    ("Name", "Nombre"),
    ("Block", "Bloque"),
    ("Code", "Código"),
    ("Done", "Listo"),
    ("Invite issued", "Invitación emitida"),
    (
        "A device used the invite",
        "Un dispositivo usó la invitación",
    ),
    (
        "Code checked and request approved",
        "Código comprobado y solicitud aprobada",
    ),
    (
        "The connector confirmed its first credential",
        "El conector confirmó su primera credencial",
    ),
    (
        "A device used the invite, from {address}",
        "Un dispositivo usó la invitación, desde {address}",
    ),
    ("waiting…", "esperando…"),
    (
        "What name will this Hermes have in the panel?",
        "¿Qué nombre tendrá este Hermes en el panel?",
    ),
    ("Name of the Hermes", "Nombre del Hermes"),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "Letras minúsculas, dígitos, guion y guion bajo, hasta 32 caracteres, empezando por una letra o un dígito. El nombre aparece en el bloque y no cambia después.",
    ),
    (
        "Preparing a secure connection",
        "Preparando una conexión segura",
    ),
    (
        "The hub drew the invite of {name}.",
        "El hub generó la invitación de {name}.",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "Invitación para {name} · válida durante {time} más · un solo uso",
    ),
    ("Copied", "Copiado"),
    (
        "Click the block to copy it.",
        "Haz clic en el bloque para copiarlo.",
    ),
    (
        "Select the whole text and copy it.",
        "Selecciona todo el texto y cópialo.",
    ),
    ("It only appears here, once.", "Solo aparece aquí, una vez."),
    (
        "The invite issued before was cancelled.",
        "La invitación emitida antes fue cancelada.",
    ),
    (
        "The request that was in progress was closed.",
        "La solicitud que estaba en curso fue cerrada.",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "El dispositivo en uso sigue funcionando hasta que el nuevo confirme su primera credencial.",
    ),
    (
        "This hub has no connector to serve",
        "Este hub no tiene ningún conector que ofrecer",
    ),
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "El bloque le indica al Hermes que descargue el conector desde este hub, y la carpeta de descargas está vacía. No se emitió nada. Emite la invitación de {name} con el comando {command}, en el servidor del hub.",
    ),
    (
        "Waiting for the Hermes {name}",
        "Esperando al Hermes {name}",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "Cada paso se enciende cuando el hub realmente lo observa. Si el Hermes pide aprobación para ejecutar el comando, quien aprueba eres tú, en el canal de órdenes.",
    ),
    (
        "A device used the invite of {name}",
        "Un dispositivo usó la invitación de {name}",
    ),
    ("unknown", "desconocida"),
    ("Seen by the hub", "Visto por el hub"),
    ("address {address}, {when}", "dirección {address}, {when}"),
    ("Said by the device", "Dicho por el dispositivo"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "máquina \"{host}\" · sistema \"{os}\" · Hermes \"{version}\" (no verificado)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "Último uso del dispositivo actual: {when}, desde {address}. ¿Lo reconoces?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "Último uso del dispositivo actual: {when}. ¿Lo reconoces?",
    ),
    ("I recognize it", "Lo reconozco"),
    ("I do not recognize it", "No lo reconozco"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Si el Hermes estaba apagado y hubo uso, alguien usó una copia.",
    ),
    ("Verification code", "Código de verificación"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Escribe el código que el Hermes respondió en el canal de órdenes. El panel nunca muestra este código.",
    ),
    (
        "The request of {name} was denied",
        "La solicitud de {name} fue denegada",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "El código se escribió mal por quinta vez. Para conectar, emite otra invitación.",
    ),
    (
        "The request of {name} expired",
        "La solicitud de {name} caducó",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "Pasaron los 10 minutos para escribir el código. Para conectar, emite otra invitación.",
    ),
    (
        "The identity of {name} was copied",
        "La identidad de {name} fue copiada",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "Dos dispositivos se presentaron con la misma identidad antes de la aprobación, y la solicitud fue detenida. Para conectar, emite otra invitación.",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} no tiene invitación ni solicitud en espera",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "La invitación caducó, fue cancelada, o la solicitud ya fue resuelta. Si el Hermes no respondió ningún código, no apruebes; si respondió AR-102, otro dispositivo usó la invitación. Para conectar, emite otra invitación.",
    ),
    ("{name} is connected", "{name} está conectado"),
    ("{name} was approved", "{name} fue aprobado"),
    (
        "What it may do on each number is in the Accesses tab.",
        "Lo que puede hacer en cada número está en la pestaña Accesos.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "En cuanto el conector confirme, aparece como conectado. Los accesos que ya tenía vuelven a aplicarse a este dispositivo.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "En cuanto el conector confirme, aparece como conectado. Todavía no tiene acceso a ningún número.",
    ),
    (
        "The hub did not do what was asked",
        "El hub no hizo lo que se pidió",
    ),
    ("Issue invite", "Emitir invitación"),
    ("Cancel", "Cancelar"),
    ("Next step", "Siguiente paso"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "Después de copiarlo, pégalo en el canal de órdenes del Hermes {name}. En ningún otro lugar.",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "La invitación es válida durante {time} más. Esta pantalla se actualiza sola.",
    ),
    ("Approve", "Aprobar"),
    ("Deny", "Denegar"),
    ("Confirm within {time}.", "Confirma en un plazo de {time}."),
    ("Issue another invite", "Emitir otra invitación"),
    ("Close", "Cerrar"),
    ("See accesses", "Ver accesos"),
    ("Finish", "Finalizar"),
    ("Copied to the clipboard.", "Copiado al portapapeles."),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "El texto está seleccionado. Cópialo con Ctrl+C.",
    ),
    ("Reconnect {name}", "Reconectar {name}"),
    ("Connect {name}", "Conectar {name}"),
    ("Steps", "Pasos"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "Ser despertado requiere leer. No se guardó nada.",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "Iniciar chats requiere enviar y el alcance \"todos los chats\". No se guardó nada.",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "La espera antes de despertar va de 0 a 60 segundos. No se guardó nada.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "Algo falló en el hub. Mira el registro del hub y comprueba el acceso en la matriz.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "La base de datos volvió de una copia de seguridad más antigua y el hub todavía no lo ha tratado. No se escribió nada. Reinicia el hub.",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "El Hermes {hermes} no tiene acceso al número {number}.",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "El Hermes {hermes} solo alcanza los chats elegidos del número {number}. Chats con línea: {lines}.",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "El Hermes {hermes} en el número {number}, en todos los chats:",
    ),
    ("reads", "lee"),
    ("does not read", "no lee"),
    ("sends", "envía"),
    ("does not send", "no envía"),
    ("modifies", "modifica"),
    ("does not modify", "no modifica"),
    (
        "is woken after {seconds} s of silence",
        "es despertado tras {seconds} s de silencio",
    ),
    ("is not woken", "no es despertado"),
    ("may start chats", "puede iniciar chats"),
    (
        "chats with their own rule: {lines}",
        "chats con regla propia: {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} en el número {number}",
    ),
    ("Where it applies", "Dónde se aplica"),
    (
        "Every chat, including the ones that come later",
        "Todos los chats, incluidos los que lleguen después",
    ),
    ("Only chosen chats", "Solo chats elegidos"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "Con \"solo chats elegidos\", para el Hermes solo existen los chats que tienen una línea propia, y lo que puede hacer en cada uno viene de la línea. Las líneas todavía se crean con el comando {command}. Hoy: {lines}.",
    ),
    ("What it may do", "Qué puede hacer"),
    ("Read", "Leer"),
    (
        "see chats and messages, search, download files",
        "ver chats y mensajes, buscar, descargar archivos",
    ),
    ("Send", "Enviar"),
    (
        "write, react, mark as read",
        "escribir, reaccionar, marcar como leído",
    ),
    ("Modify", "Modificar"),
    (
        "edit and delete what already exists",
        "editar y eliminar lo que ya existe",
    ),
    ("Be woken", "Ser despertado"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "el hub avisa cuando llega un mensaje (requiere leer)",
    ),
    ("Wait", "Espera"),
    (
        "seconds of silence before waking",
        "segundos de silencio antes de despertar",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "Puede iniciar un chat con alguien que nunca habló con el número (requiere enviar y todos los chats)",
    ),
    ("Today:", "Hoy:"),
    ("Save", "Guardar"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "Se aplica desde la siguiente llamada, sin reiniciar nada. Sin ningún verbo marcado, el acceso se elimina.",
    ),
    ("Revoke {name}", "Revocar {name}"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "Revocar cierra el Hermes en el hub: el dispositivo en uso, la solicitud en curso y la invitación emitida. El canal se corta en segundos.",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "Los accesos se conservan y vuelven a aplicarse cuando se conecta un dispositivo nuevo. El único camino de vuelta es una invitación nueva.",
    ),
    // data.rs
    ("never", "nunca"),
    ("now", "ahora"),
    ("{n} min ago", "hace {n} min"),
    ("{n} h ago", "hace {n} h"),
    ("{n} d ago", "hace {n} d"),
    (
        "a credential that had already been swapped was presented",
        "se presentó una credencial que ya había sido cambiada",
    ),
    (
        "two connections at the same time",
        "dos conexiones al mismo tiempo",
    ),
    (
        "the credential could not be read",
        "no se pudo leer la credencial",
    ),
    (
        "the last use was not recognized on the reconnect",
        "el último uso no fue reconocido al reconectar",
    ),
    (
        "the identity was copied before the approval",
        "la identidad fue copiada antes de la aprobación",
    ),
    (
        "the hub no longer trusts the credential",
        "el hub ya no confía en la credencial",
    ),
    ("Waiting for your code", "Esperando tu código"),
    (
        "Approved, waiting for the connector",
        "Aprobado, esperando al conector",
    ),
    ("Waiting for the Hermes", "Esperando al Hermes"),
    ("Connected", "Conectado"),
    ("Disconnected", "Desconectado"),
    ("In quarantine", "En cuarentena"),
    ("Suspended", "Suspendido"),
    ("No device", "Sin dispositivo"),
    ("a Hermes", "un Hermes"),
    ("Invite", "Invitación"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "otro dispositivo intentó usar la invitación de {hermes}. El bloque fue a parar a otro lugar. La primera solicitud sigue en pie.",
    ),
    ("Identity copied", "Identidad copiada"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "la identidad de {hermes} fue copiada antes de la aprobación. La solicitud fue denegada.",
    ),
    ("Hermes in quarantine", "Hermes en cuarentena"),
    ("{hermes}. Reason: {reason}.", "{hermes}. Motivo: {reason}."),
    ("Suspected copy", "Sospecha de copia"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "el último uso de {hermes} no fue reconocido al reconectar.",
    ),
    ("Connection taken", "Conexión tomada"),
    (
        "the connection of {hermes} was taken by another one.",
        "la conexión de {hermes} fue tomada por otra.",
    ),
    ("Closing", "Cierre"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} pidió su propio cierre.",
    ),
    ("Reuse", "Reutilización"),
    (
        "a copy of {hermes} was used before the closing.",
        "una copia de {hermes} fue usada antes del cierre.",
    ),
    ("Unconfirmed swaps", "Cambios sin confirmar"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} recibió credenciales y no las confirmó, tres veces en 24 horas. Puede ser una mala red, o alguien probando una copia.",
    ),
    ("Audit mark", "Marca de auditoría"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "se escribió un evento de seguridad y la marca fuera de la base de datos no lo acompañó.",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "la versión del motor no es una que el hub haya comprobado.",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "una sesión del motor tenía otra configuración, y el hub aplicó la suya.",
    ),
    ("Number swapped", "Número cambiado"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "el teléfono que se vinculó pertenece a un número distinto del indicado. Los accesos quedan detenidos hasta que decidas.",
    ),
    ("Address: {address}.", "Dirección: {address}."),
    ("an unknown date", "una fecha desconocida"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "El hub volvió de una copia de seguridad de {date}. Lo que se aprobó, revocó, reconectó o cambió en los accesos después de eso se perdió. Reconecta cada Hermes y comprueba los accesos.",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "El hub volvió de una copia de seguridad. {name} está suspendido y solo vuelve con una invitación nueva: reconéctalo o revócalo. No existe \"reactivar\".",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "El hub ya no confía en la credencial de {name}. Motivo: {reason}. Su acceso está detenido. Salidas: reconectar o revocar. No existe \"liberar\".",
    ),
    ("Not paired", "Sin vincular"),
    ("Pairing", "Vinculando"),
    ("Reconnecting", "Reconectando"),
    ("Failed", "Con fallo"),
    ("Removed", "Eliminado"),
    (
        "version {version}, checked",
        "versión {version}, comprobada",
    ),
    (
        "version {version}, not checked by the hub",
        "versión {version}, no comprobada por el hub",
    ),
    ("version not read yet", "versión todavía no leída"),
    ("last answer {when}", "última respuesta {when}"),
    ("has not answered yet", "todavía no ha respondido"),
    ("all", "todos"),
    ("all · own rules: {n}", "todos · reglas propias: {n}"),
    ("chosen chats: {n}", "chats elegidos: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: Hermes despertados en los mismos chats: {n}.",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: Hermes que envían en los mismos chats: {n}.",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} está marcado como número de órdenes y hay Hermes autorizados a enviar en él.",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "Ya existe un Hermes con ese nombre. Para asociarle otro dispositivo, usa el botón de su fila.",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "Ese nombre perteneció a un Hermes que fue eliminado, y un nombre nunca se reutiliza. Elige otro.",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "Antes del código, indica si reconoces el último uso del dispositivo actual. No se contó nada.",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "El dispositivo actual fue usado mientras respondías. Mira ahora su último uso y responde de nuevo. No se aprobó nada.",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "El código tiene 6 dígitos, como 123-456. No se contó nada.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "La base de datos volvió de una copia de seguridad más antigua y el hub todavía no lo ha tratado. No se escribió nada. Reinicia el hub: activa el modo restaurado, y después cada Hermes se reconecta con una invitación nueva.",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "Algo falló en el hub y no se hizo nada. Mira el registro del hub.",
    ),
    (
        "The name has from 1 to 32 characters.",
        "El nombre tiene de 1 a 32 caracteres.",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "El nombre empieza por una letra minúscula o un dígito.",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "Usa solo letras minúsculas, dígitos, guion y guion bajo.",
    ),
    (
        "That name is reserved. Choose another.",
        "Ese nombre está reservado. Elige otro.",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "El código no coincide. Intento {attempts} de {max}. Escribe el último código que el Hermes respondió.",
    ),
    // mod.rs
    (
        "Invalid or expired code. Issue another one on the hub's server.",
        "Código no válido o caducado. Emite otro en el servidor del hub.",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Demasiados intentos en este momento. Espera un minuto e inténtalo de nuevo.",
    ),
];
