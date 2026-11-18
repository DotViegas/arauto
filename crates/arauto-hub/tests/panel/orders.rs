//! Who gives a Hermes orders on WhatsApp, in the panel (decision 0051, part B, rebuilt as option
//! B): the window of a Hermes where the numbers of the hub are chosen by alias, the question that
//! comes first, the tags and the way in from a number.
//!
//! What the form saves is what `admin hermes whatsapp` saves (the same function), and a request
//! that only reads saves nothing. Telephones are the ones of the other tests (`phone(n)`),
//! which belong to no one.

use arauto_hub::admin::orders::OrdersEdit;
use arauto_hub::admin::whatsapp::Removal;
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

/// The same telephone without the ninth digit: the other form of a Brazilian mobile.
fn other_form(n: u32) -> String {
    format!("5511{}", &phone(n)[5..])
}

/// The body of the window of a Hermes. `takes` is `none`, `number:ALIAS`, `number`, `other` or
/// empty for a field that did not come; `rest` is added as it is.
fn form(
    name: &str,
    takes: &str,
    takes_other: &str,
    source: &str,
    gives: &[&str],
    from: &str,
    rest: &str,
) -> String {
    let mut body = format!(
        "name={name}&source={source}&takes_other={}&from={}",
        form_value(takes_other),
        form_value(from)
    );
    if !takes.is_empty() {
        body.push_str(&format!("&takes={}", form_value(takes)));
    }
    for alias in gives {
        body.push_str(&format!("&gives={alias}"));
    }
    body.push_str(rest);
    body
}

/// Asserts that no telephone of the tests, in either form, is in `text`.
fn no_telephone(text: &str) {
    for n in 0..=40 {
        for found in [phone(n), other_form(n)] {
            assert!(
                !text.contains(&found),
                "a telephone is in an address: {text}"
            );
        }
    }
    assert!(
        !text.contains("11987600"),
        "a telephone is in an address: {text}"
    );
}

