//! The guide that connects a Hermes: the steps of `docs/02-como-funciona.md`, section 1, as a
//! window over the list of Hermes, with the path of the steps at its top.
//!
//! Each step is a page of the hub. The page that shows the block is the answer to the form
//! that issued the invite, and the invite is in no other page: opening an address never
//! shows one again. The verification code is typed by the admin and never shown here, not
//! even back to whoever typed it wrong.
//!
//! Like [`super::view`], this module only writes: every value goes through [`Html::text`].

use uuid::Uuid;
use zeroize::Zeroizing;

use super::html::Html;
use super::view::{self, Frame, HermesPage, Over};

/// The mascot, drawn by the project frame by frame: no value of anyone goes inside it.
const DANCING: &str = include_str!("assets/mascote-danca.svg");
const WAVING: &str = include_str!("assets/mascote-aceno.svg");

/// The names of the steps, in the path at the top.
const STEPS: [&str; 4] = ["Nome", "Bloco", "Código", "Pronto"];

/// How often a page that waits for the Hermes asks the hub again, in seconds.
const WAITING_REFRESH_S: u32 = 3;

/// What the hub saw and what the device said, of a request that waits for its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The address the enrollment came from, as the hub saw it.
    pub address: Option<String>,
    /// How long ago, in words.
    pub when: String,
    /// The name of the machine. Said by the device, not verified.
    pub hostname: String,
    /// Its system. Said by the device.
    pub os: String,
    /// The version of its Hermes. Said by the device.
    pub hermes: String,
}

/// The last use of the device a reconnect replaces, as the admin is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastUse {
    /// The device.
    pub device: Uuid,
    /// When, in milliseconds: it goes back with the answer, to say which use was answered.
    pub at: Option<i64>,
    /// The state of the device, as stored.
    pub state: &'static str,
    /// When, in words.
    pub when: String,
    /// From where. Stored as it came: shown escaped.
    pub address: Option<String>,
}

/// Why a request ended without an approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The fifth wrong code.
    Denied,
    /// The 10 minutes of the code passed.
    Expired,
    /// There is nothing to wait for: no invite and no request.
    Nothing,
    /// The identity was copied before the approval.
    Copied,
}

/// One screen of the guide.
pub enum Screen {
    /// Step 1: which name the Hermes has.
    Name {
        /// What was typed, when the form came back.
        typed: String,
        /// Why it came back.
        error: Option<&'static str>,
    },
    /// Step 2: the block, the only time it appears.
    Block {
        /// The Hermes.
        name: String,
        /// The text of the block. **It holds the invite.**
        text: Zeroizing<String>,
        /// For how many seconds the invite is still accepted.
        seconds: i64,
        /// The Hermes has a device in use.
        reconnect: bool,
        /// An invite issued before was cancelled by this one.
        cancelled: bool,
        /// A request in progress was closed by this one.
        closed_request: bool,
    },
    /// The hub serves no connector, so there is no block to make. Nothing was issued.
    NoConnector {
        /// The Hermes.
        name: String,
    },
    /// Step 3, while no device used the invite.
    Waiting {
        /// The Hermes.
        name: String,
        /// For how many seconds the invite is still accepted.
        seconds: i64,
        /// The Hermes has a device in use.
        reconnect: bool,
    },
    /// Step 3: a device used the invite and the admin types its code.
    Code {
        /// The Hermes.
        name: String,
        /// What is known of the request.
        facts: Box<Facts>,
        /// On a reconnect, the last use of the device in use.
        last_use: Option<Box<LastUse>>,
        /// What the last answer got wrong.
        error: Option<String>,
        /// For how many seconds the code is still accepted.
        seconds: i64,
    },
    /// Step 3 ended without an approval.
    Ended {
        /// The Hermes.
        name: String,
        /// Why.
        why: Ended,
    },
    /// Step 4: approved, or already connected.
    Done {
        /// The Hermes.
        name: String,
        /// The connector confirmed its first credential.
        connected: bool,
        /// The Hermes had a device in use when this one was approved.
        reconnect: bool,
    },
    /// The hub did not do what was asked.
    Failed {
        /// One fixed sentence.
        sentence: &'static str,
    },
}

