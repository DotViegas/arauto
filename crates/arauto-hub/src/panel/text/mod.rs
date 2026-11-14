//! The languages of the panel (`docs/decisoes/0016-idioma.md`).
//!
//! English is the language the screens are written in: every sentence of a page is a plain
//! English text in the code, and that text is also the key of its translations. A language
//! that has no translation for a sentence shows it in English.
//!
//! - **A translation is plain text.** It carries no markup: the markup is in the code, and a
//!   translated text is escaped on its way to the page like any other value. A sentence with
//!   a name or a number in it is a template with `{placeholders}`; a test checks that every
//!   translation keeps the placeholders of its key and has no markup.
//! - **The language is chosen by the browser** (`Accept-Language`), unless the person chose
//!   one in the panel: that choice is a cookie with the tag of the language and nothing else.
//! - **What is translated, and what is not:** the connection sentence that goes to the Hermes is
//!   translated (its labels and its two sentences; the values in it are appended by the code).
//!   The block of the command, the instruction page and `/llms.txt` are not, and neither are the
//!   names of commands or the values that come from devices and from the engine.

mod de;
mod es;
mod fr;
mod hi;
mod ja;
mod pt_br;
mod ru;
mod zh_cn;

use std::collections::HashMap;
use std::sync::LazyLock;

/// The name of the cookie that keeps the language a person chose. It holds a tag of
/// [`Lang::ALL`] and is no secret.
pub const COOKIE: &str = "arauto-panel-lang";

/// How many ranges of an `Accept-Language` are read. A browser sends a handful.
const RANGES_MAX: usize = 16;

/// A language the panel speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    /// English: the language the screens are written in.
    #[default]
    En,
    /// Portuguese, of Brazil.
    PtBr,
    /// Spanish.
    Es,
    /// French.
    Fr,
    /// German.
    De,
    /// Russian.
    Ru,
    /// Hindi.
    Hi,
    /// Chinese, simplified.
    ZhCn,
    /// Japanese.
    Ja,
}

impl Lang {
    /// Every language, in the order the README lists them.
    pub const ALL: [Lang; 9] = [
        Lang::En,
        Lang::PtBr,
        Lang::Es,
        Lang::Fr,
        Lang::De,
        Lang::Ru,
        Lang::Hi,
        Lang::ZhCn,
        Lang::Ja,
    ];

    /// The tag of the language, as HTML and `Accept-Language` write it.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::PtBr => "pt-BR",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Ru => "ru",
            Lang::Hi => "hi",
            Lang::ZhCn => "zh-CN",
            Lang::Ja => "ja",
        }
    }

    /// The name of the language, in the language itself.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::PtBr => "Português",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Ru => "Русский",
            Lang::Hi => "हिन्दी",
            Lang::ZhCn => "简体中文",
            Lang::Ja => "日本語",
        }
    }

    /// The language with exactly this tag, whatever its case.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Lang> {
        Lang::ALL
            .into_iter()
            .find(|lang| lang.tag().eq_ignore_ascii_case(tag))
    }

    /// The language a range of `Accept-Language` asks for: by its whole tag, or by its
    /// first part (`pt`, `pt-PT` and `pt-BR` are all the Portuguese the panel has).
    fn of_range(range: &str) -> Option<Lang> {
        if let Some(exact) = Lang::from_tag(range) {
            return Some(exact);
        }
        let primary = range.split('-').next().unwrap_or(range);
        Lang::ALL.into_iter().find(|lang| {
            lang.tag()
                .split('-')
                .next()
                .is_some_and(|own| own.eq_ignore_ascii_case(primary))
        })
    }

    /// The language of a request: the one the person chose in the panel, when the cookie
    /// names one; else the first one the browser prefers that the panel speaks; else English.
    /// Both texts come from outside: what is not a tag of the list chooses nothing.
    #[must_use]
    pub fn negotiate(chosen: Option<&str>, accept_language: Option<&str>) -> Lang {
        if let Some(lang) = chosen.and_then(|tag| Lang::from_tag(tag.trim())) {
            return lang;
        }
        let mut ranges: Vec<(u16, usize, Lang)> = accept_language
            .unwrap_or_default()
            .split(',')
            .take(RANGES_MAX)
            .enumerate()
            .filter_map(|(position, entry)| {
                let mut parts = entry.split(';');
                let lang = Lang::of_range(parts.next()?.trim())?;
                // The weight, in thousandths: absent is 1; one that is not a number is 0.
                let weight = parts
                    .find_map(|part| part.trim().strip_prefix("q="))
                    .map_or(1_000, |q| {
                        q.trim()
                            .parse::<f32>()
                            .ok()
                            .filter(|q| (0.0..=1.0).contains(q))
                            .map_or(0, |q| {
                                // Between 0 and 1000 by the filter above.
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let thousandths = (q * 1_000.0).round() as u16;
                                thousandths
                            })
                    });
                (weight > 0).then_some((weight, position, lang))
            })
            .collect();
        ranges.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        ranges.first().map_or(Lang::En, |(_, _, lang)| *lang)
    }

    /// The translations of this language, as `(English, translation)`. English has none: it
    /// is the key.
    const fn pairs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::PtBr => pt_br::PAIRS,
            Lang::Es => es::PAIRS,
            Lang::Fr => fr::PAIRS,
            Lang::De => de::PAIRS,
            Lang::Ru => ru::PAIRS,
            Lang::Hi => hi::PAIRS,
            Lang::ZhCn => zh_cn::PAIRS,
            Lang::Ja => ja::PAIRS,
        }
    }
}

