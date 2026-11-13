//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "Panneau"),
    ("Language", "Langue"),
    ("Change", "Changer"),
    ("R", "L"),
    ("S", "E"),
    ("M", "M"),
    ("W", "R"),
    ("Sign in to the panel", "Entrer dans le panneau"),
    ("Entry code", "Code d'entrée"),
    (
        "Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in.",
        "Émettez le code sur le serveur du hub, avec {command}. Il est valable 10 minutes et pour une seule entrée.",
    ),
    ("Sign in", "Entrer"),
    ("Sign out", "Quitter"),
    ("Alerts", "Alertes"),
    ("Unknown devices", "Appareils inconnus"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "Appareils ayant présenté un identifiant valide que la base de données n'explique pas : {n}. Pour chacun, le seul retour possible est une nouvelle invitation. Consultez-les avec {command}.",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "Autres alertes : {n}. Consultez-les toutes avec {command}.",
    ),
    ("Accesses", "Accès"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "Dans cette version, le panneau connecte et révoque les Hermes et modifie les accès de chaque numéro. L'appairage des numéros et le choix des conversations se font encore avec la commande {command}, sur le serveur du hub.",
    ),
    ("none", "aucun"),
    (
        "Your Hermes and their [numbers]",
        "Vos Hermes et leurs [numéros]",
    ),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "Connectés : {connected} · En attente : {waiting} · En alerte : {stopped}",
    ),
    ("Connect Hermes", "Connecter un Hermes"),
    ("State", "État"),
    ("Numbers", "Numéros"),
    ("Can be woken", "Peut être réveillé"),
    ("Connector", "Connecteur"),
    ("Seen", "Vu"),
    ("Actions", "Actions"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Aucun Hermes pour l'instant. Connectez le premier avec le bouton « Connecter un Hermes ».",
    ),
    ("yes", "oui"),
    ("no", "non"),
    ("Continue", "Continuer"),
    ("Connect", "Connecter"),
    ("Reconnect", "Reconnecter"),
    ("Revoke", "Révoquer"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "Les « Numéros » sont ceux que le Hermes atteint. Le connecteur est ce que l'appareil a déclaré : il n'est pas vérifié.",
    ),
    ("Engine", "Moteur"),
    (
        "There is no engine yet. Set it with {command}.",
        "Il n'y a pas encore de moteur. Définissez-le avec {command}.",
    ),
    ("Alias", "Alias"),
    ("Number", "Numéro"),
    ("Hermes with access", "Hermes ayant accès"),
    ("Last message", "Dernier message"),
    (
        "No number yet. Add one with {command}.",
        "Aucun numéro pour l'instant. Ajoutez-en un avec {command}.",
    ),
    ("command", "commande"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "Du dernier message, seule l'heure est affichée, jamais le contenu. Le panneau liste ce qui passe par le hub : les autres appareils liés au même numéro n'apparaissent que sur le téléphone.",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "Les Hermes en lignes, les numéros en colonnes. Cliquez sur une cellule pour la modifier.",
    ),
    ("Connect a Hermes first.", "Connectez d'abord un Hermes."),
    ("Add a WhatsApp first.", "Ajoutez d'abord un WhatsApp."),
    ("(accesses kept)", "(accès conservés)"),
    ("no access", "aucun accès"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} lire · {s} envoyer · {m} modifier · {w} être réveillé.",
    ),
    ("Saved: {what}", "Enregistré : {what}"),
    (
        "Warnings that block nothing",
        "Avertissements qui ne bloquent rien",
    ),
    // guide.rs
    ("Name", "Nom"),
    ("Block", "Bloc"),
    ("Code", "Code"),
    ("Done", "Terminé"),
    ("Invite issued", "Invitation émise"),
    (
        "A device used the invite",
        "Un appareil a utilisé l'invitation",
    ),
    (
        "Code checked and request approved",
        "Code vérifié et demande approuvée",
    ),
    (
        "The connector confirmed its first credential",
        "Le connecteur a confirmé son premier identifiant",
    ),
    (
        "A device used the invite, from {address}",
        "Un appareil a utilisé l'invitation, depuis {address}",
    ),
    ("waiting…", "en attente…"),
    (
        "What name will this Hermes have in the panel?",
        "Quel nom ce Hermes portera-t-il dans le panneau ?",
    ),
    ("Name of the Hermes", "Nom du Hermes"),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "Lettres minuscules, chiffres, trait d'union et tiret bas, jusqu'à 32 caractères, en commençant par une lettre ou un chiffre. Le nom apparaît dans le bloc et ne change plus ensuite.",
    ),
    (
        "Preparing a secure connection",
        "Préparation d'une connexion sécurisée",
    ),
    (
        "The hub drew the invite of {name}.",
        "Le hub a tiré l'invitation de {name}.",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "Invitation pour {name} · valable encore {time} · usage unique",
    ),
    ("Copied", "Copié"),
    (
        "Click the block to copy it.",
        "Cliquez sur le bloc pour le copier.",
    ),
    (
        "Select the whole text and copy it.",
        "Sélectionnez tout le texte et copiez-le.",
    ),
    (
        "It only appears here, once.",
        "Il n'apparaît qu'ici, une seule fois.",
    ),
    (
        "The invite issued before was cancelled.",
        "L'invitation émise auparavant a été annulée.",
    ),
    (
        "The request that was in progress was closed.",
        "La demande qui était en cours a été fermée.",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "L'appareil en service continue de fonctionner jusqu'à ce que le nouveau confirme son premier identifiant.",
    ),
    (
        "This hub has no connector to serve",
        "Ce hub n'a aucun connecteur à fournir",
    ),
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Le bloc demande au Hermes de télécharger le connecteur depuis ce hub, et le dossier des téléchargements est vide. Rien n'a été émis. Émettez l'invitation de {name} avec la commande {command}, sur le serveur du hub.",
    ),
    (
        "Waiting for the Hermes {name}",
        "En attente du Hermes {name}",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "Chaque étape s'allume quand le hub l'observe réellement. Si le Hermes demande une approbation pour exécuter la commande, c'est vous qui approuvez, dans le canal de commande.",
    ),
    (
        "A device used the invite of {name}",
        "Un appareil a utilisé l'invitation de {name}",
    ),
    ("unknown", "inconnue"),
    ("Seen by the hub", "Vu par le hub"),
    ("address {address}, {when}", "adresse {address}, {when}"),
    ("Said by the device", "Déclaré par l'appareil"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "machine « {host} » · système « {os} » · Hermes « {version} » (non vérifié)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "Dernière utilisation de l'appareil actuel : {when}, depuis {address}. La reconnaissez-vous ?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "Dernière utilisation de l'appareil actuel : {when}. La reconnaissez-vous ?",
    ),
    ("I recognize it", "Je la reconnais"),
    ("I do not recognize it", "Je ne la reconnais pas"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Si le Hermes était éteint et qu'il y a eu une utilisation, quelqu'un a utilisé une copie.",
    ),
    ("Verification code", "Code de vérification"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Saisissez le code que le Hermes a répondu dans le canal de commande. Le panneau n'affiche jamais ce code.",
    ),
    (
        "The request of {name} was denied",
        "La demande de {name} a été refusée",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "Le code a été mal saisi pour la cinquième fois. Pour connecter, émettez une autre invitation.",
    ),
    (
        "The request of {name} expired",
        "La demande de {name} a expiré",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "Les 10 minutes pour saisir le code sont écoulées. Pour connecter, émettez une autre invitation.",
    ),
    (
        "The identity of {name} was copied",
        "L'identité de {name} a été copiée",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "Deux appareils se sont présentés avec la même identité avant l'approbation, et la demande a été arrêtée. Pour connecter, émettez une autre invitation.",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} n'a ni invitation ni demande en attente",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "L'invitation a expiré, a été annulée, ou la demande a déjà été traitée. Si le Hermes n'a répondu aucun code, n'approuvez pas ; s'il a répondu AR-102, un autre appareil a utilisé l'invitation. Pour connecter, émettez une autre invitation.",
    ),
    ("{name} is connected", "{name} est connecté"),
    ("{name} was approved", "{name} a été approuvé"),
    (
        "What it may do on each number is in the Accesses tab.",
        "Ce qu'il peut faire sur chaque numéro se trouve dans l'onglet Accès.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "Dès que le connecteur confirme, il apparaît comme connecté. Les accès qu'il avait déjà s'appliquent de nouveau à cet appareil.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "Dès que le connecteur confirme, il apparaît comme connecté. Il n'a encore accès à aucun numéro.",
    ),
    (
        "The hub did not do what was asked",
        "Le hub n'a pas fait ce qui était demandé",
    ),
    ("Issue invite", "Émettre l'invitation"),
    ("Cancel", "Annuler"),
    ("Next step", "Étape suivante"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "Après l'avoir copié, collez-le dans le canal de commande du Hermes {name}. Nulle part ailleurs.",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "L'invitation est valable encore {time}. Cet écran s'actualise tout seul.",
    ),
    ("Approve", "Approuver"),
    ("Deny", "Refuser"),
    (
        "Confirm within {time}.",
        "Confirmez dans un délai de {time}.",
    ),
    ("Issue another invite", "Émettre une autre invitation"),
    ("Close", "Fermer"),
    ("See accesses", "Voir les accès"),
    ("Finish", "Terminer"),
    ("Copied to the clipboard.", "Copié dans le presse-papiers."),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "Le texte est sélectionné. Copiez-le avec Ctrl+C.",
    ),
    ("Reconnect {name}", "Reconnecter {name}"),
    ("Connect {name}", "Connecter {name}"),
    ("Steps", "Étapes"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "Être réveillé exige de lire. Rien n'a été enregistré.",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "Démarrer des conversations exige d'envoyer et la portée « toutes les conversations ». Rien n'a été enregistré.",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "L'attente avant le réveil va de 0 à 60 secondes. Rien n'a été enregistré.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "Une erreur s'est produite dans le hub. Consultez le journal du hub et vérifiez l'accès dans la matrice.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "La base de données est revenue d'une sauvegarde plus ancienne et le hub ne l'a pas encore traitée. Rien n'a été écrit. Redémarrez le hub.",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "Le Hermes {hermes} n'a aucun accès au numéro {number}.",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Le Hermes {hermes} n'atteint que les conversations choisies du numéro {number}. Conversations avec une ligne : {lines}.",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Le Hermes {hermes} sur le numéro {number}, dans toutes les conversations :",
    ),
    ("reads", "lit"),
    ("does not read", "ne lit pas"),
    ("sends", "envoie"),
    ("does not send", "n'envoie pas"),
    ("modifies", "modifie"),
    ("does not modify", "ne modifie pas"),
    (
        "is woken after {seconds} s of silence",
        "est réveillé après {seconds} s de silence",
    ),
    ("is not woken", "n'est pas réveillé"),
    ("may start chats", "peut démarrer des conversations"),
    (
        "chats with their own rule: {lines}",
        "conversations avec leur propre règle : {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} sur le numéro {number}",
    ),
    ("Where it applies", "Où cela s'applique"),
    (
        "Every chat, including the ones that come later",
        "Toutes les conversations, y compris celles à venir",
    ),
    ("Only chosen chats", "Seulement les conversations choisies"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "Avec « seulement les conversations choisies », seules les conversations qui ont leur propre ligne existent pour le Hermes, et ce qu'il peut faire dans chacune vient de la ligne. Les lignes sont encore créées avec la commande {command}. Aujourd'hui : {lines}.",
    ),
    ("What it may do", "Ce qu'il peut faire"),
    ("Read", "Lire"),
    (
        "see chats and messages, search, download files",
        "voir les conversations et les messages, rechercher, télécharger des fichiers",
    ),
    ("Send", "Envoyer"),
    (
        "write, react, mark as read",
        "écrire, réagir, marquer comme lu",
    ),
    ("Modify", "Modifier"),
    (
        "edit and delete what already exists",
        "modifier et supprimer ce qui existe déjà",
    ),
    ("Be woken", "Être réveillé"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "le hub avertit quand un message arrive (exige de lire)",
    ),
    ("Wait", "Attente"),
    (
        "seconds of silence before waking",
        "secondes de silence avant le réveil",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "Peut démarrer une conversation avec quelqu'un qui n'a jamais parlé au numéro (exige d'envoyer et toutes les conversations)",
    ),
    ("Today:", "Aujourd'hui :"),
    ("Save", "Enregistrer"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "Cela s'applique dès le prochain appel, sans rien redémarrer. Si aucun verbe n'est coché, l'accès est supprimé.",
    ),
    ("Revoke {name}", "Révoquer {name}"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "Révoquer ferme le Hermes dans le hub : l'appareil en service, la demande en cours et l'invitation émise. Le canal tombe en quelques secondes.",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "Les accès sont conservés et s'appliquent de nouveau quand un nouvel appareil est connecté. Le seul retour possible est une nouvelle invitation.",
    ),
    // data.rs
    ("never", "jamais"),
    ("now", "à l'instant"),
    ("{n} min ago", "il y a {n} min"),
    ("{n} h ago", "il y a {n} h"),
    ("{n} d ago", "il y a {n} j"),
    (
        "a credential that had already been swapped was presented",
        "un identifiant déjà remplacé a été présenté",
    ),
    (
        "two connections at the same time",
        "deux connexions en même temps",
    ),
    (
        "the credential could not be read",
        "l'identifiant n'a pas pu être lu",
    ),
    (
        "the last use was not recognized on the reconnect",
        "la dernière utilisation n'a pas été reconnue lors de la reconnexion",
    ),
    (
        "the identity was copied before the approval",
        "l'identité a été copiée avant l'approbation",
    ),
    (
        "the hub no longer trusts the credential",
        "le hub ne fait plus confiance à l'identifiant",
    ),
    ("Waiting for your code", "En attente de votre code"),
    (
        "Approved, waiting for the connector",
        "Approuvé, en attente du connecteur",
    ),
    ("Waiting for the Hermes", "En attente du Hermes"),
    ("Connected", "Connecté"),
    ("Disconnected", "Déconnecté"),
    ("In quarantine", "En quarantaine"),
    ("Suspended", "Suspendu"),
    ("No device", "Aucun appareil"),
    ("a Hermes", "un Hermes"),
    ("Invite", "Invitation"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "un autre appareil a tenté d'utiliser l'invitation de {hermes}. Le bloc s'est retrouvé ailleurs. La première demande reste valable.",
    ),
    ("Identity copied", "Identité copiée"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "l'identité de {hermes} a été copiée avant l'approbation. La demande a été refusée.",
    ),
    ("Hermes in quarantine", "Hermes en quarantaine"),
    ("{hermes}. Reason: {reason}.", "{hermes}. Motif : {reason}."),
    ("Suspected copy", "Copie suspectée"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "la dernière utilisation de {hermes} n'a pas été reconnue lors de la reconnexion.",
    ),
    ("Connection taken", "Connexion prise"),
    (
        "the connection of {hermes} was taken by another one.",
        "la connexion de {hermes} a été prise par un autre.",
    ),
    ("Closing", "Fermeture"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} a demandé sa propre fermeture.",
    ),
    ("Reuse", "Réutilisation"),
    (
        "a copy of {hermes} was used before the closing.",
        "une copie de {hermes} a été utilisée avant la fermeture.",
    ),
    ("Unconfirmed swaps", "Remplacements non confirmés"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} a reçu des identifiants et ne les a pas confirmés, trois fois en 24 heures. Il peut s'agir d'un mauvais réseau, ou de quelqu'un qui essaie une copie.",
    ),
    ("Audit mark", "Marque d'audit"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "un événement de sécurité a été écrit et la marque hors de la base de données n'a pas suivi.",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "la version du moteur ne fait pas partie de celles que le hub a vérifiées.",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "une session du moteur avait une autre configuration, et le hub a appliqué la sienne.",
    ),
    ("Number swapped", "Numéro échangé"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "le téléphone qui s'est appairé appartient à un numéro différent de celui indiqué. Les accès restent arrêtés jusqu'à votre décision.",
    ),
    ("Address: {address}.", "Adresse : {address}."),
    ("an unknown date", "une date inconnue"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "Le hub est revenu d'une sauvegarde datant de {date}. Ce qui a été approuvé, révoqué, reconnecté ou modifié dans les accès après cela est perdu. Reconnectez chaque Hermes et vérifiez les accès.",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "Le hub est revenu d'une sauvegarde. {name} est suspendu et ne revient qu'avec une nouvelle invitation : reconnectez-le ou révoquez-le. Il n'existe pas de « réactiver ».",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "Le hub ne fait plus confiance à l'identifiant de {name}. Motif : {reason}. Son accès est arrêté. Issues possibles : reconnecter ou révoquer. Il n'existe pas de « libérer ».",
    ),
    ("Not paired", "Non appairé"),
    ("Pairing", "Appairage en cours"),
    ("Reconnecting", "Reconnexion en cours"),
    ("Failed", "Échec"),
    ("Removed", "Supprimé"),
    ("version {version}, checked", "version {version}, vérifiée"),
    (
        "version {version}, not checked by the hub",
        "version {version}, non vérifiée par le hub",
    ),
    ("version not read yet", "version pas encore lue"),
    ("last answer {when}", "dernière réponse {when}"),
    ("has not answered yet", "n'a pas encore répondu"),
    ("all", "toutes"),
    ("all · own rules: {n}", "toutes · règles propres : {n}"),
    ("chosen chats: {n}", "conversations choisies : {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias} : Hermes réveillés dans les mêmes conversations : {n}.",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias} : Hermes qui envoient dans les mêmes conversations : {n}.",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} est marqué comme numéro de commande et des Hermes sont autorisés à y envoyer.",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "Il existe déjà un Hermes portant ce nom. Pour lui lier un autre appareil, utilisez le bouton de sa ligne.",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "Ce nom appartenait à un Hermes qui a été supprimé, et un nom n'est jamais réutilisé. Choisissez-en un autre.",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "Avant le code, indiquez si vous reconnaissez la dernière utilisation de l'appareil actuel. Rien n'a été compté.",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "L'appareil actuel a été utilisé pendant que vous répondiez. Consultez sa dernière utilisation maintenant et répondez de nouveau. Rien n'a été approuvé.",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "Le code comporte 6 chiffres, comme 123-456. Rien n'a été compté.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "La base de données est revenue d'une sauvegarde plus ancienne et le hub ne l'a pas encore traitée. Rien n'a été écrit. Redémarrez le hub : il active le mode restauré, puis chaque Hermes est reconnecté avec une nouvelle invitation.",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "Une erreur s'est produite dans le hub et rien n'a été fait. Consultez le journal du hub.",
    ),
    (
        "The name has from 1 to 32 characters.",
        "Le nom comporte de 1 à 32 caractères.",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "Le nom commence par une lettre minuscule ou un chiffre.",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "Utilisez uniquement des lettres minuscules, des chiffres, le trait d'union et le tiret bas.",
    ),
    (
        "That name is reserved. Choose another.",
        "Ce nom est réservé. Choisissez-en un autre.",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "Le code ne correspond pas. Tentative {attempts} sur {max}. Saisissez le dernier code que le Hermes a répondu.",
    ),
    // mod.rs
    (
        "Invalid or expired code. Issue another one on the hub's server.",
        "Code invalide ou expiré. Émettez-en un autre sur le serveur du hub.",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Trop de tentatives pour le moment. Attendez une minute et réessayez.",
    ),
];
