//! Who gives a Hermes orders on WhatsApp, in the panel (decision 0051, part B): the window of a
//! Hermes, the question that comes first, the tags and the way in from a number.
//!
//! What the form saves is what `admin hermes whatsapp` saves (the same function), and a request
//! that only reads saves nothing. Telephones are the ones of the other tests (`phone(n)`),
//! which belong to no one.

use arauto_hub::admin::orders::OrdersEdit;
use arauto_hub::repo::orders::OrdersRecord;
use arauto_testkit::http;
use arauto_testkit::hub::support::PANEL_HOST;

use crate::numbers::{Rig, rig};
use crate::session::everything;
use crate::world::phone;
use crate::{PATIENCE, audited, form_value, rows};

const CHANGED: &str = "hermes.whatsapp_orders_changed";
const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

const RISK_SENTENCE: &str =
    "If nothing is recorded here, Arauto blocks nothing: a Hermes that can write in the chat where";
const WINDOW_TITLE: &str = "A Hermes could give";

fn plus(n: u32) -> String {
    format!("+{}", phone(n))
}

fn form(name: &str, takes: &str, source: &str, from: &str, rest: &str) -> String {
    format!(
        "name={name}&takes={}&source={source}&from={}{rest}",
        form_value(takes),
        form_value(from)
    )
}

/// What the record of a Hermes is now, as the admin command reads it.
async fn record(rig: &Rig, hermes: &str) -> OrdersRecord {
    rig.world
        .fixture
        .admin
        .hermes_orders_show(&rig.world.vault, hermes)
        .await
        .unwrap()
        .after
}

/// The record, set the way the command sets it.
async fn set(rig: &Rig, hermes: &str, takes: Option<u32>, from: &[u32], self_chat: bool) {
    let edit = OrdersEdit {
        takes_orders_on: Some(takes.map(plus)),
        self_chat,
        orders_from: (!self_chat).then(|| from.iter().map(|n| plus(*n)).collect()),
    };
    rig.world
        .fixture
        .admin
        .hermes_orders_set(&rig.world.vault, hermes, &edit, rig.world.now())
        .await
        .unwrap();
}

/// A hub with the numbers `operador` (telephone 1) and `ponte` (telephone 2) paired.
async fn ready() -> Rig {
    let rig = rig().await;
    rig.world.connected("operador", &phone(1)).await;
    rig.world.connected("ponte", &phone(2)).await;
    rig
}

