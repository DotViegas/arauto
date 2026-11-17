//! The org chart of the Hermes, stage A (`docs/decisoes/0043-organograma-e-niveis.md`): the
//! pure rules. Levels, the place of a Hermes on the map, a role text for the admin's own
//! reading, and the draft the admin publishes.
//!
//! Nothing here is read by the channel, a notice or a tool. [`kind`] says what an arrow is
//! called from the levels of its ends, for the admin to read; it gives no permission.
//!
//! **Proposal, not built beyond what the tests prove** (`docs/06-modelo-de-dados.md`, section
//! 11). In stage A no Hermes knows its level.

use std::fmt;

use arauto_protocol::names::{HermesName, RESERVED_NAMES, is_deceptive};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The most levels a chart has.
pub const LEVELS_MAX: usize = 12;

/// The most characters of a role text.
pub const ROLE_MAX_CHARS: usize = 280;

/// The largest left edge of a card on the map.
pub const MAP_X_MAX: u16 = 4_000;

/// The largest body `POST /org/publish` reads, and the largest draft file the command reads.
pub const PUBLISH_FORM_MAX: usize = 1_048_576;

/// The names of a level that is not stored yet: `new-1` to `new-12`.
const NEW_PREFIX: &str = "new-";

/// What is wrong with a draft or one of its parts. Unit variants only: no error can carry a
/// value of the draft, so none can reach a log, a page or the audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OrgInvalid {
    /// It is not the one shape a draft has.
    #[error("The draft could not be read.")]
    Unreadable,
    /// There are no levels, or more than the most.
    #[error("The org chart has from 1 to 12 levels.")]
    LevelCount,
    /// A level name breaks the format or is reserved.
    #[error(
        "A level name has 1 to 32 lowercase letters, digits, hyphens or underscores, starts with a letter or a digit, and is not a reserved word."
    )]
    LevelName,
    /// Two levels have the same name.
    #[error("Two levels have the same name.")]
    LevelNameTwice,
    /// A role breaks its rule.
    #[error(
        "A role has at most 280 characters, in one line, without control or direction characters."
    )]
    Role,
    /// A place on the map is out of range.
    #[error("A place on the map is out of range.")]
    Place,
    /// The draft names a Hermes or a level that does not exist, or leaves a Hermes out.
    #[error("The draft names a Hermes or a level that does not exist, or leaves a Hermes out.")]
    Unknown,
    /// A link is not between two different Hermes, once per direction.
    #[error("A link joins two different Hermes, once in each direction.")]
    Link,
    /// The draft is larger than a publish takes.
    #[error("The draft is too large to publish.")]
    TooLarge,
    /// A follow mark is on a link that does not go from a higher level to a lower one, or that
    /// does not wake.
    #[error("A follow mark goes only from a higher level to a lower one, on a link that wakes.")]
    Follow,
}

/// The name of a level: the format and the reserved words of a Hermes name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LevelName(String);

impl LevelName {
    /// Checks the format and the reserved words.
    ///
    /// # Errors
    /// [`OrgInvalid::LevelName`].
    pub fn parse(text: &str) -> Result<Self, OrgInvalid> {
        HermesName::parse(text)
            .map(|name| Self(name.as_str().to_owned()))
            .map_err(|_| OrgInvalid::LevelName)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A role: one line of at most 280 characters for the admin's own reading.
#[derive(Clone, PartialEq, Eq)]
pub struct RoleText(String);

impl fmt::Debug for RoleText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only the size: a role is text written by the admin and stays out of logs.
        write!(f, "RoleText({} chars)", self.0.chars().count())
    }
}

