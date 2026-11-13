//! The pages of the panel: what each one shows, as HTML.
//!
//! A page is written from plain values that [`super::data`] reads: nothing here touches the
//! database. Every value goes through [`Html::text`]; the markup is the `raw` part.
//!
//! The words of the pages are in Portuguese, as in the prototype the maintainer approved.
//! They are text of a new screen, so they are a proposal (`docs/07-telas.md`).

use super::html::Html;

/// The logo, written by the project: no value of anyone goes inside it.
const LOGO: &str = include_str!("assets/logo.svg");

const HEAD: &str = "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>Painel Arauto[IA]</title><link rel=\"stylesheet\" href=\"/assets/panel.css\">";

pub(super) const TAIL: &str = "</body></html>";

/// What stands over a page of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Over {
    /// Nothing: the page is the whole screen.
    Nothing,
    /// A guide, in a window: the page under it cannot be reached while it is open.
    Guide {
        /// Every how many seconds the page asks the hub again, when it waits for something.
        refresh: Option<u32>,
    },
}

/// The head of a page. Only a page with a guide loads the script of the panel, and it loads
/// it by its address: no page carries a script of its own.
fn head(html: &mut Html, over: Over) {
    html.raw(HEAD);
    if let Over::Guide { refresh } = over {
        if let Some(seconds) = refresh {
            html.raw("<meta http-equiv=\"refresh\" content=\"")
                .number(usize::try_from(seconds).unwrap_or(usize::MAX))
                .raw("\">");
        }
        html.raw("<script src=\"/assets/panel.js\"></script>");
    }
    html.raw("</head><body>");
}

/// How a state reads at a glance: the color of the dot, never the only sign of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// It works.
    Ok,
    /// It waits for someone.
    Wait,
    /// It stopped and needs the admin.
    Bad,
    /// It is not in use.
    Idle,
}

impl Tone {
    const fn state_class(self) -> &'static str {
        match self {
            Tone::Ok => "estado",
            Tone::Wait => "estado atencao",
            Tone::Bad => "estado grave",
            Tone::Idle => "estado parado",
        }
    }
}

/// One alert of the band at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// Whether it is of the kind that stops a Hermes or a number.
    pub grave: bool,
    /// What it is about, in a few fixed words.
    pub lead: &'static str,
    /// The sentence. It may carry a name or an address that came from outside.
    pub text: String,
}

/// Which page is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The Hermes.
    Hermes,
    /// The numbers.
    Numbers,
    /// The matrix of accesses.
    Accesses,
}

/// What every page of a session shows around its own content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frame {
    /// The sentence of the restored mode, while it is on (C11).
    pub restored: Option<String>,
    /// The alerts nobody marked as seen, the newest first.
    pub notices: Vec<Notice>,
    /// How many more there are than the ones shown.
    pub more_notices: usize,
    /// How many devices the hub does not know presented a valid credential.
    pub unknown_devices: usize,
}

/// One line of the list of Hermes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesRow {
    /// The name.
    pub name: String,
    /// The color of its state.
    pub tone: Tone,
    /// Its state, in words.
    pub state: &'static str,
    /// The aliases of the numbers it reaches.
    pub numbers: Vec<String>,
    /// Whether it can be woken.
    pub woken: bool,
    /// The version its connector declared. Not verified.
    pub connector: Option<String>,
    /// When the hub last saw it, in words.
    pub seen: String,
    /// What the admin needs to know about a Hermes that stopped.
    pub explain: Option<String>,
    /// What the button of its line does.
    pub action: RowAction,
}

/// What the admin can do from the line of a Hermes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    /// It has no device and no invite: issue its first invite.
    Connect,
    /// An invite or a request is in progress: go back to the guide.
    Continue,
    /// It has a device: issue an invite for another one.
    Reconnect,
}

/// The page of the Hermes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HermesPage {
    /// The lines, by name.
    pub rows: Vec<HermesRow>,
    /// How many are in a session of the channel now.
    pub connected: usize,
    /// How many wait for the Hermes, for the code or for the connector.
    pub waiting: usize,
    /// How many are in quarantine or suspended.
    pub stopped: usize,
}