fn window_path(hermes: &str) -> String {
    format!("/hermes/whatsapp?name={hermes}")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_window_says_the_risk_in_every_state_and_the_list_says_the_state() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let line = |page: &str, state: &str| {
        let wanted = format!(
            "href=\"/hermes/whatsapp?name={ana}\">Who gives it orders<span class=\"muted\">: {state}</span></a>"
        );
        assert!(page.contains(&wanted), "{state}: {page}");
    };

    // Nothing recorded: the list says so, and the window carries the fixed sentence.
    line(&rig.get("/").await.text(), "not recorded");
    let window = rig.get(&window_path(&ana)).await;
    assert_eq!(window.status, 200);
    let window = window.text();
    assert!(window.contains(&format!(
        "<h2 id=\"orders-title\">Who gives {ana} orders on WhatsApp</h2>"
    )));
    assert!(window.contains("<div class=\"page\" inert>"));
    assert!(window.contains(RISK_SENTENCE), "{window}");
    assert!(window.contains(&format!(
        "Arauto refuses every Hermes, {ana} included, writing in the chat where {ana} takes these orders. Who reads it follows the accesses."
    )));
    assert!(window.contains(&format!("The telephone {ana}&#39;s WhatsApp runs on")));
    assert!(window.contains("name=\"source\" value=\"list\" checked>"));
    assert!(!window.contains("This is the number"), "{window}");

    // Its own chat, a list, and the sentence is there in each.
    set(&rig, &ana, Some(2), &[], true).await;
    line(&rig.get("/").await.text(), "its own chat");
    let own = rig.get(&window_path(&ana)).await.text();
    assert!(own.contains(RISK_SENTENCE));
    assert!(own.contains("name=\"source\" value=\"self\" checked>"));
    // The telephone typed is a number of the hub: the window says which.
    assert!(
        own.contains("This is the number <b>ponte</b> in Arauto."),
        "{own}"
    );
    assert!(own.contains(&format!("value=\"{}\"", plus(2))));

    set(&rig, &ana, Some(2), &[1, 3], false).await;
    line(&rig.get("/").await.text(), "2 telephones");
    let listed = rig.get(&window_path(&ana)).await.text();
    assert!(listed.contains(RISK_SENTENCE));
    assert!(listed.contains(&plus(1)) && listed.contains(&plus(3)));
    set(&rig, &ana, Some(2), &[1], false).await;
    line(&rig.get("/").await.text(), "1 telephone");

    // A Hermes that is not there has no window.
    let nobody = rig.get("/hermes/whatsapp?name=ninguem").await;
    assert_eq!((nobody.status, nobody.header("location")), (303, Some("/")));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_that_protects_nothing_asks_first_and_saves_nothing_until_confirmed() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;

    // A first save with everything empty: the window, and nothing written.
    let asked = rig
        .post("/hermes/whatsapp", &form(&ana, "", "list", "", ""))
        .await;
    assert_eq!(asked.status, 200);
    let asked = asked.text();
    assert!(
        asked.contains(&format!("{WINDOW_TITLE} {ana} orders")),
        "{asked}"
    );
    assert!(asked.contains(&format!(
        "Nothing is blocked: any Hermes that can write in the chat where {ana} takes orders can pass for the person who gives them."
    )));
    assert!(asked.contains("Save anyway") && asked.contains("Go back"));
    assert!(asked.contains("<div class=\"page\" inert>"));
    assert_eq!(before, everything(&rig.world.fixture).await);

    // The telephone of the Hermes alone protects nothing either: the window, nothing written.
    let alone = form(&ana, &plus(2), "list", "", "");
    let asked = rig.post("/hermes/whatsapp", &alone).await.text();
    assert!(asked.contains(WINDOW_TITLE), "{asked}");
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 0);
    assert!(record(&rig, &ana).await.is_empty());

    // The answer to the window is what writes.
    let saved = rig
        .post("/hermes/whatsapp", &format!("{alone}&confirmed=1"))
        .await;
    assert_eq!((saved.status, saved.header("location")), (303, Some("/")));
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 1);
    let now = record(&rig, &ana).await;
    assert_eq!(now.takes_orders_on.unwrap().digits(), phone(2));
    assert!(now.orders_from.is_empty());

    // Protecting a chat asks nothing.
    let protected = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, &plus(2), "list", &plus(1), ""),
        )
        .await;
    assert_eq!(
        (protected.status, protected.header("location")),
        (303, Some("/"))
    );
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_telephone_asks_first_and_protecting_more_does_not() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[1, 3], false).await;
    let one_left = form(&ana, &plus(2), "list", &plus(1), "");
    let before = everything(&rig.world.fixture).await;

    let asked = rig.post("/hermes/whatsapp", &one_left).await;
    assert_eq!(asked.status, 200);
    let asked = asked.text();
    assert!(asked.contains(&format!("{WINDOW_TITLE} {ana} orders")));
    assert!(asked.contains(&format!(
        "Arauto will let every Hermes write again in the chat between each removed telephone and {ana}&#39;s WhatsApp, and pass there for the person who gives {ana} orders."
    )));
    assert!(asked.contains(">Remove</button>"), "{asked}");
    assert!(!asked.contains("Save anyway"));
    assert_eq!(before, everything(&rig.world.fixture).await);
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 2);

    // Clearing the whole list says both things.
    let cleared = rig
        .post("/hermes/whatsapp", &form(&ana, &plus(2), "list", "", ""))
        .await
        .text();
    assert!(cleared.contains("Nothing is blocked:") && cleared.contains("Arauto will let every"));

    let removed = rig
        .post("/hermes/whatsapp", &format!("{one_left}&confirmed=1"))
        .await;
    assert_eq!(removed.status, 303);
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 1);

    // Adding one back protects more: no window.
    let added = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                &plus(2),
                "list",
                &format!("{}\n{}", plus(1), plus(3)),
                "",
            ),
        )
        .await;
    assert_eq!(added.status, 303, "{}", added.text());
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn go_back_returns_to_the_window_with_what_was_typed_and_saves_nothing() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[1, 3], false).await;
    let typed = form(&ana, &plus(2), "list", &plus(1), "");
    let before = everything(&rig.world.fixture).await;

    let back = rig
        .post("/hermes/whatsapp", &format!("{typed}&back=1"))
        .await;
    assert_eq!(back.status, 200);
    let back = back.text();
    assert!(back.contains(&format!(
        "<h2 id=\"orders-title\">Who gives {ana} orders on WhatsApp</h2>"
    )));
    assert!(!back.contains(WINDOW_TITLE));
    // What was typed, not what was recorded: one telephone in the list, not two.
    assert!(back.contains(&format!("value=\"{}\"", plus(2))));
    assert!(
        back.contains(&format!(">\n{}</textarea>", plus(1))),
        "{back}"
    );
    assert!(!back.contains(&plus(3)), "{back}");
    assert_eq!(before, everything(&rig.world.fixture).await);

    // The window that asks carries what was typed in both of its forms.
    let asked = rig.post("/hermes/whatsapp", &typed).await.text();
    assert_eq!(
        asked
            .matches(&format!("name=\"from\" value=\"{}\"", plus(1)))
            .count(),
        2,
        "{asked}"
    );
    assert!(asked.contains("name=\"back\" value=\"1\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_through_the_panel_writes_what_the_command_writes() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;

    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                &plus(2),
                "list",
                &format!("{}\r\n{}", plus(1), plus(3)),
                "",
            ),
        )
        .await;
    assert_eq!((saved.status, saved.header("location")), (303, Some("/")));
    // The command, with the same words.
    rig.world
        .fixture
        .admin
        .hermes_orders_set(
            &rig.world.vault,
            &bia,
            &OrdersEdit {
                takes_orders_on: Some(Some(plus(4))),
                self_chat: false,
                orders_from: Some(vec![plus(1), plus(3)]),
            },
            rig.world.now(),
        )
        .await
        .unwrap();
    let panel = record(&rig, &ana).await;
    let command = record(&rig, &bia).await;
    assert_eq!(panel.self_chat, command.self_chat);
    assert_eq!(panel.orders_from.len(), 2);
    assert_eq!(
        panel
            .orders_from
            .iter()
            .map(|p| p.digits().to_owned())
            .collect::<Vec<_>>(),
        command
            .orders_from
            .iter()
            .map(|p| p.digits().to_owned())
            .collect::<Vec<_>>()
    );
    // The audit names the panel, carries counts and no telephone.
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 2);
    let audit = rows(&rig.world.fixture, "audit").await.join("\n");
    assert!(audit.contains("\"actor_note\":\"panel\""), "{audit}");
    for n in [1, 2, 3, 4] {
        assert!(!audit.contains(&phone(n)), "a telephone is in the audit");
    }

    // Its own chat, from the panel: the listed chats go, so the window asks, and the answer writes.
    let to_own = form(&ana, &plus(2), "self", &plus(1), "");
    assert!(
        rig.post("/hermes/whatsapp", &to_own)
            .await
            .text()
            .contains(WINDOW_TITLE)
    );
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 2);
    let own = rig
        .post("/hermes/whatsapp", &format!("{to_own}&confirmed=1"))
        .await;
    assert_eq!(own.status, 303, "{}", own.text());
    let now = record(&rig, &ana).await;
    assert!(now.self_chat && now.orders_from.is_empty(), "{now:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_rules_of_the_record_are_the_ones_of_the_command() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    set(&rig, &bia, Some(2), &[1], false).await;
    let before = everything(&rig.world.fixture).await;

    let many: Vec<String> = (10..27).map(plus).collect();
    for (body, said) in [
        (
            form(&ana, "12", "list", "", ""),
            "Type the number with the country code",
        ),
        (
            form(&ana, &plus(5), "list", "abc", ""),
            "Type the number with the country code",
        ),
        (
            form(&ana, "", "list", &plus(1), ""),
            &format!("Type the telephone {ana}&#39;s WhatsApp runs on first."),
        ),
        (
            form(&ana, "", "self", "", ""),
            &format!("Type the telephone {ana}&#39;s WhatsApp runs on first."),
        ),
        (
            form(&ana, &plus(5), "list", &many.join("\n"), ""),
            "At most 16 telephones.",
        ),
        (
            form(&ana, &plus(2), "list", &plus(1), "&confirmed=1"),
            &format!("{bia} already takes orders on this telephone."),
        ),
    ] {
        let answer = rig.post("/hermes/whatsapp", &body).await;
        assert_eq!(answer.status, 200, "{body}");
        let text = answer.text();
        assert!(text.contains(said), "{body}: {text}");
        assert!(text.contains("role=\"alert\""));
        // The form comes back as it was sent.
        assert!(text.contains("<form id=\"orders-form\""));
    }
    assert_eq!(
        before,
        everything(&rig.world.fixture).await,
        "a refusal saved"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_window_needs_a_session_and_the_origin_and_reading_saves_nothing() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;
    let body = form(&ana, &plus(2), "list", &plus(1), "&confirmed=1");

    let no_session = rig.browser.post("/hermes/whatsapp", None, &body).await;
    assert_eq!(no_session.status, 303);
    assert_eq!(no_session.header("location"), Some("/sign-in"));
    for origin in [None, Some("http://evil.test")] {
        let mut headers = vec![
            ("Host", PANEL_HOST),
            ("Cookie", rig.cookie.as_str()),
            ("Content-Type", "application/x-www-form-urlencoded"),
        ];
        if let Some(origin) = origin {
            headers.push(("Origin", origin));
        }
        let answer = http::post(
            rig.browser.panel,
            "/hermes/whatsapp",
            &headers,
            body.as_bytes(),
            PATIENCE,
        )
        .await
        .unwrap();
        assert_eq!(answer.status, 403, "{origin:?}");
    }
    // No address that is opened saves: not the window, not the chooser, not with every field.
    for path in [
        window_path(&ana),
        format!("/hermes/whatsapp?{body}"),
        "/whatsapps/orders?alias=operador".to_owned(),
    ] {
        let answer = rig.get(&path).await;
        assert_eq!(answer.status, 200, "{path}");
        let signed_out = rig.browser.get(&path, None).await;
        assert_eq!(signed_out.status, 303, "{path}");
    }
    assert!(record(&rig, &ana).await.is_empty());
    assert_eq!(
        before,
        everything(&rig.world.fixture).await,
        "reading saved"
    );

    // The detector sees a write: the same form, with the session and the origin, saves.
    assert_eq!(rig.post("/hermes/whatsapp", &body).await.status, 303);
    assert_ne!(before, everything(&rig.world.fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn what_is_typed_is_shown_as_text_and_names_that_are_not_names_open_nothing() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let clean = |page: &str| {
        assert!(
            !page.contains("<script>") && !page.contains("<img"),
            "{page}"
        );
        assert!(!page.contains("\"'>"), "{page}");
        assert!(page.contains("&lt;script&gt;"), "{page}");
    };
    // A refusal gives back what was typed, escaped.
    let refused = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, HOSTILE, "list", HOSTILE, ""),
        )
        .await;
    assert_eq!(refused.status, 200);
    clean(&refused.text());
    // So does the way back, which asks nothing of what it was given.
    let back = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, HOSTILE, "list", HOSTILE, "&back=1"),
        )
        .await;
    assert_eq!(back.status, 200);
    clean(&back.text());
    // A name or an alias that is not one opens nothing.
    for path in [
        format!("/hermes/whatsapp?name={}", form_value(HOSTILE)),
        format!("/whatsapps/orders?alias={}", form_value(HOSTILE)),
    ] {
        assert_eq!(rig.get(&path).await.status, 303, "{path}");
    }
    let unknown = rig
        .post(
            "/hermes/whatsapp",
            &form(&form_value(HOSTILE), &plus(2), "list", &plus(1), ""),
        )
        .await;
    assert_eq!(unknown.status, 303);
    assert!(record(&rig, &ana).await.is_empty());
}