/// Marks a text of the code as a sentence of the screens: one that every language
/// translates. It changes nothing; the test that reads the code finds the sentences by it.
#[must_use]
pub const fn k(text: &'static str) -> &'static str {
    text
}

static CATALOGS: LazyLock<HashMap<Lang, HashMap<&'static str, &'static str>>> =
    LazyLock::new(|| {
        Lang::ALL
            .into_iter()
            .map(|lang| (lang, lang.pairs().iter().copied().collect()))
            .collect()
    });

/// The sentence `key` in `lang`: its translation, or the English text itself.
#[must_use]
pub fn tr(lang: Lang, key: &'static str) -> &'static str {
    CATALOGS
        .get(&lang)
        .and_then(|catalog| catalog.get(key))
        .copied()
        // An empty translation is no translation.
        .filter(|found| !found.is_empty())
        .unwrap_or(key)
}

/// The pieces of a template: its plain parts and the names of its `{placeholders}`, in
/// order. A brace that opens no placeholder is plain text.
pub fn pieces(template: &str) -> Vec<Piece<'_>> {
    let mut found = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let name_end = after.find('}');
        let name = name_end.map(|end| &after[..end]);
        match name {
            Some(name)
                if !name.is_empty()
                    && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') =>
            {
                if open > 0 {
                    found.push(Piece::Plain(&rest[..open]));
                }
                found.push(Piece::Placeholder(name));
                rest = &after[name.len() + 1..];
            }
            _ => {
                found.push(Piece::Plain(&rest[..=open]));
                rest = after;
            }
        }
    }
    if !rest.is_empty() {
        found.push(Piece::Plain(rest));
    }
    found
}