/// A screen may hold a block, and a block holds an invite: what is printed of a screen is
/// only which one it is.
impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Screen::Name { .. } => "Screen::Name",
            Screen::Block { .. } => "Screen::Block",
            Screen::NoConnector { .. } => "Screen::NoConnector",
            Screen::Waiting { .. } => "Screen::Waiting",
            Screen::Code { .. } => "Screen::Code",
            Screen::Ended { .. } => "Screen::Ended",
            Screen::Done { .. } => "Screen::Done",
            Screen::Failed { .. } => "Screen::Failed",
        })
    }
}

impl Screen {
    /// Which step of the path the screen is.
    const fn step(&self) -> usize {
        match self {
            Screen::Name { .. } | Screen::Failed { .. } => 0,
            Screen::Block { .. } | Screen::NoConnector { .. } => 1,
            Screen::Waiting { .. } | Screen::Code { .. } | Screen::Ended { .. } => 2,
            Screen::Done { .. } => 3,
        }
    }

    /// The Hermes the screen is about, and whether it had a device in use.
    fn subject(&self) -> Option<(&str, bool)> {
        match self {
            Screen::Name { .. } | Screen::Failed { .. } => None,
            Screen::Block {
                name, reconnect, ..
            }
            | Screen::Waiting {
                name, reconnect, ..
            }
            | Screen::Done {
                name, reconnect, ..
            } => Some((name, *reconnect)),
            Screen::Code { name, last_use, .. } => Some((name, last_use.is_some())),
            Screen::NoConnector { name } | Screen::Ended { name, .. } => Some((name, false)),
        }
    }

    /// A page that waits for something the hub observes asks again by itself.
    const fn refresh(&self) -> Option<u32> {
        match self {
            Screen::Waiting { .. } => Some(WAITING_REFRESH_S),
            _ => None,
        }
    }
}

/// `MM:SS`, for a time left that the script of the panel then counts down.
fn clock(html: &mut Html, seconds: i64) {
    let seconds = usize::try_from(seconds.max(0)).unwrap_or(0);
    html.raw("<b data-segundos=\"").number(seconds).raw("\">");
    let (minutes, rest) = (seconds / 60, seconds % 60);
    if minutes < 10 {
        html.raw("0");
    }
    html.number(minutes).raw(":");
    if rest < 10 {
        html.raw("0");
    }
    html.number(rest).raw("</b>");
}

fn mascot(html: &mut Html, dancing: bool) {
    html.raw(if dancing {
        "<div class=\"mascote dancando\" aria-hidden=\"true\">"
    } else {
        "<div class=\"mascote acenando\" aria-hidden=\"true\">"
    })
    .raw(if dancing { DANCING } else { WAVING })
    .raw("</div>");
}

/// The lines that light up as the hub observes each thing. `seen` of them are done and the
/// next one is being waited for.
fn observed(html: &mut Html, seen: usize, address: Option<&str>) {
    html.raw("<ul class=\"etapas\">");
    for (index, label) in [
        "Convite emitido",
        "Um aparelho usou o convite",
        "Código conferido e pedido aprovado",
        "Conector confirmou a primeira credencial",
    ]
    .into_iter()
    .enumerate()
    {
        html.raw(if index < seen {
            "<li class=\"ok\">"
        } else if index == seen {
            "<li class=\"espera\">"
        } else {
            "<li>"
        })
        .raw(label);
        if index == 1
            && seen > 1
            && let Some(address) = address
        {
            html.raw(", de ").text(address);
        }
        if index == seen {
            html.raw(": aguardando…");
        }
        html.raw("</li>");
    }
    html.raw("</ul>");
}

/// A hidden field with the name of the Hermes.
fn named(html: &mut Html, name: &str) {
    html.raw("<input type=\"hidden\" name=\"name\" value=\"")
        .text(name)
        .raw("\">");
}

fn wait_address(html: &mut Html, name: &str) {
    html.raw("/hermes/wait?name=").text(name);
}