/// Every address a page carries: its links, its actions, and the values of the inputs of a form
/// that is sent with `GET` (which would end in the address).
fn addresses(page: &str) -> Vec<String> {
    let mut found = Vec::new();
    for attribute in ["href=\"", "action=\""] {
        for part in page.split(attribute).skip(1) {
            found.push(part.split('"').next().unwrap().to_owned());
        }
    }
    for part in page.split("<form ").skip(1) {
        let (tag, rest) = part.split_once('>').unwrap();
        if !tag.contains("method=\"get\"") {
            continue;
        }
        let inside = rest.split("</form>").next().unwrap();
        for value in inside.split("value=\"").skip(1) {
            found.push(value.split('"').next().unwrap().to_owned());
        }
    }
    found
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

/// The window itself, from the page that has it over another one.
fn dialog(page: &str) -> &str {
    &page[page.find("<dialog").expect("a window is open")..]
}

/// The acknowledgement a window carries: one hidden field of 64 lowercase hex characters.
fn ack_of(page: &str) -> String {
    assert_eq!(page.matches("name=\"ack\"").count(), 1, "{page}");
    let ack = page
        .split("name=\"ack\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(
        ack.len() == 64
            && ack
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{ack}"
    );
    ack.to_owned()
}

/// The error paragraph is the first child of the body of the window, before the form.
fn said_first(page: &str, wanted: &str) {
    let body = page
        .split("<div class=\"modal-body\">")
        .nth(1)
        .expect("the window has a body");
    assert!(
        body.starts_with("<p class=\"error\" id=\"orders-error\" role=\"alert\">")
            || body.starts_with("<p class=\"attention\" id=\"orders-error\" role=\"alert\">"),
        "{body}"
    );
    let alert = &body[..body.find("</p>").unwrap()];
    assert!(alert.contains(wanted), "{wanted}: {alert}");
    assert!(
        body.find("id=\"orders-error\"").unwrap() < body.find("<form id=\"orders-form\"").unwrap()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_window_says_the_risk_in_every_state_and_the_list_says_the_state() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let line = |page: &str, state: &str| {
        let wanted = format!("href=\"/hermes/whatsapp?name={ana}\">{state}</a>");
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
    assert!(window.contains("<legend>Its WhatsApp</legend>"));
    assert!(
        window.contains("name=\"takes\" value=\"none\" checked>"),
        "{window}"
    );
    assert!(!window.contains("runs on</label>"), "{window}");
    assert!(window.contains("name=\"source\" value=\"list\" checked>"));
    assert!(!window.contains("This is the number"), "{window}");

    // Its own chat, a list, and the sentence is there in each.
    set(&rig, &ana, Some(2), &[], true).await;
    line(&rig.get("/").await.text(), "its own chat");
    let own = rig.get(&window_path(&ana)).await.text();
    assert!(own.contains(RISK_SENTENCE));
    assert!(own.contains("name=\"source\" value=\"self\" checked>"));
    assert!(
        own.contains("name=\"takes\" value=\"number:ponte\" checked>"),
        "{own}"
    );

    set(&rig, &ana, Some(2), &[1, 3], false).await;
    line(&rig.get("/").await.text(), "2 telephones");
    let listed = rig.get(&window_path(&ana)).await.text();
    assert!(listed.contains(RISK_SENTENCE));
    assert!(
        listed.contains("name=\"gives\" value=\"operador\" checked>"),
        "{listed}"
    );
    assert!(
        listed.contains(&format!(">\n{}</textarea>", plus(3))),
        "{listed}"
    );
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
        .post(
            "/hermes/whatsapp",
            &form(&ana, "none", "", "list", &[], "", ""),
        )
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
    assert!(asked.contains("Save without protection") && asked.contains("Go back"));
    assert!(!asked.contains("Save anyway"));
    assert!(!asked.contains("class=\"warnings\""), "{asked}");
    assert!(asked.contains("<div class=\"page\" inert>"));
    assert_eq!(before, everything(&rig.world.fixture).await);

    // The telephone of the Hermes alone protects nothing either: the window, nothing written.
    let alone = form(&ana, "number:ponte", "", "list", &[], "", "");
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
            &form(&ana, "number:ponte", "", "list", &["operador"], "", ""),
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
    let one_left = form(&ana, "number:ponte", "", "list", &["operador"], "", "");
    let before = everything(&rig.world.fixture).await;

    let asked = rig.post("/hermes/whatsapp", &one_left).await;
    assert_eq!(asked.status, 200);
    let asked = asked.text();
    assert!(asked.contains(&format!("{WINDOW_TITLE} {ana} orders")));
    assert!(asked.contains(&format!(
        "Arauto will let every Hermes write again in these chats, and pass there for the person who gives {ana} orders."
    )));
    assert!(asked.contains(">Remove the protection</button>"), "{asked}");
    assert!(!asked.contains("Save anyway") && !asked.contains(">Remove</button>"));
    // The chats that stop being protected are named: the telephone that goes, not the one that stays.
    assert!(
        asked.contains("These chats stop being protected"),
        "{asked}"
    );
    let warnings = asked.split("<div class=\"warnings\">").nth(1).unwrap();
    let warnings = &warnings[..warnings.find("</div>").unwrap()];
    assert!(
        warnings.contains(&format!("<span class=\"mono\">{}</span>", plus(3))),
        "{warnings}"
    );
    assert!(!warnings.contains("operador"), "{warnings}");
    assert_eq!(before, everything(&rig.world.fixture).await);
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 2);

    // Clearing the whole list says both things.
    let cleared = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &[], "", ""),
        )
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
                "number:ponte",
                "",
                "list",
                &["operador"],
                &plus(3),
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
    let typed = form(&ana, "number:ponte", "", "list", &[], &plus(3), "");
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
    // What was typed, not what was recorded: the number is not checked, one telephone is typed.
    assert!(
        back.contains("name=\"takes\" value=\"number:ponte\" checked>"),
        "{back}"
    );
    assert!(
        back.contains("name=\"gives\" value=\"operador\">"),
        "{back}"
    );
    assert!(!back.contains("name=\"gives\" value=\"operador\" checked"));
    assert!(
        back.contains(&format!(">\n{}</textarea>", plus(3))),
        "{back}"
    );
    assert_eq!(before, everything(&rig.world.fixture).await);

    // The window that asks carries what was typed in both of its forms, and its X goes back.
    let asked = rig.post("/hermes/whatsapp", &typed).await.text();
    assert!(asked.contains(WINDOW_TITLE), "{asked}");
    assert_eq!(
        asked
            .matches("name=\"takes\" value=\"number:ponte\"")
            .count(),
        2,
        "{asked}"
    );
    assert_eq!(
        asked
            .matches(&format!("name=\"from\" value=\"{}\"", plus(3)))
            .count(),
        2,
        "{asked}"
    );
    assert!(asked.contains("name=\"back\" value=\"1\""));
    assert!(asked.contains("<button class=\"close\" type=\"submit\" form=\"orders-back\""));
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
                "number:ponte",
                "",
                "list",
                &["operador"],
                &format!("\r\n{}", plus(3)),
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

    // Its own chat together with a list is refused, as the command refuses it: nothing is dropped.
    let both = form(&ana, "number:ponte", "", "self", &["operador"], "", "");
    let refused = rig.post("/hermes/whatsapp", &both).await;
    assert_eq!(refused.status, 200);
    said_first(
        &refused.text(),
        "Its own chat and a list of telephones are two answers",
    );
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 2);

    // Its own chat alone: the listed chats go, so the window asks, and the answer writes.
    let to_own = form(&ana, "number:ponte", "", "self", &[], "", "");
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
            form(&ana, "other", "12", "list", &[], "", ""),
            "The other telephone of its WhatsApp is not a telephone".to_owned(),
        ),
        (
            form(&ana, "other", "", "list", &[], "", ""),
            "Type the other telephone its WhatsApp runs on".to_owned(),
        ),
        (
            form(&ana, "number:ponte", "", "list", &[], "abc", ""),
            "Line 1 of Other telephones is not a telephone.".to_owned(),
        ),
        (
            form(&ana, "none", "", "list", &["operador"], "", ""),
            format!("Choose {ana}&#39;s WhatsApp first"),
        ),
        (
            form(&ana, "none", "", "self", &[], "", ""),
            format!("Choose {ana}&#39;s WhatsApp first"),
        ),
        (
            form(&ana, "", "", "list", &[], "", ""),
            "Choose an option for its WhatsApp.".to_owned(),
        ),
        (
            form(&ana, "number:ponte", "", "list", &[], &many.join("\n"), ""),
            "these are 17".to_owned(),
        ),
        (
            form(
                &ana,
                "other",
                &plus(2),
                "list",
                &["operador"],
                "",
                "&confirmed=1",
            ),
            format!("{bia} already takes orders on this telephone."),
        ),
    ] {
        let answer = rig.post("/hermes/whatsapp", &body).await;
        assert_eq!(answer.status, 200, "{body}");
        let text = answer.text();
        assert!(text.contains(&said), "{body}: {text}");
        // The form comes back as it was sent, and the error is above it.
        assert!(text.contains("<form id=\"orders-form\""));
        said_first(&text, &said);
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
    let body = form(
        &ana,
        "number:ponte",
        "",
        "list",
        &["operador"],
        "",
        "&confirmed=1",
    );

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
    for (path, status) in [
        (window_path(&ana), 200),
        (format!("/hermes/whatsapp?{body}"), 200),
        (
            format!("/hermes/whatsapp?name={ana}&alias=operador&role=gives&return=whatsapps"),
            200,
        ),
        ("/whatsapps/orders?alias=operador".to_owned(), 200),
        (
            format!("/whatsapps/orders?alias=operador&name={ana}&role=gives"),
            303,
        ),
    ] {
        let answer = rig.get(&path).await;
        assert_eq!(answer.status, status, "{path}");
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
            &form(&ana, "other", HOSTILE, "list", &[], HOSTILE, ""),
        )
        .await;
    assert_eq!(refused.status, 200);
    clean(&refused.text());
    // So does the way back, which asks nothing of what it was given.
    let back = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "other", HOSTILE, "list", &[], HOSTILE, "&back=1"),
        )
        .await;
    assert_eq!(back.status, 200);
    clean(&back.text());
    // A form that is not the window's is not read: nothing is written.
    let before = everything(&rig.world.fixture).await;
    for body in [
        form(
            &ana,
            "number:ponte",
            "",
            "list",
            &[],
            "",
            &format!("&gives={}", form_value(HOSTILE)),
        ),
        form(&ana, &format!("number:{HOSTILE}"), "", "list", &[], "", ""),
        form(&ana, "whatever", "", "list", &[], "", ""),
    ] {
        let answer = rig.post("/hermes/whatsapp", &body).await;
        assert_eq!(answer.status, 400, "{body}");
        assert_eq!(answer.text(), "The form was not read.\n");
    }
    let sixty_five: String = (0..65).map(|n| format!("&gives=n{n}")).collect();
    let answer = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &[], "", &sixty_five),
        )
        .await;
    assert_eq!(answer.status, 400);
    assert_eq!(before, everything(&rig.world.fixture).await);
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
            &form(
                &form_value(HOSTILE),
                "number:ponte",
                "",
                "list",
                &["operador"],
                "",
                "",
            ),
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
    let link =
        format!("<a class=\"tag\" href=\"/hermes/whatsapp?name={ana}&amp;return=whatsapps\">");
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
    for alias in ["operador", "ponte"] {
        let actions = rig
            .get(&format!("/whatsapps/actions?alias={alias}"))
            .await
            .text();
        assert!(
            actions.contains(&format!("href=\"/whatsapps/orders?alias={alias}\"")),
            "{actions}"
        );
    }
    for row in [&operador, &ponte] {
        assert!(row.contains("href=\"/whatsapps/actions?alias="), "{row}");
    }

    // The way in is a short form that names the Hermes and what the number is for it now, and
    // checks nothing beforehand.
    let chooser = rig.get("/whatsapps/orders?alias=operador").await;
    assert_eq!(chooser.status, 200);
    let chooser = chooser.text();
    assert!(
        chooser.contains("Orders on WhatsApp for operador"),
        "{chooser}"
    );
    assert!(chooser.contains("<div class=\"page\" inert>"));
    let window = dialog(&chooser);
    assert!(
        window.contains("<form id=\"orders-choose\" method=\"get\" action=\"/whatsapps/orders\">"),
        "{window}"
    );
    assert!(
        window.contains(&format!("name=\"name\" value=\"{ana}\" required>")),
        "{window}"
    );
    assert!(window.contains("name=\"role\" value=\"gives\" required>"));
    assert!(window.contains("name=\"role\" value=\"takes\" required>"));
    assert!(!window.contains(" checked"), "{window}");
    assert!(window.contains("operador gives it orders now"), "{window}");
    assert!(!window.contains("takes orders on operador now"));
    assert!(!window.contains("&amp;gives=") && !window.contains("&amp;takes="));
    no_telephone(&addresses(&chooser).join("\n"));
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
async fn a_number_chosen_from_its_row_is_checked_and_saves_nothing_by_itself() {
    let rig = ready().await;
    let bia = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;

    // "Its WhatsApp": the number is chosen, and the window says so.
    let takes = rig
        .get(&format!(
            "/hermes/whatsapp?name={bia}&alias=ponte&role=takes"
        ))
        .await;
    assert_eq!(takes.status, 200);
    let takes = takes.text();
    assert!(
        takes.contains("name=\"takes\" value=\"number:ponte\" checked>"),
        "{takes}"
    );
    assert!(takes.contains("<b>ponte</b> is chosen. Nothing is saved until you save."));
    assert!(takes.contains(RISK_SENTENCE));
    // "Gives orders to": the box of the number is checked.
    let gives = rig
        .get(&format!(
            "/hermes/whatsapp?name={bia}&alias=operador&role=gives"
        ))
        .await
        .text();
    assert!(
        gives.contains("name=\"gives\" value=\"operador\" checked>"),
        "{gives}"
    );
    assert!(gives.contains("name=\"source\" value=\"list\" checked>"));
    // A number that is not there is said, and fills nothing.
    let nothing = rig
        .get(&format!(
            "/hermes/whatsapp?name={bia}&alias=ninguem&role=takes"
        ))
        .await
        .text();
    assert!(nothing.contains("ninguem is not a number of Arauto now."));
    assert!(nothing.contains("name=\"takes\" value=\"none\" checked>"));
    // A role that is neither, or an alias that is not one, is ignored.
    let ignored = rig
        .get(&format!(
            "/hermes/whatsapp?name={bia}&alias=ponte&role=other"
        ))
        .await
        .text();
    assert!(ignored.contains("name=\"takes\" value=\"none\" checked>"));
    assert!(!ignored.contains("Nothing is saved until you save."));
    // Nothing was saved by opening them.
    assert_eq!(before, everything(&rig.world.fixture).await);
    assert!(record(&rig, &bia).await.is_empty());
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, 0);

    // Saving the filled form is a save like any: the telephone alone protects nothing, so the
    // window asks; the answer to it writes.
    let filled = form(&bia, "number:ponte", "", "list", &[], "", "");
    assert!(
        rig.post("/hermes/whatsapp", &filled)
            .await
            .text()
            .contains(WINDOW_TITLE)
    );
    assert!(record(&rig, &bia).await.is_empty());
    let both = form(&bia, "number:ponte", "", "list", &["operador"], "", "");
    assert_eq!(rig.post("/hermes/whatsapp", &both).await.status, 303);
    let now = record(&rig, &bia).await;
    assert_eq!(now.takes_orders_on.unwrap().digits(), phone(2));
    assert_eq!(now.orders_from[0].digits(), phone(1));
    // And the tags are on the numbers now, as the list says.
    let list = rig.get("/whatsapps").await.text();
    assert!(line(&list, "operador").contains(&format!("gives orders to {bia}")));
}

