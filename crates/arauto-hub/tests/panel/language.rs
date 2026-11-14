//! The languages of the panel: the browser chooses, the person may choose otherwise, and the
//! choice is a cookie with the tag of a language and nothing else.

use arauto_testkit::http;
use arauto_testkit::hub::support::{PANEL_HOST, PANEL_ORIGIN};

use crate::session::everything;
use crate::{Browser, PATIENCE, hub};

/// The cookie that keeps the choice.
const CHOICE: &str = "arauto-panel-lang";

/// `GET path` as a browser that prefers `accept` sends it, with the cookies it holds.
async fn read(
    browser: &Browser,
    path: &str,
    accept: Option<&str>,
    cookies: Option<&str>,
) -> String {
    let mut headers = vec![("Host", PANEL_HOST)];
    if let Some(accept) = accept {
        headers.push(("Accept-Language", accept));
    }
    if let Some(cookies) = cookies {
        headers.push(("Cookie", cookies));
    }
    let answer = http::get(browser.panel, path, &headers, PATIENCE)
        .await
        .unwrap();
    assert_eq!(answer.status, 200, "{path}: {}", answer.text());
    answer.text()
}

/// The language a page says it is in.
fn lang_of(page: &str) -> &str {
    let start = page
        .find("<html lang=\"")
        .expect("the page says its language")
        + 12;
    &page[start..start + page[start..].find('"').unwrap()]
}

