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
    ("Sign in with a passkey", "पासकी से साइन इन करें"),
    ("Installation code", "इंस्टॉलेशन कोड"),
    ("Continue", "जारी रखें"),
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
    ("none", "कोई नहीं"),
    ("Your Hermes and their [numbers]", "आपके Hermes और उनके [नंबर]"),
    (
        "Connected: {connected} · Waiting: {waiting} · On alert: {stopped}",
        "कनेक्टेड: {connected} · प्रतीक्षा में: {waiting} · अलर्ट पर: {stopped}",
    ),
    ("Connect Hermes", "Hermes कनेक्ट करें"),
    ("State", "स्थिति"),
    ("Numbers", "नंबर"),
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
    (
        "That code did not work. Use the latest one: the one in the last block of the log, or one issued on the hub's server.",
        "उस कोड ने काम नहीं किया। सबसे ताज़ा कोड इस्तेमाल करें: लॉग के आखिरी ब्लॉक वाला, या हब के सर्वर पर जारी किया गया।",
    ),
    (
        "The hub did not accept this passkey. Try again.",
        "हब ने यह पासकी स्वीकार नहीं की। फिर से कोशिश करें।",
    ),
    (
        "Your panel session ended. Type the latest code to pick up where you left off.",
        "आपका पैनल सत्र समाप्त हो गया। जहाँ छोड़ा था वहीं से आगे बढ़ने के लिए सबसे ताज़ा कोड लिखें।",
    ),
    (
        "This browser cannot use a passkey here. Open the panel in an up-to-date browser, directly at its address (not inside another app).",
        "यह ब्राउज़र यहाँ पासकी इस्तेमाल नहीं कर सकता। पैनल को किसी अपडेटेड ब्राउज़र में, सीधे उसके पते पर खोलें (किसी दूसरे ऐप के अंदर नहीं)।",
    ),
    (
        "The passkey was not created: the request was cancelled or took too long. Just try again.",
        "पासकी नहीं बनी: अनुरोध रद्द हो गया या उसमें बहुत देर लगी। बस फिर से कोशिश करें।",
    ),
    (
        "No passkey was used. If it is not on this device, use the device or the password manager where you created it, or open “Lost your passkey?”.",
        "कोई पासकी इस्तेमाल नहीं हुई। अगर वह इस डिवाइस पर नहीं है, तो वह डिवाइस या पासवर्ड मैनेजर इस्तेमाल करें जहाँ आपने उसे बनाया था, या “पासकी खो गई?” खोलें।",
    ),
    ("Passkey", "पासकी"),
    ("Check", "जाँच"),
    ("Welcome to your Arauto hub", "आपके Arauto हब में आपका स्वागत है"),
    (
        "Let's set up your first access. It takes three quick steps, and you only do it once.",
        "आइए पहली बार के लिए आपका एक्सेस सेट करें। इसमें तीन छोटे चरण लगते हैं, और यह सिर्फ़ एक बार करना होता है।",
    ),
    ("Step 1 · Confirm it is you", "चरण 1 · पुष्टि करें कि यह आप हैं"),
    (
        "When the hub started, it wrote an installation code in its own log. Only whoever looks after the server sees that log: that is how the hub knows it is you.",
        "हब ने शुरू होते समय अपने लॉग में एक इंस्टॉलेशन कोड लिखा था। यह लॉग सिर्फ़ वही देख सकता है जो सर्वर संभालता है: इसी से हब जानता है कि यह आप हैं।",
    ),
    ("Where to find the code:", "कोड कहाँ मिलेगा:"),
    (
        "In Easypanel, open the service of the hub and go to Logs. (With Docker: {command})",
        "Easypanel में हब की सर्विस खोलें और Logs पर जाएँ। (Docker में: {command})",
    ),
    (
        "Find the ARAUTO FIRST ACCESS block: the newest one is the last, at the bottom of the log. Copy its code.",
        "ARAUTO FIRST ACCESS ब्लॉक ढूँढें: सबसे नया ब्लॉक आखिरी होता है, लॉग के सबसे नीचे। उसका कोड कॉपी करें।",
    ),
    ("Paste the code here:", "कोड यहाँ पेस्ट करें:"),
    (
        "The code does not expire and works only once. If the hub restarts, it writes a new one: always use the latest.",
        "कोड की समय-सीमा खत्म नहीं होती और यह सिर्फ़ एक बार काम करता है। हब दोबारा चालू हो तो वह नया कोड लिखता है: हमेशा सबसे ताज़ा कोड इस्तेमाल करें।",
    ),
    (
        "Cannot open the log? On the hub's server, run {command}. Careful: that new code cancels the one in the log.",
        "लॉग नहीं खुल रहा? हब के सर्वर पर {command} चलाएँ। ध्यान दें: वह नया कोड लॉग वाले कोड को रद्द कर देता है।",
    ),
    ("Good to have you back", "आपका फिर से स्वागत है"),
    (
        "Enter with your passkey: the fingerprint, the face or the PIN of this device, or your password manager.",
        "अपनी पासकी से साइन इन करें: इस डिवाइस का फ़िंगरप्रिंट, चेहरा या पिन, या आपका पासवर्ड मैनेजर।",
    ),
    ("Lost your passkey?", "पासकी खो गई?"),
    (
        "No problem. On the hub's server, run {command}. It shows a code valid for 10 minutes: type it here and create a new passkey.",
        "कोई बात नहीं। हब के सर्वर पर {command} चलाएँ। वह 10 मिनट तक मान्य एक कोड दिखाता है: उसे यहाँ लिखें और नई पासकी बनाएँ।",
    ),
    (
        "Code accepted! Now, create your passkey",
        "कोड स्वीकार हो गया! अब अपनी पासकी बनाएँ",
    ),
    (
        "It is how you will enter the panel from now on, with no password to remember.",
        "अब से आप इसी से पैनल में साइन इन करेंगे, याद रखने के लिए कोई पासवर्ड नहीं।",
    ),
    (
        "The passkey stays on this device or in your password manager, and opens with your fingerprint, your face or the PIN of the device. The hub keeps only what checks the passkey: there is no password to leak.",
        "पासकी इस डिवाइस पर या आपके पासवर्ड मैनेजर में रहती है, और आपके फ़िंगरप्रिंट, चेहरे या डिवाइस के पिन से खुलती है। हब केवल वही रखता है जिससे पासकी की जाँच होती है: लीक होने के लिए कोई पासवर्ड नहीं है।",
    ),
    ("It will work at this address:", "यह इस पते पर काम करेगी:"),
    ("Create my passkey", "मेरी पासकी बनाएँ"),
    (
        "If you use a password manager, the passkey can go along to your other devices.",
        "अगर आप पासवर्ड मैनेजर इस्तेमाल करते हैं, तो पासकी आपके दूसरे डिवाइसों पर भी पहुँच सकती है।",
    ),
    (
        "Left this page by accident? Type the same code again: it is only spent when the passkey is created.",
        "गलती से यह पेज छोड़ दिया? वही कोड फिर से लिखें: वह तभी इस्तेमाल हुआ माना जाता है जब पासकी बन जाती है।",
    ),
    ("Check the installation", "इंस्टॉलेशन की जाँच करें"),
    // guide.rs
    ("Name", "नाम"),
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
    ("How it works:", "यह कैसे काम करता है:"),
    (
        "You give the Hermes a name here.",
        "आप यहाँ Hermes को एक नाम देते हैं।",
    ),
    (
        "The Hermes downloads the connector from this hub and answers with a verification code of 6 digits.",
        "Hermes इस हब से कनेक्टर डाउनलोड करता है और 6 अंकों के सत्यापन कोड के साथ जवाब देता है।",
    ),
    (
        "You type the code here and approve. Done: the Hermes shows as connected as soon as its connector confirms.",
        "आप कोड यहाँ लिखते हैं और स्वीकृत करते हैं। बस: उसका कनेक्टर पुष्टि करते ही Hermes कनेक्टेड दिखने लगता है।",
    ),
    (
        "Let's connect your first Hermes",
        "आइए आपका पहला Hermes कनेक्ट करें",
    ),
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
        "the hub gives notice when a message arrives (requires reading). Turning it off also turns it off in the chats of this access that have their own rule.",
        "संदेश आने पर हब सूचना देता है (पढ़ना ज़रूरी है)। इसे बंद करने पर यह इस एक्सेस की उन चैट में भी बंद हो जाता है जिनका अपना नियम है।",
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
    (
        "the engine refused the key of the hub. Restart the engine: it reads its key only when it starts.",
        "इंजन ने हब की कुंजी अस्वीकार कर दी। इंजन को रीस्टार्ट करें: वह अपनी कुंजी केवल शुरू होते समय पढ़ता है।",
    ),
    (
        "the engine refused the key of the hub. Set its key again with {command}.",
        "इंजन ने हब की कुंजी अस्वीकार कर दी। उसकी कुंजी {command} से फिर सेट करें।",
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
        "Too many attempts right now. Wait a minute and try again.",
        "अभी बहुत ज़्यादा प्रयास हुए हैं। एक मिनट रुकें और फिर से कोशिश करें।",
    ),
    // check.rs
    ("ok", "ठीक"),
    ("check", "देखें"),
    ("attention", "ध्यान दें"),
    ("later", "बाद में"),
    ("failed", "विफल"),
    ("Panel address", "पैनल का पता"),
    ("Connectors address", "कनेक्टरों का पता"),
    ("Your address", "आपका पता"),
    ("Database password", "डेटाबेस का पासवर्ड"),
    ("Data folder", "डेटा फ़ोल्डर"),
    ("WhatsApp engine", "WhatsApp इंजन"),
    ("Your passkey works here.", "आपकी पासकी यहाँ काम करती है।"),
    (
        "Open it: it should say it is the address of the connectors.",
        "इसे खोलें: उसे बताना चाहिए कि यह कनेक्टरों का पता है।",
    ),
    (
        "The hub sees you at {address}. Is it yours? If not, the proxy is not passing the real address.",
        "हब आपको {address} पर देखता है। क्या यह आपका है? अगर नहीं, तो प्रॉक्सी असली पता आगे नहीं भेज रहा।",
    ),
    (
        "A connection from {address}, which is not in ARAUTO_TRUSTED_PROXIES, brought a forwarded address. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "{address} ARAUTO_TRUSTED_PROXIES में नहीं है, फिर भी वहाँ से आए एक कनेक्शन में आगे भेजा गया पता था। उस कनेक्शन का पता ARAUTO_TRUSTED_PROXIES में जोड़ें और फिर से डिप्लॉय करें।",
    ),
    (
        "The hub sees you at {address}, an address of an internal network: the proxy is not trusted yet. Add it to ARAUTO_TRUSTED_PROXIES and deploy again.",
        "हब आपको {address} पर देखता है, जो एक आंतरिक नेटवर्क का पता है: प्रॉक्सी पर अभी भरोसा नहीं किया गया है। इसे ARAUTO_TRUSTED_PROXIES में जोड़ें और फिर से डिप्लॉय करें।",
    ),
    (
        "Replaced by one nobody saw.",
        "ऐसे पासवर्ड से बदल दिया गया जिसे किसी ने नहीं देखा।",
    ),
    (
        "Not replaced by the hub: this installation sets the password of the database itself.",
        "हब ने नहीं बदला: यह इंस्टॉलेशन डेटाबेस का पासवर्ड खुद तय करता है।",
    ),
    (
        "The replacement failed. See the log of the hub, then run {command}.",
        "बदलना विफल रहा। हब का लॉग देखें, फिर {command} चलाएँ।",
    ),
    ("Ready for your Hermes.", "आपके Hermes के लिए तैयार।"),
    (
        "The downloads folder is empty: a Hermes cannot be connected through the panel. Rebuild the image of the hub.",
        "डाउनलोड फ़ोल्डर खाली है: पैनल से Hermes कनेक्ट नहीं किया जा सकता। हब की इमेज फिर से बनाएँ।",
    ),
    (
        "It holds the master key and the password of the database. Back it up: without it, the hub cannot open what it kept encrypted.",
        "इसमें मास्टर कुंजी और डेटाबेस का पासवर्ड रहता है। इसका बैकअप रखें: इसके बिना हब वह नहीं खोल सकता जो उसने एन्क्रिप्ट करके रखा है।",
    ),
    ("Configured.", "सेट है।"),
    (
        "A Hermes can be connected without it; WhatsApp numbers need it.",
        "इसके बिना भी Hermes कनेक्ट किया जा सकता है; WhatsApp नंबरों के लिए यह ज़रूरी है।",
    ),
    (
        "The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out.",
        "हब ने कुछ चीज़ें अपने आप जाँचीं। यहाँ से कुछ भी नहीं बदलता: यह सिर्फ़ इसलिए है कि आपको पता रहे कि नतीजा क्या रहा।",
    ),
    // installation.rs
    (
        "Step 3 · Let's check the installation",
        "चरण 3 · आइए इंस्टॉलेशन की जाँच करें",
    ),
    (
        "All set: your passkey was created!",
        "सब तैयार: आपकी पासकी बन गई!",
    ),
    ("The same by command: {command}", "कमांड से भी यही: {command}"),
    ("Connect my first Hermes", "मेरा पहला Hermes कनेक्ट करें"),
    ("Do it later", "बाद में करें"),
    (
        "I am asking you myself: connect this Hermes to our Arauto hub as \"{name}\".",
        "यह मेरा अपना अनुरोध है: इस Hermes को हमारे Arauto हब से \"{name}\" नाम से कनेक्ट करें।",
    ),
    (
        "Read {page} and follow it. Ask me if anything is unclear.",
        "{page} पढ़ें और उसके अनुसार करें। कुछ भी स्पष्ट न हो तो मुझसे पूछें।",
    ),
    ("Hub:", "हब:"),
    ("Hub key:", "हब की कुंजी:"),
    ("Connector sha256 ({system}):", "कनेक्टर sha256 ({system}):"),
    (
        "Invite (one use, until {deadline} UTC):",
        "आमंत्रण (एक बार के लिए, {deadline} UTC तक):",
    ),
    (
        "When it is done, send me the verification code and the lines the page asks for.",
        "काम पूरा होने पर मुझे सत्यापन कोड और वे पंक्तियाँ भेजें जो पेज माँगता है।",
    ),
    (
        "Send it as a new message, not as a reply to one of its notices. If it asks you to confirm, answer in your own words.",
        "इसे नए संदेश के रूप में भेजें, उसकी किसी सूचना के जवाब के रूप में नहीं। अगर वह पुष्टि माँगे, तो अपने शब्दों में जवाब दें।",
    ),
    ("Show the command instead", "इसके बजाय कमांड दिखाएँ"),
    (
        "Send one or the other, not both: they carry the same invite.",
        "दोनों में से एक ही भेजें, दोनों नहीं: दोनों में एक ही आमंत्रण है।",
    ),
    (
        "Click the message to copy it.",
        "संदेश को कॉपी करने के लिए उस पर क्लिक करें।",
    ),
    ("Message", "संदेश"),
    (
        "The hub draws an invite and writes a message with it, which you send in the command channel of that Hermes, and nowhere else.",
        "हब एक आमंत्रण बनाता है और उसके साथ एक संदेश लिखता है, जिसे आप उस Hermes के कमांड चैनल में भेजते हैं, और कहीं नहीं।",
    ),
    (
        "Lowercase letters, digits, hyphen and underscore, up to 32 characters, starting with a letter or a digit. The name appears in the message and does not change afterwards.",
        "छोटे अक्षर, अंक, हाइफ़न और अंडरस्कोर, अधिकतम 32 वर्ण, शुरुआत अक्षर या अंक से। नाम संदेश में दिखता है और बाद में बदलता नहीं।",
    ),
    (
        "The message tells the Hermes to download the connector from this hub, and the downloads folder is empty. Nothing was issued. Issue the invite of {name} with the {command} command, on the hub's server.",
        "संदेश Hermes को इस हब से कनेक्टर डाउनलोड करने को कहता है, और डाउनलोड फ़ोल्डर खाली है। कुछ भी जारी नहीं हुआ। {name} का आमंत्रण हब के सर्वर पर {command} कमांड से जारी करें।",
    ),
    ("Org chart", "संगठन चार्ट"),
    ("Who talks to [whom]", "कौन [किससे] बात करता है"),
    (
        "Drag a Hermes to change its level. Pull the purple dot of a card to another card to create a link. Click an arrow or a card to adjust it.",
        "स्तर बदलने के लिए किसी Hermes को खींचें। लिंक बनाने के लिए एक कार्ड की बैंगनी बिंदी को दूसरे कार्ड तक खींचें। किसी तीर या कार्ड पर क्लिक करके उसे ठीक करें।",
    ),
    ("View", "दृश्य"),
    ("Map", "नक्शा"),
    ("List", "सूची"),
    ("+ Level", "+ स्तर"),
    ("Undo", "पूर्ववत करें"),
    ("Publish", "प्रकाशित करें"),
    ("Draft: 1 change", "ड्राफ़्ट: 1 बदलाव"),
    ("Draft: {n} changes", "ड्राफ़्ट: {n} बदलाव"),
    (
        "From a higher level to a lower one: instruction",
        "ऊँचे स्तर से निचले स्तर की ओर: निर्देश",
    ),
    (
        "From a lower level to a higher one: request",
        "निचले स्तर से ऊँचे स्तर की ओर: अनुरोध",
    ),
    ("Same level: note", "समान स्तर: नोट"),
    (
        "The levels and the roles are only shown here: no Hermes is told them. A note is data, except on an instruction arrow with the follow mark: then the notice tells the lower Hermes to follow those new messages of the higher one, in that run, below its operator's instructions. Only a link lets one Hermes send a note to another.",
        "स्तर और भूमिकाएँ केवल यहीं दिखते हैं: किसी Hermes को इनके बारे में नहीं बताया जाता। नोट डेटा है, सिवाय उस निर्देश वाले तीर के जिस पर पालन का निशान हो: तब सूचना नीचे वाले Hermes से कहती है कि ऊपर वाले के उन नए संदेशों का उस रन में पालन करे, अपने ऑपरेटर के निर्देशों के नीचे रहते हुए। केवल लिंक ही एक Hermes को दूसरे को नोट भेजने देता है।",
    ),
    ("instruction", "निर्देश"),
    ("request", "अनुरोध"),
    ("note", "नोट"),
    ("Name of level {n}", "स्तर {n} का नाम"),
    ("Remove empty level", "खाली स्तर हटाएँ"),
    ("Move up", "ऊपर ले जाएँ"),
    ("Move down", "नीचे ले जाएँ"),
    (
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word.",
        "स्तर के नाम में 1 से 32 छोटे अक्षर, अंक, हाइफ़न या अंडरस्कोर होते हैं, वह अक्षर या अंक से शुरू होता है, और आरक्षित शब्द नहीं होता।",
    ),
    (
        "Another level already has this name.",
        "किसी दूसरे स्तर का नाम पहले से यही है।",
    ),
    (
        "The org chart has from 1 to {max} levels.",
        "संगठन चार्ट में 1 से {max} स्तर होते हैं।",
    ),
    (
        "Drag to another Hermes to create a link",
        "लिंक बनाने के लिए इसे किसी दूसरे Hermes तक खींचें",
    ),
    ("{alias} · answers", "{alias} · जवाब देता है"),
    ("{alias} · reads only", "{alias} · केवल पढ़ता है"),
    ("no number", "कोई नंबर नहीं"),
    ("not placed yet", "अभी जगह तय नहीं"),
    (
        "{name}, level {level}, {state}",
        "{name}, स्तर {level}, {state}",
    ),
    (
        "Not placed yet: {names}. They are shown in the lowest level; publish to keep them there.",
        "अभी जगह तय नहीं: {names}। वे सबसे निचले स्तर में दिख रहे हैं; वहाँ बनाए रखने के लिए प्रकाशित करें।",
    ),
    ("Kind: {kind}", "प्रकार: {kind}"),
    ("Wakes {name}", "{name} को जगाता है"),
    ("Create the way back", "वापसी का लिंक बनाएँ"),
    ("Remove the way back", "वापसी का लिंक हटाएँ"),
    ("Remove", "हटाएँ"),
    ("This link already exists.", "यह लिंक पहले से मौजूद है।"),
    ("Role of this Hermes", "इस Hermes की भूमिका"),
    (
        "For your own reading: in this version no Hermes sees it. Up to 280 characters, in one line.",
        "आपके अपने पढ़ने के लिए: इस संस्करण में कोई Hermes इसे नहीं देखता। अधिकतम 280 अक्षर, एक पंक्ति में।",
    ),
    ("Level", "स्तर"),
    ("Move left", "बाईं ओर ले जाएँ"),
    ("Move right", "दाईं ओर ले जाएँ"),
    ("Change them in Accesses", "इन्हें पहुँच टैब में बदलें"),
    ("No access to any number.", "किसी भी नंबर तक पहुँच नहीं है।"),
    ("Link to", "किससे लिंक करें"),
    ("Add link", "लिंक जोड़ें"),
    ("Links of this Hermes", "इस Hermes के लिंक"),
    ("Levels", "स्तर"),
    ("Links", "लिंक"),
    ("Role", "भूमिका"),
    ("From", "प्रेषक"),
    ("To", "प्राप्तकर्ता"),
    ("Kind", "प्रकार"),
    ("Wakes", "जगाता है"),
    ("nobody", "कोई नहीं"),
    ("No link.", "कोई लिंक नहीं।"),
    ("Publish these changes?", "क्या ये बदलाव प्रकाशित करें?"),
    (
        "Links are security rules. They only apply after you publish.",
        "लिंक सुरक्षा नियम हैं। वे प्रकाशित करने के बाद ही लागू होते हैं।",
    ),
    (
        "A link lets a request cross numbers: what reaches one Hermes on one number can become a note to the other, who acts on its own numbers.",
        "लिंक किसी अनुरोध को नंबरों के पार जाने देता है: जो अनुरोध एक नंबर के ज़रिए एक Hermes तक पहुँचता है, वह दूसरे Hermes के लिए नोट बन सकता है, जो अपने ही नंबरों पर काम करता है।",
    ),
    ("Back", "वापस जाएँ"),
    (
        "New link: {from} → {to} ({kind})",
        "नया लिंक: {from} → {to} ({kind})",
    ),
    (
        "New link: {from} → {to} ({kind}), wakes {to}",
        "नया लिंक: {from} → {to} ({kind}), {to} को जगाता है",
    ),
    (
        "Link {from} → {to}: now wakes {to}",
        "लिंक {from} → {to}: अब {to} को जगाता है",
    ),
    (
        "Link {from} → {to}: no longer wakes {to}",
        "लिंक {from} → {to}: अब {to} को नहीं जगाता",
    ),
    (
        "Link removed: {from} → {to}",
        "लिंक हटाया गया: {from} → {to}",
    ),
    (
        "{name} changes level: {before} → {after}",
        "{name} का स्तर बदलता है: {before} → {after}",
    ),
    (
        "{name} is placed in level {after}",
        "{name} को स्तर {after} में रखा गया है",
    ),
    ("Role of {name} changed", "{name} की भूमिका बदली गई"),
    ("Places on the map changed: {n}", "नक्शे पर बदली हुई जगहें: {n}"),
    ("New level: {name}", "नया स्तर: {name}"),
    (
        "Level renamed: {before} → {after}",
        "स्तर का नाम बदला गया: {before} → {after}",
    ),
    ("Level removed: {name}", "स्तर हटाया गया: {name}"),
    ("Order of the levels: {names}", "स्तरों का क्रम: {names}"),
    ("Published.", "प्रकाशित हो गया।"),
    (
        "Someone changed the org chart while you were editing: another publish, a link changed by the command, or a Hermes added or removed. Nothing was published. Your draft is kept: check the changes and publish again.",
        "आपके संपादन के दौरान किसी ने संगठन चार्ट बदल दिया: कोई दूसरा प्रकाशन, कमांड से बदला गया कोई लिंक, या जोड़ा या हटाया गया कोई Hermes। कुछ भी प्रकाशित नहीं हुआ। आपका ड्राफ़्ट सुरक्षित है: बदलाव जाँचें और फिर से प्रकाशित करें।",
    ),
    ("Nothing was published.", "कुछ भी प्रकाशित नहीं हुआ।"),
    ("Two levels have the same name.", "दो स्तरों का नाम एक ही है।"),
    (
        "A role has at most 280 characters, in one line, without control or direction characters.",
        "भूमिका में अधिकतम 280 अक्षर हो सकते हैं, एक पंक्ति में, बिना नियंत्रण या दिशा वाले अक्षरों के।",
    ),
    (
        "A place on the map is out of range.",
        "नक्शे पर एक जगह सीमा से बाहर है।",
    ),
    (
        "The draft names a Hermes or a level that does not exist, or leaves a Hermes out.",
        "ड्राफ़्ट में ऐसा Hermes या स्तर है जो मौजूद नहीं है, या कोई Hermes छूटा हुआ है।",
    ),
    (
        "A link joins two different Hermes, once in each direction.",
        "लिंक दो अलग-अलग Hermes को जोड़ता है, हर दिशा में एक बार।",
    ),
    ("The draft could not be read.", "ड्राफ़्ट पढ़ा नहीं जा सका।"),
    (
        "The draft is too large to publish.",
        "ड्राफ़्ट प्रकाशित करने के लिए बहुत बड़ा है।",
    ),
    (
        "Something failed in the hub. Nothing was published. See the hub's log.",
        "हब में कुछ गड़बड़ हुई। कुछ भी प्रकाशित नहीं हुआ। हब का लॉग देखें।",
    ),
    (
        "Editing the org chart needs the script of the panel. Without it, this is the org chart as it is published.",
        "संगठन चार्ट संपादित करने के लिए पैनल की स्क्रिप्ट चाहिए। उसके बिना यह संगठन चार्ट वैसा ही है जैसा प्रकाशित है।",
    ),
    (
        "No Hermes yet. Connect the first one in the Hermes tab.",
        "अभी कोई Hermes नहीं है। पहला Hermes, Hermes टैब में कनेक्ट करें।",
    ),
    ("Connect WhatsApp (WAHA)", "WhatsApp कनेक्ट करें (WAHA)"),
    (
        "Connect WhatsApp (native · in test)",
        "WhatsApp कनेक्ट करें (नेटिव · परीक्षण में)",
    ),
    (
        "No number yet. Add the first one with the \"Connect WhatsApp (WAHA)\" button.",
        "अभी कोई नंबर नहीं है। पहला नंबर \"WhatsApp कनेक्ट करें (WAHA)\" बटन से जोड़ें।",
    ),
    (
        "In this version the panel connects and revokes Hermes, pairs and manages WhatsApp numbers on WAHA, changes the accesses of each number and edits the org chart with its links. Choosing chats is still done with the {command} command, on the hub's server.",
        "इस संस्करण में पैनल Hermes को कनेक्ट और निरस्त करता है, WAHA पर WhatsApp नंबर पेयर और प्रबंधित करता है, हर नंबर की पहुँच बदलता है और लिंक के साथ ऑर्ग चार्ट संपादित करता है। चैट चुनना अभी भी {command} कमांड से होता है, हब के सर्वर पर।",
    ),
    ("Pair", "पेयर करें"),
    ("Pair again", "फिर से पेयर करें"),
    ("Continue pairing", "पेयरिंग जारी रखें"),
    ("See the pairing", "पेयरिंग देखें"),
    ("Disconnect", "डिस्कनेक्ट करें"),
    ("Restart", "रीस्टार्ट करें"),
    ("Decide", "तय करें"),
    ("Number {alias}: {state}.", "नंबर {alias}: {state}।"),
    (
        "The hub changed the number {alias}, but WAHA did not answer: its session there may still be as it was. Check WAHA with {command} on the hub's server.",
        "हब ने नंबर {alias} बदल दिया, लेकिन WAHA ने जवाब नहीं दिया: वहाँ उसका सेशन शायद पहले जैसा ही हो। हब के सर्वर पर {command} से WAHA जाँचें।",
    ),
    ("Method", "तरीका"),
    ("Phone", "फ़ोन"),
    (
        "How the panel and the Hermes call this number: lowercase letters, digits, hyphen and underscore, up to 32. An alias is never reused.",
        "पैनल और Hermes इस नंबर को किस नाम से बुलाते हैं: छोटे अक्षर, अंक, हाइफ़न और अंडरस्कोर, अधिकतम 32। उपनाम कभी दोबारा इस्तेमाल नहीं होता।",
    ),
    ("Number with the country code", "देश कोड के साथ नंबर"),
    (
        "Like +55 11 9XXXX-XXXX. Digits, spaces, hyphens and parentheses only.",
        "जैसे +55 11 9XXXX-XXXX। केवल अंक, स्पेस, हाइफ़न और कोष्ठक।",
    ),
    ("Before the first number", "पहले नंबर से पहले"),
    (
        "The engine is an unofficial WhatsApp client. WhatsApp may block the number. Use numbers you can afford to lose.",
        "इंजन WhatsApp का अनौपचारिक क्लाइंट है। WhatsApp नंबर को ब्लॉक कर सकता है। ऐसे नंबर इस्तेमाल करें जिन्हें खोना आपको मंज़ूर हो।",
    ),
    (
        "I understand and accept. The hub records this acceptance in the audit.",
        "मैं समझता/समझती हूँ और स्वीकार करता/करती हूँ। हब यह स्वीकृति ऑडिट में दर्ज करता है।",
    ),
    (
        "The number is paired through WAHA, the engine set on this hub.",
        "नंबर WAHA से पेयर होता है, जो इस हब पर सेट इंजन है।",
    ),
    ("Add and pair", "नंबर जोड़ें और पेयर करें"),
    (
        "WAHA has not answered the hub yet. You can add the number; pairing it needs WAHA to answer.",
        "WAHA ने अभी तक हब को जवाब नहीं दिया है। आप नंबर जोड़ सकते हैं; उसे पेयर करने के लिए WAHA का जवाब देना ज़रूरी है।",
    ),
    (
        "There is no WhatsApp engine on this hub yet. Set WAHA on the hub's server with {command}, then its key with {key_command}. The key is never typed in the panel.",
        "इस हब पर अभी कोई WhatsApp इंजन नहीं है। हब के सर्वर पर {command} से WAHA सेट करें, फिर {key_command} से उसकी कुंजी। कुंजी पैनल में कभी नहीं लिखी जाती।",
    ),
    (
        "The engine has no key yet. Set it on the hub's server with {command}. The key is never typed in the panel.",
        "इंजन की अभी कोई कुंजी नहीं है। हब के सर्वर पर इसे {command} से सेट करें। कुंजी पैनल में कभी नहीं लिखी जाती।",
    ),
    (
        "The alias has from 1 to 32 characters.",
        "उपनाम 1 से 32 वर्णों का होता है।",
    ),
    (
        "The alias starts with a lowercase letter or a digit.",
        "उपनाम किसी छोटे अक्षर या अंक से शुरू होता है।",
    ),
    (
        "That alias is reserved. Choose another.",
        "यह उपनाम आरक्षित है। दूसरा चुनें।",
    ),
    (
        "That alias is taken, now or by a number that was removed, and an alias is never reused. Choose another.",
        "यह उपनाम अभी इस्तेमाल में है, या किसी हटाए गए नंबर का था, और उपनाम कभी दोबारा इस्तेमाल नहीं होता। दूसरा चुनें।",
    ),
    (
        "Type the number with the country code: from 8 to 15 digits, like +55 11 9XXXX-XXXX.",
        "नंबर देश कोड के साथ लिखें: 8 से 15 अंक, जैसे +55 11 9XXXX-XXXX।",
    ),
    (
        "That number is already a WhatsApp of this hub.",
        "यह नंबर पहले से इस हब का WhatsApp है।",
    ),
    (
        "Read the sentence about the risk and mark that you accept it. Nothing was added.",
        "जोखिम वाला वाक्य पढ़ें और चिह्नित करें कि आप उसे स्वीकार करते हैं। कुछ भी नहीं जोड़ा गया।",
    ),
    ("Pair {alias}", "{alias} को पेयर करें"),
    (
        "Have the phone of {phone} at hand, with WhatsApp open. The pairing lasts up to {minutes} minutes.",
        "{phone} का फ़ोन हाथ में रखें, WhatsApp खुला हुआ। पेयरिंग अधिकतम {minutes} मिनट चलती है।",
    ),
    ("How will the phone pair?", "फ़ोन कैसे पेयर होगा?"),
    ("QR code", "QR कोड"),
    (
        "scan it with the phone's camera, from inside WhatsApp",
        "फ़ोन के कैमरे से, WhatsApp के अंदर से स्कैन करें",
    ),
    ("Pairing code", "पेयरिंग कोड"),
    (
        "type a code on the phone, for when it cannot scan this screen",
        "फ़ोन पर एक कोड लिखें, जब वह इस स्क्रीन को स्कैन न कर सके",
    ),
    ("Start pairing", "पेयरिंग शुरू करें"),
    (
        "WAHA has not created the session of this number yet. Starting the pairing tries again.",
        "WAHA ने अभी तक इस नंबर का सेशन नहीं बनाया है। पेयरिंग शुरू करने पर वह फिर कोशिश करता है।",
    ),
    (
        "Choose QR code or pairing code. Nothing was started.",
        "QR कोड या पेयरिंग कोड चुनें। कुछ भी शुरू नहीं हुआ।",
    ),
    (
        "{n} numbers are being paired right now, the most at once. Wait for one to end, or cancel one.",
        "अभी {n} नंबर पेयर हो रहे हैं, जो एक साथ अधिकतम है। किसी एक के खत्म होने की प्रतीक्षा करें, या एक को रद्द करें।",
    ),
    (
        "This cannot be done while the number is in the state \"{state}\". Nothing was done.",
        "नंबर जब \"{state}\" स्थिति में है, तब यह नहीं किया जा सकता। कुछ भी नहीं किया गया।",
    ),
    ("Starting the pairing…", "पेयरिंग शुरू हो रही है…"),
    (
        "QR code to scan with the phone",
        "फ़ोन से स्कैन करने के लिए QR कोड",
    ),
    (
        "The QR changes every few seconds: this window shows the current one.",
        "QR हर कुछ सेकंड में बदलता है: यह विंडो मौजूदा QR दिखाती है।",
    ),
    (
        "The code works while this window counts down: {time} left. WhatsApp may end it sooner.",
        "कोड तब तक चलता है जब तक यह विंडो गिनती करती है: {time} बाकी। WhatsApp उसे पहले भी खत्म कर सकता है।",
    ),
    ("Time left to scan: {time}.", "स्कैन के लिए बचा समय: {time}।"),
    (
        "This window refreshes by itself.",
        "यह विंडो अपने आप ताज़ा होती है।",
    ),
    ("Refresh now", "अभी ताज़ा करें"),
    ("Cancel the pairing", "पेयरिंग रद्द करें"),
    (
        "The QR is too long to draw here. Cancel, and pair with a pairing code instead.",
        "QR यहाँ बनाने के लिए बहुत लंबा है। रद्द करें, और इसके बजाय पेयरिंग कोड से पेयर करें।",
    ),
    ("Paired: {phone}.", "पेयर हो गया: {phone}।"),
    (
        "The number {alias} is connected. Give a Hermes access to it in the Accesses tab.",
        "नंबर {alias} कनेक्टेड है। पहुँच टैब में किसी Hermes को इसकी पहुँच दें।",
    ),
    (
        "Nothing was scanned or typed in time. The number is not paired; pair it again with the phone at hand.",
        "समय पर कुछ भी स्कैन या टाइप नहीं हुआ। नंबर पेयर नहीं है; फ़ोन हाथ में रखकर फिर से पेयर करें।",
    ),
    (
        "WAHA reported a failure of the session. The number is not paired. Try again; if it fails again, check WAHA.",
        "WAHA ने सेशन की विफलता बताई। नंबर पेयर नहीं है। फिर कोशिश करें; दोबारा विफल हो तो WAHA जाँचें।",
    ),
    (
        "The pairing was cancelled. The number is not paired.",
        "पेयरिंग रद्द कर दी गई। नंबर पेयर नहीं है।",
    ),
    (
        "This number is being paired in another window. Only that window shows the QR or the code.",
        "यह नंबर किसी दूसरी विंडो में पेयर हो रहा है। केवल वही विंडो QR या कोड दिखाती है।",
    ),
    ("Cancel that pairing", "वह पेयरिंग रद्द करें"),
    (
        "This pairing is no longer running in the hub: it ended, or the hub restarted. The number is in the state \"{state}\".",
        "यह पेयरिंग अब हब में नहीं चल रही: वह खत्म हो गई, या हब रीस्टार्ट हुआ। नंबर \"{state}\" स्थिति में है।",
    ),
    (
        "The hub could not reach WAHA. Check that it is running and on the hub's network.",
        "हब WAHA तक नहीं पहुँच सका। जाँचें कि वह चल रहा है और हब के नेटवर्क पर है।",
    ),
    (
        "WAHA refused the key the hub has. If the installation brought the engine, restart the engine: it reads its key only when it starts. If you set the engine up yourself, set the right key on the hub's server with {command}.",
        "WAHA ने हब की कुंजी अस्वीकार कर दी। अगर इंजन इंस्टॉलेशन के साथ आया है, तो इंजन को रीस्टार्ट करें: वह अपनी कुंजी केवल शुरू होते समय पढ़ता है। अगर आपने इंजन खुद सेट किया है, तो हब के सर्वर पर {command} से सही कुंजी सेट करें।",
    ),
    (
        "WAHA answered with an error, or not in time. Try again in a moment; {command} on the hub's server says more.",
        "WAHA ने त्रुटि के साथ जवाब दिया, या समय पर नहीं दिया। थोड़ी देर में फिर कोशिश करें; हब के सर्वर पर {command} और बताता है।",
    ),
    (
        "WhatsApp is limiting this number. Try again later.",
        "WhatsApp इस नंबर को सीमित कर रहा है। बाद में फिर कोशिश करें।",
    ),
    (
        "WAHA refused the request. See the hub's log.",
        "WAHA ने अनुरोध अस्वीकार कर दिया। हब का लॉग देखें।",
    ),
    (
        "Something failed in the hub. See the hub's log and check the number in the list.",
        "हब में कुछ विफल हो गया। हब का लॉग देखें और सूची में नंबर जाँचें।",
    ),
    ("{alias}: another phone paired", "{alias}: दूसरा फ़ोन पेयर हुआ"),
    (
        "The phone that paired belongs to a number different from the one given.",
        "जो फ़ोन पेयर हुआ, वह दिए गए नंबर से अलग नंबर का है।",
    ),
    (
        "Until you decide, the number is in the state \"Number swapped\" and its accesses stay stopped.",
        "आपके तय करने तक नंबर \"नंबर बदला हुआ\" स्थिति में रहता है और उसकी पहुँच रुकी रहती है।",
    ),
    ("Keep this one", "इसे रखें"),
    (
        "Undoing takes WAHA out of that phone's WhatsApp (it leaves its linked devices) and erases the history WAHA keeps of this number.",
        "पूर्ववत करने पर WAHA उस फ़ोन के WhatsApp से बाहर हो जाता है (लिंक किए गए डिवाइस से हट जाता है) और WAHA इस नंबर का जो इतिहास रखता है, वह मिट जाता है।",
    ),
    (
        "I understand that WAHA erases its history of this number.",
        "मैं समझता/समझती हूँ कि WAHA इस नंबर का अपना इतिहास मिटा देता है।",
    ),
    (
        "Mark that you understand what undoing erases. Nothing was done.",
        "चिह्नित करें कि आप समझते हैं कि पूर्ववत करने पर क्या मिटता है। कुछ भी नहीं किया गया।",
    ),
    (
        "The phone that paired is already another number of this hub. Undo this pairing, or remove the other number first.",
        "जो फ़ोन पेयर हुआ, वह पहले से इस हब का दूसरा नंबर है। इस पेयरिंग को पूर्ववत करें, या पहले दूसरा नंबर हटाएँ।",
    ),
    ("Disconnect {alias}", "{alias} को डिस्कनेक्ट करें"),
    (
        "Disconnecting stops the session of this number in WAHA. It does not take WAHA out of WhatsApp, and WAHA keeps its history. While it is disconnected, no Hermes reads or sends through it. To use it again, pair it again.",
        "डिस्कनेक्ट करने से WAHA में इस नंबर का सेशन रुक जाता है। इससे WAHA WhatsApp से बाहर नहीं होता, और WAHA इतिहास रखता है। डिस्कनेक्ट रहने तक कोई Hermes इसके ज़रिए न पढ़ता है, न भेजता है। इसे फिर इस्तेमाल करने के लिए फिर से पेयर करें।",
    ),
    (
        "Type the alias {alias} to confirm.",
        "पुष्टि के लिए उपनाम {alias} लिखें।",
    ),
    (
        "The alias typed does not match. Nothing was done.",
        "लिखा गया उपनाम मेल नहीं खाता। कुछ भी नहीं किया गया।",
    ),
    ("Remove {alias}", "{alias} को हटाएँ"),
    (
        "Removing ends every access to this number now. Its alias is never used again.",
        "हटाने से इस नंबर की हर पहुँच अभी खत्म हो जाती है। इसका उपनाम दोबारा कभी इस्तेमाल नहीं होता।",
    ),
    (
        "What happens to its session in WAHA",
        "WAHA में इसके सेशन का क्या होगा",
    ),
    (
        "Stop it and keep it in WAHA, with the history WAHA keeps",
        "इसे रोकें और WAHA में रहने दें, उस इतिहास के साथ जो WAHA रखता है",
    ),
    (
        "Take WAHA out of WhatsApp and delete it: WAHA erases the history it keeps",
        "WAHA को WhatsApp से बाहर करें और इसे मिटा दें: WAHA अपना रखा इतिहास मिटा देता है",
    ),
    (
        "Arauto has no message archive yet: the history is only what WAHA keeps.",
        "Arauto के पास अभी संदेशों का संग्रह नहीं है: इतिहास केवल वही है जो WAHA रखता है।",
    ),
    (
        "Choose what happens to its session in WAHA. Nothing was done.",
        "चुनें कि WAHA में इसके सेशन का क्या होगा। कुछ भी नहीं किया गया।",
    ),
    ("In test", "परीक्षण में"),
    (
        "A WhatsApp engine built into Arauto is being studied. It is not part of this version, and this window does nothing.",
        "Arauto में बना एक WhatsApp इंजन अध्ययन में है। वह इस संस्करण का हिस्सा नहीं है, और यह विंडो कुछ नहीं करती।",
    ),
    (
        "For now, numbers are paired through WAHA, with the \"Connect WhatsApp (WAHA)\" button.",
        "अभी के लिए, नंबर \"WhatsApp कनेक्ट करें (WAHA)\" बटन से WAHA के ज़रिए पेयर होते हैं।",
    ),
    ("Open the WAHA window", "WAHA की विंडो खोलें"),
    ("Try again", "फिर कोशिश करें"),
    (
        "Scan with the phone: WhatsApp › Linked devices › Link a device.",
        "फ़ोन से स्कैन करें: WhatsApp › लिंक किए गए डिवाइस › डिवाइस लिंक करें।",
    ),
    (
        "On the phone: WhatsApp › Linked devices › Link a device › Link with phone number instead, and type this code:",
        "फ़ोन पर: WhatsApp › लिंक किए गए डिवाइस › डिवाइस लिंक करें › इसके बजाय फ़ोन नंबर से लिंक करें, और यह कोड लिखें:",
    ),
    // decision 0045
    (
        "With this mark, a note on this link wakes {name}, within the notice limits.",
        "इस निशान के साथ, इस लिंक से भेजा गया नोट {name} को जगाता है, सूचना की सीमाओं के भीतर।",
    ),
    // decision 0050
    (
        "{to} follows the instructions of {from}",
        "{to} {from} के निर्देशों का पालन करता है",
    ),
    (
        "With this mark, the notice of a note tells {to} to do what {from} asks in that run, without waiting for its operator, unless its operator forbids it. Only on an instruction arrow, only with the wake mark; {to} needs the link back to report. It is a sentence to an AI, not a control.",
        "इस निशान के साथ, नोट की सूचना {to} से कहती है कि उस रन में वह वही करे जो {from} कहे, अपने ऑपरेटर का इंतज़ार किए बिना, जब तक उसका ऑपरेटर मना न करे। केवल निर्देश वाले तीर पर, केवल जगाने के निशान के साथ; रिपोर्ट करने के लिए {to} को लौटने वाला लिंक चाहिए। यह एक AI को दिया गया वाक्य है, नियंत्रण नहीं।",
    ),
    (
        "Link {from} → {to}: {to} now follows the instructions of {from}",
        "लिंक {from} → {to}: अब {to}, {from} के निर्देशों का पालन करता है",
    ),
    (
        "Link {from} → {to}: {to} no longer follows the instructions of {from}",
        "लिंक {from} → {to}: अब {to}, {from} के निर्देशों का पालन नहीं करता",
    ),
    (
        "A Hermes that follows another does what it asks without waiting for its operator. Whoever misleads the higher Hermes, through WhatsApp or a note, reaches the lower one too, whoever controls the hub can write as any Hermes, and whoever holds a panel session can set these marks. A connector of an older version keeps treating these notes as data.",
        "जो Hermes किसी दूसरे का पालन करता है, वह अपने ऑपरेटर का इंतज़ार किए बिना वही करता है जो दूसरा कहता है। जो ऊपर वाले Hermes को WhatsApp या किसी नोट से भटकाता है, वह नीचे वाले तक भी पहुँच जाता है; जो हब को नियंत्रित करता है, वह किसी भी Hermes की तरह लिख सकता है; और जिसके पास पैनल सत्र है, वह ये निशान लगा सकता है। पुराने संस्करण का कनेक्टर इन नोटों को अब भी डेटा ही मानता है।",
    ),
    (
        "A follow mark goes only from a higher level to a lower one, on a link that wakes.",
        "पालन का निशान केवल ऊँचे स्तर से नीचे के स्तर तक जाता है, और केवल ऐसे लिंक पर जो जगाता है।",
    ),
    // orders.rs, and the actions and tags of the lists (decision 0051)
    ("Who gives it orders", "उसे आदेश कौन देता है"),
    ("not recorded", "दर्ज नहीं"),
    ("its own chat", "अपनी ही चैट"),
    ("{n} telephones", "{n} फ़ोन"),
    ("1 telephone", "1 फ़ोन"),
    ("Orders", "आदेश"),
    (
        "Who gives {hermes} orders on WhatsApp",
        "WhatsApp पर {hermes} को आदेश कौन देता है",
    ),
    (
        "This is the number {alias} in Arauto.",
        "यह Arauto में नंबर {alias} है।",
    ),
    ("Orders come from", "आदेश कहाँ से आते हैं"),
    (
        "Its own WhatsApp, in the chat with itself (self-chat mode)",
        "उसका अपना WhatsApp, खुद से की गई चैट में (self-chat मोड)",
    ),
    (
        "Arauto refuses every Hermes, {hermes} included, writing in the chat where {hermes} takes these orders. Who reads it follows the accesses.",
        "Arauto हर Hermes को, {hermes} को भी, उस चैट में लिखने से रोकता है जहाँ {hermes} ये आदेश लेता है। पढ़ना एक्सेस के अनुसार चलता है।",
    ),
    (
        "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where {hermes} takes orders could pass for you, or for whoever gives them.",
        "यहाँ कुछ दर्ज न हो तो Arauto कुछ नहीं रोकता: जो Hermes उस चैट में लिख सकता है जहाँ {hermes} आदेश लेता है, वह आपके या आदेश देने वाले के रूप में पेश आ सकता है।",
    ),
    (
        "{other} already takes orders on this telephone.",
        "{other} पहले से इसी फ़ोन पर आदेश लेता है।",
    ),
    (
        "Something failed in the hub. See the hub's log and check the record of the Hermes.",
        "हब में कुछ विफल हुआ। हब का लॉग देखें और Hermes की प्रविष्टि जाँचें।",
    ),
    (
        "A Hermes could give {hermes} orders",
        "कोई Hermes {hermes} को आदेश दे सकता है",
    ),
    (
        "Nothing is blocked: any Hermes that can write in the chat where {hermes} takes orders can pass for the person who gives them.",
        "कुछ भी अवरुद्ध नहीं है: जो भी Hermes उस चैट में लिख सकता है जहाँ {hermes} आदेश लेता है, वह आदेश देने वाले व्यक्ति के रूप में पेश आ सकता है।",
    ),
    ("Go back", "वापस जाएँ"),
    ("gives orders to {hermes}", "{hermes} को आदेश देता है"),
    ("{hermes} takes orders here", "{hermes} यहाँ आदेश लेता है"),
    (
        "Orders on WhatsApp for {alias}",
        "{alias} के लिए WhatsApp पर आदेश",
    ),
    ("Its WhatsApp", "उसका WhatsApp"),
    (
        "The telephone {hermes}'s own WhatsApp runs on: {hermes} takes orders there.",
        "वह फ़ोन जिस पर {hermes} का अपना WhatsApp चलता है: {hermes} वहीं आदेश लेता है।",
    ),
    ("It has no WhatsApp", "उसके पास WhatsApp नहीं है"),
    ("A number of Arauto", "Arauto का एक नंबर"),
    ("Choose a number", "एक नंबर चुनें"),
    ("Another telephone", "कोई और फ़ोन"),
    (
        "Read only when Another telephone is chosen. Start with + and the country code, like +55 11 9XXXX-XXXX.",
        "केवल तब पढ़ा जाता है जब कोई और फ़ोन चुना हो। + और देश कोड से शुरू करें, जैसे +55 11 9XXXX-XXXX।",
    ),
    ("These telephones", "ये फ़ोन"),
    ("its WhatsApp", "उसका WhatsApp"),
    ("Other telephones", "अन्य फ़ोन"),
    (
        "Telephones that are not numbers of Arauto, one per line. At most 16 telephones in all.",
        "वे फ़ोन जो Arauto के नंबर नहीं हैं, हर पंक्ति में एक। कुल मिलाकर अधिकतम 16 फ़ोन।",
    ),
    (
        "{alias} is chosen. Nothing is saved until you save.",
        "{alias} चुना गया है। जब तक आप सहेजें नहीं, कुछ भी सहेजा नहीं जाता।",
    ),
    (
        "{alias} is chosen as its WhatsApp, in place of {before}. Nothing is saved until you save.",
        "{before} की जगह {alias} को उसके WhatsApp के रूप में चुना गया है। जब तक आप सहेजें नहीं, कुछ भी सहेजा नहीं जाता।",
    ),
    (
        "Choose an option for its WhatsApp. Nothing was saved.",
        "उसके WhatsApp के लिए कोई विकल्प चुनें। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "{alias} is not a number of Arauto now. Nothing was saved.",
        "{alias} अब Arauto का नंबर नहीं है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Type the other telephone its WhatsApp runs on, or choose another option. Nothing was saved.",
        "वह दूसरा फ़ोन लिखें जिस पर उसका WhatsApp चलता है, या कोई और विकल्प चुनें। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "The other telephone of its WhatsApp is not a telephone: 8 to 15 digits, with + and the country code first. Nothing was saved.",
        "उसके WhatsApp का दूसरा फ़ोन मान्य फ़ोन नहीं है: 8 से 15 अंक, पहले + और देश कोड के साथ। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Line {n} of Other telephones is not a telephone. Nothing was saved.",
        "अन्य फ़ोन की पंक्ति {n} फ़ोन नहीं है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Its own chat and a list of telephones are two answers: clear the list, or choose These telephones. Nothing was saved.",
        "उसकी अपनी चैट और फ़ोनों की सूची दो अलग उत्तर हैं: सूची खाली करें, या ये फ़ोन चुनें। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Choose {hermes}'s WhatsApp first: a chat has two ends. Nothing was saved.",
        "पहले {hermes} का WhatsApp चुनें: चैट के दो सिरे होते हैं। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "Its WhatsApp cannot also give it orders: that chat is the self-chat mode. Nothing was saved.",
        "उसका WhatsApp उसे आदेश भी नहीं दे सकता: वह चैट self-chat मोड है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "At most 16 telephones can give it orders; these are {n}. Nothing was saved.",
        "अधिकतम 16 फ़ोन उसे आदेश दे सकते हैं; ये {n} हैं। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "1 typed telephone needs a look. Nothing was saved.",
        "लिखे गए 1 फ़ोन को देखना ज़रूरी है। कुछ भी सहेजा नहीं गया।",
    ),
    (
        "{n} typed telephones need a look. Nothing was saved.",
        "लिखे गए {n} फ़ोनों को देखना ज़रूरी है। कुछ भी सहेजा नहीं गया।",
    ),
    ("Line {n}", "पंक्ति {n}"),
    (
        "Not a telephone: 8 to 15 digits, with + and the country code first.",
        "फ़ोन नहीं है: 8 से 15 अंक, पहले + और देश कोड के साथ।",
    ),
    ("Not a number of Arauto.", "Arauto का नंबर नहीं है।"),
    (
        "The same telephone as line {n}: it counts once.",
        "यह पंक्ति {n} वाला ही फ़ोन है: यह एक बार गिना जाता है।",
    ),
    (
        "The same telephone as {alias}, chosen above: it counts once.",
        "यह ऊपर चुने गए {alias} वाला ही फ़ोन है: यह एक बार गिना जाता है।",
    ),
    (
        "This is its WhatsApp: it cannot also give it orders.",
        "यह उसका WhatsApp है: यह उसे आदेश भी नहीं दे सकता।",
    ),
    (
        "Typed without +, so the first digits were read as the country code. If that is not its country, type + and the country code first. Saving again keeps it as read.",
        "+ के बिना लिखा गया, इसलिए पहले अंक देश कोड के रूप में पढ़े गए। अगर यह उसका देश नहीं है, तो पहले + और देश कोड लिखें। दोबारा सहेजने पर यह वैसा ही रहेगा जैसा पढ़ा गया।",
    ),
    (
        "{n} of 16 telephones, with the numbers chosen above.",
        "16 में से {n} फ़ोन, ऊपर चुने गए नंबरों सहित।",
    ),
    (
        "These chats stop being protected",
        "ये चैट अब सुरक्षित नहीं रहेंगी",
    ),
    (
        "Between {bridge}, its WhatsApp until now, and:",
        "{bridge} (अब तक उसका WhatsApp) और इनके बीच:",
    ),
    ("itself (self-chat mode)", "वह खुद (self-chat मोड)"),
    (
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {hermes} orders.",
        "Arauto हर Hermes को फिर से इन चैट में लिखने देगा, और वहाँ {hermes} को आदेश देने वाले व्यक्ति के रूप में पेश होने देगा।",
    ),
    ("Remove the protection", "सुरक्षा हटाएँ"),
    ("Save without protection", "बिना सुरक्षा के सहेजें"),
    ("What {alias} is for it", "{alias} उसके लिए क्या है"),
    ("A telephone that gives it orders", "वह फ़ोन जो उसे आदेश देता है"),
    (
        "Its WhatsApp, where it takes orders",
        "उसका WhatsApp, जहाँ वह आदेश लेता है",
    ),
    ("takes orders on {alias} now", "अभी {alias} पर आदेश लेता है"),
    ("{alias} gives it orders now", "{alias} अभी उसे आदेश देता है"),
    (
        "Nothing is saved here. The window of the Hermes opens with {alias} chosen.",
        "यहाँ कुछ भी सहेजा नहीं जाता। {alias} चुने हुए के साथ Hermes की विंडो खुलती है।",
    ),
    (
        "Choose a Hermes and what {alias} is for it.",
        "कोई Hermes चुनें और यह भी कि {alias} उसके लिए क्या है।",
    ),
    (
        "{alias} is the WhatsApp of {hermes}: it cannot also give it orders.",
        "{alias} {hermes} का WhatsApp है: यह उसे आदेश भी नहीं दे सकता।",
    ),
    ("Actions: {name}", "कार्रवाइयाँ: {name}"),
    (
        "Issues the first invite for {name}.",
        "{name} के लिए पहला आमंत्रण जारी करता है।",
    ),
    (
        "Goes back to the connection of {name} that is in progress.",
        "{name} के चल रहे कनेक्शन पर वापस ले जाता है।",
    ),
    (
        "Issues an invite. When it is used, {name} moves to the new device.",
        "एक आमंत्रण जारी करता है। उसका उपयोग होने पर {name} नए डिवाइस पर चला जाता है।",
    ),
    (
        "Which chat or telephones may give it orders on WhatsApp.",
        "WhatsApp पर कौन-सी चैट या कौन-से फ़ोन उसे आदेश दे सकते हैं।",
    ),
    (
        "Closes {name} in the hub. Asks first.",
        "हब में {name} को बंद करता है। पहले पूछता है।",
    ),
    (
        "Opens the window to pair {alias} with a QR code or a pairing code.",
        "{alias} को QR कोड या पेयरिंग कोड से जोड़ने की विंडो खोलता है।",
    ),
    (
        "Goes back to the pairing of {alias} that is running.",
        "{alias} की चल रही पेयरिंग पर वापस ले जाता है।",
    ),
    (
        "Shows the pairing of {alias} that another panel session started.",
        "{alias} की वह पेयरिंग दिखाता है जिसे पैनल के दूसरे सत्र ने शुरू किया।",
    ),
    (
        "Stops {alias} for every Hermes. Asks for the alias.",
        "{alias} को सभी Hermes के लिए रोकता है। उपनाम पूछता है।",
    ),
    (
        "Starts the failed session of {alias} again.",
        "{alias} का विफल हुआ सत्र फिर से शुरू करता है।",
    ),
    (
        "Another phone paired as {alias}: keep it or undo.",
        "दूसरा फ़ोन {alias} के रूप में जुड़ा: रखें या पूर्ववत करें।",
    ),
    (
        "Which Hermes takes orders in the chats of this number.",
        "इस नंबर की चैट में कौन-सा Hermes आदेश लेता है।",
    ),
    (
        "Takes {alias} out of the hub. Asks for the alias.",
        "{alias} को हब से हटाता है। उपनाम पूछता है।",
    ),
    ("Dismiss", "हटाएँ"),
    ("Dismiss: {title}", "हटाएँ: {title}"),
];