// ------------------------------------------------------------------------------------ option B

#[tokio::test(flavor = "multi_thread")]
async fn a_number_of_the_hub_is_chosen_by_alias_and_the_record_keeps_telephones() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &["operador"], "", ""),
        )
        .await;
    assert_eq!((saved.status, saved.header("location")), (303, Some("/")));
    let now = record(&rig, &ana).await;
    assert_eq!(now.takes_orders_on.unwrap().digits(), phone(2));
    assert_eq!(
        now.orders_from
            .iter()
            .map(|p| p.digits().to_owned())
            .collect::<Vec<_>>(),
        [phone(1)]
    );
    let audit = rows(&rig.world.fixture, "audit").await.join("\n");
    assert!(audit.contains("\"actor_note\":\"panel\""), "{audit}");
    assert!(!audit.contains(&phone(1)) && !audit.contains(&phone(2)));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_alias_that_is_not_a_number_now_is_refused_by_name() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    // A number that was added and then removed is not a choice either, and is not named.
    rig.world.add("velho", &phone(7)).await;
    rig.world
        .fixture
        .admin
        .number_remove(
            &rig.world.vault,
            "velho",
            Removal::KeepInEngine,
            rig.world.now(),
        )
        .await
        .unwrap();
    let before = everything(&rig.world.fixture).await;

    let window = rig.get(&window_path(&ana)).await.text();
    assert!(!window.contains("value=\"velho\"") && !window.contains(&phone(7)));
    for body in [
        form(&ana, "number:ponte", "", "list", &["ninguem"], "", ""),
        form(&ana, "number:ponte", "", "list", &["velho"], "", ""),
    ] {
        let answer = rig.post("/hermes/whatsapp", &body).await;
        assert_eq!(answer.status, 200, "{body}");
        let text = answer.text();
        let said = if body.contains("velho") {
            "velho is not a number of Arauto now."
        } else {
            "ninguem is not a number of Arauto now."
        };
        said_first(&text, said);
        assert!(
            text.contains("<fieldset class=\"orders-source\" aria-describedby=\"orders-error\">"),
            "{text}"
        );
    }
    for alias in ["ninguem", "velho"] {
        let answer = rig
            .post(
                "/hermes/whatsapp",
                &form(
                    &ana,
                    &format!("number:{alias}"),
                    "",
                    "list",
                    &["operador"],
                    "",
                    "",
                ),
            )
            .await;
        assert_eq!(answer.status, 200);
        said_first(
            &answer.text(),
            &format!("{alias} is not a number of Arauto now."),
        );
    }
    assert_eq!(before, everything(&rig.world.fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_other_form_of_a_recorded_telephone_chosen_by_alias_writes_nothing() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    rig.world
        .fixture
        .admin
        .hermes_orders_set(
            &rig.world.vault,
            &ana,
            &OrdersEdit {
                takes_orders_on: Some(Some(plus(2))),
                self_chat: false,
                orders_from: Some(vec![format!("+{}", other_form(1))]),
            },
            rig.world.now(),
        )
        .await
        .unwrap();
    let changed = audited(&rig.world.fixture, CHANGED).await;

    // The window shows the number that is the recorded telephone, checked.
    let window = rig.get(&window_path(&ana)).await.text();
    assert!(
        window.contains("name=\"gives\" value=\"operador\" checked>"),
        "{window}"
    );
    // Posting the same choices changes nothing, and the record keeps the form it had.
    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &["operador"], "", ""),
        )
        .await;
    assert_eq!((saved.status, saved.header("location")), (303, Some("/")));
    assert_eq!(audited(&rig.world.fixture, CHANGED).await, changed);
    let now = record(&rig, &ana).await;
    assert_eq!(now.orders_from[0].digits(), other_form(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_typed_telephone_without_plus_stops_save_once_and_saving_again_keeps_it() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;
    let body = form(&ana, "number:ponte", "", "list", &[], "11987600005", "");

    let stopped = rig.post("/hermes/whatsapp", &body).await;
    assert_eq!(stopped.status, 200);
    let stopped = stopped.text();
    assert!(
        stopped.contains("<p class=\"attention\" id=\"orders-error\" role=\"alert\">1 typed telephone needs a look."),
        "{stopped}"
    );
    assert!(stopped.contains("<span class=\"mono\">+11987600005</span>"));
    assert!(
        stopped.contains("Typed without +, so the first digits were read as the country code.")
    );
    let textarea = stopped
        .split("<textarea")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    assert!(
        textarea.contains("aria-describedby=\"orders-from-hint orders-readback orders-error\""),
        "{textarea}"
    );
    assert!(!textarea.contains("aria-invalid"), "{textarea}");
    let ack = ack_of(&stopped);
    assert_eq!(before, everything(&rig.world.fixture).await);

    // Saving again with the same line keeps it as read.
    let again = rig
        .post("/hermes/whatsapp", &format!("{body}&ack={ack}"))
        .await;
    assert_eq!((again.status, again.header("location")), (303, Some("/")));
    let now = record(&rig, &ana).await;
    assert_eq!(now.orders_from[0].digits(), "11987600005");

    // A line with "+" never stops.
    let bia = rig.world.fixture.hermes().await;
    let plain = rig
        .post(
            "/hermes/whatsapp",
            &form(&bia, "other", &plus(8), "list", &[], &plus(3), ""),
        )
        .await;
    assert_eq!((plain.status, plain.header("location")), (303, Some("/")));

    // The other field: a telephone typed without "+" stops once, with its read under the input.
    let cia = rig.world.fixture.hermes().await;
    let other = form(&cia, "other", "11987600002", "list", &["operador"], "", "");
    let stopped = rig.post("/hermes/whatsapp", &other).await;
    assert_eq!(stopped.status, 200);
    let stopped = stopped.text();
    assert!(
        stopped.contains("<p class=\"hint\" id=\"orders-takes-read\">"),
        "{stopped}"
    );
    assert!(stopped.contains("<span class=\"mono\">+11987600002</span>"));
    assert!(stopped.contains("1 typed telephone needs a look."));
    let input = stopped
        .split("<input id=\"orders-takes-other\"")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    assert!(
        input.contains(
            "aria-describedby=\"orders-takes-other-hint orders-takes-read orders-error\""
        ) && !input.contains("aria-invalid"),
        "{input}"
    );
    let ack = ack_of(&stopped);
    let saved = rig
        .post("/hermes/whatsapp", &format!("{other}&ack={ack}"))
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert_eq!(
        record(&rig, &cia).await.takes_orders_on.unwrap().digits(),
        "11987600002"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_acknowledgement_is_issued_for_a_warning_that_was_not_shown() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;
    // More than 64 lines: no read-back is built, so the warning of the "+"-less telephone of
    // "Its WhatsApp" is not shown, and no acknowledgement may come with the refusal.
    let many = (100..=164).map(plus).collect::<Vec<_>>().join("\n");
    let refused = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "other", "11987600002", "list", &[], &many, ""),
        )
        .await;
    assert_ne!(refused.status, 303);
    let refused = refused.text();
    assert!(!refused.contains("orders-takes-read"), "{refused}");
    assert!(!refused.contains("name=\"ack\""), "{refused}");
    assert_eq!(before, everything(&rig.world.fixture).await);

    // With the list cut, the warning is shown and stops the save; nothing is saved.
    let short = form(&ana, "other", "11987600002", "list", &[], &plus(3), "");
    let stopped = rig.post("/hermes/whatsapp", &short).await;
    assert_eq!(stopped.status, 200);
    let stopped = stopped.text();
    assert!(stopped.contains("orders-takes-read"), "{stopped}");
    assert!(stopped.contains("1 typed telephone needs a look."));
    assert_eq!(before, everything(&rig.world.fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_acknowledgement_forged_or_stale_is_not_accepted() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    let body = form(&ana, "number:ponte", "", "list", &[], "11987600005", "");
    let stopped = rig.post("/hermes/whatsapp", &body).await.text();
    let ack = ack_of(&stopped);
    let before = everything(&rig.world.fixture).await;
    let stops = |answer: &str| answer.contains("1 typed telephone needs a look.");

    // Forged: 64 zeros.
    let forged = rig
        .post(
            "/hermes/whatsapp",
            &format!("{body}&ack={}", "0".repeat(64)),
        )
        .await;
    assert_eq!(forged.status, 200);
    assert!(stops(&forged.text()));
    // Stale: the real one, with the line changed. It stops again, with another value.
    let changed = form(&ana, "number:ponte", "", "list", &[], "11987600006", "");
    let stale = rig
        .post("/hermes/whatsapp", &format!("{changed}&ack={ack}"))
        .await
        .text();
    assert!(stops(&stale));
    assert_ne!(ack_of(&stale), ack);
    // Another Hermes, the same lines.
    let other_body = form(&bia, "number:ponte", "", "list", &[], "11987600005", "");
    let other = rig
        .post("/hermes/whatsapp", &format!("{other_body}&ack={ack}"))
        .await
        .text();
    assert!(stops(&other));
    assert_ne!(ack_of(&other), ack);
    assert_eq!(
        before,
        everything(&rig.world.fixture).await,
        "a refusal saved"
    );

    // "Go back" carries it, and it is accepted.
    let back = rig
        .post("/hermes/whatsapp", &format!("{body}&ack={ack}&back=1"))
        .await;
    assert_eq!(back.status, 200);
    let carried = ack_of(&back.text());
    assert_eq!(carried, ack);
    let saved = rig
        .post("/hermes/whatsapp", &format!("{body}&ack={carried}"))
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    // A value that is not 64 lowercase hex characters is dropped by "Go back".
    let dropped = rig
        .post("/hermes/whatsapp", &format!("{body}&ack=ABC&back=1"))
        .await
        .text();
    assert!(!dropped.contains("name=\"ack\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_telephone_of_its_whatsapp_cannot_also_give_it_orders_in_the_panel() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;
    let said = "Its WhatsApp cannot also give it orders: that chat is the self-chat mode.";

    // A number checked that is its WhatsApp.
    let checked = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &["ponte"], "", ""),
        )
        .await;
    assert_eq!(checked.status, 200);
    let checked = checked.text();
    said_first(&checked, said);
    assert!(
        checked.contains("name=\"gives\" value=\"ponte\" checked aria-invalid=\"true\" aria-describedby=\"orders-error\">"),
        "{checked}"
    );
    // A typed line that is its WhatsApp, in the other form of the telephone.
    let typed = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "number:ponte",
                "",
                "list",
                &[],
                &format!("+{}", other_form(2)),
                "",
            ),
        )
        .await
        .text();
    said_first(&typed, said);
    assert!(typed.contains("This is its WhatsApp: it cannot also give it orders."));
    let textarea = typed
        .split("<textarea")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    assert!(textarea.contains("aria-invalid=\"true\""), "{textarea}");
    // The other telephone of its WhatsApp, typed, and the same one in the lines.
    let both = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "other", &plus(5), "list", &[], &plus(5), ""),
        )
        .await
        .text();
    said_first(&both, said);
    let input = both
        .split("<input id=\"orders-takes-other\"")
        .nth(1)
        .unwrap()
        .split('>')
        .next()
        .unwrap();
    assert!(input.contains("aria-invalid=\"true\""), "{input}");
    assert_eq!(before, everything(&rig.world.fixture).await);
    assert!(record(&rig, &ana).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn self_chat_with_a_list_is_refused_and_nothing_is_dropped() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let before = everything(&rig.world.fixture).await;
    let said = "Its own chat and a list of telephones are two answers";

    let boxed = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "self", &["operador"], "", ""),
        )
        .await;
    assert_eq!(boxed.status, 200);
    let boxed = boxed.text();
    said_first(&boxed, said);
    assert!(
        boxed.contains("name=\"gives\" value=\"operador\" checked>"),
        "{boxed}"
    );
    assert!(boxed.contains("name=\"source\" value=\"self\" checked>"));
    let typed = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "self", &[], &plus(3), ""),
        )
        .await
        .text();
    said_first(&typed, said);
    assert!(
        typed.contains(&format!(">\n{}</textarea>", plus(3))),
        "{typed}"
    );
    assert_eq!(before, everything(&rig.world.fixture).await);

    // Self-chat with nothing listed is saved.
    let own = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "self", &[], "", ""),
        )
        .await;
    assert_eq!((own.status, own.header("location")), (303, Some("/")));
    assert!(record(&rig, &ana).await.self_chat);
}

