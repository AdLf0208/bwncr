// Bad Wither No Cookie Reloaded for Horizon — the sound-silencing policy
// (pure, no game access).
//
// Ported from Bad Wither No Cookie Reloaded 3.20.4 (`bwncr`, MIT, Kreezxil /
// Eleksploded) for Minecraft 1.21.1 NeoForge. Local-only until the maintainer
// approves publishing (ARCHITECTURE.md section 8).
//
// Behaviour, from the jar and the mod's 1.21.1 source
// (`SoundHandler.java:24-70`, `BWNCR_Config.java:23-47`):
//
// 1. When the client is about to play a sound, it is replaced with nothing if
//    it names `entity.wither.spawn` (exact), `entity.wither.death`
//    (case-insensitive), `entity.wandering_trader.ambient` or
//    `entity.llama.ambient` (case-insensitive), `entity.ender_dragon.death`
//    (exact) or `entity.lightning_bolt.thunder` (exact) and the matching
//    switch is on. All five switches default to true.
// 2. Any name in `silenceUs` (exact, case-sensitive) is silenced too; the
//    default list is a single empty string, which matches no real sound.
// 3. `debugMode` defaults to false and only logs each intercepted name.
//
// The pin has no "a sound is about to play" hook, so the suppression half is
// recorded as a gap in `notes.md`; this module is the decision it will use.
// Horizon names carry a `minecraft:` prefix where NeoForge's event names do
// not, so a queried name is compared without it (the port's adaptation).
//
// The slice also carries the `/bwncr listen` name feed (upstream's discovery
// command): the client hears the sound packets the server sends and reports
// the names, with vanilla's global level-event ids named as `level_particles.rs`
// names them.

use std::collections::VecDeque;

/// The config file's name in the game's `config/` folder (NeoForge's COMMON
/// config for this mod is `bwncr-common.toml`).
pub const CONFIG_FILE: &str = "bwncr-common.toml";

/// How many heard sound names `/bwncr heard` keeps.
pub const HEARD_LIMIT: usize = 32;

/// Upstream's `general` switches and the custom silence list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `silenceWither`: the wither's spawn and death broadcasts.
    pub silence_wither: bool,
    /// `silenceTrader`: the wandering trader's and llama's ambient sounds.
    pub silence_trader: bool,
    /// `silenceDragon`: the ender dragon's death broadcast.
    pub silence_dragon: bool,
    /// `silenceLightning`: the thunder broadcast.
    pub silence_lightning: bool,
    /// `silenceUs`: further sound names, compared exactly.
    pub silence_us: Vec<String>,
    /// `debugMode`: log every sound the policy is asked about.
    pub debug_mode: bool,
}

impl Default for Config {
    /// Upstream's defaults (`BWNCR_Config.java:23-47`).
    fn default() -> Self {
        Self {
            silence_wither: true,
            silence_trader: true,
            silence_dragon: true,
            silence_lightning: true,
            silence_us: vec![String::new()],
            debug_mode: false,
        }
    }
}

/// Which rule silences a sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Wither,
    Trader,
    Dragon,
    Lightning,
    Custom,
}

impl Rule {
    /// The config key the rule comes from, as `/bwncr status` reports it.
    pub fn key(self) -> &'static str {
        match self {
            Rule::Wither => "silenceWither",
            Rule::Trader => "silenceTrader",
            Rule::Dragon => "silenceDragon",
            Rule::Lightning => "silenceLightning",
            Rule::Custom => "silenceUs",
        }
    }
}

/// What to do with a sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Silence(Rule),
    Allow,
}

