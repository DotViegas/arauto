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
