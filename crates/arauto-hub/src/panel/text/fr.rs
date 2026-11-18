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
    ("Waiting for the device…", "En attente de l'appareil…"),
    ("Sign in with a passkey", "Entrer avec une clé d'accès"),
    ("Installation code", "Code d'installation"),
    ("Continue", "Continuer"),
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
    ("Connector", "Connecteur"),
    ("Seen", "Vu"),
    ("Actions", "Actions"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Aucun Hermes pour l'instant. Connectez le premier avec le bouton « Connecter un Hermes ».",
    ),
    ("yes", "oui"),
    ("no", "non"),
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
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "Ce code n'a pas fonctionné. Utilisez le plus récent : celui du dernier bloc du journal, ou un code émis sur le serveur du hub.",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "Le hub n'a pas accepté cette clé d'accès. Réessayez.",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "Votre session du panneau a pris fin. Saisissez le code le plus récent pour reprendre là où vous en étiez.",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "Ce navigateur ne peut pas utiliser de clé d'accès ici. Ouvrez le panneau dans un navigateur à jour, directement à son adresse (et non dans une autre application).",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "La clé d'accès n'a pas été créée : la demande a été annulée ou a pris trop de temps. Réessayez simplement.",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "Aucune clé d'accès n'a été utilisée. Si elle ne se trouve pas sur cet appareil, utilisez l'appareil ou le gestionnaire de mots de passe où vous l'avez créée, ou ouvrez « Vous avez perdu votre clé d'accès ? ».",
    ),
    ("Passkey", "Clé d'accès"),
    ("Check", "Vérifier"),
    (
        "Welcome to your Arauto hub",
        "Bienvenue sur votre hub Arauto",
    ),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "Configurons votre premier accès. Ce sont trois étapes rapides, et vous ne le faites qu'une seule fois.",
    ),
    (
        "Step 1 · Confirm it is you",
        "Étape 1 · Confirmez que c'est bien vous",
    ),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "Au démarrage, le hub a écrit un code d'installation dans son propre journal. Seule la personne qui s'occupe du serveur voit ce journal : c'est ainsi que le hub sait que c'est vous.",
    ),
    ("Where to find the code:", "Où trouver le code :"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "Dans Easypanel, ouvrez le service du hub et allez dans Logs. (Avec Docker : {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "Cherchez le bloc ARAUTO FIRST ACCESS : le plus récent est le dernier, tout en bas du journal. Copiez son code.",
    ),
    ("Paste the code here:", "Collez le code ici :"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "Le code n'expire pas et ne fonctionne qu'une seule fois. Si le hub redémarre, il en écrit un nouveau : utilisez toujours le plus récent.",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "Vous n'arrivez pas à ouvrir le journal ? Sur le serveur du hub, exécutez {command}. Attention : ce nouveau code annule celui du journal.",
    ),
    ("Good to have you back", "Ravi de vous revoir"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "Entrez avec votre clé d'accès : l'empreinte, le visage ou le code PIN de cet appareil, ou votre gestionnaire de mots de passe.",
    ),
    ("Lost your passkey?", "Vous avez perdu votre clé d'accès ?"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "Pas de problème. Sur le serveur du hub, exécutez {command}. La commande affiche un code valable 10 minutes : saisissez-le ici et créez une nouvelle clé d'accès.",
    ),
    (
        "Code accepted! Now, create your passkey",
        "Code accepté ! Créez maintenant votre clé d'accès",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "C'est avec elle que vous entrerez dans le panneau désormais, sans mot de passe à retenir.",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "La clé d'accès reste sur cet appareil ou dans votre gestionnaire de mots de passe, et s'ouvre avec votre empreinte, votre visage ou le code PIN de l'appareil. Le hub ne garde que ce qui vérifie la clé : il n'y a aucun mot de passe qui puisse fuiter.",
    ),
    (
        "It will work at this address:",
        "Elle fonctionnera à cette adresse :",
    ),
    ("Create my passkey", "Créer ma clé d'accès"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "Si vous utilisez un gestionnaire de mots de passe, la clé d'accès peut passer sur vos autres appareils.",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "Vous avez quitté cette page par erreur ? Saisissez à nouveau le même code : il n'est consommé qu'au moment où la clé d'accès est créée.",
    ),
    ("Check the installation", "Vérifier l'installation"),
    // guide.rs
    ("Name", "Nom"),
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
    ("How it works:", "Comment ça marche :"),
    (
        "You give the Hermes a name here.",
        "Ici, vous donnez un nom au Hermes.",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Le Hermes télécharge le connecteur depuis ce hub et répond avec un code de vérification à 6 chiffres.",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "Vous saisissez le code ici et approuvez. C'est fait : le Hermes apparaît comme connecté dès que son connecteur le confirme.",
    ),
    (
        "Let's connect your first Hermes",
        "Connectons votre premier Hermes",
    ),
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
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "le hub avertit quand un message arrive (exige de lire). Le désactiver le désactive aussi dans les chats de cet accès qui ont leur propre règle.",
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
    (
        "the engine refused the key of the hub. Restart the engine: it reads its key only when it starts.",
        "le moteur a refusé la clé du hub. Redémarrez le moteur : il ne lit sa clé qu'au démarrage.",
    ),
    (
        "the engine refused the key of the hub. Set its key again with {command}.",
        "le moteur a refusé la clé du hub. Redéfinissez sa clé avec {command}.",
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
        "Too many attempts right now. Wait a minute and try again.",
        "Trop de tentatives pour le moment. Attendez une minute et réessayez.",
    ),
    // check.rs
    ("ok", "ok"),
    ("check", "à vérifier"),
    ("attention", "attention"),
    ("later", "plus tard"),
    ("failed", "échec"),
    ("Panel address", "Adresse du panneau"),
    ("Connectors address", "Adresse des connecteurs"),
    ("Your address", "Votre adresse"),
    ("Database password", "Mot de passe de la base de données"),
    ("Data folder", "Dossier des données"),
    ("WhatsApp engine", "Moteur WhatsApp"),
    (
        "Your passkey works here.",
        "Votre clé d'accès fonctionne ici.",
    ),
    (
        "Open it: it should say it is the address of the connectors.",
        "Ouvrez-la : elle doit indiquer qu'il s'agit de l'adresse des connecteurs.",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "Le hub vous voit à {address}. Est-ce la vôtre ? Sinon, le proxy ne transmet pas la vraie adresse.",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Une connexion venue de {address}, qui ne figure pas dans ARAUTO_TRUSTED_PROXIES, a apporté une adresse transmise. Ajoutez l'adresse de cette connexion à ARAUTO_TRUSTED_PROXIES et déployez de nouveau.",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Le hub vous voit à {address}, une adresse de réseau interne : le proxy n'est pas encore considéré comme fiable. Ajoutez-la à ARAUTO_TRUSTED_PROXIES et déployez de nouveau.",
    ),
    (
        "Replaced by one nobody saw.",
        "Remplacé par un mot de passe que personne n'a vu.",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "Non remplacé par le hub : cette installation définit elle-même le mot de passe de la base de données.",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "Le remplacement a échoué. Consultez le journal du hub, puis exécutez {command}.",
    ),
    ("Ready for your Hermes.", "Prêt pour vos Hermes."),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "Le dossier des téléchargements est vide : un Hermes ne peut pas être connecté depuis le panneau. Reconstruisez l'image du hub.",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "Il contient la clé maîtresse et le mot de passe de la base de données. Sauvegardez-le : sans lui, le hub ne peut pas ouvrir ce qu'il a conservé chiffré.",
    ),
    ("Configured.", "Configuré."),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "Un Hermes peut être connecté sans lui ; les numéros WhatsApp en ont besoin.",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "Le hub a vérifié certains points tout seul. Rien ici ne modifie quoi que ce soit : c'est seulement pour que vous sachiez comment cela s'est passé.",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "Étape 3 · Vérifions l'installation",
    ),
    (
        "All set: your passkey was created!",
        "C'est fait : votre clé d'accès a été créée !",
    ),
    (
        "The same by command: {command}",
        "La même chose par commande : {command}",
    ),
    ("Connect my first Hermes", "Connecter mon premier Hermes"),
    ("Do it later", "Le faire plus tard"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "Je vous le demande moi-même : connectez ce Hermes à notre hub Arauto sous le nom « {name} ».",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "Lisez {page} et suivez ses instructions. Si quelque chose n'est pas clair, demandez-moi.",
    ),
    ("Hub:", "Hub :"),
    ("Hub key:", "Clé du hub :"),
    (
        "Connector sha256 ({system}):",
        "sha256 du connecteur ({system}) :",
    ),
    (
        "Invite (one use, until {deadline} UTC):",
        "Invitation (usage unique, jusqu'au {deadline} UTC) :",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "Quand c'est fait, envoyez-moi le code de vérification et les lignes que demande la page.",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "Envoyez-le comme un nouveau message, et non en réponse à l'un de ses avis. S'il vous demande de confirmer, répondez avec vos propres mots.",
    ),
    (
        "Show the command instead",
        "Afficher la commande à la place",
    ),
    (
        "Send one or the other, not both: they carry the same invite.",
        "Envoyez l'un ou l'autre, pas les deux : ils contiennent la même invitation.",
    ),
    (
        "Click the message to copy it.",
        "Cliquez sur le message pour le copier.",
    ),
    ("Message", "Message"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "Le hub émet une invitation et rédige avec elle un message, que vous envoyez dans le canal de commande de ce Hermes, et nulle part ailleurs.",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "Lettres minuscules, chiffres, trait d'union et tiret bas, jusqu'à 32 caractères, commençant par une lettre ou un chiffre. Le nom apparaît dans le message et ne change plus ensuite.",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Le message demande au Hermes de télécharger le connecteur depuis ce hub, et le dossier des téléchargements est vide. Rien n'a été émis. Émettez l'invitation de {name} avec la commande {command}, sur le serveur du hub.",
    ),
    ("Org chart", "Organigramme"),
    ("Who talks to [whom]", "Qui parle à [qui] ?"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "Faites glisser un Hermes pour changer son niveau. Tirez le point violet d'une carte jusqu'à une autre carte pour créer un lien. Cliquez sur une flèche ou sur une carte pour la régler.",
    ),
    ("View", "Affichage"),
    ("Map", "Carte"),
    ("List", "Liste"),
    ("+ Level", "+ Niveau"),
    ("Undo", "Défaire"),
    ("Publish", "Publier"),
    ("Draft: 1 change", "Brouillon : 1 modification"),
    ("Draft: {n} changes", "Brouillon : {n} modifications"),
    (
        "From a higher level to a lower one: instruction",
        "D'un niveau supérieur à un niveau inférieur : instruction",
    ),
    (
        "From a lower level to a higher one: request",
        "D'un niveau inférieur à un niveau supérieur : demande",
    ),
    ("Same level: note", "Même niveau : note"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "Les niveaux et les rôles ne s'affichent qu'ici : aucun Hermes n'en est informé. Une note est une donnée, sauf sur une flèche d'instruction avec la marque de suivi : l'avis dit alors au Hermes du bas de suivre ces nouveaux messages de celui du haut, pendant cette exécution, sous les instructions de son opérateur. Seul un lien permet à un Hermes d'envoyer une note à un autre.",
    ),
    ("instruction", "instruction"),
    ("request", "demande"),
    ("note", "note"),
    ("Name of level {n}", "Nom du niveau {n}"),
    ("Remove empty level", "Supprimer le niveau vide"),
    ("Move up", "Monter"),
    ("Move down", "Descendre"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "Un nom de niveau comporte de 1 à 32 lettres minuscules, chiffres, tirets ou tirets bas, commence par une lettre ou un chiffre, et n'est pas un mot réservé.",
    ),
    (
        "Another level already has this name.",
        "Un autre niveau porte déjà ce nom.",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "L'organigramme comporte de 1 à {max} niveaux.",
    ),
    (
        "Drag to another Hermes to create a link",
        "Faites glisser jusqu'à un autre Hermes pour créer un lien",
    ),
    ("{alias} · answers", "{alias} · répond"),
    ("{alias} · reads only", "{alias} · lecture seule"),
    ("no number", "aucun numéro"),
    ("not placed yet", "pas encore placé"),
    (
        "{name}, level {level}, {state}",
        "{name}, niveau {level}, {state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "Pas encore placés : {names}. Ils apparaissent au niveau le plus bas ; publiez pour qu'ils y restent.",
    ),
    ("Kind: {kind}", "Type : {kind}"),
    ("Wakes {name}", "Réveille {name}"),
    ("Create the way back", "Créer le retour"),
    ("Remove the way back", "Supprimer le retour"),
    ("Remove", "Supprimer"),
    ("This link already exists.", "Ce lien existe déjà."),
    ("Role of this Hermes", "Rôle de ce Hermes"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "Pour votre propre lecture : dans cette version, aucun Hermes ne le voit. Jusqu'à 280 caractères, sur une seule ligne.",
    ),
    ("Level", "Niveau"),
    ("Move left", "Déplacer à gauche"),
    ("Move right", "Déplacer à droite"),
    ("Change them in Accesses", "Modifiez-les dans Accès"),
    ("No access to any number.", "Aucun accès à aucun numéro."),
    ("Link to", "Lier à"),
    ("Add link", "Ajouter le lien"),
    ("Links of this Hermes", "Liens de ce Hermes"),
    ("Levels", "Niveaux"),
    ("Links", "Liens"),
    ("Role", "Rôle"),
    ("From", "De"),
    ("To", "À"),
    ("Kind", "Type"),
    ("Wakes", "Réveille"),
    ("nobody", "personne"),
    ("No link.", "Aucun lien."),
    ("Publish these changes?", "Publier ces modifications ?"),
    (
        "Links are security rules. They only apply after you publish.",
        "Les liens sont des règles de sécurité. Ils ne prennent effet qu'après publication.",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "Un lien laisse une demande traverser les numéros : ce qui arrive à un Hermes par un numéro peut devenir une note pour l'autre, qui agit avec ses propres numéros.",
    ),
    ("Back", "Retour"),
    (
        "New link: {from} → {to} ({kind})",
        "Nouveau lien : {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "Nouveau lien : {from} → {to} ({kind}), réveille {to}",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "Lien {from} → {to} : réveille désormais {to}",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "Lien {from} → {to} : ne réveille plus {to}",
    ),
    (
        "Link removed: {from} → {to}",
        "Lien supprimé : {from} → {to}",
    ),
    (
        "{name} changes level: {before} → {after}",
        "{name} change de niveau : {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} est placé au niveau {after}",
    ),
    ("Role of {name} changed", "Rôle de {name} modifié"),
    (
        "Places on the map changed: {n}",
        "Positions sur la carte modifiées : {n}",
    ),
    ("New level: {name}", "Nouveau niveau : {name}"),
    (
        "Level renamed: {before} → {after}",
        "Niveau renommé : {before} → {after}",
    ),
    ("Level removed: {name}", "Niveau supprimé : {name}"),
    (
        "Order of the levels: {names}",
        "Ordre des niveaux : {names}",
    ),
    ("Published.", "Publié."),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "Quelqu'un a modifié l'organigramme pendant que vous éditiez : une autre publication, un lien changé par la commande, ou un Hermes ajouté ou supprimé. Rien n'a été publié. Votre brouillon est conservé : vérifiez les modifications et publiez de nouveau.",
    ),
    ("Nothing was published.", "Rien n'a été publié."),
    (
        "Two levels have the same name.",
        "Deux niveaux portent le même nom.",
    ),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "Un rôle comporte au plus 280 caractères, sur une seule ligne, sans caractères de contrôle ou de direction.",
    ),
    (
        "A place on the map is out of range.",
        "Une position sur la carte est hors limites.",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "Le brouillon cite un Hermes ou un niveau qui n'existe pas, ou oublie un Hermes.",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "Un lien relie deux Hermes différents, une fois dans chaque sens.",
    ),
    (
        "The draft could not be read.",
        "Le brouillon n'a pas pu être lu.",
    ),
    (
        "The draft is too large to publish.",
        "Le brouillon est trop volumineux pour être publié.",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "Une erreur s'est produite dans le hub. Rien n'a été publié. Consultez le journal du hub.",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "Modifier l'organigramme nécessite le script du panneau. Sans lui, voici l'organigramme tel qu'il est publié.",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "Aucun Hermes pour l'instant. Connectez le premier dans l'onglet Hermes.",
    ),
    ("Connect WhatsApp (WAHA)", "Connecter WhatsApp (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "Connecter WhatsApp (natif · en test)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "Aucun numéro pour l'instant. Ajoutez le premier avec le bouton « Connecter WhatsApp (WAHA) ».",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "Dans cette version, le panneau connecte et révoque les Hermes, appaire et gère les numéros WhatsApp sur WAHA, modifie les accès de chaque numéro et édite l'organigramme avec ses liens. Le choix des conversations se fait encore avec la commande {command}, sur le serveur du hub.",
    ),
    ("Pair", "Appairer"),
    ("Pair again", "Appairer de nouveau"),
    ("Continue pairing", "Continuer l'appairage"),
    ("See the pairing", "Voir l'appairage"),
    ("Disconnect", "Déconnecter"),
    ("Restart", "Redémarrer"),
    ("Decide", "Décider"),
    ("Number {alias}: {state}.", "Numéro {alias} : {state}."),
    (
        "Number {alias} was removed.",
        "Le numéro {alias} a été retiré.",
    ),
    ("Telephone given", "Téléphone indiqué"),
    ("Telephone that paired", "Téléphone qui a appairé"),
    ("Undo…", "Défaire…"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "Le hub a modifié le numéro {alias}, mais WAHA n'a pas répondu : sa session là-bas peut être restée telle qu'elle était. Vérifiez WAHA avec {command} sur le serveur du hub.",
    ),
    ("Method", "Méthode"),
    ("Phone", "Téléphone"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "Le panneau et les Hermes appellent ce numéro ainsi : lettres minuscules, chiffres, trait d'union et tiret bas, jusqu'à 32. Un alias n'est jamais réutilisé.",
    ),
    (
        "Number with the country code",
        "Numéro avec l'indicatif du pays",
    ),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "Comme +55 11 9XXXX-XXXX. Chiffres, espaces, traits d'union et parenthèses seulement.",
    ),
    ("Before the first number", "Avant le premier numéro"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "Le moteur est un client WhatsApp non officiel. WhatsApp peut bloquer le numéro. Utilisez des numéros que vous pouvez vous permettre de perdre.",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "Je comprends et j'accepte. Le hub enregistre cet accord dans l'audit.",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "Le numéro est appairé par WAHA, le moteur défini sur ce hub.",
    ),
    ("Add and pair", "Ajouter et appairer"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA n'a pas encore répondu au hub. Vous pouvez ajouter le numéro ; l'appairer demande que WAHA réponde.",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "Il n'y a pas encore de moteur WhatsApp sur ce hub. Définissez WAHA sur le serveur du hub avec {command}, puis sa clé avec {key_command}. La clé n'est jamais saisie dans le panneau.",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "Le moteur n'a pas encore de clé. Définissez-la sur le serveur du hub avec {command}. La clé n'est jamais saisie dans le panneau.",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "L'alias comporte de 1 à 32 caractères.",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "L'alias commence par une lettre minuscule ou un chiffre.",
    ),
    (
        "That alias is reserved. Choose another.",
        "Cet alias est réservé. Choisissez-en un autre.",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "Cet alias est déjà pris, maintenant ou par un numéro qui a été supprimé, et un alias n'est jamais réutilisé. Choisissez-en un autre.",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "Saisissez le numéro avec l'indicatif du pays : de 8 à 15 chiffres, comme +55 11 9XXXX-XXXX.",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "Ce numéro est déjà un WhatsApp de ce hub.",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "Lisez la phrase sur le risque et cochez que vous l'acceptez. Rien n'a été ajouté.",
    ),
    ("Pair {alias}", "Appairer {alias}"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "Ayez le téléphone de {phone} à portée de main, avec WhatsApp ouvert. L'appairage dure jusqu'à {minutes} minutes.",
    ),
    (
        "How will the phone pair?",
        "Comment le téléphone va-t-il s'appairer ?",
    ),
    ("QR code", "Code QR"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "scannez-le avec la caméra du téléphone, depuis WhatsApp",
    ),
    ("Pairing code", "Code d'appairage"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "saisissez un code sur le téléphone, si celui-ci ne peut pas scanner cet écran",
    ),
    ("Start pairing", "Commencer l'appairage"),
    (
        "{alias} is in the list now. Pair it now, or later from its line.",
        "{alias} est maintenant dans la liste. Appairez-le maintenant, ou plus tard depuis sa ligne.",
    ),
    ("Pair later", "Appairer plus tard"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA n'a pas encore créé la session de ce numéro. Commencer l'appairage réessaie.",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "Choisissez code QR ou code d'appairage. Rien n'a été lancé.",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "{n} numéros sont en cours d'appairage, le maximum à la fois. Attendez qu'un se termine, ou annulez-en un.",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "Ce n'est pas possible tant que le numéro est à l'état « {state} ». Rien n'a été fait.",
    ),
    ("Starting the pairing…", "Démarrage de l'appairage…"),
    (
        "QR code to scan with the phone",
        "Code QR à scanner avec le téléphone",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "Le QR change régulièrement, à quelques secondes d'intervalle : cette fenêtre affiche le QR en cours.",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "Le code fonctionne pendant que cette fenêtre compte à rebours : il reste {time}. WhatsApp peut y mettre fin plus tôt.",
    ),
    (
        "Time left to scan: {time}.",
        "Temps restant pour scanner : {time}.",
    ),
    (
        "This window refreshes by itself.",
        "Cette fenêtre s'actualise toute seule.",
    ),
    ("Refresh now", "Actualiser maintenant"),
    ("Cancel the pairing", "Annuler l'appairage"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "Le QR est trop long pour être affiché ici. Annulez, et appairez avec un code d'appairage.",
    ),
    ("Paired: {phone}.", "Appairé : {phone}."),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "Le numéro {alias} est connecté. Donnez accès à un Hermes dans l'onglet Accès.",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "Rien n'a été scanné ni saisi à temps. Le numéro n'est pas appairé ; appairez-le de nouveau avec le téléphone à portée de main.",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA a signalé un échec de la session. Le numéro n'est pas appairé. Réessayez ; si cela échoue encore, vérifiez WAHA.",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "L'appairage a été annulé. Le numéro n'est pas appairé.",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "Ce numéro est en cours d'appairage dans une autre fenêtre. Seule cette autre fenêtre affiche le QR ou le code.",
    ),
    ("Cancel that pairing", "Annuler cet appairage"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "Cet appairage ne tourne plus sur le hub : il s'est terminé, ou le hub a redémarré. Le numéro est à l'état « {state} ».",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "Le hub n'a pas pu joindre WAHA. Vérifiez qu'il tourne et qu'il est sur le réseau du hub.",
    ),
    (
        "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
        "WAHA a refusé la clé que possède le hub. Si l'installation a apporté le moteur, redémarrez le moteur : il ne lit sa clé qu'au démarrage. Si vous avez configuré le moteur vous-même, définissez la bonne clé sur le serveur du hub avec {command}.",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA a répondu par une erreur, ou hors délai. Réessayez dans un moment ; {command} sur le serveur du hub en dit plus.",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp limite ce numéro. Réessayez plus tard.",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA a refusé la demande. Consultez le journal du hub.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "Une erreur s'est produite dans le hub. Consultez le journal du hub et vérifiez le numéro dans la liste.",
    ),
    (
        "{alias}: another phone paired",
        "{alias} : un autre téléphone s'est appairé",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "Le téléphone qui s'est appairé appartient à un numéro différent de celui indiqué.",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "Tant que vous ne décidez pas, le numéro est à l'état « Numéro échangé » et ses accès restent arrêtés.",
    ),
    ("Keep this one", "Garder celui-ci"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "Défaire retire WAHA du WhatsApp de ce téléphone (il quitte ses appareils connectés) et efface l'historique que WAHA conserve pour ce numéro.",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "Je comprends que WAHA efface son historique de ce numéro.",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "Cochez que vous comprenez ce que défaire efface. Rien n'a été fait.",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "Le téléphone qui s'est appairé est déjà un autre numéro de ce hub. Défaites cet appairage, ou supprimez d'abord l'autre numéro.",
    ),
    ("Disconnect {alias}", "Déconnecter {alias}"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "Déconnecter arrête la session de ce numéro dans WAHA. Cela ne retire pas WAHA de WhatsApp, et WAHA conserve son historique. Pendant qu'il est déconnecté, aucun Hermes ne lit ni n'envoie par lui. Pour l'utiliser à nouveau, appairez-le de nouveau.",
    ),
    (
        "Type the alias {alias} to confirm.",
        "Saisissez l'alias {alias} pour confirmer.",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "L'alias saisi ne correspond pas. Rien n'a été fait.",
    ),
    ("Remove {alias}", "Supprimer {alias}"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "Supprimer met fin maintenant à tous les accès à ce numéro. Son alias n'est plus jamais utilisé.",
    ),
    (
        "What happens to its session in WAHA",
        "Ce qui arrive à sa session dans WAHA",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "L'arrêter et la garder dans WAHA, avec l'historique que WAHA conserve",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "Retirer WAHA de WhatsApp et la supprimer : WAHA efface l'historique qu'il conserve",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "Arauto n'a pas encore d'archive de messages : l'historique n'est que ce que WAHA conserve.",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "Choisissez ce qui arrive à sa session dans WAHA. Rien n'a été fait.",
    ),
    ("In test", "En test"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Un moteur WhatsApp intégré à Arauto est à l'étude. Il ne fait pas partie de cette version, et cette fenêtre ne fait rien.",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "Pour l'instant, les numéros sont appairés par WAHA, avec le bouton « Connecter WhatsApp (WAHA) ».",
    ),
    ("Open the WAHA window", "Ouvrir la fenêtre WAHA"),
    ("Try again", "Réessayer"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "Scannez avec le téléphone : WhatsApp › Appareils connectés › Connecter un appareil.",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "Sur le téléphone : WhatsApp › Appareils connectés › Connecter un appareil › Connecter avec le numéro de téléphone, puis saisissez ce code :",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "Avec cette marque, une note sur ce lien réveille {name}, dans les limites des avis.",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} suit les instructions de {from}",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "Avec cette marque, l'avis d'une note dit à {to} de faire ce que {from} demande pendant cette exécution, sans attendre son opérateur, sauf si son opérateur l'interdit. Seulement sur une flèche d'instruction, seulement avec la marque de réveil ; {to} a besoin du lien retour pour rendre compte. C'est une phrase adressée à une IA, pas un contrôle.",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "Lien {from} → {to} : {to} suit désormais les instructions de {from}",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "Lien {from} → {to} : {to} ne suit plus les instructions de {from}",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "Un Hermes qui en suit un autre fait ce que celui-ci demande sans attendre son opérateur. Qui trompe le Hermes du haut, par WhatsApp ou par une note, atteint aussi celui du bas ; qui contrôle le hub peut écrire comme n'importe quel Hermes ; et qui détient une session du panneau peut poser ces marques. Un connecteur d'une version plus ancienne continue de traiter ces notes comme des données.",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "Une marque de suivi va seulement d'un niveau plus haut vers un niveau plus bas, sur un lien qui réveille.",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "Qui lui donne des ordres"),
    ("not recorded", "non enregistré"),
    ("its own chat", "son propre chat"),
    ("{n} telephones", "{n} téléphones"),
    ("1 telephone", "1 téléphone"),
    ("Orders", "Ordres"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "Qui donne des ordres à {hermes} sur WhatsApp",
    ),
    (
        "This is the number {alias} in Arauto.",
        "C'est le numéro {alias} dans Arauto.",
    ),
    ("Orders come from", "Les ordres viennent de"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "Son propre WhatsApp, dans la conversation avec lui-même (mode self-chat)",
    ),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "Arauto refuse que tout Hermes, {hermes} compris, écrive dans la conversation où {hermes} reçoit ces ordres. Qui la lit suit les accès.",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "Si rien n'est enregistré ici, Arauto ne bloque rien : un Hermes qui peut écrire dans la conversation où {hermes} reçoit ses ordres pourrait se faire passer pour vous, ou pour celui qui les donne.",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} reçoit déjà des ordres sur ce téléphone.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "Une erreur s'est produite dans le hub. Consultez le journal du hub et vérifiez ce qui est enregistré pour le Hermes.",
    ),
    (
        "A Hermes could give {hermes} orders",
        "Un Hermes pourrait donner des ordres à {hermes}",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "Rien n'est bloqué : tout Hermes qui peut écrire dans la conversation où {hermes} reçoit ses ordres peut se faire passer pour la personne qui les donne.",
    ),
    ("Go back", "Revenir en arrière"),
    ("gives orders to {hermes}", "donne des ordres à {hermes}"),
    (
        "{hermes} takes orders here",
        "{hermes} reçoit ses ordres ici",
    ),
    (
        "Orders on WhatsApp for {alias}",
        "Ordres sur WhatsApp pour {alias}",
    ),
    ("Its WhatsApp", "Son WhatsApp"),
    (
        "The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.",
        "Le téléphone sur lequel tourne le propre WhatsApp de {hermes} : {hermes} y reçoit ses ordres.",
    ),
    ("It has no WhatsApp", "Il n'a pas de WhatsApp"),
    ("A number of Arauto", "Un numéro d'Arauto"),
    ("Choose a number", "Choisissez un numéro"),
    ("Another telephone", "Un autre téléphone"),
    (
        "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
        "Lu seulement si Un autre téléphone est choisi. Commencez par + et l'indicatif du pays, comme +55 11 9XXXX-XXXX.",
    ),
    ("These telephones", "Ces téléphones"),
    ("its WhatsApp", "son WhatsApp"),
    ("Other telephones", "Autres téléphones"),
    (
        "Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.",
        "Téléphones qui ne sont pas des numéros d'Arauto, un par ligne. 16 téléphones au plus en tout.",
    ),
    (
        "{alias} is chosen. Nothing is saved until you save.",
        "{alias} est choisi. Rien n'est enregistré avant que vous enregistriez.",
    ),
    (
        "{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.",
        "{alias} est choisi comme son WhatsApp, à la place de {before}. Rien n'est enregistré avant que vous enregistriez.",
    ),
    (
        "Choose an option for its WhatsApp. Nothing was saved.",
        "Choisissez une option pour son WhatsApp. Rien n'a été enregistré.",
    ),
    (
        "{alias} is not a number of Arauto now. Nothing was saved.",
        "{alias} n'est plus un numéro d'Arauto. Rien n'a été enregistré.",
    ),
    (
        "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
        "Saisissez l'autre téléphone sur lequel tourne son WhatsApp, ou choisissez une autre option. Rien n'a été enregistré.",
    ),
    (
        "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
        "L'autre téléphone de son WhatsApp n'est pas un téléphone : de 8 à 15 chiffres, en commençant par + et l'indicatif du pays. Rien n'a été enregistré.",
    ),
    (
        "Line {n} of Other telephones is not a telephone. Nothing was saved.",
        "La ligne {n} de Autres téléphones n'est pas un téléphone. Rien n'a été enregistré.",
    ),
    (
        "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
        "Sa propre conversation et une liste de téléphones sont deux réponses : videz la liste, ou choisissez Ces téléphones. Rien n'a été enregistré.",
    ),
    (
        "Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.",
        "Choisissez d'abord le WhatsApp de {hermes} : une conversation a deux bouts. Rien n'a été enregistré.",
    ),
    (
        "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
        "Son WhatsApp ne peut pas aussi lui donner des ordres : cette conversation est le mode self-chat. Rien n'a été enregistré.",
    ),
    (
        "At most 16 telephones can give it orders; these are {n}. Nothing was saved.",
        "16 téléphones au plus peuvent lui donner des ordres ; en voici {n}. Rien n'a été enregistré.",
    ),
    (
        "1 typed telephone needs a look. Nothing was saved.",
        "1 téléphone saisi demande un contrôle. Rien n'a été enregistré.",
    ),
    (
        "{n} typed telephones need a look. Nothing was saved.",
        "{n} téléphones saisis demandent un contrôle. Rien n'a été enregistré.",
    ),
    ("Line {n}", "Ligne {n}"),
    (
        "Not a telephone: 8 to 15 digits, with + and the country code first.",
        "Ce n'est pas un téléphone : de 8 à 15 chiffres, en commençant par + et l'indicatif du pays.",
    ),
    (
        "Not a number of Arauto.",
        "Ce n'est pas un numéro d'Arauto.",
    ),
    (
        "The same telephone as line {n}: it counts once.",
        "Le même téléphone que la ligne {n} : il ne compte qu'une fois.",
    ),
    (
        "The same telephone as {alias}, chosen above: it counts once.",
        "Le même téléphone que {alias}, choisi plus haut : il ne compte qu'une fois.",
    ),
    (
        "This is its WhatsApp: it cannot also give it orders.",
        "C'est son WhatsApp : il ne peut pas aussi lui donner des ordres.",
    ),
    (
        "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
        "Saisi sans +, donc les premiers chiffres ont été lus comme l'indicatif du pays. Si ce n'est pas son pays, saisissez d'abord + et l'indicatif du pays. Enregistrer de nouveau le garde tel qu'il a été lu.",
    ),
    (
        "{n} of 16 telephones, with the numbers chosen above.",
        "{n} téléphones sur 16, avec les numéros choisis plus haut.",
    ),
    (
        "These chats stop being protected",
        "Ces conversations ne seront plus protégées",
    ),
    (
        "Between {bridge}, its WhatsApp until now, and:",
        "Entre {bridge}, son WhatsApp jusqu'ici, et :",
    ),
    ("itself (self-chat mode)", "lui-même (mode self-chat)"),
    (
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
        "Arauto laissera de nouveau tout Hermes écrire dans ces conversations, et s'y faire passer pour la personne qui donne des ordres à {hermes}.",
    ),
    ("Remove the protection", "Retirer la protection"),
    ("Save without protection", "Enregistrer sans protection"),
    ("What {alias} is for it", "Ce que {alias} est pour lui"),
    (
        "A telephone that gives it orders",
        "Un téléphone qui lui donne des ordres",
    ),
    (
        "Its WhatsApp, where it takes orders",
        "Son WhatsApp, où il reçoit ses ordres",
    ),
    (
        "takes orders on {alias} now",
        "reçoit ses ordres sur {alias} actuellement",
    ),
    (
        "{alias} gives it orders now",
        "{alias} lui donne des ordres actuellement",
    ),
    (
        "Nothing is saved here. The window of the Hermes opens with {alias} chosen.",
        "Rien n'est enregistré ici. La fenêtre du Hermes s'ouvre avec {alias} choisi.",
    ),
    (
        "Choose a Hermes and what {alias} is for it.",
        "Choisissez un Hermes et ce que {alias} est pour lui.",
    ),
    (
        "{alias} is the WhatsApp of {hermes}: it cannot also give it orders.",
        "{alias} est le WhatsApp de {hermes} : il ne peut pas aussi lui donner des ordres.",
    ),
    ("Actions: {name}", "Actions : {name}"),
    (
        "Issues the first invite for {name}.",
        "Émet la première invitation pour {name}.",
    ),
    (
        "Goes back to the connection of {name} that is in progress.",
        "Revient à la connexion de {name} en cours.",
    ),
    (
        "Issues an invite. When it is used, {name} moves to the new device.",
        "Émet une invitation. Quand elle sera utilisée, {name} passera au nouvel appareil.",
    ),
    (
        "Which chat or telephones may give it orders on WhatsApp.",
        "Quelle conversation ou quels téléphones peuvent lui donner des ordres sur WhatsApp.",
    ),
    (
        "Closes {name} in the hub. Asks first.",
        "Ferme {name} dans le hub. Demande confirmation avant.",
    ),
    (
        "Opens the window to pair {alias} with a QR code or a pairing code.",
        "Ouvre la fenêtre pour associer {alias} avec un code QR ou un code d'association.",
    ),
    (
        "Goes back to the pairing of {alias} that is running.",
        "Revient à l'association de {alias} en cours.",
    ),
    (
        "Shows the pairing of {alias} that another panel session started.",
        "Montre l'association de {alias} qu'une autre session du panneau a lancée.",
    ),
    (
        "Stops {alias} for every Hermes. Asks for the alias.",
        "Arrête {alias} pour tous les Hermes. Demande l'alias.",
    ),
    (
        "Starts the failed session of {alias} again.",
        "Relance la session de {alias} qui a échoué.",
    ),
    (
        "Another phone paired as {alias}: keep it or undo.",
        "Un autre téléphone s'est associé comme {alias} : le garder ou annuler.",
    ),
    (
        "Which Hermes takes orders in the chats of this number.",
        "Quel Hermes reçoit des ordres dans les conversations de ce numéro.",
    ),
    (
        "Takes {alias} out of the hub. Asks for the alias.",
        "Retire {alias} du hub. Demande l'alias.",
    ),
    ("Dismiss", "Écarter"),
    ("Dismiss: {title}", "Écarter : {title}"),
    (
        "The message of this invite cannot be shown again. If it did not reach the Hermes, issue another invite: this one then stops working.",
        "Le message de cette invitation ne peut plus être affiché. S'il n'est pas arrivé au Hermes, émettez une autre invitation : celle-ci cessera alors de fonctionner.",
    ),
    ("Nothing has arrived yet", "Rien n'est encore arrivé"),
    (
        "The Hermes may be waiting for your approval to run the command, in the command channel.",
        "Le Hermes attend peut-être votre approbation pour exécuter la commande, dans le canal de commande.",
    ),
    (
        "The Hermes may not reach this hub: check that its machine reaches the address of the hub.",
        "Le Hermes n'atteint peut-être pas ce hub : vérifiez que sa machine atteint l'adresse du hub.",
    ),
    (
        "The message may have gone to another chat: it only works in the command channel of this Hermes.",
        "Le message est peut-être parti dans un autre chat : il ne fonctionne que dans le canal de commande de ce Hermes.",
    ),
    (
        "Type the name {name} to confirm.",
        "Saisissez le nom {name} pour confirmer.",
    ),
    (
        "The name typed does not match. Nothing was done.",
        "Le nom saisi ne correspond pas. Rien n'a été fait.",
    ),
    ("Hermes {name}: {state}.", "Hermes {name} : {state}."),
    (
        "Saved. Who gives {hermes} orders on WhatsApp: {orders}.",
        "Enregistré. Qui donne des ordres à {hermes} sur WhatsApp : {orders}.",
    ),
    (
        "Outside WhatsApp: another conversation platform, like Telegram",
        "Hors de WhatsApp : une autre plateforme de conversation, comme Telegram",
    ),
    (
        "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead.",
        "Arauto ne refuse alors rien pour {hermes}. Le hub ne peut pas le vérifier : si {hermes} reçoit aussi des ordres sur un WhatsApp, enregistrez plutôt ce WhatsApp.",
    ),
    (
        "Orders from outside WhatsApp take no telephone: choose It has no WhatsApp, and leave These telephones and Other telephones empty. Nothing was saved.",
        "Des ordres venus de hors de WhatsApp n'ont pas de téléphone : choisissez Il n'a pas de WhatsApp, et laissez vides Ces téléphones et Autres téléphones. Rien n'a été enregistré.",
    ),
    ("outside WhatsApp", "hors de WhatsApp"),
    (
        "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it.",
        "Enregistré. {hermes} reçoit ses ordres hors de WhatsApp : Arauto ne refuse rien pour lui.",
    ),
];
