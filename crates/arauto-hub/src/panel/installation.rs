//! The window that checks the installation (`GET /installation`): step 3 of the first access,
//! and a window anyone in the panel may open later from the footer. It only reads: the rows
//! come from [`crate::check::rows`], the same function `arauto-hub admin check` prints.
//!
//! Like [`super::view`], this module only writes: every value goes through [`Html::text`].

use super::html::{Arg, Html};
use super::text::k;
use super::view::{self, Frame, HermesPage, Over, Tone};
use crate::check::{Detail, Row, Status, Value};

/// How a state reads at a glance: green ok, amber attention, red failed, and a neutral
/// outline for what the admin looks at and for what comes later.
const fn tone(status: Status) -> Tone {
    match status {
        Status::Ok => Tone::Ok,
        Status::Attention => Tone::Wait,
        Status::Failed => Tone::Bad,
        Status::Check | Status::Later => Tone::Idle,
    }
}

/// The sentence of a row, with its command in code and its address escaped.
fn sentence(html: &mut Html, detail: &Detail) {
    let Some((key, values)) = detail.sentence() else {
        return;
    };
    let args: Vec<(&str, Arg<'_>)> = values
        .iter()
        .map(|(name, value)| {
            (
                *name,
                match value {
                    Value::Text(text) => Arg::Text(text),
                    Value::Code(code) => Arg::Code(code),
                },
            )
        })
        .collect();
    html.fill(key, &args);
}

fn row(html: &mut Html, row: &Row) {
    html.raw("<tr><th scope=\"row\">")
        .t(row.subject.label())
        .raw("</th><td>");
    view::state(html, tone(row.status), row.status.label());
    html.raw("</td><td class=\"detail\">");
    match &row.detail {
        Detail::PanelWorks { origin } => {
            html.raw("<code>").text(origin).raw("</code><br>");
        }
        Detail::ConnectorsOpen { origin } => {
            // The address of the connectors opens in a new tab, so the window stays where it is.
            html.raw("<a class=\"link\" href=\"")
                .text(origin)
                .raw("\" target=\"_blank\" rel=\"noopener noreferrer\">")
                .text(origin)
                .raw("</a><br>");
        }
        _ => {}
    }
    sentence(html, &row.detail);
    html.raw("</td></tr>");
}

/// The page of the Hermes with the window over it. `welcome` is right after the first passkey:
/// the window then is step 3, with the path of the first access above it.
#[must_use]
pub fn page(frame: &Frame, hermes: &HermesPage, rows: &[Row], welcome: bool) -> String {
    let mut html = Html::new(frame.lang);
    view::hermes_into(&mut html, frame, hermes, Over::Guide { refresh: None });
    html.raw("<dialog open class=\"modal check\" id=\"check\" aria-labelledby=\"check-title\"><div class=\"modal-head\"><h2 id=\"check-title\">");
    if welcome {
        html.t(k("Step 3 · Let's check the installation"));
    } else {
        html.t(k("Check the installation"));
    }
    html.raw("</h2><a class=\"close\" href=\"/\" aria-label=\"")
        .t(k("Close"))
        .raw("\"><svg viewBox=\"0 0 14 14\" aria-hidden=\"true\"><path d=\"M2 2l10 10M12 2L2 12\"/></svg></a></div>");
    if welcome {
        view::stepper(&mut html, 2);
    }
    html.raw("<div class=\"modal-body\">");
    if welcome {
        html.raw("<h3>")
            .t(k("All set: your passkey was created!"))
            .raw("</h3>");
    }
    html.raw("<p>")
        .t(k("The hub checked a few things by itself. Nothing here changes anything: it is only so you know how it turned out."))
        .raw("</p><div class=\"frame\"><table class=\"checks\"><tbody>");
    for one in rows {
        row(&mut html, one);
    }
    html.raw("</tbody></table></div><p class=\"hint\">")
        .fill(
            k("The same by command: {command}"),
            &[("command", Arg::Code("arauto-hub admin check"))],
        )
        .raw("</p></div><div class=\"modal-foot\">");
    if welcome {
        html.raw("<a class=\"button primary\" href=\"/hermes/connect\">")
            .t(k("Connect my first Hermes"))
            .raw("</a><a class=\"button\" href=\"/\">")
            .t(k("Do it later"))
            .raw("</a>");
    } else {
        html.raw("<a class=\"button\" href=\"/\">")
            .t(k("Close"))
            .raw("</a>");
    }
    html.raw("</div></dialog>").raw(view::TAIL);
    html.finish()
}
