//! The sentences of the panel in this language, as `(English, translation)`.

pub(super) const PAIRS: &[(&str, &str)] = &[
    // view.rs
    ("Panel", "पैनल"),
    ("Language", "भाषा"),
    ("Change", "बदलें"),
    ("R", "प"),
    ("S", "भ"),
    ("M", "ब"),
    ("W", "ज"),
    ("Waiting for the device…", "डिवाइस की प्रतीक्षा…"),
    (
        "The passkey was not accepted. Try again.",
        "पासकी स्वीकार नहीं हुई। फिर से कोशिश करें।",
    ),
    (
        "This browser cannot use a passkey here. Use a current browser, at the address of the panel.",
        "यह ब्राउज़र यहाँ पासकी इस्तेमाल नहीं कर सकता। पैनल के पते पर कोई नया ब्राउज़र इस्तेमाल करें।",
    ),
    ("Sign in to the panel", "पैनल में साइन इन करें"),
    (
        "Use the passkey you registered: on this device, or in your password manager.",
        "वह पासकी इस्तेमाल करें जो आपने रजिस्टर की है: इस डिवाइस पर या अपने पासवर्ड मैनेजर में।",
    ),
    ("Sign in with a passkey", "पासकी से साइन इन करें"),
    ("Installation code", "इंस्टॉलेशन कोड"),
    (
        "First access, or a lost passkey: type the code the hub wrote in its log when it started, or issue one on the hub's server with {command}. The code only opens the registration of a passkey.",
        "पहली बार, या पासकी खो गई है: वह कोड लिखें जो हब ने शुरू होते समय अपने लॉग में लिखा था, या हब के सर्वर पर {command} से नया जारी करें। कोड केवल पासकी का रजिस्ट्रेशन खोलता है।",
    ),
    ("Continue", "जारी रखें"),
    ("Register a passkey", "पासकी रजिस्टर करें"),
    (
        "A passkey stays on this device or in your password manager, and opens with the fingerprint, the face or the PIN of the device. The hub keeps only what checks it: there is no password to leak.",
        "पासकी इस डिवाइस पर या आपके पासवर्ड मैनेजर में रहती है, और डिवाइस के फ़िंगरप्रिंट, चेहरे या पिन से खुलती है। हब केवल वही रखता है जिससे उसकी जाँच होती है: लीक होने के लिए कोई पासवर्ड नहीं है।",
    ),
    (
        "This session only reaches the registration of a passkey. The rest of the panel opens after it.",
        "यह सत्र केवल पासकी के रजिस्ट्रेशन तक पहुँचता है। पैनल का बाकी हिस्सा उसके बाद खुलता है।",
    ),
    ("Sign out", "साइन आउट करें"),
    ("Alerts", "अलर्ट"),
    ("Unknown devices", "अज्ञात डिवाइस"),
    (
        "{n} presented a valid credential the database does not explain. The way back for each one is a new invite. See them with {command}.",
        "{n} ने ऐसा मान्य क्रेडेंशियल पेश किया जिसका डेटाबेस में कोई हिसाब नहीं है। हर एक के लिए वापसी का रास्ता नया आमंत्रण है। इन्हें {command} से देखें।",
    ),
    (
        "More alerts: {n}. See them all with {command}.",
        "और अलर्ट: {n}। सभी को {command} से देखें।",
    ),
    ("Accesses", "पहुँच"),
    (
        "In this version the panel connects and revokes Hermes and changes the accesses of each number. Pairing numbers and choosing chats are still done with the {command} command, on the hub's server.",
        "इस संस्करण में पैनल Hermes को कनेक्ट और निरस्त करता है और हर नंबर की पहुँच बदलता है। नंबर पेयर करना और चैट चुनना अभी भी हब के सर्वर पर {command} कमांड से होता है।",
    ),
    ("none", "कोई नहीं"),
    ("Your Hermes and their [numbers]", "आपके Hermes और उनके [नंबर]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "कनेक्टेड: {connected} · प्रतीक्षा में: {waiting} · अलर्ट पर: {stopped}",
    ),
    ("Connect Hermes", "Hermes कनेक्ट करें"),
    ("State", "स्थिति"),
    ("Numbers", "नंबर"),
    ("Can be woken", "जगाया जा सकता है"),
    ("Connector", "कनेक्टर"),
    ("Seen", "देखा गया"),
    ("Actions", "कार्रवाइयाँ"),
    (
        "No Hermes yet. Connect the first one with the \"Connect Hermes\" button.",
        "अभी कोई Hermes नहीं है। पहला Hermes \"Hermes कनेक्ट करें\" बटन से कनेक्ट करें।",
    ),
    ("yes", "हाँ"),
    ("no", "नहीं"),
    ("Connect", "कनेक्ट करें"),
    ("Reconnect", "फिर से कनेक्ट करें"),
    ("Revoke", "निरस्त करें"),
    (
        "\"Numbers\" are the ones the Hermes reaches. The connector is what the device declared: it is not verified.",
        "\"नंबर\" वे हैं जिन तक Hermes पहुँचता है। कनेक्टर वही है जो डिवाइस ने बताया: यह सत्यापित नहीं है।",
    ),
    ("Engine", "इंजन"),
    (
        "There is no engine yet. Set it with {command}.",
        "अभी कोई इंजन नहीं है। इसे {command} से सेट करें।",
    ),
    ("Alias", "उपनाम"),
    ("Number", "नंबर"),
    ("Hermes with access", "पहुँच वाले Hermes"),
    ("Last message", "आखिरी संदेश"),
    (
        "No number yet. Add one with {command}.",
        "अभी कोई नंबर नहीं है। {command} से एक जोड़ें।",
    ),
    ("command", "कमांड"),
    (
        "The last message is shown only by its time, never its content. The panel lists what goes through the hub: other linked devices of the same number only show on the phone.",
        "आखिरी संदेश का केवल समय दिखाया जाता है, उसकी सामग्री कभी नहीं। पैनल वही दिखाता है जो हब से होकर गुज़रता है: उसी नंबर के दूसरे लिंक किए गए डिवाइस केवल फ़ोन पर दिखते हैं।",
    ),
    (
        "Hermes in the rows, numbers in the columns. Click a cell to change it.",
        "पंक्तियों में Hermes, कॉलम में नंबर। किसी सेल को बदलने के लिए उस पर क्लिक करें।",
    ),
    ("Connect a Hermes first.", "पहले एक Hermes कनेक्ट करें।"),
    ("Add a WhatsApp first.", "पहले एक WhatsApp जोड़ें।"),
    ("(accesses kept)", "(पहुँच सुरक्षित रखी गई)"),
    ("no access", "कोई पहुँच नहीं"),
    (
        "{r} read · {s} send · {m} modify · {w} be woken.",
        "{r} पढ़ना · {s} भेजना · {m} बदलाव करना · {w} जगाया जाना।",
    ),
    ("Saved: {what}", "सहेजा गया: {what}"),
    ("Warnings that block nothing", "चेतावनियाँ जो कुछ नहीं रोकतीं"),
    // guide.rs
    ("Name", "नाम"),
    ("Block", "ब्लॉक"),
    ("Code", "कोड"),
    ("Done", "पूर्ण"),
    ("Invite issued", "आमंत्रण जारी हुआ"),
    ("A device used the invite", "एक डिवाइस ने आमंत्रण इस्तेमाल किया"),
    (
        "Code checked and request approved",
        "कोड जाँचा गया और अनुरोध स्वीकृत हुआ",
    ),
    (
        "The connector confirmed its first credential",
        "कनेक्टर ने अपने पहले क्रेडेंशियल की पुष्टि की",
    ),
    (
        "A device used the invite, from {address}",
        "एक डिवाइस ने आमंत्रण इस्तेमाल किया, पता {address} से",
    ),
    ("waiting…", "प्रतीक्षा में…"),
    (
        "What name will this Hermes have in the panel?",
        "पैनल में इस Hermes का नाम क्या होगा?",
    ),
    ("Name of the Hermes", "Hermes का नाम"),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the block and does not change afterwards.",
        "छोटे अक्षर, अंक, हाइफ़न और अंडरस्कोर, अधिकतम 32 वर्ण, शुरुआत किसी अक्षर या अंक से। नाम ब्लॉक में दिखता है और बाद में नहीं बदलता।",
    ),
    (
        "Preparing a secure connection",
        "सुरक्षित कनेक्शन तैयार किया जा रहा है",
    ),
    (
        "The hub drew the invite of {name}.",
        "हब ने {name} का आमंत्रण तैयार किया।",
    ),
    (
        "Invite for {name} · valid for another {time} · single use",
        "{name} के लिए आमंत्रण · अभी {time} और मान्य · केवल एक बार इस्तेमाल",
    ),
    ("Copied", "कॉपी किया गया"),
    (
        "Click the block to copy it.",
        "ब्लॉक को कॉपी करने के लिए उस पर क्लिक करें।",
    ),
    (
        "Select the whole text and copy it.",
        "पूरा टेक्स्ट चुनें और कॉपी करें।",
    ),
    (
        "It only appears here, once.",
        "यह केवल यहाँ, एक ही बार दिखता है।",
    ),
    (
        "The invite issued before was cancelled.",
        "पहले जारी किया गया आमंत्रण रद्द कर दिया गया।",
    ),
    (
        "The request that was in progress was closed.",
        "जो अनुरोध चल रहा था उसे बंद कर दिया गया।",
    ),
    (
        "The device in use keeps working until the new one confirms its first credential.",
        "इस्तेमाल में चल रहा डिवाइस तब तक काम करता रहेगा जब तक नया डिवाइस अपने पहले क्रेडेंशियल की पुष्टि नहीं करता।",
    ),
    (
        "This hub has no connector to serve",
        "इस हब के पास देने के लिए कोई कनेक्टर नहीं है",
    ),
    (
        "The block tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "ब्लॉक Hermes को इस हब से कनेक्टर डाउनलोड करने को कहता है, और डाउनलोड फ़ोल्डर खाली है। कुछ भी जारी नहीं हुआ। {name} का आमंत्रण हब के सर्वर पर {command} कमांड से जारी करें।",
    ),
    (
        "Waiting for the Hermes {name}",
        "Hermes {name} की प्रतीक्षा में",
    ),
    (
        "Each step lights up when the hub actually observes it. If the Hermes asks for approval to run the command, you are the one who approves, in the command channel.",
        "हर चरण तभी जलता है जब हब उसे सचमुच देखता है। अगर Hermes कमांड चलाने के लिए स्वीकृति माँगे, तो स्वीकृति आप ही देते हैं, कमांड चैनल में।",
    ),
    (
        "A device used the invite of {name}",
        "एक डिवाइस ने {name} का आमंत्रण इस्तेमाल किया",
    ),
    ("unknown", "अज्ञात"),
    ("Seen by the hub", "हब ने देखा"),
    ("address {address}, {when}", "पता {address}, {when}"),
    ("Said by the device", "डिवाइस ने बताया"),
    (
        "machine \"{host}\" · system \"{os}\" · Hermes \"{version}\" (not verified)",
        "मशीन \"{host}\" · सिस्टम \"{os}\" · Hermes \"{version}\" (सत्यापित नहीं)",
    ),
    (
        "Last use of the current device: {when}, from {address}. Do you recognize it?",
        "मौजूदा डिवाइस का आखिरी इस्तेमाल: {when}, पता {address} से। क्या आप इसे पहचानते हैं?",
    ),
    (
        "Last use of the current device: {when}. Do you recognize it?",
        "मौजूदा डिवाइस का आखिरी इस्तेमाल: {when}। क्या आप इसे पहचानते हैं?",
    ),
    ("I recognize it", "मैं इसे पहचानता हूँ"),
    ("I do not recognize it", "मैं इसे नहीं पहचानता"),
    (
        "If the Hermes was off and there was use, someone used a copy.",
        "अगर Hermes बंद था और फिर भी इस्तेमाल हुआ, तो किसी ने कॉपी इस्तेमाल की।",
    ),
    ("Verification code", "सत्यापन कोड"),
    (
        "Type the code the Hermes answered in the command channel. The panel never shows this code.",
        "वह कोड लिखें जो Hermes ने कमांड चैनल में जवाब में दिया। पैनल यह कोड कभी नहीं दिखाता।",
    ),
    (
        "The request of {name} was denied",
        "{name} का अनुरोध अस्वीकार कर दिया गया",
    ),
    (
        "The code was typed wrong for the fifth time. To connect, issue another invite.",
        "कोड पाँचवीं बार गलत लिखा गया। कनेक्ट करने के लिए दूसरा आमंत्रण जारी करें।",
    ),
    (
        "The request of {name} expired",
        "{name} के अनुरोध की समय-सीमा खत्म हो गई",
    ),
    (
        "The 10 minutes to type the code passed. To connect, issue another invite.",
        "कोड लिखने के 10 मिनट बीत गए। कनेक्ट करने के लिए दूसरा आमंत्रण जारी करें।",
    ),
    (
        "The identity of {name} was copied",
        "{name} की पहचान कॉपी की गई",
    ),
    (
        "Two devices showed up with the same identity before the approval, and the request was stopped. To connect, issue another invite.",
        "स्वीकृति से पहले दो डिवाइस एक ही पहचान के साथ आए, और अनुरोध रोक दिया गया। कनेक्ट करने के लिए दूसरा आमंत्रण जारी करें।",
    ),
    (
        "{name} has no invite and no request waiting",
        "{name} का न कोई आमंत्रण है और न कोई अनुरोध प्रतीक्षा में",
    ),
    (
        "The invite expired, was cancelled, or the request was already resolved. If the Hermes answered no code, do not approve; if it answered AR-102, another device used the invite. To connect, issue another invite.",
        "आमंत्रण की समय-सीमा खत्म हो गई, वह रद्द कर दिया गया, या अनुरोध पहले ही निपटाया जा चुका है। अगर Hermes ने कोई कोड नहीं दिया, तो स्वीकृत न करें; अगर उसने AR-102 दिया, तो किसी दूसरे डिवाइस ने आमंत्रण इस्तेमाल किया। कनेक्ट करने के लिए दूसरा आमंत्रण जारी करें।",
    ),
    ("{name} is connected", "{name} कनेक्टेड है"),
    ("{name} was approved", "{name} स्वीकृत हुआ"),
    (
        "What it may do on each number is in the Accesses tab.",
        "हर नंबर पर यह क्या कर सकता है, यह पहुँच टैब में है।",
    ),
    (
        "As soon as the connector confirms, it shows as connected. The accesses it already had apply to this device again.",
        "कनेक्टर के पुष्टि करते ही यह कनेक्टेड दिखेगा। इसके पास पहले से जो पहुँच थी, वह इस डिवाइस पर फिर से लागू होगी।",
    ),
    (
        "As soon as the connector confirms, it shows as connected. It has no access to any number yet.",
        "कनेक्टर के पुष्टि करते ही यह कनेक्टेड दिखेगा। इसके पास अभी किसी नंबर की पहुँच नहीं है।",
    ),
    (
        "The hub did not do what was asked",
        "हब ने वह नहीं किया जो कहा गया था",
    ),
    ("Issue invite", "आमंत्रण जारी करें"),
    ("Cancel", "रद्द करें"),
    ("Next step", "अगला चरण"),
    (
        "After copying, paste it into the command channel of the Hermes {name}. Nowhere else.",
        "कॉपी करने के बाद इसे Hermes {name} के कमांड चैनल में पेस्ट करें। और कहीं नहीं।",
    ),
    (
        "The invite is valid for another {time}. This screen refreshes by itself.",
        "आमंत्रण अभी {time} और मान्य है। यह स्क्रीन अपने आप रीफ़्रेश होती है।",
    ),
    ("Approve", "स्वीकृत करें"),
    ("Deny", "अस्वीकार करें"),
    ("Confirm within {time}.", "{time} के भीतर पुष्टि करें।"),
    ("Issue another invite", "दूसरा आमंत्रण जारी करें"),
    ("Close", "बंद करें"),
    ("See accesses", "पहुँच देखें"),
    ("Finish", "समाप्त करें"),
    ("Copied to the clipboard.", "क्लिपबोर्ड पर कॉपी किया गया।"),
    (
        "The text is selected. Copy it with Ctrl+C.",
        "टेक्स्ट चुना हुआ है। इसे Ctrl+C से कॉपी करें।",
    ),
    ("Reconnect {name}", "{name} को फिर से कनेक्ट करें"),
    ("Connect {name}", "{name} को कनेक्ट करें"),
    ("Steps", "चरण"),
    // editor.rs
    (
        "Being woken requires reading. Nothing was saved.",
        "जगाए जाने के लिए पढ़ना ज़रूरी है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Starting chats requires sending and the scope \"every chat\". Nothing was saved.",
        "चैट शुरू करने के लिए भेजना और दायरा \"हर चैट\" ज़रूरी है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "The wait before waking goes from 0 to 60 seconds. Nothing was saved.",
        "जगाने से पहले की प्रतीक्षा 0 से 60 सेकंड तक होती है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Something failed in the hub. See the hub's log and check the access in the matrix.",
        "हब में कुछ विफल हुआ। हब का लॉग देखें और मैट्रिक्स में पहुँच जाँचें।",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub.",
        "डेटाबेस एक पुराने बैकअप से वापस आया है और हब ने इसे अभी तक नहीं सँभाला है। कुछ भी लिखा नहीं गया। हब को फिर से चालू करें।",
    ),
    (
        "The Hermes {hermes} has no access to the number {number}.",
        "Hermes {hermes} के पास नंबर {number} की कोई पहुँच नहीं है।",
    ),
    (
        "The Hermes {hermes} only reaches the chosen chats of the number {number}. Chats with a line: {lines}.",
        "Hermes {hermes} नंबर {number} की केवल चुनी हुई चैट तक पहुँचता है। पंक्ति वाली चैट: {lines}।",
    ),
    (
        "The Hermes {hermes} on the number {number}, in every chat:",
        "Hermes {hermes}, नंबर {number} पर, हर चैट में:",
    ),
    ("reads", "पढ़ता है"),
    ("does not read", "नहीं पढ़ता"),
    ("sends", "भेजता है"),
    ("does not send", "नहीं भेजता"),
    ("modifies", "बदलाव करता है"),
    ("does not modify", "बदलाव नहीं करता"),
    (
        "is woken after {seconds} s of silence",
        "{seconds} s की चुप्पी के बाद जगाया जाता है",
    ),
    ("is not woken", "जगाया नहीं जाता"),
    ("may start chats", "चैट शुरू कर सकता है"),
    (
        "chats with their own rule: {lines}",
        "अपने नियम वाली चैट: {lines}",
    ),
    (
        "{hermes} on the number {number}",
        "{hermes}, नंबर {number} पर",
    ),
    ("Where it applies", "यह कहाँ लागू होता है"),
    (
        "Every chat, including the ones that come later",
        "हर चैट, बाद में आने वाली भी",
    ),
    ("Only chosen chats", "केवल चुनी हुई चैट"),
    (
        "With \"only chosen chats\", only the chats that have a line of their own exist for the Hermes, and what it may do in each one comes from the line. The lines are still created with the {command} command. Today: {lines}.",
        "\"केवल चुनी हुई चैट\" के साथ, Hermes के लिए केवल वही चैट मौजूद हैं जिनकी अपनी पंक्ति है, और हर एक में यह क्या कर सकता है, यह पंक्ति से तय होता है। पंक्तियाँ अभी भी {command} कमांड से बनाई जाती हैं। आज: {lines}।",
    ),
    ("What it may do", "यह क्या कर सकता है"),
    ("Read", "पढ़ना"),
    (
        "see chats and messages, search, download files",
        "चैट और संदेश देखना, खोजना, फ़ाइलें डाउनलोड करना",
    ),
    ("Send", "भेजना"),
    (
        "write, react, mark as read",
        "लिखना, प्रतिक्रिया देना, पढ़ा हुआ चिह्नित करना",
    ),
    ("Modify", "बदलाव करना"),
    (
        "edit and delete what already exists",
        "जो पहले से मौजूद है उसे संपादित करना और हटाना",
    ),
    ("Be woken", "जगाया जाना"),
    (
        "the hub gives notice when a message arrives (requires reading)",
        "संदेश आने पर हब सूचना देता है (पढ़ना ज़रूरी है)",
    ),
    ("Wait", "प्रतीक्षा"),
    ("seconds of silence before waking", "जगाने से पहले चुप्पी के सेकंड"),
    (
        "May start a chat with someone who never talked to the number (requires sending and every chat)",
        "ऐसे व्यक्ति से चैट शुरू कर सकता है जिसने नंबर से कभी बात नहीं की (भेजना और हर चैट ज़रूरी है)",
    ),
    ("Today:", "आज:"),
    ("Save", "सहेजें"),
    (
        "It applies from the next call, with nothing to restart. With no verb marked, the access is removed.",
        "यह अगली कॉल से लागू होता है, कुछ भी फिर से चालू करने की ज़रूरत नहीं। कोई क्रिया चिह्नित न होने पर पहुँच हटा दी जाती है।",
    ),
    ("Revoke {name}", "{name} को निरस्त करें"),
    (
        "Revoking closes the Hermes in the hub: the device in use, the request in progress and the issued invite. The channel drops within seconds.",
        "निरस्त करने से Hermes हब में बंद हो जाता है: इस्तेमाल में चल रहा डिवाइस, चल रहा अनुरोध और जारी किया गया आमंत्रण। चैनल कुछ ही सेकंड में टूट जाता है।",
    ),
    (
        "The accesses are kept and apply again when a new device is connected. The only way back is a new invite.",
        "पहुँच सुरक्षित रहती है और नया डिवाइस कनेक्ट होने पर फिर से लागू होती है। वापसी का एकमात्र रास्ता नया आमंत्रण है।",
    ),
    // data.rs
    ("never", "कभी नहीं"),
    ("now", "अभी"),
    ("{n} min ago", "{n} मिनट पहले"),
    ("{n} h ago", "{n} घं. पहले"),
    ("{n} d ago", "{n} दिन पहले"),
    (
        "a credential that had already been swapped was presented",
        "ऐसा क्रेडेंशियल पेश किया गया जिसकी अदला-बदली पहले ही हो चुकी थी",
    ),
    ("two connections at the same time", "एक ही समय पर दो कनेक्शन"),
    ("the credential could not be read", "क्रेडेंशियल पढ़ा नहीं जा सका"),
    (
        "the last use was not recognized on the reconnect",
        "फिर से कनेक्ट करते समय आखिरी इस्तेमाल पहचाना नहीं गया",
    ),
    (
        "the identity was copied before the approval",
        "स्वीकृति से पहले पहचान कॉपी की गई",
    ),
    (
        "the hub no longer trusts the credential",
        "हब अब क्रेडेंशियल पर भरोसा नहीं करता",
    ),
    ("Waiting for your code", "आपके कोड की प्रतीक्षा में"),
    (
        "Approved, waiting for the connector",
        "स्वीकृत, कनेक्टर की प्रतीक्षा में",
    ),
    ("Waiting for the Hermes", "Hermes की प्रतीक्षा में"),
    ("Connected", "कनेक्टेड"),
    ("Disconnected", "डिस्कनेक्टेड"),
    ("In quarantine", "क्वारंटीन में"),
    ("Suspended", "निलंबित"),
    ("No device", "कोई डिवाइस नहीं"),
    ("a Hermes", "एक Hermes"),
    ("Invite", "आमंत्रण"),
    (
        "another device tried to use the invite of {hermes}. The block ended up somewhere else. The first request still stands.",
        "किसी दूसरे डिवाइस ने {hermes} का आमंत्रण इस्तेमाल करने की कोशिश की। ब्लॉक कहीं और पहुँच गया। पहला अनुरोध अब भी कायम है।",
    ),
    ("Identity copied", "पहचान कॉपी हुई"),
    (
        "the identity of {hermes} was copied before the approval. The request was denied.",
        "स्वीकृति से पहले {hermes} की पहचान कॉपी की गई। अनुरोध अस्वीकार कर दिया गया।",
    ),
    ("Hermes in quarantine", "Hermes क्वारंटीन में"),
    ("{hermes}. Reason: {reason}.", "{hermes}। कारण: {reason}।"),
    ("Suspected copy", "कॉपी का संदेह"),
    (
        "the last use of {hermes} was not recognized on the reconnect.",
        "फिर से कनेक्ट करते समय {hermes} का आखिरी इस्तेमाल पहचाना नहीं गया।",
    ),
    ("Connection taken", "कनेक्शन ले लिया गया"),
    (
        "the connection of {hermes} was taken by another one.",
        "{hermes} का कनेक्शन किसी दूसरे ने ले लिया।",
    ),
    ("Closing", "बंद करना"),
    (
        "{hermes} asked for its own closing.",
        "{hermes} ने खुद को बंद करने का अनुरोध किया।",
    ),
    ("Reuse", "दोबारा इस्तेमाल"),
    (
        "a copy of {hermes} was used before the closing.",
        "बंद किए जाने से पहले {hermes} की एक कॉपी इस्तेमाल की गई।",
    ),
    ("Unconfirmed swaps", "बिना पुष्टि की अदला-बदली"),
    (
        "{hermes} received credentials and did not confirm them, three times in 24 hours. It may be a bad network, or someone trying a copy.",
        "{hermes} को क्रेडेंशियल मिले और उसने उनकी पुष्टि नहीं की, 24 घंटों में तीन बार। हो सकता है नेटवर्क खराब हो, या कोई कॉपी आज़मा रहा हो।",
    ),
    ("Audit mark", "ऑडिट चिह्न"),
    (
        "a security event was written and the mark outside the database did not follow.",
        "एक सुरक्षा घटना लिखी गई और डेटाबेस के बाहर का चिह्न उसके साथ नहीं बदला।",
    ),
    (
        "the version of the engine is not one the hub has checked.",
        "इंजन का यह संस्करण हब द्वारा जाँचे गए संस्करणों में नहीं है।",
    ),
    (
        "an engine session had another configuration, and the hub applied its own.",
        "एक इंजन सेशन का कॉन्फ़िगरेशन अलग था, और हब ने अपना कॉन्फ़िगरेशन लागू किया।",
    ),
    ("Number swapped", "नंबर बदला हुआ"),
    (
        "the phone that paired belongs to a different number from the one given. The accesses stay stopped until you decide.",
        "जो फ़ोन पेयर हुआ वह बताए गए नंबर से अलग नंबर का है। आपके फ़ैसले तक पहुँच रुकी रहेगी।",
    ),
    ("Address: {address}.", "पता: {address}।"),
    ("an unknown date", "अज्ञात तारीख"),
    (
        "The hub came back from a backup of {date}. What was approved, revoked, reconnected or changed in the accesses after that is lost. Reconnect each Hermes and check the accesses.",
        "हब इस तारीख के बैकअप से वापस आया है: {date}। उसके बाद जो कुछ स्वीकृत, निरस्त, फिर से कनेक्ट या पहुँच में बदला गया था, वह खो गया है। हर Hermes को फिर से कनेक्ट करें और पहुँच जाँचें।",
    ),
    (
        "The hub came back from a backup. {name} is suspended and only comes back with a new invite: reconnect or revoke it. There is no \"reactivate\".",
        "हब एक बैकअप से वापस आया है। {name} निलंबित है और केवल नए आमंत्रण से वापस आता है: इसे फिर से कनेक्ट करें या निरस्त करें। \"फिर से सक्रिय करें\" जैसा कुछ नहीं है।",
    ),
    (
        "The hub no longer trusts the credential of {name}. Reason: {reason}. Its access is stopped. Ways out: reconnect or revoke. There is no \"release\".",
        "हब अब {name} के क्रेडेंशियल पर भरोसा नहीं करता। कारण: {reason}। इसकी पहुँच रुकी हुई है। रास्ते: फिर से कनेक्ट करें या निरस्त करें। \"मुक्त करें\" जैसा कुछ नहीं है।",
    ),
    ("Not paired", "पेयर नहीं है"),
    ("Pairing", "पेयर हो रहा है"),
    ("Reconnecting", "फिर से कनेक्ट हो रहा है"),
    ("Failed", "विफल"),
    ("Removed", "हटाया गया"),
    ("version {version}, checked", "संस्करण {version}, जाँचा हुआ"),
    (
        "version {version}, not checked by the hub",
        "संस्करण {version}, हब द्वारा जाँचा नहीं गया",
    ),
    ("version not read yet", "संस्करण अभी तक पढ़ा नहीं गया"),
    ("last answer {when}", "आखिरी जवाब {when}"),
    ("has not answered yet", "अभी तक जवाब नहीं दिया"),
    ("all", "सभी"),
    ("all · own rules: {n}", "सभी · अपने नियम: {n}"),
    ("chosen chats: {n}", "चुनी हुई चैट: {n}"),
    (
        "{alias}: {n} Hermes are woken in the same chats.",
        "{alias}: एक ही चैट में जगाए जाने वाले Hermes: {n}।",
    ),
    (
        "{alias}: {n} Hermes send in the same chats.",
        "{alias}: एक ही चैट में भेजने वाले Hermes: {n}।",
    ),
    (
        "{alias} is marked as a command number and there are Hermes allowed to send on it.",
        "{alias} कमांड नंबर के रूप में चिह्नित है और कुछ Hermes को इस पर भेजने की अनुमति है।",
    ),
    // connect.rs
    (
        "There already is a Hermes with that name. To link another device to it, use the button on its line.",
        "इस नाम का Hermes पहले से मौजूद है। उससे दूसरा डिवाइस जोड़ने के लिए उसकी पंक्ति का बटन इस्तेमाल करें।",
    ),
    (
        "That name belonged to a Hermes that was removed, and a name is never reused. Choose another.",
        "यह नाम एक ऐसे Hermes का था जिसे हटा दिया गया, और कोई नाम कभी दोबारा इस्तेमाल नहीं होता। दूसरा नाम चुनें।",
    ),
    (
        "Before the code, say whether you recognize the last use of the current device. Nothing was counted.",
        "कोड से पहले बताएँ कि आप मौजूदा डिवाइस का आखिरी इस्तेमाल पहचानते हैं या नहीं। कुछ भी गिना नहीं गया।",
    ),
    (
        "The current device was used while you were answering. See its last use now and answer again. Nothing was approved.",
        "जब आप जवाब दे रहे थे, तब मौजूदा डिवाइस इस्तेमाल हुआ। अब उसका आखिरी इस्तेमाल देखें और फिर से जवाब दें। कुछ भी स्वीकृत नहीं हुआ।",
    ),
    (
        "The code has 6 digits, like 123-456. Nothing was counted.",
        "कोड में 6 अंक होते हैं, जैसे 123-456। कुछ भी गिना नहीं गया।",
    ),
    (
        "The database came back from an older backup and the hub has not handled it yet. Nothing was written. Restart the hub: it turns the restored mode on, and then each Hermes is reconnected with a new invite.",
        "डेटाबेस एक पुराने बैकअप से वापस आया है और हब ने इसे अभी तक नहीं सँभाला है। कुछ भी लिखा नहीं गया। हब को फिर से चालू करें: वह बहाली मोड चालू करता है, और फिर हर Hermes नए आमंत्रण से फिर से कनेक्ट किया जाता है।",
    ),
    (
        "Something failed in the hub and nothing was done. See the hub's log.",
        "हब में कुछ विफल हुआ और कुछ भी नहीं किया गया। हब का लॉग देखें।",
    ),
    (
        "The name has from 1 to 32 characters.",
        "नाम में 1 से 32 वर्ण होते हैं।",
    ),
    (
        "The name starts with a lowercase letter or a digit.",
        "नाम छोटे अक्षर या अंक से शुरू होता है।",
    ),
    (
        "Use only lowercase letters, digits, hyphen and underscore.",
        "केवल छोटे अक्षर, अंक, हाइफ़न और अंडरस्कोर इस्तेमाल करें।",
    ),
    (
        "That name is reserved. Choose another.",
        "यह नाम आरक्षित है। दूसरा नाम चुनें।",
    ),
    (
        "The code does not match. Attempt {attempts} of {max}. Type the last code the Hermes answered.",
        "कोड मेल नहीं खाता। प्रयास {attempts}, कुल {max} में से। वह आखिरी कोड लिखें जो Hermes ने जवाब में दिया।",
    ),
    // mod.rs
    (
        "Invalid or expired code. Issue another one on the hub's server.",
        "कोड अमान्य है या उसकी समय-सीमा खत्म हो गई है। हब के सर्वर पर दूसरा कोड जारी करें।",
    ),
    (
        "Too many attempts right now. Wait a minute and try again.",
        "अभी बहुत ज़्यादा प्रयास हुए हैं। एक मिनट रुकें और फिर से कोशिश करें।",
    ),
];
