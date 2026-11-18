//! The deployment recipes, read as files of the repository, and the security rules each must keep
//! (decision 0052). No database, no network, no container: the YAML is parsed for its structure and
//! the wrapper of the engine is compared, line by line, with the text the test keeps.
//!
//! One named test per rule. Each test fails with a message that names the rule and the place.
//! The rules are functions that collect violations, so that `each_rule_bites_...` can break the
//! recipe in memory and prove that the rule that guards it says so (a rule that cannot fail is not
//! a rule). The wrapper also runs under `/bin/sh` on Unix (`wrapper_behaviour`); that part does not
//! run on Windows.
//!
//! These tests do not run a container: they say that the recipe still has the shape that is safe,
//! not that it works on a real Easypanel (it has never run on one).
//!
//! What the YAML parser cannot see, the rules refuse outright, in all three recipes: it does not
//! apply merge keys (`<<`) and drops tags (`!reset`, `!override`), which Compose acts on, so a
//! merge key, a tag, an anchor or an alias anywhere is a violation, and so is a second document;
//! Compose interpolates every `$` of the file that is not doubled, so the recipe that brings the
//! engine may hold no single `$`. Every map the rules care about is a closed set: a key the test
//! does not know is a violation, not something it skips.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arauto_hub::engine::types::EngineVersion;
use yaml_rust2::parser::{MarkedEventReceiver, Parser, Tag};
use yaml_rust2::scanner::Marker;
use yaml_rust2::{Event, Yaml, YamlLoader};

const ENGINE_RECIPE_PATH: &str = "deploy/easypanel/compose.engine.yaml";
const EASYPANEL_OWN_ENGINE_PATH: &str = "deploy/easypanel/compose.yaml";
const DOCKER_OWN_ENGINE_PATH: &str = "deploy/docker/compose.yaml";
const DOCKERFILE_PATH: &str = "deploy/docker/Dockerfile";
const EASYPANEL_DEFAULTS_PATH: &str = "deploy/easypanel/defaults.env";
const DOCKER_ENV_EXAMPLE_PATH: &str = "deploy/docker/arauto.env.example";

const ENGINE_RECIPE: &str = include_str!("../../../deploy/easypanel/compose.engine.yaml");
const EASYPANEL_OWN_ENGINE: &str = include_str!("../../../deploy/easypanel/compose.yaml");
const DOCKER_OWN_ENGINE: &str = include_str!("../../../deploy/docker/compose.yaml");
const DOCKERFILE: &str = include_str!("../../../deploy/docker/Dockerfile");
const EASYPANEL_DEFAULTS: &str = include_str!("../../../deploy/easypanel/defaults.env");
const DOCKER_ENV_EXAMPLE: &str = include_str!("../../../deploy/docker/arauto.env.example");

const ENGINE_DIGEST_VOLUME: &str = "arauto-engine-digest";
const ENGINE_SESSIONS_VOLUME: &str = "arauto-engine-sessions";
const HUB_DATA_VOLUME: &str = "arauto-data";
const POSTGRES_VOLUME: &str = "arauto-postgres";
const ENGINE_DIGEST_DIR: &str = "/run/arauto-engine";
const HUB_DATA_DIR: &str = "/var/lib/arauto";
const ENGINE_IMAGE_PREFIX: &str = "devlikeapro/waha:gows-";

/// The image reference of the engine that was tested, tag and digest together: the tag says which
/// version, the digest which bytes. The recipe must name exactly this, so other bytes under the
/// tested tag do not pass. A new version changes this line, and `EngineVersion::TESTED` with it.
const ENGINE_IMAGE: &str = "devlikeapro/waha:gows-2026.10.1@sha256:1f7faeb0227ad5edb5711f7058bfce807ea65e5f92234af0778e4db140f6eb46";

/// How the hub is built in every recipe: from the Dockerfile this test reads, and nothing else
/// (no `args`, `target`, `dockerfile_inline` or `additional_contexts`).
const HUB_BUILD: [(&str, &str); 2] = [("context", "../.."), ("dockerfile", DOCKERFILE_PATH)];

/// The only keys of a long-form mount. `bind`, `tmpfs`, `consistency` and the like are refused.
const MOUNT_KEYS: [&str; 5] = ["type", "source", "target", "read_only", "volume"];

/// The only options of the short form of a mount (`source:target:options`).
const MOUNT_OPTIONS: [&str; 3] = ["ro", "rw", "nocopy"];

/// The three variables of the hub that say it brings an engine (plan section 1.1).
const ENGINE_VARIABLES: [&str; 3] = [
    "ARAUTO_ENGINE_URL",
    "ARAUTO_ENGINE_EVENTS_URL",
    "ARAUTO_ENGINE_DIGEST_DIR",
];

/// The only keys the engine service may have. Anything else (`ports`, `container_name`,
/// `command`, `cap_add`, `devices`, `volumes_from`, `network_mode`, `privileged`, `env_file`, ...)
/// is a decision that has not been made, so it fails here.
const ENGINE_KEYS: [&str; 9] = [
    "image",
    "restart",
    "depends_on",
    "entrypoint",
    "environment",
    "volumes",
    "networks",
    "healthcheck",
    "stop_grace_period",
];

/// The command of the health check of the engine, item for item. It runs inside the engine
/// container, also while the wrapper is still waiting for the digest: another command there could
/// start WAHA a second way, with no key or with a key it draws and prints. So, like the wrapper, it
/// is compared with the text this test keeps, and a change of it is read by a person.
const ENGINE_HEALTHCHECK_TEST: [&str; 4] = [
    "CMD",
    "node",
    "-e",
    "fetch('http://127.0.0.1:3000/ping').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))",
];

/// The keys of the health check of the engine, all of them and nothing else (`disable` turns it
/// off, `start_interval` is a decision that has not been made).
const ENGINE_HEALTHCHECK_KEYS: [&str; 5] =
    ["test", "interval", "timeout", "start_period", "retries"];

/// The images postgres may run, exactly. `starts_with("postgres:")` would also take a registry
/// host named postgres (`postgres:5000/acme/engine:1`), which is another image.
const POSTGRES_IMAGES: [&str; 1] = ["postgres:16"];

/// The image the last stage of the Dockerfile starts from. An `ENV` of a base image is not in the
/// text this test reads, so the base is pinned too.
const DOCKERFILE_RUN_BASE: &str = "debian:bookworm-slim";

/// The only `ARAUTO_*` names an `ENV` or an `ARG` of the Dockerfile may set.
const DOCKERFILE_ARAUTO_NAMES: [&str; 4] = [
    "ARAUTO_DATA_DIR",
    "ARAUTO_DOWNLOAD_DIR",
    "ARAUTO_LISTEN",
    "ARAUTO_LISTEN_INTERNAL",
];

/// The only keys the hub service may have. No `ports` (the proxy of Easypanel reaches the hub by
/// the domain set on its screens), no `network_mode`, `volumes_from`, `privileged`, `cap_add` or
/// `devices`.
const HUB_KEYS: [&str; 9] = [
    "build",
    "restart",
    "depends_on",
    "env_file",
    "environment",
    "volumes",
    "networks",
    "ulimits",
    "stop_grace_period",
];

/// The only keys the postgres service may have. The same refusals as for the hub.
const POSTGRES_KEYS: [&str; 8] = [
    "image",
    "restart",
    "environment",
    "command",
    "volumes",
    "networks",
    "healthcheck",
    "stop_grace_period",
];

/// The only services of the recipe: the composition is these three, and a fourth is a decision
/// that has not been made.
const SERVICE_NAMES: [&str; 3] = ["hub", "postgres", "engine"];

/// The only services of a bring-your-own-engine recipe.
const BRING_YOUR_OWN_SERVICES: [&str; 2] = ["hub", "postgres"];

/// The only top-level keys of a bring-your-own-engine recipe (`include` would bring services of
/// another file, `x-...` is where an anchor hides).
const BRING_YOUR_OWN_TOP_LEVEL_KEYS: [&str; 4] = ["name", "services", "volumes", "networks"];

/// The only keys of the hub of a bring-your-own-engine recipe. `extends` would bring a service of
/// another file with its environment; `ports` and `image` are part of those recipes on purpose.
const BRING_YOUR_OWN_HUB_KEYS: [&str; 11] = [
    "image",
    "build",
    "restart",
    "depends_on",
    "env_file",
    "environment",
    "volumes",
    "ports",
    "networks",
    "ulimits",
    "stop_grace_period",
];

/// The name the recipe for Docker gives the image it builds for the hub (the admin may set
/// `ARAUTO_IMAGE`). The recipe for Easypanel gives none.
const BRING_YOUR_OWN_HUB_IMAGE: &str = "${ARAUTO_IMAGE:-arauto-hub:local}";

/// The env files the bring-your-own recipe for Easypanel reads, as it names them: the defaults of
/// the recipe (read by this test) and the `.env` Easypanel writes from its Environment screen
/// (the admin's, which this test cannot read).
const EASYPANEL_OWN_ENV_FILES: [&str; 2] = ["defaults.env", "../../.env"];

/// The recipe for Docker reads no env file into the hub: `--env-file` only feeds interpolation.
const DOCKER_OWN_ENV_FILES: [&str; 0] = [];

/// The only top-level keys of the recipe (`name` would fix the project name, `x-...` is where an
/// anchor hides).
const TOP_LEVEL_KEYS: [&str; 3] = ["services", "volumes", "networks"];

/// The script of the wrapper of the engine, as the shell reads it (the `$$` of Compose already
/// turned into `$`), without comments and without blank lines. It is compared line by line, so no
/// line can be added, removed or changed without changing this text too, and that is the point:
/// a change of the wrapper is a change of a security guard and is read by a person.
const WRAPPER_GOLDEN: &str = r##"
set -eu
digest_file=/run/arauto-engine/api-key.sha512
if [ -n "${WAHA_NO_API_KEY:-}" ] || [ -n "${WHATSAPP_API_KEY_EXCLUDE_PATH:-}" ]; then
  echo "arauto: the engine does not start: a switch that lets calls in without the key is set" >&2
  exit 78
fi
unset WHATSAPP_API_KEY WAHA_API_KEY WAHA_API_KEY_PLAIN DEBUG
trap 'exit 143' TERM INT
waiting=0
while [ ! -e "$digest_file" ] && [ ! -L "$digest_file" ]; do
  if [ "$waiting" -eq 0 ]; then
    echo "arauto: the engine is waiting for the hub to write the key digest" >&2
    waiting=1
  fi
  sleep 2
done
if [ -L "$digest_file" ] || [ ! -f "$digest_file" ]; then
  echo "arauto: the engine does not start: the key digest is a link or not a regular file" >&2
  exit 78
fi
if [ "$(wc -c < "$digest_file")" -ne 135 ]; then
  echo "arauto: the engine does not start: the key digest file is not in the expected format" >&2
  exit 78
fi
digest=$(cat "$digest_file")
if ! printf '%s\n' "$digest" | grep -Eqx 'sha512:[0-9a-f]{128}'; then
  echo "arauto: the engine does not start: the key digest file is not in the expected format" >&2
  exit 78
fi
export WAHA_API_KEY="$digest"
exec "$@"
"##;

/// The title of each rule, for the failure message.
fn rule_title(rule: u8) -> &'static str {
    match rule {
        1 => {
            "the recipe is one YAML document with exactly the services hub, postgres and engine, each with keys from its allowlist (no port, no host network, no volumes_from); postgres runs exactly an image of POSTGRES_IMAGES; no YAML merge key, tag, anchor or alias"
        }
        2 => "the engine image is exactly the tested reference, tag and sha256 digest together",
        3 => {
            "the engine has no env_file, and the recipe has no single `$` anywhere (Compose would interpolate it)"
        }
        4 => {
            "the engine pins exactly the 13 variables as quoted text, and names no key or hook variable"
        }
        5 => {
            "every mount is a named volume from the list of its service (no host path, no bind), with keys and options from a closed set; the engine has exactly two; the digest volume is read-only in the engine and shared with the hub only; the data volume is the hub's alone; no volume_from; every top-level volume is declared with no key (no name, external, driver or driver_opts)"
        }
        6 => {
            "networks have no key but `internal` and `driver: bridge` (no fixed name, external, macvlan, driver_opts, ipam or enable_ipv6); a service joins a network with nothing more (no aliases, no fixed address); engine on one non-internal network with the hub; postgres only on internal networks; engine and postgres share none"
        }
        7 => {
            "the hub sets the three ARAUTO_ENGINE_* values, and the digest folder is the mount target on both sides"
        }
        8 => "the wrapper of the engine and its health check are exactly the text the test keeps",
        9 => {
            "the engine depends on exactly a healthy hub (no required: false, no restart); the hub is built from the Dockerfile this test reads, read as Docker reads it (no parser directive); the image can write the digest"
        }
        10 => {
            "the bring-your-own-engine recipes are one YAML document with no merge key, tag, anchor or alias, keys from closed sets, no engine service, postgres running an image of POSTGRES_IMAGES on internal networks with nothing more, none of the ARAUTO_ENGINE_* variables named at all (not even empty), env files of plain NAME=value lines only, no env file this test does not know, and the hub built from the Dockerfile, which names none of them and sets no ARAUTO_* name off its allowlist"
        }
        _ => "unknown rule",
    }
}

// ------------------------------------------------------------------------------------ report

#[derive(Debug)]
struct Violation {
    rule: u8,
    place: String,
    detail: String,
}

/// What the rules found. A rule never panics: it adds to the report, so a test can also read the
/// report of a recipe that was broken on purpose.
#[derive(Debug, Default)]
struct Report {
    found: Vec<Violation>,
}

impl Report {
    fn fail(&mut self, rule: u8, place: &str, detail: &str) {
        self.found.push(Violation {
            rule,
            place: place.to_owned(),
            detail: detail.to_owned(),
        });
    }

    fn check(&mut self, condition: bool, rule: u8, place: &str, detail: &str) {
        if !condition {
            self.fail(rule, place, detail);
        }
    }

    /// Unwraps a helper's result, or records its message as a violation of the rule.
    fn must<T>(&mut self, rule: u8, place: &str, result: Result<T, String>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(detail) => {
                self.fail(rule, place, &detail);
                None
            }
        }
    }

    fn of_rule(&self, rule: u8) -> Vec<&Violation> {
        self.found.iter().filter(|v| v.rule == rule).collect()
    }

    /// Fails the test, naming the rule and every place, if the rule found anything.
    #[allow(clippy::panic)]
    fn assert_clean(&self, rule: u8) {
        let found = self.of_rule(rule);
        if !found.is_empty() {
            let lines: Vec<String> = found
                .iter()
                .map(|v| format!("  - at {} | {}", v.place, v.detail))
                .collect();
            panic!(
                "RECIPE RULE {rule} BROKEN: {}\n{}",
                rule_title(rule),
                lines.join("\n")
            );
        }
    }
}

/// Fails a test for a reason that is not a rule (a file that cannot be read).
#[allow(clippy::panic)]
fn broken(what: &str, place: &str, detail: &str) -> ! {
    panic!("RECIPE TEST BROKEN: {what} | at {place} | {detail}");
}

// ------------------------------------------------------------------------------------ helpers

fn parse(text: &str) -> Result<Yaml, String> {
    match YamlLoader::load_from_str(text) {
        Ok(mut documents) if documents.len() == 1 => Ok(documents.remove(0)),
        Ok(documents) => Err(format!("found {} YAML documents, not one", documents.len())),
        Err(error) => Err(format!("not valid YAML: {error}")),
    }
}

#[cfg(unix)]
fn load(text: &str, file: &str) -> Yaml {
    parse(text).unwrap_or_else(|detail| broken("the file is one YAML document", file, &detail))
}

fn recipe_text() -> String {
    ENGINE_RECIPE.replace("\r\n", "\n")
}

fn dockerfile_text() -> String {
    DOCKERFILE.replace("\r\n", "\n")
}

#[cfg(unix)]
fn engine_recipe() -> Yaml {
    load(&recipe_text(), ENGINE_RECIPE_PATH)
}

fn key(name: &str) -> Yaml {
    Yaml::String(name.to_owned())
}

/// A child of a mapping, or `None` when the key is missing or the node is not a mapping.
fn child<'a>(node: &'a Yaml, name: &str) -> Option<&'a Yaml> {
    match node {
        Yaml::Hash(map) => map.get(&key(name)),
        _ => None,
    }
}

fn keys_of(node: &Yaml) -> Vec<String> {
    match node {
        Yaml::Hash(map) => map
            .keys()
            .map(|k| scalar(k).unwrap_or_else(|| "<not a scalar>".to_owned()))
            .collect(),
        _ => Vec::new(),
    }
}

/// A scalar as the text YAML gave it; `null` is the empty text.
fn scalar(node: &Yaml) -> Option<String> {
    match node {
        Yaml::String(text) | Yaml::Real(text) => Some(text.clone()),
        Yaml::Integer(number) => Some(number.to_string()),
        Yaml::Boolean(flag) => Some(flag.to_string()),
        Yaml::Null => Some(String::new()),
        _ => None,
    }
}

/// Visits every node of the tree, the keys of the mappings included, with its path.
fn walk(node: &Yaml, path: &str, visit: &mut dyn FnMut(&str, &Yaml)) {
    visit(path, node);
    match node {
        Yaml::Hash(map) => {
            for (name, value) in map {
                let label = scalar(name).unwrap_or_else(|| "<not a scalar>".to_owned());
                walk(name, &format!("{path}.{label} (the key)"), visit);
                walk(value, &format!("{path}.{label}"), visit);
            }
        }
        Yaml::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                walk(item, &format!("{path}[{index}]"), visit);
            }
        }
        _ => {}
    }
}

fn services_of(compose: &Yaml) -> Result<&Yaml, String> {
    match child(compose, "services") {
        Some(services @ Yaml::Hash(_)) => Ok(services),
        _ => Err("no `services` mapping".to_owned()),
    }
}

fn service<'a>(compose: &'a Yaml, name: &str) -> Result<&'a Yaml, String> {
    match child(services_of(compose)?, name) {
        Some(found @ Yaml::Hash(_)) => Ok(found),
        _ => Err(format!("the service `{name}` is missing")),
    }
}