/// One piece of a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'t> {
    /// Text as it is.
    Plain(&'t str),
    /// The name between braces.
    Placeholder(&'t str),
}

/// The template `key` in `lang`, with each `{name}` replaced by its value, as plain text:
/// for a sentence that is kept as a value and escaped when it is written.
#[must_use]
pub fn fill(lang: Lang, key: &'static str, values: &[(&str, &str)]) -> String {
    let mut out = String::new();
    fill_into(&mut out, lang, key, values);
    out
}

/// [`fill`], written at the end of `out`. A caller whose values are secret gives a buffer that
/// is wiped when dropped and already has the room, so that no copy is left behind.
pub fn fill_into(out: &mut String, lang: Lang, key: &'static str, values: &[(&str, &str)]) {
    for piece in pieces(tr(lang, key)) {
        match piece {
            Piece::Plain(text) => out.push_str(text),
            Piece::Placeholder(name) => {
                if let Some((_, value)) = values.iter().find(|(found, _)| *found == name) {
                    out.push_str(value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_is_the_chosen_one_then_the_first_the_browser_prefers() {
        let pick = |chosen, accept| Lang::negotiate(chosen, accept);
        assert_eq!(pick(None, None), Lang::En);
        assert_eq!(pick(None, Some("")), Lang::En);
        assert_eq!(pick(None, Some("pt-BR,pt;q=0.9,en;q=0.8")), Lang::PtBr);
        assert_eq!(pick(None, Some("pt-PT")), Lang::PtBr);
        assert_eq!(pick(None, Some("zh-Hans-CN, zh;q=0.9")), Lang::ZhCn);
        assert_eq!(pick(None, Some("JA")), Lang::Ja);
        // The weights decide, and the order breaks a tie.
        assert_eq!(pick(None, Some("fr;q=0.5, de;q=0.9")), Lang::De);
        assert_eq!(pick(None, Some("es, ru")), Lang::Es);
        assert_eq!(pick(None, Some("en;q=0, hi;q=0.1")), Lang::Hi);
        // A language the panel does not speak is passed over.
        assert_eq!(pick(None, Some("it, nl;q=0.9, fr;q=0.2")), Lang::Fr);
        assert_eq!(pick(None, Some("it, *")), Lang::En);
        // The choice made in the panel comes first, when it is a language of the list.
        assert_eq!(pick(Some("de"), Some("ja")), Lang::De);
        assert_eq!(pick(Some(" ZH-cn "), None), Lang::ZhCn);
        assert_eq!(pick(Some("xx"), Some("ru")), Lang::Ru);
        assert_eq!(pick(Some("<script>"), None), Lang::En);
        // What is not a weight chooses nothing, and a long header is cut.
        assert_eq!(pick(None, Some("fr;q=abc, es;q=2, de;q=-1")), Lang::En);
        let long = "it,".repeat(40) + "ja";
        assert_eq!(pick(None, Some(&long)), Lang::En);
    }

    #[test]
    fn the_tags_are_distinct_and_each_language_names_itself() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_tag(lang.tag()), Some(lang));
            assert!(!lang.name().is_empty());
        }
        let tags: std::collections::BTreeSet<&str> = Lang::ALL.into_iter().map(Lang::tag).collect();
        assert_eq!(tags.len(), Lang::ALL.len());
        assert_eq!(Lang::from_tag("pt"), None, "a choice is a whole tag");
    }

    #[test]
    fn a_template_is_cut_at_its_placeholders_and_filled_by_name() {
        assert_eq!(
            pieces("Attempt {n} of {max}."),
            [
                Piece::Plain("Attempt "),
                Piece::Placeholder("n"),
                Piece::Plain(" of "),
                Piece::Placeholder("max"),
                Piece::Plain(".")
            ]
        );
        // A brace that opens no placeholder is text.
        assert_eq!(pieces("a { b"), [Piece::Plain("a {"), Piece::Plain(" b")]);
        assert_eq!(pieces("{}x"), [Piece::Plain("{"), Piece::Plain("}x")]);
        assert_eq!(pieces("{A}"), [Piece::Plain("{"), Piece::Plain("A}")]);
        assert_eq!(
            fill(
                Lang::En,
                k("Attempt {n} of {max}."),
                &[("max", "5"), ("n", "2")]
            ),
            "Attempt 2 of 5."
        );
        // A value is not read again: a placeholder inside it stays as it came.
        assert_eq!(
            fill(Lang::En, k("Hello, {name}."), &[("name", "{name}{n}")]),
            "Hello, {name}{n}."
        );
        // A sentence with no translation is shown in English.
        assert_eq!(
            tr(Lang::Ja, "a sentence nobody translated"),
            "a sentence nobody translated"
        );
    }

    /// The code of the panel that writes sentences, as text: this test reads it.
    const SOURCES: [(&str, &str); 9] = [
        ("view.rs", include_str!("../view.rs")),
        ("guide.rs", include_str!("../guide.rs")),
        ("editor.rs", include_str!("../editor.rs")),
        ("data.rs", include_str!("../data.rs")),
        ("connect.rs", include_str!("../connect.rs")),
        ("mod.rs", include_str!("../mod.rs")),
        ("check.rs", include_str!("../../check.rs")),
        ("installation.rs", include_str!("../installation.rs")),
        ("block.rs", include_str!("../block.rs")),
    ];

    /// Every sentence the code marks with `k("...")`, outside the tests of each file.
    fn sentences() -> Vec<String> {
        let mut found = Vec::new();
        for (_, source) in SOURCES {
            let code = source.split("#[cfg(test)]").next().unwrap();
            let bytes = code.as_bytes();
            let mut at = 0;
            while let Some(offset) = code[at..].find("k(") {
                let start = at + offset;
                at = start + 2;
                // `k(` alone: not the end of a longer name.
                if start > 0
                    && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')
                {
                    continue;
                }
                let rest = code[at..].trim_start();
                let Some(literal) = rest.strip_prefix('"') else {
                    continue;
                };
                let mut sentence = String::new();
                let mut characters = literal.chars();
                loop {
                    match characters.next().expect("a string of the code ends") {
                        '"' => break,
                        '\\' => sentence.push(characters.next().unwrap()),
                        other => sentence.push(other),
                    }
                }
                if !found.contains(&sentence) {
                    found.push(sentence);
                }
            }
        }
        found
    }

    fn placeholders(text: &str) -> Vec<&str> {
        let mut names: Vec<&str> = pieces(text)
            .into_iter()
            .filter_map(|piece| match piece {
                Piece::Placeholder(name) => Some(name),
                Piece::Plain(_) => None,
            })
            .collect();
        names.sort_unstable();
        names
    }

    #[test]
    fn the_code_marks_its_sentences_and_no_other_text_reaches_a_page_as_one() {
        let sentences = sentences();
        assert!(
            sentences.len() > 200,
            "the code was read: {}",
            sentences.len()
        );
        assert!(sentences.iter().any(|s| s == "Welcome to your Arauto hub"));
        assert!(
            sentences.iter().any(|s| s.contains("\"Connect Hermes\"")),
            "escapes are read"
        );
        // A sentence handed to the page without the mark would never be translated: every
        // text given to `t`, `fill`, `bracketed`, `tr` and `text::fill` is a marked one, or a
        // name that holds one.
        for (name, source) in SOURCES {
            let code = source.split("#[cfg(test)]").next().unwrap();
            for call in [
                ".t(\"",
                ".fill(\"",
                ".bracketed(\"",
                "tr(lang, \"",
                "fill(lang, \"",
            ] {
                assert!(
                    !code.contains(call),
                    "{name} writes a sentence without k(): {call}"
                );
            }
        }
    }

    #[test]
    fn every_language_translates_every_sentence_and_keeps_what_is_not_text() {
        let sentences = sentences();
        for lang in Lang::ALL {
            if lang == Lang::En {
                assert!(lang.pairs().is_empty(), "English is the key");
                continue;
            }
            let tag = lang.tag();
            let pairs = lang.pairs();
            let keys: Vec<&str> = pairs.iter().map(|(key, _)| *key).collect();
            for sentence in &sentences {
                assert!(
                    keys.contains(&sentence.as_str()),
                    "{tag} does not translate: {sentence}"
                );
            }
            for (key, translation) in pairs {
                assert!(
                    sentences.iter().any(|s| s == key),
                    "{tag} translates what no page says: {key}"
                );
                assert_eq!(
                    keys.iter().filter(|other| *other == key).count(),
                    1,
                    "{tag} translates twice: {key}"
                );
                assert!(!translation.trim().is_empty(), "{tag} left empty: {key}");
                // A translation is text: it never opens markup, and the page escapes it anyway.
                assert!(
                    !translation.contains('<') && !translation.contains('>'),
                    "{tag} has markup in: {key}"
                );
                assert!(
                    translation.chars().all(|c| !c.is_control()),
                    "{tag} has a control character in: {key}"
                );
                assert_eq!(
                    placeholders(key),
                    placeholders(translation),
                    "{tag} changed the placeholders of: {key}"
                );
            }
            // The word the brand colors keeps its brackets.
            let title = tr(lang, "Your Hermes and their [numbers]");
            let brackets = title.find('[').zip(title.find(']'));
            assert!(
                brackets.is_some_and(|(open, close)| open + 1 < close),
                "{tag}: {title}"
            );
            // The four letters of the matrix are four, short and different.
            let letters: Vec<&str> = ["R", "S", "M", "W"]
                .into_iter()
                .map(|key| tr(lang, key))
                .collect();
            for letter in &letters {
                assert!((1..=2).contains(&letter.chars().count()), "{tag}: {letter}");
            }
            let distinct: std::collections::BTreeSet<&&str> = letters.iter().collect();
            assert_eq!(distinct.len(), 4, "{tag}: {letters:?}");
            // Two answers offered side by side never read the same.
            for (one, other) in [
                ("Revoke", "Cancel"),
                ("Save", "Cancel"),
                ("Issue invite", "Cancel"),
                ("Approve", "Deny"),
                ("I recognize it", "I do not recognize it"),
                ("Connect", "Reconnect"),
                ("yes", "no"),
                (
                    "Every chat, including the ones that come later",
                    "Only chosen chats",
                ),
            ] {
                assert_ne!(tr(lang, one), tr(lang, other), "{tag}: {one} and {other}");
            }
        }
    }
}