fn body(html: &mut Html, screen: &Screen) {
    match screen {
        Screen::Name { typed, error } => {
            html.raw("<h3>Que nome este Hermes vai ter no painel?</h3>")
                .raw("<form id=\"guia-form\" method=\"post\" action=\"/hermes/connect\" autocomplete=\"off\">")
                .raw("<div class=\"campo\"><label for=\"guia-nome\">Nome do Hermes</label>")
                .raw("<input id=\"guia-nome\" name=\"name\" type=\"text\" maxlength=\"32\" spellcheck=\"false\" autocapitalize=\"none\" required autofocus value=\"")
                .text(typed)
                .raw("\"><p class=\"dica\">Minúsculas, números, hífen e sublinhado, até 32 caracteres, começando por letra ou número. O nome aparece no bloco e não muda depois.</p></div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"erro\" role=\"alert\">")
                    .raw(error)
                    .raw("</p>");
            }
        }
        Screen::Block {
            name,
            text,
            seconds,
            reconnect,
            cancelled,
            closed_request,
        } => {
            html.raw("<div class=\"centro preparo\">");
            mascot(html, true);
            html.raw("<h3 id=\"guia-preparo\" tabindex=\"-1\">Preparando conexão segura</h3><p class=\"dica\">O hub sorteou o convite de ")
                .text(name)
                .raw(".</p></div><div class=\"depois\"><p class=\"linha-prazo\">Convite para <b>")
                .text(name)
                .raw("</b> · vale por mais ");
            clock(html, *seconds);
            html.raw(" · uso único</p><div class=\"bloco\" id=\"guia-bloco\" aria-describedby=\"guia-como\"><pre id=\"guia-texto\">")
                .text(text)
                .raw("</pre><span class=\"selo\" aria-hidden=\"true\"><span>Copiado</span></span></div>")
                .raw("<p class=\"dica\" id=\"guia-como\"><span class=\"com-script\">Clique no bloco para copiar. </span><span class=\"sem-script\">Selecione o texto todo e copie. </span>Ele só aparece aqui, uma vez.</p>")
                .raw("<p class=\"copiado\" id=\"guia-copiado\" role=\"status\" hidden></p>");
            if *cancelled {
                html.raw("<p class=\"dica\">O convite emitido antes foi cancelado.</p>");
            }
            if *closed_request {
                html.raw("<p class=\"dica\">O pedido que estava em andamento foi encerrado.</p>");
            }
            if *reconnect {
                html.raw("<p class=\"dica\">O aparelho em uso continua funcionando até o novo confirmar a primeira credencial.</p>");
            }
            html.raw("</div>");
        }
        Screen::NoConnector { name } => {
            html.raw("<h3>Este hub não tem conector para servir</h3><p>O bloco manda o Hermes baixar o conector deste hub, e a pasta de downloads está vazia. Nada foi emitido. Emita o convite de ")
                .text(name)
                .raw(" pelo comando <code>arauto-hub admin invite</code>, no servidor do hub.</p>");
        }
        Screen::Waiting { name, .. } => {
            html.raw("<h3>Aguardando o Hermes ").text(name).raw("</h3>");
            observed(html, 1, None);
            html.raw("<p class=\"dica\">Cada etapa acende quando o hub de fato a observa. Se o Hermes pedir aprovação para rodar o comando, quem aprova é você, no canal de comando.</p>");
        }
        Screen::Code {
            name,
            facts,
            last_use,
            error,
            ..
        } => {
            html.raw("<h3>Um aparelho usou o convite de ")
                .text(name)
                .raw("</h3>");
            observed(html, 2, facts.address.as_deref());
            html.raw("<dl class=\"fatos\"><dt>Visto pelo hub</dt><dd>endereço ");
            match &facts.address {
                Some(address) => html.text(address),
                None => html.raw("desconhecido"),
            };
            html.raw(", ")
                .text(&facts.when)
                .raw("</dd><dt>Dito pelo aparelho</dt><dd>máquina \"")
                .text(&facts.hostname)
                .raw("\" · sistema \"")
                .text(&facts.os)
                .raw("\" · Hermes \"")
                .text(&facts.hermes)
                .raw("\" (não verificado)</dd></dl>")
                .raw("<form id=\"guia-negar\" method=\"post\" action=\"/hermes/deny\" hidden>");
            named(html, name);
            html.raw("</form><form id=\"guia-form\" method=\"post\" action=\"/hermes/approve\" autocomplete=\"off\">");
            named(html, name);
            if let Some(last) = last_use {
                html.raw("<input type=\"hidden\" name=\"seen_device\" value=\"")
                    .text(&last.device.as_hyphenated().to_string())
                    .raw("\"><input type=\"hidden\" name=\"seen_at\" value=\"");
                if let Some(at) = last.at {
                    html.text(&at.to_string());
                }
                html.raw("\"><input type=\"hidden\" name=\"seen_state\" value=\"")
                    .raw(last.state)
                    .raw("\"><fieldset class=\"reconhece\"><legend>Último uso do aparelho atual: ")
                    .text(&last.when);
                if let Some(address) = &last.address {
                    html.raw(", de ").text(address);
                }
                html.raw(". Você reconhece?</legend>")
                    .raw("<label class=\"op\"><input type=\"radio\" name=\"use\" value=\"recognized\"><span>Reconheço</span></label>")
                    .raw("<label class=\"op\"><input type=\"radio\" name=\"use\" value=\"not-recognized\"><span>Não reconheço</span></label>")
                    .raw("<p class=\"dica\">Se o Hermes estava desligado e houve uso, alguém usou uma cópia.</p></fieldset>");
            }
            html.raw("<div class=\"campo\"><label for=\"guia-codigo\">Código de conferência</label>")
                .raw("<input class=\"codigo\" id=\"guia-codigo\" name=\"code\" type=\"text\" inputmode=\"numeric\" maxlength=\"7\" placeholder=\"000-000\" spellcheck=\"false\" required autofocus>")
                .raw("<p class=\"dica\">Digite o código que o Hermes respondeu no canal de comando. O painel nunca mostra este código.</p></div></form>");
            if let Some(error) = error {
                html.raw("<p class=\"erro\" role=\"alert\">")
                    .text(error)
                    .raw("</p>");
            }
        }
        Screen::Ended { name, why } => {
            html.raw("<h3>");
            match why {
                Ended::Denied => html
                    .raw("O pedido de ")
                    .text(name)
                    .raw(" foi negado</h3><p>O código foi digitado errado pela quinta vez. Para conectar, emita outro convite.</p>"),
                Ended::Expired => html
                    .raw("O pedido de ")
                    .text(name)
                    .raw(" venceu</h3><p>Os 10 minutos para digitar o código passaram. Para conectar, emita outro convite.</p>"),
                Ended::Copied => html
                    .raw("A identidade de ")
                    .text(name)
                    .raw(" foi copiada</h3><p>Dois aparelhos se apresentaram com a mesma identidade antes da aprovação, e o pedido foi parado. Para conectar, emita outro convite.</p>"),
                Ended::Nothing => html
                    .text(name)
                    .raw(" não tem convite nem pedido aguardando</h3><p>O convite venceu, foi cancelado, ou o pedido já foi resolvido. Se o Hermes não respondeu um código, não aprove; se respondeu AR-102, outro aparelho usou o convite. Para conectar, emita outro convite.</p>"),
            };
        }
        Screen::Done {
            name,
            connected,
            reconnect,
        } => {
            html.raw("<div class=\"centro\">");
            mascot(html, false);
            html.raw("<h3>").text(name).raw(if *connected {
                " está conectado</h3>"
            } else {
                " foi aprovado</h3>"
            });
            observed(html, if *connected { 4 } else { 3 }, None);
            html.raw(match (*connected, *reconnect) {
                (true, _) => "<p class=\"dica\">O que ele pode fazer em cada número está na aba Acessos.</p>",
                (false, true) => "<p class=\"dica\">Assim que o conector confirmar, ele aparece como conectado. Os acessos que ele já tinha voltam a valer para este aparelho.</p>",
                (false, false) => "<p class=\"dica\">Assim que o conector confirmar, ele aparece como conectado. Ele ainda não tem acesso a número nenhum.</p>",
            })
            .raw("</div>");
        }
        Screen::Failed { sentence } => {
            html.raw("<h3>O hub não fez o que foi pedido</h3><p>")
                .raw(sentence)
                .raw("</p>");
        }
    }
}