#[tokio::test(flavor = "multi_thread")]
async fn sixteen_are_counted_after_the_ninth_digit_merge_as_the_command_counts() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    let lines = |range: std::ops::RangeInclusive<u32>| -> Vec<String> {
        range.map(plus).collect::<Vec<_>>()
    };

    // 17 lines, 16 chats: one telephone is typed in both forms.
    let mut merged = lines(10..=25);
    merged.push(format!("+{}", other_form(10)));
    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "number:ponte",
                "",
                "list",
                &[],
                &merged.join("\n"),
                "",
            ),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 16);
    // The command, given the same 17 texts, records 16 as well.
    rig.world
        .fixture
        .admin
        .hermes_orders_set(
            &rig.world.vault,
            &bia,
            &OrdersEdit {
                takes_orders_on: Some(Some(plus(4))),
                self_chat: false,
                orders_from: Some(merged.clone()),
            },
            rig.world.now(),
        )
        .await
        .unwrap();
    assert_eq!(record(&rig, &bia).await.orders_from.len(), 16);

    // 17 chats are 17.
    let seventeen = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "number:ponte",
                "",
                "list",
                &[],
                &lines(10..=26).join("\n"),
                "",
            ),
        )
        .await
        .text();
    said_first(&seventeen, "these are 17");
    assert_eq!(record(&rig, &ana).await.orders_from.len(), 16);
    // A number checked counts with the lines, and the read-back says so.
    let counted = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "number:ponte",
                "",
                "list",
                &["operador"],
                &lines(10..=25).join("\n"),
                "",
            ),
        )
        .await
        .text();
    said_first(&counted, "these are 17");
    assert!(
        counted.contains("17 of 16 telephones, with the numbers chosen above."),
        "{counted}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_taken_whatsapp_is_said_before_the_window_that_asks() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    set(&rig, &bia, Some(2), &[1], false).await;
    let before = everything(&rig.world.fixture).await;

    let window = rig.get(&window_path(&ana)).await.text();
    assert!(
        window.contains("value=\"number:ponte\" disabled"),
        "{window}"
    );
    assert!(window.contains(&format!("{bia} takes orders here")));
    // A forged choice of it, with nothing listed (which would ask): said, not asked.
    let forged = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &[], "", ""),
        )
        .await;
    assert_eq!(forged.status, 200);
    let forged = forged.text();
    said_first(
        &forged,
        &format!("{bia} already takes orders on this telephone."),
    );
    assert!(!forged.contains(WINDOW_TITLE));
    // The same telephone typed, in its other form.
    let typed = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "other",
                &format!("+{}", other_form(2)),
                "list",
                &[],
                "",
                "",
            ),
        )
        .await
        .text();
    said_first(
        &typed,
        &format!("{bia} already takes orders on this telephone."),
    );
    assert!(!typed.contains(WINDOW_TITLE));
    assert_eq!(before, everything(&rig.world.fixture).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_chooser_checks_nothing_and_opens_the_window_by_alias() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[1], false).await;
    let before = everything(&rig.world.fixture).await;

    let first = rig.get("/whatsapps/orders?alias=operador").await.text();
    let window = dialog(&first);
    assert!(!window.contains(" checked"), "{window}");
    assert!(
        window.contains("<form id=\"orders-choose\" method=\"get\" action=\"/whatsapps/orders\">")
    );
    assert!(window.contains("name=\"role\" value=\"gives\" required>"));
    assert!(window.contains("name=\"role\" value=\"takes\" required>"));
    assert!(window.contains(&format!("name=\"name\" value=\"{bia}\" required>")));
    assert!(window.contains("operador gives it orders now"));
    // One of the two missing: said, with the valid choice kept.
    let partial = rig.get("/whatsapps/orders?alias=operador&role=gives").await;
    assert_eq!(partial.status, 200);
    let partial = partial.text();
    assert!(
        partial.contains("Choose a Hermes and what operador is for it."),
        "{partial}"
    );
    assert!(
        partial.contains("name=\"role\" value=\"gives\" required checked>"),
        "{partial}"
    );
    // Both: the window of the Hermes, by an address that names no telephone.
    let both = rig
        .get(&format!(
            "/whatsapps/orders?alias=operador&name={ana}&role=gives"
        ))
        .await;
    assert_eq!(both.status, 303);
    assert_eq!(
        both.header("location"),
        Some(
            format!("/hermes/whatsapp?name={ana}&alias=operador&role=gives&return=whatsapps")
                .as_str()
        )
    );
    // A Hermes that is not there is no choice.
    let unknown = rig
        .get("/whatsapps/orders?alias=operador&name=ninguem&role=gives")
        .await;
    assert_eq!(unknown.status, 200);
    assert!(
        unknown
            .text()
            .contains("Choose a Hermes and what operador is for it.")
    );
    // Its WhatsApp is the number of the Hermes: it cannot also give it orders.
    let giver = rig
        .get(&format!(
            "/whatsapps/orders?alias=ponte&name={ana}&role=gives"
        ))
        .await
        .text();
    assert!(
        giver.contains(&format!("ponte is the WhatsApp of {ana}")),
        "{giver}"
    );
    assert_eq!(before, everything(&rig.world.fixture).await);
    drop(rig);

    // Another Hermes already takes orders on the number.
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    set(&rig, &bia, Some(2), &[1], false).await;
    let taken = rig
        .get(&format!(
            "/whatsapps/orders?alias=ponte&name={ana}&role=takes"
        ))
        .await;
    assert_eq!(taken.status, 200);
    assert!(
        taken
            .text()
            .contains(&format!("{bia} already takes orders on this telephone.")),
    );
    let mine = rig
        .get(&format!(
            "/whatsapps/orders?alias=ponte&name={bia}&role=takes"
        ))
        .await;
    assert_eq!(mine.status, 303);
    drop(rig);

    // No Hermes yet.
    let rig = ready().await;
    let empty = rig.get("/whatsapps/orders?alias=operador").await.text();
    assert!(empty.contains("Connect a Hermes first."));
    assert!(empty.contains("href=\"/hermes/connect\""));
    assert!(!dialog(&empty).contains("<form id=\"orders-choose\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_telephone_is_in_an_address_or_a_location() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let bia = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[1, 3], false).await;
    let check = |page: &str| no_telephone(&addresses(page).join("\n"));

    for path in ["/", "/whatsapps", "/whatsapps/orders?alias=operador"] {
        let answer = rig.get(path).await;
        assert_eq!(answer.status, 200, "{path}");
        check(&answer.text());
    }
    let chosen = rig
        .get(&format!(
            "/whatsapps/orders?alias=operador&name={ana}&role=gives"
        ))
        .await;
    assert_eq!(chosen.status, 303);
    no_telephone(chosen.header("location").unwrap());
    for path in [
        window_path(&ana),
        format!("/hermes/whatsapp?name={ana}&return=whatsapps"),
        format!("/hermes/whatsapp?name={ana}&alias=operador&role=gives&return=whatsapps"),
    ] {
        let answer = rig.get(&path).await;
        assert_eq!(answer.status, 200, "{path}");
        check(&answer.text());
    }
    // A stop with a read-back, the window that asks, the way back, and a save.
    let stop = form(
        &ana,
        "number:ponte",
        "",
        "list",
        &["operador"],
        "11987600005",
        "",
    );
    let stopped = rig.post("/hermes/whatsapp", &stop).await;
    assert_eq!(stopped.status, 200);
    let stopped = stopped.text();
    assert!(stopped.contains("11987600005"), "the page itself shows it");
    check(&stopped);
    let asked = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &["operador"], "", ""),
        )
        .await;
    assert_eq!(asked.status, 200);
    assert!(asked.text().contains(&plus(3)), "the page itself shows it");
    check(&asked.text());
    let back = rig
        .post(
            "/hermes/whatsapp",
            &form(
                &ana,
                "number:ponte",
                "",
                "list",
                &["operador"],
                &plus(3),
                "&back=1",
            ),
        )
        .await;
    check(&back.text());
    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(&bia, "other", &plus(9), "list", &["operador"], &plus(3), ""),
        )
        .await;
    assert_eq!(saved.status, 303);
    no_telephone(saved.header("location").unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_flow_from_the_whatsapps_tab_returns_there() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    let opened = rig
        .get(&format!("/hermes/whatsapp?name={ana}&return=whatsapps"))
        .await;
    assert_eq!(opened.status, 200);
    let opened = opened.text();
    assert!(opened.contains("<h1>WhatsApps</h1>"), "{opened}");
    assert!(opened.contains("<div class=\"page\" inert>"));
    let window = dialog(&opened);
    assert!(
        window.contains("<a class=\"close\" href=\"/whatsapps\""),
        "{window}"
    );
    assert!(window.contains("<a class=\"button\" href=\"/whatsapps\">Cancel</a>"));
    assert!(window.contains("<input type=\"hidden\" name=\"return\" value=\"whatsapps\">"));
    // Any other value goes back to the first page.
    let elsewhere = rig
        .get(&format!("/hermes/whatsapp?name={ana}&return=elsewhere"))
        .await
        .text();
    assert!(dialog(&elsewhere).contains("<a class=\"close\" href=\"/\""));
    assert!(!elsewhere.contains("name=\"return\""));

    // A save returns there, and so does a save that asks, through both of its forms.
    let body = form(
        &ana,
        "number:ponte",
        "",
        "list",
        &["operador"],
        "",
        "&return=whatsapps",
    );
    let saved = rig.post("/hermes/whatsapp", &body).await;
    assert_eq!(
        (saved.status, saved.header("location")),
        (303, Some("/whatsapps"))
    );
    let asks = form(
        &ana,
        "number:ponte",
        "",
        "list",
        &[],
        "",
        "&return=whatsapps",
    );
    let asked = rig.post("/hermes/whatsapp", &asks).await.text();
    assert!(asked.contains(WINDOW_TITLE));
    assert!(asked.contains("<h1>WhatsApps</h1>"));
    assert_eq!(
        asked.matches("name=\"return\" value=\"whatsapps\"").count(),
        2,
        "{asked}"
    );
    let back = rig
        .post("/hermes/whatsapp", &format!("{asks}&back=1"))
        .await
        .text();
    assert!(
        back.contains("name=\"return\" value=\"whatsapps\""),
        "{back}"
    );
    assert!(back.contains("<h1>WhatsApps</h1>"));
    let kept = rig
        .post("/hermes/whatsapp", &format!("{asks}&confirmed=1"))
        .await;
    assert_eq!(
        (kept.status, kept.header("location")),
        (303, Some("/whatsapps"))
    );
    // Another value goes to the first page.
    let other = form(
        &ana,
        "number:ponte",
        "",
        "list",
        &["operador"],
        "",
        "&return=elsewhere",
    );
    let saved = rig.post("/hermes/whatsapp", &other).await;
    assert_eq!((saved.status, saved.header("location")), (303, Some("/")));
}

