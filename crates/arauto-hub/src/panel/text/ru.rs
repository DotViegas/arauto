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
    ("Sign in to the panel", "Вход в панель"),
    ("Entry code", "Код входа"),
    (
        "Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in.",
        "Выпустите код на сервере хаба командой {command}. Он действует 10 минут и только для одного входа.",
    ),
    ("Sign in", "Войти"),
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
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "В этой версии панель подключает и отзывает Hermes и меняет доступы к каждому номеру. Привязка номеров и выбор чатов пока выполняются командой {command} на сервере хаба.",
    ),
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
    ("Can be woken", "Пробуждение"),
    ("Connector", "Коннектор"),
    ("Seen", "На связи"),
    ("Actions", "Действия"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Пока нет ни одного Hermes. Подключите первый кнопкой «Подключить Hermes».",
    ),
    ("yes", "да"),
    ("no", "нет"),
    ("Continue", "Продолжить"),
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
        "No number yet. Add one with {command}.",
        "Пока нет ни одного номера. Добавьте номер командой {command}.",
    ),
    ("command", "командный"),
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
    // guide.rs
    ("Name", "Имя"),
    ("Block", "Блок"),
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
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "Строчные буквы, цифры, дефис и подчёркивание, до 32 символов, первый символ — буква или цифра. Имя попадает в блок и потом не меняется.",
    ),
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
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "Блок велит Hermes скачать коннектор с этого хаба, а папка загрузок пуста. Ничего не выпущено. Выпустите приглашение для {name} командой {command} на сервере хаба.",
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
        "the hub gives notice when a message arrives (requires reading)",
        "хаб даёт уведомление, когда приходит сообщение (нужно право читать)",
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
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} отмечен как командный номер, и есть Hermes, которым разрешено отправлять на нём.",
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
        "Invalid or expired code. Issue another one on the hub's server.",
        "Код неверен или срок его действия истёк. Выпустите другой на сервере хаба.",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "Сейчас слишком много попыток. Подождите минуту и попробуйте снова.",
    ),
];