fn footer(html: &mut Html, screen: &Screen) {
    html.raw("<div class=\"modal-pe\">");
    match screen {
        Screen::Name { .. } => {
            html.raw("<button class=\"botao principal\" type=\"submit\" form=\"guia-form\">Gerar convite</button><a class=\"botao\" href=\"/\">Cancelar</a>");
        }
        Screen::Block { name, .. } => {
            html.raw("<a class=\"botao principal apos-copiar surge\" id=\"guia-avancar\" href=\"");
            wait_address(html, name);
            html.raw(
                "\">Próxima etapa</a><span>Depois de copiar, cole no canal de comando do Hermes ",
            )
            .text(name)
            .raw(". Em nenhum outro lugar.</span>");
        }
        Screen::Waiting { seconds, .. } => {
            html.raw("<span>O convite vale por mais ");
            clock(html, *seconds);
            html.raw(". Esta tela se atualiza sozinha.</span>");
        }
        Screen::Code { seconds, .. } => {
            html.raw("<button class=\"botao principal\" type=\"submit\" form=\"guia-form\">Aprovar</button><button class=\"botao\" type=\"submit\" form=\"guia-negar\">Negar</button><span>Confirme em ");
            clock(html, *seconds);
            html.raw(".</span>");
        }
        Screen::Ended { name, .. } => {
            html.raw("<form method=\"post\" action=\"/hermes/invite\">");
            named(html, name);
            html.raw("<button class=\"botao principal\" type=\"submit\">Emitir outro convite</button></form><a class=\"botao\" href=\"/\">Fechar</a>");
        }
        Screen::Done { .. } => {
            html.raw("<a class=\"botao principal\" href=\"/accesses\">Ver acessos</a><a class=\"botao\" href=\"/\">Concluir</a>");
        }
        Screen::NoConnector { .. } | Screen::Failed { .. } => {
            html.raw("<a class=\"botao principal\" href=\"/\">Fechar</a>");
        }
    }
    html.raw("</div>");
}