const TAG_DETAIL: &str = "a YAML tag: Compose acts on tags (`!reset` removes a node, `!override` replaces it) and this test's parser drops them, so the rules could not see the result";
const ANCHOR_DETAIL: &str = "a YAML anchor (`&name`): it exists to be repeated by an alias or merged by a merge key, which the rules refuse; a recipe names every value where it is used";
const ALIAS_DETAIL: &str = "a YAML alias (`*name`): it repeats a node written elsewhere, so a rule reading one place would not see where else it lands";
const MERGE_DETAIL: &str = "a YAML merge key `<<`: Compose applies it and this test's parser does not, so the rules could not see what it adds";
const COMPLEX_KEY_DETAIL: &str = "a key that is not a scalar (`? [a, b]`): Compose has no such key, and the rules read keys as text";

/// Finds, on the events of the parser, what `YamlLoader` does not resolve the way Compose does:
/// tags (it throws them away: it keeps neither the tag of a sequence or a mapping, nor that of a
/// quoted scalar), merge keys (it does not apply them), anchors and aliases, keys that are not
/// scalars, and every document after the first.
#[derive(Default)]
struct FeatureFinder {
    /// `(where, what)`, in the order of the text.
    found: Vec<(String, &'static str)>,
    documents: usize,
    /// One frame per open node: `Some(next_is_key)` for a mapping, `None` for a list.
    frames: Vec<Option<bool>>,
}

impl FeatureFinder {
    /// For every node: whether it is the key of the mapping it is in. Flips the turn of that
    /// mapping, so the next node is its value (or the next key).
    fn is_key(&mut self) -> bool {
        match self.frames.last_mut() {
            Some(Some(next_is_key)) => {
                let was = *next_is_key;
                *next_is_key = !was;
                was
            }
            _ => false,
        }
    }

    fn anchor_and_tag(&mut self, line: usize, kind: &str, anchor: usize, tag: Option<&Tag>) {
        if anchor > 0 {
            self.found
                .push((format!("line {line}: {kind} with an anchor"), ANCHOR_DETAIL));
        }
        if let Some(tag) = tag {
            self.found.push((
                format!(
                    "line {line}: {kind} with the tag `{}{}`",
                    tag.handle, tag.suffix
                ),
                TAG_DETAIL,
            ));
        }
    }
}

impl MarkedEventReceiver for FeatureFinder {
    fn on_event(&mut self, event: Event, mark: Marker) {
        let line = mark.line();
        match event {
            Event::DocumentStart => self.documents += 1,
            Event::Alias(_) => {
                self.is_key();
                self.found
                    .push((format!("line {line}: an alias"), ALIAS_DETAIL));
            }
            Event::Scalar(value, _, anchor, tag) => {
                // Any `<<` key, plain or quoted, is refused: refusing the quoted one too costs
                // nothing and leaves no question about how a parser types it.
                if self.is_key() && value == "<<" {
                    self.found
                        .push((format!("line {line}: a merge key `<<`"), MERGE_DETAIL));
                }
                self.anchor_and_tag(line, "a scalar", anchor, tag.as_ref());
            }
            Event::SequenceStart(anchor, tag) => {
                if self.is_key() {
                    self.found
                        .push((format!("line {line}: a list as a key"), COMPLEX_KEY_DETAIL));
                }
                self.anchor_and_tag(line, "a list", anchor, tag.as_ref());
                self.frames.push(None);
            }
            Event::MappingStart(anchor, tag) => {
                if self.is_key() {
                    self.found.push((
                        format!("line {line}: a mapping as a key"),
                        COMPLEX_KEY_DETAIL,
                    ));
                }
                self.anchor_and_tag(line, "a mapping", anchor, tag.as_ref());
                self.frames.push(Some(true));
            }
            Event::SequenceEnd | Event::MappingEnd => {
                self.frames.pop();
            }
            _ => {}
        }
    }
}

/// Records, for the rule, every YAML feature of the text that this test's parser does not resolve
/// the way Compose does (see `FeatureFinder`), and a second document. One helper, called for each
/// of the three recipes.
fn refuse_unresolved(rule: u8, file: &str, text: &str, r: &mut Report) {
    let mut finder = FeatureFinder::default();
    if let Err(error) = Parser::new_from_str(text).load(&mut finder, true) {
        r.fail(rule, file, &format!("not valid YAML: {error}"));
        return;
    }
    for (place, detail) in &finder.found {
        r.fail(rule, &format!("{file} {place}"), detail);
    }
    r.check(
        finder.documents == 1,
        rule,
        file,
        &format!(
            "found {} YAML documents, not one: what a second document says is not read by the rules",
            finder.documents
        ),
    );
}

/// The value of a variable of `environment`, as Compose would hand it to the container.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    /// A quoted or plain string of the map form, or what follows `=` in the list form.
    Text(String),
    /// A number, a boolean or a real of the map form, as this test's parser resolved it. Compose's
    /// parser can resolve the same plain text to another value (`03000` is octal to it), so a
    /// pinned value must be quoted text.
    Typed(String),
    /// No value: `NAME:` (null) in the map form, or a bare `- NAME` in the list form. Compose fills
    /// it from the environment of the shell and from the `.env`, so it is not "unset".
    Unset,
}

impl Value {
    fn text(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    fn shown(&self) -> String {
        match self {
            Value::Text(text) => format!("the text `{text}`"),
            Value::Typed(text) => format!(
                "`{text}`, which is not quoted text (a number, a boolean or a real: Compose's parser can read the same plain text as another value)"
            ),
            Value::Unset => {
                "no value (Compose fills it from the shell or from the .env)".to_owned()
            }
        }
    }
}

/// A name the shell and Compose read as one variable: `[A-Za-z_][A-Za-z0-9_]*`. A `$` in a name of
/// the list form is interpolated, so the name could become anything: it is refused with the rest.
fn is_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The `environment` of a service, in the map form or in the `KEY=value` list form. A name that is
/// not a plain variable name, a name twice in the list form, and a line that is not text are
/// errors (the map form cannot hold a name twice: the parser refuses a duplicated key).
fn environment(service: &Yaml) -> Result<BTreeMap<String, Value>, String> {
    let mut found = BTreeMap::new();
    let mut add = |name: String, value: Value| -> Result<(), String> {
        if !is_variable_name(&name) {
            return Err(format!(
                "the environment name `{name}` is not a plain variable name ([A-Za-z_][A-Za-z0-9_]*)"
            ));
        }
        if found.insert(name.clone(), value).is_some() {
            return Err(format!(
                "the environment names `{name}` twice: which one Compose keeps is not something this test reads"
            ));
        }
        Ok(())
    };
    match child(service, "environment") {
        None | Some(Yaml::Null) => {}
        Some(Yaml::Hash(map)) => {
            for (name, value) in map {
                let Some(name) = scalar(name) else {
                    return Err("an environment name is not a scalar".to_owned());
                };
                let value = match value {
                    Yaml::String(text) => Value::Text(text.clone()),
                    Yaml::Null => Value::Unset,
                    Yaml::Integer(_) | Yaml::Boolean(_) | Yaml::Real(_) => {
                        Value::Typed(scalar(value).unwrap_or_default())
                    }
                    _ => return Err(format!("the value of `{name}` is not a scalar")),
                };
                add(name, value)?;
            }
        }
        Some(Yaml::Array(items)) => {
            for item in items {
                let Yaml::String(line) = item else {
                    return Err("an environment line is not text".to_owned());
                };
                match line.split_once('=') {
                    Some((name, value)) => add(name.to_owned(), Value::Text(value.to_owned()))?,
                    None => add(line.clone(), Value::Unset)?,
                }
            }
        }
        Some(_) => return Err("the environment is not a mapping or a list".to_owned()),
    }
    Ok(found)
}

/// A volume mount of a service: where it comes from, where it lands, and whether it is read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mount {
    /// `volume`, `bind`, `tmpfs`, ...: the long form's `type`; the short form is a `bind` when its
    /// source is a path (starts with `/`, `.` or `~`) and a `volume` otherwise. Empty if missing.
    kind: String,
    source: String,
    target: String,
    read_only: bool,
}

/// The short form's `source` is a path on the host, not the name of a volume.
fn is_host_path(source: &str) -> bool {
    source.starts_with(['/', '.', '~'])
}

fn short_kind(source: &str) -> String {
    if is_host_path(source) {
        "bind"
    } else {
        "volume"
    }
    .to_owned()
}

/// The short form `source:target[:options]`. The options are a closed set (`ro`, `rw`, `nocopy`),
/// and `ro` with `rw` is refused: which one wins is not something this test reads.
fn short_mount(text: &str) -> Result<Mount, String> {
    let parts: Vec<&str> = text.split(':').collect();
    let (source, target, options) = match parts.as_slice() {
        [source, target] => (*source, *target, Vec::new()),
        [source, target, options] => (*source, *target, options.split(',').collect()),
        _ => return Err(format!("`{text}` is not source:target[:options]")),
    };
    if let Some(option) = options.iter().find(|o| !MOUNT_OPTIONS.contains(o)) {
        return Err(format!(
            "`{text}` has the option `{option}`: the options of a mount are ro, rw and nocopy only"
        ));
    }
    if options.contains(&"ro") && options.contains(&"rw") {
        return Err(format!("`{text}` is both ro and rw"));
    }
    Ok(Mount {
        kind: short_kind(source),
        source: source.to_owned(),
        target: target.to_owned(),
        read_only: options.contains(&"ro"),
    })
}

/// The long form: keys from `MOUNT_KEYS`, `type`, `source` and `target` as text, `read_only` a
/// boolean, and `volume` holding at most `nocopy` (a boolean). `bind` (propagation), `tmpfs`,
/// `volume.subpath` and the rest are refused.
fn long_mount(item: &Yaml) -> Result<Mount, String> {
    for field in keys_of(item) {
        if !MOUNT_KEYS.contains(&field.as_str()) {
            return Err(format!(
                "a mount sets `{field}`: the keys of a mount are {MOUNT_KEYS:?} only"
            ));
        }
    }
    let text = |field: &str| match child(item, field) {
        Some(Yaml::String(text)) => Ok(text.clone()),
        None => Ok(String::new()),
        Some(_) => Err(format!("the `{field}` of a mount is not text")),
    };
    let read_only = match child(item, "read_only") {
        None => false,
        Some(Yaml::Boolean(flag)) => *flag,
        Some(_) => return Err("read_only is not a boolean".to_owned()),
    };
    match child(item, "volume") {
        None => {}
        Some(options @ Yaml::Hash(_)) => {
            for field in keys_of(options) {
                let boolean = matches!(child(options, &field), Some(Yaml::Boolean(_)));
                if field != "nocopy" || !boolean {
                    return Err(format!(
                        "the volume options of a mount set `{field}`: only `nocopy`, a boolean (no subpath)"
                    ));
                }
            }
        }
        Some(_) => return Err("the volume options of a mount are not a mapping".to_owned()),
    }
    Ok(Mount {
        kind: text("type")?,
        source: text("source")?,
        target: text("target")?,
        read_only,
    })
}

fn mounts(service: &Yaml) -> Result<Vec<Mount>, String> {
    let Some(node) = child(service, "volumes") else {
        return Ok(Vec::new());
    };
    let Yaml::Array(items) = node else {
        return Err("volumes is not a list".to_owned());
    };
    items
        .iter()
        .map(|item| match item {
            Yaml::String(text) => short_mount(text),
            Yaml::Hash(_) => long_mount(item),
            _ => Err("a volume is not text or a mapping".to_owned()),
        })
        .collect()
}

/// The networks a service joins, in the list form or the map form. In the map form each entry is
/// null or an empty mapping: a service joins a network with nothing more. `aliases` would let it
/// answer for another service (`postgres`, `hub`) on that network; `ipv4_address`,
/// `ipv6_address`, `link_local_ips`, `mac_address`, `driver_opts`, `priority` and the rest are
/// decisions that have not been made.
fn service_networks(service: &Yaml) -> Result<Vec<String>, String> {
    match child(service, "networks") {
        None | Some(Yaml::Null) => Ok(Vec::new()),
        Some(Yaml::Array(items)) => items
            .iter()
            .map(|item| match item {
                Yaml::String(name) => Ok(name.clone()),
                _ => Err("a network of the list is not text".to_owned()),
            })
            .collect(),
        Some(Yaml::Hash(map)) => {
            let mut names = Vec::new();
            for (name, entry) in map {
                let Yaml::String(name) = name else {
                    return Err("a network name is not text".to_owned());
                };
                match entry {
                    Yaml::Null => {}
                    Yaml::Hash(fields) if fields.is_empty() => {}
                    Yaml::Hash(_) => {
                        return Err(format!(
                            "the entry of network `{name}` sets {:?}: a service joins a network with nothing more (no aliases, ipv4_address, ipv6_address, link_local_ips, mac_address, driver_opts or priority); an alias such as `postgres` or `hub` would make it answer for another service",
                            keys_of(entry)
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "the entry of network `{name}` is neither null nor a mapping"
                        ));
                    }
                }
                names.push(name.clone());
            }
            Ok(names)
        }
        Some(_) => Err("networks is not a list or a mapping".to_owned()),
    }
}

/// The wrapper of the engine: the items of its entrypoint, and the raw script (item 3, as the
/// parser read it, with Compose's `$$` still doubled).
fn wrapper(compose: &Yaml) -> Result<(Vec<String>, String), String> {
    let engine = service(compose, "engine")?;
    let Some(Yaml::Array(items)) = child(engine, "entrypoint") else {
        return Err("the engine has no entrypoint list".to_owned());
    };
    let parts: Vec<String> = items.iter().filter_map(scalar).collect();
    if parts.len() != items.len() || parts.len() < 4 {
        return Err("the entrypoint is too short, or an item is not text".to_owned());
    }
    let script = parts[2].clone();
    Ok((parts, script))
}