#[tokio::test(flavor = "multi_thread")]
async fn more_than_eight_numbers_make_its_whatsapp_a_list_box() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    for n in 3..=9u32 {
        rig.world.add(&format!("n{n}"), &phone(30 + n)).await;
    }
    let window = rig.get(&window_path(&ana)).await.text();
    assert!(
        window.contains("<select id=\"orders-takes-number\" name=\"takes_number\""),
        "{window}"
    );
    assert!(!window.contains("value=\"number:"), "{window}");
    assert_eq!(
        window.matches("type=\"checkbox\" name=\"gives\"").count(),
        9
    );

    let saved = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number", "", "list", &["n4"], "", "&takes_number=n3"),
        )
        .await;
    assert_eq!(saved.status, 303, "{}", saved.text());
    let now = record(&rig, &ana).await;
    assert_eq!(now.takes_orders_on.unwrap().digits(), phone(33));
    assert_eq!(now.orders_from[0].digits(), phone(34));
    // The window shows it again, chosen.
    let again = rig.get(&window_path(&ana)).await.text();
    assert!(
        again.contains("name=\"takes\" value=\"number\" checked>"),
        "{again}"
    );
    assert!(again.contains("<option value=\"n3\" selected>"), "{again}");

    let bia = rig.world.fixture.hermes().await;
    let unchosen = rig
        .post(
            "/hermes/whatsapp",
            &form(&bia, "number", "", "list", &[], "", "&takes_number="),
        )
        .await;
    assert_eq!(unchosen.status, 200);
    said_first(&unchosen.text(), "Choose an option for its WhatsApp.");
    // A number another Hermes takes is a disabled option.
    let taken = rig.get(&window_path(&bia)).await.text();
    assert!(taken.contains("<option value=\"n3\" disabled>"), "{taken}");
    assert!(
        taken.contains(&format!("n3 +{} · {ana} takes orders here", phone(33))),
        "{taken}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_window_that_asks_names_the_chats_and_its_x_goes_back() {
    let rig = ready().await;
    let ana = rig.world.fixture.hermes().await;
    set(&rig, &ana, Some(2), &[], true).await;
    let before = everything(&rig.world.fixture).await;

    let asked = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &["operador"], "", ""),
        )
        .await;
    assert_eq!(asked.status, 200);
    let asked = asked.text();
    assert!(
        asked.contains("These chats stop being protected"),
        "{asked}"
    );
    assert!(
        asked.contains(&format!(
            "Between ponte {}, its WhatsApp until now, and:",
            plus(2)
        )),
        "{asked}"
    );
    assert!(asked.contains("itself (self-chat mode)"));
    assert!(asked.contains(">Remove the protection</button>"));
    assert!(asked.contains(
        "<button class=\"close\" type=\"submit\" form=\"orders-back\" aria-label=\"Go back\">"
    ));
    assert!(
        asked.contains("<form id=\"orders-back\" method=\"post\" action=\"/hermes/whatsapp\">")
    );
    assert!(asked.contains("name=\"back\" value=\"1\""));
    assert_eq!(before, everything(&rig.world.fixture).await);
    // A chat that is a number of the hub is named by its alias, and the one that is not by its telephone.
    set(&rig, &ana, Some(2), &[1, 3], false).await;
    let asked = rig
        .post(
            "/hermes/whatsapp",
            &form(&ana, "number:ponte", "", "list", &[], "", ""),
        )
        .await
        .text();
    let warnings = asked.split("<div class=\"warnings\">").nth(1).unwrap();
    let warnings = &warnings[..warnings.find("</div>").unwrap()];
    assert!(
        warnings.contains(&format!(
            "<span class=\"name\">operador</span> <span class=\"mono\">{}</span>",
            plus(1)
        )),
        "{warnings}"
    );
    assert!(warnings.contains(&format!("<span class=\"mono\">{}</span>", plus(3))));
}
