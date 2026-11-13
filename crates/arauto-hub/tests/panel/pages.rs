//! What the three pages show, read from a hub with Hermes, numbers and accesses in it.

use arauto_hub::core::access::{Grant, Scope, Verbs};
use arauto_testkit::hub::fixture::Fixture;

use crate::world::{World, phone};
use crate::{Browser, hub};

const fn all(read: bool, send: bool, modify: bool, wake: bool) -> Grant {
    Grant {
        scope: Scope::All,
        verbs: Verbs {
            read,
            send,
            modify,
            wake,
        },
        can_start_chats: false,
    }
}

/// The line of a list that is about `name`: from its name to the end of its row.
fn line<'p>(page: &'p str, name: &str) -> &'p str {
    let start = page
        .find(&format!("<span class=\"name\">{name}</span>"))
        .unwrap_or_else(|| panic!("{name} is not on the page: {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</tr>").expect("the row ends")]
}

/// The line of the matrix of the Hermes `name`.
fn matrix_line<'p>(page: &'p str, name: &str) -> &'p str {
    let start = page
        .find(&format!("<th scope=\"row\">{name}</th>"))
        .unwrap_or_else(|| panic!("{name} is not in the matrix: {page}"));
    let rest = &page[start..];
    &rest[..rest.find("</tr>").expect("the row ends")]
}

