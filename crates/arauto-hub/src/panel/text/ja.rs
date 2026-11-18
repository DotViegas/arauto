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
    ("Sign in with a passkey", "パスキーでログイン"),
    ("Installation code", "インストールコード"),
    ("Continue", "続行"),
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
    ("none", "なし"),
    ("Your Hermes and their [numbers]", "Hermes とその[番号]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "接続済み: {connected} · 待機中: {waiting} · 警告中: {stopped}",
    ),
    ("Connect Hermes", "Hermes を接続"),
    ("State", "状態"),
    ("Numbers", "番号"),
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
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "そのコードは使えませんでした。最新のコードを使ってください。ログの最後のブロックにあるコード、またはハブのサーバーで発行したコードです。",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "ハブはこのパスキーを受け付けませんでした。もう一度お試しください。",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "パネルのセッションが終了しました。最新のコードを入力すると、続きから再開できます。",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "このブラウザーでは、ここでパスキーを使えません。最新のブラウザーで、パネルのアドレスを直接開いてください（ほかのアプリの中ではなく）。",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "パスキーを作成できませんでした。操作がキャンセルされたか、時間がかかりすぎました。もう一度お試しください。",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "パスキーが使われませんでした。このデバイスにない場合は、作成したデバイスまたはパスワードマネージャーを使うか、「パスキーを紛失しましたか？」を開いてください。",
    ),
    ("Passkey", "パスキー"),
    ("Check", "チェック"),
    ("Welcome to your Arauto hub", "Arauto ハブへようこそ"),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "最初のアクセスを設定しましょう。手順は3つだけで、設定は一度きりです。",
    ),
    (
        "Step 1 · Confirm it is you",
        "ステップ 1 · ご本人であることを確認",
    ),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "ハブは起動時に、自分のログにインストールコードを書き出しました。このログを見られるのはサーバーを管理している人だけです。それでハブはご本人だとわかります。",
    ),
    ("Where to find the code:", "コードの見つけ方:"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "Easypanel では、ハブのサービスを開いて Logs に進みます。(Docker の場合: {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "ARAUTO FIRST ACCESS のブロックを探します。いちばん新しいブロックは最後、ログのいちばん下にあります。そのコードをコピーします。",
    ),
    ("Paste the code here:", "ここにコードを貼り付けてください:"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "コードに有効期限はなく、使えるのは一度だけです。ハブが再起動すると新しいコードが書き出されます。必ず最新のものを使ってください。",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "ログを開けませんか？ハブのサーバーで {command} を実行してください。ご注意: その新しいコードを発行すると、ログにあるコードは無効になります。",
    ),
    ("Good to have you back", "おかえりなさい"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "パスキーでログインしてください。このデバイスの指紋、顔、PIN、またはパスワードマネージャーを使います。",
    ),
    ("Lost your passkey?", "パスキーを紛失しましたか？"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "ご心配なく。ハブのサーバーで {command} を実行してください。10分間有効なコードが表示されるので、ここに入力して新しいパスキーを作成してください。",
    ),
    (
        "Code accepted! Now, create your passkey",
        "コードを受け付けました！次はパスキーを作成します",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "今後はこれでパネルにログインします。覚えておくパスワードはありません。",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "パスキーはこのデバイスまたはパスワードマネージャーに保存され、指紋、顔、またはデバイスの PIN で開きます。ハブが保持するのはパスキーを検証するための情報だけで、漏えいするパスワードはありません。",
    ),
    ("It will work at this address:", "このアドレスで使えます:"),
    ("Create my passkey", "パスキーを作成"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "パスワードマネージャーをお使いなら、パスキーはほかのデバイスにも引き継げます。",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "うっかりこのページを離れましたか？同じコードをもう一度入力してください。コードが使われるのは、パスキーを作成したときだけです。",
    ),
    ("Check the installation", "インストールの確認"),
    // guide.rs
    ("Name", "名前"),
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
    ("How it works:", "しくみ:"),
    (
        "You give the Hermes a name here.",
        "ここで Hermes に名前を付けます。",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Hermes がこのハブからコネクターをダウンロードし、6桁の確認コードを返します。",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "ここでコードを入力して承認します。これで完了です。コネクターが確認した時点で、Hermes は接続済みと表示されます。",
    ),
    (
        "Let's connect your first Hermes",
        "最初の Hermes を接続しましょう",
    ),
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
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "メッセージが届くとハブが通知します(読み取りが必要)。オフにすると、このアクセスの中で独自のルールを持つチャットでもオフになります。",
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
    (
        "the engine refused the key of the hub. Restart the engine: it reads its key only when it starts.",
        "エンジンがハブのキーを拒否しました。エンジンを再起動してください。エンジンはキーを起動時にしか読み込みません。",
    ),
    (
        "the engine refused the key of the hub. Set its key again with {command}.",
        "エンジンがハブのキーを拒否しました。{command} でエンジンのキーを設定し直してください。",
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
        "Too many attempts right now. Wait a minute and try again.",
        "試行回数が多すぎます。1 分待ってから、もう一度お試しください。",
    ),
    // check.rs
    ("ok", "問題なし"),
    ("check", "要確認"),
    ("attention", "注意"),
    ("later", "あとで"),
    ("failed", "失敗"),
    ("Panel address", "パネルのアドレス"),
    ("Connectors address", "コネクターのアドレス"),
    ("Your address", "お使いのアドレス"),
    ("Database password", "データベースのパスワード"),
    ("Data folder", "データフォルダー"),
    ("WhatsApp engine", "WhatsApp エンジン"),
    ("Your passkey works here.", "パスキーはここで使えます。"),
    (
        "Open it: it should say it is the address of the connectors.",
        "開いてみてください。コネクター用のアドレスだと表示されるはずです。",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "ハブにはあなたのアドレスが {address} と見えています。合っていますか？違う場合は、プロキシが実際のアドレスを渡していません。",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "ARAUTO_TRUSTED_PROXIES に含まれていない {address} からの接続に、転送されたアドレスが含まれていました。この接続元のアドレスを ARAUTO_TRUSTED_PROXIES に追加して、もう一度デプロイしてください。",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "ハブにはあなたのアドレスが {address} と見えています。これは内部ネットワークのアドレスで、プロキシはまだ信頼されていません。プロキシのアドレスを ARAUTO_TRUSTED_PROXIES に追加して、もう一度デプロイしてください。",
    ),
    (
        "Replaced by one nobody saw.",
        "誰も見ていないパスワードに置き換えました。",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "ハブは置き換えていません。この環境では、データベースのパスワードを環境側で設定しています。",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "置き換えに失敗しました。ハブのログを確認してから、{command} を実行してください。",
    ),
    (
        "Ready for your Hermes.",
        "Hermes を迎える準備ができています。",
    ),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "ダウンロード用のフォルダーが空です。パネルから Hermes を接続できません。ハブのイメージをビルドし直してください。",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "マスターキーとデータベースのパスワードが入っています。バックアップを取ってください。これがないと、ハブは暗号化して保存したデータを開けません。",
    ),
    ("Configured.", "設定済みです。"),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "これがなくても Hermes は接続できます。WhatsApp の番号には必要です。",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "ハブがいくつかの項目を自動で確認しました。ここでは何も変更されません。結果をお知らせするだけです。",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "ステップ 3 · インストールを確認しましょう",
    ),
    (
        "All set: your passkey was created!",
        "準備完了です。パスキーを作成しました！",
    ),
    (
        "The same by command: {command}",
        "コマンドでも同じ確認ができます: {command}",
    ),
    ("Connect my first Hermes", "最初の Hermes を接続する"),
    ("Do it later", "あとで行う"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "私自身がお願いしています。この Hermes を、私たちの Arauto ハブに「{name}」として接続してください。",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "{page} を読んで、その手順に従ってください。わからないことがあれば私に聞いてください。",
    ),
    ("Hub:", "ハブ:"),
    ("Hub key:", "ハブキー:"),
    (
        "Connector sha256 ({system}):",
        "コネクター sha256 ({system}):",
    ),
    (
        "Invite (one use, until {deadline} UTC):",
        "招待（1回限り、{deadline} UTC まで）:",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "完了したら、確認コードと、ページが求める行を私に送ってください。",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "新しいメッセージとして送ってください。Hermes からの通知への返信としては送らないでください。確認を求められたら、ご自身の言葉で答えてください。",
    ),
    ("Show the command instead", "代わりにコマンドを表示"),
    (
        "Send one or the other, not both: they carry the same invite.",
        "どちらか一方だけを送ってください。両方とも同じ招待を含んでいます。",
    ),
    (
        "Click the message to copy it.",
        "メッセージをクリックするとコピーできます。",
    ),
    ("Message", "メッセージ"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "ハブが招待を発行し、それを含むメッセージを作成します。このメッセージは、その Hermes のコマンドチャネルにだけ送ってください。ほかの場所には送らないでください。",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "英小文字、数字、ハイフン、アンダースコアが使えます。32 文字以内で、先頭は英字または数字にしてください。名前はメッセージに表示され、あとから変更できません。",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "メッセージは Hermes にこのハブからコネクターをダウンロードするよう指示しますが、ダウンロード用フォルダーが空です。何も発行されていません。{name} の招待は、ハブのサーバーで {command} コマンドを使って発行してください。",
    ),
    ("Org chart", "組織図"),
    ("Who talks to [whom]", "誰が[誰と]話すか"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "Hermes をドラッグするとレベルを変更できます。カードの紫色の点を別のカードまで引くと、リンクを作成できます。矢印またはカードをクリックして調整します。",
    ),
    ("View", "表示"),
    ("Map", "マップ"),
    ("List", "リスト"),
    ("+ Level", "+ レベル"),
    ("Undo", "元に戻す"),
    ("Publish", "公開"),
    ("Draft: 1 change", "下書き: 1 件の変更"),
    ("Draft: {n} changes", "下書き: {n} 件の変更"),
    (
        "From a higher level to a lower one: instruction",
        "上位レベルから下位レベルへ: 指示",
    ),
    (
        "From a lower level to a higher one: request",
        "下位レベルから上位レベルへ: 依頼",
    ),
    ("Same level: note", "同じレベル: メモ"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "レベルと役割はここに表示されるだけで、どの Hermes にも伝えられません。メモはデータです。ただし、従う設定のある指示の矢印は別で、その場合、通知は下位の Hermes に、上位の Hermes からのその新しいメッセージを、その実行の間、オペレーターの指示の下で従うよう伝えます。Hermes がほかの Hermes にメモを送れるのは、リンクがある場合だけです。",
    ),
    ("instruction", "指示"),
    ("request", "依頼"),
    ("note", "メモ"),
    ("Name of level {n}", "レベル {n} の名前"),
    ("Remove empty level", "空のレベルを削除"),
    ("Move up", "上へ移動"),
    ("Move down", "下へ移動"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "レベル名は、小文字の英字、数字、ハイフン、アンダースコアの 1〜32 文字で、英字または数字で始まり、予約語ではない必要があります。",
    ),
    (
        "Another level already has this name.",
        "ほかのレベルがすでにこの名前を使っています。",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "組織図のレベルは 1〜{max} 個です。",
    ),
    (
        "Drag to another Hermes to create a link",
        "ほかの Hermes までドラッグしてリンクを作成",
    ),
    ("{alias} · answers", "{alias} · 応答"),
    ("{alias} · reads only", "{alias} · 読み取りのみ"),
    ("no number", "番号なし"),
    ("not placed yet", "未配置"),
    (
        "{name}, level {level}, {state}",
        "{name}、レベル {level}、{state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "未配置: {names}。最下位のレベルに表示されています。そこに固定するには公開してください。",
    ),
    ("Kind: {kind}", "種類: {kind}"),
    ("Wakes {name}", "{name} に起動通知"),
    ("Create the way back", "戻りのリンクを作成"),
    ("Remove the way back", "戻りのリンクを削除"),
    ("Remove", "削除"),
    (
        "This link already exists.",
        "このリンクはすでに存在します。",
    ),
    ("Role of this Hermes", "この Hermes の役割"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "あなた自身が読むためのものです。このバージョンでは、どの Hermes にも表示されません。280 文字まで、1 行で入力してください。",
    ),
    ("Level", "レベル"),
    ("Move left", "左へ移動"),
    ("Move right", "右へ移動"),
    (
        "Change them in Accesses",
        "アクセス権の画面で変更してください",
    ),
    (
        "No access to any number.",
        "どの番号にもアクセス権がありません。",
    ),
    ("Link to", "リンク先"),
    ("Add link", "リンクを追加"),
    ("Links of this Hermes", "この Hermes のリンク"),
    ("Levels", "レベル"),
    ("Links", "リンク"),
    ("Role", "役割"),
    ("From", "送信元"),
    ("To", "送信先"),
    ("Kind", "種類"),
    ("Wakes", "起動通知"),
    ("nobody", "なし"),
    ("No link.", "リンクはありません。"),
    ("Publish these changes?", "これらの変更を公開しますか?"),
    (
        "Links are security rules. They only apply after you publish.",
        "リンクはセキュリティ上のルールです。公開した後にのみ有効になります。",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "リンクがあると、リクエストが番号をまたげるようになります。ある番号経由で一方の Hermes に届いた内容が、もう一方の Hermes へのメモになることがあり、その Hermes は自分の番号で動作します。",
    ),
    ("Back", "戻る"),
    (
        "New link: {from} → {to} ({kind})",
        "新しいリンク: {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "新しいリンク: {from} → {to} ({kind})、{to} に起動通知",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "リンク {from} → {to}: {to} への起動通知を開始",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "リンク {from} → {to}: {to} への起動通知を停止",
    ),
    ("Link removed: {from} → {to}", "リンクを削除: {from} → {to}"),
    (
        "{name} changes level: {before} → {after}",
        "{name} のレベルが変更されます: {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} をレベル {after} に配置します",
    ),
    ("Role of {name} changed", "{name} の役割を変更しました"),
    (
        "Places on the map changed: {n}",
        "マップ上の位置を変更: {n} 件",
    ),
    ("New level: {name}", "新しいレベル: {name}"),
    (
        "Level renamed: {before} → {after}",
        "レベル名を変更: {before} → {after}",
    ),
    ("Level removed: {name}", "レベルを削除: {name}"),
    ("Order of the levels: {names}", "レベルの順序: {names}"),
    ("Published.", "公開しました。"),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "編集中に、ほかの誰かが組織図を変更しました。別の公開、コマンドによるリンクの変更、または Hermes の追加か削除です。何も公開されていません。下書きは保持されています。変更内容を確認して、もう一度公開してください。",
    ),
    ("Nothing was published.", "何も公開されていません。"),
    (
        "Two levels have the same name.",
        "同じ名前のレベルが 2 つあります。",
    ),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "役割は 280 文字以内、1 行で、制御文字や方向文字を含めることはできません。",
    ),
    (
        "A place on the map is out of range.",
        "マップ上の位置が範囲外です。",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "下書きに、存在しない Hermes やレベルが含まれているか、Hermes が欠けています。",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "リンクは、異なる 2 つの Hermes を、それぞれの方向に 1 つずつ結びます。",
    ),
    (
        "The draft could not be read.",
        "下書きを読み取れませんでした。",
    ),
    (
        "The draft is too large to publish.",
        "下書きが大きすぎて公開できません。",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "ハブで問題が発生しました。何も公開されていません。ハブのログを確認してください。",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "組織図を編集するには、パネルのスクリプトが必要です。スクリプトがない場合は、公開されている状態の組織図が表示されます。",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "Hermes はまだありません。最初の Hermes は Hermes タブで接続してください。",
    ),
    ("Connect WhatsApp (WAHA)", "WhatsApp を接続 (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "WhatsApp を接続 (ネイティブ · テスト中)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "番号はまだありません。最初の番号は「WhatsApp を接続 (WAHA)」ボタンで追加してください。",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "このバージョンでは、パネルから Hermes の接続と失効、WAHA 上の WhatsApp 番号のペアリングと管理、各番号のアクセス権の変更、リンク付きの組織図の編集ができます。チャットの選択は、引き続きハブのサーバーで {command} コマンドを使って行います。",
    ),
    ("Pair", "ペアリング"),
    ("Pair again", "再ペアリング"),
    ("Continue pairing", "ペアリングを続ける"),
    ("See the pairing", "ペアリングを表示"),
    ("Disconnect", "切断"),
    ("Restart", "再起動"),
    ("Decide", "対応を選ぶ"),
    ("Number {alias}: {state}.", "番号 {alias}: {state}。"),
    (
        "Number {alias} was removed.",
        "番号 {alias} は削除されました。",
    ),
    ("Telephone given", "指定された電話"),
    ("Telephone that paired", "ペアリングした電話"),
    ("Undo…", "元に戻す…"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "ハブは番号 {alias} を変更しましたが、WAHA から応答がありませんでした。WAHA 側のセッションは元のままかもしれません。ハブのサーバーで {command} を実行して WAHA を確認してください。",
    ),
    ("Method", "方法"),
    ("Phone", "電話"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "パネルと Hermes がこの番号を呼ぶ名前です。英小文字、数字、ハイフン、アンダースコアで最大 32 文字。別名は再利用されません。",
    ),
    ("Number with the country code", "国番号付きの番号"),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "例: +55 11 9XXXX-XXXX。数字、スペース、ハイフン、かっこのみ使えます。",
    ),
    ("Before the first number", "最初の番号の前に"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "エンジンは WhatsApp の非公式クライアントです。WhatsApp が番号をブロックすることがあります。失っても困らない番号を使ってください。",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "理解し、同意します。ハブはこの同意を監査に記録します。",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "番号は、このハブに設定されたエンジンである WAHA でペアリングします。",
    ),
    ("Add and pair", "追加してペアリング"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA はまだハブに応答していません。番号は追加できますが、ペアリングには WAHA の応答が必要です。",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "このハブにはまだ WhatsApp エンジンがありません。ハブのサーバーで {command} を使って WAHA を設定し、続けて {key_command} でそのキーを設定してください。キーをパネルに入力することはありません。",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "エンジンにはまだキーがありません。ハブのサーバーで {command} を使って設定してください。キーをパネルに入力することはありません。",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "別名は 1 文字から 32 文字です。",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "別名は英小文字または数字で始めてください。",
    ),
    (
        "That alias is reserved. Choose another.",
        "その別名は予約されています。別の別名を選んでください。",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "その別名は、現在使用中か、削除された番号が使っていたもので、別名は再利用されません。別の別名を選んでください。",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "国番号付きで番号を入力してください。8 桁から 15 桁で、例: +55 11 9XXXX-XXXX。",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "その番号はすでにこのハブの WhatsApp です。",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "リスクに関する文を読み、同意にチェックを入れてください。何も追加されていません。",
    ),
    ("Pair {alias}", "{alias} をペアリング"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "{phone} の電話を手元に用意し、WhatsApp を開いてください。ペアリングは最大 {minutes} 分間です。",
    ),
    (
        "How will the phone pair?",
        "電話をどの方法でペアリングしますか?",
    ),
    ("QR code", "QR コード"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "電話のカメラで、WhatsApp の中からスキャンします",
    ),
    ("Pairing code", "ペアリングコード"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "電話にコードを入力します。この画面をスキャンできない場合に使います",
    ),
    ("Start pairing", "ペアリングを開始"),
    (
        "{alias} is in the list now. Pair it now, or later from its line.",
        "{alias} は一覧に追加されました。今ペアリングするか、後でその行からペアリングできます。",
    ),
    ("Pair later", "後でペアリング"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA はまだこの番号のセッションを作成していません。ペアリングを開始すると再試行します。",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "QR コードかペアリングコードを選んでください。何も開始されていません。",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "現在 {n} 件の番号をペアリング中で、これが同時の上限です。どれかが終わるのを待つか、1 件をキャンセルしてください。",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "番号が「{state}」の状態では、これはできません。何も実行されていません。",
    ),
    ("Starting the pairing…", "ペアリングを開始しています…"),
    (
        "QR code to scan with the phone",
        "電話でスキャンする QR コード",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "QR は数秒ごとに変わります。このウィンドウには現在のものが表示されます。",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "このウィンドウがカウントダウンしている間、コードは有効です。残り {time}。WhatsApp がそれより早く無効にすることもあります。",
    ),
    ("Time left to scan: {time}.", "スキャンの残り時間: {time}。"),
    (
        "This window refreshes by itself.",
        "このウィンドウは自動で更新されます。",
    ),
    ("Refresh now", "今すぐ更新"),
    ("Cancel the pairing", "ペアリングをキャンセル"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "QR が長すぎてここでは描画できません。キャンセルして、ペアリングコードでペアリングしてください。",
    ),
    ("Paired: {phone}.", "ペアリング完了: {phone}。"),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "番号 {alias} は接続されました。アクセス権タブで Hermes にこの番号へのアクセス権を与えてください。",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "時間内にスキャンも入力もされませんでした。番号はペアリングされていません。電話を手元に用意して、もう一度ペアリングしてください。",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA がセッションの失敗を報告しました。番号はペアリングされていません。もう一度試し、再び失敗する場合は WAHA を確認してください。",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "ペアリングはキャンセルされました。番号はペアリングされていません。",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "この番号は別のウィンドウでペアリング中です。QR やコードを表示できるのはそのウィンドウだけです。",
    ),
    ("Cancel that pairing", "そのペアリングをキャンセル"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "このペアリングはもうハブで実行されていません。終了したか、ハブが再起動しました。番号の状態は「{state}」です。",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "ハブは WAHA に接続できませんでした。WAHA が動作していて、ハブのネットワーク上にあるか確認してください。",
    ),
    (
        "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
        "WAHA はハブが持っているキーを拒否しました。エンジンがインストールに同梱されていた場合は、エンジンを再起動してください。エンジンはキーを起動時にしか読み込みません。エンジンを自分で設定した場合は、ハブのサーバーで {command} を使って正しいキーを設定してください。",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA がエラーを返したか、時間内に応答しませんでした。しばらくしてからもう一度試してください。詳細はハブのサーバーで {command} を実行すると分かります。",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp がこの番号を制限しています。しばらくしてからもう一度試してください。",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA がリクエストを拒否しました。ハブのログを確認してください。",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "ハブで何かが失敗しました。ハブのログを確認し、一覧で番号を確認してください。",
    ),
    (
        "{alias}: another phone paired",
        "{alias}: 別の電話がペアリングされました",
    ),
    (
        "The phone that paired belongs to a number different from the one given.",
        "ペアリングされた電話は、指定された番号とは別の番号のものです。",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "決定するまで、番号は「番号の相違」の状態のままで、その番号へのアクセス権は停止したままです。",
    ),
    ("Keep this one", "これを維持"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "取り消すと、WAHA はその電話の WhatsApp から外れ（リンク済みのデバイスから消えます）、WAHA が保持しているこの番号の履歴は消去されます。",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "WAHA がこの番号の履歴を消去することを理解しました。",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "取り消しで何が消去されるかを理解したことにチェックを入れてください。何も実行されていません。",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "ペアリングされた電話は、すでにこのハブの別の番号です。このペアリングを取り消すか、先にもう一方の番号を削除してください。",
    ),
    ("Disconnect {alias}", "{alias} を切断"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "切断すると、WAHA でのこの番号のセッションが停止します。WAHA が WhatsApp から外れることはなく、WAHA は履歴を保持します。切断中は、どの Hermes もこの番号で読み取りも送信もできません。再び使うには、もう一度ペアリングしてください。",
    ),
    (
        "Type the alias {alias} to confirm.",
        "確認のため、別名 {alias} を入力してください。",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "入力した別名が一致しません。何も実行されていません。",
    ),
    ("Remove {alias}", "{alias} を削除"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "削除すると、この番号へのすべてのアクセス権が今すぐ終了します。その別名は二度と使われません。",
    ),
    (
        "What happens to its session in WAHA",
        "WAHA でのこの番号のセッションをどうするか",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "停止して WAHA に残す（WAHA が保持する履歴もそのまま）",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "WAHA を WhatsApp から外して削除する：WAHA が保持する履歴は消去されます",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "Arauto にはまだメッセージのアーカイブがありません。履歴は WAHA が保持するものだけです。",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "WAHA でのこの番号のセッションをどうするか選んでください。何も実行されていません。",
    ),
    ("In test", "テスト中"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Arauto に組み込む WhatsApp エンジンを検討中です。このバージョンには含まれておらず、このウィンドウは何も実行しません。",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "当面は、「WhatsApp を接続 (WAHA)」ボタンから WAHA で番号をペアリングします。",
    ),
    ("Open the WAHA window", "WAHA のウィンドウを開く"),
    ("Try again", "もう一度試す"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "電話でスキャンしてください: WhatsApp › リンク済みのデバイス › デバイスをリンク。",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "電話で: WhatsApp › リンク済みのデバイス › デバイスをリンク › 電話番号でリンク の順に進み、次のコードを入力してください：",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "この設定がある場合、このリンクで届いたメモは、通知の上限の範囲内で {name} に起動通知を送ります。",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} は {from} の指示に従う",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "この設定があると、メモの通知は、その実行の間、{from} が求めることを、オペレーターを待たずに、オペレーターが禁じていない限り行うよう {to} に伝えます。指示の矢印でのみ、起動通知の設定とあわせてのみ使えます。{to} が報告するには戻りのリンクが必要です。これは AI に向けた文であり、制御ではありません。",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "リンク {from} → {to}: {to} が {from} の指示に従うようになりました",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "リンク {from} → {to}: {to} は {from} の指示に従わなくなりました",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "ほかの Hermes に従う Hermes は、オペレーターを待たずにその指示を実行します。上位の Hermes を WhatsApp やメモで惑わせた人は、下位の Hermes にも届きます。ハブを制御できる人はどの Hermes としても書き込めます。パネルのセッションを持つ人はこれらの設定を付けられます。古いバージョンのコネクターは、これらのメモを引き続きデータとして扱います。",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "従う設定は、上位のレベルから下位のレベルへ向かい、起動通知のあるリンクにだけ付けられます。",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "誰が命令を出すか"),
    ("not recorded", "未登録"),
    ("its own chat", "自分自身とのチャット"),
    ("{n} telephones", "電話 {n} 件"),
    ("1 telephone", "電話 1 件"),
    ("Orders", "命令"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "WhatsApp で {hermes} に命令を出すのは誰か",
    ),
    (
        "This is the number {alias} in Arauto.",
        "これは Arauto の番号 {alias} です。",
    ),
    ("Orders come from", "命令の出どころ"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "自分自身の WhatsApp、自分とのチャット内（self-chat モード）",
    ),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "Arauto は、{hermes} を含むすべての Hermes が、{hermes} がこれらの命令を受ける会話に書き込むことを拒否します。読み取りはアクセスの設定に従います。",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "ここに何も登録されていなければ、Arauto は何もブロックしません。{hermes} が命令を受ける会話に書き込める Hermes が、あなた、または命令を出す人になりすます可能性があります。",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} はすでにこの電話で命令を受けています。",
    ),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "ハブでエラーが発生しました。ハブのログを確認し、その Hermes の登録内容を確かめてください。",
    ),
    (
        "A Hermes could give {hermes} orders",
        "ある Hermes が {hermes} に命令を出せるようになります",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "何もブロックされていません。{hermes} が命令を受ける会話に書き込める Hermes は、命令を出す人になりすませます。",
    ),
    ("Go back", "戻る"),
    ("gives orders to {hermes}", "{hermes} に命令を出す"),
    (
        "{hermes} takes orders here",
        "{hermes} はここで命令を受ける",
    ),
    (
        "Orders on WhatsApp for {alias}",
        "{alias} の WhatsApp での命令",
    ),
    ("Its WhatsApp", "その WhatsApp"),
    (
        "The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.",
        "{hermes} 自身の WhatsApp が動いている電話：{hermes} はそこで命令を受けます。",
    ),
    ("It has no WhatsApp", "WhatsApp はありません"),
    ("A number of Arauto", "Arauto の番号"),
    ("Choose a number", "番号を選んでください"),
    ("Another telephone", "別の電話"),
    (
        "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
        "「別の電話」を選んだときだけ読み取ります。+ と国番号から始めてください（例：+55 11 9XXXX-XXXX）。",
    ),
    ("These telephones", "これらの電話"),
    ("its WhatsApp", "その WhatsApp"),
    ("Other telephones", "その他の電話"),
    (
        "Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.",
        "Arauto の番号ではない電話を、1 行に 1 件ずつ。合計で最大 16 件です。",
    ),
    (
        "{alias} is chosen. Nothing is saved until you save.",
        "{alias} を選びました。保存するまで何も保存されません。",
    ),
    (
        "{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.",
        "{before} の代わりに {alias} をその WhatsApp として選びました。保存するまで何も保存されません。",
    ),
    (
        "Choose an option for its WhatsApp. Nothing was saved.",
        "その WhatsApp の選択肢を選んでください。何も保存されていません。",
    ),
    (
        "{alias} is not a number of Arauto now. Nothing was saved.",
        "{alias} は現在 Arauto の番号ではありません。何も保存されていません。",
    ),
    (
        "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
        "その WhatsApp が動いている別の電話を入力するか、別の選択肢を選んでください。何も保存されていません。",
    ),
    (
        "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
        "その WhatsApp の別の電話が電話として読み取れません。8〜15 桁で、+ と国番号から始めてください。何も保存されていません。",
    ),
    (
        "Line {n} of Other telephones is not a telephone. Nothing was saved.",
        "「その他の電話」の {n} 行目は電話ではありません。何も保存されていません。",
    ),
    (
        "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
        "自分自身とのチャットと電話の一覧は別々の答えです。一覧を空にするか、「これらの電話」を選んでください。何も保存されていません。",
    ),
    (
        "Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.",
        "先に {hermes} の WhatsApp を選んでください。チャットには 2 つの端があります。何も保存されていません。",
    ),
    (
        "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
        "その WhatsApp が同時に命令を出すことはできません。そのチャットは self-chat モードです。何も保存されていません。",
    ),
    (
        "At most 16 telephones can give it orders; these are {n}. Nothing was saved.",
        "命令を出せる電話は最大 16 件です。現在 {n} 件です。何も保存されていません。",
    ),
    (
        "1 typed telephone needs a look. Nothing was saved.",
        "入力された電話 1 件の確認が必要です。何も保存されていません。",
    ),
    (
        "{n} typed telephones need a look. Nothing was saved.",
        "入力された電話 {n} 件の確認が必要です。何も保存されていません。",
    ),
    ("Line {n}", "{n} 行目"),
    (
        "Not a telephone: 8 to 15 digits, with + and the country code first.",
        "電話ではありません。8〜15 桁で、+ と国番号から始めてください。",
    ),
    ("Not a number of Arauto.", "Arauto の番号ではありません。"),
    (
        "The same telephone as line {n}: it counts once.",
        "{n} 行目と同じ電話です。1 件として数えます。",
    ),
    (
        "The same telephone as {alias}, chosen above: it counts once.",
        "上で選んだ {alias} と同じ電話です。1 件として数えます。",
    ),
    (
        "This is its WhatsApp: it cannot also give it orders.",
        "これはその WhatsApp です。同時に命令を出すことはできません。",
    ),
    (
        "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
        "+ なしで入力されたため、先頭の数字を国番号として読み取りました。その国でない場合は、+ と国番号から入力してください。もう一度保存すると、読み取ったとおりに保持されます。",
    ),
    (
        "{n} of 16 telephones, with the numbers chosen above.",
        "16 件中 {n} 件（上で選んだ番号を含む）。",
    ),
    (
        "These chats stop being protected",
        "これらのチャットは保護されなくなります",
    ),
    (
        "Between {bridge}, its WhatsApp until now, and:",
        "これまでその WhatsApp だった {bridge} と、次との間：",
    ),
    ("itself (self-chat mode)", "自分自身（self-chat モード）"),
    (
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
        "Arauto は、これらのチャットにあらゆる Hermes が再び書き込み、{hermes} に命令を出す人になりすますことを許可します。",
    ),
    ("Remove the protection", "保護を解除"),
    ("Save without protection", "保護なしで保存"),
    (
        "What {alias} is for it",
        "{alias} がその Hermes にとって何か",
    ),
    ("A telephone that gives it orders", "命令を出す電話"),
    (
        "Its WhatsApp, where it takes orders",
        "その WhatsApp（命令を受ける場所）",
    ),
    (
        "takes orders on {alias} now",
        "現在 {alias} で命令を受けています",
    ),
    (
        "{alias} gives it orders now",
        "{alias} は現在命令を出しています",
    ),
    (
        "Nothing is saved here. The window of the Hermes opens with {alias} chosen.",
        "ここでは何も保存されません。{alias} を選んだ状態で Hermes のウィンドウが開きます。",
    ),
    (
        "Choose a Hermes and what {alias} is for it.",
        "Hermes と、{alias} がその Hermes にとって何かを選んでください。",
    ),
    (
        "{alias} is the WhatsApp of {hermes}: it cannot also give it orders.",
        "{alias} は {hermes} の WhatsApp です。同時に命令を出すことはできません。",
    ),
    ("Actions: {name}", "操作: {name}"),
    (
        "Issues the first invite for {name}.",
        "{name} に最初の招待を発行します。",
    ),
    (
        "Goes back to the connection of {name} that is in progress.",
        "進行中の {name} の接続に戻ります。",
    ),
    (
        "Issues an invite. When it is used, {name} moves to the new device.",
        "招待を発行します。使われると、{name} は新しい端末に移ります。",
    ),
    (
        "Which chat or telephones may give it orders on WhatsApp.",
        "WhatsApp でどのチャットまたはどの電話番号が指示を出せるか。",
    ),
    (
        "Closes {name} in the hub. Asks first.",
        "ハブで {name} を閉じます。先に確認します。",
    ),
    (
        "Opens the window to pair {alias} with a QR code or a pairing code.",
        "QR コードまたはペアリングコードで {alias} をペアリングするウィンドウを開きます。",
    ),
    (
        "Goes back to the pairing of {alias} that is running.",
        "進行中の {alias} のペアリングに戻ります。",
    ),
    (
        "Shows the pairing of {alias} that another panel session started.",
        "パネルの別のセッションが始めた {alias} のペアリングを表示します。",
    ),
    (
        "Stops {alias} for every Hermes. Asks for the alias.",
        "すべての Hermes に対して {alias} を停止します。エイリアスの入力を求めます。",
    ),
    (
        "Starts the failed session of {alias} again.",
        "失敗した {alias} のセッションをもう一度開始します。",
    ),
    (
        "Another phone paired as {alias}: keep it or undo.",
        "別の電話が {alias} としてペアリングされました: 残すか取り消すか。",
    ),
    (
        "Which Hermes takes orders in the chats of this number.",
        "この番号のチャットで指示を受ける Hermes。",
    ),
    (
        "Takes {alias} out of the hub. Asks for the alias.",
        "{alias} をハブから外します。エイリアスの入力を求めます。",
    ),
    ("Dismiss", "非表示にする"),
    ("Dismiss: {title}", "非表示にする: {title}"),
    (
        "The message of this invite cannot be shown again. If it did not reach the Hermes, issue another invite: this one then stops working.",
        "この招待のメッセージは再表示できません。Hermes に届かなかった場合は、別の招待を発行してください。この招待は使えなくなります。",
    ),
    ("Nothing has arrived yet", "まだ何も届いていません"),
    (
        "The Hermes may be waiting for your approval to run the command, in the command channel.",
        "Hermes が、コマンドチャネルでコマンドの実行に対するあなたの承認を待っている可能性があります。",
    ),
    (
        "The Hermes may not reach this hub: check that its machine reaches the address of the hub.",
        "Hermes がこのハブに接続できない可能性があります。そのマシンがハブのアドレスに届くか確認してください。",
    ),
    (
        "The message may have gone to another chat: it only works in the command channel of this Hermes.",
        "メッセージが別のチャットに送られた可能性があります。この Hermes のコマンドチャネルでのみ機能します。",
    ),
    (
        "Type the name {name} to confirm.",
        "確認のため、名前 {name} を入力してください。",
    ),
    (
        "The name typed does not match. Nothing was done.",
        "入力した名前が一致しません。何も実行されていません。",
    ),
    ("Hermes {name}: {state}.", "Hermes {name}: {state}。"),
    (
        "Saved. Who gives {hermes} orders on WhatsApp: {orders}.",
        "保存しました。WhatsApp で {hermes} に指示できる相手: {orders}。",
    ),
    (
        "Outside WhatsApp: another conversation platform, like Telegram",
        "WhatsApp の外: Telegram など、別の会話プラットフォーム",
    ),
    (
        "Arauto then refuses nothing for {hermes}. The hub cannot check it: if {hermes} also takes orders on a WhatsApp, record that WhatsApp instead.",
        "この場合、Arauto は {hermes} について何も拒否しません。ハブはこれを確認できません。{hermes} が WhatsApp でも指示を受ける場合は、代わりにその WhatsApp を登録してください。",
    ),
    (
        "Orders from outside WhatsApp take no telephone: choose It has no WhatsApp, and leave These telephones and Other telephones empty. Nothing was saved.",
        "WhatsApp の外からの指示には電話番号は不要です。「WhatsApp はありません」を選び、「これらの電話」と「その他の電話」は空のままにしてください。何も保存されていません。",
    ),
    ("outside WhatsApp", "WhatsApp の外"),
    (
        "Saved. {hermes} takes its orders outside WhatsApp: Arauto refuses nothing for it.",
        "保存しました。{hermes} は WhatsApp の外から指示を受けます。Arauto はこれについて何も拒否しません。",
    ),
];