/// Lines of the script, trimmed of ASCII spaces only, without blanks and without comment lines.
/// `str::trim` would also strip Unicode whitespace (a no-break space, say), which the shell does
/// not treat as blank: such a line must stay different from the line the test keeps.
fn script_lines(script: &str) -> Vec<&str> {
    script
        .lines()
        .map(|line| line.trim_matches(' '))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A `$` that Compose would read as the start of an interpolation: every `$` that is not part of
/// `$$`.
fn has_single_dollar(text: &str) -> bool {
    text.replace("$$", "").contains('$')
}

/// A Compose duration as the recipes write it: text of digits and one unit (`15s`, `500ms`).
fn is_duration(node: Option<&Yaml>) -> bool {
    let Some(Yaml::String(text)) = node else {
        return false;
    };
    ["ms", "s", "m", "h"].iter().any(|unit| {
        text.strip_suffix(unit)
            .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The image of postgres is one of `POSTGRES_IMAGES`, as text, exactly.
fn is_postgres_image(node: Option<&Yaml>) -> bool {
    matches!(node, Some(Yaml::String(text)) if POSTGRES_IMAGES.contains(&text.as_str()))
}

// -------------------------------------------------------------------------------- the rules

fn rule_01(compose: &Yaml, text: &str, r: &mut Report) {
    let file = ENGINE_RECIPE_PATH;

    // A tag (`!reset`, `!override`), a merge key, an anchor or an alias is something Compose acts
    // on and this parser does not resolve the same way: refuse every one, anywhere.
    refuse_unresolved(1, file, text, r);
    let mut unresolved: Vec<String> = Vec::new();
    walk(compose, "", &mut |path, node| {
        if matches!(node, Yaml::Alias(_) | Yaml::BadValue) {
            unresolved.push(path.to_owned());
        }
    });
    for path in unresolved {
        r.fail(
            1,
            &format!("{file}{path}"),
            "a value the parser could not resolve",
        );
    }

    for name in keys_of(compose) {
        r.check(
            TOP_LEVEL_KEYS.contains(&name.as_str()),
            1,
            &format!("{file} {name}"),
            "a top-level key that is not services, volumes or networks",
        );
    }

    // The composition is exactly these three services: a fourth could run an engine of its own
    // (unpinned, without a key, with a published port) and no rule below would look at it.
    if let Ok(services) = services_of(compose) {
        for name in keys_of(services) {
            r.check(
                SERVICE_NAMES.contains(&name.as_str()),
                1,
                &format!("{file} services.{name}"),
                "a service that is not hub, postgres or engine",
            );
        }
    }
    for (name, allowed) in [
        ("hub", HUB_KEYS.as_slice()),
        ("postgres", POSTGRES_KEYS.as_slice()),
    ] {
        match service(compose, name) {
            Ok(this) => {
                for field in keys_of(this) {
                    r.check(
                        allowed.contains(&field.as_str()),
                        1,
                        &format!("{file} services.{name}.{field}"),
                        "this key is not on the allowlist of this service (no port, no network_mode, no volumes_from, no privileged, no cap_add, no devices)",
                    );
                }
            }
            Err(detail) => r.fail(1, &format!("{file} services.{name}"), &detail),
        }
    }
    if let Ok(postgres) = service(compose, "postgres") {
        let image = child(postgres, "image")
            .and_then(scalar)
            .unwrap_or_default();
        r.check(
            is_postgres_image(child(postgres, "image")),
            1,
            &format!("{file} services.postgres.image"),
            &format!(
                "`{image}` is not a postgres image of {POSTGRES_IMAGES:?} (a registry host named postgres, `postgres:5000/...`, is another image)"
            ),
        );
    }

    let engine = match service(compose, "engine") {
        Ok(engine) => engine,
        Err(detail) => {
            r.fail(1, &format!("{file} services.engine"), &detail);
            return;
        }
    };
    for name in keys_of(engine) {
        r.check(
            ENGINE_KEYS.contains(&name.as_str()),
            1,
            &format!("{file} services.engine.{name}"),
            "this key is not on the allowlist of the engine service (no port, no container_name, no command, no capability, no device, no volumes_from, no host network)",
        );
    }
    // Every other key of the engine is read by a rule; this one only needs its shape.
    if let Some(grace) = child(engine, "stop_grace_period") {
        r.check(
            is_duration(Some(grace)),
            1,
            &format!("{file} services.engine.stop_grace_period"),
            "the stop_grace_period of the engine is not a duration as text (`30s`)",
        );
    }
}

fn rule_02(compose: &Yaml, r: &mut Report) {
    let place = format!("{ENGINE_RECIPE_PATH} services.engine.image");
    // The constant itself names a tested version: the test and the hub agree on what was tested.
    let constant_version = ENGINE_IMAGE
        .strip_prefix(ENGINE_IMAGE_PREFIX)
        .and_then(|rest| rest.split_once('@'))
        .map(|(version, _)| version);
    r.check(
        constant_version.is_some_and(|version| {
            EngineVersion::TESTED
                .iter()
                .any(|(tested, engine)| *tested == version && engine.eq_ignore_ascii_case("gows"))
        }),
        2,
        "this test (ENGINE_IMAGE)",
        "the tested image reference of this test does not name a GOWS version of EngineVersion::TESTED",
    );
    let Some(engine) = r.must(2, &place, service(compose, "engine")) else {
        return;
    };
    let Some(Yaml::String(image)) = child(engine, "image") else {
        r.fail(2, &place, "no image, or the image is not text");
        return;
    };
    let image = image.clone();
    r.check(
        image == ENGINE_IMAGE,
        2,
        &place,
        &format!(
            "`{image}` is not the tested image reference `{ENGINE_IMAGE}`: tag and digest are compared together, so other bytes under the tested tag do not pass"
        ),
    );
    let Some((named, digest)) = image.split_once('@') else {
        r.fail(2, &place, &format!("`{image}` has no @sha256 digest"));
        return;
    };
    let hex = digest.strip_prefix("sha256:").unwrap_or("");
    r.check(
        is_hex(hex, 64),
        2,
        &place,
        "the digest is not `sha256:` and 64 lowercase hex characters",
    );
    let Some(version) = named.strip_prefix(ENGINE_IMAGE_PREFIX) else {
        r.fail(
            2,
            &place,
            &format!("`{named}` is not `{ENGINE_IMAGE_PREFIX}<version>`"),
        );
        return;
    };
    let numbers: Vec<&str> = version.split('.').collect();
    let shaped = matches!(numbers.as_slice(), [year, month, patch]
        if year.len() == 4 && (1..=2).contains(&month.len()) && (1..=3).contains(&patch.len())
            && numbers.iter().all(|n| n.bytes().all(|b| b.is_ascii_digit())));
    r.check(
        shaped,
        2,
        &place,
        &format!("the tag version `{version}` is not <year>.<month>.<patch>"),
    );
    r.check(
        EngineVersion::TESTED
            .iter()
            .any(|(tested, engine)| *tested == version && engine.eq_ignore_ascii_case("gows")),
        2,
        &place,
        &format!(
            "version `{version}` (GOWS) is not in EngineVersion::TESTED {:?}",
            EngineVersion::TESTED
        ),
    );
}

fn rule_03(compose: &Yaml, r: &mut Report) {
    let place = format!("{ENGINE_RECIPE_PATH} services.engine");
    let Some(engine) = r.must(3, &place, service(compose, "engine")) else {
        return;
    };
    r.check(
        child(engine, "env_file").is_none(),
        3,
        &format!("{place}.env_file"),
        "the engine must not read an env file",
    );
    // Compose reads every `$` of every value of the file, not only of `environment` and not only
    // of the engine: a value of the hub, of postgres, of a network or a volume would change too.
    // Keys are read as well, which costs nothing.
    let mut dollars: Vec<String> = Vec::new();
    walk(compose, "", &mut |path, node| {
        if let Yaml::String(text) | Yaml::Real(text) = node
            && has_single_dollar(text)
        {
            dollars.push(path.trim_start_matches('.').to_owned());
        }
    });
    for path in dollars {
        r.fail(
            3,
            &format!("{ENGINE_RECIPE_PATH} {path}"),
            "a `$` that is not doubled: Compose would interpolate it from the .env of Easypanel",
        );
    }
}

fn rule_04(compose: &Yaml, r: &mut Report) {
    let place = format!("{ENGINE_RECIPE_PATH} services.engine.environment");
    let Some(engine) = r.must(4, &place, service(compose, "engine")) else {
        return;
    };
    let Some(found) = r.must(4, &place, environment(engine)) else {
        return;
    };

    for name in [
        "WAHA_API_KEY",
        "WHATSAPP_API_KEY",
        "WAHA_API_KEY_PLAIN",
        "WAHA_NO_API_KEY",
        "WHATSAPP_API_KEY_EXCLUDE_PATH",
        "DEBUG",
        "WAHA_DASHBOARD_USERNAME",
        "WAHA_DASHBOARD_PASSWORD",
        "WHATSAPP_SWAGGER_USERNAME",
        "WHATSAPP_SWAGGER_PASSWORD",
    ] {
        r.check(
            !found.contains_key(name),
            4,
            &format!("{place}.{name}"),
            "this variable must not be named in the recipe at all (with no value, Compose fills it from the shell or the .env)",
        );
    }
    for name in found.keys() {
        r.check(
            !name.starts_with("WHATSAPP_HOOK_"),
            4,
            &format!("{place}.{name}"),
            "a WHATSAPP_HOOK_* variable must not be set in the recipe",
        );
    }

    let expected: BTreeMap<String, String> = [
        ("WHATSAPP_DEFAULT_ENGINE", "GOWS"),
        ("WHATSAPP_API_PORT", "3000"),
        ("WAHA_DASHBOARD_ENABLED", "false"),
        ("WAHA_DASHBOARD_NO_PASSWORD", "true"),
        ("WHATSAPP_SWAGGER_ENABLED", "false"),
        ("WHATSAPP_SWAGGER_NO_PASSWORD", "true"),
        ("WAHA_APPS_ENABLED", "false"),
        ("WAHA_PROMETHEUS_ENABLED", "false"),
        ("WAHA_DEBUG_MODE", "false"),
        ("WAHA_PRINT_QR", "false"),
        ("WAHA_LOG_LEVEL", "info"),
        ("WAHA_HTTP_LOG_LEVEL", "debug"),
        ("WAHA_LOCAL_STORE_BASE_DIR", "/app/.sessions"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
    r.check(
        expected.len() == 13,
        4,
        "this test",
        "the table of the test must have 13 entries",
    );

    for (name, value) in &expected {
        match found.get(name) {
            Some(actual) => r.check(
                actual.text() == Some(value.as_str()),
                4,
                &format!("{place}.{name}"),
                &format!("expected the text `{value}`, found {}", actual.shown()),
            ),
            None => r.fail(4, &format!("{place}.{name}"), "the variable is missing"),
        }
    }
    for name in found.keys() {
        r.check(
            expected.contains_key(name),
            4,
            &format!("{place}.{name}"),
            "a variable that is not one of the 13 pinned",
        );
    }
}

fn rule_05(compose: &Yaml, r: &mut Report) {
    let file = ENGINE_RECIPE_PATH;
    let services = match services_of(compose) {
        Ok(services) => services,
        Err(detail) => {
            r.fail(5, file, &detail);
            return;
        }
    };

    let mut digest_mounted_by: Vec<String> = Vec::new();
    for name in keys_of(services) {
        let place = format!("{file} services.{name}");
        let Some(this) = r.must(5, &place, service(compose, &name)) else {
            continue;
        };
        r.check(
            child(this, "volumes_from").is_none(),
            5,
            &format!("{place}.volumes_from"),
            "volumes_from hands a service every volume of another, read-write: it is refused",
        );
        let Some(list) = r.must(5, &format!("{place}.volumes"), mounts(this)) else {
            continue;
        };
        // Every mount of every service is a named volume from the list of that service. The
        // engine is an unofficial client that runs as root: a host path (the Docker socket, `/`,
        // the folder of another volume) would hand it the host.
        let allowed: &[&str] = match name.as_str() {
            "hub" => &[HUB_DATA_VOLUME, ENGINE_DIGEST_VOLUME],
            "postgres" => &[POSTGRES_VOLUME],
            "engine" => &[ENGINE_DIGEST_VOLUME, ENGINE_SESSIONS_VOLUME],
            _ => &[],
        };
        if name == "engine" {
            r.check(
                list.len() == 2,
                5,
                &format!("{place}.volumes"),
                &format!(
                    "the engine has {} mounts, not exactly two (the digest folder, read-only, and the sessions)",
                    list.len()
                ),
            );
        }
        for mount in &list {
            r.check(
                mount.kind == "volume",
                5,
                &format!("{place}.volumes"),
                &format!(
                    "a mount of type `{}` (source `{}`): only named volumes are allowed",
                    mount.kind, mount.source
                ),
            );
            r.check(
                !is_host_path(&mount.source),
                5,
                &format!("{place}.volumes"),
                &format!("the source `{}` is a host path", mount.source),
            );
            r.check(
                allowed.contains(&mount.source.as_str()),
                5,
                &format!("{place}.volumes"),
                &format!(
                    "mounts `{}`, which is not on the list of volumes this service may mount",
                    mount.source
                ),
            );
            if mount.source == HUB_DATA_VOLUME {
                r.check(
                    name == "hub",
                    5,
                    &format!("{place}.volumes"),
                    "the hub's data volume (master key, database password) is mounted by a service that is not the hub",
                );
            }
            if mount.source == ENGINE_DIGEST_VOLUME {
                digest_mounted_by.push(name.clone());
                if name == "engine" {
                    r.check(
                        mount.read_only,
                        5,
                        &format!("{place}.volumes"),
                        "the engine mounts the digest volume read-write",
                    );
                    r.check(
                        mount.target == ENGINE_DIGEST_DIR,
                        5,
                        &format!("{place}.volumes"),
                        &format!("the engine mounts it at `{}`", mount.target),
                    );
                }
            }
        }
    }
    digest_mounted_by.sort();
    r.check(
        digest_mounted_by == ["engine", "hub"],
        5,
        &format!("{file} services.*.volumes"),
        &format!(
            "the digest volume is mounted by {digest_mounted_by:?}, not by exactly hub and engine"
        ),
    );

    // A top-level volume is declared bare (`name:` and nothing else) or as an empty mapping. A
    // fixed `name` or `external` is shared across installations, and `driver` with `driver_opts`
    // (`type: none, o: bind, device: <path>`) is a bind of the host under the name of a volume:
    // the set of keys is closed, so none of them can be added.
    match child(compose, "volumes") {
        Some(volumes @ Yaml::Hash(_)) => {
            for name in keys_of(volumes) {
                match child(volumes, &name) {
                    None | Some(Yaml::Null) => {}
                    Some(Yaml::Hash(map)) if map.is_empty() => {}
                    Some(definition @ Yaml::Hash(_)) => {
                        for field in keys_of(definition) {
                            r.fail(
                                5,
                                &format!("{file} volumes.{name}.{field}"),
                                "a volume is declared with no key: a fixed name or an external volume is shared across installations, and a driver with driver_opts can bind a path of the host under the name of a volume",
                            );
                        }
                    }
                    Some(_) => r.fail(
                        5,
                        &format!("{file} volumes.{name}"),
                        "a volume is declared with no key: this definition is neither empty nor a mapping",
                    ),
                }
            }
        }
        _ => r.fail(5, &format!("{file} volumes"), "no `volumes` mapping"),
    }

    let hub_place = format!("{file} services.hub.volumes");
    let Some(hub) = r.must(5, &hub_place, service(compose, "hub")) else {
        return;
    };
    let Some(hub_mounts) = r.must(5, &hub_place, mounts(hub)) else {
        return;
    };
    match hub_mounts.iter().find(|mount| mount.target == HUB_DATA_DIR) {
        Some(data) => {
            r.check(
                data.source != ENGINE_DIGEST_VOLUME,
                5,
                &hub_place,
                "the digest volume is the hub's data volume",
            );
            let engine_place = format!("{file} services.engine.volumes");
            if let Some(engine) = r.must(5, &engine_place, service(compose, "engine"))
                && let Some(engine_mounts) = r.must(5, &engine_place, mounts(engine))
            {
                r.check(
                    engine_mounts
                        .iter()
                        .all(|mount| mount.source != data.source),
                    5,
                    &engine_place,
                    "the engine mounts the hub's data volume",
                );
                let store = environment(engine).ok().and_then(|env| {
                    env.get("WAHA_LOCAL_STORE_BASE_DIR")
                        .and_then(Value::text)
                        .map(str::to_owned)
                });
                r.check(
                    engine_mounts.iter().any(|mount| {
                        mount.source == ENGINE_SESSIONS_VOLUME
                            && Some(&mount.target) == store.as_ref()
                    }),
                    5,
                    &engine_place,
                    &format!(
                        "no mount of `{ENGINE_SESSIONS_VOLUME}` at WAHA_LOCAL_STORE_BASE_DIR ({store:?}): the sessions would not persist, or would sit in the wrong place"
                    ),
                );
            }
        }
        None => r.fail(
            5,
            &hub_place,
            "the hub has no data volume at /var/lib/arauto",
        ),
    }
    r.check(
        hub_mounts
            .iter()
            .filter(|mount| mount.source == ENGINE_DIGEST_VOLUME)
            .all(|mount| mount.target == ENGINE_DIGEST_DIR),
        5,
        &hub_place,
        "the hub mounts the digest volume somewhere else than /run/arauto-engine",
    );
}

fn rule_06(compose: &Yaml, r: &mut Report) {
    let file = ENGINE_RECIPE_PATH;
    let Some(networks @ Yaml::Hash(_)) = child(compose, "networks") else {
        r.fail(6, &format!("{file} networks"), "no `networks` mapping");
        return;
    };
    let mut internal: BTreeMap<String, bool> = BTreeMap::new();
    for name in keys_of(networks) {
        let place = format!("{file} networks.{name}");
        let definition = child(networks, &name);
        // The keys of a network are a closed set: `internal` (a boolean) and `driver: bridge`.
        // `name` and `external` share the network across installations; `driver_opts` (a macvlan
        // on `parent: eth0`), `ipam`, `enable_ipv6` or another driver can give the engine an
        // address on the LAN of the host without a `ports` key.
        match definition {
            None | Some(Yaml::Null) => {}
            Some(map @ Yaml::Hash(_)) => {
                for field in keys_of(map) {
                    let value = child(map, &field);
                    let allowed = match field.as_str() {
                        "internal" => matches!(value, Some(Yaml::Boolean(_))),
                        "driver" => value.and_then(scalar).as_deref() == Some("bridge"),
                        _ => false,
                    };
                    r.check(
                        allowed,
                        6,
                        &format!("{place}.{field}"),
                        "this key is not allowed on a network: only `internal` (a boolean) and `driver: bridge`; a fixed name or an external network is shared across installations, and a driver, driver_opts, ipam or enable_ipv6 can expose the engine on the LAN of the host",
                    );
                }
            }
            Some(_) => r.fail(
                6,
                &place,
                "a network definition that is neither empty nor a mapping",
            ),
        }
        let is_internal = definition
            .and_then(|definition| child(definition, "internal"))
            .is_some_and(|flag| matches!(flag, Yaml::Boolean(true)));
        internal.insert(name, is_internal);
    }

    let engine_place = format!("{file} services.engine.networks");
    let engine_networks = service(compose, "engine")
        .and_then(service_networks)
        .map_err(|detail| format!("the engine: {detail}"));
    let Some(engine_networks) = r.must(6, &engine_place, engine_networks) else {
        return;
    };
    r.check(
        engine_networks.len() == 1,
        6,
        &engine_place,
        &format!(
            "the engine is on {} networks, not exactly one",
            engine_networks.len()
        ),
    );
    for name in &engine_networks {
        match internal.get(name) {
            Some(false) => {}
            Some(true) => r.fail(
                6,
                &engine_place,
                &format!("network `{name}` is internal: the engine could not reach WhatsApp"),
            ),
            None => r.fail(
                6,
                &engine_place,
                &format!("network `{name}` is not defined in the file"),
            ),
        }
    }

    let hub_place = format!("{file} services.hub.networks");
    let hub_networks = service(compose, "hub")
        .and_then(service_networks)
        .map_err(|detail| format!("the hub: {detail}"));
    if let Some(hub_networks) = r.must(6, &hub_place, hub_networks) {
        r.check(
            engine_networks
                .iter()
                .all(|name| hub_networks.contains(name)),
            6,
            &hub_place,
            "the hub is not on the network of the engine: it could not reach it",
        );
    }

    let postgres_place = format!("{file} services.postgres.networks");
    let postgres_networks = service(compose, "postgres")
        .and_then(service_networks)
        .map_err(|detail| format!("postgres: {detail}"));
    let Some(postgres_networks) = r.must(6, &postgres_place, postgres_networks) else {
        return;
    };
    r.check(
        !postgres_networks.is_empty(),
        6,
        &postgres_place,
        "postgres is on no network of the file (it would get the default one, which is not internal)",
    );
    for name in &postgres_networks {
        r.check(
            internal.get(name) == Some(&true),
            6,
            &postgres_place,
            &format!("postgres is on `{name}`, which is not an internal network of the file"),
        );
        r.check(
            !engine_networks.contains(name),
            6,
            &postgres_place,
            &format!("the engine and postgres share the network `{name}`"),
        );
    }
}

fn rule_07(compose: &Yaml, r: &mut Report) {
    let hub_place = format!("{ENGINE_RECIPE_PATH} services.hub.environment");
    let Some(hub) = r.must(7, &hub_place, service(compose, "hub")) else {
        return;
    };
    let Some(hub_environment) = r.must(7, &hub_place, environment(hub)) else {
        return;
    };
    for (name, value) in [
        ("ARAUTO_ENGINE_URL", "http://engine:3000"),
        ("ARAUTO_ENGINE_EVENTS_URL", "http://hub:8444"),
        ("ARAUTO_ENGINE_DIGEST_DIR", ENGINE_DIGEST_DIR),
    ] {
        match hub_environment.get(name) {
            Some(actual) => r.check(
                actual.text() == Some(value),
                7,
                &format!("{hub_place}.{name}"),
                &format!("expected the text `{value}`, found {}", actual.shown()),
            ),
            None => r.fail(7, &format!("{hub_place}.{name}"), "the variable is missing"),
        }
    }
    let folder = hub_environment
        .get("ARAUTO_ENGINE_DIGEST_DIR")
        .and_then(Value::text)
        .unwrap_or_default()
        .to_owned();
    for side in ["hub", "engine"] {
        let place = format!("{ENGINE_RECIPE_PATH} services.{side}.volumes");
        let Some(this) = r.must(7, &place, service(compose, side)) else {
            continue;
        };
        let Some(list) = r.must(7, &place, mounts(this)) else {
            continue;
        };
        let digest_mounts: Vec<&Mount> = list
            .iter()
            .filter(|mount| mount.source == ENGINE_DIGEST_VOLUME)
            .collect();
        r.check(
            digest_mounts.len() == 1 && digest_mounts[0].target == folder,
            7,
            &place,
            &format!("{side} must mount `{ENGINE_DIGEST_VOLUME}` exactly once, at `{folder}`"),
        );
    }
}

/// Rule 8, on the health check of the engine: a closed set of keys, the command exactly
/// `ENGINE_HEALTHCHECK_TEST`, the times as durations and the retries a positive number.
fn check_engine_healthcheck(compose: &Yaml, r: &mut Report) {
    let place = format!("{ENGINE_RECIPE_PATH} services.engine.healthcheck");
    let Some(engine) = r.must(8, &place, service(compose, "engine")) else {
        return;
    };
    let Some(check @ Yaml::Hash(_)) = child(engine, "healthcheck") else {
        r.fail(
            8,
            &place,
            "the engine has no health check mapping: it is pinned like the wrapper, because it runs in the engine container while the wrapper waits",
        );
        return;
    };
    let fields = keys_of(check);
    r.check(
        fields.len() == ENGINE_HEALTHCHECK_KEYS.len()
            && ENGINE_HEALTHCHECK_KEYS
                .iter()
                .all(|wanted| fields.iter().any(|field| field == wanted)),
        8,
        &place,
        &format!(
            "the health check of the engine has the keys {fields:?}, not exactly {ENGINE_HEALTHCHECK_KEYS:?}"
        ),
    );
    let test: Option<Vec<&str>> = match child(check, "test") {
        Some(Yaml::Array(items)) => items
            .iter()
            .map(|item| match item {
                Yaml::String(text) => Some(text.as_str()),
                _ => None,
            })
            .collect(),
        _ => None,
    };
    r.check(
        test.as_deref() == Some(ENGINE_HEALTHCHECK_TEST.as_slice()),
        8,
        &format!("{place}.test"),
        &format!(
            "the health check runs {test:?}, not the command this test keeps {ENGINE_HEALTHCHECK_TEST:?}: it runs inside the engine container while the wrapper waits for the digest, so another command could start the engine without it. If the change is on purpose, change ENGINE_HEALTHCHECK_TEST with it and have it read as a change of a security guard"
        ),
    );
    for field in ["interval", "timeout", "start_period"] {
        r.check(
            is_duration(child(check, field)),
            8,
            &format!("{place}.{field}"),
            "not a duration as text (`15s`)",
        );
    }
    r.check(
        matches!(child(check, "retries"), Some(Yaml::Integer(n)) if *n > 0),
        8,
        &format!("{place}.retries"),
        "not a positive number",
    );
}

fn rule_08(compose: &Yaml, r: &mut Report) {
    check_engine_healthcheck(compose, r);
    let place = format!("{ENGINE_RECIPE_PATH} services.engine.entrypoint");
    let Some((parts, raw)) = r.must(8, &place, wrapper(compose)) else {
        return;
    };
    r.check(
        parts[0] == "/bin/sh" && parts[1] == "-c",
        8,
        &place,
        "the entrypoint must start with /bin/sh and -c",
    );
    r.check(
        parts[3..] == ["arauto-engine", "/usr/bin/tini", "--", "/entrypoint.sh"],
        8,
        &place,
        &format!(
            "the arguments after the script must be exactly arauto-engine, /usr/bin/tini, --, /entrypoint.sh; found {:?}",
            &parts[3..]
        ),
    );
    r.check(
        !has_single_dollar(&raw),
        8,
        &place,
        "a `$` of the script is not doubled: Compose would interpolate it",
    );

    let script = raw.replace("$$", "$");
    let found = script_lines(&script);
    let expected = script_lines(WRAPPER_GOLDEN);
    if found != expected {
        let at = (0..found.len().max(expected.len()))
            .find(|index| found.get(*index) != expected.get(*index))
            .unwrap_or(0);
        r.fail(
            8,
            &format!("{place} (script, line {} without comments and blanks)", at + 1),
            &format!(
                "the wrapper is not the text this test keeps: found {:?}, expected {:?}. If the change is on purpose, change WRAPPER_GOLDEN with it and have it read as a change of a security guard",
                found.get(at),
                expected.get(at)
            ),
        );
    }
}

/// The arguments of a Dockerfile line if its instruction is `word`: instructions are
/// case-insensitive, so `healthcheck none` and `user root` count as much as the upper-case form.
fn instruction_arguments<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let (first, rest) = line.trim().split_once(char::is_whitespace)?;
    first.eq_ignore_ascii_case(word).then(|| rest.trim())
}

/// The instructions of a Dockerfile as Docker reads them, each with the number of its first line:
/// CRLF made LF; comment lines and blank lines dropped, inside a continued instruction too (Docker
/// drops them there as well, so a step written in a comment is no step); and a line that ends in
/// `\` (spaces or tabs after it allowed) joined to the next with nothing in between, so
/// `ARAUTO_\` + `ENGINE_URL=...` is the one name Docker sees. `refuse_dockerfile_directives` keeps
/// `\` the character that continues a line.
fn dockerfile_instructions(dockerfile: &str) -> Vec<(usize, String)> {
    let text = dockerfile.replace("\r\n", "\n");
    let mut found = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let end = line.trim_end_matches([' ', '\t']);
        let (piece, continues) = match end.strip_suffix('\\') {
            Some(piece) => (piece, true),
            None => (line, false),
        };
        let (_, joined) = open.get_or_insert_with(|| (index + 1, String::new()));
        joined.push_str(piece);
        if !continues {
            found.extend(open.take());
        }
    }
    found.extend(open);
    found
}

/// Records every comment line of the Dockerfile with the shape of a parser directive
/// (`# escape=`, `# syntax=`, `# check=`): `escape` changes the character that continues a line,
/// and `syntax` hands the whole file to another frontend, which may read it any way it likes. The
/// rules read the Dockerfile as the default parser does, so a directive is refused wherever a
/// comment has its shape.
fn refuse_dockerfile_directives(rule: u8, dockerfile: &str, r: &mut Report) {
    for (index, line) in dockerfile.replace("\r\n", "\n").lines().enumerate() {
        let Some(comment) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let comment = comment.trim_start();
        let word_end = comment
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(comment.len());
        let (word, rest) = comment.split_at(word_end);
        let directive = word.starts_with(|c: char| c.is_ascii_alphabetic())
            && rest.trim_start().starts_with('=');
        r.check(
            !directive,
            rule,
            &format!("{DOCKERFILE_PATH}:{}", index + 1),
            &format!(
                "a parser directive `{word}=`: it changes how Docker reads the file (`escape` changes the continuation of a line, `syntax` hands the file to another frontend), and this test reads it as the default parser does"
            ),
        );
    }
}

fn rule_09(compose: &Yaml, dockerfile: &str, r: &mut Report) {
    let engine_place = format!("{ENGINE_RECIPE_PATH} services.engine");
    if let Some(engine) = r.must(9, &engine_place, service(compose, "engine")) {
        // `depends_on` is a closed set: exactly `hub`, and on it exactly `condition`. The list
        // form only waits for the start of the hub; `required: false` lets Compose start the
        // engine without a healthy hub; `restart: true` bounces the engine with the hub.
        let depends = child(engine, "depends_on");
        match depends {
            Some(map @ Yaml::Hash(_)) => {
                let names = keys_of(map);
                r.check(
                    names == ["hub"],
                    9,
                    &format!("{engine_place}.depends_on"),
                    &format!("the engine depends on exactly the hub, found {names:?}"),
                );
            }
            _ => r.fail(
                9,
                &format!("{engine_place}.depends_on"),
                "depends_on is not a mapping (the list form waits for the start of the hub, not for its health)",
            ),
        }
        let hub_dependency = depends.and_then(|depends| child(depends, "hub"));
        if let Some(entry @ Yaml::Hash(_)) = hub_dependency {
            for field in keys_of(entry) {
                r.check(
                    field == "condition",
                    9,
                    &format!("{engine_place}.depends_on.hub.{field}"),
                    "the dependency on the hub has only `condition`: `required: false` lets the engine start without a healthy hub, and a restart of the hub must not bounce the engine (decision 0003)",
                );
            }
        }
        let condition = match hub_dependency.and_then(|hub| child(hub, "condition")) {
            Some(Yaml::String(text)) => Some(text.clone()),
            _ => None,
        };
        r.check(
            condition.as_deref() == Some("service_healthy"),
            9,
            &format!("{engine_place}.depends_on.hub.condition"),
            &format!("found {condition:?}, expected service_healthy"),
        );
        let restart = child(engine, "restart").and_then(scalar);
        r.check(
            restart.as_deref() == Some("unless-stopped"),
            9,
            &format!("{engine_place}.restart"),
            &format!(
                "found {restart:?}, expected unless-stopped (it is the retry loop of the wait)"
            ),
        );
    }

    // The checks of the Dockerfile below are worth something only if the hub is built from it.
    let hub_place = format!("{ENGINE_RECIPE_PATH} services.hub");
    if let Some(hub) = r.must(9, &hub_place, service(compose, "hub")) {
        check_build(9, &hub_place, hub, r);
    }

    // The instructions as Docker reads them: a comment is no step, even inside a continued line.
    refuse_dockerfile_directives(9, dockerfile, r);
    let instructions: Vec<String> = dockerfile_instructions(dockerfile)
        .into_iter()
        .map(|(_, instruction)| instruction)
        .collect();
    // Only the last stage is the image that runs: a step, a user or a health check of a builder
    // stage says nothing about it.
    let first = instructions
        .iter()
        .rposition(|instruction| instruction_arguments(instruction, "FROM").is_some())
        .map_or(0, |index| index + 1);
    let last_stage = &instructions[first..];
    let healthchecks: Vec<&str> = last_stage
        .iter()
        .map(|instruction| instruction.trim_start())
        .filter(|instruction| instruction_arguments(instruction, "HEALTHCHECK").is_some())
        .collect();
    r.check(
        !healthchecks.is_empty(),
        9,
        &format!("{DOCKERFILE_PATH} HEALTHCHECK"),
        "the image has no HEALTHCHECK instruction",
    );
    for line in healthchecks {
        let first = line
            .split_whitespace()
            .skip(1)
            .find(|word| !word.starts_with("--"))
            .unwrap_or("");
        r.check(
            first.eq_ignore_ascii_case("CMD"),
            9,
            &format!("{DOCKERFILE_PATH} HEALTHCHECK"),
            &format!("the HEALTHCHECK is `{first}`, not CMD (`HEALTHCHECK NONE` turns it off)"),
        );
    }
    let user = last_stage
        .iter()
        .rev()
        .find_map(|instruction| instruction_arguments(instruction, "USER"));
    r.check(
        user == Some("arauto"),
        9,
        &format!("{DOCKERFILE_PATH} USER"),
        &format!("the image must end running as the user arauto, found {user:?}"),
    );
    let steps: Vec<&str> = last_stage
        .iter()
        .flat_map(|instruction| instruction.split("&&"))
        .map(str::trim)
        .collect();
    r.check(
        steps
            .iter()
            .any(|step| step.starts_with("mkdir") && step.contains(ENGINE_DIGEST_DIR)),
        9,
        &format!("{DOCKERFILE_PATH} mkdir"),
        "the image does not create /run/arauto-engine",
    );
    r.check(
        steps.iter().any(|step| {
            step.starts_with("chown ")
                && step.split_whitespace().any(|word| word == "arauto:arauto")
                && step
                    .split_whitespace()
                    .any(|word| word == ENGINE_DIGEST_DIR)
        }),
        9,
        &format!("{DOCKERFILE_PATH} chown"),
        "the image does not give /run/arauto-engine to arauto:arauto",
    );
}

/// The hub is built from the Dockerfile this test reads, and from nothing else: `build` is exactly
/// `HUB_BUILD`. Another `dockerfile`, a `dockerfile_inline`, `args`, a `target` or
/// `additional_contexts` would build an image the test never read.
fn check_build(rule: u8, place: &str, hub: &Yaml, r: &mut Report) {
    let place = format!("{place}.build");
    let Some(build @ Yaml::Hash(_)) = child(hub, "build") else {
        r.fail(
            rule,
            &place,
            "the hub is not built from the Dockerfile this test reads (no build mapping)",
        );
        return;
    };
    for field in keys_of(build) {
        let expected = HUB_BUILD
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, value)| *value);
        let found = match child(build, &field) {
            Some(Yaml::String(text)) => Some(text.as_str()),
            _ => None,
        };
        r.check(
            expected.is_some() && found == expected,
            rule,
            &format!("{place}.{field}"),
            &format!(
                "the hub is built with `{field}` = {found:?}: it must be built from the Dockerfile this test reads, with exactly {HUB_BUILD:?}"
            ),
        );
    }
    for (field, _) in HUB_BUILD {
        r.check(
            child(build, field).is_some(),
            rule,
            &format!("{place}.{field}"),
            &format!("the hub's build has no `{field}`: it must be built from the Dockerfile this test reads"),
        );
    }
}

/// Rules 1 to 9, on the text of one recipe and one Dockerfile. A text that does not load as one
/// YAML document breaks every rule: none of them can be read.
fn check_all(recipe: &str, dockerfile: &str) -> Report {
    let mut report = Report::default();
    let compose = match parse(recipe) {
        Ok(compose) => compose,
        Err(detail) => {
            refuse_unresolved(1, ENGINE_RECIPE_PATH, recipe, &mut report);
            for rule in 1..=9 {
                report.fail(rule, ENGINE_RECIPE_PATH, &detail);
            }
            return report;
        }
    };
    let compose = &compose;
    rule_01(compose, recipe, &mut report);
    rule_02(compose, &mut report);
    rule_03(compose, &mut report);
    rule_04(compose, &mut report);
    rule_05(compose, &mut report);
    rule_06(compose, &mut report);
    rule_07(compose, &mut report);
    rule_08(compose, &mut report);
    rule_09(compose, dockerfile, &mut report);
    report
}

/// The env files a service reads, as the recipe names them: the short form (`env_file: a.env`),
/// the list of text, or the list of `{path, required}`. Any other key (`format`) is an error.
fn env_file_paths(service: &Yaml) -> Result<Vec<String>, String> {
    let entry = |item: &Yaml| match item {
        Yaml::String(path) => Ok(path.clone()),
        Yaml::Hash(_) => {
            for field in keys_of(item) {
                let allowed = match field.as_str() {
                    "path" => matches!(child(item, "path"), Some(Yaml::String(_))),
                    "required" => matches!(child(item, "required"), Some(Yaml::Boolean(_))),
                    _ => false,
                };
                if !allowed {
                    return Err(format!(
                        "an env_file entry sets `{field}`: only `path` (text) and `required` (a boolean)"
                    ));
                }
            }
            match child(item, "path") {
                Some(Yaml::String(path)) => Ok(path.clone()),
                _ => Err("an env_file entry has no path".to_owned()),
            }
        }
        _ => Err("an env_file entry is neither text nor a mapping".to_owned()),
    };
    match child(service, "env_file") {
        None => Ok(Vec::new()),
        Some(Yaml::Array(items)) => items.iter().map(entry).collect(),
        Some(single) => Ok(vec![entry(single)?]),
    }
}

/// Rule 10, on recipes of the bring-your-own-engine kind: `(path, text, env files it may read)`
/// of each.
fn rule_10_recipes(files: &[(&str, &str, &[&str])], r: &mut Report) {
    for (path, text, env_files) in files {
        // The same refusal of merge keys, tags, anchors, aliases and second documents as the
        // recipe that brings the engine: a `<<: *engine` under `environment` is three variables
        // this parser would not see.
        refuse_unresolved(10, path, text, r);
        let compose = match parse(text) {
            Ok(compose) => compose,
            Err(detail) => {
                r.fail(10, path, &detail);
                continue;
            }
        };
        for name in keys_of(&compose) {
            r.check(
                BRING_YOUR_OWN_TOP_LEVEL_KEYS.contains(&name.as_str()),
                10,
                &format!("{path} {name}"),
                "a top-level key that is not name, services, volumes or networks (`include` brings the services of another file)",
            );
        }
        let names = match services_of(&compose) {
            Ok(services) => keys_of(services),
            Err(detail) => {
                r.fail(10, path, &detail);
                continue;
            }
        };
        for name in names {
            // Two services only: whatever else is there could be an engine under another name.
            r.check(
                name != "engine" && BRING_YOUR_OWN_SERVICES.contains(&name.as_str()),
                10,
                &format!("{path} services.{name}"),
                "this recipe must not have an engine service, or any service but hub and postgres",
            );
            let service_place = format!("{path} services.{name}");
            let Some(this) = r.must(10, &service_place, service(&compose, &name)) else {
                continue;
            };
            // The keys of each service are a closed set: `extends` would bring a service of
            // another file with its environment, and a key this test does not know is a decision
            // that has not been made.
            let allowed: &[&str] = match name.as_str() {
                "hub" => &BRING_YOUR_OWN_HUB_KEYS,
                "postgres" => &POSTGRES_KEYS,
                _ => &[],
            };
            for field in keys_of(this) {
                r.check(
                    allowed.contains(&field.as_str()),
                    10,
                    &format!("{service_place}.{field}"),
                    "this key is not on the allowlist of this service (`extends` brings a service of another file with its environment)",
                );
            }
            let image = child(this, "image").and_then(scalar).unwrap_or_default();
            r.check(
                !image.to_ascii_lowercase().contains("waha"),
                10,
                &format!("{service_place}.image"),
                &format!("`{image}` looks like the WhatsApp engine"),
            );
            // The image of each service is a closed set too: an engine of another author would
            // not have `waha` in its name. The hub is built (its `image` only names the result,
            // and in the recipe for Docker the admin may name it), postgres runs postgres.
            let image_node = child(this, "image");
            let image_known = match name.as_str() {
                "hub" => match image_node {
                    None => true,
                    Some(Yaml::String(text)) => text == BRING_YOUR_OWN_HUB_IMAGE,
                    Some(_) => false,
                },
                "postgres" => is_postgres_image(image_node),
                _ => false,
            };
            r.check(
                image_known,
                10,
                &format!("{service_place}.image"),
                &format!(
                    "`{image}` is not the image of this service: the hub has none or `{BRING_YOUR_OWN_HUB_IMAGE}`, postgres one of {POSTGRES_IMAGES:?} (a registry host named postgres is another image)"
                ),
            );
            if name == "hub" {
                check_build(10, &service_place, this, r);
                // A service joins a network with nothing more: an alias `postgres` on the shared
                // network of the engine would answer for the database there.
                r.must(
                    10,
                    &format!("{service_place}.networks"),
                    service_networks(this),
                );
            }
            if name == "postgres" {
                check_own_postgres_networks(path, &compose, this, r);
            }
            // Every env file the service reads is one this test reads, or the admin's `.env`.
            if let Some(paths) = r.must(
                10,
                &format!("{service_place}.env_file"),
                env_file_paths(this),
            ) {
                for env_file in paths {
                    r.check(
                        env_files.contains(&env_file.as_str()),
                        10,
                        &format!("{service_place}.env_file"),
                        &format!(
                            "reads the env file `{env_file}`, which is not one of {env_files:?}: this test does not read it, and it could set the ARAUTO_ENGINE_* variables"
                        ),
                    );
                }
            }
            let place = format!("{service_place}.environment");
            let Some(variables) = r.must(10, &place, environment(this)) else {
                continue;
            };
            for variable in ENGINE_VARIABLES {
                if let Some(value) = variables.get(variable) {
                    r.fail(
                        10,
                        &format!("{place}.{variable}"),
                        &format!(
                            "this variable must not be named in this recipe at all; found {}",
                            value.shown()
                        ),
                    );
                }
            }
        }
    }
}

/// Rule 10, on postgres of a bring-your-own recipe: it joins at least one network, each defined in
/// the file with `internal: true` and nothing more but `driver: bridge` (the closed set of rule 6).
/// A network with a fixed name, an external one, or one that is not internal (the shared network
/// of the engine, `arauto-engine`) would let the engine of the admin, or anything on it, reach the
/// database by the name postgres.
fn check_own_postgres_networks(path: &str, compose: &Yaml, postgres: &Yaml, r: &mut Report) {
    let place = format!("{path} services.postgres.networks");
    let Some(names) = r.must(10, &place, service_networks(postgres)) else {
        return;
    };
    r.check(
        !names.is_empty(),
        10,
        &place,
        "postgres is on no network of the file (it would get the default one, which is not internal)",
    );
    let defined = child(compose, "networks");
    for name in &names {
        let closed = match defined.and_then(|networks| child(networks, name)) {
            Some(map @ Yaml::Hash(_)) => {
                matches!(child(map, "internal"), Some(Yaml::Boolean(true)))
                    && keys_of(map).iter().all(|field| match field.as_str() {
                        "internal" => true,
                        "driver" => {
                            child(map, "driver").and_then(scalar).as_deref() == Some("bridge")
                        }
                        _ => false,
                    })
            }
            _ => false,
        };
        r.check(
            closed,
            10,
            &place,
            &format!(
                "postgres is on `{name}`, which is not a network of the file with `internal: true` and nothing more (no name, external, ipam or driver_opts): the engine of the admin, or anything else on a shared network, would reach the database by the name postgres"
            ),
        );
    }
}

/// A line of a dotenv file without a leading byte order mark and the blanks around it.
fn dotenv_trimmed(line: &str) -> &str {
    line.trim_start_matches('\u{feff}').trim()
}

/// The name a line of a dotenv file sets, if it sets one: after an optional `export` and any blank
/// (Compose strips `^export\s+`, a tab as much as a space), up to `=`, `:` (the YAML-like form the
/// dotenv parser of Compose also takes) or a blank. A bare `NAME` counts: Compose fills it from
/// the environment.
fn dotenv_name(line: &str) -> Option<&str> {
    let line = dotenv_trimmed(line);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let line = match line.split_once(char::is_whitespace) {
        Some(("export", rest)) => rest.trim_start(),
        _ => line,
    };
    line.split(|c: char| c == '=' || c == ':' || c.is_whitespace())
        .next()
        .map(|name| name.trim_matches(|c| c == '"' || c == '\''))
}

/// A line of a dotenv file is in the one form this test reads: blank, a comment, or `NAME=value`
/// with a plain variable name and a value of letters, digits and `._,:/+@%-` only. Every other
/// form (`export`, a quote, which can carry the value over lines, a `$`, a blank, a backslash, the
/// YAML-like `NAME: value`, a bare `NAME`) is one whose reading by Compose this test does not
/// reproduce, so it is refused rather than read.
fn dotenv_line_is_plain(line: &str) -> bool {
    let line = dotenv_trimmed(line);
    if line.is_empty() || line.starts_with('#') {
        return true;
    }
    line.split_once('=').is_some_and(|(name, value)| {
        is_variable_name(name)
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._,:/+@%-".contains(c))
    })
}

/// Rule 10, on the files of variables those recipes read: `(path, text)` of each. One of the three
/// names is refused with any value, an empty one, or none; so is a `COMPOSE_*` name, which an env
/// file given to Compose with `--env-file` turns into a setting of Compose itself
/// (`COMPOSE_FILE` adds files to the recipe, `COMPOSE_PROFILES` starts services of a profile).
fn rule_10_env_files(files: &[(&str, &str)], r: &mut Report) {
    for (path, text) in files {
        for (number, line) in text.lines().enumerate() {
            // The name only, never the value: an env file of an admin can hold a password.
            r.check(
                dotenv_line_is_plain(line),
                10,
                &format!("{path}:{}", number + 1),
                &format!(
                    "the line that names {:?} is not a plain NAME=value (no export, quote, `$`, blank, backslash or `NAME: value`): this test reads only that form",
                    dotenv_name(line).unwrap_or("?")
                ),
            );
            if let Some(name) = dotenv_name(line) {
                r.check(
                    !ENGINE_VARIABLES.contains(&name),
                    10,
                    &format!("{path}:{}", number + 1),
                    &format!("{name} is named in this file (with any value, an empty one or none)"),
                );
                r.check(
                    !name.starts_with("COMPOSE_"),
                    10,
                    &format!("{path}:{}", number + 1),
                    &format!(
                        "{name} is a setting of Compose itself (COMPOSE_FILE adds files to the recipe): the rules would not read what it brings"
                    ),
                );
            }
        }
    }
}

/// The files Compose reads by itself beside a `compose.yaml`, without a `-f`: an override file is
/// merged into the recipe, and a second default name is one more recipe. The `.env` is not here:
/// it is ignored by git, so it is the admin's and not part of a published recipe.
const IMPLICIT_COMPOSE_FILES: [&str; 7] = [
    "compose.override.yaml",
    "compose.override.yml",
    "docker-compose.override.yaml",
    "docker-compose.override.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
    "compose.yml",
];

/// Records, for the rule, every file of `folder` that Compose would read by itself.
fn refuse_implicit_files(rule: u8, label: &str, folder: &Path, r: &mut Report) {
    match std::fs::read_dir(folder) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                r.check(
                    !IMPLICIT_COMPOSE_FILES.contains(&name.as_str()),
                    rule,
                    &format!("{label}/{name}"),
                    "Compose reads this file by itself beside compose.yaml (an override is merged into the recipe): the rules never read it",
                );
            }
        }
        Err(error) => r.fail(
            rule,
            label,
            &format!("the folder cannot be listed: {error}"),
        ),
    }
}