/// The row of the list of numbers of one alias, as markup.
fn line(list: &str, alias: &str) -> String {
    let start = list
        .find(&format!("<tr><td><span class=\"name\">{alias}</span>"))
        .unwrap_or_else(|| panic!("no line for {alias}: {list}"));
    let rest = &list[start..];
    rest[..rest.find("</tr>").unwrap()].to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_numbers_carry_the_tags_and_lead_to_the_window_of_the_hermes() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[1], false).await;

    let list = rig.get("/whatsapps").await.text();
    let operador = line(&list, "operador");
    let ponte = line(&list, "ponte");
    let link = format!("<a class=\"tag\" href=\"/hermes/whatsapp?name={ana}\">");
    assert!(
        operador.contains(&format!("{link}gives orders to {ana}</a>")),
        "{operador}"
    );
    assert!(
        ponte.contains(&format!("{link}{ana} takes orders here</a>")),
        "{ponte}"
    );
    assert!(!operador.contains("takes orders here") && !ponte.contains("gives orders to"));
    // Any time after a number is in the list: every line has the way in.
    for row in [&operador, &ponte] {
        assert!(row.contains("href=\"/whatsapps/orders?alias="), "{row}");
    }

    // The way in names the Hermes and what the number is for it, and the alias, not a telephone.
    let chooser = rig.get("/whatsapps/orders?alias=operador").await;
    assert_eq!(chooser.status, 200);
    let chooser = chooser.text();
    assert!(
        chooser.contains("Orders on WhatsApp for operador"),
        "{chooser}"
    );
    assert!(chooser.contains("<div class=\"page\" inert>"));
    for wanted in [
        format!(
            "href=\"/hermes/whatsapp?name={ana}&amp;gives=operador\">gives orders to {ana}</a>"
        ),
        format!(
            "href=\"/hermes/whatsapp?name={ana}&amp;takes=operador\">{ana} takes orders here</a>"
        ),
    ] {
        assert!(chooser.contains(&wanted), "{wanted}: {chooser}");
    }
    for href in chooser.split("href=\"").skip(1) {
        let href = href.split('"').next().unwrap();
        assert!(
            !href.contains(&phone(1)),
            "a telephone is in an address: {href}"
        );
    }
    // A number that is not there, or no longer, leads nowhere.
    for alias in ["ninguem", "x%3Cb%3E"] {
        let gone = rig.get(&format!("/whatsapps/orders?alias={alias}")).await;
        assert_eq!(
            (gone.status, gone.header("location")),
            (303, Some("/whatsapps"))
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_telephone_of_a_number_is_typed_in_and_saves_nothing_by_itself() {
    let rig = ready().await;
    let bia = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;

    // "Takes orders here": the telephone of the number is in the field, and the window says
    // which number it is.
    let takes = rig
        .get(&format!("/hermes/whatsapp?name={bia}&takes=ponte"))
        .await;
    assert_eq!(takes.status, 200);
    let takes = takes.text();
    assert!(takes.contains(&format!("value=\"{}\"", plus(2))), "{takes}");
    assert!(takes.contains("This is the number <b>ponte</b> in Arauto."));
    assert!(takes.contains(RISK_SENTENCE));
    // "Gives orders to": the telephone is added to the list.
    let gives = rig
        .get(&format!("/hermes/whatsapp?name={bia}&gives=operador"))
        .await
        .text();
    assert!(
        gives.contains(&format!(">\n{}</textarea>", plus(1))),
        "{gives}"
    );
    assert!(gives.contains("name=\"source\" value=\"list\" checked>"));
    // Nothing was saved by opening them, and a number that is not there fills nothing.
    let nothing = rig
        .get(&format!("/hermes/whatsapp?name={bia}&takes=ninguem"))
        .await
        .text();
    assert!(nothing.contains("name=\"takes\" type=\"text\" inputmode=\"tel\" value=\"\""));
    assert_eq!(before, everything(&rig.world.fixture).await);
    assert!(record(&rig, &bia).await.is_empty());
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 0);

    // Saving the filled form is a save like any: the telephone alone protects nothing, so the
    // window asks; the answer to it writes.
    let filled = form(&bia, &plus(2), "list", "", "");
    assert!(
        rig.post("/hermes/whatsapp", &filled)
            .await
            .text()
            .contains(WINDOW_TITLE)
    );
    assert!(record(&rig, &bia).await.is_empty());
    let both = form(&bia, &plus(2), "list", &plus(1), "");
    assert_eq!(rig.post("/hermes/whatsapp", &both).await.status, 303);
    let now = record(&rig, &bia).await;
    assert_eq!(now.takes_orders_on.unwrap().digits(), phone(2));
    assert_eq!(now.orders_from[0].digits(), phone(1));
    // And the tags are on the numbers now, as the list says.
    let list = rig.get("/whatsapps").await.text();
    assert!(line(&list, "operador").contains(&format!("gives orders to {bia}")));
}