#[tokio::test(flavor = "multi_thread")]
async fn the_browser_chooses_the_language_and_the_person_may_choose_another() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);

    // No preference, or one the panel does not speak: English.
    for accept in [None, Some("it-IT, nl;q=0.8"), Some(""), Some("<script>")] {
        let page = read(&browser, "/sign-in", accept, None).await;
        assert_eq!(lang_of(&page), "en", "{accept:?}");
        assert!(page.contains("Welcome to your Arauto hub"));
    }
    // The first language of the browser that the panel speaks.
    for (accept, tag, title) in [
        (
            "pt-BR,pt;q=0.9,en;q=0.8",
            "pt-BR",
            "Boas-vindas ao seu hub do Arauto",
        ),
        ("pt-PT", "pt-BR", "Boas-vindas ao seu hub do Arauto"),
        ("it, es;q=0.9, en;q=0.1", "es", ""),
        ("fr-CA", "fr", ""),
        ("de;q=0.7, ja;q=0.9", "ja", ""),
        ("zh-Hans-CN", "zh-CN", ""),
        ("ru", "ru", ""),
        ("hi-IN", "hi", ""),
        ("DE", "de", ""),
    ] {
        let page = read(&browser, "/sign-in", Some(accept), None).await;
        assert_eq!(lang_of(&page), tag, "{accept}");
        assert!(page.contains(title), "{accept}: {page}");
        // The sentence is not the English one, and the page still has its form and its field.
        // Every language, not only Portuguese: a translation pasted as the English sentence
        // would leave step 1 in English, and the completeness test of the catalogs does not see
        // that.
        assert!(
            !page.contains("Welcome to your Arauto hub"),
            "{accept} was left in English"
        );
        assert!(page.contains("<form class=\"code-form\" method=\"post\" action=\"/sign-in\""));
        // Every language is offered by its own name, with the one of the page marked.
        assert!(
            page.contains(&format!("<option value=\"{tag}\" selected>")),
            "{accept}"
        );
        assert_eq!(page.matches("<option value=").count(), 9);
        assert_eq!(page.matches(" selected>").count(), 1);
    }

    // The choice: one form, a cookie with the tag, and back to the panel.
    let form = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ];
    let chosen = http::post(browser.panel, "/language", &form, b"lang=fr", PATIENCE)
        .await
        .unwrap();
    assert_eq!(chosen.status, 303);
    assert_eq!(chosen.header("location"), Some("/"));
    let set = chosen.header("set-cookie").unwrap();
    assert!(set.starts_with(&format!("{CHOICE}=fr; ")), "{set}");
    assert!(
        set.contains("Path=/") && set.contains("SameSite=Lax"),
        "{set}"
    );
    // It is the choice and nothing else: no session comes with it.
    assert!(!set.contains("arauto-panel="), "{set}");
    // The choice comes before what the browser prefers.
    let cookie = format!("{CHOICE}=fr");
    let page = read(&browser, "/sign-in", Some("ja, de"), Some(&cookie)).await;
    assert_eq!(lang_of(&page), "fr");
    // A choice that is no language of the list is as if there were none.
    for stale in ["xx", "", "fr-FR-x-<b>", "../../etc"] {
        let cookie = format!("{CHOICE}={stale}");
        let page = read(&browser, "/sign-in", Some("ja"), Some(&cookie)).await;
        assert_eq!(lang_of(&page), "ja", "{stale}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn choosing_a_language_changes_nothing_in_the_hub_and_takes_only_a_language() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let session = browser.signed_in(&fixture).await;
    let before = everything(&fixture).await;

    let form = [
        ("Host", PANEL_HOST),
        ("Origin", PANEL_ORIGIN),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ];
    // What is not a language of the list sets nothing.
    for body in [
        "lang=xx",
        "lang=",
        "other=1",
        "lang=%3Cscript%3E",
        "lang=en%0d%0aSet-Cookie:%20x=1",
    ] {
        let answer = http::post(browser.panel, "/language", &form, body.as_bytes(), PATIENCE)
            .await
            .unwrap();
        assert_eq!(answer.status, 303, "{body}");
        assert!(answer.header("set-cookie").is_none(), "{body}");
        assert_eq!(answer.all("set-cookie").count(), 0, "{body}");
    }
    // A form that did not come from a page of the panel is refused like any other.
    let elsewhere = [("Host", PANEL_HOST), ("Origin", "http://evil.test")];
    let refused = http::post(browser.panel, "/language", &elsewhere, b"lang=de", PATIENCE)
        .await
        .unwrap();
    assert_eq!(refused.status, 403);
    // An address that is opened chooses nothing.
    let opened = http::get(
        browser.panel,
        "/language?lang=de",
        &[("Host", PANEL_HOST)],
        PATIENCE,
    )
    .await
    .unwrap();
    assert_eq!(opened.status, 404);
    assert!(opened.header("set-cookie").is_none());

    // Each language, chosen with a session open: the pages come in it, the session stays.
    for (tag, own_name) in [
        ("pt-BR", "Português"),
        ("es", "Español"),
        ("fr", "Français"),
        ("de", "Deutsch"),
        ("ru", "Русский"),
        ("hi", "हिन्दी"),
        ("zh-CN", "简体中文"),
        ("ja", "日本語"),
        ("en", "English"),
    ] {
        let body = format!("lang={tag}");
        let chosen = http::post(browser.panel, "/language", &form, body.as_bytes(), PATIENCE)
            .await
            .unwrap();
        assert_eq!(chosen.status, 303, "{tag}");
        let cookies = format!("{session}; {CHOICE}={tag}");
        for path in ["/", "/whatsapps", "/accesses", "/hermes/connect"] {
            let page = read(&browser, path, Some("en"), Some(&cookies)).await;
            assert_eq!(lang_of(&page), tag, "{path}");
            assert!(page.contains(&format!(
                "<option value=\"{tag}\" selected>{own_name}</option>"
            )));
            // The names are the same in every language, and the markup is whole.
            assert!(
                page.contains(">Hermes</a>") && page.contains(">WhatsApps</a>"),
                "{tag} {path}"
            );
            assert!(page.ends_with("</html>"), "{tag} {path}");
            assert!(
                !page.contains("{") || path == "/hermes/connect",
                "{tag} {path}: a placeholder was left"
            );
        }
    }
    assert_eq!(
        before,
        everything(&fixture).await,
        "a language was kept in the hub"
    );
}