/// A folder of the repository, from the folder of this crate.
fn repository_folder(folder: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(folder)
}

/// Rule 10, on the Dockerfile every recipe builds the hub from, read as Docker reads it (lines
/// continued, comments dropped, no parser directive): no instruction names `ARAUTO_ENGINE`, even
/// with blanks inside the name; every `ENV` and `ARG` is plain `NAME=value` words and sets no
/// `ARAUTO_*` name off `DOCKERFILE_ARAUTO_NAMES`; and the last stage starts from
/// `DOCKERFILE_RUN_BASE`, whose `ENV` this test does not read. So the image does not turn the
/// engine installation on in the bring-your-own recipes.
fn rule_10_dockerfile(dockerfile: &str, r: &mut Report) {
    refuse_dockerfile_directives(10, dockerfile, r);
    let instructions = dockerfile_instructions(dockerfile);
    for (number, instruction) in &instructions {
        let place = format!("{DOCKERFILE_PATH}:{number}");
        let squeezed: String = instruction
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_uppercase();
        r.check(
            !squeezed.contains("ARAUTO_ENGINE"),
            10,
            &place,
            "the Dockerfile names an ARAUTO_ENGINE variable: every recipe, the bring-your-own ones included, would get it",
        );
        for word in ["ENV", "ARG"] {
            let Some(arguments) = instruction_arguments(instruction, word) else {
                continue;
            };
            if arguments.contains(['"', '\'', '$', '\\', '`']) {
                r.fail(
                    10,
                    &place,
                    &format!(
                        "an {word} with a quote, a `$` or an escape: this test reads only plain NAME=value words, and a name built from a variable could be any name"
                    ),
                );
                continue;
            }
            let tokens: Vec<&str> = arguments.split_whitespace().collect();
            // `ENV NAME value` (the old form) sets one name; otherwise every word is a name, with
            // `=value` or (for an ARG) without.
            let names: Vec<&str> = match tokens.first() {
                Some(first) if word == "ENV" && !first.contains('=') => vec![*first],
                _ => tokens
                    .iter()
                    .map(|token| token.split_once('=').map_or(*token, |(name, _)| name))
                    .collect(),
            };
            for name in names {
                r.check(
                    is_variable_name(name)
                        && (!name.starts_with("ARAUTO_") || DOCKERFILE_ARAUTO_NAMES.contains(&name)),
                    10,
                    &place,
                    &format!(
                        "the Dockerfile sets `{name}` with an {word}: an ARAUTO_* name must be one of {DOCKERFILE_ARAUTO_NAMES:?}, and every name a plain variable name"
                    ),
                );
            }
        }
    }
    let base = instructions
        .iter()
        .rev()
        .find_map(|(_, instruction)| instruction_arguments(instruction, "FROM"));
    r.check(
        base == Some(DOCKERFILE_RUN_BASE),
        10,
        &format!("{DOCKERFILE_PATH} FROM"),
        &format!(
            "the last stage starts FROM {base:?}, not `{DOCKERFILE_RUN_BASE}`: the ENV of another base image is not in the text this test reads"
        ),
    );
}