/// The page of the Hermes with the guide over it.
#[must_use]
pub fn page(frame: &Frame, hermes: &HermesPage, screen: &Screen) -> String {
    let mut html = Html::new();
    view::hermes_into(
        &mut html,
        frame,
        hermes,
        Over::Guide {
            refresh: screen.refresh(),
        },
    );
    html.raw(if matches!(screen, Screen::Block { .. }) {
        "<dialog open class=\"modal guia prepara\" id=\"guia\" aria-labelledby=\"t-guia\">"
    } else {
        "<dialog open class=\"modal guia\" id=\"guia\" aria-labelledby=\"t-guia\">"
    })
    .raw("<div class=\"modal-topo\"><h2 id=\"t-guia\">");
    match screen.subject() {
        Some((name, true)) => html.raw("Reconectar ").text(name),
        Some((name, false)) => html.raw("Conectar ").text(name),
        None => html.raw("Conectar Hermes"),
    };
    html.raw("</h2><a class=\"fechar\" href=\"/\" aria-label=\"Fechar\"><svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg></a></div>")
        .raw("<ol class=\"percurso\" aria-label=\"Etapas\">");
    let step = screen.step();
    for (index, label) in STEPS.into_iter().enumerate() {
        html.raw(if index < step {
            "<li class=\"feita\"><span class=\"bola\">"
        } else if index == step {
            "<li class=\"atual\" aria-current=\"step\"><span class=\"bola\">"
        } else {
            "<li><span class=\"bola\">"
        })
        .number(index.saturating_add(1))
        .raw("</span><span class=\"rot\">")
        .raw(label)
        .raw("</span></li>");
    }
    html.raw("</ol><div class=\"modal-corpo\">");
    body(&mut html, screen);
    html.raw("</div>");
    footer(&mut html, screen);
    html.raw("</dialog>").raw(view::TAIL);
    html.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTILE: &str = "<script>alert(1)</script>\"'><img src=x onerror=alert(2)>";

    fn shown(screen: &Screen) -> String {
        page(&Frame::default(), &HermesPage::default(), screen)
    }

    fn clean(page: &str) {
        assert!(!page.contains("<script>"), "{page}");
        assert!(!page.contains("<img"), "{page}");
        assert!(!page.contains("\"'>"), "{page}");
        assert!(
            page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{page}"
        );
        // The one script of a page is the file of the panel, by its address.
        assert_eq!(page.matches("<script").count(), 1, "{page}");
        assert!(page.contains("<script src=\"/assets/panel.js\"></script>"));
        assert!(
            !page.contains(" style=\""),
            "the policy of the page blocks inline styles"
        );
        assert!(!page.contains(" onclick=") && !page.contains("javascript:"));
    }

    #[test]
    fn what_came_from_outside_is_shown_as_text_in_every_screen() {
        let hostile = || HOSTILE.to_owned();
        let facts = Facts {
            address: Some(hostile()),
            when: hostile(),
            hostname: hostile(),
            os: hostile(),
            hermes: hostile(),
        };
        let last_use = LastUse {
            device: Uuid::nil(),
            at: Some(7),
            state: "active",
            when: hostile(),
            address: Some(hostile()),
        };
        for screen in [
            Screen::Name {
                typed: hostile(),
                error: Some("Use só minúsculas."),
            },
            Screen::Block {
                name: hostile(),
                text: Zeroizing::new(hostile()),
                seconds: 900,
                reconnect: true,
                cancelled: true,
                closed_request: true,
            },
            Screen::NoConnector { name: hostile() },
            Screen::Waiting {
                name: hostile(),
                seconds: 5,
                reconnect: false,
            },
            Screen::Code {
                name: hostile(),
                facts: Box::new(facts),
                last_use: Some(Box::new(last_use)),
                error: Some(hostile()),
                seconds: 600,
            },
            Screen::Ended {
                name: hostile(),
                why: Ended::Nothing,
            },
            Screen::Done {
                name: hostile(),
                connected: false,
                reconnect: true,
            },
        ] {
            clean(&shown(&screen));
        }
    }

    #[test]
    fn the_path_marks_the_step_and_only_a_page_that_waits_asks_again() {
        let waiting = shown(&Screen::Waiting {
            name: "ana".to_owned(),
            seconds: 61,
            reconnect: false,
        });
        assert!(waiting.contains("<meta http-equiv=\"refresh\" content=\"3\">"));
        assert!(waiting.contains("<b data-segundos=\"61\">01:01</b>"));
        assert_eq!(waiting.matches("<li class=\"feita\">").count(), 2);
        assert!(
            waiting.contains(
                "<li class=\"atual\" aria-current=\"step\"><span class=\"bola\">3</span>"
            )
        );
        // What is under the guide cannot be reached while it is open.
        assert!(waiting.contains("<div class=\"pagina\" inert>"));

        let code = shown(&Screen::Code {
            name: "ana".to_owned(),
            facts: Box::new(Facts {
                address: None,
                when: "agora".to_owned(),
                hostname: "pc".to_owned(),
                os: "linux".to_owned(),
                hermes: "0.22".to_owned(),
            }),
            last_use: None,
            error: None,
            seconds: 0,
        });
        // A page with a field being typed never reloads by itself.
        assert!(!code.contains("http-equiv"));
        assert!(code.contains("<b data-segundos=\"0\">00:00</b>"));
        assert!(
            !code.contains("name=\"use\""),
            "no question about a use there was not"
        );
        assert!(
            code.contains("action=\"/hermes/approve\"") && code.contains("action=\"/hermes/deny\"")
        );

        let name = shown(&Screen::Name {
            typed: String::new(),
            error: None,
        });
        assert!(name.contains("<h2 id=\"t-guia\">Conectar Hermes</h2>"));
        assert!(!name.contains("class=\"feita\""));
        let done = shown(&Screen::Done {
            name: "ana".to_owned(),
            connected: true,
            reconnect: false,
        });
        assert_eq!(done.matches("<li class=\"feita\">").count(), 3);
        assert_eq!(done.matches("<li class=\"ok\">").count(), 4);
    }

    #[test]
    fn the_block_waits_for_the_mascot_and_the_way_on_waits_for_the_copy() {
        let block = shown(&Screen::Block {
            name: "ana".to_owned(),
            text: Zeroizing::new("the block".to_owned()),
            seconds: 900,
            reconnect: false,
            cancelled: false,
            closed_request: false,
        });
        assert!(block.contains("class=\"modal guia prepara\""));
        assert!(block.contains("<pre id=\"guia-texto\">the block</pre>"));
        assert!(block.contains("class=\"botao principal apos-copiar surge\" id=\"guia-avancar\" href=\"/hermes/wait?name=ana\""));
        assert!(block.contains("<h2 id=\"t-guia\">Conectar ana</h2>"));
        assert!(!block.contains("foi cancelado") && !block.contains("continua funcionando"));
        assert!(
            !block.contains("http-equiv"),
            "the block is never asked for again"
        );
    }
}
