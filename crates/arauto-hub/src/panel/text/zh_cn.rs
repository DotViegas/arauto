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
    ("none", "无"),
    ("Your Hermes and their [numbers]", "您的 Hermes 及其 [号码]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "已连接：{connected} · 等待中：{waiting} · 告警中：{stopped}",
    ),
    ("Connect Hermes", "连接 Hermes"),
    ("State", "状态"),
    ("Numbers", "号码"),
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
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "消息到达时 hub 发出通知（需要读取权限）。关闭它时，也会在此访问中有独立规则的聊天里关闭。",
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
    ("Connector sha256 ({system}):", "连接器 sha256 ({system})："),
    (
        "Invite (one use, until {deadline} UTC):",
        "邀请（仅限一次，有效期至 {deadline} UTC）：",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "完成后，请把验证码和页面要求的那几行发给我。",
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
    ("Org chart", "组织架构图"),
    ("Who talks to [whom]", "谁与[谁]对话"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "拖动 Hermes 可更改其层级。将卡片上的紫色圆点拖到另一张卡片，即可创建链接。点击箭头或卡片可进行调整。",
    ),
    ("View", "视图"),
    ("Map", "地图"),
    ("List", "列表"),
    ("+ Level", "+ 层级"),
    ("Undo", "撤销"),
    ("Publish", "发布"),
    ("Draft: 1 change", "草稿：1 项更改"),
    ("Draft: {n} changes", "草稿：{n} 项更改"),
    (
        "From a higher level to a lower one: instruction",
        "从较高层级到较低层级：指示",
    ),
    (
        "From a lower level to a higher one: request",
        "从较低层级到较高层级：请求",
    ),
    ("Same level: note", "同一层级：便条"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "层级和职责只在这里显示：不会告知任何 Hermes。便条是数据，但带有遵循标记的指示箭头除外：此时通知会告诉下层 Hermes，在那一次运行中遵循上层 Hermes 的这些新消息，且低于其操作者的指示。只有链接才能让一个 Hermes 向另一个 Hermes 发送便条。",
    ),
    ("instruction", "指示"),
    ("request", "请求"),
    ("note", "便条"),
    ("Name of level {n}", "第 {n} 层级的名称"),
    ("Remove empty level", "移除空层级"),
    ("Move up", "上移"),
    ("Move down", "下移"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "层级名称由 1 到 32 个小写字母、数字、连字符或下划线组成，以字母或数字开头，且不能是保留字。",
    ),
    (
        "Another level already has this name.",
        "已有另一个层级使用此名称。",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "组织架构图有 1 到 {max} 个层级。",
    ),
    (
        "Drag to another Hermes to create a link",
        "拖到另一个 Hermes 以创建链接",
    ),
    ("{alias} · answers", "{alias} · 应答"),
    ("{alias} · reads only", "{alias} · 仅读取"),
    ("no number", "无号码"),
    ("not placed yet", "尚未安排位置"),
    (
        "{name}, level {level}, {state}",
        "{name}，第 {level} 层级，{state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "尚未安排位置：{names}。它们显示在最低层级；发布后才会保留在那里。",
    ),
    ("Kind: {kind}", "类型：{kind}"),
    ("Wakes {name}", "唤醒 {name}"),
    ("Create the way back", "创建返程链接"),
    ("Remove the way back", "移除返程链接"),
    ("Remove", "移除"),
    ("This link already exists.", "此链接已存在。"),
    ("Role of this Hermes", "此 Hermes 的职责"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "供您自己阅读：在此版本中没有任何 Hermes 会看到它。最多 280 个字符，单行。",
    ),
    ("Level", "层级"),
    ("Move left", "左移"),
    ("Move right", "右移"),
    ("Change them in Accesses", "请在“访问权限”中更改"),
    ("No access to any number.", "没有任何号码的访问权限。"),
    ("Link to", "链接到"),
    ("Add link", "添加链接"),
    ("Links of this Hermes", "此 Hermes 的链接"),
    ("Levels", "层级"),
    ("Links", "链接"),
    ("Role", "职责"),
    ("From", "来源"),
    ("To", "目标"),
    ("Kind", "类型"),
    ("Wakes", "唤醒"),
    ("nobody", "无人"),
    ("No link.", "没有链接。"),
    ("Publish these changes?", "发布这些更改？"),
    (
        "Links are security rules. They only apply after you publish.",
        "链接属于安全规则。发布之后才会生效。",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "链接会让请求跨越号码：通过某个号码到达一个 Hermes 的内容，可能变成发给另一个 Hermes 的便条，而后者会在自己的号码上执行操作。",
    ),
    ("Back", "返回"),
    (
        "New link: {from} → {to} ({kind})",
        "新链接：{from} → {to}（{kind}）",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "新链接：{from} → {to}（{kind}），唤醒 {to}",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "链接 {from} → {to}：现在会唤醒 {to}",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "链接 {from} → {to}：不再唤醒 {to}",
    ),
    ("Link removed: {from} → {to}", "链接已移除：{from} → {to}"),
    (
        "{name} changes level: {before} → {after}",
        "{name} 更换层级：{before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} 被安排到层级 {after}",
    ),
    ("Role of {name} changed", "{name} 的职责已更改"),
    ("Places on the map changed: {n}", "地图上已更改的位置：{n}"),
    ("New level: {name}", "新层级：{name}"),
    (
        "Level renamed: {before} → {after}",
        "层级已重命名：{before} → {after}",
    ),
    ("Level removed: {name}", "层级已移除：{name}"),
    ("Order of the levels: {names}", "层级顺序：{names}"),
    ("Published.", "已发布。"),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "在您编辑期间，有人更改了组织架构图：另一次发布、通过命令更改的链接，或新增或移除的 Hermes。没有发布任何内容。您的草稿已保留：请检查更改后重新发布。",
    ),
    ("Nothing was published.", "没有发布任何内容。"),
    ("Two levels have the same name.", "有两个层级同名。"),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "职责最多 280 个字符，单行，且不含控制字符或方向字符。",
    ),
    (
        "A place on the map is out of range.",
        "地图上有一个位置超出范围。",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "草稿中包含不存在的 Hermes 或层级，或遗漏了某个 Hermes。",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "一个链接连接两个不同的 Hermes，每个方向各一次。",
    ),
    ("The draft could not be read.", "无法读取草稿。"),
    ("The draft is too large to publish.", "草稿太大，无法发布。"),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "hub 出现故障。没有发布任何内容。请查看 hub 的日志。",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "编辑组织架构图需要面板的脚本。没有脚本时，这里显示的是已发布的组织架构图。",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "还没有 Hermes。请在 Hermes 标签页中连接第一个。",
    ),
    ("Connect WhatsApp (WAHA)", "连接 WhatsApp (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "连接 WhatsApp (原生 · 测试中)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "还没有号码。用“连接 WhatsApp (WAHA)”按钮添加第一个号码。",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "在此版本中，面板可以连接和撤销 Hermes，在 WAHA 上配对和管理 WhatsApp 号码，更改每个号码的访问权限，并编辑带链接的组织架构图。选择聊天仍需在 hub 服务器上用 {command} 命令完成。",
    ),
    ("Pair", "配对"),
    ("Pair again", "重新配对"),
    ("Continue pairing", "继续配对"),
    ("See the pairing", "查看配对"),
    ("Disconnect", "断开连接"),
    ("Restart", "重启"),
    ("Decide", "处理"),
    ("Number {alias}: {state}.", "号码 {alias}：{state}。"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "hub 已更改号码 {alias}，但 WAHA 没有响应：它在 WAHA 中的会话可能仍是原样。请在 hub 服务器上用 {command} 检查 WAHA。",
    ),
    ("Method", "方式"),
    ("Phone", "手机"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "面板和 Hermes 用来称呼此号码的名称：小写字母、数字、连字符和下划线，最多 32 个字符。别名绝不会被重复使用。",
    ),
    ("Number with the country code", "带国家代码的号码"),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "例如 +55 11 9XXXX-XXXX。只能包含数字、空格、连字符和括号。",
    ),
    ("Before the first number", "添加第一个号码之前"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "引擎是非官方的 WhatsApp 客户端。WhatsApp 可能会封禁该号码。请使用您可以承受损失的号码。",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "我已了解并接受。hub 会在审计中记录这次接受。",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "号码通过 WAHA 配对，WAHA 是此 hub 上设置的引擎。",
    ),
    ("Add and pair", "添加并配对"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA 还没有响应过 hub。您可以添加号码；配对则需要 WAHA 有响应。",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "此 hub 上还没有 WhatsApp 引擎。请在 hub 服务器上用 {command} 设置 WAHA，再用 {key_command} 设置它的密钥。密钥绝不会在面板中输入。",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "引擎还没有密钥。请在 hub 服务器上用 {command} 设置。密钥绝不会在面板中输入。",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "别名长度为 1 到 32 个字符。",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "别名以小写字母或数字开头。",
    ),
    (
        "That alias is reserved. Choose another.",
        "该别名是保留的。请选择另一个。",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "该别名正在使用，或曾被已移除的号码使用，而别名绝不会被重复使用。请选择另一个。",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "请输入带国家代码的号码：8 到 15 位数字，例如 +55 11 9XXXX-XXXX。",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "该号码已经是此 hub 的一个 WhatsApp。",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "请阅读有关风险的说明，并勾选表示您接受。没有添加任何内容。",
    ),
    ("Pair {alias}", "配对 {alias}"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "请准备好 {phone} 的手机，并打开 WhatsApp。配对最长持续 {minutes} 分钟。",
    ),
    ("How will the phone pair?", "手机要如何配对？"),
    ("QR code", "二维码"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "在 WhatsApp 里用手机摄像头扫描",
    ),
    ("Pairing code", "配对码"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "在手机上输入一个代码，适用于手机无法扫描此屏幕的情况",
    ),
    ("Start pairing", "开始配对"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA 还没有创建此号码的会话。开始配对会再试一次。",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "请选择二维码或配对码。没有开始任何操作。",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "现在有 {n} 个号码正在配对，已达同时配对的上限。请等待其中一个结束，或取消一个。",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "号码处于“{state}”状态时无法执行此操作。没有执行任何操作。",
    ),
    ("Starting the pairing…", "正在开始配对…"),
    ("QR code to scan with the phone", "用手机扫描的二维码"),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "二维码每隔几秒就会变化：此窗口显示的是当前的二维码。",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "此窗口倒计时期间代码有效：剩余 {time}。WhatsApp 也可能提前让它失效。",
    ),
    ("Time left to scan: {time}.", "扫描剩余时间：{time}。"),
    ("This window refreshes by itself.", "此窗口会自动刷新。"),
    ("Refresh now", "立即刷新"),
    ("Cancel the pairing", "取消配对"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "二维码太长，无法在此绘制。请取消，改用配对码进行配对。",
    ),
    ("Paired: {phone}.", "已配对：{phone}。"),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "号码 {alias} 已连接。请在“访问权限”标签页中让某个 Hermes 访问它。",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "在规定时间内没有扫描或输入任何内容。号码未配对；请准备好手机后重新配对。",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA 报告会话失败。号码未配对。请重试；如果再次失败，请检查 WAHA。",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "配对已取消。号码未配对。",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "此号码正在另一个窗口中配对。只有那个窗口会显示二维码或代码。",
    ),
    ("Cancel that pairing", "取消那次配对"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "此配对已不在 hub 中运行：它已结束，或 hub 已重启。号码处于“{state}”状态。",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "hub 无法连接到 WAHA。请检查它是否正在运行，并且位于 hub 的网络中。",
    ),
    (
        "WAHA refused the key the hub has. Set the right key on the hub's server with {command}.",
        "WAHA 拒绝了 hub 持有的密钥。请在 hub 服务器上用 {command} 设置正确的密钥。",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA 返回了错误，或没有及时响应。请稍后重试；在 hub 服务器上运行 {command} 可以了解更多。",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp 正在限制此号码。请稍后重试。",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA 拒绝了该请求。请查看 hub 的日志。",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "hub 出现故障。请查看 hub 的日志，并在列表中检查该号码。",
    ),
    (
        "{alias}: another phone paired",
        "{alias}：另一部手机完成了配对",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "完成配对的手机属于与所填号码不同的号码。",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "在您处理之前，号码保持“号码不符”状态，其访问权限保持停止。",
    ),
    ("Keep this one", "保留此项"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "撤销会让 WAHA 退出该手机的 WhatsApp（它会从已关联的设备中消失），并清除 WAHA 保存的此号码的历史记录。",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "我了解 WAHA 会清除它保存的此号码的历史记录。",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "请勾选表示您了解撤销会清除什么。没有执行任何操作。",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "完成配对的手机已经是此 hub 的另一个号码。请撤销这次配对，或先移除另一个号码。",
    ),
    ("Disconnect {alias}", "断开 {alias}"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "断开连接会停止 WAHA 中此号码的会话。这不会让 WAHA 退出 WhatsApp，WAHA 仍会保留历史记录。断开期间，没有任何 Hermes 能通过它读取或发送。若要再次使用，请重新配对。",
    ),
    (
        "Type the alias {alias} to confirm.",
        "请输入别名 {alias} 以确认。",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "输入的别名不匹配。没有执行任何操作。",
    ),
    ("Remove {alias}", "移除 {alias}"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "移除会立即结束对此号码的所有访问。它的别名绝不会再被使用。",
    ),
    (
        "What happens to its session in WAHA",
        "它在 WAHA 中的会话将如何处理",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "停止它并保留在 WAHA 中，连同 WAHA 保存的历史记录",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "让 WAHA 退出 WhatsApp 并删除它：WAHA 会清除它保存的历史记录",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "Arauto 还没有消息存档：历史记录只有 WAHA 保存的那部分。",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "请选择它在 WAHA 中的会话将如何处理。没有执行任何操作。",
    ),
    ("In test", "测试中"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "正在研究内置于 Arauto 的 WhatsApp 引擎。它不属于此版本，此窗口不执行任何操作。",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "目前，号码通过 WAHA 配对，使用“连接 WhatsApp (WAHA)”按钮。",
    ),
    ("Open the WAHA window", "打开 WAHA 窗口"),
    ("Try again", "重试"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "用手机扫描：WhatsApp › 已关联的设备 › 关联设备。",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "在手机上：WhatsApp › 已关联的设备 › 关联设备 › 改为用手机号码关联，然后输入此代码：",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "有此标记时，通过该链接发出的便条会在通知限额内唤醒 {name}。",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} 遵循 {from} 的指示",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "有此标记时，便条的通知会告诉 {to}：在那一次运行中，按 {from} 的要求去做，无需等待其操作者，除非其操作者禁止。仅限指示箭头，且仅限有唤醒标记；{to} 需要返回的链接才能回报。这是写给 AI 的一句话，不是控制手段。",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "链接 {from} → {to}：{to} 现在遵循 {from} 的指示",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "链接 {from} → {to}：{to} 不再遵循 {from} 的指示",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "遵循另一个 Hermes 的 Hermes，会不等其操作者就去做对方要求的事。误导上层 Hermes 的人（通过 WhatsApp 或便条）也能触及下层 Hermes；控制 hub 的人可以冒充任何 Hermes 写入；持有面板会话的人可以设置这些标记。旧版本的连接器仍会把这些便条当作数据。",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "遵循标记只能从较高层级指向较低层级，且只能设在会唤醒的链接上。",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "谁向它下达指令"),
    ("not recorded", "未记录"),
    ("its own chat", "它自己的聊天"),
    ("{n} telephones", "{n} 个电话"),
    ("1 telephone", "1 个电话"),
    ("Orders", "指令"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "谁在 WhatsApp 上向 {hermes} 下达指令",
    ),
    (
        "The telephone {hermes}'s WhatsApp runs on",
        "{hermes} 的 WhatsApp 所运行的电话",
    ),
    (
        "{hermes} takes orders here. Empty if it has no WhatsApp.",
        "{hermes} 在这里接收指令。若它没有 WhatsApp，请留空。",
    ),
    (
        "This is the number {alias} in Arauto.",
        "这是 Arauto 中的号码 {alias}。",
    ),
    ("Orders come from", "指令来自"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "它自己的 WhatsApp，在与自己的聊天中（self-chat 模式）",
    ),
    ("These telephones, one per line", "这些电话，每行一个"),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "Arauto 拒绝任何 Hermes（包括 {hermes}）在 {hermes} 接收这些指令的聊天中写入。读取则遵循访问权限。",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "如果这里没有记录任何内容，Arauto 不会拦截任何东西：能在 {hermes} 接收指令的聊天中写入的 Hermes，可能冒充你，或冒充下达指令的人。",
    ),
    (
        "Type the telephone {hermes}'s WhatsApp runs on first.",
        "请先输入 {hermes} 的 WhatsApp 所运行的电话。",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} 已在这部电话上接收指令。",
    ),
    ("At most 16 telephones.", "最多 16 个电话。"),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "hub 中出现故障。请查看 hub 的日志，并核对该 Hermes 的记录。",
    ),
    (
        "A Hermes could give {hermes} orders",
        "某个 Hermes 可能向 {hermes} 下达指令",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "没有任何拦截：任何能在 {hermes} 接收指令的聊天中写入的 Hermes，都可以冒充下达指令的人。",
    ),
    (
        "Arauto will let every Hermes write again in the chat between each removed telephone and {hermes}'s WhatsApp, and pass there for the person who gives {hermes} orders.",
        "Arauto 将再次允许任何 Hermes 在每个被移除的电话与 {hermes} 的 WhatsApp 之间的聊天中写入，并在那里冒充向 {hermes} 下达指令的人。",
    ),
    ("Save anyway", "仍然保存"),
    ("Go back", "返回"),
    ("gives orders to {hermes}", "向 {hermes} 下达指令"),
    ("{hermes} takes orders here", "{hermes} 在这里接收指令"),
    ("Orders on WhatsApp for {alias}", "{alias} 的 WhatsApp 指令"),
    (
        "Choose the Hermes. Nothing is saved until you save in its window.",
        "选择 Hermes。在其窗口中保存之前，不会保存任何内容。",
    ),
];