/// The bring-your-own recipes as this test reads them, with the env files each may name.
fn bring_your_own_recipes() -> [(&'static str, &'static str, &'static [&'static str]); 2] {
    [
        (
            EASYPANEL_OWN_ENGINE_PATH,
            EASYPANEL_OWN_ENGINE,
            &EASYPANEL_OWN_ENV_FILES,
        ),
        (
            DOCKER_OWN_ENGINE_PATH,
            DOCKER_OWN_ENGINE,
            &DOCKER_OWN_ENV_FILES,
        ),
    ]
}

// -------------------------------------------------------------------------------- rule tests

/// Rule 1. Protects decision 0052 (the engine has no published port and no domain) and the
/// principle "never expose the WhatsApp engine outside the internal network" (CLAUDE.md). The
/// engine answers to the hub on a network of the composition, never on the host. The engine
/// service has a closed list of keys, so nothing that changes how it runs is added unseen, and no
/// YAML merge key can add one behind the parser.
#[test]
fn rule_01_engine_publishes_no_port() {
    let mut report = check_all(&recipe_text(), &dockerfile_text());
    refuse_implicit_files(
        1,
        "deploy/easypanel",
        &repository_folder("deploy/easypanel"),
        &mut report,
    );
    report.assert_clean(1);
}

/// Rule 2. Protects the supply chain of the engine, an unofficial WhatsApp client that runs as
/// root: the tag says which version (one the hub was tested with), the digest says which bytes
/// (decision 0052).
#[test]
fn rule_02_engine_image_is_pinned_by_tag_and_digest() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(2);
}

/// Rule 3. Protects "nothing from the `.env` of Easypanel reaches the engine" and the principle
/// that a long-lived secret never passes through a person or through an environment the admin
/// fills: the engine gets its key only from the wrapper. Compose interpolates every `$` of the
/// file, not only those of `environment`, so the whole engine service is read.
#[test]
fn rule_03_engine_has_no_env_file_and_no_interpolation() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(3);
}

/// Rule 4. Protects the engine's authentication: the key is only the digest the hub writes, the
/// dashboard and the Swagger page never ask for a password they do not have, no screen is on, no QR
/// is printed in a log, and the engine sends no webhook of its own (decision 0052, security
/// principles 2 and 10).
#[test]
fn rule_04_engine_pins_thirteen_variables_and_no_key_switch() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(4);
}

/// Rule 5. Protects the principle "the hub only writes the digest; the engine can never write it":
/// the digest volume is read-only in the engine, only the hub and the engine mount it, the hub's
/// data volume (master key, audit mark, database password) is mounted by the hub alone, no
/// service takes another's volumes with `volumes_from`, and no volume has a name that two
/// installations could share.
#[test]
fn rule_05_digest_volume_is_read_only_in_the_engine_and_shared_with_nobody_else() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(5);
}

/// Rule 6. Protects the principle "expose the engine outside the internal network, never": the
/// engine reaches WhatsApp through one network and the hub through it; the database is on an
/// internal network only, which the engine does not share; no network has a fixed name that two
/// installations could share, and none is external.
#[test]
fn rule_06_networks_keep_the_engine_apart_from_the_database() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(6);
}

/// Rule 7. Protects the contract between the hub and the engine of the same composition (plan
/// section 1.1): the hub fills a missing engine only with these three values, and the folder in
/// which the hub writes the digest is the folder in which the engine reads it.
#[test]
fn rule_07_hub_variables_and_digest_folder_agree_with_the_mounts() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(7);
}

/// Rule 8. Protects security principles 2 and 11 and the rule of the wrapper (critique, security
/// correction 6): the engine never starts without the key digest the hub wrote, never with a
/// switch that lets calls in without a key, never with a key from the environment, and the
/// digest never reaches a log. The script is compared line by line with `WRAPPER_GOLDEN`: a line
/// added after the refusal (`set -a`, a variable of the dashboard) is as much a change of the
/// guard as a line removed.
#[test]
fn rule_08_wrapper_text_keeps_its_guards() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(8);
}

/// Rule 9. Protects the order of the start (decision 0052): the engine never starts before the hub
/// answers its health check, so the digest folder and the first digest are there; a restart of the
/// hub does not bounce the engine; the hub image answers the check, and its user owns the folder,
/// so it can write the digest.
#[test]
fn rule_09_engine_waits_for_a_healthy_hub_and_the_image_can_write_the_digest() {
    check_all(&recipe_text(), &dockerfile_text()).assert_clean(9);
}

/// Rule 10. Protects decision 0052: the "bring your own engine" recipes stay as they were. They
/// bring no engine and do not turn on the hub's engine installation (all three ARAUTO_ENGINE_*
/// variables set, or none: here, none, not even named with no value), so a hub of those recipes
/// never draws a key for an engine it does not own. The recipe, the env files it reads that are
/// in the repository, and the Dockerfile it builds the hub from are read; the admin's `.env` is
/// not (it is not in the repository).
#[test]
fn rule_10_bring_your_own_engine_recipes_bring_no_engine() {
    let mut report = Report::default();
    rule_10_recipes(&bring_your_own_recipes(), &mut report);
    rule_10_dockerfile(DOCKERFILE, &mut report);
    for folder in ["deploy/docker", "deploy/easypanel"] {
        refuse_implicit_files(10, folder, &repository_folder(folder), &mut report);
    }
    rule_10_env_files(
        &[
            (EASYPANEL_DEFAULTS_PATH, EASYPANEL_DEFAULTS),
            (DOCKER_ENV_EXAMPLE_PATH, DOCKER_ENV_EXAMPLE),
        ],
        &mut report,
    );
    report.assert_clean(10);
}

// ------------------------------------------------------------------- the rules bite (in memory)

/// An edit of a file's text.
type Edit = Box<dyn Fn(&str) -> String>;

/// A Dockerfile broken on purpose: what it is, a word the message holds, the edit.
type DockerfileBreak = (&'static str, &'static str, Edit);

/// One way to break the recipe: the rule that must say so, a word its message holds, and the edit.
struct Break {
    name: &'static str,
    rule: u8,
    needle: &'static str,
    edit: Edit,
}

/// `text` with `old` replaced by `new` once; the edit that does not apply is a mistake of the test.
fn swap(old: &'static str, new: &'static str) -> Edit {
    Box::new(move |text| text.replacen(old, new, 1))
}

/// `extra` (already indented, with its line break) as the first lines of the engine service.
fn engine_gets(extra: &'static str) -> Edit {
    Box::new(move |text| text.replacen("  engine:\n", &format!("  engine:\n{extra}"), 1))
}

