//! The contract test of the adapter of the WAHA against a REAL engine, with a test number
//! (`referencia/waha-fatos.md`, "Regra para versões novas" and "Teste de contrato").
//!
//! Never part of the normal suite: it is `#[ignore]`d, and it runs only when someone asks for
//! it, by hand, with the variables below. It never touches a real number of anyone: the
//! session and the telephones are the test ones of whoever runs it. It only reads, unless
//! `ARAUTO_CONTRACT_SEND_TO` asks for a text and a reaction.
//!
//! ```text
//! ARAUTO_CONTRACT_ENGINE_URL=http://host:port
//! ARAUTO_CONTRACT_ENGINE_KEY=...            the key of the API of that engine
//! ARAUTO_CONTRACT_SESSION=...               a session already paired with the test number,
//!                                           named like the hub names them: ^[a-z][a-z0-9]{15,62}$
//! ARAUTO_CONTRACT_PHONE=...                 the telephone of that test number
//! ARAUTO_CONTRACT_SEND_TO=...               optional: a test telephone that gets a text
//! cargo test -p arauto-hub --test whatsapp_contract -- --ignored --nocapture
//! ```
//!
//! What it prints is only what each check found (counts and kinds), never a text, a
//! telephone or an identifier.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use arauto_hub::engine::waha::Waha;
use arauto_hub::engine::{
    ApiKey, Deadlines, EngineAddress, EngineStatus, MessageBody, MessageQuery, Page, Phone,
    Reaction, SendText, SessionName, WhatsappEngine,
};

fn variable(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("the contract test needs {name}"))
}

fn adapter() -> Waha {
    Waha::new(
        EngineAddress::parse(&variable("ARAUTO_CONTRACT_ENGINE_URL")).unwrap(),
        Some(ApiKey::parse(&variable("ARAUTO_CONTRACT_ENGINE_KEY")).unwrap()),
        Deadlines {
            connect: Duration::from_secs(5),
            call: Duration::from_secs(30),
        },
    )
}

#[tokio::test]
#[ignore = "talks to a real engine with a test number: run by hand (referencia/waha-fatos.md)"]
async fn the_adapter_reads_and_sends_on_a_real_engine() {
    let engine = adapter();
    let session = SessionName::parse(&variable("ARAUTO_CONTRACT_SESSION")).unwrap();
    let phone = Phone::parse(&variable("ARAUTO_CONTRACT_PHONE")).unwrap();

    engine.health().await.unwrap();
    let version = engine.version().await.unwrap();
    println!(
        "version {} {} {}: tested = {}",
        version.version,
        version.engine,
        version.tier,
        version.is_tested()
    );

    let info = engine
        .session(&session)
        .await
        .unwrap()
        .expect("the session exists");
    assert_eq!(
        info.status,
        EngineStatus::Working,
        "the session is paired and working"
    );
    let me = info.me.expect("a working session says who it is");
    assert!(
        me.phone.is_some_and(|own| own.same_as(&phone)),
        "the session is the test number"
    );

    let chats = engine
        .chats(&session, Page::new(20, 0).unwrap())
        .await
        .unwrap();
    println!("chats: {}", chats.len());
    if let Some(chat) = chats.first() {
        let query = MessageQuery {
            page: Page::new(30, 0).unwrap(),
            before_s: None,
            after_s: None,
        };
        let messages = engine.messages(&session, &chat.id, query).await.unwrap();
        println!("messages of the newest chat: {}", messages.len());
        assert!(
            messages
                .windows(2)
                .all(|pair| pair[0].at_ms >= pair[1].at_ms),
            "the newest first"
        );
        let contact = engine.contact(&session, &chat.id).await;
        println!("contact read: {}", contact.is_ok());
    }

    let groups = engine
        .groups(&session, Page::new(20, 0).unwrap())
        .await
        .unwrap();
    println!("groups: {}", groups.len());
    if let Some(group) = groups.first() {
        let detail = engine
            .group(&session, &group.id, Page::new(50, 0).unwrap())
            .await
            .unwrap();
        println!(
            "participants of the first group: {} in the list, {} in the page",
            detail.total,
            detail.participants.len()
        );
        assert_eq!(
            group.participants, detail.total,
            "the count is the one of the list"
        );
    }

    let checked = engine.check_number(&session, &phone).await.unwrap();
    assert!(checked.is_some(), "the test number has WhatsApp");

    let Ok(to) = std::env::var("ARAUTO_CONTRACT_SEND_TO") else {
        println!("nothing was sent: ARAUTO_CONTRACT_SEND_TO is not set");
        return;
    };
    let to = Phone::parse(&to).unwrap();
    let chat = engine
        .check_number(&session, &to)
        .await
        .unwrap()
        .expect("the telephone that receives has WhatsApp");
    let text = SendText {
        chat: chat.clone(),
        text: MessageBody::to_send("Arauto contract test").unwrap(),
        reply_to: None,
        mentions: Vec::new(),
    };
    let sent = engine.send_text(&session, &text).await.unwrap();
    println!(
        "the text was sent; the engine gave its identifier: {}",
        sent.id.is_some()
    );
    if let Some(id) = sent.id {
        engine
            .react(&session, &id, &Reaction::parse("\u{1f44d}").unwrap())
            .await
            .unwrap();
        println!("the reaction was sent");
    }
}