/// The sound name without Horizon's `minecraft:` prefix.
fn bare(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// Upstream's rules in upstream's order (`SoundHandler.java:27-59`): the four
/// named families first, then the custom list.
pub fn decide(config: &Config, name: &str) -> Decision {
    let name = bare(name);
    if config.silence_wither
        && (name == "entity.wither.spawn" || name.eq_ignore_ascii_case("entity.wither.death"))
    {
        return Decision::Silence(Rule::Wither);
    }
    if config.silence_trader
        && (name.eq_ignore_ascii_case("entity.wandering_trader.ambient")
            || name.eq_ignore_ascii_case("entity.llama.ambient"))
    {
        return Decision::Silence(Rule::Trader);
    }
    if config.silence_dragon && name == "entity.ender_dragon.death" {
        return Decision::Silence(Rule::Dragon);
    }
    if config.silence_lightning && name == "entity.lightning_bolt.thunder" {
        return Decision::Silence(Rule::Lightning);
    }
    if config
        .silence_us
        .iter()
        .any(|entry| !entry.is_empty() && bare(entry) == name)
    {
        return Decision::Silence(Rule::Custom);
    }
    Decision::Allow
}

/// Adds `name` to the custom list; false when it is already there (or blank).
pub fn set_custom(config: &mut Config, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || config.silence_us.iter().any(|entry| entry == name) {
        return false;
    }
    config.silence_us.push(name.to_owned());
    true
}

/// Removes `name` from the custom list; false when it was not there.
pub fn remove_custom(config: &mut Config, name: &str) -> bool {
    let before = config.silence_us.len();
    config.silence_us.retain(|entry| entry != name);
    config.silence_us.len() != before
}

/// Reads the `[general]` section of an upstream-shaped `bwncr-common.toml`.
/// Missing keys and unknown sections keep the defaults.
pub fn parse_config(text: &str) -> Config {
    let mut config = Config::default();
    let mut section = String::new();
    let mut pending: Option<String> = None;
    for raw in text.lines() {
        let line = match pending.as_mut() {
            Some(buffer) => {
                buffer.push(' ');
                buffer.push_str(raw);
                if raw.contains(']') {
                    let value = pending.take().unwrap_or_default();
                    apply_value(&mut config, &section, value.trim());
                }
                continue;
            }
            None => raw,
        };
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.trim().to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_owned();
        let value = value.trim();
        if value.starts_with('[') && !value.contains(']') {
            pending = Some(value.to_owned());
            continue;
        }
        apply(&mut config, &section, &key, value);
    }
    config
}

fn apply(config: &mut Config, section: &str, key: &str, value: &str) {
    if section != "general" {
        return;
    }
    match key {
        "silenceWither" => set_bool(&mut config.silence_wither, value),
        "silenceTrader" => set_bool(&mut config.silence_trader, value),
        "silenceDragon" => set_bool(&mut config.silence_dragon, value),
        "silenceLightning" => set_bool(&mut config.silence_lightning, value),
        "debugMode" => set_bool(&mut config.debug_mode, value),
        "silenceUs" => config.silence_us = parse_list(value),
        _ => {}
    }
}

fn apply_value(config: &mut Config, section: &str, value: &str) {
    apply(config, section, "silenceUs", value);
}

fn set_bool(target: &mut bool, value: &str) {
    match value {
        "true" => *target = true,
        "false" => *target = false,
        _ => {}
    }
}

/// A TOML string list, single- or multi-line: `["a", "b"]`, `[]`, `[""]`.
fn parse_list(value: &str) -> Vec<String> {
    let inner = value
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or("");
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    // An explicitly quoted empty string (`[""]`, the default list) is an item.
    let mut seen = false;
    for ch in inner.chars() {
        if escaped {
            current.push(ch);
            seen = true;
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => {
                quoted = !quoted;
                seen = true;
            }
            ',' if !quoted => {
                if seen {
                    out.push(current.trim().to_owned());
                }
                current.clear();
                seen = false;
            }
            _ => {
                current.push(ch);
                seen |= !ch.is_whitespace();
            }
        }
    }
    if seen {
        out.push(current.trim().to_owned());
    }
    out
}

/// Writes the config in the file's upstream key shape, with the defaults named.
pub fn format_config(config: &Config) -> String {
    let mut text = String::from(
        "# Bad Wither No Cookie Reloaded — general settings\n\
         # The keys are the upstream mod's; see notes.md for the port's limits.\n\
         [general]\n",
    );
    for (key, value, comment) in [
        (
            "silenceWither",
            config.silence_wither,
            "Silence the server-wide Wither spawn and death broadcast sounds.",
        ),
        (
            "silenceTrader",
            config.silence_trader,
            "Silence the wandering trader's and llama's ambient sounds.",
        ),
        (
            "silenceDragon",
            config.silence_dragon,
            "Silence the server-wide Ender Dragon death broadcast sound.",
        ),
        (
            "silenceLightning",
            config.silence_lightning,
            "Silence the server-wide Thunder broadcast sound.",
        ),
    ] {
        text.push_str(&format!("# {comment}\n{key} = {value}\n"));
    }
    text.push_str(
        "# Sound names to silence as well, one per line with no commas beyond the list.\n\
         silenceUs = [",
    );
    for (index, entry) in config.silence_us.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        text.push('"');
        text.push_str(&entry.replace('\\', "\\\\").replace('"', "\\\""));
        text.push('"');
    }
    text.push_str("]\n");
    text.push_str(&format!(
        "# Log every sound the policy is asked about.\ndebugMode = {}\n",
        config.debug_mode
    ));
    text
}