fn breaks() -> Vec<Break> {
    let tail = "      - --\n      - /entrypoint.sh\n";
    let unset = "        unset WHATSAPP_API_KEY WAHA_API_KEY WAHA_API_KEY_PLAIN DEBUG\n";
    vec![
        // Rule 1
        Break {
            name: "a published port",
            rule: 1,
            needle: "ports",
            edit: engine_gets("    ports: [\"3000:3000\"]\n"),
        },
        Break {
            name: "a fixed container_name",
            rule: 1,
            needle: "container_name",
            edit: engine_gets("    container_name: arauto-engine\n"),
        },
        Break {
            name: "a command",
            rule: 1,
            needle: "command",
            edit: engine_gets("    command: [\"--anything\"]\n"),
        },
        Break {
            name: "a capability",
            rule: 1,
            needle: "cap_add",
            edit: engine_gets("    cap_add: [NET_ADMIN]\n"),
        },
        Break {
            name: "a device",
            rule: 1,
            needle: "devices",
            edit: engine_gets("    devices: [\"/dev/net/tun\"]\n"),
        },
        Break {
            name: "the host network",
            rule: 1,
            needle: "network_mode",
            edit: engine_gets("    network_mode: host\n"),
        },
        Break {
            name: "a merge key that adds a port",
            rule: 1,
            needle: "<<",
            edit: Box::new(|text| {
                let merged = text.replacen("  engine:\n", "  engine:\n    <<: *open\n", 1);
                format!("x-open: &open\n  ports: [\"3000:3000\"]\n{merged}")
            }),
        },
        Break {
            name: "a fourth service that looks like an engine",
            rule: 1,
            needle: "services.sidecar",
            edit: swap(
                "\nvolumes:\n  arauto-data:",
                "\n  sidecar:\n    image: devlikeapro/waha:latest\n    ports: [\"3000:3000\"]\n    environment:\n      WAHA_NO_API_KEY: \"1\"\n    volumes:\n      - arauto-engine-sessions:/app/.sessions\n    networks:\n      - engine\n      - internal\n\nvolumes:\n  arauto-data:",
            ),
        },
        Break {
            name: "a published port on the hub",
            rule: 1,
            needle: "services.hub.ports",
            edit: swap("  hub:\n", "  hub:\n    ports: [\"8444:8444\"]\n"),
        },
        Break {
            name: "the host network on postgres",
            rule: 1,
            needle: "services.postgres.network_mode",
            edit: swap(
                "    image: postgres:16\n",
                "    image: postgres:16\n    network_mode: host\n",
            ),
        },
        Break {
            name: "an engine image in the place of postgres",
            rule: 1,
            needle: "not a postgres image",
            edit: swap("image: postgres:16", "image: devlikeapro/waha:latest"),
        },
        Break {
            name: "a !reset tag on the entrypoint (Compose drops the wrapper)",
            rule: 1,
            needle: "tag",
            edit: swap(
                "    entrypoint:
",
                "    entrypoint: !reset
",
            ),
        },
        Break {
            name: "an !override tag on the entrypoint",
            rule: 1,
            needle: "!override",
            edit: swap(
                "    entrypoint:
",
                "    entrypoint: !override
",
            ),
        },
        Break {
            name: "a !reset tag on a pinned variable",
            rule: 1,
            needle: "!reset",
            edit: swap(
                "WAHA_DASHBOARD_ENABLED: \"false\"",
                "WAHA_DASHBOARD_ENABLED: !reset \"false\"",
            ),
        },
        Break {
            name: "a tag on a mapping (the environment of the engine)",
            rule: 1,
            needle: "a mapping with the tag",
            edit: swap(
                "      WHATSAPP_DEFAULT_ENGINE: \"GOWS\"
",
                "      WHATSAPP_DEFAULT_ENGINE: \"GOWS\"
      WAHA_X: !override {}
",
            ),
        },
        // Rule 2
        Break {
            name: "another repository, same digest",
            rule: 2,
            needle: "image",
            edit: swap(
                "devlikeapro/waha:gows-2026.10.1@",
                "example.invalid/other:1.0@",
            ),
        },
        Break {
            name: "the latest tag",
            rule: 2,
            needle: "image",
            edit: swap("waha:gows-2026.10.1@", "waha:latest@"),
        },
        Break {
            name: "a version that was not tested",
            rule: 2,
            needle: "TESTED",
            edit: swap("gows-2026.10.1@", "gows-2026.9.9@"),
        },
        Break {
            name: "no sha256 digest",
            rule: 2,
            needle: "digest",
            edit: swap("@sha256:", "@md5:"),
        },
        // Rule 3
        Break {
            name: "an env_file",
            rule: 3,
            needle: "env_file",
            edit: engine_gets("    env_file: [../../.env]\n"),
        },
        Break {
            name: "an interpolation in the script",
            rule: 3,
            needle: "not doubled",
            edit: swap("sleep 2", "sleep ${ARAUTO_ENGINE_WAIT:-2}"),
        },
        Break {
            name: "an interpolation in a variable",
            rule: 3,
            needle: "not doubled",
            edit: swap(
                "WAHA_LOG_LEVEL: \"info\"",
                "WAHA_LOG_LEVEL: \"${LOG_LEVEL}\"",
            ),
        },
        // Rule 4
        Break {
            name: "a key in the environment",
            rule: 4,
            needle: "WAHA_API_KEY",
            edit: swap(
                "      WAHA_PRINT_QR: \"false\"\n",
                "      WAHA_PRINT_QR: \"false\"\n      WAHA_API_KEY: \"x\"\n",
            ),
        },
        Break {
            name: "a missing pinned variable",
            rule: 4,
            needle: "WAHA_PRINT_QR",
            edit: swap("      WAHA_PRINT_QR: \"false\"\n", ""),
        },
        Break {
            name: "the dashboard turned on",
            rule: 4,
            needle: "WAHA_DASHBOARD_ENABLED",
            edit: swap(
                "WAHA_DASHBOARD_ENABLED: \"false\"",
                "WAHA_DASHBOARD_ENABLED: \"true\"",
            ),
        },
        Break {
            name: "a global webhook",
            rule: 4,
            needle: "WHATSAPP_HOOK_",
            edit: swap(
                "      WAHA_PRINT_QR: \"false\"\n",
                "      WAHA_PRINT_QR: \"false\"\n      WHATSAPP_HOOK_URL: \"http://x\"\n",
            ),
        },
        // Rule 5
        Break {
            name: "volumes_from the hub",
            rule: 5,
            needle: "volumes_from",
            edit: engine_gets("    volumes_from: [hub]\n"),
        },
        Break {
            name: "postgres mounting the data volume",
            rule: 5,
            needle: "not the hub",
            edit: swap(
                "      - arauto-postgres:/var/lib/postgresql/data\n",
                "      - arauto-postgres:/var/lib/postgresql/data\n      - arauto-data:/stolen\n",
            ),
        },
        Break {
            name: "the digest volume read-write in the engine",
            rule: 5,
            needle: "read-write",
            edit: swap("        read_only: true\n", ""),
        },
        Break {
            name: "a volume with a fixed name",
            rule: 5,
            needle: "fixed name",
            edit: swap(
                "  arauto-engine-digest:\n  arauto-engine-sessions:",
                "  arauto-engine-digest:\n    name: shared-digest\n  arauto-engine-sessions:",
            ),
        },
        Break {
            name: "an external volume",
            rule: 5,
            needle: "external",
            edit: swap(
                "  arauto-postgres:\n",
                "  arauto-postgres:\n    external: true\n",
            ),
        },
        Break {
            name: "the sessions mounted elsewhere than the store",
            rule: 5,
            needle: "WAHA_LOCAL_STORE_BASE_DIR",
            edit: swap("target: /app/.sessions", "target: /app/other"),
        },
        Break {
            name: "the digest volume mounted by postgres",
            rule: 5,
            needle: "exactly hub and engine",
            edit: swap(
                "      - arauto-postgres:/var/lib/postgresql/data\n",
                "      - arauto-postgres:/var/lib/postgresql/data\n      - arauto-engine-digest:/digest\n",
            ),
        },
        Break {
            name: "the Docker socket mounted in the engine (short form)",
            rule: 5,
            needle: "host path",
            edit: swap(
                "        target: /app/.sessions\n",
                "        target: /app/.sessions\n      - /var/run/docker.sock:/var/run/docker.sock\n",
            ),
        },
        Break {
            name: "the root of the host mounted in the engine (short form)",
            rule: 5,
            needle: "exactly two",
            edit: swap(
                "        target: /app/.sessions\n",
                "        target: /app/.sessions\n      - /:/host\n",
            ),
        },
        Break {
            name: "a bind of the folder of the data volume in the engine (long form)",
            rule: 5,
            needle: "only named volumes",
            edit: swap(
                "        target: /app/.sessions\n",
                "        target: /app/.sessions\n      - type: bind\n        source: /var/lib/docker/volumes/arauto-data/_data\n        target: /stolen\n",
            ),
        },
        Break {
            name: "the sessions volume mounted by the hub",
            rule: 5,
            needle: "not on the list",
            edit: swap(
                "      - arauto-data:/var/lib/arauto\n",
                "      - arauto-data:/var/lib/arauto\n      - arauto-engine-sessions:/stolen\n",
            ),
        },
        Break {
            name: "a volume that binds a path of the host (driver_opts)",
            rule: 5,
            needle: "driver_opts",
            edit: swap(
                "  arauto-engine-sessions:
",
                "  arauto-engine-sessions:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /var/lib/docker/volumes
",
            ),
        },
        Break {
            name: "a volume with a driver",
            rule: 5,
            needle: "volumes.arauto-data.driver",
            edit: swap(
                "  arauto-data:
",
                "  arauto-data:
    driver: local
",
            ),
        },
        // Rule 6
        Break {
            name: "a network with a fixed name",
            rule: 6,
            needle: "fixed name",
            edit: swap("  engine: {}", "  engine:\n    name: shared-engine"),
        },
        Break {
            name: "an external network",
            rule: 6,
            needle: "external",
            edit: swap("  engine: {}", "  engine:\n    external: true"),
        },
        Break {
            name: "the engine network as a macvlan on the LAN of the host",
            rule: 6,
            needle: "networks.engine.driver",
            edit: swap(
                "  engine: {}",
                "  engine:
    driver: macvlan
    driver_opts:
      parent: eth0",
            ),
        },
        Break {
            name: "driver_opts on the engine network",
            rule: 6,
            needle: "driver_opts",
            edit: swap(
                "  engine: {}",
                "  engine:
    driver: bridge
    driver_opts:
      parent: eth0",
            ),
        },
        Break {
            name: "ipv6 on the engine network",
            rule: 6,
            needle: "enable_ipv6",
            edit: swap(
                "  engine: {}",
                "  engine:
    enable_ipv6: true",
            ),
        },
        Break {
            name: "an ipam on the engine network",
            rule: 6,
            needle: "ipam",
            edit: swap(
                "  engine: {}",
                "  engine:
    ipam:
      config:
        - subnet: 192.0.2.0/24",
            ),
        },
        Break {
            name: "the engine on two networks",
            rule: 6,
            needle: "not exactly one",
            edit: swap(
                "    networks:\n      - engine\n    # Until",
                "    networks:\n      - engine\n      - internal\n    # Until",
            ),
        },
        Break {
            name: "postgres on the engine network",
            rule: 6,
            needle: "share",
            edit: swap(
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - internal\n",
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - internal\n      - engine\n",
            ),
        },
        Break {
            name: "the hub off the engine network",
            rule: 6,
            needle: "hub is not on",
            edit: swap(
                "      - internal\n      # The network the WhatsApp engine shares: it reaches http://hub:8444, never published.\n      - engine\n",
                "      - internal\n",
            ),
        },
        // Rule 7
        Break {
            name: "another engine address",
            rule: 7,
            needle: "ARAUTO_ENGINE_URL",
            edit: swap(
                "ARAUTO_ENGINE_URL: http://engine:3000",
                "ARAUTO_ENGINE_URL: http://engine:3001",
            ),
        },
        Break {
            name: "another digest folder",
            rule: 7,
            needle: "ARAUTO_ENGINE_DIGEST_DIR",
            edit: swap(
                "ARAUTO_ENGINE_DIGEST_DIR: /run/arauto-engine",
                "ARAUTO_ENGINE_DIGEST_DIR: /run/other",
            ),
        },
        // Rule 8
        Break {
            name: "lines after the refusal (set -a, the dashboard, no key)",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap(
                unset,
                "        unset WHATSAPP_API_KEY WAHA_API_KEY WAHA_API_KEY_PLAIN DEBUG\n        set -a\n        WAHA_DASHBOARD_ENABLED=true\n        WAHA_NO_API_KEY=1\n",
            ),
        },
        Break {
            name: "a changed length",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap("-ne 135", "-ne 136"),
        },
        Break {
            name: "a refusal that exits with another code",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap("          exit 78\n", "          exit 1\n"),
        },
        Break {
            name: "tracing",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap("        set -eu\n", "        set -eux\n"),
        },
        Break {
            name: "a no-break space before set -eu",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap("        set -eu\n", "        \u{a0}set -eu\n"),
        },
        Break {
            name: "no unset",
            rule: 8,
            needle: "wrapper is not the text",
            edit: swap(unset, ""),
        },
        Break {
            name: "an argument after the script",
            rule: 8,
            needle: "arguments after the script",
            edit: swap(
                tail,
                "      - --\n      - /entrypoint.sh\n      - --extra\n",
            ),
        },
        Break {
            name: "a script without the escape",
            rule: 8,
            needle: "not doubled",
            edit: swap("exec \"$$@\"", "exec \"$@\""),
        },
        // Rule 9
        Break {
            name: "the engine not waiting for a healthy hub",
            rule: 9,
            needle: "service_healthy",
            edit: swap(
                "condition: service_healthy\n    # The engine starts",
                "condition: service_started\n    # The engine starts",
            ),
        },
        Break {
            name: "a restart of the hub that bounces the engine",
            rule: 9,
            needle: "bounce",
            edit: swap(
                "condition: service_healthy\n    # The engine starts",
                "condition: service_healthy\n        restart: true\n    # The engine starts",
            ),
        },
        Break {
            name: "the engine that restarts always",
            rule: 9,
            needle: "unless-stopped",
            edit: swap(
                "    restart: unless-stopped\n    depends_on:\n      hub:",
                "    restart: always\n    depends_on:\n      hub:",
            ),
        },
    ]
    .into_iter()
    .chain(closed_set_breaks())
    .collect()
}

/// The line of the recipe with the command of the health check of the engine.
const ENGINE_HEALTHCHECK_LINE: &str = r#"      test: ["CMD", "node", "-e", "fetch('http://127.0.0.1:3000/ping').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"]"#;

