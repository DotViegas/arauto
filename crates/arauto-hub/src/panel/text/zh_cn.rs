//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "面板"),
    ("Language", "语言"),
    ("Change", "更改"),
    ("R", "读"),
    ("S", "发"),
    ("M", "改"),
    ("W", "醒"),
    ("Waiting for the device…", "正在等待设备…"),
    ("Sign in with a passkey", "使用通行密钥登录"),
    ("Installation code", "安装码"),
    ("Continue", "继续"),
    ("Sign out", "退出登录"),
    ("Alerts", "告警"),
    ("Unknown devices", "未知设备"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "有 {n} 个设备出示了有效凭据，但数据库中没有对应记录。每个设备都只能通过新的邀请恢复。用 {command} 查看。",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "还有 {n} 条告警。用 {command} 查看全部。",
    ),
    ("Accesses", "访问权限"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "在此版本中，面板可以连接和撤销 Hermes，并更改每个号码的访问权限。配对号码和选择会话仍需在 hub 的服务器上用 {command} 命令完成。",
    ),
    ("none", "无"),
    ("Your Hermes and their [numbers]", "您的 Hermes 及其 [号码]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "已连接：{connected} · 等待中：{waiting} · 告警中：{stopped}",
    ),
    ("Connect Hermes", "连接 Hermes"),
    ("State", "状态"),
    ("Numbers", "号码"),
    ("Can be woken", "可被唤醒"),
    ("Connector", "连接器"),
    ("Seen", "最近出现"),
    ("Actions", "操作"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "还没有 Hermes。用“连接 Hermes”按钮连接第一个。",
    ),
    ("yes", "是"),
    ("no", "否"),
    ("Connect", "连接"),
    ("Reconnect", "重新连接"),
    ("Revoke", "撤销"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "“号码”指该 Hermes 可以访问的号码。连接器一栏是设备自行声明的内容，未经验证。",
    ),
    ("Engine", "引擎"),
    (
        "There is no engine yet. Set it with {command}.",
        "还没有引擎。用 {command} 设置。",
    ),
    ("Alias", "别名"),
    ("Number", "号码"),
    ("Hermes with access", "有访问权限的 Hermes"),
    ("Last message", "最后一条消息"),
    (
        "No number yet. Add one with {command}.",
        "还没有号码。用 {command} 添加。",
    ),
    ("command", "指令"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "最后一条消息只显示时间，从不显示内容。面板只列出经过 hub 的内容：同一号码关联的其他设备只在手机上显示。",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "行是 Hermes，列是号码。点击单元格即可更改。",
    ),
    ("Connect a Hermes first.", "请先连接一个 Hermes。"),
    ("Add a WhatsApp first.", "请先添加一个 WhatsApp。"),
    ("(accesses kept)", "（访问权限已保留）"),
    ("no access", "无访问权限"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} 读取 · {s} 发送 · {m} 修改 · {w} 被唤醒。",
    ),
    ("Saved: {what}", "已保存：{what}"),
    ("Warnings that block nothing", "不阻止任何操作的警告"),
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "这个安装码无效。请使用最新的：日志最后一个块中的安装码，或在 hub 的服务器上签发的安装码。",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "hub 没有接受这个通行密钥。请重试。",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "您的面板会话已结束。输入最新的安装码，即可从中断的地方继续。",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "此浏览器无法在这里使用通行密钥。请用最新的浏览器，直接在面板地址打开面板（不要在其他应用内打开）。",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "通行密钥没有创建成功：请求被取消，或等待时间太长。再试一次即可。",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "没有使用任何通行密钥。如果它不在此设备上，请使用您创建它的那台设备或密码管理器，或者打开“丢了通行密钥？”。",
    ),
    ("Passkey", "通行密钥"),
    ("Check", "检查"),
    ("Welcome to your Arauto hub", "欢迎使用您的 Arauto hub"),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "我们来设置您的首次访问。只需三个简单的步骤，而且只需做一次。",
    ),
    ("Step 1 · Confirm it is you", "第 1 步 · 确认是您本人"),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "hub 启动时，在自己的日志中写入了一个安装码。只有管理服务器的人才能看到这份日志：hub 就是这样确认是您本人的。",
    ),
    ("Where to find the code:", "在哪里找到安装码："),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "在 Easypanel 中，打开 hub 的服务，进入 Logs。（使用 Docker 时：{command}）",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "找到 ARAUTO FIRST ACCESS 块：最新的是最后一个，在日志的最底部。复制其中的安装码。",
    ),
    ("Paste the code here:", "请在这里粘贴安装码："),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "安装码不会过期，只能使用一次。hub 重启后会写入新的安装码：请始终使用最新的那个。",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "打不开日志？请在 hub 的服务器上运行 {command}。注意：这个新安装码会让日志中的安装码失效。",
    ),
    ("Good to have you back", "欢迎回来"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "请用您的通行密钥登录：使用此设备的指纹、面容或 PIN，或者您的密码管理器。",
    ),
    ("Lost your passkey?", "丢了通行密钥？"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "没关系。请在 hub 的服务器上运行 {command}。它会显示一个 10 分钟内有效的安装码：在这里输入它，然后创建新的通行密钥。",
    ),
    (
        "Code accepted! Now, create your passkey",
        "安装码已接受！现在来创建您的通行密钥",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "今后您就用它登录面板，不需要记任何密码。",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "通行密钥保存在此设备或您的密码管理器中，用您的指纹、面容或设备的 PIN 打开。hub 只保存用来验证通行密钥的内容：没有可泄露的密码。",
    ),
    ("It will work at this address:", "它将在此地址生效："),
    ("Create my passkey", "创建我的通行密钥"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "如果您使用密码管理器，通行密钥可以同步到您的其他设备。",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "不小心离开了这个页面？再输入同一个安装码即可：只有在创建通行密钥时它才会被用掉。",
    ),
    ("Check the installation", "检查安装情况"),
    // guide.rs
    ("Name", "名称"),
    ("Code", "验证码"),
    ("Done", "完成"),
    ("Invite issued", "邀请已签发"),
    ("A device used the invite", "有设备使用了邀请"),
    (
        "Code checked and request approved",
        "验证码已核对，请求已批准",
    ),
    (
        "The connector confirmed its first credential",
        "连接器已确认其第一份凭据",
    ),
    (
        "A device used the invite, from {address}",
        "有设备使用了邀请，来自 {address}",
    ),
    ("waiting…", "等待中…"),
    (
        "What name will this Hermes have in the panel?",
        "这个 Hermes 在面板中叫什么名称？",
    ),
    ("Name of the Hermes", "Hermes 的名称"),
    ("Preparing a secure connection", "正在准备安全连接"),
    (
        "The hub drew the invite of {name}.",
        "hub 已为 {name} 生成邀请。",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "{name} 的邀请 · 剩余有效期 {time} · 仅限使用一次",
    ),
    ("Copied", "已复制"),
    ("Click the block to copy it.", "点击文本块即可复制。"),
    (
        "Select the whole text and copy it.",
        "请选中全部文本并复制。",
    ),
    ("It only appears here, once.", "它只在这里显示一次。"),
    (
        "The invite issued before was cancelled.",
        "之前签发的邀请已取消。",
    ),
    (
        "The request that was in progress was closed.",
        "进行中的请求已关闭。",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "在新设备确认其第一份凭据之前，正在使用的设备继续工作。",
    ),
    (
        "This hub has no connector to serve",
        "此 hub 没有可提供的连接器",
    ),
    ("Waiting for the Hermes {name}", "正在等待 Hermes {name}"),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "hub 实际观察到某一步时，该步骤才会亮起。如果 Hermes 请求批准运行命令，由您在指令通道中批准。",
    ),
    (
        "A device used the invite of {name}",
        "有设备使用了 {name} 的邀请",
    ),
    ("unknown", "未知"),
    ("Seen by the hub", "hub 观察到的"),
    ("address {address}, {when}", "地址 {address}，{when}"),
    ("Said by the device", "设备声明的"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "主机“{host}” · 系统“{os}” · Hermes“{version}”（未验证）",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "当前设备的最后一次使用：{when}，来自 {address}。您认得这次使用吗？",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "当前设备的最后一次使用：{when}。您认得这次使用吗？",
    ),
    ("I recognize it", "我认得"),
    ("I do not recognize it", "我不认得"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "如果 Hermes 当时已关闭却有使用记录，说明有人使用了副本。",
    ),
    ("Verification code", "验证码"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "请输入 Hermes 在指令通道中回复的验证码。面板从不显示此验证码。",
    ),
    ("The request of {name} was denied", "{name} 的请求已被拒绝"),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "验证码已第五次输入错误。如需连接，请签发新的邀请。",
    ),
    ("The request of {name} expired", "{name} 的请求已过期"),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "输入验证码的 10 分钟已过。如需连接，请签发新的邀请。",
    ),
    ("The identity of {name} was copied", "{name} 的身份已被复制"),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "批准之前有两个设备以相同身份出现，请求已被中止。如需连接，请签发新的邀请。",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} 没有邀请，也没有等待中的请求",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "邀请已过期、已取消，或请求已处理完毕。如果 Hermes 没有回复验证码，请不要批准；如果它回复了 AR-102，说明另一个设备使用了邀请。如需连接，请签发新的邀请。",
    ),
    ("{name} is connected", "{name} 已连接"),
    ("{name} was approved", "{name} 已批准"),
    (
        "What it may do on each number is in the Accesses tab.",
        "它在每个号码上可以做什么，见“访问权限”标签页。",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "连接器一经确认，即显示为已连接。它原有的访问权限会重新适用于此设备。",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "连接器一经确认，即显示为已连接。它目前对任何号码都没有访问权限。",
    ),
    (
        "The hub did not do what was asked",
        "hub 未执行所请求的操作",
    ),
    ("Issue invite", "签发邀请"),
    ("Cancel", "取消"),
    ("Next step", "下一步"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "复制后，请将其粘贴到 Hermes {name} 的指令通道中。不要粘贴到其他任何地方。",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "邀请的剩余有效期为 {time}。此页面会自动刷新。",
    ),
    ("Approve", "批准"),
    ("Deny", "拒绝"),
    ("Confirm within {time}.", "请在 {time} 内确认。"),
    ("Issue another invite", "签发新的邀请"),
    ("Close", "关闭"),
    ("See accesses", "查看访问权限"),
    ("Finish", "结束"),
    ("Copied to the clipboard.", "已复制到剪贴板。"),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "文本已选中。请按 Ctrl+C 复制。",
    ),
    ("Reconnect {name}", "重新连接 {name}"),
    ("Connect {name}", "连接 {name}"),
    ("Steps", "步骤"),
    ("How it works:", "工作方式："),
    (
        "You give the Hermes a name here.",
        "您在这里给 Hermes 起个名字。",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Hermes 会从这个 hub 下载连接器，并回复一个 6 位数的验证码。",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "您在这里输入验证码并批准。完成：该 Hermes 的连接器一确认，它就会显示为已连接。",
    ),
    (
        "Let's connect your first Hermes",
        "我们来连接您的第一个 Hermes",
    ),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "被唤醒需要读取权限。未保存任何内容。",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "发起会话需要发送权限，且范围须为“所有会话”。未保存任何内容。",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "唤醒前的等待时间为 0 到 60 秒。未保存任何内容。",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "hub 中出现故障。请查看 hub 的日志，并在矩阵中核对访问权限。",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "数据库已从较旧的备份恢复，hub 尚未对此进行处理。未写入任何内容。请重启 hub。",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "Hermes {hermes} 对号码 {number} 没有访问权限。",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Hermes {hermes} 只能访问号码 {number} 的选定会话。有规则行的会话：{lines}。",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Hermes {hermes} 在号码 {number} 上，所有会话：",
    ),
    ("reads", "可读取"),
    ("does not read", "不可读取"),
    ("sends", "可发送"),
    ("does not send", "不可发送"),
    ("modifies", "可修改"),
    ("does not modify", "不可修改"),
    (
        "is woken after {seconds} s of silence",
        "静默 {seconds} 秒后被唤醒",
    ),
    ("is not woken", "不被唤醒"),
    ("may start chats", "可发起会话"),
    (
        "chats with their own rule: {lines}",
        "有单独规则的会话：{lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes} 在号码 {number} 上",
    ),
    ("Where it applies", "适用范围"),
    (
        "Every chat, including the ones that come later",
        "所有会话，包括以后出现的会话",
    ),
    ("Only chosen chats", "仅选定会话"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "选择“仅选定会话”时，对该 Hermes 而言只存在有单独规则行的会话，它在每个会话中可以做什么由该规则行决定。规则行仍需用 {command} 命令创建。当前：{lines}。",
    ),
    ("What it may do", "可以做什么"),
    ("Read", "读取"),
    (
        "see chats and messages, search, download files",
        "查看会话和消息、搜索、下载文件",
    ),
    ("Send", "发送"),
    ("write, react, mark as read", "写消息、回应、标记为已读"),
    ("Modify", "修改"),
    ("edit and delete what already exists", "编辑和删除已有内容"),
    ("Be woken", "被唤醒"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "消息到达时 hub 发出通知（需要读取权限）",
    ),
    ("Wait", "等待时间"),
    ("seconds of silence before waking", "唤醒前的静默秒数"),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "可以与从未和该号码交谈过的人发起会话（需要发送权限和所有会话）",
    ),
    ("Today:", "当前："),
    ("Save", "保存"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "从下一次调用起生效，无需重启任何程序。如果一项都不勾选，访问权限将被移除。",
    ),
    ("Revoke {name}", "撤销 {name}"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "撤销会在 hub 中关闭该 Hermes：正在使用的设备、进行中的请求和已签发的邀请。通道会在几秒内断开。",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "访问权限会保留，并在连接新设备后重新生效。恢复的唯一途径是新的邀请。",
    ),
    // data.rs
    ("never", "从未"),
    ("now", "刚刚"),
    ("{n} min ago", "{n} 分钟前"),
    ("{n} h ago", "{n} 小时前"),
    ("{n} d ago", "{n} 天前"),
    (
        "a credential that had already been swapped was presented",
        "出示了已被更换的凭据",
    ),
    ("two connections at the same time", "同时出现两个连接"),
    ("the credential could not be read", "无法读取凭据"),
    (
        "the last use was not recognized on the reconnect",
        "重新连接时最后一次使用未被认可",
    ),
    (
        "the identity was copied before the approval",
        "身份在批准之前被复制",
    ),
    (
        "the hub no longer trusts the credential",
        "hub 不再信任该凭据",
    ),
    ("Waiting for your code", "正在等待您输入验证码"),
    (
        "Approved, waiting for the connector",
        "已批准，正在等待连接器",
    ),
    ("Waiting for the Hermes", "正在等待 Hermes"),
    ("Connected", "已连接"),
    ("Disconnected", "已断开"),
    ("In quarantine", "隔离中"),
    ("Suspended", "已暂停"),
    ("No device", "无设备"),
    ("a Hermes", "某个 Hermes"),
    ("Invite", "邀请"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "另一个设备试图使用 {hermes} 的邀请。文本块流到了别处。第一个请求仍然有效。",
    ),
    ("Identity copied", "身份被复制"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "{hermes} 的身份在批准之前被复制。请求已被拒绝。",
    ),
    ("Hermes in quarantine", "Hermes 隔离中"),
    ("{hermes}. Reason: {reason}.", "{hermes}。原因：{reason}。"),
    ("Suspected copy", "疑似副本"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "重新连接时，{hermes} 的最后一次使用未被认可。",
    ),
    ("Connection taken", "连接被接管"),
    (
        "the connection of {hermes} was taken by another one.",
        "{hermes} 的连接已被另一个连接接管。",
    ),
    ("Closing", "关闭"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} 请求关闭自身。",
    ),
    ("Reuse", "重复使用"),
    (
        "a copy of {hermes} was used before the closing.",
        "{hermes} 的副本在关闭之前被使用过。",
    ),
    ("Unconfirmed swaps", "未确认的更换"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} 在 24 小时内三次收到凭据而未确认。可能是网络不佳，也可能是有人在尝试使用副本。",
    ),
    ("Audit mark", "审计标记"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "已写入一条安全事件，但数据库之外的标记没有同步更新。",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "引擎的版本不是 hub 已检查过的版本。",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "某个引擎会话使用了其他配置，hub 已应用自己的配置。",
    ),
    ("Number swapped", "号码不符"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "完成配对的手机所属号码与所提供的号码不同。在您作出决定之前，访问权限保持停用。",
    ),
    ("Address: {address}.", "地址：{address}。"),
    ("an unknown date", "未知日期"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "hub 已从 {date} 的备份恢复。此后批准、撤销、重新连接的内容以及对访问权限的更改均已丢失。请重新连接每个 Hermes 并核对访问权限。",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "hub 已从备份恢复。{name} 已暂停，只能通过新的邀请恢复：请重新连接或撤销它。没有“重新激活”这一操作。",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "hub 不再信任 {name} 的凭据。原因：{reason}。它的访问已停用。解决办法：重新连接或撤销。没有“解除”这一操作。",
    ),
    ("Not paired", "未配对"),
    ("Pairing", "配对中"),
    ("Reconnecting", "重新连接中"),
    ("Failed", "失败"),
    ("Removed", "已移除"),
    ("version {version}, checked", "版本 {version}，已检查"),
    (
        "version {version}, not checked by the hub",
        "版本 {version}，hub 未检查",
    ),
    ("version not read yet", "尚未读取版本"),
    ("last answer {when}", "最后一次响应：{when}"),
    ("has not answered yet", "尚未响应"),
    ("all", "全部"),
    ("all · own rules: {n}", "全部 · 单独规则：{n}"),
    ("chosen chats: {n}", "选定会话：{n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}：有 {n} 个 Hermes 在相同的会话中被唤醒。",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}：有 {n} 个 Hermes 在相同的会话中发送。",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} 被标记为指令号码，却有 Hermes 获准在其上发送。",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "已有使用该名称的 Hermes。如需为它关联另一个设备，请使用它所在行的按钮。",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "该名称属于一个已移除的 Hermes，名称不会重复使用。请另选一个。",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "输入验证码之前，请先说明您是否认得当前设备的最后一次使用。未计入尝试次数。",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "在您回答期间，当前设备被使用过。请查看它现在的最后一次使用并重新回答。未批准任何内容。",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "验证码为 6 位数字，例如 123-456。未计入尝试次数。",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "数据库已从较旧的备份恢复，hub 尚未对此进行处理。未写入任何内容。请重启 hub：它会开启恢复模式，之后每个 Hermes 都需用新的邀请重新连接。",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "hub 中出现故障，未执行任何操作。请查看 hub 的日志。",
    ),
    (
        "The name has from 1 to 32 characters.",
        "名称长度为 1 到 32 个字符。",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "名称须以小写字母或数字开头。",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "只能使用小写字母、数字、连字符和下划线。",
    ),
    (
        "That name is reserved. Choose another.",
        "该名称已被保留。请另选一个。",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "验证码不匹配。第 {attempts} 次尝试，共 {max} 次。请输入 Hermes 最后回复的验证码。",
    ),
    // mod.rs
    (
        "Too many attempts right now. Wait a minute and try again.",
        "当前尝试次数过多。请等待一分钟后重试。",
    ),
    // check.rs
    ("ok", "正常"),
    ("check", "请核对"),
    ("attention", "注意"),
    ("later", "稍后"),
    ("failed", "失败"),
    ("Panel address", "面板地址"),
    ("Connectors address", "连接器地址"),
    ("Your address", "您的地址"),
    ("Database password", "数据库密码"),
    ("Data folder", "数据文件夹"),
    ("WhatsApp engine", "WhatsApp 引擎"),
    ("Your passkey works here.", "您的通行密钥在这里可以使用。"),
    (
        "Open it: it should say it is the address of the connectors.",
        "请打开它：页面应当说明这是连接器的地址。",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "hub 看到您的地址是 {address}。这是您的地址吗？如果不是，说明代理没有传递真实地址。",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "{address} 不在 ARAUTO_TRUSTED_PROXIES 中，但来自它的一个连接带来了转发的地址。请把这个连接的来源地址加入 ARAUTO_TRUSTED_PROXIES，然后重新部署。",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "hub 看到您的地址是 {address}，这是内部网络的地址：代理尚未被信任。请把代理的地址加入 ARAUTO_TRUSTED_PROXIES，然后重新部署。",
    ),
    (
        "Replaced by one nobody saw.",
        "已换成一个无人见过的新密码。",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "hub 没有替换：此安装环境自行设置数据库密码。",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "替换失败。请查看 hub 的日志，然后运行 {command}。",
    ),
    ("Ready for your Hermes.", "已为您的 Hermes 准备就绪。"),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "下载文件夹是空的：无法通过面板连接 Hermes。请重新构建 hub 的镜像。",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "里面存放着主密钥和数据库密码。请备份：没有它，hub 就无法打开它加密保存的内容。",
    ),
    ("Configured.", "已配置。"),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "没有它也可以连接 Hermes；WhatsApp 号码则需要它。",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "hub 自己检查了几项内容。这里不会更改任何东西：只是让您了解检查结果。",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "第 3 步 · 来检查一下安装情况",
    ),
    (
        "All set: your passkey was created!",
        "一切就绪：您的通行密钥已创建！",
    ),
    (
        "The same by command: {command}",
        "用命令也能得到同样的结果：{command}",
    ),
    ("Connect my first Hermes", "连接我的第一个 Hermes"),
    ("Do it later", "以后再说"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "是我本人在请你：把这个 Hermes 以“{name}”的名称连接到我们的 Arauto hub。",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "请阅读 {page} 并按其中的说明操作。如有不清楚的地方，请问我。",
    ),
    ("Hub:", "hub："),
    ("Hub key:", "hub 密钥："),
    ("Connector sha256:", "连接器 sha256："),
    (
        "Invite (one use, until {deadline} UTC):",
        "邀请（仅限一次，有效期至 {deadline} UTC）：",
    ),
    (
        "When it is done, send me only the verification code.",
        "完成后，请只把验证码发给我。",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "请把它作为一条新消息发送，不要作为对它某条通知的回复。如果它要求您确认，请用您自己的话回答。",
    ),
    ("Show the command instead", "改为显示指令"),
    (
        "Send one or the other, not both: they carry the same invite.",
        "两种方式只需发送其中一种，不要都发：它们包含同一个邀请。",
    ),
    ("Click the message to copy it.", "点击消息即可复制。"),
    ("Message", "消息"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "hub 会生成一个邀请并用它写成一条消息，您只需把这条消息发到该 Hermes 的指令通道，不要发到其他任何地方。",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "小写字母、数字、连字符和下划线，最多 32 个字符，以字母或数字开头。名称会显示在消息中，之后不能更改。",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "这条消息会让 Hermes 从此 hub 下载连接器，而下载文件夹是空的。没有签发任何内容。请在 hub 的服务器上用 {command} 命令签发 {name} 的邀请。",
    ),
];
