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
    ("Sign in to the panel", "登录面板"),
    ("Entry code", "登录码"),
    (
        "Issue the code on the hub's server, with {command}. It is valid for 10 minutes and for one sign-in.",
        "在 hub 的服务器上用 {command} 签发登录码。登录码 10 分钟内有效，只能登录一次。",
    ),
    ("Sign in", "登录"),
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
    ("Continue", "继续"),
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
    // guide.rs
    ("Name", "名称"),
    ("Block", "文本块"),
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
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "小写字母、数字、连字符和下划线，最多 32 个字符，以字母或数字开头。名称会出现在文本块中，之后不能更改。",
    ),
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
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "文本块会让 Hermes 从此 hub 下载连接器，但下载文件夹是空的。未签发任何内容。请在 hub 的服务器上用 {command} 命令签发 {name} 的邀请。",
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
        "Invalid or expired code. Issue another one on the hub's server.",
        "登录码无效或已过期。请在 hub 的服务器上签发新的登录码。",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "当前尝试次数过多。请等待一分钟后重试。",
    ),
];
