//! What the three pages show, read from a hub with Hermes, numbers and accesses in it.

use arauto_hub::core::access::{Grant, Scope, Verbs};
use arauto_testkit::hub::fixture::Fixture;

use crate::world::{World, phone};
use crate::{Browser, code, cookie_of, hub};

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
        .find(&format!("<span class=\"nome\">{name}</span>"))
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
    assert!(hermes.contains("Nenhum Hermes ainda"));
    assert!(hermes.contains("0 conectados · 0 aguardando · 0 em alerta"));
    assert!(
        !hermes.contains("class=\"alertas\""),
        "nothing to warn about"
    );
    assert!(hermes.contains("<a href=\"/\" aria-current=\"page\">Hermes</a>"));

    let numbers = page(&browser, &cookie, "/whatsapps").await;
    assert!(numbers.contains("Nenhum número ainda"));
    assert!(numbers.contains("Ainda não há motor"));
    assert!(numbers.contains("<a href=\"/whatsapps\" aria-current=\"page\">WhatsApps</a>"));

    let accesses = page(&browser, &cookie, "/accesses").await;
    assert!(accesses.contains("Conecte um Hermes primeiro"));
    assert!(accesses.contains("<a href=\"/accesses\" aria-current=\"page\">Acessos</a>"));
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
        (&alone, "estado parado", "Sem aparelho"),
        (&invited, "estado atencao", "Aguardando o Hermes"),
        (&asking, "estado atencao", "Aguardando seu código"),
        (
            &approved,
            "estado atencao",
            "Aprovado, aguardando o conector",
        ),
        (&connected, "estado", "Conectado"),
    ] {
        let line = line(&shown, name);
        assert!(
            line.contains(&format!("<span class=\"{class}\">{state}</span>")),
            "{name}: {line}"
        );
    }
    assert!(
        shown.contains("1 conectado · 3 aguardando · 0 em alerta"),
        "{shown}"
    );
    assert!(line(&shown, &connected).contains(">agora</td>"));
    assert!(line(&shown, &alone).contains(">nunca</td>"));
    // The code the admin types to approve is never on a page.
    assert!(!shown.contains(&typed), "the panel shows no code");

    // Only the running hub knows who is in a session: the page follows it.
    drop(session);
    let mut gone = false;
    for _ in 0..200 {
        let shown = page(&browser, &cookie, "/").await;
        if line(&shown, &connected).contains("<span class=\"estado parado\">Desconectado</span>") {
            assert!(shown.contains("0 conectados · 3 aguardando · 0 em alerta"));
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
        balcao.contains("<span class=\"estado\">Conectado</span>"),
        "{balcao}"
    );
    assert!(balcao.contains(&ana) && balcao.contains(&bia), "{balcao}");
    assert!(!balcao.contains(&nobody), "{balcao}");
    let compras = line(&numbers, "compras");
    assert!(
        compras.contains("<span class=\"estado parado\">Sem pareamento</span>"),
        "{compras}"
    );
    assert!(
        compras.contains(&bia) && !compras.contains(&ana),
        "{compras}"
    );
    assert!(
        numbers.contains("\">Motor</span>"),
        "the engine has its line"
    );
    assert!(!numbers.contains("Ainda não há motor"));
    // Nothing of the engine that is a secret or an address is on the page.
    assert!(!numbers.contains(crate::world::KEY));
    assert!(!numbers.contains(&world.fake.address()));

    let hermes = page(&browser, &cookie, "/").await;
    assert!(line(&hermes, &ana).contains(">balcao</td>"));
    assert!(line(&hermes, &bia).contains(">balcao, compras</td>"));
    assert!(line(&hermes, &nobody).contains(">nenhum</td>"));

    let matrix = page(&browser, &cookie, "/accesses").await;
    let read_send_wake = "<span>L</span><span>E</span><span class=\"nao\">M</span><span>A</span>";
    let read_send =
        "<span>L</span><span>E</span><span class=\"nao\">M</span><span class=\"nao\">A</span>";
    let read = "<span>L</span><span class=\"nao\">E</span><span class=\"nao\">M</span>\
                <span class=\"nao\">A</span>";
    let nothing = "<span class=\"nao\">L</span><span class=\"nao\">E</span>\
                   <span class=\"nao\">M</span><span class=\"nao\">A</span>";
    // The columns are the numbers by alias: balcao, then compras.
    let of_ana = matrix_line(&matrix, &ana);
    assert!(
        of_ana.find(read_send_wake).unwrap() < of_ana.find(nothing).unwrap(),
        "{of_ana}"
    );
    assert_eq!(of_ana.matches("sem acesso").count(), 1);
    let of_bia = matrix_line(&matrix, &bia);
    assert!(
        of_bia.find(read_send).unwrap() < of_bia.find(read).unwrap(),
        "{of_bia}"
    );
    assert_eq!(of_bia.matches(">todas<").count(), 2);
    assert_eq!(
        matrix_line(&matrix, &nobody).matches("sem acesso").count(),
        2
    );
    // What `admin access grant` warns of and does not prevent, the page says too.
    assert!(
        matrix.contains("balcao: 2 Hermes enviam nas mesmas conversas."),
        "{matrix}"
    );
    assert!(!matrix.contains("compras: 2 Hermes"));
    assert!(!matrix.contains("são acordados nas mesmas conversas"));
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
    let answer = browser.enter(&code(&fixture).await).await;
    assert_eq!(answer.status, 303, "{}", answer.text());
    let cookie = cookie_of(&answer).unwrap();

    let shown = page(&browser, &cookie, "/").await;
    assert!(shown.contains("class=\"alertas\""));
    assert!(
        shown.contains("<li class=\"alerta restaurado\">O hub voltou de uma cópia de "),
        "{shown}"
    );
    let line = line(&shown, &name);
    assert!(
        line.contains("<span class=\"estado grave\">Suspenso</span>"),
        "{line}"
    );
    assert!(shown.contains("0 conectados · 0 aguardando · 1 em alerta"));
    assert!(shown.contains(&format!("{name} está suspenso e só volta com convite novo")));

    // Its accesses are kept and none applies: the matrix says the state, not the cells.
    let matrix = page(&browser, &cookie, "/accesses").await;
    assert!(matrix.contains("Adicione um WhatsApp primeiro") || matrix.contains("Suspenso"));
}
