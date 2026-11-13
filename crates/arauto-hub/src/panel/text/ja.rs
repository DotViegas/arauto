//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "パネル"),
    ("Language", "言語"),
    ("Change", "変更"),
    ("R", "読"),
    ("S", "送"),
    ("M", "変"),
    ("W", "起"),
    ("Waiting for the device…", "デバイスを待っています…"),
    (
        "The passkey was not accepted. Try again.",
        "パスキーは受け付けられませんでした。もう一度お試しください。",
    ),
    (
        "This browser cannot use a passkey here. Use a current browser, at the address of the panel.",
        "このブラウザーでは、ここでパスキーを使えません。パネルのアドレスで、新しいブラウザーをお使いください。",
    ),
    ("Sign in to the panel", "パネルにログイン"),
    (
        "Use the passkey you registered: on this device, or in your password manager.",
        "登録済みのパスキーを使ってください。このデバイス、またはパスワードマネージャーにあります。",
    ),
    ("Sign in with a passkey", "パスキーでログイン"),
    ("Installation code", "インストールコード"),
    (
        "First access, or a lost passkey: type the code the hub wrote in its log when it started, or issue one on the hub's server with {command}. The code only opens the registration of a passkey.",
        "初回アクセス、またはパスキーを紛失した場合: ハブが起動時にログに書き出したコードを入力するか、ハブのサーバーで {command} を実行して発行してください。このコードで開けるのはパスキーの登録だけです。",
    ),
    ("Continue", "続行"),
    ("Register a passkey", "パスキーを登録"),
    (
        "A passkey stays on this device or in your password manager, and opens with the fingerprint, the face or the PIN of the device. The hub keeps only what checks it: there is no password to leak.",
        "パスキーはこのデバイスまたはパスワードマネージャーに保存され、デバイスの指紋、顔、または PIN で開きます。ハブが保持するのはパスキーを検証するための情報だけで、漏えいするパスワードはありません。",
    ),
    (
        "This session only reaches the registration of a passkey. The rest of the panel opens after it.",
        "このセッションで行えるのはパスキーの登録だけです。パネルの残りは登録後に開きます。",
    ),
    ("Sign out", "ログアウト"),
    ("Alerts", "アラート"),
    ("Unknown devices", "不明なデバイス"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "{n} 台が、データベースでは説明のつかない有効な資格情報を提示しました。復帰するには、それぞれ新しい招待が必要です。{command} で確認できます。",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "ほかのアラート: {n} 件。{command} ですべて確認できます。",
    ),
    ("Accesses", "アクセス権"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "このバージョンのパネルでは、Hermes の接続と失効、各番号のアクセス権の変更ができます。番号のペアリングとチャットの選択は、引き続きハブのサーバーで {command} コマンドを使って行います。",
    ),
    ("none", "なし"),
    ("Your Hermes and their [numbers]", "Hermes とその[番号]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "接続済み: {connected} · 待機中: {waiting} · 警告中: {stopped}",
    ),
    ("Connect Hermes", "Hermes を接続"),
    ("State", "状態"),
    ("Numbers", "番号"),
    ("Can be woken", "起動通知の可否"),
    ("Connector", "コネクター"),
    ("Seen", "最終確認"),
    ("Actions", "操作"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "Hermes はまだありません。「Hermes を接続」ボタンで最初の Hermes を接続してください。",
    ),
    ("yes", "可"),
    ("no", "不可"),
    ("Connect", "接続"),
    ("Reconnect", "再接続"),
    ("Revoke", "失効させる"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "「番号」は、その Hermes がアクセスできる番号です。コネクターの欄はデバイスが申告した内容で、検証されていません。",
    ),
    ("Engine", "エンジン"),
    (
        "There is no engine yet. Set it with {command}.",
        "エンジンはまだありません。{command} で設定してください。",
    ),
    ("Alias", "別名"),
    ("Number", "番号"),
    ("Hermes with access", "アクセス権のある Hermes"),
    ("Last message", "最後のメッセージ"),
    (
        "No number yet. Add one with {command}.",
        "番号はまだありません。{command} で追加してください。",
    ),
    ("command", "コマンド"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "最後のメッセージは時刻だけを表示し、内容は表示しません。パネルに表示されるのはハブを通るものだけです。同じ番号にリンクされたほかのデバイスは、スマートフォンにしか表示されません。",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "行が Hermes、列が番号です。セルをクリックすると変更できます。",
    ),
    (
        "Connect a Hermes first.",
        "先に Hermes を接続してください。",
    ),
    (
        "Add a WhatsApp first.",
        "先に WhatsApp を追加してください。",
    ),
    ("(accesses kept)", "(アクセス権は保持)"),
    ("no access", "アクセス権なし"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} 読み取り · {s} 送信 · {m} 変更 · {w} 起動通知。",
    ),
    ("Saved: {what}", "保存しました: {what}"),
    ("Warnings that block nothing", "動作を妨げない警告"),
    // guide.rs
    ("Name", "名前"),
    ("Block", "ブロック"),
    ("Code", "コード"),
    ("Done", "完了"),
    ("Invite issued", "招待を発行しました"),
    ("A device used the invite", "デバイスが招待を使用しました"),
    (
        "Code checked and request approved",
        "コードを確認し、リクエストを承認しました",
    ),
    (
        "The connector confirmed its first credential",
        "コネクターが最初の資格情報を確認しました",
    ),
    (
        "A device used the invite, from {address}",
        "デバイスが {address} から招待を使用しました",
    ),
    ("waiting…", "待機中…"),
    (
        "What name will this Hermes have in the panel?",
        "この Hermes をパネルで何という名前にしますか?",
    ),
    ("Name of the Hermes", "Hermes の名前"),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "英小文字、数字、ハイフン、アンダースコアが使えます。32 文字以内で、先頭は英字または数字にしてください。名前はブロックに表示され、あとから変更できません。",
    ),
    (
        "Preparing a secure connection",
        "安全な接続を準備しています",
    ),
    (
        "The hub drew the invite of {name}.",
        "ハブが {name} の招待を生成しました。",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "{name} の招待 · 有効期間は残り {time} · 1 回限り",
    ),
    ("Copied", "コピーしました"),
    (
        "Click the block to copy it.",
        "ブロックをクリックするとコピーできます。",
    ),
    (
        "Select the whole text and copy it.",
        "テキスト全体を選択してコピーしてください。",
    ),
    (
        "It only appears here, once.",
        "ここに 1 回だけ表示されます。",
    ),
    (
        "The invite issued before was cancelled.",
        "以前に発行した招待はキャンセルされました。",
    ),
    (
        "The request that was in progress was closed.",
        "進行中だったリクエストは終了しました。",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "使用中のデバイスは、新しいデバイスが最初の資格情報を確認するまで動作し続けます。",
    ),
    (
        "This hub has no connector to serve",
        "このハブには配布できるコネクターがありません",
    ),
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "ブロックは Hermes にこのハブからコネクターをダウンロードするよう指示しますが、ダウンロード用フォルダーが空です。何も発行されていません。{name} の招待は、ハブのサーバーで {command} コマンドを使って発行してください。",
    ),
    (
        "Waiting for the Hermes {name}",
        "Hermes {name} を待っています",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "各ステップは、ハブが実際に確認した時点で点灯します。Hermes がコマンドの実行に承認を求めた場合は、あなたがコマンドチャネルで承認します。",
    ),
    (
        "A device used the invite of {name}",
        "デバイスが {name} の招待を使用しました",
    ),
    ("unknown", "不明"),
    ("Seen by the hub", "ハブが確認した情報"),
    ("address {address}, {when}", "アドレス {address}、{when}"),
    ("Said by the device", "デバイスが申告した情報"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "マシン「{host}」 · システム「{os}」 · Hermes「{version}」(未検証)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "現在のデバイスの最終使用: {when}、{address} から。心当たりはありますか?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "現在のデバイスの最終使用: {when}。心当たりはありますか?",
    ),
    ("I recognize it", "心当たりがあります"),
    ("I do not recognize it", "心当たりがありません"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "Hermes が停止していたのに使用があった場合は、誰かがコピーを使用しています。",
    ),
    ("Verification code", "確認コード"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "Hermes がコマンドチャネルで返答したコードを入力してください。パネルがこのコードを表示することはありません。",
    ),
    (
        "The request of {name} was denied",
        "{name} のリクエストは拒否されました",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "コードの入力を 5 回間違えました。接続するには、別の招待を発行してください。",
    ),
    (
        "The request of {name} expired",
        "{name} のリクエストは期限切れになりました",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "コードを入力できる 10 分が過ぎました。接続するには、別の招待を発行してください。",
    ),
    (
        "The identity of {name} was copied",
        "{name} の ID がコピーされました",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "承認の前に同じ ID を持つデバイスが 2 台現れたため、リクエストを停止しました。接続するには、別の招待を発行してください。",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} には待機中の招待もリクエストもありません",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "招待が期限切れになったか、キャンセルされたか、リクエストがすでに処理されています。Hermes がコードを返答していない場合は、承認しないでください。AR-102 と返答した場合は、別のデバイスが招待を使用しています。接続するには、別の招待を発行してください。",
    ),
    ("{name} is connected", "{name} は接続済みです"),
    ("{name} was approved", "{name} を承認しました"),
    (
        "What it may do on each number is in the Accesses tab.",
        "各番号で何ができるかは「アクセス権」タブで確認できます。",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "コネクターが確認するとすぐに、接続済みと表示されます。以前のアクセス権は、このデバイスにも引き続き適用されます。",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "コネクターが確認するとすぐに、接続済みと表示されます。まだどの番号にもアクセス権がありません。",
    ),
    (
        "The hub did not do what was asked",
        "ハブは要求された操作を実行しませんでした",
    ),
    ("Issue invite", "招待を発行"),
    ("Cancel", "キャンセル"),
    ("Next step", "次のステップ"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "コピーしたら、Hermes {name} のコマンドチャネルに貼り付けてください。ほかの場所には貼り付けないでください。",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "招待の有効期間は残り {time} です。この画面は自動的に更新されます。",
    ),
    ("Approve", "承認"),
    ("Deny", "拒否"),
    ("Confirm within {time}.", "{time} 以内に確認してください。"),
    ("Issue another invite", "別の招待を発行"),
    ("Close", "閉じる"),
    ("See accesses", "アクセス権を表示"),
    ("Finish", "完了する"),
    (
        "Copied to the clipboard.",
        "クリップボードにコピーしました。",
    ),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "テキストを選択しました。Ctrl+C でコピーしてください。",
    ),
    ("Reconnect {name}", "{name} を再接続"),
    ("Connect {name}", "{name} を接続"),
    ("Steps", "ステップ"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "起動通知には読み取りが必要です。何も保存されていません。",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "チャットの開始には、送信と適用範囲「すべてのチャット」が必要です。何も保存されていません。",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "起動通知までの待機時間は 0 秒から 60 秒までです。何も保存されていません。",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "ハブでエラーが発生しました。ハブのログを確認し、表でアクセス権を確かめてください。",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "データベースが古いバックアップから復元されましたが、ハブはまだ対応を済ませていません。何も書き込まれていません。ハブを再起動してください。",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "Hermes {hermes} には番号 {number} へのアクセス権がありません。",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Hermes {hermes} は、番号 {number} の選択したチャットにだけアクセスできます。行のあるチャット: {lines}。",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Hermes {hermes}、番号 {number}、すべてのチャット:",
    ),
    ("reads", "読み取り可"),
    ("does not read", "読み取り不可"),
    ("sends", "送信可"),
    ("does not send", "送信不可"),
    ("modifies", "変更可"),
    ("does not modify", "変更不可"),
    (
        "is woken after {seconds} s of silence",
        "メッセージが {seconds} 秒途絶えたら起動通知",
    ),
    ("is not woken", "起動通知なし"),
    ("may start chats", "チャット開始可"),
    (
        "chats with their own rule: {lines}",
        "個別ルールのあるチャット: {lines}",
    ),
    ("{hermes} on the number {number}", "{hermes}、番号 {number}"),
    ("Where it applies", "適用範囲"),
    (
        "Every chat, including the ones that come later",
        "すべてのチャット(今後追加されるものを含む)",
    ),
    ("Only chosen chats", "選択したチャットのみ"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "「選択したチャットのみ」では、個別の行があるチャットだけが Hermes から見え、各チャットで何ができるかはその行で決まります。行は引き続き {command} コマンドで作成します。現在: {lines}。",
    ),
    ("What it may do", "許可する操作"),
    ("Read", "読み取り"),
    (
        "see chats and messages, search, download files",
        "チャットとメッセージの閲覧、検索、ファイルのダウンロード",
    ),
    ("Send", "送信"),
    (
        "write, react, mark as read",
        "メッセージの作成、リアクション、既読にする",
    ),
    ("Modify", "変更"),
    (
        "edit and delete what already exists",
        "既存のものの編集と削除",
    ),
    ("Be woken", "起動通知"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "メッセージが届くとハブが通知します(読み取りが必要)",
    ),
    ("Wait", "待機時間"),
    (
        "seconds of silence before waking",
        "起動通知までにメッセージが途絶える秒数",
    ),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "この番号と一度もやり取りのない相手とチャットを開始できる(送信と「すべてのチャット」が必要)",
    ),
    ("Today:", "現在:"),
    ("Save", "保存"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "次の呼び出しから適用され、再起動は不要です。操作を 1 つも選ばない場合、アクセス権は削除されます。",
    ),
    ("Revoke {name}", "{name} を失効させる"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "失効させると、ハブ上でこの Hermes が閉鎖されます。対象は、使用中のデバイス、進行中のリクエスト、発行済みの招待です。チャネルは数秒以内に切断されます。",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "アクセス権は保持され、新しいデバイスを接続すると再び適用されます。復帰する方法は新しい招待だけです。",
    ),
    // data.rs
    ("never", "なし"),
    ("now", "たった今"),
    ("{n} min ago", "{n} 分前"),
    ("{n} h ago", "{n} 時間前"),
    ("{n} d ago", "{n} 日前"),
    (
        "a credential that had already been swapped was presented",
        "交換済みの資格情報が提示されました",
    ),
    (
        "two connections at the same time",
        "同時に 2 つの接続がありました",
    ),
    (
        "the credential could not be read",
        "資格情報を読み取れませんでした",
    ),
    (
        "the last use was not recognized on the reconnect",
        "再接続の際、最終使用に心当たりがないと回答されました",
    ),
    (
        "the identity was copied before the approval",
        "承認の前に ID がコピーされました",
    ),
    (
        "the hub no longer trusts the credential",
        "ハブはこの資格情報を信頼しなくなりました",
    ),
    ("Waiting for your code", "コードの入力待ち"),
    (
        "Approved, waiting for the connector",
        "承認済み、コネクターを待機中",
    ),
    ("Waiting for the Hermes", "Hermes を待機中"),
    ("Connected", "接続済み"),
    ("Disconnected", "切断"),
    ("In quarantine", "隔離中"),
    ("Suspended", "一時停止中"),
    ("No device", "デバイスなし"),
    ("a Hermes", "ある Hermes"),
    ("Invite", "招待"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "別のデバイスが {hermes} の招待を使おうとしました。ブロックがほかの場所に渡っています。最初のリクエストは引き続き有効です。",
    ),
    ("Identity copied", "ID のコピー"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "承認の前に {hermes} の ID がコピーされました。リクエストは拒否されました。",
    ),
    ("Hermes in quarantine", "隔離中の Hermes"),
    ("{hermes}. Reason: {reason}.", "{hermes}。理由: {reason}。"),
    ("Suspected copy", "コピーの疑い"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "再接続の際、{hermes} の最終使用に心当たりがないと回答されました。",
    ),
    ("Connection taken", "接続の奪取"),
    (
        "the connection of {hermes} was taken by another one.",
        "{hermes} の接続が別の接続に奪われました。",
    ),
    ("Closing", "閉鎖"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} が自身の閉鎖を要求しました。",
    ),
    ("Reuse", "再使用"),
    (
        "a copy of {hermes} was used before the closing.",
        "閉鎖の前に {hermes} のコピーが使用されました。",
    ),
    ("Unconfirmed swaps", "未確認の交換"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} は資格情報を受け取ったまま確認しないことが、24 時間に 3 回ありました。ネットワークの不調か、誰かがコピーを試している可能性があります。",
    ),
    ("Audit mark", "監査マーク"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "セキュリティイベントが記録されましたが、データベース外のマークが追従していません。",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "エンジンのバージョンは、ハブが確認済みのものではありません。",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "エンジンセッションの設定が異なっていたため、ハブが自身の設定を適用しました。",
    ),
    ("Number swapped", "番号の相違"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "ペアリングしたスマートフォンは、指定した番号とは別の番号のものです。あなたが判断するまで、アクセス権は停止されたままです。",
    ),
    ("Address: {address}.", "アドレス: {address}。"),
    ("an unknown date", "不明な日付"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "ハブは {date} のバックアップから復元されました。それ以降の承認、失効、再接続、アクセス権の変更は失われています。各 Hermes を再接続し、アクセス権を確認してください。",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "ハブはバックアップから復元されました。{name} は一時停止中で、新しい招待でのみ復帰できます。再接続するか、失効させてください。「再有効化」はありません。",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "ハブは {name} の資格情報を信頼しなくなりました。理由: {reason}。アクセスは停止されています。対処方法は再接続または失効です。「解除」はありません。",
    ),
    ("Not paired", "未ペアリング"),
    ("Pairing", "ペアリング中"),
    ("Reconnecting", "再接続中"),
    ("Failed", "失敗"),
    ("Removed", "削除済み"),
    (
        "version {version}, checked",
        "バージョン {version}、確認済み",
    ),
    (
        "version {version}, not checked by the hub",
        "バージョン {version}、ハブは未確認",
    ),
    ("version not read yet", "バージョンは未取得"),
    ("last answer {when}", "最終応答: {when}"),
    ("has not answered yet", "まだ応答なし"),
    ("all", "すべて"),
    ("all · own rules: {n}", "すべて · 個別ルール: {n}"),
    ("chosen chats: {n}", "選択したチャット: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: {n} 個の Hermes が同じチャットで起動通知を受けます。",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: {n} 個の Hermes が同じチャットで送信します。",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} はコマンド用の番号に指定されていますが、この番号で送信できる Hermes があります。",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "その名前の Hermes はすでに存在します。別のデバイスをその Hermes に結び付けるには、その行のボタンを使ってください。",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "その名前は削除済みの Hermes が使っていました。名前は再利用されません。別の名前を選んでください。",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "コードを入力する前に、現在のデバイスの最終使用に心当たりがあるかどうかを回答してください。試行回数には数えていません。",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "回答している間に現在のデバイスが使用されました。最新の最終使用を確認して、もう一度回答してください。何も承認されていません。",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "コードは 123-456 のような 6 桁です。試行回数には数えていません。",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "データベースが古いバックアップから復元されましたが、ハブはまだ対応を済ませていません。何も書き込まれていません。ハブを再起動してください。復元モードが有効になり、その後、各 Hermes を新しい招待で再接続します。",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "ハブでエラーが発生し、何も実行されませんでした。ハブのログを確認してください。",
    ),
    (
        "The name has from 1 to 32 characters.",
        "名前は 1 文字以上 32 文字以内です。",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "名前の先頭は英小文字または数字です。",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "英小文字、数字、ハイフン、アンダースコアだけを使ってください。",
    ),
    (
        "That name is reserved. Choose another.",
        "その名前は予約されています。別の名前を選んでください。",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "コードが一致しません。試行 {attempts} 回目(最大 {max} 回)。Hermes が最後に返答したコードを入力してください。",
    ),
    // mod.rs
    (
        "Invalid or expired code. Issue another one on the hub's server.",
        "コードが無効か、期限切れです。ハブのサーバーで別のコードを発行してください。",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "試行回数が多すぎます。1 分待ってから、もう一度お試しください。",
    ),
];