/// The breaks of the closed sets: each is a form a review round found, or a form of the list of
/// forms a rule must refuse, that the rules did not see before.
fn closed_set_breaks() -> Vec<Break> {
    let engine_networks = "    networks:\n      - engine\n    # Until";
    let hub_networks = "      - internal\n      # The network the WhatsApp engine shares: it reaches http://hub:8444, never published.\n      - engine\n";
    let digest_mount = "      - type: volume\n        source: arauto-engine-digest\n        target: /run/arauto-engine\n        read_only: true\n        volume:\n          nocopy: true\n";
    let hub_condition = "condition: service_healthy\n    # The engine starts";
    let hub_dockerfile = "      dockerfile: deploy/docker/Dockerfile\n";
    vec![
        // Rule 1: YAML features, anywhere in the file.
        Break {
            name: "an anchor",
            rule: 1,
            needle: "anchor",
            edit: swap(
                "WAHA_LOG_LEVEL: \"info\"",
                "WAHA_LOG_LEVEL: &level \"info\"",
            ),
        },
        Break {
            name: "an alias that repeats a pinned value",
            rule: 1,
            needle: "an alias",
            edit: Box::new(|text| {
                text.replacen(
                    "WAHA_LOG_LEVEL: \"info\"",
                    "WAHA_LOG_LEVEL: &level \"info\"",
                    1,
                )
                .replacen(
                    "WAHA_HTTP_LOG_LEVEL: \"debug\"",
                    "WAHA_HTTP_LOG_LEVEL: *level",
                    1,
                )
            }),
        },
        Break {
            name: "a merge key in quotes",
            rule: 1,
            needle: "merge key",
            edit: Box::new(|text| {
                let merged = text.replacen("  engine:\n", "  engine:\n    \"<<\": *open\n", 1);
                format!("x-open: &open\n  ports: [\"3000:3000\"]\n{merged}")
            }),
        },
        Break {
            name: "a second document",
            rule: 1,
            needle: "documents",
            edit: Box::new(|text| {
                format!("{text}---\nservices:\n  engine:\n    ports: [\"3000:3000\"]\n")
            }),
        },
        Break {
            name: "a key that is not a scalar",
            rule: 1,
            needle: "not a scalar",
            edit: engine_gets("    ? [ports]\n    : [\"3000:3000\"]\n"),
        },
        Break {
            name: "an include at the top",
            rule: 1,
            needle: "compose.engine.yaml include",
            edit: Box::new(|text| format!("include:\n  - other.yaml\n{text}")),
        },
        Break {
            name: "a fixed project name",
            rule: 1,
            needle: "compose.engine.yaml name",
            edit: Box::new(|text| format!("name: shared\n{text}")),
        },
        Break {
            name: "an extension field at the top",
            rule: 1,
            needle: "x-note",
            edit: Box::new(|text| format!("x-note: hello\n{text}")),
        },
        Break {
            name: "labels a proxy reads, on the engine",
            rule: 1,
            needle: "services.engine.labels",
            edit: engine_gets("    labels:\n      traefik.enable: \"true\"\n"),
        },
        Break {
            name: "an exposed port on the engine",
            rule: 1,
            needle: "services.engine.expose",
            edit: engine_gets("    expose: [\"3000\"]\n"),
        },
        Break {
            name: "extra hosts on the engine",
            rule: 1,
            needle: "services.engine.extra_hosts",
            edit: engine_gets("    extra_hosts: [\"postgres:192.0.2.1\"]\n"),
        },
        Break {
            name: "a privileged engine",
            rule: 1,
            needle: "services.engine.privileged",
            edit: engine_gets("    privileged: true\n"),
        },
        Break {
            name: "the pid namespace of the host",
            rule: 1,
            needle: "services.engine.pid",
            edit: engine_gets("    pid: host\n"),
        },
        Break {
            name: "the Docker socket through use_api_socket",
            rule: 1,
            needle: "services.engine.use_api_socket",
            edit: engine_gets("    use_api_socket: true\n"),
        },
        Break {
            name: "the hub extending a service of another file",
            rule: 1,
            needle: "services.hub.extends",
            edit: swap(
                "  hub:\n",
                "  hub:\n    extends:\n      file: other.yaml\n      service: hub\n",
            ),
        },
        Break {
            name: "a hostname on postgres",
            rule: 1,
            needle: "services.postgres.hostname",
            edit: swap(
                "    image: postgres:16\n",
                "    image: postgres:16\n    hostname: engine\n",
            ),
        },
        Break {
            name: "a registry host named postgres in the place of postgres",
            rule: 1,
            needle: "not a postgres image",
            edit: swap(
                "image: postgres:16",
                "image: postgres:5000/acme/whatsapp-engine:1",
            ),
        },
        Break {
            name: "a stop_grace_period of the engine that is not a duration",
            rule: 1,
            needle: "services.engine.stop_grace_period",
            edit: swap(
                "    stop_grace_period: 30s\n",
                "    stop_grace_period: [30s]\n",
            ),
        },
        // Rule 8: the health check runs in the engine container while the wrapper waits.
        Break {
            name: "a health check that starts the engine a second way",
            rule: 8,
            needle: "services.engine.healthcheck.test",
            edit: swap(
                ENGINE_HEALTHCHECK_LINE,
                r#"      test: ["CMD-SHELL", "WAHA_NO_API_KEY=True setsid /usr/bin/tini -s -- /entrypoint.sh >/dev/null 2>&1 < /dev/null & exit 0"]"#,
            ),
        },
        Break {
            name: "a health check written as one string",
            rule: 8,
            needle: "services.engine.healthcheck.test",
            edit: swap(ENGINE_HEALTHCHECK_LINE, "      test: /entrypoint.sh"),
        },
        Break {
            name: "a health check turned off",
            rule: 8,
            needle: "not exactly",
            edit: swap(
                "      start_period: 60s\n",
                "      start_period: 60s\n      disable: true\n",
            ),
        },
        Break {
            name: "retries of the health check as text",
            rule: 8,
            needle: "healthcheck.retries",
            edit: swap("      retries: 3\n", "      retries: \"3\"\n"),
        },
        Break {
            name: "no health check on the engine",
            rule: 8,
            needle: "no health check",
            edit: Box::new(|text| {
                let start = text.find("    healthcheck:\n      test: [\"CMD\", \"node\"");
                let end = text.find("    stop_grace_period: 30s\n");
                match (start, end) {
                    (Some(start), Some(end)) if start < end => {
                        format!("{}{}", &text[..start], &text[end..])
                    }
                    _ => text.to_owned(),
                }
            }),
        },
        // Rule 2
        Break {
            name: "other bytes under the tested tag",
            rule: 2,
            needle: "tested image reference",
            edit: swap("@sha256:1f7faeb0", "@sha256:00000000"),
        },
        // Rule 3: interpolation anywhere in the file.
        Break {
            name: "an interpolation in the hub",
            rule: 3,
            needle: "services.hub.environment.ARAUTO_LISTEN_PANEL",
            edit: swap(
                "ARAUTO_LISTEN_PANEL: 0.0.0.0:8445",
                "ARAUTO_LISTEN_PANEL: ${PANEL:-0.0.0.0:8445}",
            ),
        },
        Break {
            name: "an interpolation in a network of postgres",
            rule: 3,
            needle: "services.postgres.networks",
            edit: swap(
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - internal\n",
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - ${NETWORK:-internal}\n",
            ),
        },
        // Rule 4
        Break {
            name: "a pinned variable as a plain number (octal to Compose's parser)",
            rule: 4,
            needle: "not quoted text",
            edit: swap("WHATSAPP_API_PORT: \"3000\"", "WHATSAPP_API_PORT: 03000"),
        },
        Break {
            name: "a key switch named with no value",
            rule: 4,
            needle: "services.engine.environment.WAHA_NO_API_KEY",
            edit: swap(
                "      WAHA_PRINT_QR: \"false\"\n",
                "      WAHA_PRINT_QR: \"false\"\n      WAHA_NO_API_KEY:\n",
            ),
        },
        // Rule 5: the keys and options of a mount.
        Break {
            name: "a subpath of the digest volume",
            rule: 5,
            needle: "subpath",
            edit: swap(
                "          nocopy: true\n",
                "          nocopy: true\n          subpath: x\n",
            ),
        },
        Break {
            name: "bind options on the sessions mount",
            rule: 5,
            needle: "`bind`",
            edit: swap(
                "        target: /app/.sessions\n",
                "        target: /app/.sessions\n        bind:\n          propagation: shared\n",
            ),
        },
        Break {
            name: "the digest mount in the short form, ro and rw",
            rule: 5,
            needle: "both ro and rw",
            edit: swap(
                digest_mount,
                "      - arauto-engine-digest:/run/arauto-engine:ro,rw\n",
            ),
        },
        Break {
            name: "read_only as text",
            rule: 5,
            needle: "read_only is not a boolean",
            edit: swap("        read_only: true\n", "        read_only: \"true\"\n"),
        },
        // Rule 6: a service joins a network with nothing more.
        Break {
            name: "the engine taking the names postgres and hub",
            rule: 6,
            needle: "aliases",
            edit: swap(
                engine_networks,
                "    networks:\n      engine:\n        aliases: [postgres, hub]\n    # Until",
            ),
        },
        Break {
            name: "a fixed address for the hub",
            rule: 6,
            needle: "ipv4_address",
            edit: swap(
                hub_networks,
                "      internal:\n        ipv4_address: 192.0.2.10\n      engine:\n",
            ),
        },
        // Rule 7
        Break {
            name: "the engine address named with no value",
            rule: 7,
            needle: "services.hub.environment.ARAUTO_ENGINE_URL",
            edit: swap(
                "ARAUTO_ENGINE_URL: http://engine:3000",
                "ARAUTO_ENGINE_URL:",
            ),
        },
        // Rule 9
        Break {
            name: "a dependency on the hub that is not required",
            rule: 9,
            needle: "depends_on.hub.required",
            edit: swap(
                hub_condition,
                "condition: service_healthy\n        required: false\n    # The engine starts",
            ),
        },
        Break {
            name: "the list form of depends_on",
            rule: 9,
            needle: "not a mapping",
            edit: swap(
                "    depends_on:\n      hub:\n        condition: service_healthy\n    # The engine",
                "    depends_on: [hub]\n    # The engine",
            ),
        },
        Break {
            name: "the hub built from another Dockerfile",
            rule: 9,
            needle: "Dockerfile this test reads",
            edit: swap(
                hub_dockerfile,
                "      dockerfile: deploy/docker/Other.Dockerfile\n",
            ),
        },
        Break {
            name: "the hub built from an inline Dockerfile",
            rule: 9,
            needle: "build.dockerfile_inline",
            edit: swap(
                hub_dockerfile,
                "      dockerfile: deploy/docker/Dockerfile\n      dockerfile_inline: \"FROM scratch\"\n",
            ),
        },
    ]
}

/// A Dockerfile broken on purpose: the rule is always 9.
fn dockerfile_breaks() -> Vec<DockerfileBreak> {
    vec![
        (
            "the HEALTHCHECK turned off",
            "NONE",
            swap(
                "HEALTHCHECK --interval=15s",
                "HEALTHCHECK NONE #--interval=15s",
            ),
        ),
        (
            "the HEALTHCHECK turned off, in lower case",
            "NONE",
            swap(
                "HEALTHCHECK --interval=15s",
                "healthcheck none #--interval=15s",
            ),
        ),
        (
            "a user of the last stage in lower case",
            "root",
            swap(
                "USER arauto
",
                "USER arauto
user root
",
            ),
        ),
        (
            "no HEALTHCHECK",
            "no HEALTHCHECK",
            swap("HEALTHCHECK ", "# HEALTHCHECK "),
        ),
        (
            "a folder the hub user does not own",
            "chown",
            swap(
                " && chown arauto:arauto /var/lib/arauto /run/arauto-engine",
                "",
            ),
        ),
        (
            "no digest folder",
            "mkdir",
            swap(
                "/usr/share/arauto/download /run/arauto-engine",
                "/usr/share/arauto/download",
            ),
        ),
        (
            "the folder made and given only in a builder stage",
            "mkdir",
            Box::new(|text| {
                text.replacen(
                    "/usr/share/arauto/download /run/arauto-engine",
                    "/usr/share/arauto/download",
                    1,
                )
                .replacen(
                    " && chown arauto:arauto /var/lib/arauto /run/arauto-engine",
                    " && chown arauto:arauto /var/lib/arauto",
                    1,
                )
                .replacen(
                    " && mkdir -p /out/download",
                    " && mkdir -p /out/download /run/arauto-engine && chown arauto:arauto /run/arauto-engine",
                    1,
                )
            }),
        ),
        (
            "the folder made and given only in a comment of the last stage",
            "mkdir",
            Box::new(|text| {
                text.replacen(
                    "/usr/share/arauto/download /run/arauto-engine",
                    "/usr/share/arauto/download",
                    1,
                )
                .replacen(
                    " && chown arauto:arauto /var/lib/arauto /run/arauto-engine",
                    " && chown arauto:arauto /var/lib/arauto",
                    1,
                )
                .replacen(
                    " && chmod 700 /var/lib/arauto \\\n",
                    " && chmod 700 /var/lib/arauto \\\n# && mkdir -p /run/arauto-engine && chown arauto:arauto /run/arauto-engine \\\n",
                    1,
                )
            }),
        ),
        (
            "an escape directive",
            "directive",
            Box::new(|text| format!("# escape=`\n{text}")),
        ),
    ]
}

#[allow(clippy::panic)]
fn mutated(name: &str, edit: &dyn Fn(&str) -> String, original: &str) -> String {
    let changed = edit(original);
    if changed == original {
        panic!("THE TEST IS WRONG: the break `{name}` did not apply to the file");
    }
    changed
}

/// "Fails when the rule is removed": each break below is applied to the recipe text, parsed again
/// and checked; the rule that guards it must say so, and say it by the place it names.
#[test]
#[allow(clippy::panic)]
fn each_rule_bites_when_the_recipe_is_broken() {
    let mut silent: Vec<String> = Vec::new();
    for case in breaks() {
        let text = mutated(case.name, &*case.edit, &recipe_text());
        let report = check_all(&text, &dockerfile_text());
        let said = report
            .of_rule(case.rule)
            .iter()
            .any(|v| v.detail.contains(case.needle) || v.place.contains(case.needle));
        if !said {
            silent.push(format!(
                "`{}`: rule {} did not say `{}`; it said {:?}",
                case.name, case.rule, case.needle, report.found
            ));
        }
    }
    for (name, needle, edit) in dockerfile_breaks() {
        let text = mutated(name, &*edit, &dockerfile_text());
        let report = check_all(&recipe_text(), &text);
        let said = report
            .of_rule(9)
            .iter()
            .any(|v| v.detail.contains(needle) || v.place.contains(needle));
        if !said {
            silent.push(format!(
                "Dockerfile `{name}`: rule 9 did not say `{needle}`; it said {:?}",
                report.found
            ));
        }
    }
    if !silent.is_empty() {
        panic!(
            "{} break(s) went unseen:\n  - {}",
            silent.len(),
            silent.join("\n  - ")
        );
    }
}

/// A comment, a blank line and another indentation in the script are not a change of the guard.
#[test]
fn a_comment_or_a_blank_line_in_the_wrapper_is_not_a_violation() {
    let text = recipe_text().replacen(
        "        set -eu\n",
        "        # a note\n\n        set -eu\n          # another\n",
        1,
    );
    assert_ne!(text, recipe_text(), "the edit did not apply");
    let report = check_all(&text, &dockerfile_text());
    report.assert_clean(8);
}

/// Rule 10 on one recipe text, as if it were at `path`, with the env files it may name.
fn rule_10_on(path: &str, text: &str, env_files: &[&str]) -> Report {
    let mut report = Report::default();
    rule_10_recipes(&[(path, text, env_files)], &mut report);
    report
}