impl RoleText {
    /// Trims the two ends. Empty gives `None`.
    ///
    /// # Errors
    /// [`OrgInvalid::Role`] for more than 280 characters, a control character (a line feed
    /// included), a direction or tag character, or a line or paragraph separator.
    pub fn parse(text: &str) -> Result<Option<Self>, OrgInvalid> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        // 280 characters never take more than 1,120 bytes: the long texts stop here.
        if text.len() > ROLE_MAX_CHARS * 4 || text.chars().count() > ROLE_MAX_CHARS {
            return Err(OrgInvalid::Role);
        }
        let allowed =
            |c: char| !c.is_control() && !is_deceptive(c) && !matches!(c, '\u{2028}' | '\u{2029}');
        if !text.chars().all(allowed) {
            return Err(OrgInvalid::Role);
        }
        Ok(Some(Self(text.to_owned())))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The left edge of a card on the map, in pixels: `0..=4000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MapX(u16);

impl MapX {
    /// Checks the range.
    ///
    /// # Errors
    /// [`OrgInvalid::Place`].
    pub fn new(value: i64) -> Result<Self, OrgInvalid> {
        u16::try_from(value)
            .ok()
            .filter(|value| *value <= MAP_X_MAX)
            .map(Self)
            .ok_or(OrgInvalid::Place)
    }

    /// The pixels.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// The digest that says which chart a draft started from.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChartVersion([u8; 32]);

impl fmt::Debug for ChartVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChartVersion({})", self.to_hex())
    }
}

impl ChartVersion {
    /// The 64 lowercase hexadecimal digits.
    #[must_use]
    pub fn to_hex(&self) -> String {
        data_encoding::HEXLOWER.encode(&self.0)
    }

    /// Reads exactly 64 lowercase hexadecimal digits.
    #[must_use]
    pub fn parse_hex(text: &str) -> Option<Self> {
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let bytes = data_encoding::HEXLOWER.decode(text.as_bytes()).ok()?;
        <[u8; 32]>::try_from(bytes).ok().map(Self)
    }
}

/// What an arrow is called, from the levels of its ends. For the admin to read: nothing in
/// the hub decides anything by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// From a higher level to a lower one.
    Instruction,
    /// From a lower level to a higher one.
    Request,
    /// Between two Hermes of the same level.
    Note,
}

/// The kind of the arrow from a Hermes at `from` to one at `to`. A smaller position is higher.
/// `None` when an end is not placed yet.
#[must_use]
pub fn kind(from_position: Option<usize>, to_position: Option<usize>) -> Option<Kind> {
    let (from, to) = (from_position?, to_position?);
    Some(match from.cmp(&to) {
        std::cmp::Ordering::Less => Kind::Instruction,
        std::cmp::Ordering::Greater => Kind::Request,
        std::cmp::Ordering::Equal => Kind::Note,
    })
}

// ---------------------------------------------------------------------------- the draft

/// How a draft names a level: one the database has, or one it will draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelKey {
    /// A level of the database.
    Stored(Uuid),
    /// A new level: `new-1` to `new-12`.
    New(u8),
}

impl LevelKey {
    fn parse(text: &str) -> Option<Self> {
        if let Some(rest) = text.strip_prefix(NEW_PREFIX) {
            let number: u8 = rest.parse().ok()?;
            // No leading zeros and no sign: the key has one spelling.
            if rest != number.to_string() || !(1..=12).contains(&number) {
                return None;
            }
            return Some(Self::New(number));
        }
        let id = Uuid::try_parse(text).ok()?;
        (text == id.as_hyphenated().to_string()).then_some(Self::Stored(id))
    }

    fn text(self) -> String {
        match self {
            Self::Stored(id) => id.as_hyphenated().to_string(),
            Self::New(number) => format!("{NEW_PREFIX}{number}"),
        }
    }
}

/// A level of the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantedLevel {
    /// How the draft names it.
    pub key: LevelKey,
    /// Its name.
    pub name: LevelName,
}

/// A Hermes of the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantedMember {
    /// Its name.
    pub name: HermesName,
    /// Its level.
    pub level: LevelKey,
    /// Its place.
    pub x: MapX,
    /// Its role.
    pub role: Option<RoleText>,
}

/// A link of the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantedLink {
    /// Who may send.
    pub from: HermesName,
    /// Who receives.
    pub to: HermesName,
    /// Whether a message of `from` wakes `to`.
    pub wake: bool,
    /// Whether `to` follows the instructions of `from` (decision 0050).
    pub follow: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevel {
    key: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMember {
    name: String,
    level: String,
    x: serde_json::Value,
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLink {
    from: String,
    to: String,
    wake: bool,
    #[serde(default)]
    follow: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDraft {
    levels: Vec<RawLevel>,
    hermes: Vec<RawMember>,
    links: Vec<RawLink>,
}

/// The wanted state of the chart, parsed: what Publish writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The levels, top first. The index is the position.
    pub levels: Vec<WantedLevel>,
    /// Every live Hermes, once.
    pub hermes: Vec<WantedMember>,
    /// The links.
    pub links: Vec<WantedLink>,
}

impl Draft {
    /// Reads a draft in its one shape and checks every rule that needs no database.
    ///
    /// # Errors
    /// An [`OrgInvalid`]. The error of the JSON reader is dropped, never formatted: it can
    /// quote the value it refused.
    pub fn parse(text: &str) -> Result<Self, OrgInvalid> {
        let raw: RawDraft = serde_json::from_str(text).map_err(|_| OrgInvalid::Unreadable)?;
        if !(1..=LEVELS_MAX).contains(&raw.levels.len()) {
            return Err(OrgInvalid::LevelCount);
        }
        let mut levels = Vec::with_capacity(raw.levels.len());
        for level in &raw.levels {
            let name = LevelName::parse(&level.name)?;
            if levels.iter().any(|seen: &WantedLevel| seen.name == name) {
                return Err(OrgInvalid::LevelNameTwice);
            }
            let key = LevelKey::parse(&level.key).ok_or(OrgInvalid::Unreadable)?;
            if levels.iter().any(|seen: &WantedLevel| seen.key == key) {
                return Err(OrgInvalid::Unreadable);
            }
            levels.push(WantedLevel { key, name });
        }
        let mut hermes: Vec<WantedMember> = Vec::with_capacity(raw.hermes.len());
        for member in &raw.hermes {
            let name = HermesName::parse(&member.name).map_err(|_| OrgInvalid::Unknown)?;
            if hermes.iter().any(|seen| seen.name == name) {
                return Err(OrgInvalid::Unknown);
            }
            let level = LevelKey::parse(&member.level)
                .filter(|key| levels.iter().any(|level| level.key == *key))
                .ok_or(OrgInvalid::Unknown)?;
            let x = member
                .x
                .as_i64()
                .ok_or(if member.x.is_number() {
                    OrgInvalid::Place
                } else {
                    OrgInvalid::Unreadable
                })
                .and_then(MapX::new)?;
            let role = RoleText::parse(&member.role)?;
            hermes.push(WantedMember {
                name,
                level,
                x,
                role,
            });
        }
        let mut links: Vec<WantedLink> = Vec::with_capacity(raw.links.len());
        for link in &raw.links {
            let from = HermesName::parse(&link.from).map_err(|_| OrgInvalid::Link)?;
            let to = HermesName::parse(&link.to).map_err(|_| OrgInvalid::Link)?;
            let known = |name: &HermesName| hermes.iter().any(|member| member.name == *name);
            if from == to
                || !known(&from)
                || !known(&to)
                || links.iter().any(|seen| seen.from == from && seen.to == to)
            {
                return Err(OrgInvalid::Link);
            }
            links.push(WantedLink {
                from,
                to,
                wake: link.wake,
                follow: link.follow,
            });
        }
        Ok(Self {
            levels,
            hermes,
            links,
        })
    }

    /// The canonical JSON of the draft, written from the parsed values: the echo of a refused
    /// draft. It never copies input bytes.
    #[must_use]
    pub fn to_json(&self) -> String {
        let levels: Vec<_> = self
            .levels
            .iter()
            .map(
                |level| serde_json::json!({ "key": level.key.text(), "name": level.name.as_str() }),
            )
            .collect();
        let hermes: Vec<_> = self
            .hermes
            .iter()
            .map(|member| {
                serde_json::json!({
                    "name": member.name.as_str(),
                    "level": member.level.text(),
                    "x": member.x.get(),
                    "role": member.role.as_ref().map_or("", RoleText::as_str),
                })
            })
            .collect();
        let links: Vec<_> = self
            .links
            .iter()
            .map(|link| {
                serde_json::json!({
                    "from": link.from.as_str(),
                    "to": link.to.as_str(),
                    "wake": link.wake,
                    "follow": link.follow,
                })
            })
            .collect();
        serde_json::json!({ "levels": levels, "hermes": hermes, "links": links }).to_string()
    }
}

// ---------------------------------------------------------------------------- the stored chart

/// A level, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLevel {
    /// Its row.
    pub id: Uuid,
    /// Its name.
    pub name: LevelName,
}

/// Where a Hermes sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The level.
    pub level: Uuid,
    /// The left edge on the map.
    pub x: MapX,
    /// The role.
    pub role: Option<RoleText>,
}

/// A live Hermes and its place, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMember {
    /// Its row.
    pub id: Uuid,
    /// Its name.
    pub name: HermesName,
    /// Its place. `None`: not placed yet.
    pub place: Option<Place>,
}

/// A link, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLink {
    /// The row of the sender.
    pub from_id: Uuid,
    /// Its name.
    pub from: HermesName,
    /// The row of the recipient.
    pub to_id: Uuid,
    /// Its name.
    pub to: HermesName,
    /// The wake mark.
    pub wake: bool,
    /// The follow mark.
    pub follow: bool,
}

/// The chart as the database holds it, read in one snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chart {
    /// The levels, by position.
    pub levels: Vec<StoredLevel>,
    /// The live Hermes.
    pub members: Vec<StoredMember>,
    /// The links between live Hermes.
    pub links: Vec<StoredLink>,
}

fn put_name(hasher: &mut Sha256, name: &str) {
    // A name is at most 32 bytes.
    hasher.update([u8::try_from(name.len()).unwrap_or(u8::MAX)]);
    hasher.update(name.as_bytes());
}

fn put_count(hasher: &mut Sha256, count: usize) {
    hasher.update(u32::try_from(count).unwrap_or(u32::MAX).to_be_bytes());
}

impl Chart {
    /// The version of the chart: the SHA-256 of its levels, its Hermes with their places and
    /// roles, and its links, each part with its length in front, so that no two charts share a
    /// digest by concatenation. The connection state and the numbers are not
    /// in it: none of them is edited here.
    #[must_use]
    pub fn version(&self) -> ChartVersion {
        let mut hasher = Sha256::new();
        hasher.update(b"arauto/org-chart/1\0");
        put_count(&mut hasher, self.levels.len());
        for level in &self.levels {
            hasher.update(level.id.as_bytes());
            put_name(&mut hasher, level.name.as_str());
        }
        let mut members: Vec<&StoredMember> = self.members.iter().collect();
        members.sort_by_key(|member| member.id);
        put_count(&mut hasher, members.len());
        for member in members {
            hasher.update(member.id.as_bytes());
            put_name(&mut hasher, member.name.as_str());
            match &member.place {
                None => hasher.update([0]),
                Some(place) => {
                    hasher.update([1]);
                    hasher.update(place.level.as_bytes());
                    hasher.update(u32::from(place.x.get()).to_be_bytes());
                    let role = place.role.as_ref().map_or("", RoleText::as_str);
                    hasher.update(u16::try_from(role.len()).unwrap_or(u16::MAX).to_be_bytes());
                    hasher.update(role.as_bytes());
                }
            }
        }
        let mut links: Vec<&StoredLink> = self.links.iter().collect();
        links.sort_by_key(|link| (link.from_id, link.to_id));
        put_count(&mut hasher, links.len());
        for link in links {
            hasher.update(link.from_id.as_bytes());
            hasher.update(link.to_id.as_bytes());
            // wake | follow << 1: the same byte as before while no link is followed.
            hasher.update([u8::from(link.wake) | (u8::from(link.follow) << 1)]);
        }
        ChartVersion(hasher.finalize().into())
    }

    /// The position of a level, if it is one of the chart.
    #[must_use]
    pub fn position_of(&self, level: Uuid) -> Option<usize> {
        self.levels.iter().position(|stored| stored.id == level)
    }

    /// The position of the level a Hermes sits in.
    #[must_use]
    pub fn position_of_member(&self, name: &HermesName) -> Option<usize> {
        let member = self.members.iter().find(|member| member.name == *name)?;
        self.position_of(member.place.as_ref()?.level)
    }
}

/// The reserved words, written the way the page hands them to the script.
#[must_use]
pub fn reserved_words() -> String {
    RESERVED_NAMES.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> HermesName {
        HermesName::parse(text).unwrap()
    }

    #[test]
    fn a_level_name_has_the_format_and_the_reserved_words_of_a_hermes_name() {
        for reserved in RESERVED_NAMES {
            assert_eq!(LevelName::parse(reserved), Err(OrgInvalid::LevelName));
        }
        for bad in ["", &"a".repeat(33), "Board", "-board", "a b", "a.b", "é"] {
            assert_eq!(LevelName::parse(bad), Err(OrgInvalid::LevelName), "{bad}");
        }
        for good in ["board", "level-1", "a_b", "0", &"a".repeat(32)] {
            assert_eq!(LevelName::parse(good).unwrap().as_str(), good);
        }
    }

    #[test]
    fn a_role_is_one_line_of_at_most_280_characters_without_control_or_direction() {
        assert!(RoleText::parse(&"a".repeat(280)).unwrap().is_some());
        assert_eq!(RoleText::parse(&"a".repeat(281)), Err(OrgInvalid::Role));
        assert!(RoleText::parse(&"é".repeat(280)).unwrap().is_some());
        assert_eq!(RoleText::parse(&"é".repeat(281)), Err(OrgInvalid::Role));
        assert_eq!(RoleText::parse(&"😀".repeat(281)), Err(OrgInvalid::Role));
        for bad in [
            "a\nb",
            "a\u{7}b",
            "a\u{202e}b",
            "a\u{2066}b",
            "a\u{e0041}b",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\tb",
            "a\u{85}b",
        ] {
            assert_eq!(RoleText::parse(bad), Err(OrgInvalid::Role), "{bad:?}");
        }
        assert_eq!(
            RoleText::parse("  keeps the books \n")
                .unwrap()
                .unwrap()
                .as_str(),
            "keeps the books"
        );
        assert_eq!(RoleText::parse("").unwrap(), None);
        assert_eq!(RoleText::parse("   \n ").unwrap(), None);
        let shown = format!("{:?}", RoleText::parse("secret words").unwrap());
        assert!(!shown.contains("secret"));
    }

    #[test]
    fn the_kind_of_a_link_comes_from_the_levels_of_its_ends() {
        assert_eq!(kind(Some(0), Some(2)), Some(Kind::Instruction));
        assert_eq!(kind(Some(2), Some(0)), Some(Kind::Request));
        assert_eq!(kind(Some(1), Some(1)), Some(Kind::Note));
        assert_eq!(kind(None, Some(1)), None);
        assert_eq!(kind(Some(1), None), None);
        assert_eq!(kind(None, None), None);
    }

    const A: &str = "11111111-1111-4111-8111-111111111111";

    fn draft_json() -> serde_json::Value {
        serde_json::json!({
            "levels": [
                { "key": A, "name": "board" },
                { "key": "new-1", "name": "level-2" },
            ],
            "hermes": [
                { "name": "ana", "level": A, "x": 300, "role": "leads" },
                { "name": "bia", "level": "new-1", "x": 0, "role": "" },
            ],
            "links": [ { "from": "ana", "to": "bia", "wake": true } ],
        })
    }

    fn parse(value: &serde_json::Value) -> Result<Draft, OrgInvalid> {
        Draft::parse(&value.to_string())
    }

    #[test]
    fn a_draft_is_read_only_in_its_one_shape() {
        let good = parse(&draft_json()).unwrap();
        assert_eq!(good.levels.len(), 2);
        assert_eq!(good.hermes[0].x.get(), 300);
        assert_eq!(good.hermes[1].role, None);
        assert!(good.links[0].wake);

        let with = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut value = draft_json();
            edit(&mut value);
            parse(&value)
        };
        let canary = "zq-canary-9f3a";
        // Shape.
        assert_eq!(Draft::parse("not json"), Err(OrgInvalid::Unreadable));
        assert_eq!(
            with(&|v| v["extra"] = canary.into()),
            Err(OrgInvalid::Unreadable)
        );
        assert_eq!(
            with(&|v| {
                v.as_object_mut().unwrap().remove("links");
            }),
            Err(OrgInvalid::Unreadable)
        );
        assert_eq!(
            with(&|v| v["hermes"][0]["role"] = 5.into()),
            Err(OrgInvalid::Unreadable)
        );
        assert_eq!(
            with(&|v| v["hermes"][0]["x"] = canary.into()),
            Err(OrgInvalid::Unreadable)
        );
        assert_eq!(
            with(&|v| v["hermes"][0][canary] = 1.into()),
            Err(OrgInvalid::Unreadable)
        );
        // Levels.
        assert_eq!(
            with(&|v| v["levels"] = serde_json::json!([])),
            Err(OrgInvalid::LevelCount)
        );
        assert_eq!(
            with(&|v| {
                let many: Vec<_> = (1..=13)
                    .map(|n| serde_json::json!({ "key": format!("new-{n}"), "name": format!("l{n}") }))
                    .collect();
                v["levels"] = many.into();
            }),
            Err(OrgInvalid::LevelCount)
        );
        assert_eq!(
            with(&|v| v["levels"][1]["name"] = "board".into()),
            Err(OrgInvalid::LevelNameTwice)
        );
        assert_eq!(
            with(&|v| v["levels"][1]["name"] = "Bad Name".into()),
            Err(OrgInvalid::LevelName)
        );
        assert_eq!(
            with(&|v| v["levels"][1]["key"] = A.into()),
            Err(OrgInvalid::Unreadable)
        );
        for bad in [
            "new-0",
            "new-13",
            "new-01",
            "new-",
            "x",
            "NEW-1",
            &A.to_uppercase(),
        ] {
            assert_eq!(
                with(&|v| v["levels"][1]["key"] = bad.into()),
                Err(OrgInvalid::Unreadable),
                "{bad}"
            );
        }
        // Hermes.
        assert_eq!(
            with(&|v| v["hermes"][1]["level"] = "new-2".into()),
            Err(OrgInvalid::Unknown)
        );
        assert_eq!(
            with(&|v| v["hermes"][1]["name"] = "ana".into()),
            Err(OrgInvalid::Unknown)
        );
        assert_eq!(
            with(&|v| v["hermes"][1]["name"] = "Bad".into()),
            Err(OrgInvalid::Unknown)
        );
        for bad in [-1_i64, 4001] {
            assert_eq!(
                with(&|v| v["hermes"][0]["x"] = bad.into()),
                Err(OrgInvalid::Place)
            );
        }
        assert_eq!(
            with(&|v| v["hermes"][0]["x"] = 1.5.into()),
            Err(OrgInvalid::Place)
        );
        assert!(with(&|v| v["hermes"][0]["x"] = 4000.into()).is_ok());
        assert_eq!(
            with(&|v| v["hermes"][0]["role"] = "a\nb".into()),
            Err(OrgInvalid::Role)
        );
        // Links.
        assert_eq!(
            with(&|v| v["links"][0]["to"] = "ana".into()),
            Err(OrgInvalid::Link)
        );
        assert_eq!(
            with(&|v| v["links"][0]["to"] = "cleo".into()),
            Err(OrgInvalid::Link)
        );
        assert_eq!(
            with(&|v| {
                let again = v["links"][0].clone();
                v["links"].as_array_mut().unwrap().push(again);
            }),
            Err(OrgInvalid::Link)
        );
        // The follow mark is a boolean that may be left out; anything else is not a draft.
        assert!(!parse(&draft_json()).unwrap().links[0].follow);
        assert!(
            with(&|v| v["links"][0]["follow"] = true.into())
                .unwrap()
                .links[0]
                .follow
        );
        assert_eq!(
            with(&|v| v["links"][0]["follow"] = "true".into()),
            Err(OrgInvalid::Unreadable)
        );
        // No error carries a value of the draft.
        let refused = with(&|v| {
            v["hermes"][0]["role"] = format!("{canary}\n").into();
            v["levels"][1]["name"] = canary.to_uppercase().into();
        })
        .unwrap_err();
        assert!(!refused.to_string().contains(canary));
        assert!(!format!("{refused:?}").to_lowercase().contains(canary));
        let unreadable = Draft::parse(&format!("{{\"levels\": \"{canary}\"")).unwrap_err();
        assert!(!unreadable.to_string().contains(canary));
        assert!(!format!("{unreadable:?}").contains(canary));
    }

    #[test]
    fn the_echo_of_a_draft_is_written_from_the_parsed_values() {
        let mut value = draft_json();
        value["hermes"][0]["role"] = "<script>alert(1)</script>\"'".into();
        let draft = parse(&value).unwrap();
        let echo = draft.to_json();
        assert_eq!(Draft::parse(&echo).unwrap(), draft);
        assert!(
            draft.hermes[0]
                .role
                .as_ref()
                .unwrap()
                .as_str()
                .contains("<script>")
        );
    }

    fn chart() -> Chart {
        let level = Uuid::from_u128(10);
        Chart {
            levels: vec![
                StoredLevel {
                    id: level,
                    name: LevelName::parse("board").unwrap(),
                },
                StoredLevel {
                    id: Uuid::from_u128(11),
                    name: LevelName::parse("floor").unwrap(),
                },
            ],
            members: vec![
                StoredMember {
                    id: Uuid::from_u128(1),
                    name: name("ana"),
                    place: Some(Place {
                        level,
                        x: MapX::new(10).unwrap(),
                        role: RoleText::parse("one").unwrap(),
                    }),
                },
                StoredMember {
                    id: Uuid::from_u128(2),
                    name: name("bia"),
                    place: None,
                },
            ],
            links: vec![StoredLink {
                from_id: Uuid::from_u128(1),
                from: name("ana"),
                to_id: Uuid::from_u128(2),
                to: name("bia"),
                wake: false,
                follow: false,
            }],
        }
    }

    #[test]
    fn the_version_changes_with_every_part_of_the_chart_and_only_with_it() {
        let base = chart();
        let version = base.version();
        assert_eq!(ChartVersion::parse_hex(&version.to_hex()), Some(version));
        assert_eq!(version.to_hex().len(), 64);
        type Change = Box<dyn Fn(&mut Chart)>;
        let changes: Vec<(&str, Change)> = vec![
            (
                "level renamed",
                Box::new(|c| c.levels[0].name = LevelName::parse("top").unwrap()),
            ),
            ("levels reordered", Box::new(|c| c.levels.reverse())),
            (
                "level added",
                Box::new(|c| {
                    c.levels.push(StoredLevel {
                        id: Uuid::from_u128(12),
                        name: LevelName::parse("x").unwrap(),
                    });
                }),
            ),
            (
                "member level",
                Box::new(|c| c.members[0].place.as_mut().unwrap().level = Uuid::from_u128(11)),
            ),
            (
                "member x",
                Box::new(|c| c.members[0].place.as_mut().unwrap().x = MapX::new(11).unwrap()),
            ),
            (
                "member role",
                Box::new(|c| c.members[0].place.as_mut().unwrap().role = None),
            ),
            (
                "member placed",
                Box::new(|c| {
                    c.members[1].place = Some(Place {
                        level: Uuid::from_u128(10),
                        x: MapX::new(0).unwrap(),
                        role: None,
                    });
                }),
            ),
            ("link wake", Box::new(|c| c.links[0].wake = true)),
            ("link follow", Box::new(|c| c.links[0].follow = true)),
            ("link removed", Box::new(|c| c.links.clear())),
            (
                "link added",
                Box::new(|c| {
                    c.links.push(StoredLink {
                        from_id: Uuid::from_u128(2),
                        from: name("bia"),
                        to_id: Uuid::from_u128(1),
                        to: name("ana"),
                        wake: false,
                        follow: false,
                    });
                }),
            ),
            (
                "hermes added",
                Box::new(|c| {
                    c.members.push(StoredMember {
                        id: Uuid::from_u128(3),
                        name: name("cleo"),
                        place: None,
                    });
                }),
            ),
            (
                "hermes removed",
                Box::new(|c| {
                    c.members.pop();
                    c.links.clear();
                }),
            ),
        ];
        let mut seen = vec![version];
        for (what, change) in changes {
            let mut other = base.clone();
            change(&mut other);
            let changed = other.version();
            assert!(!seen.contains(&changed), "{what}");
            seen.push(changed);
        }
        // The same chart built in another order has the same version.
        let mut shuffled = base.clone();
        shuffled.members.reverse();
        assert_eq!(shuffled.version(), version);
        assert_eq!(ChartVersion::parse_hex("ABC"), None);
        assert_eq!(ChartVersion::parse_hex(&"A".repeat(64)), None);
        assert_eq!(ChartVersion::parse_hex(&"g".repeat(64)), None);
    }
}
