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
    ("Sign in with a passkey", "Mit Passkey anmelden"),
    ("Installation code", "Installationscode"),
    ("Continue", "Fortsetzen"),
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
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "Dieser Code hat nicht funktioniert. Verwenden Sie den neuesten: den aus dem letzten Block im Log oder einen, der auf dem Server des Hubs ausgestellt wurde.",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "Der Hub hat diesen Passkey nicht akzeptiert. Versuchen Sie es erneut.",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "Ihre Sitzung im Panel ist beendet. Geben Sie den neuesten Code ein, um dort weiterzumachen, wo Sie aufgehört haben.",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "Dieser Browser kann hier keinen Passkey verwenden. Öffnen Sie das Panel in einem aktuellen Browser, direkt unter seiner Adresse (nicht innerhalb einer anderen App).",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "Der Passkey wurde nicht erstellt: Die Anfrage wurde abgebrochen oder hat zu lange gedauert. Versuchen Sie es einfach noch einmal.",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "Es wurde kein Passkey verwendet. Falls er nicht auf diesem Gerät liegt, nutzen Sie das Gerät oder den Passwortmanager, in dem Sie ihn erstellt haben, oder öffnen Sie „Passkey verloren?“.",
    ),
    ("Passkey", "Passkey"),
    ("Check", "Prüfen"),
    (
        "Welcome to your Arauto hub",
        "Willkommen bei Ihrem Arauto-Hub",
    ),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "Richten wir Ihren ersten Zugang ein. Es sind drei kurze Schritte, und Sie machen das nur einmal.",
    ),
    (
        "Step 1 · Confirm it is you",
        "Schritt 1 · Bestätigen Sie, dass Sie es sind",
    ),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "Als der Hub gestartet ist, hat er einen Installationscode in sein eigenes Log geschrieben. Nur wer den Server betreut, sieht dieses Log: So erkennt der Hub, dass Sie es sind.",
    ),
    ("Where to find the code:", "Wo Sie den Code finden:"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "Öffnen Sie in Easypanel den Dienst des Hubs und wechseln Sie zu Logs. (Mit Docker: {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "Suchen Sie den Block ARAUTO FIRST ACCESS: Der neueste ist der letzte, ganz unten im Log. Kopieren Sie seinen Code.",
    ),
    ("Paste the code here:", "Fügen Sie den Code hier ein:"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "Der Code läuft nicht ab und funktioniert nur einmal. Startet der Hub neu, schreibt er einen neuen: Verwenden Sie immer den neuesten.",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "Sie können das Log nicht öffnen? Führen Sie auf dem Server des Hubs {command} aus. Achtung: Dieser neue Code macht den Code im Log ungültig.",
    ),
    ("Good to have you back", "Schön, dass Sie wieder da sind"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "Melden Sie sich mit Ihrem Passkey an: Fingerabdruck, Gesicht oder PIN dieses Geräts oder Ihr Passwortmanager.",
    ),
    ("Lost your passkey?", "Passkey verloren?"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "Kein Problem. Führen Sie auf dem Server des Hubs {command} aus. Der Befehl zeigt einen Code, der 10 Minuten gilt: Geben Sie ihn hier ein und erstellen Sie einen neuen Passkey.",
    ),
    (
        "Code accepted! Now, create your passkey",
        "Code akzeptiert! Erstellen Sie jetzt Ihren Passkey",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "Damit melden Sie sich ab jetzt am Panel an, ohne ein Passwort, das Sie sich merken müssen.",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "Der Passkey bleibt auf diesem Gerät oder in Ihrem Passwortmanager und wird mit Ihrem Fingerabdruck, Ihrem Gesicht oder der PIN des Geräts geöffnet. Der Hub bewahrt nur auf, was den Passkey prüft: Es gibt kein Passwort, das durchsickern könnte.",
    ),
    (
        "It will work at this address:",
        "Er funktioniert unter dieser Adresse:",
    ),
    ("Create my passkey", "Meinen Passkey erstellen"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "Wenn Sie einen Passwortmanager verwenden, kann der Passkey mit auf Ihre anderen Geräte wandern.",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "Diese Seite versehentlich verlassen? Geben Sie denselben Code noch einmal ein: Er wird erst verbraucht, wenn der Passkey erstellt wird.",
    ),
    ("Check the installation", "Installation prüfen"),
    // guide.rs
    ("Name", "Name"),
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
    ("How it works:", "So funktioniert es:"),
    (
        "You give the Hermes a name here.",
        "Hier geben Sie dem Hermes einen Namen.",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Der Hermes lädt den Connector von diesem Hub herunter und antwortet mit einem 6-stelligen Prüfcode.",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "Sie geben den Code hier ein und genehmigen die Verbindung. Fertig: Der Hermes wird als verbunden angezeigt, sobald sein Connector bestätigt.",
    ),
    (
        "Let's connect your first Hermes",
        "Verbinden wir Ihren ersten Hermes",
    ),
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
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "der Hub benachrichtigt, wenn eine Nachricht eintrifft (setzt Lesen voraus). Wird es ausgeschaltet, wird es auch in den Chats dieses Zugriffs mit eigener Regel ausgeschaltet.",
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
        "Too many attempts right now. Wait a minute and try again.",
        "Im Moment zu viele Versuche. Warten Sie eine Minute und versuchen Sie es erneut.",
    ),
    // check.rs
    ("ok", "ok"),
    ("check", "prüfen"),
    ("attention", "Achtung"),
    ("later", "später"),
    ("failed", "fehlgeschlagen"),
    ("Panel address", "Adresse des Panels"),
    ("Connectors address", "Adresse der Connectors"),
    ("Your address", "Ihre Adresse"),
    ("Database password", "Datenbankpasswort"),
    ("Data folder", "Datenordner"),
    ("WhatsApp engine", "WhatsApp-Engine"),
    ("Your passkey works here.", "Ihr Passkey funktioniert hier."),
    (
        "Open it: it should say it is the address of the connectors.",
        "Öffnen Sie sie: Dort sollte stehen, dass dies die Adresse der Connectors ist.",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "Der Hub sieht Sie unter {address}. Ist das Ihre Adresse? Wenn nicht, leitet der Proxy die echte Adresse nicht weiter.",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Eine Verbindung von {address}, die nicht in ARAUTO_TRUSTED_PROXIES steht, hat eine weitergeleitete Adresse mitgebracht. Tragen Sie die Adresse dieser Verbindung in ARAUTO_TRUSTED_PROXIES ein und deployen Sie neu.",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Der Hub sieht Sie unter {address}, einer Adresse aus einem internen Netz: Dem Proxy wird noch nicht vertraut. Tragen Sie sie in ARAUTO_TRUSTED_PROXIES ein und deployen Sie neu.",
    ),
    (
        "Replaced by one nobody saw.",
        "Durch ein neues ersetzt, das niemand gesehen hat.",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "Nicht durch den Hub ersetzt: Diese Installation legt das Datenbankpasswort selbst fest.",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "Der Austausch ist fehlgeschlagen. Sehen Sie im Protokoll des Hubs nach und führen Sie dann {command} aus.",
    ),
    ("Ready for your Hermes.", "Bereit für Ihre Hermes."),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "Der Download-Ordner ist leer: Ein Hermes kann über das Panel nicht verbunden werden. Bauen Sie das Image des Hubs neu.",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "Er enthält den Hauptschlüssel und das Datenbankpasswort. Sichern Sie ihn: Ohne ihn kann der Hub nicht öffnen, was er verschlüsselt aufbewahrt hat.",
    ),
    ("Configured.", "Konfiguriert."),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "Ein Hermes lässt sich ohne sie verbinden; WhatsApp-Nummern brauchen sie.",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "Der Hub hat einiges selbst geprüft. Hier wird nichts verändert: Es dient nur dazu, dass Sie wissen, wie es ausgegangen ist.",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "Schritt 3 · Prüfen wir die Installation",
    ),
    (
        "All set: your passkey was created!",
        "Geschafft: Ihr Passkey wurde erstellt!",
    ),
    (
        "The same by command: {command}",
        "Dasselbe per Befehl: {command}",
    ),
    ("Connect my first Hermes", "Meinen ersten Hermes verbinden"),
    ("Do it later", "Später erledigen"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "Ich bitte Sie persönlich darum: Verbinden Sie diesen Hermes als „{name}“ mit unserem Arauto-Hub.",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "Lesen Sie {page} und befolgen Sie die Anweisungen darauf. Fragen Sie mich, wenn etwas unklar ist.",
    ),
    ("Hub:", "Hub:"),
    ("Hub key:", "Hub-Schlüssel:"),
    (
        "Connector sha256 ({system}):",
        "sha256 des Connectors ({system}):",
    ),
    (
        "Invite (one use, until {deadline} UTC):",
        "Einladung (einmalig, bis {deadline} UTC):",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "Wenn Sie fertig sind, schicken Sie mir den Prüfcode und die Zeilen, die die Seite verlangt.",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "Senden Sie den Text als neue Nachricht, nicht als Antwort auf einen seiner Hinweise. Wenn er Sie um eine Bestätigung bittet, antworten Sie mit Ihren eigenen Worten.",
    ),
    (
        "Show the command instead",
        "Stattdessen den Befehl anzeigen",
    ),
    (
        "Send one or the other, not both: they carry the same invite.",
        "Senden Sie eins von beiden, nicht beides: Beide enthalten dieselbe Einladung.",
    ),
    (
        "Click the message to copy it.",
        "Klicken Sie auf die Nachricht, um sie zu kopieren.",
    ),
    ("Message", "Nachricht"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "Der Hub erzeugt eine Einladung und schreibt damit eine Nachricht, die Sie im Befehlskanal dieses Hermes senden, und nirgendwo sonst.",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "Kleinbuchstaben, Ziffern, Bindestrich und Unterstrich, bis zu 32 Zeichen, beginnend mit einem Buchstaben oder einer Ziffer. Der Name erscheint in der Nachricht und ändert sich danach nicht mehr.",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Die Nachricht weist den Hermes an, den Connector von diesem Hub herunterzuladen, und der Download-Ordner ist leer. Es wurde nichts ausgestellt. Stellen Sie die Einladung für {name} mit dem Befehl {command} auf dem Server des Hubs aus.",
    ),
    ("Org chart", "Organigramm"),
    ("Who talks to [whom]", "Wer spricht mit [wem]?"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "Ziehen Sie einen Hermes, um seine Ebene zu ändern. Ziehen Sie den lila Punkt einer Karte auf eine andere Karte, um eine Verknüpfung zu erstellen. Klicken Sie auf einen Pfeil oder eine Karte, um sie anzupassen.",
    ),
    ("View", "Ansicht"),
    ("Map", "Karte"),
    ("List", "Liste"),
    ("+ Level", "+ Ebene"),
    ("Undo", "Rückgängig"),
    ("Publish", "Veröffentlichen"),
    ("Draft: 1 change", "Entwurf: 1 Änderung"),
    ("Draft: {n} changes", "Entwurf: {n} Änderungen"),
    (
        "From a higher level to a lower one: instruction",
        "Von einer höheren zu einer niedrigeren Ebene: Anweisung",
    ),
    (
        "From a lower level to a higher one: request",
        "Von einer niedrigeren zu einer höheren Ebene: Anfrage",
    ),
    ("Same level: note", "Gleiche Ebene: Notiz"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "Die Ebenen und die Rollen werden nur hier angezeigt: Kein Hermes erfährt davon. Eine Notiz ist eine Angabe, außer an einem Anweisungspfeil mit der Folgen-Markierung: Dann sagt der Hinweis dem niedrigeren Hermes, diese neuen Nachrichten des höheren in diesem Durchlauf zu befolgen, unterhalb der Anweisungen seines Betreibers. Nur eine Verknüpfung erlaubt es einem Hermes, einem anderen eine Notiz zu schicken.",
    ),
    ("instruction", "Anweisung"),
    ("request", "Anfrage"),
    ("note", "Notiz"),
    ("Name of level {n}", "Name der Ebene {n}"),
    ("Remove empty level", "Leere Ebene entfernen"),
    ("Move up", "Nach oben"),
    ("Move down", "Nach unten"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "Ein Ebenenname hat 1 bis 32 Kleinbuchstaben, Ziffern, Bindestriche oder Unterstriche, beginnt mit einem Buchstaben oder einer Ziffer und ist kein reserviertes Wort.",
    ),
    (
        "Another level already has this name.",
        "Eine andere Ebene hat bereits diesen Namen.",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "Das Organigramm hat 1 bis {max} Ebenen.",
    ),
    (
        "Drag to another Hermes to create a link",
        "Auf einen anderen Hermes ziehen, um eine Verknüpfung zu erstellen",
    ),
    ("{alias} · answers", "{alias} · antwortet"),
    ("{alias} · reads only", "{alias} · liest nur"),
    ("no number", "keine Nummer"),
    ("not placed yet", "noch ohne Platz"),
    (
        "{name}, level {level}, {state}",
        "{name}, Ebene {level}, {state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "Noch ohne Platz: {names}. Sie werden in der untersten Ebene angezeigt; veröffentlichen Sie, damit sie dort bleiben.",
    ),
    ("Kind: {kind}", "Art: {kind}"),
    ("Wakes {name}", "Weckt {name}"),
    ("Create the way back", "Rückweg anlegen"),
    ("Remove the way back", "Rückweg entfernen"),
    ("Remove", "Entfernen"),
    (
        "This link already exists.",
        "Diese Verknüpfung existiert bereits.",
    ),
    ("Role of this Hermes", "Rolle dieses Hermes"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "Zu Ihrer eigenen Information: In dieser Version sieht kein Hermes sie. Bis zu 280 Zeichen, in einer Zeile.",
    ),
    ("Level", "Ebene"),
    ("Move left", "Nach links verschieben"),
    ("Move right", "Nach rechts verschieben"),
    ("Change them in Accesses", "Ändern Sie sie unter Zugriffe"),
    ("No access to any number.", "Kein Zugriff auf eine Nummer."),
    ("Link to", "Verknüpfen mit"),
    ("Add link", "Verknüpfung hinzufügen"),
    ("Links of this Hermes", "Verknüpfungen dieses Hermes"),
    ("Levels", "Ebenen"),
    ("Links", "Verknüpfungen"),
    ("Role", "Rolle"),
    ("From", "Von"),
    ("To", "An"),
    ("Kind", "Art"),
    ("Wakes", "Weckt"),
    ("nobody", "niemand"),
    ("No link.", "Keine Verknüpfung."),
    (
        "Publish these changes?",
        "Diese Änderungen veröffentlichen?",
    ),
    (
        "Links are security rules. They only apply after you publish.",
        "Verknüpfungen sind Sicherheitsregeln. Sie gelten erst nach dem Veröffentlichen.",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "Eine Verknüpfung lässt eine Anfrage über Nummern hinweg laufen: Was einen Hermes über eine Nummer erreicht, kann für den anderen zur Notiz werden, der über seine eigenen Nummern handelt.",
    ),
    ("Back", "Zurück"),
    (
        "New link: {from} → {to} ({kind})",
        "Neue Verknüpfung: {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "Neue Verknüpfung: {from} → {to} ({kind}), weckt {to}",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "Verknüpfung {from} → {to}: weckt jetzt {to}",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "Verknüpfung {from} → {to}: weckt {to} nicht mehr",
    ),
    (
        "Link removed: {from} → {to}",
        "Verknüpfung entfernt: {from} → {to}",
    ),
    (
        "{name} changes level: {before} → {after}",
        "{name} wechselt die Ebene: {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} kommt auf Ebene {after}",
    ),
    ("Role of {name} changed", "Rolle von {name} geändert"),
    (
        "Places on the map changed: {n}",
        "Positionen auf der Karte geändert: {n}",
    ),
    ("New level: {name}", "Neue Ebene: {name}"),
    (
        "Level renamed: {before} → {after}",
        "Ebene umbenannt: {before} → {after}",
    ),
    ("Level removed: {name}", "Ebene entfernt: {name}"),
    (
        "Order of the levels: {names}",
        "Reihenfolge der Ebenen: {names}",
    ),
    ("Published.", "Veröffentlicht."),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "Jemand hat das Organigramm geändert, während Sie bearbeiteten: eine andere Veröffentlichung, eine per Befehl geänderte Verknüpfung oder ein hinzugefügter oder entfernter Hermes. Nichts wurde veröffentlicht. Ihr Entwurf bleibt erhalten: Prüfen Sie die Änderungen und veröffentlichen Sie erneut.",
    ),
    ("Nothing was published.", "Nichts wurde veröffentlicht."),
    (
        "Two levels have the same name.",
        "Zwei Ebenen haben denselben Namen.",
    ),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "Eine Rolle hat höchstens 280 Zeichen, in einer Zeile, ohne Steuer- oder Richtungszeichen.",
    ),
    (
        "A place on the map is out of range.",
        "Eine Position auf der Karte liegt außerhalb des Bereichs.",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "Der Entwurf nennt einen Hermes oder eine Ebene, die es nicht gibt, oder lässt einen Hermes aus.",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "Eine Verknüpfung verbindet zwei verschiedene Hermes, einmal in jede Richtung.",
    ),
    (
        "The draft could not be read.",
        "Der Entwurf konnte nicht gelesen werden.",
    ),
    (
        "The draft is too large to publish.",
        "Der Entwurf ist zu groß zum Veröffentlichen.",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "Im Hub ist etwas fehlgeschlagen. Nichts wurde veröffentlicht. Sehen Sie im Protokoll des Hubs nach.",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "Zum Bearbeiten des Organigramms wird das Skript des Panels benötigt. Ohne es ist dies das Organigramm, wie es veröffentlicht ist.",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "Noch kein Hermes. Verbinden Sie den ersten im Tab Hermes.",
    ),
    ("Connect WhatsApp (WAHA)", "WhatsApp verbinden (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "WhatsApp verbinden (nativ · im Test)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "Noch keine Nummer. Fügen Sie die erste mit der Schaltfläche „WhatsApp verbinden (WAHA)“ hinzu.",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "In dieser Version verbindet und widerruft das Panel Hermes, koppelt und verwaltet WhatsApp-Nummern über WAHA, ändert die Zugriffe jeder Nummer und bearbeitet das Organigramm mit seinen Verknüpfungen. Das Auswählen von Chats erfolgt weiterhin mit dem Befehl {command} auf dem Server des Hubs.",
    ),
    ("Pair", "Koppeln"),
    ("Pair again", "Erneut koppeln"),
    ("Continue pairing", "Kopplung fortsetzen"),
    ("See the pairing", "Kopplung anzeigen"),
    ("Disconnect", "Trennen"),
    ("Restart", "Neu starten"),
    ("Decide", "Entscheiden"),
    ("Number {alias}: {state}.", "Nummer {alias}: {state}."),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "Der Hub hat die Nummer {alias} geändert, aber WAHA hat nicht geantwortet: Die Sitzung der Nummer dort ist möglicherweise noch wie vorher. Prüfen Sie WAHA mit {command} auf dem Server des Hubs.",
    ),
    ("Method", "Methode"),
    ("Phone", "Telefon"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "So nennen das Panel und die Hermes diese Nummer: Kleinbuchstaben, Ziffern, Bindestrich und Unterstrich, bis zu 32. Ein Alias wird nie wiederverwendet.",
    ),
    ("Number with the country code", "Nummer mit Ländervorwahl"),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "Zum Beispiel +55 11 9XXXX-XXXX. Nur Ziffern, Leerzeichen, Bindestriche und Klammern.",
    ),
    (
        "It is the command channel of a Hermes",
        "Er ist der Befehlskanal eines Hermes",
    ),
    (
        "It shows as \"command\" in the list.",
        "In der Liste erscheint er als „Befehl“.",
    ),
    ("Before the first number", "Vor der ersten Nummer"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "Die Engine ist ein inoffizieller WhatsApp-Client. WhatsApp kann die Nummer sperren. Verwenden Sie Nummern, deren Verlust Sie verschmerzen können.",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "Ich verstehe und akzeptiere. Der Hub protokolliert diese Zustimmung im Audit.",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "Die Nummer wird über WAHA gekoppelt, die auf diesem Hub eingestellte Engine.",
    ),
    ("Add and pair", "Hinzufügen und koppeln"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA hat dem Hub noch nicht geantwortet. Sie können die Nummer hinzufügen; zum Koppeln muss WAHA antworten.",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "Auf diesem Hub gibt es noch keine WhatsApp-Engine. Richten Sie WAHA auf dem Server des Hubs mit {command} ein, danach den Schlüssel mit {key_command}. Der Schlüssel wird nie im Panel eingetippt.",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "Die Engine hat noch keinen Schlüssel. Legen Sie ihn auf dem Server des Hubs mit {command} fest. Der Schlüssel wird nie im Panel eingetippt.",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "Der Alias hat 1 bis 32 Zeichen.",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "Der Alias beginnt mit einem Kleinbuchstaben oder einer Ziffer.",
    ),
    (
        "That alias is reserved. Choose another.",
        "Dieser Alias ist reserviert. Wählen Sie einen anderen.",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "Dieser Alias ist vergeben, jetzt oder durch eine entfernte Nummer, und ein Alias wird nie wiederverwendet. Wählen Sie einen anderen.",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "Geben Sie die Nummer mit Ländervorwahl ein: 8 bis 15 Ziffern, zum Beispiel +55 11 9XXXX-XXXX.",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "Diese Nummer ist bereits ein WhatsApp dieses Hubs.",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "Lesen Sie den Satz zum Risiko und markieren Sie, dass Sie ihn akzeptieren. Es wurde nichts hinzugefügt.",
    ),
    ("Pair {alias}", "{alias} koppeln"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "Halten Sie das Telefon von {phone} bereit, mit geöffnetem WhatsApp. Die Kopplung dauert bis zu {minutes} Minuten.",
    ),
    (
        "How will the phone pair?",
        "Wie soll das Telefon gekoppelt werden?",
    ),
    ("QR code", "QR-Code"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "mit der Kamera des Telefons scannen, aus WhatsApp heraus",
    ),
    ("Pairing code", "Kopplungscode"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "einen Code auf dem Telefon eingeben, falls es diesen Bildschirm nicht scannen kann",
    ),
    ("Start pairing", "Kopplung starten"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA hat die Sitzung dieser Nummer noch nicht angelegt. Beim Starten der Kopplung wird es erneut versucht.",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "Wählen Sie QR-Code oder Kopplungscode. Es wurde nichts gestartet.",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "{n} Nummern werden gerade gekoppelt, das Maximum gleichzeitig. Warten Sie, bis eine fertig ist, oder brechen Sie eine ab.",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "Das ist nicht möglich, solange die Nummer im Status „{state}“ ist. Es wurde nichts getan.",
    ),
    ("Starting the pairing…", "Kopplung wird gestartet…"),
    (
        "QR code to scan with the phone",
        "QR-Code zum Scannen mit dem Telefon",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "Der QR-Code ändert sich alle paar Sekunden: Dieses Fenster zeigt den aktuellen.",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "Der Code gilt, solange dieses Fenster herunterzählt: noch {time}. WhatsApp kann ihn früher beenden.",
    ),
    ("Time left to scan: {time}.", "Zeit zum Scannen: {time}."),
    (
        "This window refreshes by itself.",
        "Dieses Fenster aktualisiert sich von selbst.",
    ),
    ("Refresh now", "Jetzt aktualisieren"),
    ("Cancel the pairing", "Kopplung abbrechen"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "Der QR-Code ist zu lang, um ihn hier darzustellen. Brechen Sie ab und koppeln Sie mit einem Kopplungscode.",
    ),
    ("Paired: {phone}.", "Gekoppelt: {phone}."),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "Die Nummer {alias} ist verbunden. Geben Sie einem Hermes im Tab Zugriffe Zugang dazu.",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "Es wurde nichts rechtzeitig gescannt oder eingegeben. Die Nummer ist nicht gekoppelt; koppeln Sie sie mit dem Telefon zur Hand erneut.",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA hat einen Fehler der Sitzung gemeldet. Die Nummer ist nicht gekoppelt. Versuchen Sie es erneut; schlägt es wieder fehl, prüfen Sie WAHA.",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "Die Kopplung wurde abgebrochen. Die Nummer ist nicht gekoppelt.",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "Diese Nummer wird in einem anderen Fenster gekoppelt. Nur jenes Fenster zeigt den QR-Code oder den Code.",
    ),
    ("Cancel that pairing", "Diese Kopplung abbrechen"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "Diese Kopplung läuft auf dem Hub nicht mehr: Sie ist beendet, oder der Hub wurde neu gestartet. Die Nummer ist im Status „{state}“.",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "Der Hub konnte WAHA nicht erreichen. Prüfen Sie, ob er läuft und im Netz des Hubs ist.",
    ),
    (
        "WAHA refused the key the hub has. Set the right key on the hub's server with {command}.",
        "WAHA hat den Schlüssel abgelehnt, den der Hub hat. Legen Sie den richtigen Schlüssel auf dem Server des Hubs mit {command} fest.",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA hat mit einem Fehler geantwortet oder zu spät. Versuchen Sie es gleich noch einmal; {command} auf dem Server des Hubs gibt mehr Auskunft.",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp begrenzt diese Nummer. Versuchen Sie es später noch einmal.",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA hat die Anfrage abgelehnt. Sehen Sie im Log des Hubs nach.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "Im Hub ist etwas fehlgeschlagen. Sehen Sie im Log des Hubs nach und prüfen Sie die Nummer in der Liste.",
    ),
    (
        "{alias}: another phone paired",
        "{alias}: ein anderes Telefon wurde gekoppelt",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "Das gekoppelte Telefon gehört zu einer anderen Nummer als der angegebenen.",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "Bis Sie entscheiden, ist die Nummer im Status „Nummer vertauscht“, und ihre Zugriffe bleiben gestoppt.",
    ),
    ("Keep this one", "Diese behalten"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "Rückgängig machen entfernt WAHA aus dem WhatsApp dieses Telefons (es verlässt dessen verknüpfte Geräte) und löscht den Verlauf, den WAHA zu dieser Nummer speichert.",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "Ich verstehe, dass WAHA den Verlauf zu dieser Nummer löscht.",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "Markieren Sie, dass Sie verstehen, was Rückgängig machen löscht. Es wurde nichts getan.",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "Das gekoppelte Telefon ist bereits eine andere Nummer dieses Hubs. Machen Sie diese Kopplung rückgängig oder entfernen Sie zuerst die andere Nummer.",
    ),
    ("Disconnect {alias}", "{alias} trennen"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "Trennen beendet die Sitzung dieser Nummer in WAHA. Dabei wird WAHA nicht aus WhatsApp entfernt, und WAHA behält seinen Verlauf. Solange sie getrennt ist, liest und sendet darüber kein Hermes. Um sie wieder zu nutzen, koppeln Sie sie erneut.",
    ),
    (
        "Type the alias {alias} to confirm.",
        "Geben Sie zur Bestätigung den Alias {alias} ein.",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "Der eingegebene Alias stimmt nicht überein. Es wurde nichts getan.",
    ),
    ("Remove {alias}", "{alias} entfernen"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "Entfernen beendet jetzt alle Zugriffe auf diese Nummer. Ihr Alias wird nie wieder verwendet.",
    ),
    (
        "What happens to its session in WAHA",
        "Was mit ihrer Sitzung in WAHA geschieht",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "Anhalten und in WAHA behalten, mit dem Verlauf, den WAHA speichert",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "WAHA aus WhatsApp entfernen und löschen: WAHA löscht den Verlauf, den es speichert",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "Arauto hat noch kein Nachrichtenarchiv: Der Verlauf besteht nur aus dem, was WAHA speichert.",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "Wählen Sie, was mit ihrer Sitzung in WAHA geschieht. Es wurde nichts getan.",
    ),
    ("In test", "Im Test"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Eine in Arauto eingebaute Engine für WhatsApp wird gerade untersucht. Sie ist nicht Teil dieser Version, und dieses Fenster tut nichts.",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "Vorerst werden Nummern über WAHA gekoppelt, mit der Schaltfläche „WhatsApp verbinden (WAHA)“.",
    ),
    ("Open the WAHA window", "Das WAHA-Fenster öffnen"),
    ("Try again", "Erneut versuchen"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "Mit dem Telefon scannen: WhatsApp › Verknüpfte Geräte › Gerät verknüpfen.",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "Auf dem Telefon: WhatsApp › Verknüpfte Geräte › Gerät verknüpfen › Stattdessen mit Telefonnummer verknüpfen, und geben Sie diesen Code ein:",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "Mit dieser Markierung weckt eine Notiz über diese Verknüpfung {name}, innerhalb der Grenzen für Hinweise.",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} folgt den Anweisungen von {from}",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "Mit dieser Markierung sagt der Hinweis einer Notiz {to}, in diesem Durchlauf zu tun, was {from} verlangt, ohne auf seinen Betreiber zu warten, es sei denn, sein Betreiber verbietet es. Nur an einem Anweisungspfeil, nur mit der Weck-Markierung; {to} braucht die Verknüpfung zurück, um zu berichten. Es ist ein Satz an eine KI, keine Kontrolle.",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "Verknüpfung {from} → {to}: {to} folgt jetzt den Anweisungen von {from}",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "Verknüpfung {from} → {to}: {to} folgt den Anweisungen von {from} nicht mehr",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "Ein Hermes, der einem anderen folgt, tut, was dieser verlangt, ohne auf seinen Betreiber zu warten. Wer den höheren Hermes täuscht, über WhatsApp oder eine Notiz, erreicht auch den niedrigeren; wer den Hub kontrolliert, kann als jeder Hermes schreiben; und wer eine Sitzung im Panel hat, kann diese Markierungen setzen. Ein Connector einer älteren Version behandelt diese Notizen weiterhin als Angaben.",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "Eine Folgen-Markierung geht nur von einer höheren Ebene zu einer niedrigeren, an einer Verknüpfung, die weckt.",
    ),
];
