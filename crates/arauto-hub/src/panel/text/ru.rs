//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "Панель"),
    ("Language", "Язык"),
    ("Change", "Сменить"),
    ("R", "Ч"),
    ("S", "О"),
    ("M", "И"),
    ("W", "П"),
    ("Waiting for the device…", "Ожидание устройства…"),
    ("Sign in with a passkey", "Войти с ключом доступа"),
    ("Installation code", "Код установки"),
    ("Continue", "Продолжить"),
    ("Sign out", "Выйти"),
    ("Alerts", "Оповещения"),
    ("Unknown devices", "Неизвестные устройства"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "Устройств, предъявивших действительные учётные данные, которых база данных не объясняет: {n}. Вернуть каждое из них можно только новым приглашением. Посмотреть их: {command}.",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "Ещё оповещений: {n}. Посмотреть все: {command}.",
    ),
    ("Accesses", "Доступы"),
    ("none", "нет"),
    (
        "Your Hermes and their [numbers]",
        "Ваши Hermes и их [номера]",
    ),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "Подключено: {connected} · В ожидании: {waiting} · С оповещением: {stopped}",
    ),
    ("Connect Hermes", "Подключить Hermes"),
    ("State", "Состояние"),
    ("Numbers", "Номера"),
    ("Connector", "Коннектор"),
    ("Seen", "На связи"),
    ("Actions", "Действия"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Пока нет ни одного Hermes. Подключите первый кнопкой «Подключить Hermes».",
    ),
    ("yes", "да"),
    ("no", "нет"),
    ("Connect", "Подключить"),
    ("Reconnect", "Переподключить"),
    ("Revoke", "Отозвать"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "«Номера» — те, до которых Hermes может дотянуться. Коннектор — то, что сообщило о себе устройство: это не проверяется.",
    ),
    ("Engine", "Движок"),
    (
        "There is no engine yet. Set it with {command}.",
        "Движка пока нет. Задайте его командой {command}.",
    ),
    ("Alias", "Псевдоним"),
    ("Number", "Номер"),
    ("Hermes with access", "Hermes с доступом"),
    ("Last message", "Последнее сообщение"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "Для последнего сообщения показывается только время, содержимое — никогда. Панель показывает то, что проходит через хаб: другие связанные устройства того же номера видны только в телефоне.",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "В строках — Hermes, в столбцах — номера. Нажмите на ячейку, чтобы изменить её.",
    ),
    ("Connect a Hermes first.", "Сначала подключите Hermes."),
    ("Add a WhatsApp first.", "Сначала добавьте WhatsApp."),
    ("(accesses kept)", "(доступы сохранены)"),
    ("no access", "нет доступа"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} читать · {s} отправлять · {m} изменять · {w} пробуждаться.",
    ),
    ("Saved: {what}", "Сохранено: {what}"),
    (
        "Warnings that block nothing",
        "Предупреждения, которые ничего не блокируют",
    ),
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "Этот код не подошёл. Используйте самый последний: тот, что в последнем блоке журнала, или выпущенный на сервере хаба.",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "Хаб не принял этот ключ доступа. Попробуйте ещё раз.",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "Сеанс панели завершился. Введите самый последний код, чтобы продолжить с того места, где остановились.",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "Этот браузер здесь не может использовать ключ доступа. Откройте панель в актуальном браузере, напрямую по её адресу (а не внутри другого приложения).",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "Ключ доступа не создан: запрос был отменён или занял слишком много времени. Просто попробуйте ещё раз.",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "Ключ доступа не использовался. Если его нет на этом устройстве, используйте устройство или менеджер паролей, где вы его создали, либо откройте «Потеряли ключ доступа?».",
    ),
    ("Passkey", "Ключ доступа"),
    ("Check", "Проверка"),
    (
        "Welcome to your Arauto hub",
        "Добро пожаловать в ваш хаб Arauto",
    ),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "Настроим первый вход. Это три быстрых шага, и выполнить их нужно только один раз.",
    ),
    (
        "Step 1 · Confirm it is you",
        "Шаг 1 · Подтвердите, что это вы",
    ),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "Когда хаб запустился, он записал код установки в свой журнал. Видеть этот журнал может только тот, кто обслуживает сервер: так хаб понимает, что это вы.",
    ),
    ("Where to find the code:", "Где найти код:"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "В Easypanel откройте сервис хаба и перейдите в Logs. (Через Docker: {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "Найдите блок ARAUTO FIRST ACCESS: самый новый — последний, в самом конце журнала. Скопируйте его код.",
    ),
    ("Paste the code here:", "Вставьте код сюда:"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "У кода нет срока действия, и он работает только один раз. Если хаб перезапустится, он запишет новый: всегда используйте самый последний.",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "Не получается открыть журнал? На сервере хаба выполните {command}. Внимание: этот новый код отменяет код из журнала.",
    ),
    ("Good to have you back", "Рады снова видеть вас"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "Войдите с ключом доступа: отпечатком пальца, лицом или PIN-кодом этого устройства либо через менеджер паролей.",
    ),
    ("Lost your passkey?", "Потеряли ключ доступа?"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "Ничего страшного. На сервере хаба выполните {command}. Команда покажет код, действующий 10 минут: введите его здесь и создайте новый ключ доступа.",
    ),
    (
        "Code accepted! Now, create your passkey",
        "Код принят! Теперь создайте ключ доступа",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "С его помощью вы теперь будете входить в панель, и запоминать пароль не нужно.",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "Ключ доступа хранится на этом устройстве или в менеджере паролей и открывается отпечатком пальца, лицом или PIN-кодом устройства. Хаб хранит только то, чем ключ проверяется: пароля, который мог бы утечь, нет.",
    ),
    (
        "It will work at this address:",
        "Он будет работать по этому адресу:",
    ),
    ("Create my passkey", "Создать мой ключ доступа"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "Если вы пользуетесь менеджером паролей, ключ доступа может появиться и на других ваших устройствах.",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "Случайно покинули эту страницу? Введите тот же код ещё раз: он расходуется только при создании ключа доступа.",
    ),
    ("Check the installation", "Проверить установку"),
    // guide.rs
    ("Name", "Имя"),
    ("Code", "Код"),
    ("Done", "Готово"),
    ("Invite issued", "Приглашение выпущено"),
    (
        "A device used the invite",
        "Устройство использовало приглашение",
    ),
    (
        "Code checked and request approved",
        "Код проверен, запрос одобрен",
    ),
    (
        "The connector confirmed its first credential",
        "Коннектор подтвердил свои первые учётные данные",
    ),
    (
        "A device used the invite, from {address}",
        "Устройство использовало приглашение, адрес: {address}",
    ),
    ("waiting…", "ожидание…"),
    (
        "What name will this Hermes have in the panel?",
        "Какое имя будет у этого Hermes в панели?",
    ),
    ("Name of the Hermes", "Имя Hermes"),
    (
        "Preparing a secure connection",
        "Подготовка защищённого подключения",
    ),
    (
        "The hub drew the invite of {name}.",
        "Хаб сгенерировал приглашение для {name}.",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "Приглашение для {name} · действует ещё {time} · одноразовое",
    ),
    ("Copied", "Скопировано"),
    (
        "Click the block to copy it.",
        "Нажмите на блок, чтобы скопировать его.",
    ),
    (
        "Select the whole text and copy it.",
        "Выделите весь текст и скопируйте его.",
    ),
    (
        "It only appears here, once.",
        "Он показывается только здесь и только один раз.",
    ),
    (
        "The invite issued before was cancelled.",
        "Выпущенное ранее приглашение отменено.",
    ),
    (
        "The request that was in progress was closed.",
        "Незавершённый запрос закрыт.",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "Используемое устройство продолжает работать, пока новое не подтвердит свои первые учётные данные.",
    ),
    (
        "This hub has no connector to serve",
        "У этого хаба нет коннектора для раздачи",
    ),
    ("Waiting for the Hermes {name}", "Ожидание Hermes {name}"),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "Каждый шаг загорается, когда хаб действительно его наблюдает. Если Hermes попросит разрешения выполнить команду, одобряете её вы — в командном канале.",
    ),
    (
        "A device used the invite of {name}",
        "Устройство использовало приглашение для {name}",
    ),
    ("unknown", "неизвестен"),
    ("Seen by the hub", "Что видел хаб"),
    ("address {address}, {when}", "адрес: {address}, {when}"),
    ("Said by the device", "Что сообщило устройство"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "машина «{host}» · система «{os}» · Hermes «{version}» (не проверено)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "Последнее использование текущего устройства: {when}, адрес: {address}. Вы его узнаёте?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "Последнее использование текущего устройства: {when}. Вы его узнаёте?",
    ),
    ("I recognize it", "Узнаю"),
    ("I do not recognize it", "Не узнаю"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Если Hermes был выключен, а использование было, кто-то воспользовался копией.",
    ),
    ("Verification code", "Проверочный код"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Введите код, который Hermes прислал в ответ в командном канале. Панель никогда не показывает этот код.",
    ),
    (
        "The request of {name} was denied",
        "Запрос Hermes {name} отклонён",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "Код введён неверно в пятый раз. Чтобы подключить, выпустите другое приглашение.",
    ),
    (
        "The request of {name} expired",
        "Срок запроса Hermes {name} истёк",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "10 минут на ввод кода истекли. Чтобы подключить, выпустите другое приглашение.",
    ),
    (
        "The identity of {name} was copied",
        "Идентичность Hermes {name} скопирована",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "До одобрения появились два устройства с одной и той же идентичностью, и запрос был остановлен. Чтобы подключить, выпустите другое приглашение.",
    ),
    (
        "{name} has no invite and no request waiting",
        "У Hermes {name} нет ни приглашения, ни ожидающего запроса",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "Срок приглашения истёк, оно было отменено, или запрос уже обработан. Если Hermes не прислал код, не одобряйте; если он ответил AR-102, приглашение использовало другое устройство. Чтобы подключить, выпустите другое приглашение.",
    ),
    ("{name} is connected", "Hermes {name} подключён"),
    ("{name} was approved", "Hermes {name} одобрен"),
    (
        "What it may do on each number is in the Accesses tab.",
        "Что ему разрешено на каждом номере, указано на вкладке «Доступы».",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "Как только коннектор подтвердит, он будет показан как подключённый. Доступы, которые у него уже были, снова действуют для этого устройства.",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "Как только коннектор подтвердит, он будет показан как подключённый. Доступа ни к одному номеру у него пока нет.",
    ),
    (
        "The hub did not do what was asked",
        "Хаб не выполнил запрошенное",
    ),
    ("Issue invite", "Выпустить приглашение"),
    ("Cancel", "Отмена"),
    ("Next step", "Следующий шаг"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "После копирования вставьте его в командный канал Hermes {name}. Больше никуда.",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "Приглашение действует ещё {time}. Этот экран обновляется сам.",
    ),
    ("Approve", "Одобрить"),
    ("Deny", "Отклонить"),
    (
        "Confirm within {time}.",
        "Осталось времени на подтверждение: {time}.",
    ),
    ("Issue another invite", "Выпустить другое приглашение"),
    ("Close", "Закрыть"),
    ("See accesses", "Посмотреть доступы"),
    ("Finish", "Завершить"),
    ("Copied to the clipboard.", "Скопировано в буфер обмена."),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "Текст выделен. Скопируйте его сочетанием Ctrl+C.",
    ),
    ("Reconnect {name}", "Переподключить {name}"),
    ("Connect {name}", "Подключить {name}"),
    ("Steps", "Шаги"),
    ("How it works:", "Как это работает:"),
    (
        "You give the Hermes a name here.",
        "Здесь вы даёте Hermes имя.",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Hermes скачивает коннектор с этого хаба и отвечает проверочным кодом из 6 цифр.",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "Вы вводите код здесь и одобряете. Готово: Hermes отображается как подключённый, как только его коннектор это подтвердит.",
    ),
    (
        "Let's connect your first Hermes",
        "Подключим ваш первый Hermes",
    ),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "Чтобы пробуждаться, нужно право читать. Ничего не сохранено.",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "Чтобы начинать чаты, нужны право отправлять и область «все чаты». Ничего не сохранено.",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "Ожидание перед пробуждением — от 0 до 60 секунд. Ничего не сохранено.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "В хабе произошёл сбой. Посмотрите журнал хаба и проверьте доступ в матрице.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "База данных восстановлена из более старой резервной копии, и хаб это ещё не обработал. Ничего не записано. Перезапустите хаб.",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "У Hermes {hermes} нет доступа к номеру {number}.",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Hermes {hermes} достаёт только до выбранных чатов номера {number}. Чаты со строкой: {lines}.",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Hermes {hermes} на номере {number}, во всех чатах:",
    ),
    ("reads", "читает"),
    ("does not read", "не читает"),
    ("sends", "отправляет"),
    ("does not send", "не отправляет"),
    ("modifies", "изменяет"),
    ("does not modify", "не изменяет"),
    (
        "is woken after {seconds} s of silence",
        "пробуждается после {seconds} с тишины",
    ),
    ("is not woken", "не пробуждается"),
    ("may start chats", "может начинать чаты"),
    (
        "chats with their own rule: {lines}",
        "чаты со своим правилом: {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} на номере {number}",
    ),
    ("Where it applies", "Где действует"),
    (
        "Every chat, including the ones that come later",
        "Все чаты, включая те, что появятся позже",
    ),
    ("Only chosen chats", "Только выбранные чаты"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "При варианте «только выбранные чаты» для Hermes существуют лишь чаты, у которых есть своя строка, и что ему разрешено в каждом из них, определяет эта строка. Строки пока создаются командой {command}. Сегодня: {lines}.",
    ),
    ("What it may do", "Что ему разрешено"),
    ("Read", "Читать"),
    (
        "see chats and messages, search, download files",
        "видеть чаты и сообщения, искать, скачивать файлы",
    ),
    ("Send", "Отправлять"),
    (
        "write, react, mark as read",
        "писать, ставить реакции, отмечать как прочитанное",
    ),
    ("Modify", "Изменять"),
    (
        "edit and delete what already exists",
        "редактировать и удалять то, что уже существует",
    ),
    ("Be woken", "Пробуждаться"),
    (
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "хаб даёт уведомление, когда приходит сообщение (нужно право читать). Если его выключить, он выключается и в чатах этого доступа, у которых есть собственное правило.",
    ),
    ("Wait", "Ожидание"),
    (
        "seconds of silence before waking",
        "секунд тишины перед пробуждением",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "Может начать чат с тем, кто никогда не общался с этим номером (нужны право отправлять и все чаты)",
    ),
    ("Today:", "Сегодня:"),
    ("Save", "Сохранить"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "Действует со следующего вызова, перезапускать ничего не нужно. Если не отмечено ни одного действия, доступ удаляется.",
    ),
    ("Revoke {name}", "Отозвать {name}"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "Отзыв закрывает Hermes в хабе: используемое устройство, незавершённый запрос и выпущенное приглашение. Канал обрывается за несколько секунд.",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "Доступы сохраняются и снова действуют, когда подключается новое устройство. Вернуться можно только по новому приглашению.",
    ),
    // data.rs
    ("never", "никогда"),
    ("now", "сейчас"),
    ("{n} min ago", "{n} мин назад"),
    ("{n} h ago", "{n} ч назад"),
    ("{n} d ago", "{n} дн. назад"),
    (
        "a credential that had already been swapped was presented",
        "предъявлены учётные данные, которые уже были заменены",
    ),
    (
        "two connections at the same time",
        "два подключения одновременно",
    ),
    (
        "the credential could not be read",
        "учётные данные не удалось прочитать",
    ),
    (
        "the last use was not recognized on the reconnect",
        "последнее использование не было узнано при переподключении",
    ),
    (
        "the identity was copied before the approval",
        "идентичность скопирована до одобрения",
    ),
    (
        "the hub no longer trusts the credential",
        "хаб больше не доверяет учётным данным",
    ),
    ("Waiting for your code", "Ожидает ваш код"),
    (
        "Approved, waiting for the connector",
        "Одобрен, ожидает коннектор",
    ),
    ("Waiting for the Hermes", "Ожидает Hermes"),
    ("Connected", "Подключён"),
    ("Disconnected", "Отключён"),
    ("In quarantine", "В карантине"),
    ("Suspended", "Приостановлен"),
    ("No device", "Нет устройства"),
    ("a Hermes", "Hermes с неизвестным именем"),
    ("Invite", "Приглашение"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "другое устройство попыталось использовать приглашение для {hermes}. Блок попал куда-то ещё. Первый запрос остаётся в силе.",
    ),
    ("Identity copied", "Идентичность скопирована"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "идентичность, выданная для {hermes}, скопирована до одобрения. Запрос отклонён.",
    ),
    ("Hermes in quarantine", "Hermes в карантине"),
    (
        "{hermes}. Reason: {reason}.",
        "{hermes}. Причина: {reason}.",
    ),
    ("Suspected copy", "Подозрение на копию"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "при переподключении {hermes} последнее использование не было узнано.",
    ),
    ("Connection taken", "Подключение перехвачено"),
    (
        "the connection of {hermes} was taken by another one.",
        "подключение, которое держал {hermes}, перехвачено другим подключением.",
    ),
    ("Closing", "Закрытие"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} сам запросил своё закрытие.",
    ),
    ("Reuse", "Повторное использование"),
    (
        "a copy of {hermes} was used before the closing.",
        "до закрытия использовалась копия {hermes}.",
    ),
    ("Unconfirmed swaps", "Неподтверждённые замены"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} получил учётные данные и не подтвердил их — трижды за 24 часа. Возможно, плохая сеть, или кто-то пробует копию.",
    ),
    ("Audit mark", "Отметка аудита"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "событие безопасности записано, а отметка вне базы данных за ним не последовала.",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "версия движка не входит в число проверенных хабом.",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "у сеанса движка была другая конфигурация, и хаб применил свою.",
    ),
    (
        "the engine refused the key of the hub. Restart the engine: it reads its key only when it starts.",
        "движок отклонил ключ хаба. Перезапустите движок: он читает свой ключ только при запуске.",
    ),
    (
        "the engine refused the key of the hub. Set its key again with {command}.",
        "движок отклонил ключ хаба. Задайте его ключ заново командой {command}.",
    ),
    ("Number swapped", "Номер подменён"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "привязанный телефон относится к другому номеру, а не к указанному. Доступы остановлены, пока вы не примете решение.",
    ),
    ("Address: {address}.", "Адрес: {address}."),
    ("an unknown date", "неизвестна"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "Хаб восстановлен из резервной копии. Дата копии: {date}. Всё, что после этого было одобрено, отозвано, переподключено или изменено в доступах, потеряно. Переподключите каждый Hermes и проверьте доступы.",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "Хаб восстановлен из резервной копии. Hermes {name} приостановлен и вернётся только по новому приглашению: переподключите или отзовите его. Действия «активировать снова» нет.",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "Хаб больше не доверяет учётным данным Hermes {name}. Причина: {reason}. Его доступ остановлен. Выходы: переподключить или отозвать. Действия «освободить» нет.",
    ),
    ("Not paired", "Не привязан"),
    ("Pairing", "Привязка"),
    ("Reconnecting", "Переподключение"),
    ("Failed", "Сбой"),
    ("Removed", "Удалён"),
    ("version {version}, checked", "версия {version}, проверена"),
    (
        "version {version}, not checked by the hub",
        "версия {version}, не проверена хабом",
    ),
    ("version not read yet", "версия ещё не прочитана"),
    ("last answer {when}", "последний ответ: {when}"),
    ("has not answered yet", "ещё не отвечал"),
    ("all", "все"),
    ("all · own rules: {n}", "все · своих правил: {n}"),
    ("chosen chats: {n}", "выбранные чаты: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: в одних и тех же чатах пробуждаются несколько Hermes, всего {n}.",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: в одних и тех же чатах отправляют несколько Hermes, всего {n}.",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "Hermes с таким именем уже есть. Чтобы связать с ним другое устройство, используйте кнопку в его строке.",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "Это имя принадлежало удалённому Hermes, а имена никогда не используются повторно. Выберите другое.",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "Прежде чем вводить код, ответьте, узнаёте ли вы последнее использование текущего устройства. Попытка не засчитана.",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "Пока вы отвечали, текущее устройство использовалось. Посмотрите его последнее использование и ответьте снова. Ничего не одобрено.",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "Код состоит из 6 цифр, например 123-456. Попытка не засчитана.",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "База данных восстановлена из более старой резервной копии, и хаб это ещё не обработал. Ничего не записано. Перезапустите хаб: он включит режим восстановления, после чего каждый Hermes переподключается по новому приглашению.",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "В хабе произошёл сбой, и ничего не сделано. Посмотрите журнал хаба.",
    ),
    (
        "The name has from 1 to 32 characters.",
        "В имени от 1 до 32 символов.",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "Имя начинается со строчной буквы или цифры.",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "Используйте только строчные буквы, цифры, дефис и подчёркивание.",
    ),
    (
        "That name is reserved. Choose another.",
        "Это имя зарезервировано. Выберите другое.",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "Код не совпадает. Попытка {attempts} из {max}. Введите последний код, который прислал Hermes.",
    ),
    // mod.rs
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Сейчас слишком много попыток. Подождите минуту и попробуйте снова.",
    ),
    // check.rs
    ("ok", "ок"),
    ("check", "проверить"),
    ("attention", "внимание"),
    ("later", "позже"),
    ("failed", "сбой"),
    ("Panel address", "Адрес панели"),
    ("Connectors address", "Адрес коннекторов"),
    ("Your address", "Ваш адрес"),
    ("Database password", "Пароль базы данных"),
    ("Data folder", "Папка данных"),
    ("WhatsApp engine", "Движок WhatsApp"),
    (
        "Your passkey works here.",
        "Ваш ключ доступа работает здесь.",
    ),
    (
        "Open it: it should say it is the address of the connectors.",
        "Откройте его: должно появиться сообщение, что это адрес коннекторов.",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "Хаб видит вас по адресу {address}. Это ваш адрес? Если нет, прокси не передаёт настоящий адрес.",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Соединение с адреса {address}, которого нет в ARAUTO_TRUSTED_PROXIES, передало пересланный адрес. Добавьте адрес этого соединения в ARAUTO_TRUSTED_PROXIES и разверните заново.",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "Хаб видит вас по адресу {address}, это адрес внутренней сети: прокси пока не считается доверенным. Добавьте его в ARAUTO_TRUSTED_PROXIES и разверните заново.",
    ),
    (
        "Replaced by one nobody saw.",
        "Заменён на тот, который никто не видел.",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "Не заменён хабом: эта установка сама задаёт пароль базы данных.",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "Замена не удалась. Посмотрите журнал хаба и выполните {command}.",
    ),
    ("Ready for your Hermes.", "Готов к работе с вашими Hermes."),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "Папка загрузок пуста: подключить Hermes через панель нельзя. Пересоберите образ хаба.",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "Она хранит главный ключ и пароль базы данных. Сделайте резервную копию: без неё хаб не сможет открыть то, что хранил зашифрованным.",
    ),
    ("Configured.", "Настроен."),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "Hermes можно подключить и без него; номера WhatsApp без него работать не будут.",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "Хаб проверил некоторые вещи сам. Здесь ничего не меняется: это просто чтобы вы знали, как всё прошло.",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "Шаг 3 · Проверим установку",
    ),
    (
        "All set: your passkey was created!",
        "Готово: ваш ключ доступа создан!",
    ),
    (
        "The same by command: {command}",
        "То же командой: {command}",
    ),
    ("Connect my first Hermes", "Подключить мой первый Hermes"),
    ("Do it later", "Сделать позже"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "Это моя личная просьба: подключите этот Hermes к нашему хабу Arauto под именем «{name}».",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "Прочитайте {page} и выполняйте то, что там написано. Если что-то непонятно, спросите меня.",
    ),
    ("Hub:", "Хаб:"),
    ("Hub key:", "Ключ хаба:"),
    (
        "Connector sha256 ({system}):",
        "sha256 коннектора ({system}):",
    ),
    (
        "Invite (one use, until {deadline} UTC):",
        "Приглашение (одноразовое, до {deadline} UTC):",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "Когда всё будет готово, пришлите мне проверочный код и строки, которые просит страница.",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "Отправьте этот текст новым сообщением, а не ответом на одно из уведомлений Hermes. Если Hermes попросит подтвердить, ответьте своими словами.",
    ),
    ("Show the command instead", "Показать команду вместо этого"),
    (
        "Send one or the other, not both: they carry the same invite.",
        "Отправьте одно из двух, но не оба: в них одно и то же приглашение.",
    ),
    (
        "Click the message to copy it.",
        "Нажмите на сообщение, чтобы скопировать его.",
    ),
    ("Message", "Сообщение"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "Хаб создаёт приглашение и пишет с ним сообщение, которое вы отправляете в командный канал этого Hermes, и больше никуда.",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "Строчные буквы, цифры, дефис и подчёркивание, до 32 символов, первый символ — буква или цифра. Имя попадает в сообщение и потом не меняется.",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Сообщение велит Hermes скачать коннектор с этого хаба, а папка загрузок пуста. Ничего не выпущено. Выпустите приглашение для {name} командой {command} на сервере хаба.",
    ),
    ("Org chart", "Оргсхема"),
    ("Who talks to [whom]", "Кто говорит с [кем]?"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "Перетащите Hermes, чтобы изменить уровень. Потяните фиолетовую точку карточки на другую карточку, чтобы создать связь. Нажмите на стрелку или на карточку, чтобы настроить.",
    ),
    ("View", "Вид"),
    ("Map", "Карта"),
    ("List", "Список"),
    ("+ Level", "+ Уровень"),
    ("Undo", "Отменить"),
    ("Publish", "Опубликовать"),
    ("Draft: 1 change", "Черновик: 1 изменение"),
    ("Draft: {n} changes", "Черновик, изменений: {n}"),
    (
        "From a higher level to a lower one: instruction",
        "Сверху вниз: инструкция",
    ),
    (
        "From a lower level to a higher one: request",
        "Снизу вверх: запрос",
    ),
    ("Same level: note", "Тот же уровень: заметка"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "Уровни и роли показываются только здесь: ни один Hermes о них не узнаёт. Заметка — это данные, кроме стрелки типа «инструкция» с отметкой следования: тогда уведомление говорит нижнему Hermes выполнять эти новые сообщения верхнего в этом запуске, с приоритетом указаний его оператора. Только связь позволяет одному Hermes отправить заметку другому.",
    ),
    ("instruction", "инструкция"),
    ("request", "запрос"),
    ("note", "заметка"),
    ("Name of level {n}", "Имя уровня {n}"),
    ("Remove empty level", "Удалить пустой уровень"),
    ("Move up", "Вверх"),
    ("Move down", "Вниз"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "Имя уровня — от 1 до 32 строчных букв, цифр, дефисов или подчёркиваний, начинается с буквы или цифры и не является зарезервированным словом.",
    ),
    (
        "Another level already has this name.",
        "Другой уровень уже имеет это имя.",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "В оргсхеме от 1 до {max} уровней.",
    ),
    (
        "Drag to another Hermes to create a link",
        "Перетащите на другой Hermes, чтобы создать связь",
    ),
    ("{alias} · answers", "{alias} · отвечает"),
    ("{alias} · reads only", "{alias} · только чтение"),
    ("no number", "нет номера"),
    ("not placed yet", "ещё не размещён"),
    (
        "{name}, level {level}, {state}",
        "{name}, уровень {level}, {state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "Ещё не размещены: {names}. Они показаны на самом нижнем уровне; опубликуйте, чтобы они там и остались.",
    ),
    ("Kind: {kind}", "Тип: {kind}"),
    ("Wakes {name}", "Пробуждает {name}"),
    ("Create the way back", "Создать обратный путь"),
    ("Remove the way back", "Удалить обратный путь"),
    ("Remove", "Удалить"),
    ("This link already exists.", "Такая связь уже есть."),
    ("Role of this Hermes", "Роль этого Hermes"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "Для вашего личного чтения: в этой версии ни один Hermes этого не видит. До 280 символов, в одной строке.",
    ),
    ("Level", "Уровень"),
    ("Move left", "Влево"),
    ("Move right", "Вправо"),
    ("Change them in Accesses", "Измените на вкладке «Доступы»"),
    (
        "No access to any number.",
        "Нет доступа ни к одному номеру.",
    ),
    ("Link to", "Связать с"),
    ("Add link", "Добавить связь"),
    ("Links of this Hermes", "Связи этого Hermes"),
    ("Levels", "Уровни"),
    ("Links", "Связи"),
    ("Role", "Роль"),
    ("From", "От"),
    ("To", "Кому"),
    ("Kind", "Тип"),
    ("Wakes", "Пробуждает"),
    ("nobody", "никто"),
    ("No link.", "Связей нет."),
    ("Publish these changes?", "Опубликовать эти изменения?"),
    (
        "Links are security rules. They only apply after you publish.",
        "Связи — это правила безопасности. Они начинают действовать только после публикации.",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "Связь позволяет запросу проходить между номерами: то, что приходит одному Hermes по одному номеру, может стать заметкой для другого, который действует по своим номерам.",
    ),
    ("Back", "Назад"),
    (
        "New link: {from} → {to} ({kind})",
        "Новая связь: {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "Новая связь: {from} → {to} ({kind}), пробуждает {to}",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "Связь {from} → {to}: теперь пробуждает {to}",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "Связь {from} → {to}: больше не пробуждает {to}",
    ),
    (
        "Link removed: {from} → {to}",
        "Связь удалена: {from} → {to}",
    ),
    (
        "{name} changes level: {before} → {after}",
        "{name} меняет уровень: {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} размещается на уровне {after}",
    ),
    ("Role of {name} changed", "Роль {name} изменена"),
    (
        "Places on the map changed: {n}",
        "Изменено положений на карте: {n}",
    ),
    ("New level: {name}", "Новый уровень: {name}"),
    (
        "Level renamed: {before} → {after}",
        "Уровень переименован: {before} → {after}",
    ),
    ("Level removed: {name}", "Уровень удалён: {name}"),
    ("Order of the levels: {names}", "Порядок уровней: {names}"),
    ("Published.", "Опубликовано."),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "Кто-то изменил оргсхему, пока вы её редактировали: другая публикация, связь, изменённая командой, или Hermes, добавленный или удалённый. Ничего не опубликовано. Ваш черновик сохранён: проверьте изменения и опубликуйте снова.",
    ),
    ("Nothing was published.", "Ничего не опубликовано."),
    (
        "Two levels have the same name.",
        "У двух уровней одно и то же имя.",
    ),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "Роль — не более 280 символов, в одной строке, без управляющих и направляющих символов.",
    ),
    (
        "A place on the map is out of range.",
        "Положение на карте вне допустимого диапазона.",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "В черновике указан Hermes или уровень, которых нет, либо пропущен Hermes.",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "Связь соединяет два разных Hermes, по одной в каждом направлении.",
    ),
    (
        "The draft could not be read.",
        "Черновик не удалось прочитать.",
    ),
    (
        "The draft is too large to publish.",
        "Черновик слишком большой для публикации.",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "В хабе что-то пошло не так. Ничего не опубликовано. Посмотрите журнал хаба.",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "Для редактирования оргсхемы нужен скрипт панели. Без него это оргсхема в том виде, в каком она опубликована.",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "Пока нет ни одного Hermes. Подключите первый на вкладке Hermes.",
    ),
    ("Connect WhatsApp (WAHA)", "Подключить WhatsApp (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "Подключить WhatsApp (нативный · тестируется)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "Пока нет ни одного номера. Добавьте первый кнопкой «Подключить WhatsApp (WAHA)».",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "В этой версии панель подключает и отзывает Hermes, привязывает и ведёт номера WhatsApp через WAHA, меняет доступы каждого номера и редактирует оргсхему с её связями. Выбор чатов по-прежнему выполняется командой {command} на сервере хаба.",
    ),
    ("Pair", "Привязать"),
    ("Pair again", "Привязать снова"),
    ("Continue pairing", "Продолжить привязку"),
    ("See the pairing", "Посмотреть привязку"),
    ("Disconnect", "Отключить"),
    ("Restart", "Перезапустить"),
    ("Decide", "Решить"),
    ("Number {alias}: {state}.", "Номер {alias}: {state}."),
    ("Number {alias} was removed.", "Номер {alias} удалён."),
    ("Telephone given", "Указанный телефон"),
    ("Telephone that paired", "Телефон, который подключился"),
    ("Undo…", "Отменить…"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "Хаб изменил номер {alias}, но WAHA не ответил: его сеанс там мог остаться прежним. Проверьте WAHA командой {command} на сервере хаба.",
    ),
    ("Method", "Способ"),
    ("Phone", "Телефон"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "Так панель и Hermes называют этот номер: строчные буквы, цифры, дефис и подчёркивание, до 32 символов. Псевдоним никогда не используется повторно.",
    ),
    ("Number with the country code", "Номер с кодом страны"),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "Например, +55 11 9XXXX-XXXX. Только цифры, пробелы, дефисы и скобки.",
    ),
    ("Before the first number", "Перед первым номером"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "Движок — это неофициальный клиент WhatsApp. WhatsApp может заблокировать номер. Используйте номера, потеря которых вам не критична.",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "Я понимаю и принимаю. Хаб записывает это согласие в аудит.",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "Номер привязывается через WAHA, движок, заданный на этом хабе.",
    ),
    ("Add and pair", "Добавить и привязать"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA ещё не ответил хабу. Номер можно добавить; привязать его можно, только когда WAHA ответит.",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "На этом хабе ещё нет движка WhatsApp. Задайте WAHA на сервере хаба командой {command}, затем его ключ командой {key_command}. Ключ никогда не вводится в панели.",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "У движка ещё нет ключа. Задайте его на сервере хаба командой {command}. Ключ никогда не вводится в панели.",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "Псевдоним — от 1 до 32 символов.",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "Псевдоним начинается со строчной буквы или цифры.",
    ),
    (
        "That alias is reserved. Choose another.",
        "Этот псевдоним зарезервирован. Выберите другой.",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "Этот псевдоним занят — сейчас или номером, который был удалён. Псевдоним никогда не используется повторно. Выберите другой.",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "Введите номер с кодом страны: от 8 до 15 цифр, например +55 11 9XXXX-XXXX.",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "Этот номер уже есть среди WhatsApp этого хаба.",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "Прочитайте фразу о риске и отметьте, что вы её принимаете. Ничего не добавлено.",
    ),
    ("Pair {alias}", "Привязать {alias}"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "Держите под рукой телефон {phone} с открытым WhatsApp. Привязка длится до {minutes} мин.",
    ),
    (
        "How will the phone pair?",
        "Как будет привязываться телефон?",
    ),
    ("QR code", "QR-код"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "отсканируйте камерой телефона изнутри WhatsApp",
    ),
    ("Pairing code", "Код привязки"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "введите код на телефоне, если он не может отсканировать этот экран",
    ),
    ("Start pairing", "Начать привязку"),
    (
        "{alias} is in the list now. Pair it now, or later from its line.",
        "{alias} теперь в списке. Привяжите его сейчас или позже из его строки.",
    ),
    ("Pair later", "Привязать позже"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA ещё не создал сеанс этого номера. При запуске привязки попытка повторится.",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "Выберите QR-код или код привязки. Ничего не запущено.",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "Сейчас привязываются номера: {n}, это максимум одновременно. Дождитесь окончания одного или отмените один.",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "Это нельзя сделать, пока номер в состоянии «{state}». Ничего не сделано.",
    ),
    ("Starting the pairing…", "Запуск привязки…"),
    (
        "QR code to scan with the phone",
        "QR-код для сканирования телефоном",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "QR-код меняется каждые несколько секунд: в этом окне показан текущий.",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "Код действует, пока в этом окне идёт отсчёт: осталось {time}. WhatsApp может завершить привязку раньше.",
    ),
    (
        "Time left to scan: {time}.",
        "Осталось времени на сканирование: {time}.",
    ),
    (
        "This window refreshes by itself.",
        "Это окно обновляется само.",
    ),
    ("Refresh now", "Обновить сейчас"),
    ("Cancel the pairing", "Отменить привязку"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "QR-код слишком длинный, чтобы вывести его здесь. Отмените и привяжите по коду привязки.",
    ),
    ("Paired: {phone}.", "Привязан: {phone}."),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "Номер {alias} подключён. Дайте Hermes доступ к нему на вкладке «Доступы».",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "Ни скан, ни ввод кода не были сделаны вовремя. Номер не привязан; привяжите его снова, когда телефон будет под рукой.",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA сообщил о сбое сеанса. Номер не привязан. Попробуйте снова; если сбой повторится, проверьте WAHA.",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "Привязка отменена. Номер не привязан.",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "Этот номер привязывается в другом окне. Только то окно показывает QR-код или код.",
    ),
    ("Cancel that pairing", "Отменить ту привязку"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "Эта привязка больше не выполняется на хабе: она завершилась или хаб перезапустился. Номер в состоянии «{state}».",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "Хаб не смог связаться с WAHA. Проверьте, запущен ли он и находится ли в сети хаба.",
    ),
    (
        "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
        "WAHA отклонил ключ, который есть у хаба. Если движок поставлялся вместе с установкой, перезапустите движок: он читает свой ключ только при запуске. Если вы настроили движок сами, задайте правильный ключ на сервере хаба командой {command}.",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA ответил ошибкой или не уложился в срок. Попробуйте снова через минуту; {command} на сервере хаба расскажет подробнее.",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp ограничивает этот номер. Попробуйте позже.",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA отклонил запрос. Посмотрите журнал хаба.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "В хабе что-то пошло не так. Посмотрите журнал хаба и проверьте номер в списке.",
    ),
    (
        "{alias}: another phone paired",
        "{alias}: привязан другой телефон",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "Привязанный телефон относится к номеру, отличному от указанного.",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "Пока вы не решили, номер в состоянии «Номер подменён», а его доступы остановлены.",
    ),
    ("Keep this one", "Оставить этот"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "Отмена привязки удаляет WAHA из WhatsApp этого телефона (он выходит из связанных устройств) и стирает историю, которую WAHA хранит по этому номеру.",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "Я понимаю, что WAHA стирает свою историю по этому номеру.",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "Отметьте, что вы понимаете, что стирает отмена. Ничего не сделано.",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "Привязанный телефон уже является другим номером этого хаба. Отмените эту привязку или сначала удалите другой номер.",
    ),
    ("Disconnect {alias}", "Отключить {alias}"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "Отключение останавливает сеанс этого номера в WAHA. WAHA не удаляется из WhatsApp, и WAHA сохраняет свою историю. Пока номер отключён, ни один Hermes не читает и не отправляет через него. Чтобы снова использовать, привяжите его заново.",
    ),
    (
        "Type the alias {alias} to confirm.",
        "Введите псевдоним {alias} для подтверждения.",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "Введённый псевдоним не совпадает. Ничего не сделано.",
    ),
    ("Remove {alias}", "Удалить {alias}"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "Удаление сразу прекращает все доступы к этому номеру. Его псевдоним больше никогда не используется.",
    ),
    (
        "What happens to its session in WAHA",
        "Что будет с его сеансом в WAHA",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "Остановить и оставить в WAHA, с историей, которую хранит WAHA",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "Вывести WAHA из WhatsApp и удалить: WAHA стирает историю, которую хранит",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "В Arauto пока нет архива сообщений: история — это только то, что хранит WAHA.",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "Выберите, что будет с его сеансом в WAHA. Ничего не сделано.",
    ),
    ("In test", "Тестируется"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Движок WhatsApp, встроенный в Arauto, сейчас изучается. Он не входит в эту версию, и это окно ничего не делает.",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "Пока что номера привязываются через WAHA, кнопкой «Подключить WhatsApp (WAHA)».",
    ),
    ("Open the WAHA window", "Открыть окно WAHA"),
    ("Try again", "Попробовать снова"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "Отсканируйте телефоном: WhatsApp › Связанные устройства › Привязка устройства.",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "На телефоне: WhatsApp › Связанные устройства › Привязка устройства › Привязать по номеру телефона, и введите этот код:",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "С этой отметкой заметка по этой связи пробуждает {name} в пределах лимитов уведомлений.",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} следует указаниям {from}",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "С этой отметкой уведомление о заметке говорит {to} выполнить в этом запуске то, о чём просит {from}, не дожидаясь своего оператора, если только его оператор не запретит. Только на стрелке типа «инструкция» и только с отметкой пробуждения; {to} нужен обратный путь, чтобы ответить. Это фраза для ИИ, а не средство контроля.",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "Связь {from} → {to}: {to} теперь следует указаниям {from}",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "Связь {from} → {to}: {to} больше не следует указаниям {from}",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "Hermes, который следует за другим, делает то, о чём тот просит, не дожидаясь своего оператора. Кто обманет верхний Hermes через WhatsApp или заметку, достанет и нижний; кто управляет хабом, может писать от имени любого Hermes; а кто владеет сеансом панели, может ставить эти отметки. Коннектор старой версии по-прежнему считает такие заметки данными.",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "Отметка следования идёт только с более высокого уровня на более низкий и только на связи с пробуждением.",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "Кто отдаёт ему приказы"),
    ("not recorded", "не записано"),
    ("its own chat", "свой собственный чат"),
    ("{n} telephones", "телефонов: {n}"),
    ("1 telephone", "1 телефон"),
    ("Orders", "Приказы"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "Кто отдаёт приказы {hermes} в WhatsApp",
    ),
    (
        "This is the number {alias} in Arauto.",
        "Это номер {alias} в Arauto.",
    ),
    ("Orders come from", "Приказы приходят от"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "Его собственный WhatsApp, в чате с самим собой (режим self-chat)",
    ),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "Arauto не позволяет ни одному Hermes, включая {hermes}, писать в чат, где {hermes} принимает эти приказы. Кто может читать, определяют доступы.",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "Если здесь ничего не записано, Arauto ничего не блокирует: Hermes, который может писать в чат, где {hermes} принимает приказы, мог бы выдать себя за вас или за того, кто их отдаёт.",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} уже принимает приказы на этом телефоне.",
    ),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "В хабе произошёл сбой. Посмотрите журнал хаба и проверьте запись Hermes.",
    ),
    (
        "A Hermes could give {hermes} orders",
        "Какой-то Hermes мог бы отдавать приказы {hermes}",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "Ничего не заблокировано: любой Hermes, который может писать в чат, где {hermes} принимает приказы, может выдать себя за того, кто их отдаёт.",
    ),
    ("Go back", "Вернуться"),
    ("gives orders to {hermes}", "отдаёт приказы {hermes}"),
    (
        "{hermes} takes orders here",
        "{hermes} принимает здесь приказы",
    ),
    (
        "Orders on WhatsApp for {alias}",
        "Приказы в WhatsApp для {alias}",
    ),
    ("Its WhatsApp", "Его WhatsApp"),
    (
        "The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.",
        "Телефон, на котором работает собственный WhatsApp {hermes}: {hermes} принимает там приказы.",
    ),
    ("It has no WhatsApp", "У него нет WhatsApp"),
    ("A number of Arauto", "Номер Arauto"),
    ("Choose a number", "Выберите номер"),
    ("Another telephone", "Другой телефон"),
    (
        "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
        "Учитывается, только если выбран вариант Другой телефон. Начните с + и кода страны, например +55 11 9XXXX-XXXX.",
    ),
    ("These telephones", "Эти телефоны"),
    ("its WhatsApp", "его WhatsApp"),
    ("Other telephones", "Другие телефоны"),
    (
        "Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.",
        "Телефоны, которые не являются номерами Arauto, по одному в строке. Всего не более 16 телефонов.",
    ),
    (
        "{alias} is chosen. Nothing is saved until you save.",
        "{alias} выбран. Ничего не сохраняется, пока вы не сохраните.",
    ),
    (
        "{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.",
        "{alias} выбран как его WhatsApp вместо {before}. Ничего не сохраняется, пока вы не сохраните.",
    ),
    (
        "Choose an option for its WhatsApp. Nothing was saved.",
        "Выберите вариант для его WhatsApp. Ничего не было сохранено.",
    ),
    (
        "{alias} is not a number of Arauto now. Nothing was saved.",
        "{alias} сейчас не является номером Arauto. Ничего не было сохранено.",
    ),
    (
        "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
        "Введите другой телефон, на котором работает его WhatsApp, или выберите другой вариант. Ничего не было сохранено.",
    ),
    (
        "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
        "Другой телефон для его WhatsApp не является телефоном: от 8 до 15 цифр, сначала + и код страны. Ничего не было сохранено.",
    ),
    (
        "Line {n} of Other telephones is not a telephone. Nothing was saved.",
        "Строка {n} в Других телефонах не является телефоном. Ничего не было сохранено.",
    ),
    (
        "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
        "Его собственный чат и список телефонов — это два разных ответа: очистите список или выберите Эти телефоны. Ничего не было сохранено.",
    ),
    (
        "Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.",
        "Сначала выберите WhatsApp для {hermes}: у чата два конца. Ничего не было сохранено.",
    ),
    (
        "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
        "Его WhatsApp не может одновременно отдавать ему приказы: этот чат — режим self-chat. Ничего не было сохранено.",
    ),
    (
        "At most 16 telephones can give it orders; these are {n}. Nothing was saved.",
        "Приказы ему могут отдавать не более 16 телефонов; здесь их {n}. Ничего не было сохранено.",
    ),
    (
        "1 typed telephone needs a look. Nothing was saved.",
        "1 введённый телефон требует проверки. Ничего не было сохранено.",
    ),
    (
        "{n} typed telephones need a look. Nothing was saved.",
        "Введённых телефонов, требующих проверки: {n}. Ничего не было сохранено.",
    ),
    ("Line {n}", "Строка {n}"),
    (
        "Not a telephone: 8 to 15 digits, with + and the country code first.",
        "Это не телефон: от 8 до 15 цифр, сначала + и код страны.",
    ),
    ("Not a number of Arauto.", "Это не номер Arauto."),
    (
        "The same telephone as line {n}: it counts once.",
        "Тот же телефон, что в строке {n}: он считается один раз.",
    ),
    (
        "The same telephone as {alias}, chosen above: it counts once.",
        "Тот же телефон, что {alias}, выбранный выше: он считается один раз.",
    ),
    (
        "This is its WhatsApp: it cannot also give it orders.",
        "Это его WhatsApp: он не может одновременно отдавать ему приказы.",
    ),
    (
        "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
        "Введён без +, поэтому первые цифры прочитаны как код страны. Если это не его страна, сначала введите + и код страны. Повторное сохранение оставит его в том виде, как он прочитан.",
    ),
    (
        "{n} of 16 telephones, with the numbers chosen above.",
        "{n} из 16 телефонов, считая выбранные выше номера.",
    ),
    (
        "These chats stop being protected",
        "Эти чаты перестанут быть защищёнными",
    ),
    (
        "Between {bridge}, its WhatsApp until now, and:",
        "Между {bridge}, его прежним WhatsApp, и:",
    ),
    ("itself (self-chat mode)", "им самим (режим self-chat)"),
    (
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
        "Arauto снова позволит любому Hermes писать в этих чатах и выдавать себя там за того, кто отдаёт приказы {hermes}.",
    ),
    ("Remove the protection", "Снять защиту"),
    ("Save without protection", "Сохранить без защиты"),
    ("What {alias} is for it", "Чем {alias} является для него"),
    (
        "A telephone that gives it orders",
        "Телефон, который отдаёт ему приказы",
    ),
    (
        "Its WhatsApp, where it takes orders",
        "Его WhatsApp, где он принимает приказы",
    ),
    (
        "takes orders on {alias} now",
        "сейчас принимает приказы на {alias}",
    ),
    (
        "{alias} gives it orders now",
        "{alias} сейчас отдаёт ему приказы",
    ),
    (
        "Nothing is saved here. The window of the Hermes opens with {alias} chosen.",
        "Здесь ничего не сохраняется. Окно Hermes откроется с выбранным {alias}.",
    ),
    (
        "Choose a Hermes and what {alias} is for it.",
        "Выберите Hermes и чем {alias} является для него.",
    ),
    (
        "{alias} is the WhatsApp of {hermes}: it cannot also give it orders.",
        "{alias} — WhatsApp {hermes}: он не может одновременно отдавать ему приказы.",
    ),
    ("Actions: {name}", "Действия: {name}"),
    (
        "Issues the first invite for {name}.",
        "Выдаёт первое приглашение для {name}.",
    ),
    (
        "Goes back to the connection of {name} that is in progress.",
        "Возвращает к текущему подключению {name}.",
    ),
    (
        "Issues an invite. When it is used, {name} moves to the new device.",
        "Выдаёт приглашение. Когда его используют, {name} переходит на новое устройство.",
    ),
    (
        "Which chat or telephones may give it orders on WhatsApp.",
        "Какой чат или какие телефоны могут отдавать ему приказы в WhatsApp.",
    ),
    (
        "Closes {name} in the hub. Asks first.",
        "Закрывает {name} в хабе. Сначала спрашивает.",
    ),
    (
        "Opens the window to pair {alias} with a QR code or a pairing code.",
        "Открывает окно, чтобы связать {alias} через QR-код или код сопряжения.",
    ),
    (
        "Goes back to the pairing of {alias} that is running.",
        "Возвращает к текущему сопряжению {alias}.",
    ),
    (
        "Shows the pairing of {alias} that another panel session started.",
        "Показывает сопряжение {alias}, которое начала другая сессия панели.",
    ),
    (
        "Stops {alias} for every Hermes. Asks for the alias.",
        "Останавливает {alias} для всех Hermes. Просит ввести псевдоним.",
    ),
    (
        "Starts the failed session of {alias} again.",
        "Заново запускает сессию {alias}, которая завершилась сбоем.",
    ),
    (
        "Another phone paired as {alias}: keep it or undo.",
        "Другой телефон сопряжён как {alias}: оставить или отменить.",
    ),
    (
        "Which Hermes takes orders in the chats of this number.",
        "Какой Hermes принимает приказы в чатах этого номера.",
    ),
    (
        "Takes {alias} out of the hub. Asks for the alias.",
        "Убирает {alias} из хаба. Просит ввести псевдоним.",
    ),
    ("Dismiss", "Скрыть"),
    ("Dismiss: {title}", "Скрыть: {title}"),
    (
        "The message of this invite cannot be shown again. If it did not reach the Hermes, issue another invite: this one then stops working.",
        "Сообщение этого приглашения нельзя показать снова. Если оно не дошло до Hermes, выпустите другое приглашение: это перестанет работать.",
    ),
    ("Nothing has arrived yet", "Пока ничего не пришло"),
    (
        "The Hermes may be waiting for your approval to run the command, in the command channel.",
        "Возможно, Hermes ждёт вашего разрешения выполнить команду, в командном канале.",
    ),
    (
        "The Hermes may not reach this hub: check that its machine reaches the address of the hub.",
        "Возможно, Hermes не достаёт до этого хаба: проверьте, что его машина доходит до адреса хаба.",
    ),
    (
        "The message may have gone to another chat: it only works in the command channel of this Hermes.",
        "Возможно, сообщение ушло в другой чат: оно работает только в командном канале этого Hermes.",
    ),
    (
        "Type the name {name} to confirm.",
        "Введите имя {name} для подтверждения.",
    ),
    (
        "The name typed does not match. Nothing was done.",
        "Введённое имя не совпадает. Ничего не сделано.",
    ),
    ("Hermes {name}: {state}.", "Hermes {name}: {state}."),
    (
        "Saved. Who gives {hermes} orders on WhatsApp: {orders}.",
        "Сохранено. Кто отдаёт приказы {hermes} в WhatsApp: {orders}.",
    ),
    (
        "Outside WhatsApp: another conversation platform, like Telegram",
        "Вне WhatsApp: другая платформа для общения, например Telegram",
    ),
    (
        "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead.",
        "Тогда Arauto ничего не отклоняет для {hermes}. Хаб не может это проверить: если {hermes} принимает приказы ещё и в каком-то WhatsApp, запишите вместо этого тот WhatsApp.",
    ),
    (
        "Orders from outside WhatsApp take no telephone: choose It has no WhatsApp, and leave These telephones and Other telephones empty. Nothing was saved.",
        "Приказы вне WhatsApp не требуют телефона: выберите У него нет WhatsApp и оставьте пустыми Эти телефоны и Другие телефоны. Ничего не сохранено.",
    ),
    ("outside WhatsApp", "вне WhatsApp"),
    (
        "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it.",
        "Сохранено. {hermes} принимает приказы вне WhatsApp: Arauto ничего для него не отклоняет.",
    ),
];