/// One line of the list of numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberRow {
    /// The alias.
    pub alias: String,
    /// The telephone, when the key of the organization opened it.
    pub phone: Option<String>,
    /// The color of its state.
    pub tone: Tone,
    /// Its state, in words.
    pub state: &'static str,
    /// Whether the admin marked it as a command channel.
    pub command: bool,
    /// The Hermes with an access to it.
    pub hermes: Vec<String>,
    /// When its last message came, in words. Never what it said.
    pub last_message: String,
}

/// The engine, as the page of the numbers says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineLine {
    /// The color of the line.
    pub tone: Tone,
    /// The sentence. The version in it came from the engine.
    pub text: String,
}

/// The page of the numbers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NumbersPage {
    /// The lines, by alias.
    pub rows: Vec<NumberRow>,
    /// The engine, when there is one.
    pub engine: Option<EngineLine>,
}

/// What a Hermes may do on a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Read, send, modify, be woken: in this order.
    pub verbs: [bool; 4],
    /// Which chats, in words.
    pub scope: String,
}

/// One line of the matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessRow {
    /// The Hermes.
    pub hermes: String,
    /// The state of a Hermes whose device does not serve: its accesses are kept and none
    /// applies.
    pub stopped: Option<&'static str>,
    /// One cell per number, in the order of the columns. `None`: no access.
    pub cells: Vec<Option<Cell>>,
}

/// The page of the accesses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccessPage {
    /// The columns: the alias and the telephone of each number.
    pub numbers: Vec<(String, Option<String>)>,
    /// The lines, by name of Hermes.
    pub rows: Vec<AccessRow>,
    /// What the admin is told and nothing prevents.
    pub warnings: Vec<String>,
}

/// The page where the entry code is typed.
#[must_use]
pub fn sign_in(refusal: Option<&'static str>) -> String {
    let mut html = Html::new();
    head(&mut html, Over::Nothing);
    html.raw("<main class=\"entrada\">")
        .raw(LOGO)
        .raw("<h1>Entrar no painel</h1><form class=\"cartao\" method=\"post\" action=\"/sign-in\" autocomplete=\"off\">")
        .raw("<div class=\"campo\"><label for=\"code\">Código de entrada</label>")
        .raw("<input id=\"code\" name=\"code\" type=\"password\" inputmode=\"text\" autocapitalize=\"none\" spellcheck=\"false\" maxlength=\"64\" required autofocus></div>");
    if let Some(refusal) = refusal {
        html.raw("<p class=\"erro\" role=\"alert\">")
            .raw(refusal)
            .raw("</p>");
    }
    html.raw("<p class=\"dica\">Gere o código no servidor do hub, com <code>arauto-hub admin panel code</code>. Ele vale 10 minutos e uma entrada.</p>")
        .raw("<div><button class=\"botao principal\" type=\"submit\">Entrar</button></div></form></main>")
        .raw(TAIL);
    html.finish()
}