async fn page(browser: &Browser, cookie: &str, path: &str) -> String {
    let answer = browser.get(path, Some(cookie)).await;
    assert_eq!(answer.status, 200, "{path}: {}", answer.text());
    assert_eq!(
        answer.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    answer.text()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_hub_says_what_to_do_first() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let hermes = page(&browser, &cookie, "/").await;
    assert!(hermes.contains("No Hermes yet"));
    assert!(hermes.contains("Connected: 0 · Waiting: 0 · On alert: 0"));
    assert!(
        !hermes.contains("class=\"alerts\""),
        "nothing to warn about"
    );
    assert!(hermes.contains("<a href=\"/\" aria-current=\"page\">Hermes</a>"));

    let numbers = page(&browser, &cookie, "/whatsapps").await;
    assert!(numbers.contains("No number yet"));
    assert!(numbers.contains("There is no engine yet"));
    assert!(numbers.contains("<a href=\"/whatsapps\" aria-current=\"page\">WhatsApps</a>"));

    let accesses = page(&browser, &cookie, "/accesses").await;
    assert!(accesses.contains("Connect a Hermes first"));
    assert!(accesses.contains("<a href=\"/accesses\" aria-current=\"page\">Accesses</a>"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_page_of_the_hermes_shows_each_one_in_its_state() {
    let fixture = hub().await;
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let alone = fixture.hermes().await;
    let invited = fixture.hermes().await;
    fixture.invite(&invited).await;
    let asking = fixture.hermes().await;
    let device = fixture.device();
    let typed = fixture.enroll(&asking, &device).await;
    let (approved, _device) = fixture.approved().await;
    let (connected, session) = fixture.connected().await;

    let shown = page(&browser, &cookie, "/").await;
    for (name, class, state) in [
        (&alone, "state idle", "No device"),
        (&invited, "state wait", "Waiting for the Hermes"),
        (&asking, "state wait", "Waiting for your code"),
        (
            &approved,
            "state wait",
            "Approved, waiting for the connector",
        ),
        (&connected, "state", "Connected"),
    ] {
        let line = line(&shown, name);
        assert!(
            line.contains(&format!("<span class=\"{class}\">{state}</span>")),
            "{name}: {line}"
        );
    }
    assert!(
        shown.contains("Connected: 1 · Waiting: 3 · On alert: 0"),
        "{shown}"
    );
    assert!(line(&shown, &connected).contains(">now</td>"));
    assert!(line(&shown, &alone).contains(">never</td>"));
    // The code the admin types to approve is never on a page.
    assert!(!shown.contains(&typed), "the panel shows no code");

    // Only the running hub knows who is in a session: the page follows it.
    drop(session);
    let mut gone = false;
    for _ in 0..200 {
        let shown = page(&browser, &cookie, "/").await;
        if line(&shown, &connected).contains("<span class=\"state idle\">Disconnected</span>") {
            assert!(shown.contains("Connected: 0 · Waiting: 3 · On alert: 0"));
            gone = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(gone, "a Hermes that left is still shown as connected");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_numbers_and_the_matrix_are_the_ones_of_the_admin_command() {
    let world = World::new().await;
    let fixture: &Fixture = &world.fixture;
    let browser = Browser::of(fixture);
    let cookie = browser.signed_in(fixture).await;

    world.connected("balcao", &phone(1)).await;
    world.add("compras", &phone(2)).await;
    let ana = fixture.hermes().await;
    let bia = fixture.hermes().await;
    let nobody = fixture.hermes().await;
    let admin = &fixture.admin;
    let now = world.now();
    admin
        .access_grant(&ana, "balcao", all(true, true, false, true), now)
        .await
        .unwrap();
    admin
        .access_grant(&bia, "balcao", all(true, true, false, false), now)
        .await
        .unwrap();
    admin
        .access_grant(&bia, "compras", all(true, false, false, false), now)
        .await
        .unwrap();

    let numbers = page(&browser, &cookie, "/whatsapps").await;
    let balcao = line(&numbers, "balcao");
    assert!(balcao.contains("+55119876"), "{balcao}");
    assert!(balcao.contains(&format!("+{}", phone(1))), "{balcao}");
    assert!(
        balcao.contains("<span class=\"state\">Connected</span>"),
        "{balcao}"
    );
    assert!(balcao.contains(&ana) && balcao.contains(&bia), "{balcao}");
    assert!(!balcao.contains(&nobody), "{balcao}");
    let compras = line(&numbers, "compras");
    assert!(
        compras.contains("<span class=\"state idle\">Not paired</span>"),
        "{compras}"
    );
    assert!(
        compras.contains(&bia) && !compras.contains(&ana),
        "{compras}"
    );
    assert!(
        numbers.contains("\">Engine</span>"),
        "the engine has its line"
    );
    assert!(!numbers.contains("There is no engine yet"));
    // Nothing of the engine that is a secret or an address is on the page.
    assert!(!numbers.contains(crate::world::KEY));
    assert!(!numbers.contains(&world.fake.address()));

    let hermes = page(&browser, &cookie, "/").await;
    assert!(line(&hermes, &ana).contains(">balcao</td>"));
    assert!(line(&hermes, &bia).contains(">balcao, compras</td>"));
    assert!(line(&hermes, &nobody).contains(">none</td>"));

    let matrix = page(&browser, &cookie, "/accesses").await;
    let read_send_wake = "<span>R</span><span>S</span><span class=\"off\">M</span><span>W</span>";
    let read_send =
        "<span>R</span><span>S</span><span class=\"off\">M</span><span class=\"off\">W</span>";
    let read = "<span>R</span><span class=\"off\">S</span><span class=\"off\">M</span>\
                <span class=\"off\">W</span>";
    let nothing = "<span class=\"off\">R</span><span class=\"off\">S</span>\
                   <span class=\"off\">M</span><span class=\"off\">W</span>";
    // The columns are the numbers by alias: balcao, then compras.
    let of_ana = matrix_line(&matrix, &ana);
    assert!(
        of_ana.find(read_send_wake).unwrap() < of_ana.find(nothing).unwrap(),
        "{of_ana}"
    );
    assert_eq!(of_ana.matches("no access").count(), 1);
    let of_bia = matrix_line(&matrix, &bia);
    assert!(
        of_bia.find(read_send).unwrap() < of_bia.find(read).unwrap(),
        "{of_bia}"
    );
    assert_eq!(of_bia.matches(">all<").count(), 2);
    assert_eq!(
        matrix_line(&matrix, &nobody).matches("no access").count(),
        2
    );
    // What `admin access grant` warns of and does not prevent, the page says too.
    assert!(
        matrix.contains("balcao: 2 Hermes send in the same chats."),
        "{matrix}"
    );
    assert!(!matrix.contains("compras: 2 Hermes"));
    assert!(!matrix.contains("are woken in the same chats"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hub_that_came_back_from_a_copy_says_so_and_still_lets_the_admin_in() {
    let mut fixture = hub().await;
    let (name, session) = fixture.connected().await;
    drop(session);
    fixture.stop().await;
    fixture.restore_an_older_copy().await;
    fixture.start().await;
    assert!(fixture.restored_mode().await);

    // The panel is how the admin sees what happened: the entry works in this mode.
    let browser = Browser::of(&fixture);
    let cookie = browser.signed_in(&fixture).await;

    let shown = page(&browser, &cookie, "/").await;
    assert!(shown.contains("class=\"alerts\""));
    assert!(
        shown.contains("<li class=\"alert restored\">The hub came back from a backup of "),
        "{shown}"
    );
    let line = line(&shown, &name);
    assert!(
        line.contains("<span class=\"state bad\">Suspended</span>"),
        "{line}"
    );
    assert!(shown.contains("Connected: 0 · Waiting: 0 · On alert: 1"));
    assert!(shown.contains(&format!(
        "{name} is suspended and only comes back with a new invite"
    )));

    // Its accesses are kept and none applies: the matrix says the state, not the cells.
    let matrix = page(&browser, &cookie, "/accesses").await;
    assert!(matrix.contains("Add a WhatsApp first") || matrix.contains("Suspended"));
}