/// The sound a vanilla global level event plays (`level_particles.rs:330-334`).
/// Other local level events carry no name here; the feed reports their id.
pub fn global_event_sound(id: i32) -> Option<&'static str> {
    match id {
        1023 => Some("minecraft:entity.wither.spawn"),
        1028 => Some("minecraft:entity.ender_dragon.death"),
        1038 => Some("minecraft:block.end_portal.spawn"),
        _ => None,
    }
}

/// The newest sound names the client heard, oldest first, capped at
/// [`HEARD_LIMIT`].
#[derive(Debug, Default)]
pub struct Feed {
    entries: VecDeque<String>,
}

impl Feed {
    /// Records one heard name.
    pub fn push(&mut self, name: impl Into<String>) {
        if self.entries.len() == HEARD_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(name.into());
    }

    /// The newest `count` names, oldest first.
    pub fn recent(&self, count: usize) -> Vec<String> {
        let skip = self.entries.len().saturating_sub(count);
        self.entries.iter().skip(skip).cloned().collect()
    }

    /// How many names are held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no name has been heard.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn defaults_match_upstream() {
        let config = config();
        assert!(config.silence_wither);
        assert!(config.silence_trader);
        assert!(config.silence_dragon);
        assert!(config.silence_lightning);
        assert_eq!(config.silence_us, vec![String::new()]);
        assert!(!config.debug_mode);
    }

    #[test]
    fn the_wither_spawn_and_death_are_silenced() {
        for name in [
            "entity.wither.spawn",
            "minecraft:entity.wither.spawn",
            "entity.wither.death",
            "minecraft:ENTITY.WITHER.DEATH",
        ] {
            assert_eq!(
                decide(&config(), name),
                Decision::Silence(Rule::Wither),
                "{name}"
            );
        }
    }

    #[test]
    fn the_trader_and_llama_ambient_sounds_are_silenced() {
        for name in [
            "entity.wandering_trader.ambient",
            "minecraft:entity.llama.ambient",
            "ENTITY.WANDERING_TRADER.AMBIENT",
        ] {
            assert_eq!(
                decide(&config(), name),
                Decision::Silence(Rule::Trader),
                "{name}"
            );
        }
    }

    #[test]
    fn the_dragon_death_and_thunder_are_silenced() {
        assert_eq!(
            decide(&config(), "minecraft:entity.ender_dragon.death"),
            Decision::Silence(Rule::Dragon)
        );
        assert_eq!(
            decide(&config(), "entity.lightning_bolt.thunder"),
            Decision::Silence(Rule::Lightning)
        );
        assert_eq!(
            decide(&config(), "minecraft:entity.cow.ambient"),
            Decision::Allow
        );
    }

    #[test]
    fn a_disabled_switch_allows_its_sounds() {
        let mut config = config();
        config.silence_wither = false;
        assert_eq!(
            decide(&config, "minecraft:entity.wither.spawn"),
            Decision::Allow
        );
        let mut config = Config {
            silence_trader: false,
            ..Config::default()
        };
        assert_eq!(decide(&config, "entity.llama.ambient"), Decision::Allow);
        config.silence_dragon = false;
        assert_eq!(
            decide(&config, "entity.ender_dragon.death"),
            Decision::Allow
        );
        config.silence_lightning = false;
        assert_eq!(
            decide(&config, "entity.lightning_bolt.thunder"),
            Decision::Allow
        );
    }

    #[test]
    fn the_default_empty_entry_matches_nothing() {
        assert_eq!(
            decide(&config(), "minecraft:entity.sheep.step"),
            Decision::Allow
        );
    }

    #[test]
    fn the_custom_list_is_exact_and_case_sensitive() {
        let mut config = config();
        assert!(set_custom(&mut config, "entity.sheep.step"));
        assert_eq!(
            decide(&config, "minecraft:entity.sheep.step"),
            Decision::Silence(Rule::Custom)
        );
        assert_eq!(
            decide(&config, "minecraft:ENTITY.SHEEP.STEP"),
            Decision::Allow
        );
        assert!(!set_custom(&mut config, "entity.sheep.step"));
    }

    #[test]
    fn a_named_rule_wins_over_the_custom_list() {
        let mut config = config();
        assert!(set_custom(&mut config, "entity.wither.spawn"));
        assert_eq!(
            decide(&config, "minecraft:entity.wither.spawn"),
            Decision::Silence(Rule::Wither)
        );
    }

    #[test]
    fn remove_custom_takes_one_name_and_keeps_the_rest() {
        let mut config = config();
        assert!(set_custom(&mut config, "entity.sheep.step"));
        assert!(set_custom(&mut config, "block.note_block.harp"));
        assert!(remove_custom(&mut config, "entity.sheep.step"));
        assert!(!remove_custom(&mut config, "entity.sheep.step"));
        assert_eq!(
            config.silence_us,
            vec![String::new(), "block.note_block.harp".to_owned()]
        );
    }

    #[test]
    fn parse_reads_an_upstream_shaped_file() {
        let text = "\
# NeoForge writes this file
[general]
\t# a comment
\tsilenceWither = false
\tsilenceTrader = false
\tsilenceDragon = false
\tsilenceLightning = false
\tsilenceUs = [
\t\t\"entity.sheep.step\",
\t\t\"block.note_block.harp\"
\t]
\tdebugMode = true
";
        let config = parse_config(text);
        assert!(!config.silence_wither);
        assert!(!config.silence_trader);
        assert!(!config.silence_dragon);
        assert!(!config.silence_lightning);
        assert_eq!(
            config.silence_us,
            vec![
                "entity.sheep.step".to_owned(),
                "block.note_block.harp".to_owned()
            ]
        );
        assert!(config.debug_mode);
    }

    #[test]
    fn parse_keeps_defaults_for_a_missing_or_other_section() {
        let config = parse_config("[other]\nsilenceWither = false\n");
        assert_eq!(config, Config::default());
        let config = parse_config("silenceWither = false\n");
        assert!(config.silence_wither, "a key outside [general] is ignored");
    }

    #[test]
    fn parse_reads_a_one_line_list() {
        let config = parse_config("[general]\nsilenceUs = [\"a\", \"b\"]\n");
        assert_eq!(config.silence_us, vec!["a".to_owned(), "b".to_owned()]);
        let config = parse_config("[general]\nsilenceUs = []\n");
        assert!(config.silence_us.is_empty());
        let config = parse_config("[general]\nsilenceUs = [\"\"]\n");
        assert_eq!(config.silence_us, vec![String::new()]);
    }

    #[test]
    fn format_round_trips_through_parse() {
        let mut config = Config {
            silence_dragon: false,
            debug_mode: true,
            ..Config::default()
        };
        assert!(set_custom(&mut config, "entity.sheep.step"));
        let text = format_config(&config);
        assert!(text.contains("[general]"));
        assert_eq!(parse_config(&text), config);
        assert_eq!(
            parse_config(&format_config(&Config::default())),
            Config::default()
        );
    }

    #[test]
    fn global_level_events_are_named_as_the_client_names_them() {
        assert_eq!(
            global_event_sound(1023),
            Some("minecraft:entity.wither.spawn")
        );
        assert_eq!(
            global_event_sound(1028),
            Some("minecraft:entity.ender_dragon.death")
        );
        assert_eq!(
            global_event_sound(1038),
            Some("minecraft:block.end_portal.spawn")
        );
        assert_eq!(global_event_sound(1022), None);
    }

    #[test]
    fn the_feed_keeps_the_newest_names_in_order() {
        let mut feed = Feed::default();
        for i in 0..HEARD_LIMIT + 3 {
            feed.push(format!("minecraft:sound.{i}"));
        }
        assert_eq!(feed.len(), HEARD_LIMIT);
        let recent = feed.recent(2);
        assert_eq!(
            recent,
            vec![
                format!("minecraft:sound.{}", HEARD_LIMIT + 1),
                format!("minecraft:sound.{}", HEARD_LIMIT + 2),
            ]
        );
        assert!(Feed::default().recent(4).is_empty());
    }
}