fn open(html: &mut Html, frame: &Frame, tab: Tab, over: Over) {
    head(html, over);
    html.raw(if over == Over::Nothing {
        "<div class=\"pagina\"><header class=\"topo\">"
    } else {
        "<div class=\"pagina\" inert><header class=\"topo\">"
    })
        .raw(LOGO)
        .raw("<div class=\"quem\"><span>admin</span><form method=\"post\" action=\"/sign-out\"><button class=\"sair\" type=\"submit\">Sair</button></form></div></header>");
    let alerts = frame.restored.is_some() || !frame.notices.is_empty() || frame.unknown_devices > 0;
    if alerts {
        html.raw("<ul class=\"alertas\" aria-label=\"Alertas\">");
        if let Some(restored) = &frame.restored {
            html.raw("<li class=\"alerta restaurado\">")
                .text(restored)
                .raw("</li>");
        }
        for notice in &frame.notices {
            html.raw(if notice.grave {
                "<li class=\"alerta grave\"><span class=\"estado grave\">"
            } else {
                "<li class=\"alerta\"><span class=\"estado atencao\">"
            })
            .raw(notice.lead)
            .raw(":</span> ")
            .text(&notice.text)
            .raw("</li>");
        }
        if frame.unknown_devices > 0 {
            html.raw("<li class=\"alerta grave\"><span class=\"estado grave\">Aparelhos desconhecidos:</span> ")
                .number(frame.unknown_devices)
                .raw(" apresentaram uma credencial válida que o banco não explica. O caminho de cada um é um convite novo. Veja com <code>arauto-hub admin alerts</code>.</li>");
        }
        if frame.more_notices > 0 {
            html.raw("<li class=\"alerta\"><span class=\"mais\">E mais ")
                .number(frame.more_notices)
                .raw(
                    " alerta(s). Veja todos com <code>arauto-hub admin alerts</code>.</span></li>",
                );
        }
        html.raw("</ul>");
    }
    html.raw("<nav class=\"abas\" aria-label=\"Painel\">");
    for (this, address, label) in [
        (Tab::Hermes, "<a href=\"/\"", "Hermes"),
        (Tab::Numbers, "<a href=\"/whatsapps\"", "WhatsApps"),
        (Tab::Accesses, "<a href=\"/accesses\"", "Acessos"),
    ] {
        html.raw(address)
            .raw(if this == tab {
                " aria-current=\"page\">"
            } else {
                ">"
            })
            .raw(label)
            .raw("</a>");
    }
    html.raw("</nav><main>");
}

/// Ends the page, up to where a window over it would go.
fn close(html: &mut Html) {
    html.raw("</main><p class=\"rodape\">Nesta versão o painel conecta Hermes. Parear números e dar acesso continuam pelo comando <code>arauto-hub admin</code>, no servidor do hub.</p></div>");
}

fn state(html: &mut Html, tone: Tone, label: &'static str) {
    html.raw("<span class=\"")
        .raw(tone.state_class())
        .raw("\">")
        .raw(label)
        .raw("</span>");
}

fn names(html: &mut Html, list: &[String], none: &'static str) {
    if list.is_empty() {
        html.raw(none);
        return;
    }
    for (index, name) in list.iter().enumerate() {
        if index > 0 {
            html.raw(", ");
        }
        html.text(name);
    }
}

/// The page of the Hermes: the same list `arauto-hub admin hermes list` gives, and whether
/// each one is connected now, which only the running hub knows.
#[must_use]
pub fn hermes(frame: &Frame, page: &HermesPage) -> String {
    let mut html = Html::new();
    hermes_into(&mut html, frame, page, Over::Nothing);
    html.raw(TAIL);
    html.finish()
}

