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
//! What the YAML parser cannot see, the rules refuse outright: it does not apply merge keys (`<<`),
//! which Compose does, so a `<<` anywhere is a violation; and Compose interpolates every `$` of
//! the file that is not doubled, so the engine service may hold no single `$`.

use std::collections::BTreeMap;

use arauto_hub::engine::types::EngineVersion;
use yaml_rust2::{Yaml, YamlLoader};

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
            "the recipe has exactly the services hub, postgres and engine, each with keys from its allowlist (no port, no host network, no volumes_from); no YAML merge key"
        }
        2 => "the engine image is pinned by tag and sha256 digest, to a tested version",
        3 => "the engine has no env_file and no single `$` (Compose would interpolate it)",
        4 => "the engine pins exactly the 13 variables, and sets no key or hook variable",
        5 => {
            "every mount is a named volume from the list of its service (no host path, no bind); the engine has exactly two; the digest volume is read-only in the engine and shared with the hub only; the data volume is the hub's alone; no volume_from, name or external"
        }
        6 => {
            "no fixed or external network; engine on one non-internal network with the hub; postgres only on internal networks; engine and postgres share none"
        }
        7 => {
            "the hub sets the three ARAUTO_ENGINE_* values, and the digest folder is the mount target on both sides"
        }
        8 => "the wrapper of the engine is exactly the text the test keeps",
        9 => "the engine depends on a healthy hub, without restart; the image can write the digest",
        10 => {
            "the bring-your-own-engine recipes have no engine service and none of the ARAUTO_ENGINE_* variables set"
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

fn load(text: &str, file: &str) -> Yaml {
    parse(text).unwrap_or_else(|detail| broken("the file is one YAML document", file, &detail))
}

fn recipe_text() -> String {
    ENGINE_RECIPE.replace("\r\n", "\n")
}

fn dockerfile_text() -> String {
    DOCKERFILE.replace("\r\n", "\n")
}

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

/// The `environment` of a service, in the map form or in the `KEY=value` list form.
fn environment(service: &Yaml) -> Result<BTreeMap<String, String>, String> {
    let mut found = BTreeMap::new();
    match child(service, "environment") {
        None | Some(Yaml::Null) => {}
        Some(Yaml::Hash(map)) => {
            for (name, value) in map {
                let (Some(name), Some(value)) = (scalar(name), scalar(value)) else {
                    return Err("an environment name or value is not a scalar".to_owned());
                };
                found.insert(name, value);
            }
        }
        Some(Yaml::Array(items)) => {
            for item in items {
                let Some(line) = scalar(item) else {
                    return Err("an environment line is not text".to_owned());
                };
                match line.split_once('=') {
                    Some((name, value)) => found.insert(name.to_owned(), value.to_owned()),
                    None => found.insert(line, String::new()),
                };
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
            Yaml::String(text) => {
                let parts: Vec<&str> = text.split(':').collect();
                match parts.as_slice() {
                    [source, target] => Ok(Mount {
                        kind: short_kind(source),
                        source: (*source).to_owned(),
                        target: (*target).to_owned(),
                        read_only: false,
                    }),
                    [source, target, modes] => Ok(Mount {
                        kind: short_kind(source),
                        source: (*source).to_owned(),
                        target: (*target).to_owned(),
                        read_only: modes.split(',').any(|mode| mode == "ro"),
                    }),
                    _ => Err(format!("`{text}` is not source:target[:mode]")),
                }
            }
            Yaml::Hash(_) => Ok(Mount {
                kind: child(item, "type").and_then(scalar).unwrap_or_default(),
                source: child(item, "source").and_then(scalar).unwrap_or_default(),
                target: child(item, "target").and_then(scalar).unwrap_or_default(),
                read_only: matches!(child(item, "read_only"), Some(Yaml::Boolean(true))),
            }),
            _ => Err("a volume is not text or a mapping".to_owned()),
        })
        .collect()
}

fn list_of_text(node: Option<&Yaml>) -> Result<Vec<String>, String> {
    match node {
        None | Some(Yaml::Null) => Ok(Vec::new()),
        // A mapping form (`networks: {name: {...}}`) lists its keys.
        Some(map @ Yaml::Hash(_)) => Ok(keys_of(map)),
        Some(Yaml::Array(items)) => items
            .iter()
            .map(|item| scalar(item).ok_or_else(|| "an item is not text".to_owned()))
            .collect(),
        Some(_) => Err("not a list of text".to_owned()),
    }
}

fn service_networks(service: &Yaml) -> Result<Vec<String>, String> {
    list_of_text(child(service, "networks"))
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

// -------------------------------------------------------------------------------- the rules

fn rule_01(compose: &Yaml, r: &mut Report) {
    let file = ENGINE_RECIPE_PATH;

    // A merge key adds keys that this parser does not apply and Compose does: refuse it anywhere.
    let mut merges: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    walk(compose, "", &mut |path, node| match node {
        Yaml::Hash(map) => {
            for name in map.keys() {
                if scalar(name).as_deref() == Some("<<") {
                    merges.push(path.to_owned());
                }
            }
        }
        Yaml::Alias(_) | Yaml::BadValue => unresolved.push(path.to_owned()),
        _ => {}
    });
    for path in merges {
        r.fail(
            1,
            &format!("{file}{path}"),
            "a YAML merge key `<<`: Compose applies it and this test's parser does not, so the rules could not see what it adds",
        );
    }
    for path in unresolved {
        r.fail(
            1,
            &format!("{file}{path}"),
            "an alias or a value the parser could not resolve",
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
            image.starts_with("postgres:"),
            1,
            &format!("{file} services.postgres.image"),
            &format!("`{image}` is not a postgres image"),
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
}

fn rule_02(compose: &Yaml, r: &mut Report) {
    let place = format!("{ENGINE_RECIPE_PATH} services.engine.image");
    let Some(engine) = r.must(2, &place, service(compose, "engine")) else {
        return;
    };
    let Some(image) = child(engine, "image").and_then(scalar) else {
        r.fail(2, &place, "no image");
        return;
    };
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
    // Compose reads every `$` of every field of the service, not only of `environment`.
    let mut dollars: Vec<String> = Vec::new();
    walk(engine, &place, &mut |path, node| {
        if let Yaml::String(text) | Yaml::Real(text) = node
            && has_single_dollar(text)
        {
            dollars.push(path.to_owned());
        }
    });
    for path in dollars {
        r.fail(
            3,
            &path,
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
            "this variable must not be set in the recipe",
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
                actual == value,
                4,
                &format!("{place}.{name}"),
                &format!("expected `{value}`, found `{actual}`"),
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

    // Like a network, a volume with a fixed name or an external one is shared across installations.
    if let Some(volumes) = child(compose, "volumes") {
        for name in keys_of(volumes) {
            if let Some(definition) = child(volumes, &name) {
                for forbidden in ["name", "external"] {
                    r.check(
                        child(definition, forbidden).is_none(),
                        5,
                        &format!("{file} volumes.{name}.{forbidden}"),
                        "a volume with a fixed name or an external one is shared across installations",
                    );
                }
            }
        }
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
                let store = environment(engine)
                    .ok()
                    .and_then(|env| env.get("WAHA_LOCAL_STORE_BASE_DIR").cloned());
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
        if let Some(definition) = definition {
            r.check(
                child(definition, "name").is_none(),
                6,
                &format!("{place}.name"),
                "a network with a fixed name is shared across installations",
            );
            r.check(
                child(definition, "external").is_none(),
                6,
                &format!("{place}.external"),
                "a network must not be external",
            );
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
                actual == value,
                7,
                &format!("{hub_place}.{name}"),
                &format!("expected `{value}`, found `{actual}`"),
            ),
            None => r.fail(7, &format!("{hub_place}.{name}"), "the variable is missing"),
        }
    }
    let folder = hub_environment
        .get("ARAUTO_ENGINE_DIGEST_DIR")
        .cloned()
        .unwrap_or_default();
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

fn rule_08(compose: &Yaml, r: &mut Report) {
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

fn rule_09(compose: &Yaml, dockerfile: &str, r: &mut Report) {
    let engine_place = format!("{ENGINE_RECIPE_PATH} services.engine");
    if let Some(engine) = r.must(9, &engine_place, service(compose, "engine")) {
        let hub_dependency = child(engine, "depends_on").and_then(|depends| child(depends, "hub"));
        let condition = hub_dependency
            .and_then(|hub| child(hub, "condition"))
            .and_then(scalar);
        r.check(
            condition.as_deref() == Some("service_healthy"),
            9,
            &format!("{engine_place}.depends_on.hub.condition"),
            &format!("found {condition:?}, expected service_healthy"),
        );
        r.check(
            !matches!(
                hub_dependency.and_then(|hub| child(hub, "restart")),
                Some(Yaml::Boolean(true))
            ),
            9,
            &format!("{engine_place}.depends_on.hub.restart"),
            "a restart of the hub must not bounce the engine (decision 0003)",
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

    // Only the last stage is the image that runs: a step, a user or a health check of a builder
    // stage says nothing about it.
    let all = dockerfile.replace("\r\n", "\n").replace("\\\n", " ");
    let lines: Vec<&str> = all.lines().collect();
    let first = lines
        .iter()
        .rposition(|line| line.trim_start().to_ascii_uppercase().starts_with("FROM "))
        .map_or(0, |index| index + 1);
    let joined = lines[first..].join("\n");
    let healthchecks: Vec<&str> = joined
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("HEALTHCHECK "))
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
    let user = joined
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("USER "))
        .map(str::trim);
    r.check(
        user == Some("arauto"),
        9,
        &format!("{DOCKERFILE_PATH} USER"),
        &format!("the image must end running as the user arauto, found {user:?}"),
    );
    let steps: Vec<&str> = joined
        .lines()
        .flat_map(|line| line.split("&&"))
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

/// Rules 1 to 9, on one recipe and one Dockerfile.
fn check_all(compose: &Yaml, dockerfile: &str) -> Report {
    let mut report = Report::default();
    rule_01(compose, &mut report);
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

/// Rule 10, on recipes of the bring-your-own-engine kind: `(path, text)` of each.
fn rule_10_recipes(files: &[(&str, &str)], r: &mut Report) {
    for (path, text) in files {
        let compose = match parse(text) {
            Ok(compose) => compose,
            Err(detail) => {
                r.fail(10, path, &detail);
                continue;
            }
        };
        let names = match services_of(&compose) {
            Ok(services) => keys_of(services),
            Err(detail) => {
                r.fail(10, path, &detail);
                continue;
            }
        };
        for name in names {
            r.check(
                name != "engine",
                10,
                &format!("{path} services.{name}"),
                "this recipe must not have an engine service",
            );
            let Some(this) = r.must(10, path, service(&compose, &name)) else {
                continue;
            };
            let image = child(this, "image").and_then(scalar).unwrap_or_default();
            r.check(
                !image.to_ascii_lowercase().contains("waha"),
                10,
                &format!("{path} services.{name}.image"),
                &format!("`{image}` looks like the WhatsApp engine"),
            );
            let place = format!("{path} services.{name}.environment");
            let Some(variables) = r.must(10, &place, environment(this)) else {
                continue;
            };
            for variable in ENGINE_VARIABLES {
                r.check(
                    variables.get(variable).is_none_or(String::is_empty),
                    10,
                    &format!("{place}.{variable}"),
                    "this variable must not be set to a non-empty value",
                );
            }
        }
    }
}

/// Rule 10, on the files of variables those recipes read: `(path, text)` of each.
fn rule_10_env_files(files: &[(&str, &str)], r: &mut Report) {
    for (path, text) in files {
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((name, value)) = line.split_once('=') {
                let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
                r.check(
                    !(ENGINE_VARIABLES.contains(&name.trim()) && !value.is_empty()),
                    10,
                    &format!("{path}:{}", number + 1),
                    &format!("{} is set to a non-empty value", name.trim()),
                );
            }
        }
    }
}

// -------------------------------------------------------------------------------- rule tests

/// Rule 1. Protects decision 0052 (the engine has no published port and no domain) and the
/// principle "never expose the WhatsApp engine outside the internal network" (CLAUDE.md). The
/// engine answers to the hub on a network of the composition, never on the host. The engine
/// service has a closed list of keys, so nothing that changes how it runs is added unseen, and no
/// YAML merge key can add one behind the parser.
#[test]
fn rule_01_engine_publishes_no_port() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(1);
}

/// Rule 2. Protects the supply chain of the engine, an unofficial WhatsApp client that runs as
/// root: the tag says which version (one the hub was tested with), the digest says which bytes
/// (decision 0052).
#[test]
fn rule_02_engine_image_is_pinned_by_tag_and_digest() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(2);
}

/// Rule 3. Protects "nothing from the `.env` of Easypanel reaches the engine" and the principle
/// that a long-lived secret never passes through a person or through an environment the admin
/// fills: the engine gets its key only from the wrapper. Compose interpolates every `$` of the
/// file, not only those of `environment`, so the whole engine service is read.
#[test]
fn rule_03_engine_has_no_env_file_and_no_interpolation() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(3);
}

/// Rule 4. Protects the engine's authentication: the key is only the digest the hub writes, the
/// dashboard and the Swagger page never ask for a password they do not have, no screen is on, no QR
/// is printed in a log, and the engine sends no webhook of its own (decision 0052, security
/// principles 2 and 10).
#[test]
fn rule_04_engine_pins_thirteen_variables_and_no_key_switch() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(4);
}

/// Rule 5. Protects the principle "the hub only writes the digest; the engine can never write it":
/// the digest volume is read-only in the engine, only the hub and the engine mount it, the hub's
/// data volume (master key, audit mark, database password) is mounted by the hub alone, no
/// service takes another's volumes with `volumes_from`, and no volume has a name that two
/// installations could share.
#[test]
fn rule_05_digest_volume_is_read_only_in_the_engine_and_shared_with_nobody_else() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(5);
}

/// Rule 6. Protects the principle "expose the engine outside the internal network, never": the
/// engine reaches WhatsApp through one network and the hub through it; the database is on an
/// internal network only, which the engine does not share; no network has a fixed name that two
/// installations could share, and none is external.
#[test]
fn rule_06_networks_keep_the_engine_apart_from_the_database() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(6);
}

/// Rule 7. Protects the contract between the hub and the engine of the same composition (plan
/// section 1.1): the hub fills a missing engine only with these three values, and the folder in
/// which the hub writes the digest is the folder in which the engine reads it.
#[test]
fn rule_07_hub_variables_and_digest_folder_agree_with_the_mounts() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(7);
}

/// Rule 8. Protects security principles 2 and 11 and the rule of the wrapper (critique, security
/// correction 6): the engine never starts without the key digest the hub wrote, never with a
/// switch that lets calls in without a key, never with a key from the environment, and the
/// digest never reaches a log. The script is compared line by line with `WRAPPER_GOLDEN`: a line
/// added after the refusal (`set -a`, a variable of the dashboard) is as much a change of the
/// guard as a line removed.
#[test]
fn rule_08_wrapper_text_keeps_its_guards() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(8);
}

/// Rule 9. Protects the order of the start (decision 0052): the engine never starts before the hub
/// answers its health check, so the digest folder and the first digest are there; a restart of the
/// hub does not bounce the engine; the hub image answers the check, and its user owns the folder,
/// so it can write the digest.
#[test]
fn rule_09_engine_waits_for_a_healthy_hub_and_the_image_can_write_the_digest() {
    check_all(&engine_recipe(), &dockerfile_text()).assert_clean(9);
}

/// Rule 10. Protects decision 0052: the "bring your own engine" recipes stay as they were. They
/// bring no engine and do not turn on the hub's engine installation (all three ARAUTO_ENGINE_*
/// variables set, or none: here, none), so a hub of those recipes never draws a key for an
/// engine it does not own.
#[test]
fn rule_10_bring_your_own_engine_recipes_bring_no_engine() {
    let mut report = Report::default();
    rule_10_recipes(
        &[
            (EASYPANEL_OWN_ENGINE_PATH, EASYPANEL_OWN_ENGINE),
            (DOCKER_OWN_ENGINE_PATH, DOCKER_OWN_ENGINE),
        ],
        &mut report,
    );
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
        let compose = load(&text, case.name);
        let report = check_all(&compose, &dockerfile_text());
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
        let report = check_all(&engine_recipe(), &text);
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
    let report = check_all(&load(&text, "edited"), &dockerfile_text());
    report.assert_clean(8);
}

/// The two kinds of break rule 10 reads: an engine in a bring-your-own recipe, and the three
/// variables set in a file such a recipe reads.
#[test]
fn rule_10_bites() {
    let mut recipes = Report::default();
    rule_10_recipes(
        &[(
            "synthetic.yaml",
            "services:\n  engine:\n    image: example.invalid/x:1\n",
        )],
        &mut recipes,
    );
    assert!(
        recipes
            .of_rule(10)
            .iter()
            .any(|v| v.detail.contains("engine service")),
        "an engine service went unseen: {:?}",
        recipes.found
    );
    let mut image = Report::default();
    rule_10_recipes(
        &[(
            "synthetic.yaml",
            "services:\n  hub:\n    image: devlikeapro/waha:gows-2026.10.1\n",
        )],
        &mut image,
    );
    assert!(
        !image.of_rule(10).is_empty(),
        "an engine image went unseen: {:?}",
        image.found
    );
    let mut environment = Report::default();
    rule_10_recipes(
        &[(
            "synthetic.yaml",
            "services:\n  hub:\n    image: x\n    environment:\n      ARAUTO_ENGINE_URL: http://engine:3000\n",
        )],
        &mut environment,
    );
    assert!(
        !environment.of_rule(10).is_empty(),
        "a set ARAUTO_ENGINE_URL went unseen: {:?}",
        environment.found
    );
    let mut files = Report::default();
    rule_10_env_files(
        &[("synthetic.env", "# note\nARAUTO_ENGINE_DIGEST_DIR=/run/x\n")],
        &mut files,
    );
    assert!(
        !files.of_rule(10).is_empty(),
        "a set ARAUTO_ENGINE_DIGEST_DIR went unseen: {:?}",
        files.found
    );
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