/// A bring-your-own recipe broken on purpose: what it is, which recipe, a word the message of rule
/// 10 holds (in its detail or its place), and the edit.
type OwnBreak = (&'static str, &'static str, &'static str, Edit);

/// The breaks of rule 10, each applied to the real text of a bring-your-own recipe.
fn bring_your_own_breaks() -> Vec<OwnBreak> {
    let easypanel = EASYPANEL_OWN_ENGINE_PATH;
    let docker = DOCKER_OWN_ENGINE_PATH;
    let easypanel_env = "      ARAUTO_CLIENT_IP_SOURCE: forwarded\n";
    let docker_env = "      RUST_LOG: ${RUST_LOG:-info}\n";
    vec![
        (
            "the merge key of the review: three variables behind `<<: *engine`",
            easypanel,
            "merge key",
            Box::new(move |text| {
                let merged = text.replacen(
                    easypanel_env,
                    "      ARAUTO_CLIENT_IP_SOURCE: forwarded\n      <<: *engine\n",
                    1,
                );
                format!(
                    "x-engine: &engine {{ARAUTO_ENGINE_URL: http://engine:3000, ARAUTO_ENGINE_EVENTS_URL: http://hub:8444, ARAUTO_ENGINE_DIGEST_DIR: /run/arauto-engine}}\n{merged}"
                )
            }),
        ),
        (
            "an anchor alone",
            docker,
            "anchor",
            swap("    image: postgres:16\n", "    image: &db postgres:16\n"),
        ),
        (
            "a tag",
            easypanel,
            "tag",
            swap("    environment:\n", "    environment: !override\n"),
        ),
        (
            "a second document with an engine",
            docker,
            "documents",
            Box::new(|text| {
                format!("{text}---\nservices:\n  engine:\n    image: example.invalid/x:1\n")
            }),
        ),
        (
            "ARAUTO_ENGINE_URL named with no value (map form)",
            easypanel,
            "services.hub.environment.ARAUTO_ENGINE_URL",
            swap(
                easypanel_env,
                "      ARAUTO_CLIENT_IP_SOURCE: forwarded\n      ARAUTO_ENGINE_URL:\n",
            ),
        ),
        (
            "ARAUTO_ENGINE_URL interpolated from the .env (map form)",
            docker,
            "services.hub.environment.ARAUTO_ENGINE_URL",
            swap(
                docker_env,
                "      RUST_LOG: ${RUST_LOG:-info}\n      ARAUTO_ENGINE_URL: ${ARAUTO_ENGINE_URL:-}\n",
            ),
        ),
        (
            "an env file this test does not read",
            easypanel,
            "engine.env",
            swap(
                "      - path: ../../.env\n",
                "      - path: ../../.env\n      - path: engine.env\n",
            ),
        ),
        (
            "an env file in the recipe for Docker",
            docker,
            "env_file",
            swap(
                "    restart: unless-stopped\n    depends_on:\n      postgres:",
                "    restart: unless-stopped\n    env_file: arauto.env\n    depends_on:\n      postgres:",
            ),
        ),
        (
            "an env file entry with another format",
            easypanel,
            "format",
            swap(
                "      - path: ../../.env\n        required: false\n",
                "      - path: ../../.env\n        required: false\n        format: raw\n",
            ),
        ),
        (
            "the hub extending a service of another file",
            docker,
            "services.hub.extends",
            swap(
                "  hub:\n",
                "  hub:\n    extends:\n      file: ../easypanel/compose.engine.yaml\n      service: hub\n",
            ),
        ),
        (
            "an include of the recipe that brings the engine",
            easypanel,
            "include",
            Box::new(|text| format!("include:\n  - compose.engine.yaml\n{text}")),
        ),
        (
            "an extension field at the top",
            docker,
            "x-engine",
            Box::new(|text| format!("x-engine: none\n{text}")),
        ),
        (
            "the hub built from another Dockerfile",
            easypanel,
            "Dockerfile this test reads",
            swap(
                "      dockerfile: deploy/docker/Dockerfile\n",
                "      dockerfile: deploy/docker/Engine.Dockerfile\n",
            ),
        ),
        (
            "build arguments for the hub",
            docker,
            "build.args",
            swap(
                "      dockerfile: deploy/docker/Dockerfile\n",
                "      dockerfile: deploy/docker/Dockerfile\n      args:\n        ARAUTO_ENGINE_URL: http://engine:3000\n",
            ),
        ),
        (
            "an engine of another author in the place of postgres",
            docker,
            "services.postgres.image",
            swap(
                "    image: postgres:16\n",
                "    image: example.invalid/engine:1\n",
            ),
        ),
        (
            "an image of another author for the hub",
            easypanel,
            "services.hub.image",
            swap(
                "  hub:\n    build:\n",
                "  hub:\n    image: example.invalid/engine:1\n    build:\n",
            ),
        ),
        (
            "a registry host named postgres in the place of postgres",
            easypanel,
            "services.postgres.image",
            swap(
                "    image: postgres:16\n",
                "    image: postgres:16/acme/anything:1\n",
            ),
        ),
        (
            "postgres on the shared network of the engine",
            docker,
            "services.postgres.networks",
            swap(
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - internal\n",
                "      - arauto-postgres:/var/lib/postgresql/data\n    networks:\n      - internal\n      - engine\n",
            ),
        ),
        (
            "the internal network of postgres with a fixed name",
            easypanel,
            "services.postgres.networks",
            swap(
                "  internal:\n    internal: true\n",
                "  internal:\n    internal: true\n    name: arauto-engine\n",
            ),
        ),
        (
            "an alias postgres for the hub on the shared network of the engine",
            docker,
            "aliases",
            swap(
                "      - internal\n      - outside\n      # The network the WhatsApp engine shares: it reaches http://hub:8444, never published.\n      - engine\n",
                "      internal:\n      outside:\n      engine:\n        aliases: [postgres]\n",
            ),
        ),
        (
            "an engine under the name of a profile-only service",
            docker,
            "services.whatsapp",
            swap(
                "\nvolumes:\n",
                "\n  whatsapp:\n    image: example.invalid/x:1\n    profiles: [engine]\n\nvolumes:\n",
            ),
        ),
    ]
}

/// The breaks of rule 10 on a bring-your-own recipe, its env files and the Dockerfile, each seen
/// by rule 10 and by the place it names.
#[test]
#[allow(clippy::panic)]
fn rule_10_bites() {
    let mut silent: Vec<String> = Vec::new();
    let mut said = |name: &str, report: &Report, needle: &str| {
        let seen = report
            .of_rule(10)
            .iter()
            .any(|v| v.detail.contains(needle) || v.place.contains(needle));
        if !seen {
            silent.push(format!(
                "`{name}`: rule 10 did not say `{needle}`; it said {:?}",
                report.found
            ));
        }
    };

    for (name, path, needle, edit) in bring_your_own_breaks() {
        let (_, original, env_files) = bring_your_own_recipes()
            .into_iter()
            .find(|(file, _, _)| *file == path)
            .unwrap_or_else(|| broken("the recipe of the break", path, name));
        let text = mutated(name, &*edit, original);
        said(name, &rule_10_on(path, &text, env_files), needle);
    }

    // Synthetic recipes: the forms of the list form of `environment`, and an engine.
    for (name, text, needle) in [
        (
            "an engine service",
            "services:\n  engine:\n    image: example.invalid/x:1\n",
            "engine service",
        ),
        (
            "an engine image on the hub",
            "services:\n  hub:\n    image: devlikeapro/waha:gows-2026.10.1\n",
            "looks like the WhatsApp engine",
        ),
        (
            "a service under another name",
            "services:\n  whatsapp:\n    image: example.invalid/x:1\n",
            "services.whatsapp",
        ),
        (
            "ARAUTO_ENGINE_URL set (map form)",
            "services:\n  postgres:\n    image: postgres:16\n    environment:\n      ARAUTO_ENGINE_URL: http://engine:3000\n",
            "ARAUTO_ENGINE_URL",
        ),
        (
            "a bare ARAUTO_ENGINE_URL (list form)",
            "services:\n  postgres:\n    image: postgres:16\n    environment:\n      - ARAUTO_ENGINE_URL\n",
            "ARAUTO_ENGINE_URL",
        ),
        (
            "an empty ARAUTO_ENGINE_DIGEST_DIR (list form)",
            "services:\n  postgres:\n    image: postgres:16\n    environment:\n      - ARAUTO_ENGINE_DIGEST_DIR=\n",
            "ARAUTO_ENGINE_DIGEST_DIR",
        ),
        (
            "a name interpolated in the list form",
            "services:\n  postgres:\n    image: postgres:16\n    environment:\n      - ${NAME}=http://engine:3000\n",
            "plain variable name",
        ),
        (
            "a name twice in the list form",
            "services:\n  postgres:\n    image: postgres:16\n    environment:\n      - A=1\n      - A=2\n",
            "twice",
        ),
    ] {
        said(name, &rule_10_on("synthetic.yaml", text, &[]), needle);
    }

    // The env files those recipes read: a name is refused with any value, an empty one or none.
    for (name, text) in [
        (
            "a set ARAUTO_ENGINE_DIGEST_DIR",
            "# note\nARAUTO_ENGINE_DIGEST_DIR=/run/x\n",
        ),
        (
            "an exported ARAUTO_ENGINE_URL",
            "export ARAUTO_ENGINE_URL=http://engine:3000\n",
        ),
        ("an empty ARAUTO_ENGINE_URL", "ARAUTO_ENGINE_URL=\n"),
        ("a bare ARAUTO_ENGINE_URL", "ARAUTO_ENGINE_URL\n"),
        (
            "ARAUTO_ENGINE_URL in the YAML-like form",
            "ARAUTO_ENGINE_URL: http://engine:3000\n",
        ),
        (
            "an ARAUTO_ENGINE_URL exported after a tab",
            "export\tARAUTO_ENGINE_URL=http://engine:3000\n",
        ),
        (
            "a byte order mark before ARAUTO_ENGINE_URL",
            "\u{feff}ARAUTO_ENGINE_URL=http://engine:3000\n",
        ),
    ] {
        let mut report = Report::default();
        rule_10_env_files(&[("synthetic.env", text)], &mut report);
        said(name, &report, "ARAUTO_ENGINE");
    }
    for (name, text, needle) in [
        (
            "COMPOSE_FILE in an env file",
            "COMPOSE_FILE=compose.yaml:../easypanel/compose.engine.yaml\n",
            "COMPOSE_FILE",
        ),
        (
            "COMPOSE_FILE exported after a tab",
            "export\tCOMPOSE_FILE=compose.yaml:../easypanel/compose.engine.yaml\n",
            "COMPOSE_FILE",
        ),
        (
            "an exported name that is not an engine variable",
            "export RUST_LOG=info\n",
            "plain NAME=value",
        ),
        (
            "a quoted value, which can carry over lines",
            "RUST_LOG=\"info\n",
            "plain NAME=value",
        ),
        (
            "a value with a blank and an inline comment",
            "RUST_LOG=info # note\n",
            "plain NAME=value",
        ),
    ] {
        let mut report = Report::default();
        rule_10_env_files(&[("synthetic.env", text)], &mut report);
        said(name, &report, needle);
    }

    // An override file beside compose.yaml, which Compose merges by itself.
    let folder = tempfile::tempdir()
        .unwrap_or_else(|error| broken("a temporary folder", "this test", &error.to_string()));
    std::fs::write(
        folder.path().join("compose.override.yaml"),
        "services:\n  engine:\n    image: example.invalid/x:1\n",
    )
    .unwrap_or_else(|error| broken("a file is written", "this test", &error.to_string()));
    let mut override_file = Report::default();
    refuse_implicit_files(10, "synthetic", folder.path(), &mut override_file);
    said(
        "an override file beside compose.yaml",
        &override_file,
        "compose.override.yaml",
    );

    // The Dockerfile every recipe builds the hub from.
    for (name, edit) in [
        (
            "an ENV of the engine address in the Dockerfile",
            swap(
                "ENV ARAUTO_DATA_DIR=/var/lib/arauto \\\n",
                "ENV ARAUTO_DATA_DIR=/var/lib/arauto \\\n    ARAUTO_ENGINE_URL=http://engine:3000 \\\n",
            ),
        ),
        (
            "an ARG of the engine address in the Dockerfile",
            swap(
                "USER arauto\n",
                "ARG ARAUTO_ENGINE_URL=http://engine:3000\nUSER arauto\n",
            ),
        ),
        (
            "the engine address split over a continued line",
            swap(
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444\n",
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444 \\\n    ARAUTO_\\\nENGINE_URL=http://engine:3000\n",
            ),
        ),
        (
            "the engine address split around a comment line",
            swap(
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444\n",
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444 \\\n    ARAUTO_\\\n# a note\nENGINE_URL=http://engine:3000\n",
            ),
        ),
        (
            "the engine address split with a blank before the continuation",
            swap(
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444\n",
                "    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444 \\\n    ARAUTO_ \\\n    ENGINE_URL=http://engine:3000\n",
            ),
        ),
        (
            "the engine address in lower-case ENV",
            swap(
                "USER arauto\n",
                "env ARAUTO_ENGINE_URL http://engine:3000\nUSER arauto\n",
            ),
        ),
    ] {
        let text = mutated(name, &*edit, &dockerfile_text());
        let mut report = Report::default();
        rule_10_dockerfile(&text, &mut report);
        said(name, &report, "ARAUTO_ENGINE");
    }
    for (name, needle, edit) in [
        (
            "an ARAUTO_ name off the allowlist",
            "ARAUTO_PANEL_URL",
            swap(
                "USER arauto\n",
                "ARG ARAUTO_PANEL_URL=http://localhost:8445\nUSER arauto\n",
            ),
        ),
        (
            "an ENV name built from a variable",
            "built from a variable",
            swap("USER arauto\n", "ARG N=X\nENV ${N}=1\nUSER arauto\n"),
        ),
        (
            "an escape directive, after which `\\` no longer continues a line",
            "directive",
            Box::new(|text: &str| format!("# escape=`\n{text}")) as Edit,
        ),
        (
            "another base image for the last stage",
            "FROM",
            swap(
                "FROM debian:bookworm-slim\n",
                "FROM example.invalid/base:1\n",
            ),
        ),
    ] {
        let text = mutated(name, &*edit, &dockerfile_text());
        let mut report = Report::default();
        rule_10_dockerfile(&text, &mut report);
        said(name, &report, needle);
    }

    assert!(
        silent.is_empty(),
        "{} break(s) of rule 10 went unseen:\n  - {}",
        silent.len(),
        silent.join("\n  - ")
    );
}

/// What the closed sets must still let through: a network entry with nothing more, the short form
/// of a read-only mount, and a variable with a value in the list form.
#[test]
fn the_closed_sets_take_the_plain_forms() {
    let text = recipe_text()
        .replacen(
            "    networks:\n      - engine\n    # Until",
            "    networks:\n      engine:\n    # Until",
            1,
        )
        .replacen(
            "      - internal\n      # The network the WhatsApp engine shares: it reaches http://hub:8444, never published.\n      - engine\n",
            "      internal: {}\n      engine:\n",
            1,
        )
        .replacen(
            "      - type: volume\n        source: arauto-engine-digest\n        target: /run/arauto-engine\n        read_only: true\n        volume:\n          nocopy: true\n",
            "      - arauto-engine-digest:/run/arauto-engine:ro,nocopy\n",
            1,
        );
    assert_ne!(text, recipe_text(), "the edits did not apply");
    let report = check_all(&text, &dockerfile_text());
    assert!(
        report.found.is_empty(),
        "a plain form was refused: {:?}",
        report.found
    );

    let service = parse("environment:\n  - A=1\n  - B\n  - C=\n")
        .unwrap_or_else(|detail| broken("the synthetic service", "this test", &detail));
    let found = environment(&service)
        .unwrap_or_else(|detail| broken("the list form is read", "this test", &detail));
    assert_eq!(found.get("A"), Some(&Value::Text("1".to_owned())));
    assert_eq!(found.get("B"), Some(&Value::Unset));
    assert_eq!(found.get("C"), Some(&Value::Text(String::new())));
}

// ---------------------------------------------------------- the wrapper, run (Unix only)

/// The wrapper run under `/bin/sh` on a file of a temporary folder, with `/usr/bin/env` in place of
/// the engine. This does not run on Windows: there is no `/bin/sh`.
#[cfg(unix)]
mod wrapper_behaviour {
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{broken, engine_recipe, wrapper};

    const RECIPE_DIGEST_PATH: &str = "/run/arauto-engine/api-key.sha512";

    struct Outcome {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    }

    fn digest() -> String {
        format!("sha512:{}", "ab".repeat(64))
    }

    /// The script as the shell reads it, with the digest file at `file`.
    fn script_for(file: &Path) -> String {
        let (_, raw) = wrapper(&engine_recipe())
            .unwrap_or_else(|detail| broken("the wrapper is readable", "the recipe", &detail));
        let script = raw.replace("$$", "$");
        let changed = script.replace(RECIPE_DIGEST_PATH, &file.display().to_string());
        if changed == script {
            broken(
                "the digest path is in the script",
                "the recipe",
                RECIPE_DIGEST_PATH,
            );
        }
        changed
    }

    fn spawn(script: &str, variables: &[(&str, &str)]) -> Child {
        Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .arg("arauto-engine")
            .arg("/usr/bin/env")
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/local/bin")
            .envs(variables.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| broken("/bin/sh starts", "the test", &error.to_string()))
    }

    /// Waits up to `within` for the end; a shell still running at the deadline is killed and has no
    /// exit code.
    fn finish(mut child: Child, within: Duration) -> Outcome {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let finished = matches!(child.try_wait(), Ok(Some(_)));
        if !finished {
            let _ = child.kill();
        }
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| broken("the output is read", "the test", &error.to_string()));
        Outcome {
            code: if finished { output.status.code() } else { None },
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn folder() -> tempfile::TempDir {
        tempfile::tempdir()
            .unwrap_or_else(|error| broken("a temporary folder", "the test", &error.to_string()))
    }

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes)
            .unwrap_or_else(|error| broken("a file is written", "the test", &error.to_string()));
    }

    enum Present {
        Missing,
        Bytes(Vec<u8>),
        DanglingLink,
        LinkToValid,
        Directory,
    }

    type Case = (&'static str, Vec<(&'static str, &'static str)>, Present);

    fn prepare(dir: &Path, present: &Present) -> PathBuf {
        let file = dir.join("api-key.sha512");
        match present {
            Present::Missing => {}
            Present::Bytes(bytes) => write(&file, bytes),
            Present::DanglingLink => {
                symlink(dir.join("nothing"), &file).unwrap_or_else(|error| {
                    broken("a link is made", "the test", &error.to_string());
                });
            }
            Present::LinkToValid => {
                let target = dir.join("target");
                write(&target, digest().as_bytes());
                symlink(&target, &file).unwrap_or_else(|error| {
                    broken("a link is made", "the test", &error.to_string());
                });
            }
            Present::Directory => std::fs::create_dir(&file).unwrap_or_else(|error| {
                broken("a folder is made", "the test", &error.to_string());
            }),
        }
        file
    }

    fn has_line(stdout: &str, wanted: &str) -> bool {
        stdout.lines().any(|line| line == wanted)
    }

    fn starts(stdout: &str, prefix: &str) -> usize {
        stdout
            .lines()
            .filter(|line| line.starts_with(prefix))
            .count()
    }

    /// Every way of being wrong, and the two switches, end with exit 78 and run nothing.
    #[test]
    fn wrapper_refuses_with_exit_78_and_runs_nothing() {
        let valid = digest().into_bytes();
        let one_short = valid[..valid.len() - 1].to_vec();
        let mut one_over = valid.clone();
        one_over.push(b'\n');
        let upper = format!("sha512:{}", "AB".repeat(64)).into_bytes();
        let wrong_prefix = format!("sha256:{}", "ab".repeat(64)).into_bytes();
        let cases: Vec<Case> = vec![
            ("134 bytes", vec![], Present::Bytes(one_short)),
            (
                "136 bytes (an extra line)",
                vec![],
                Present::Bytes(one_over),
            ),
            ("uppercase hex", vec![], Present::Bytes(upper)),
            (
                "a prefix that is not sha512",
                vec![],
                Present::Bytes(wrong_prefix),
            ),
            (
                "a plain key",
                vec![],
                Present::Bytes(b"plain-key-not-a-digest".to_vec()),
            ),
            ("an empty file", vec![], Present::Bytes(Vec::new())),
            ("a dangling link", vec![], Present::DanglingLink),
            ("a link to a valid file", vec![], Present::LinkToValid),
            ("a folder", vec![], Present::Directory),
            (
                "WAHA_NO_API_KEY set, file valid",
                vec![("WAHA_NO_API_KEY", "1")],
                Present::Bytes(valid.clone()),
            ),
            (
                "WAHA_NO_API_KEY set, file missing",
                vec![("WAHA_NO_API_KEY", "1")],
                Present::Missing,
            ),
            (
                "WHATSAPP_API_KEY_EXCLUDE_PATH set, file valid",
                vec![("WHATSAPP_API_KEY_EXCLUDE_PATH", "/x")],
                Present::Bytes(valid),
            ),
            (
                "WHATSAPP_API_KEY_EXCLUDE_PATH set, file missing",
                vec![("WHATSAPP_API_KEY_EXCLUDE_PATH", "/x")],
                Present::Missing,
            ),
        ];
        let mut wrong: Vec<String> = Vec::new();
        for (label, variables, present) in cases {
            let dir = folder();
            let file = prepare(dir.path(), &present);
            let outcome = finish(
                spawn(&script_for(&file), &variables),
                Duration::from_secs(5),
            );
            if outcome.code != Some(78) {
                wrong.push(format!("{label}: exit {:?}, expected 78", outcome.code));
            }
            if !outcome.stdout.is_empty() {
                wrong.push(format!("{label}: the engine ran (printed on stdout)"));
            }
            if outcome.stderr.contains("abab") {
                wrong.push(format!("{label}: the digest reached stderr"));
            }
        }
        assert!(
            wrong.is_empty(),
            "the wrapper let something through: {wrong:?}"
        );
    }

    /// With the file missing it waits (it does not exit and does not start the engine); when the
    /// file shows up it starts with the digest as the key and none of the other key variables.
    #[test]
    fn wrapper_waits_for_the_file_then_hands_the_digest_to_the_engine() {
        let dir = folder();
        let file = dir.path().join("api-key.sha512");
        let variables = [
            ("WHATSAPP_API_KEY", "old-1"),
            ("WAHA_API_KEY", "old-2"),
            ("WAHA_API_KEY_PLAIN", "old-3"),
            ("DEBUG", "*"),
            // A switch that is set but empty is not an error.
            ("WAHA_NO_API_KEY", ""),
            ("WHATSAPP_API_KEY_EXCLUDE_PATH", ""),
        ];
        let mut child = spawn(&script_for(&file), &variables);
        std::thread::sleep(Duration::from_millis(700));
        let waiting = matches!(child.try_wait(), Ok(None));
        let wrote = dir.path().join("written");
        write(&wrote, digest().as_bytes());
        std::fs::rename(&wrote, &file)
            .unwrap_or_else(|error| broken("the file is renamed", "the test", &error.to_string()));
        let outcome = finish(child, Duration::from_secs(8));
        assert!(waiting, "the wrapper did not wait for the missing file");
        assert_eq!(outcome.code, Some(0), "stderr: {}", outcome.stderr);
        assert!(
            outcome.stderr.contains("waiting"),
            "it did not say that it waits: {}",
            outcome.stderr
        );
        assert!(
            !outcome.stderr.contains("abab"),
            "the digest reached stderr"
        );
        assert!(has_line(
            &outcome.stdout,
            &format!("WAHA_API_KEY={}", digest())
        ));
        assert_eq!(starts(&outcome.stdout, "WAHA_API_KEY="), 1);
        assert_eq!(starts(&outcome.stdout, "WHATSAPP_API_KEY="), 0);
        assert_eq!(starts(&outcome.stdout, "WAHA_API_KEY_PLAIN="), 0);
        assert_eq!(starts(&outcome.stdout, "DEBUG="), 0);
    }

    /// With the file there at the start, no wait: it runs at once.
    #[test]
    fn wrapper_starts_at_once_when_the_file_is_there() {
        let dir = folder();
        let file = prepare(dir.path(), &Present::Bytes(digest().into_bytes()));
        let outcome = finish(spawn(&script_for(&file), &[]), Duration::from_secs(5));
        assert_eq!(outcome.code, Some(0), "stderr: {}", outcome.stderr);
        assert!(has_line(
            &outcome.stdout,
            &format!("WAHA_API_KEY={}", digest())
        ));
        assert!(!outcome.stderr.contains("waiting"));
    }
}
