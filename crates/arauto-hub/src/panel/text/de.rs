//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "Panel"),
    ("Language", "Sprache"),
    ("Change", "Übernehmen"),
    ("R", "L"),
    ("S", "S"),
    ("M", "Ä"),
    ("W", "G"),
    ("Waiting for the device…", "Warten auf das Gerät…"),
    (
        "The passkey was not accepted. Try again.",
        "Der Passkey wurde nicht akzeptiert. Versuchen Sie es erneut.",
    ),
    (
        "This browser cannot use a passkey here. Use a current browser, at the address of the panel.",
        "Dieser Browser kann hier keinen Passkey verwenden. Verwenden Sie einen aktuellen Browser unter der Adresse des Panels.",
    ),
    ("Sign in to the panel", "Am Panel anmelden"),
    (
        "Use the passkey you registered: on this device, or in your password manager.",
        "Verwenden Sie den Passkey, den Sie registriert haben: auf diesem Gerät oder in Ihrem Passwortmanager.",
    ),
    ("Sign in with a passkey", "Mit Passkey anmelden"),
    ("Installation code", "Installationscode"),
    (
        "First access, or a lost passkey: type the code the hub wrote in its log when it started, or issue one on the hub's server with {command}. The code only opens the registration of a passkey.",
        "Erster Zugang oder verlorener Passkey: Geben Sie den Code ein, den der Hub beim Start in sein Log geschrieben hat, oder stellen Sie auf dem Server des Hubs mit {command} einen aus. Der Code öffnet nur die Registrierung eines Passkeys.",
    ),
    ("Continue", "Fortsetzen"),
    ("Register a passkey", "Passkey registrieren"),
    (
        "A passkey stays on this device or in your password manager, and opens with the fingerprint, the face or the PIN of the device. The hub keeps only what checks it: there is no password to leak.",
        "Ein Passkey bleibt auf diesem Gerät oder in Ihrem Passwortmanager und wird mit dem Fingerabdruck, dem Gesicht oder der PIN des Geräts geöffnet. Der Hub bewahrt nur auf, was ihn prüft: Es gibt kein Passwort, das durchsickern könnte.",
    ),
    (
        "This session only reaches the registration of a passkey. The rest of the panel opens after it.",
        "Diese Sitzung reicht nur bis zur Registrierung eines Passkeys. Der Rest des Panels öffnet sich danach.",
    ),
    ("Sign out", "Abmelden"),
    ("Alerts", "Warnmeldungen"),
    ("Unknown devices", "Unbekannte Geräte"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "Anzahl: {n}. Sie haben einen gültigen Berechtigungsnachweis vorgelegt, den die Datenbank nicht erklärt. Der Weg zurück führt für jedes über eine neue Einladung. Anzeigen mit {command}.",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "Weitere Warnmeldungen: {n}. Alle anzeigen mit {command}.",
    ),
    ("Accesses", "Zugriffe"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "In dieser Version verbindet und widerruft das Panel Hermes und ändert die Zugriffe jeder Nummer. Nummern koppeln und Chats auswählen erfolgt weiterhin mit dem Befehl {command} auf dem Server des Hubs.",
    ),
    ("none", "keine"),
    (
        "Your Hermes and their [numbers]",
        "Ihre Hermes und deren [Nummern]",
    ),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "Verbunden: {connected} · Wartend: {waiting} · Mit Warnmeldung: {stopped}",
    ),
    ("Connect Hermes", "Hermes verbinden"),
    ("State", "Status"),
    ("Numbers", "Nummern"),
    ("Can be woken", "Kann geweckt werden"),
    ("Connector", "Connector"),
    ("Seen", "Gesehen"),
    ("Actions", "Aktionen"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Noch kein Hermes. Verbinden Sie den ersten mit der Schaltfläche „Hermes verbinden“.",
    ),
    ("yes", "ja"),
    ("no", "nein"),
    ("Connect", "Verbinden"),
    ("Reconnect", "Neu verbinden"),
    ("Revoke", "Widerrufen"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "„Nummern“ sind die, die der Hermes erreicht. Der Connector ist das, was das Gerät angegeben hat: Es ist nicht geprüft.",
    ),
    ("Engine", "Engine"),
    (
        "There is no engine yet. Set it with {command}.",
        "Es gibt noch keine Engine. Legen Sie sie mit {command} fest.",
    ),
    ("Alias", "Alias"),
    ("Number", "Nummer"),
    ("Hermes with access", "Hermes mit Zugriff"),
    ("Last message", "Letzte Nachricht"),
    (
        "No number yet. Add one with {command}.",
        "Noch keine Nummer. Fügen Sie eine mit {command} hinzu.",
    ),
    ("command", "Befehl"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "Von der letzten Nachricht wird nur die Zeit gezeigt, nie der Inhalt. Das Panel listet auf, was über den Hub läuft: Andere verknüpfte Geräte derselben Nummer erscheinen nur auf dem Telefon.",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "Hermes in den Zeilen, Nummern in den Spalten. Klicken Sie auf eine Zelle, um sie zu ändern.",
    ),
    (
        "Connect a Hermes first.",
        "Verbinden Sie zuerst einen Hermes.",
    ),
    (
        "Add a WhatsApp first.",
        "Fügen Sie zuerst ein WhatsApp hinzu.",
    ),
    ("(accesses kept)", "(Zugriffe beibehalten)"),
    ("no access", "kein Zugriff"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} lesen · {s} senden · {m} ändern · {w} geweckt werden.",
    ),
    ("Saved: {what}", "Gespeichert: {what}"),
    (
        "Warnings that block nothing",
        "Hinweise, die nichts blockieren",
    ),
    // guide.rs
    ("Name", "Name"),
    ("Block", "Block"),
    ("Code", "Code"),
    ("Done", "Fertig"),
    ("Invite issued", "Einladung ausgestellt"),
    (
        "A device used the invite",
        "Ein Gerät hat die Einladung verwendet",
    ),
    (
        "Code checked and request approved",
        "Code geprüft und Anfrage genehmigt",
    ),
    (
        "The connector confirmed its first credential",
        "Der Connector hat seinen ersten Berechtigungsnachweis bestätigt",
    ),
    (
        "A device used the invite, from {address}",
        "Ein Gerät hat die Einladung verwendet, von {address}",
    ),
    ("waiting…", "wartet…"),
    (
        "What name will this Hermes have in the panel?",
        "Welchen Namen soll dieser Hermes im Panel haben?",
    ),
    ("Name of the Hermes", "Name des Hermes"),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "Kleinbuchstaben, Ziffern, Bindestrich und Unterstrich, bis zu 32 Zeichen, beginnend mit einem Buchstaben oder einer Ziffer. Der Name erscheint im Block und ändert sich danach nicht mehr.",
    ),
    (
        "Preparing a secure connection",
        "Sichere Verbindung wird vorbereitet",
    ),
    (
        "The hub drew the invite of {name}.",
        "Der Hub hat die Einladung für {name} erzeugt.",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "Einladung für {name} · noch {time} gültig · einmalig verwendbar",
    ),
    ("Copied", "Kopiert"),
    (
        "Click the block to copy it.",
        "Klicken Sie auf den Block, um ihn zu kopieren.",
    ),
    (
        "Select the whole text and copy it.",
        "Markieren Sie den gesamten Text und kopieren Sie ihn.",
    ),
    (
        "It only appears here, once.",
        "Er erscheint nur hier, ein einziges Mal.",
    ),
    (
        "The invite issued before was cancelled.",
        "Die zuvor ausgestellte Einladung wurde storniert.",
    ),
    (
        "The request that was in progress was closed.",
        "Die laufende Anfrage wurde geschlossen.",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "Das verwendete Gerät arbeitet weiter, bis das neue seinen ersten Berechtigungsnachweis bestätigt.",
    ),
    (
        "This hub has no connector to serve",
        "Dieser Hub hat keinen Connector zum Bereitstellen",
    ),
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Der Block weist den Hermes an, den Connector von diesem Hub herunterzuladen, und der Download-Ordner ist leer. Es wurde nichts ausgestellt. Stellen Sie die Einladung für {name} mit dem Befehl {command} auf dem Server des Hubs aus.",
    ),
    (
        "Waiting for the Hermes {name}",
        "Warten auf den Hermes {name}",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "Jeder Schritt leuchtet auf, sobald der Hub ihn tatsächlich beobachtet. Wenn der Hermes um Genehmigung bittet, den Befehl auszuführen, genehmigen Sie selbst, im Befehlskanal.",
    ),
    (
        "A device used the invite of {name}",
        "Ein Gerät hat die Einladung für {name} verwendet",
    ),
    ("unknown", "unbekannt"),
    ("Seen by the hub", "Vom Hub gesehen"),
    ("address {address}, {when}", "Adresse {address}, {when}"),
    ("Said by the device", "Vom Gerät angegeben"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "Rechner „{host}“ · System „{os}“ · Hermes „{version}“ (nicht geprüft)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "Letzte Verwendung des aktuellen Geräts: {when}, von {address}. Erkennen Sie sie?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "Letzte Verwendung des aktuellen Geräts: {when}. Erkennen Sie sie?",
    ),
    ("I recognize it", "Ich erkenne sie"),
    ("I do not recognize it", "Ich erkenne sie nicht"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Wenn der Hermes ausgeschaltet war und es eine Verwendung gab, hat jemand eine Kopie verwendet.",
    ),
    ("Verification code", "Prüfcode"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Geben Sie den Code ein, den der Hermes im Befehlskanal geantwortet hat. Das Panel zeigt diesen Code nie an.",
    ),
    (
        "The request of {name} was denied",
        "Die Anfrage von {name} wurde abgelehnt",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "Der Code wurde zum fünften Mal falsch eingegeben. Stellen Sie zum Verbinden eine weitere Einladung aus.",
    ),
    (
        "The request of {name} expired",
        "Die Anfrage von {name} ist abgelaufen",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "Die 10 Minuten zum Eingeben des Codes sind vorbei. Stellen Sie zum Verbinden eine weitere Einladung aus.",
    ),
    (
        "The identity of {name} was copied",
        "Die Identität von {name} wurde kopiert",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "Vor der Genehmigung sind zwei Geräte mit derselben Identität aufgetreten, und die Anfrage wurde gestoppt. Stellen Sie zum Verbinden eine weitere Einladung aus.",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} hat keine Einladung und keine wartende Anfrage",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "Die Einladung ist abgelaufen, wurde storniert, oder die Anfrage wurde bereits erledigt. Wenn der Hermes keinen Code geantwortet hat, genehmigen Sie nicht; wenn er AR-102 geantwortet hat, hat ein anderes Gerät die Einladung verwendet. Stellen Sie zum Verbinden eine weitere Einladung aus.",
    ),
    ("{name} is connected", "{name} ist verbunden"),
    ("{name} was approved", "{name} wurde genehmigt"),
    (
        "What it may do on each number is in the Accesses tab.",
        "Was er auf jeder Nummer darf, steht im Tab „Zugriffe“.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "Sobald der Connector bestätigt, wird er als verbunden angezeigt. Die Zugriffe, die er bereits hatte, gelten wieder für dieses Gerät.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "Sobald der Connector bestätigt, wird er als verbunden angezeigt. Er hat noch keinen Zugriff auf eine Nummer.",
    ),
    (
        "The hub did not do what was asked",
        "Der Hub hat die Anforderung nicht ausgeführt",
    ),
    ("Issue invite", "Einladung ausstellen"),
    ("Cancel", "Abbrechen"),
    ("Next step", "Nächster Schritt"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "Fügen Sie ihn nach dem Kopieren in den Befehlskanal des Hermes {name} ein. Nirgendwo sonst.",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "Die Einladung ist noch {time} gültig. Diese Ansicht aktualisiert sich von selbst.",
    ),
    ("Approve", "Genehmigen"),
    ("Deny", "Ablehnen"),
    (
        "Confirm within {time}.",
        "Bestätigen Sie innerhalb von {time}.",
    ),
    ("Issue another invite", "Weitere Einladung ausstellen"),
    ("Close", "Schließen"),
    ("See accesses", "Zugriffe anzeigen"),
    ("Finish", "Abschließen"),
    ("Copied to the clipboard.", "In die Zwischenablage kopiert."),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "Der Text ist markiert. Kopieren Sie ihn mit Strg+C.",
    ),
    ("Reconnect {name}", "{name} neu verbinden"),
    ("Connect {name}", "{name} verbinden"),
    ("Steps", "Schritte"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "Geweckt werden setzt Lesen voraus. Es wurde nichts gespeichert.",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "Chats beginnen setzt Senden und den Geltungsbereich „jeder Chat“ voraus. Es wurde nichts gespeichert.",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "Die Wartezeit vor dem Wecken reicht von 0 bis 60 Sekunden. Es wurde nichts gespeichert.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "Im Hub ist etwas fehlgeschlagen. Sehen Sie im Protokoll des Hubs nach und prüfen Sie den Zugriff in der Matrix.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "Die Datenbank stammt aus einer älteren Sicherung, und der Hub hat das noch nicht verarbeitet. Es wurde nichts geschrieben. Starten Sie den Hub neu.",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "Der Hermes {hermes} hat keinen Zugriff auf die Nummer {number}.",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Der Hermes {hermes} erreicht nur die ausgewählten Chats der Nummer {number}. Chats mit einer Zeile: {lines}.",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Der Hermes {hermes} auf der Nummer {number}, in jedem Chat:",
    ),
    ("reads", "liest"),
    ("does not read", "liest nicht"),
    ("sends", "sendet"),
    ("does not send", "sendet nicht"),
    ("modifies", "ändert"),
    ("does not modify", "ändert nicht"),
    (
        "is woken after {seconds} s of silence",
        "wird nach {seconds} s Stille geweckt",
    ),
    ("is not woken", "wird nicht geweckt"),
    ("may start chats", "darf Chats beginnen"),
    (
        "chats with their own rule: {lines}",
        "Chats mit eigener Regel: {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} auf der Nummer {number}",
    ),
    ("Where it applies", "Wo es gilt"),
    (
        "Every chat, including the ones that come later",
        "Jeder Chat, auch die später hinzukommenden",
    ),
    ("Only chosen chats", "Nur ausgewählte Chats"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "Bei „nur ausgewählte Chats“ existieren für den Hermes nur die Chats mit einer eigenen Zeile, und was er in jedem darf, ergibt sich aus der Zeile. Die Zeilen werden weiterhin mit dem Befehl {command} angelegt. Heute: {lines}.",
    ),
    ("What it may do", "Was er darf"),
    ("Read", "Lesen"),
    (
        "see chats and messages, search, download files",
        "Chats und Nachrichten sehen, suchen, Dateien herunterladen",
    ),
    ("Send", "Senden"),
    (
        "write, react, mark as read",
        "schreiben, reagieren, als gelesen markieren",
    ),
    ("Modify", "Ändern"),
    (
        "edit and delete what already exists",
        "Vorhandenes bearbeiten und löschen",
    ),
    ("Be woken", "Geweckt werden"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "der Hub benachrichtigt, wenn eine Nachricht eintrifft (setzt Lesen voraus)",
    ),
    ("Wait", "Wartezeit"),
    (
        "seconds of silence before waking",
        "Sekunden Stille vor dem Wecken",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "Darf einen Chat mit jemandem beginnen, der nie mit der Nummer gesprochen hat (setzt Senden und jeden Chat voraus)",
    ),
    ("Today:", "Heute:"),
    ("Save", "Speichern"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "Es gilt ab dem nächsten Aufruf, ohne Neustart. Ist kein Verb markiert, wird der Zugriff entfernt.",
    ),
    ("Revoke {name}", "{name} widerrufen"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "Widerrufen schließt den Hermes im Hub: das verwendete Gerät, die laufende Anfrage und die ausgestellte Einladung. Der Kanal bricht innerhalb von Sekunden ab.",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "Die Zugriffe bleiben erhalten und gelten wieder, sobald ein neues Gerät verbunden ist. Der einzige Weg zurück ist eine neue Einladung.",
    ),
    // data.rs
    ("never", "nie"),
    ("now", "jetzt"),
    ("{n} min ago", "vor {n} Min."),
    ("{n} h ago", "vor {n} Std."),
    ("{n} d ago", "vor {n} Tg."),
    (
        "a credential that had already been swapped was presented",
        "ein bereits gewechselter Berechtigungsnachweis wurde vorgelegt",
    ),
    (
        "two connections at the same time",
        "zwei Verbindungen gleichzeitig",
    ),
    (
        "the credential could not be read",
        "der Berechtigungsnachweis konnte nicht gelesen werden",
    ),
    (
        "the last use was not recognized on the reconnect",
        "die letzte Verwendung wurde beim erneuten Verbinden nicht erkannt",
    ),
    (
        "the identity was copied before the approval",
        "die Identität wurde vor der Genehmigung kopiert",
    ),
    (
        "the hub no longer trusts the credential",
        "der Hub vertraut dem Berechtigungsnachweis nicht mehr",
    ),
    ("Waiting for your code", "Wartet auf Ihren Code"),
    (
        "Approved, waiting for the connector",
        "Genehmigt, wartet auf den Connector",
    ),
    ("Waiting for the Hermes", "Wartet auf den Hermes"),
    ("Connected", "Verbunden"),
    ("Disconnected", "Getrennt"),
    ("In quarantine", "In Quarantäne"),
    ("Suspended", "Gesperrt"),
    ("No device", "Kein Gerät"),
    ("a Hermes", "Hermes (unbekannt)"),
    ("Invite", "Einladung"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "ein anderes Gerät hat versucht, die Einladung für {hermes} zu verwenden. Der Block ist an eine andere Stelle gelangt. Die erste Anfrage bleibt bestehen.",
    ),
    ("Identity copied", "Identität kopiert"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "die Identität von {hermes} wurde vor der Genehmigung kopiert. Die Anfrage wurde abgelehnt.",
    ),
    ("Hermes in quarantine", "Hermes in Quarantäne"),
    ("{hermes}. Reason: {reason}.", "{hermes}. Grund: {reason}."),
    ("Suspected copy", "Verdacht auf Kopie"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "die letzte Verwendung von {hermes} wurde beim erneuten Verbinden nicht erkannt.",
    ),
    ("Connection taken", "Verbindung übernommen"),
    (
        "the connection of {hermes} was taken by another one.",
        "die Verbindung von {hermes} wurde von einem anderen übernommen.",
    ),
    ("Closing", "Schließung"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} hat die eigene Schließung angefordert.",
    ),
    ("Reuse", "Wiederverwendung"),
    (
        "a copy of {hermes} was used before the closing.",
        "eine Kopie von {hermes} wurde vor der Schließung verwendet.",
    ),
    ("Unconfirmed swaps", "Unbestätigte Wechsel"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} hat Berechtigungsnachweise erhalten und nicht bestätigt, dreimal in 24 Stunden. Es kann ein schlechtes Netz sein oder jemand, der eine Kopie ausprobiert.",
    ),
    ("Audit mark", "Audit-Markierung"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "ein Sicherheitsereignis wurde geschrieben, und die Markierung außerhalb der Datenbank ist nicht gefolgt.",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "die Version der Engine ist keine, die der Hub geprüft hat.",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "eine Engine-Sitzung hatte eine andere Konfiguration, und der Hub hat seine eigene angewendet.",
    ),
    ("Number swapped", "Nummer vertauscht"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "das gekoppelte Telefon gehört zu einer anderen Nummer als der angegebenen. Die Zugriffe bleiben gestoppt, bis Sie entscheiden.",
    ),
    ("Address: {address}.", "Adresse: {address}."),
    ("an unknown date", "unbekanntes Datum"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "Der Hub wurde aus einer Sicherung wiederhergestellt, Stand: {date}. Was danach genehmigt, widerrufen, neu verbunden oder an den Zugriffen geändert wurde, ist verloren. Verbinden Sie jeden Hermes neu und prüfen Sie die Zugriffe.",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "Der Hub wurde aus einer Sicherung wiederhergestellt. {name} ist gesperrt und kommt nur mit einer neuen Einladung zurück: neu verbinden oder widerrufen. Ein „Reaktivieren“ gibt es nicht.",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "Der Hub vertraut dem Berechtigungsnachweis von {name} nicht mehr. Grund: {reason}. Der Zugriff ist gestoppt. Auswege: neu verbinden oder widerrufen. Ein „Freigeben“ gibt es nicht.",
    ),
    ("Not paired", "Nicht gekoppelt"),
    ("Pairing", "Kopplung läuft"),
    ("Reconnecting", "Verbindet neu"),
    ("Failed", "Fehlgeschlagen"),
    ("Removed", "Entfernt"),
    ("version {version}, checked", "Version {version}, geprüft"),
    (
        "version {version}, not checked by the hub",
        "Version {version}, vom Hub nicht geprüft",
    ),
    ("version not read yet", "Version noch nicht gelesen"),
    ("last answer {when}", "letzte Antwort {when}"),
    ("has not answered yet", "hat noch nicht geantwortet"),
    ("all", "alle"),
    ("all · own rules: {n}", "alle · eigene Regeln: {n}"),
    ("chosen chats: {n}", "ausgewählte Chats: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: {n} Hermes werden in denselben Chats geweckt.",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: {n} Hermes senden in denselben Chats.",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} ist als Befehlsnummer markiert, und es gibt Hermes, die darauf senden dürfen.",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "Es gibt bereits einen Hermes mit diesem Namen. Um ein weiteres Gerät mit ihm zu verknüpfen, verwenden Sie die Schaltfläche in seiner Zeile.",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "Dieser Name gehörte einem Hermes, der entfernt wurde, und ein Name wird nie wiederverwendet. Wählen Sie einen anderen.",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "Geben Sie vor dem Code an, ob Sie die letzte Verwendung des aktuellen Geräts erkennen. Es wurde nichts gezählt.",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "Das aktuelle Gerät wurde verwendet, während Sie geantwortet haben. Sehen Sie sich jetzt seine letzte Verwendung an und antworten Sie erneut. Es wurde nichts genehmigt.",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "Der Code hat 6 Ziffern, wie 123-456. Es wurde nichts gezählt.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "Die Datenbank stammt aus einer älteren Sicherung, und der Hub hat das noch nicht verarbeitet. Es wurde nichts geschrieben. Starten Sie den Hub neu: Er schaltet den Wiederherstellungsmodus ein, und danach wird jeder Hermes mit einer neuen Einladung neu verbunden.",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "Im Hub ist etwas fehlgeschlagen, und es wurde nichts ausgeführt. Sehen Sie im Protokoll des Hubs nach.",
    ),
    (
        "The name has from 1 to 32 characters.",
        "Der Name hat 1 bis 32 Zeichen.",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "Der Name beginnt mit einem Kleinbuchstaben oder einer Ziffer.",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "Verwenden Sie nur Kleinbuchstaben, Ziffern, Bindestrich und Unterstrich.",
    ),
    (
        "That name is reserved. Choose another.",
        "Dieser Name ist reserviert. Wählen Sie einen anderen.",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "Der Code stimmt nicht. Versuch {attempts} von {max}. Geben Sie den letzten Code ein, den der Hermes geantwortet hat.",
    ),
    // mod.rs
    (
        "Invalid or expired code. Issue another one on the hub's server.",
        "Ungültiger oder abgelaufener Code. Stellen Sie auf dem Server des Hubs einen neuen aus.",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Im Moment zu viele Versuche. Warten Sie eine Minute und versuchen Sie es erneut.",
    ),
];
