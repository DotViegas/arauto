//! HTML written by the hub, with every value escaped by construction.
//!
//! "Every text that comes from a device or from WhatsApp is hostile: validate on the way in,
//! escape on the way out" (rule 10 of the project). The way out is this type. It takes two
//! kinds of text and nothing else:
//!
//! - [`Html::raw`]: markup that is written in the program. It asks for a `&'static str`, so a
//!   name of a machine, an address or an alias read from the database cannot be handed to it;
//! - [`Html::text`]: a value. It is escaped, whatever it is.
//!
//! Every attribute of the pages is written between double quotes, in the `raw` part: an
//! escaped value is safe inside an element and inside such an attribute.
//!
//! A sentence of the screens is a third kind, and it is a value too: [`Html::t`] and
//! [`Html::fill`] look it up in the language of the page ([`super::text`]) and escape what
//! they find. A translation never reaches a page as markup.

use super::text::{Lang, Piece, pieces, tr};
use crate::admin::qr::QrCode;

/// What goes in the place of a `{placeholder}` of a sentence.
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    /// A value, escaped.
    Text(&'a str),
    /// A value, escaped, in bold.
    Strong(&'a str),
    /// A command or a name of the program, as code.
    Code(&'static str),
    /// A number.
    Number(usize),
    /// A time left, in seconds, as `MM:SS`: the script of the panel counts it down.
    Clock(i64),
    /// An absolute time, in milliseconds, written in UTC inside a `<time>` element: the script
    /// of the panel shows it in the zone of the browser.
    Time(i64),
    /// A relative text, escaped, and the absolute time after it, in parentheses.
    AgoTime(&'a str, i64),
}

/// A page, or a piece of one, being written.
#[derive(Debug, Default)]
pub struct Html {
    out: String,
    lang: Lang,
}

impl Html {
    /// An empty page, in a language.
    #[must_use]
    pub fn new(lang: Lang) -> Self {
        Self {
            out: String::new(),
            lang,
        }
    }

    /// The language of the page.
    #[must_use]
    pub const fn lang(&self) -> Lang {
        self.lang
    }

    /// A sentence of the screens, in the language of the page, escaped.
    pub fn t(&mut self, key: &'static str) -> &mut Self {
        escape_into(&mut self.out, tr(self.lang, key));
        self
    }

    /// A sentence of the screens with values in it: the template in the language of the
    /// page, escaped, with each `{name}` replaced by its argument. A placeholder with no
    /// argument writes nothing.
    pub fn fill(&mut self, key: &'static str, args: &[(&str, Arg<'_>)]) -> &mut Self {
        for piece in pieces(tr(self.lang, key)) {
            match piece {
                Piece::Plain(text) => escape_into(&mut self.out, text),
                Piece::Placeholder(name) => {
                    let Some((_, arg)) = args.iter().find(|(found, _)| *found == name) else {
                        continue;
                    };
                    match *arg {
                        Arg::Text(value) => escape_into(&mut self.out, value),
                        Arg::Strong(value) => {
                            self.out.push_str("<b>");
                            escape_into(&mut self.out, value);
                            self.out.push_str("</b>");
                        }
                        Arg::Code(value) => {
                            self.out.push_str("<code>");
                            escape_into(&mut self.out, value);
                            self.out.push_str("</code>");
                        }
                        Arg::Number(value) => self.out.push_str(&value.to_string()),
                        Arg::Clock(seconds) => self.clock(seconds),
                        Arg::Time(at) => self.time(at),
                        Arg::AgoTime(ago, at) => {
                            escape_into(&mut self.out, ago);
                            self.out.push_str(" (");
                            self.time(at);
                            self.out.push(')');
                        }
                    }
                }
            }
        }
        self
    }

    /// A sentence of the screens whose part between square brackets is the one the brand
    /// colors: the brackets stay, in bold with what is inside them.
    pub fn bracketed(&mut self, key: &'static str) -> &mut Self {
        let text = tr(self.lang, key);
        let cut = text
            .find('[')
            .zip(text.find(']'))
            .filter(|(open, close)| open < close);
        match cut {
            Some((open, close)) => {
                escape_into(&mut self.out, &text[..open]);
                self.out.push_str("<b>");
                escape_into(&mut self.out, &text[open..=close]);
                self.out.push_str("</b>");
                escape_into(&mut self.out, &text[close + 1..]);
            }
            None => escape_into(&mut self.out, text),
        }
        self
    }

    /// `<time datetime="2026-11-14T12:34:56Z">2026-11-14 12:34 UTC</time>`: both parts come from
    /// numbers, so nothing from outside reaches the markup.
    fn time(&mut self, at_ms: i64) {
        let full = crate::admin::render::time(at_ms).replacen(' ', "T", 1);
        self.out.push_str(&format!(
            "<time datetime=\"{full}\">{} UTC</time>",
            crate::join_page::utc_minute(at_ms)
        ));
    }

    fn clock(&mut self, seconds: i64) {
        let seconds = usize::try_from(seconds.max(0)).unwrap_or(0);
        let (minutes, rest) = (seconds / 60, seconds % 60);
        self.out.push_str(&format!(
            "<b data-seconds=\"{seconds}\">{minutes:02}:{rest:02}</b>"
        ));
    }

    /// Markup written in the program.
    pub fn raw(&mut self, markup: &'static str) -> &mut Self {
        self.out.push_str(markup);
        self
    }

    /// A value, escaped: text of an element, or the content of a double-quoted attribute.
    pub fn text(&mut self, value: &str) -> &mut Self {
        escape_into(&mut self.out, value);
        self
    }

    /// A QR code, drawn as an inline SVG: one segment per horizontal run of dark modules, on a
    /// white square with a quiet zone of four modules. Only digits and the letters `M`, `h`,
    /// `v`, `z` and `-` are written for the modules: no value of the engine reaches the markup
    /// as text, and the drawing has no `style` and no script. `label` is a sentence of the
    /// screens, said to whoever cannot see it.
    pub fn qr(&mut self, code: &QrCode, label: &'static str) -> &mut Self {
        let size = code.size();
        let side = size + 8;
        self.out.push_str(&format!(
            "<svg class=\"qr\" viewBox=\"0 0 {side} {side}\" role=\"img\" aria-label=\""
        ));
        escape_into(&mut self.out, tr(self.lang, label));
        self.out.push_str(&format!(
            "\" shape-rendering=\"crispEdges\"><rect width=\"{side}\" height=\"{side}\" fill=\"#fff\"/><path fill=\"#000\" d=\""
        ));
        for y in 0..size {
            let mut x = 0;
            while x < size {
                if code.dark(x, y) {
                    let start = x;
                    while x < size && code.dark(x, y) {
                        x += 1;
                    }
                    let run = x - start;
                    self.out
                        .push_str(&format!("M{} {}h{run}v1h-{run}z", start + 4, y + 4));
                } else {
                    x += 1;
                }
            }
        }
        self.out.push_str("\"/></svg>");
        self
    }

    /// A number.
    pub fn number(&mut self, value: usize) -> &mut Self {
        self.out.push_str(&value.to_string());
        self
    }

    /// The page, as text.
    #[must_use]
    pub fn finish(self) -> String {
        self.out
    }
}

/// Writes `value` so that a browser reads it as text and never as markup: the five
/// characters that open markup, an entity or the end of an attribute become entities, and a
/// control character becomes the replacement character.
pub fn escape_into(out: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\n' | '\t' => out.push(character),
            other if other.is_control() => out.push('\u{FFFD}'),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escaped(value: &str) -> String {
        let mut html = Html::new(Lang::En);
        html.text(value);
        html.finish()
    }

    #[test]
    fn a_time_is_written_in_utc_with_its_instant() {
        let time = "<time datetime=\"2026-11-14T12:34:56Z\">2026-11-14 12:34 UTC</time>";
        let mut html = Html::new(Lang::En);
        html.fill("{at}", &[("at", Arg::Time(1_794_659_696_000))]);
        assert_eq!(html.finish(), time);
        let mut html = Html::new(Lang::En);
        html.fill(
            "{at}",
            &[("at", Arg::AgoTime("<3 h> ago", 1_794_659_696_000))],
        );
        let out = html.finish();
        assert!(out.starts_with("&lt;3 h&gt; ago ("), "{out}");
        assert!(out.ends_with(&format!("{time})")), "{out}");
    }

    #[test]
    fn a_value_never_becomes_markup() {
        for hostile in [
            "<script>alert(1)</script>",
            "\"><img src=x onerror=alert(1)>",
            "' onmouseover='alert(1)",
            "</td></tr></table><h1>x",
            "&lt;already&gt;",
            "javascript:alert(1)",
            "a\u{0}b\u{7}c\u{1b}[31m",
        ] {
            let out = escaped(hostile);
            assert!(!out.contains('<'), "{out}");
            assert!(!out.contains('>'), "{out}");
            assert!(!out.contains('"'), "{out}");
            assert!(!out.contains('\''), "{out}");
            assert!(!out.chars().any(|c| c.is_control()), "{out:?}");
            // An ampersand only ever opens one of the entities written here.
            for (at, _) in out.match_indices('&') {
                let rest = &out[at..];
                assert!(
                    ["&amp;", "&lt;", "&gt;", "&quot;", "&#39;"]
                        .iter()
                        .any(|entity| rest.starts_with(entity)),
                    "{out}"
                );
            }
        }
    }

    #[test]
    fn what_is_escaped_reads_back_as_what_was_given() {
        assert_eq!(
            escaped("a < b && \"c\" > 'd'"),
            "a &lt; b &amp;&amp; &quot;c&quot; &gt; &#39;d&#39;"
        );
        assert_eq!(
            escaped("portaria, bloco B · ção"),
            "portaria, bloco B · ção"
        );
        assert_eq!(escaped("two\nlines\tand a tab"), "two\nlines\tand a tab");
    }

    #[test]
    fn the_qr_markup_has_only_numbers_and_path_letters() {
        let code = QrCode::encode(b"\"><script>alert(1)</script>").unwrap();
        let mut html = Html::new(Lang::En);
        html.qr(
            &code,
            crate::panel::text::k("QR code to scan with the phone"),
        );
        let out = html.finish();
        assert!(!out.contains("style=") && !out.contains("<script"), "{out}");
        let d = out.split("d=\"").nth(1).unwrap().split('"').next().unwrap();
        assert!(
            d.bytes()
                .all(|b| b.is_ascii_digit() || b" Mhvz-".contains(&b)),
            "{d}"
        );
        assert!(out.starts_with("<svg class=\"qr\" viewBox=\"0 0 "));
        // Every dark module is in a segment: the number of unit squares equals the dark ones.
        let squares: usize = d
            .split('M')
            .filter(|segment| !segment.is_empty())
            .map(|segment| {
                segment
                    .split('h')
                    .nth(1)
                    .unwrap()
                    .split('v')
                    .next()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
            })
            .sum();
        let dark = (0..code.size())
            .flat_map(|y| (0..code.size()).map(move |x| (x, y)))
            .filter(|(x, y)| code.dark(*x, *y))
            .count();
        assert_eq!(squares, dark);
    }

    #[test]
    fn markup_and_values_are_kept_apart() {
        let mut html = Html::new(Lang::En);
        html.raw("<td class=\"name\">")
            .text("<b>ana</b>")
            .raw("</td><td>")
            .number(3)
            .raw("</td>");
        assert_eq!(
            html.finish(),
            "<td class=\"name\">&lt;b&gt;ana&lt;/b&gt;</td><td>3</td>"
        );
    }
}