/// Writes the page of the Hermes up to where a window over it would go.
pub(super) fn hermes_into(html: &mut Html, frame: &Frame, page: &HermesPage, over: Over) {
    open(html, frame, Tab::Hermes, over);
    html.raw("<div class=\"cabeca\"><div><h1>Seus Hermes e os <b>[números]</b> deles</h1><p class=\"contadores\">")
        .number(page.connected)
        .raw(if page.connected == 1 { " conectado · " } else { " conectados · " })
        .number(page.waiting)
        .raw(" aguardando · ")
        .number(page.stopped)
        .raw(" em alerta</p></div><a class=\"botao principal\" href=\"/hermes/connect\">Conectar Hermes</a></div>")
        .raw("<div class=\"quadro\"><table class=\"lista\"><thead><tr>")
        .raw("<th scope=\"col\">Hermes</th><th scope=\"col\">Estado</th><th scope=\"col\">Números</th>")
        .raw("<th scope=\"col\">Acordado</th><th scope=\"col\">Conector</th><th scope=\"col\">Visto</th>")
        .raw("<th scope=\"col\"><span class=\"so-leitor\">Ações</span></th></tr></thead><tbody>");
    if page.rows.is_empty() {
        html.raw("<tr class=\"vazio\"><td colspan=\"7\">Nenhum Hermes ainda. Conecte o primeiro pelo botão \"Conectar Hermes\".</td></tr>");
    }
    for row in &page.rows {
        html.raw("<tr><td><span class=\"nome\">")
            .text(&row.name)
            .raw("</span></td><td>");
        state(html, row.tone, row.state);
        html.raw("</td><td class=\"linha2\">");
        names(html, &row.numbers, "nenhum");
        html.raw("</td><td class=\"so-largo\">")
            .raw(if row.woken { "sim" } else { "não" })
            .raw("</td><td class=\"so-largo fixa\">");
        match &row.connector {
            Some(version) => html.text(version),
            None => html.raw("—"),
        };
        html.raw("</td><td class=\"so-largo num\">")
            .text(&row.seen)
            .raw("</td><td class=\"c-acoes\"><div class=\"acoes\">");
        match row.action {
            RowAction::Continue => {
                html.raw("<a class=\"botao\" href=\"/hermes/wait?name=")
                    .text(&row.name)
                    .raw("\">Continuar</a>");
            }
            RowAction::Connect | RowAction::Reconnect => {
                html.raw("<form method=\"post\" action=\"/hermes/invite\"><input type=\"hidden\" name=\"name\" value=\"")
                    .text(&row.name)
                    .raw(if row.action == RowAction::Connect {
                        "\"><button class=\"botao\" type=\"submit\">Conectar</button></form>"
                    } else {
                        "\"><button class=\"botao\" type=\"submit\">Reconectar</button></form>"
                    });
            }
        }
        html.raw("</div></td></tr>");
        if let Some(explain) = &row.explain {
            html.raw("<tr class=\"explica\"><td colspan=\"7\">")
                .text(explain)
                .raw("</td></tr>");
        }
    }
    html.raw("</tbody></table></div><p class=\"nota\">\"Números\" são os que o Hermes alcança. O conector é o que o aparelho declarou: não é verificado.</p>");
    close(html);
}

/// The page of the numbers.
#[must_use]
pub fn numbers(frame: &Frame, page: &NumbersPage) -> String {
    let mut html = Html::new();
    open(&mut html, frame, Tab::Numbers, Over::Nothing);
    html.raw("<div class=\"cabeca\"><h1>WhatsApps</h1><p class=\"contadores\">");
    match &page.engine {
        Some(engine) => {
            html.raw("<span class=\"")
                .raw(engine.tone.state_class())
                .raw("\">Motor</span> ")
                .text(&engine.text);
        }
        None => {
            html.raw("Ainda não há motor. Configure com <code>arauto-hub admin engine set</code>.");
        }
    }
    html.raw("</p></div><div class=\"quadro\"><table class=\"lista\"><thead><tr>")
        .raw("<th scope=\"col\">Apelido</th><th scope=\"col\">Número</th><th scope=\"col\">Estado</th>")
        .raw("<th scope=\"col\">Hermes com acesso</th><th scope=\"col\">Última mensagem</th></tr></thead><tbody>");
    if page.rows.is_empty() {
        html.raw("<tr class=\"vazio\"><td colspan=\"5\">Nenhum número ainda. Adicione com <code>arauto-hub admin number add APELIDO TELEFONE</code>.</td></tr>");
    }
    for row in &page.rows {
        html.raw("<tr><td><span class=\"nome\">")
            .text(&row.alias)
            .raw("</span>");
        if row.command {
            html.raw("<span class=\"marca\">de comando</span>");
        }
        html.raw("</td><td class=\"so-largo fixa\">");
        match &row.phone {
            Some(phone) => html.text(phone),
            None => html.raw("—"),
        };
        html.raw("</td><td>");
        state(&mut html, row.tone, row.state);
        html.raw("</td><td class=\"linha2\">");
        names(&mut html, &row.hermes, "nenhum");
        html.raw("</td><td class=\"so-largo num\">")
            .text(&row.last_message)
            .raw("</td></tr>");
    }
    html.raw("</tbody></table></div><p class=\"nota\">A última mensagem aparece só com o tempo, sem conteúdo. O painel lista o que passa pelo hub: outros dispositivos conectados ao mesmo número só aparecem no celular.</p>");
    close(&mut html);
    html.raw(TAIL);
    html.finish()
}

/// The page of the accesses: the matrix, to read.
#[must_use]
pub fn accesses(frame: &Frame, page: &AccessPage) -> String {
    let mut html = Html::new();
    open(&mut html, frame, Tab::Accesses, Over::Nothing);
    html.raw("<div class=\"cabeca\"><h1>Acessos</h1><p class=\"contadores\">Nas linhas, os Hermes. Nas colunas, os números.</p></div>");
    if page.rows.is_empty() || page.numbers.is_empty() {
        html.raw("<div class=\"quadro\"><table class=\"lista\"><tbody><tr class=\"vazio\"><td>")
            .raw(if page.rows.is_empty() {
                "Conecte um Hermes primeiro."
            } else {
                "Adicione um WhatsApp primeiro."
            })
            .raw("</td></tr></tbody></table></div>");
        close(&mut html);
        html.raw(TAIL);
        return html.finish();
    }
    html.raw("<div class=\"quadro\"><div class=\"rola\"><table class=\"matriz\"><thead><tr><th scope=\"col\">Hermes</th>");
    for (alias, phone) in &page.numbers {
        html.raw("<th scope=\"col\">").text(alias);
        if let Some(phone) = phone {
            html.raw("<span class=\"tel\">").text(phone).raw("</span>");
        }
        html.raw("</th>");
    }
    html.raw("</tr></thead><tbody>");
    for row in &page.rows {
        html.raw("<tr><th scope=\"row\">")
            .text(&row.hermes)
            .raw("</th>");
        if let Some(stopped) = row.stopped {
            html.raw("<td colspan=\"")
                .number(page.numbers.len())
                .raw("\"><span class=\"estado grave\">")
                .raw(stopped)
                .raw("</span><span class=\"guardado\"> (acessos preservados)</span></td></tr>");
            continue;
        }
        for cell in &row.cells {
            html.raw("<td><span class=\"celula\"><span class=\"verbos\">");
            let verbs = cell.as_ref().map_or([false; 4], |cell| cell.verbs);
            for (granted, letter) in verbs.iter().zip(["L", "E", "M", "A"]) {
                html.raw(if *granted {
                    "<span>"
                } else {
                    "<span class=\"nao\">"
                })
                .raw(letter)
                .raw("</span>");
            }
            html.raw("</span><span class=\"escopo\">");
            match cell {
                Some(cell) => html.text(&cell.scope),
                None => html.raw("sem acesso"),
            };
            html.raw("</span></span></td>");
        }
        html.raw("</tr>");
    }
    html.raw("</tbody></table></div></div><p class=\"nota\"><b>L</b> ler · <b>E</b> enviar · <b>M</b> modificar · <b>A</b> ser acordado.</p>");
    if !page.warnings.is_empty() {
        html.raw("<div class=\"avisos\"><b>Avisos que não bloqueiam</b><ul>");
        for warning in &page.warnings {
            html.raw("<li>").text(warning).raw("</li>");
        }
        html.raw("</ul></div>");
    }
    close(&mut html);
    html.raw(TAIL);
    html.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    /// What a page may contain besides what the program wrote: nothing that opens markup.
    fn clean(page: &str) {
        assert!(!page.contains("<script"), "{page}");
        assert!(!page.contains("<img"), "{page}");
        // The quotes of the value did not close an attribute nor an element.
        assert!(!page.contains("\"'>"), "{page}");
        assert!(
            page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{page}"
        );
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.ends_with("</html>"));
    }

    fn hostile_frame() -> Frame {
        Frame {
            restored: Some(HOSTILE.to_owned()),
            notices: vec![Notice {
                grave: true,
                lead: "Conexão tomada",
                text: HOSTILE.to_owned(),
            }],
            more_notices: 2,
            unknown_devices: 1,
        }
    }

    #[test]
    fn what_a_device_or_the_engine_wrote_is_shown_as_text_on_every_page() {
        let frame = hostile_frame();
        let hermes_page = HermesPage {
            rows: vec![HermesRow {
                name: HOSTILE.to_owned(),
                tone: Tone::Bad,
                state: "Em quarentena",
                numbers: vec![HOSTILE.to_owned(), HOSTILE.to_owned()],
                woken: true,
                connector: Some(HOSTILE.to_owned()),
                seen: HOSTILE.to_owned(),
                explain: Some(HOSTILE.to_owned()),
                action: RowAction::Reconnect,
            }],
            connected: 0,
            waiting: 0,
            stopped: 1,
        };
        let page = hermes(&frame, &hermes_page);
        clean(&page);
        // The values of the row (its name twice: in the list and in the form of its button),
        // and the ones of the frame: every one came out escaped.
        assert_eq!(page.matches("&lt;script&gt;").count(), 9);

        let numbers_page = NumbersPage {
            rows: vec![NumberRow {
                alias: HOSTILE.to_owned(),
                phone: Some(HOSTILE.to_owned()),
                tone: Tone::Ok,
                state: "Conectado",
                command: true,
                hermes: vec![HOSTILE.to_owned()],
                last_message: HOSTILE.to_owned(),
            }],
            engine: Some(EngineLine {
                tone: Tone::Wait,
                text: HOSTILE.to_owned(),
            }),
        };
        clean(&numbers(&frame, &numbers_page));

        let access_page = AccessPage {
            numbers: vec![(HOSTILE.to_owned(), Some(HOSTILE.to_owned()))],
            rows: vec![AccessRow {
                hermes: HOSTILE.to_owned(),
                stopped: None,
                cells: vec![Some(Cell {
                    verbs: [true, false, false, true],
                    scope: HOSTILE.to_owned(),
                })],
            }],
            warnings: vec![HOSTILE.to_owned()],
        };
        clean(&accesses(&frame, &access_page));
    }

    #[test]
    fn the_pages_say_when_there_is_nothing_and_never_run_a_script() {
        let empty = Frame::default();
        for page in [
            hermes(&empty, &HermesPage::default()),
            numbers(&empty, &NumbersPage::default()),
            accesses(&empty, &AccessPage::default()),
            sign_in(None),
            sign_in(Some("Código inválido ou vencido.")),
        ] {
            assert!(
                !page.contains("<script"),
                "a page without a guide loads no script"
            );
            assert!(
                !page.contains(" style=\""),
                "the policy of the page blocks inline styles"
            );
            assert!(!page.contains("javascript:"));
            assert!(page.contains("/assets/panel.css"));
        }
        assert!(hermes(&empty, &HermesPage::default()).contains("Nenhum Hermes ainda"));
        assert!(numbers(&empty, &NumbersPage::default()).contains("Nenhum número ainda"));
        assert!(accesses(&empty, &AccessPage::default()).contains("Conecte um Hermes primeiro"));
        // The band of alerts only exists when there is something to say.
        assert!(!hermes(&empty, &HermesPage::default()).contains("class=\"alertas\""));
        assert!(hermes(&hostile_frame(), &HermesPage::default()).contains("class=\"alertas\""));
    }

    #[test]
    fn the_matrix_shows_each_verb_and_keeps_a_stopped_hermes_apart() {
        let page = AccessPage {
            numbers: vec![("balcao".to_owned(), None), ("compras".to_owned(), None)],
            rows: vec![
                AccessRow {
                    hermes: "ana".to_owned(),
                    stopped: None,
                    cells: vec![
                        Some(Cell {
                            verbs: [true, true, false, true],
                            scope: "todas".to_owned(),
                        }),
                        None,
                    ],
                },
                AccessRow {
                    hermes: "antigo".to_owned(),
                    stopped: Some("Em quarentena"),
                    cells: Vec::new(),
                },
            ],
            warnings: Vec::new(),
        };
        let html = accesses(&Frame::default(), &page);
        assert!(
            html.contains("<span>L</span><span>E</span><span class=\"nao\">M</span><span>A</span>")
        );
        assert!(html.contains("sem acesso"));
        assert!(
            html.contains("<td colspan=\"2\"><span class=\"estado grave\">Em quarentena</span>")
        );
        assert!(!html.contains("class=\"avisos\""));
    }
}
