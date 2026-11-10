// Game-side glue for Bad Wither No Cookie Reloaded. Injected into `crate::game`
// as `bwncr_state`; it needs the game, so it is not part of the standalone
// library (only `runtime.rs` is).
//
// Upstream keeps its switches in NeoForge's common config and asks them on
// every `PlaySoundEvent` (`SoundEventHandler.java:21-27`). The port keeps the
// same file shape in the game's `config/` folder and the same defaults, plus
// the `/bwncr` command that exposes the policy and the `/bwncr listen` name
// feed. The pin has no "a sound is about to play" hook, so the policy is asked
// by the command and by the feed; nothing here can silence a sound yet (see
// notes.md, "gaps").

use std::fs;
use std::sync::Mutex;

use horizon_client::text::Text;
use horizon_protocol::packets::play::Clientbound;

use crate::game::bwncr::{
    CONFIG_FILE, Config, Decision, Feed, decide, format_config, global_event_sound,
    parse_config, remove_custom, set_custom,
};

/// What the mod keeps for the whole process: on single player the window's
/// client and the server thread are two `Game`s in one process, so the feed the
/// client hears is the one `/bwncr heard` reads on the server.
#[derive(Default)]
struct Core {
    config: Config,
    /// Upstream's `whatWasThat`: `/bwncr listen` toggles it.
    listen: bool,
    /// The sounds heard while listening.
    feed: Feed,
}

static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn with_core<T>(update: impl FnOnce(&mut Core) -> T) -> T {
    let mut guard = CORE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    update(guard.get_or_insert_with(Core::default))
}

fn config() -> Config {
    with_core(|core| core.config.clone())
}

fn on(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

/// The `[general]` switches in one line, for `/bwncr status` and the e2e.
fn summary(config: &Config, listen: bool) -> String {
    let custom = config
        .silence_us
        .iter()
        .filter(|entry| !entry.is_empty())
        .count();
    format!(
        "silenceWither={} silenceTrader={} silenceDragon={} silenceLightning={} custom={custom} \
         debug={} listen={}",
        on(config.silence_wither),
        on(config.silence_trader),
        on(config.silence_dragon),
        on(config.silence_lightning),
        on(config.debug_mode),
        on(listen)
    )
}

fn config_path() -> std::io::Result<std::path::PathBuf> {
    let dir = crate::game::Game::native_config_dir()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(dir.join(CONFIG_FILE))
}

/// Writes the current config back to `config/bwncr-common.toml`.
fn save(config: &Config) -> std::io::Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, format_config(config))
}

/// Before the server starts: read `config/bwncr-common.toml`, writing the
/// upstream defaults when it is absent (NeoForge does the same for a COMMON
/// config on first load).
#[allow(private_interfaces)]
pub fn on_server_about_to_start(_game: &mut crate::game::Game<'_>) {
    let mut config = Config::default();
    match config_path() {
        Ok(path) => {
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    let text = format_config(&config);
                    if save(&config).is_err() {
                        log::warn!("[bwncr] could not write {}", path.display());
                    }
                    text
                }
            };
            config = parse_config(&text);
            log::info!(
                "[bwncr] config {} ({})",
                path.display(),
                summary(&config, false)
            );
        }
        Err(error) => log::warn!("[bwncr] no config dir: {error}"),
    }
    with_core(|core| core.config = config);
}

/// `/bwncr`'s usages in the server's command tree (API 8).
pub fn on_register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    use crate::game::native_server::NativeCommand;
    for usage in [
        "bwncr",
        "bwncr status",
        "bwncr check <sound>",
        "bwncr listen",
        "bwncr heard [count]",
    ] {
        commands.push(NativeCommand::new(usage));
    }
    for usage in [
        "bwncr silence <sound>",
        "bwncr allow <sound>",
        "bwncr reload",
    ] {
        commands.push(NativeCommand::op(usage, 2));
    }
}

/// What `/bwncr …` was asked to do.
enum Action {
    Status,
    Check(String),
    Silence(String),
    Allow(String),
    Listen,
    Heard(usize),
    Reload,
    Usage,
}

/// The action a command line names, or None when it is not ours.
fn parse(command: &str) -> Option<Action> {
    let line = command.trim().trim_start_matches('/').trim();
    let mut words = line.split_whitespace();
    if words.next()? != "bwncr" {
        return None;
    }
    let Some(action) = words.next() else {
        return Some(Action::Usage);
    };
    let rest = words.collect::<Vec<_>>().join(" ");
    Some(match action {
        "status" => Action::Status,
        "check" => Action::Check(rest),
        "silence" | "add" => Action::Silence(rest),
        "allow" | "remove" => Action::Allow(rest),
        "listen" => Action::Listen,
        "heard" => Action::Heard(rest.parse().unwrap_or(8)),
        "reload" => Action::Reload,
        _ => Action::Usage,
    })
}

fn reload(game: &mut crate::game::Game<'_>) -> Config {
    let loaded = match config_path().and_then(|path| fs::read_to_string(path)) {
        Ok(text) => parse_config(&text),
        Err(error) => {
            game.native_feedback(Text::literal(format!("bwncr: reload failed: {error}")));
            return config();
        }
    };
    with_core(|core| core.config = loaded.clone());
    loaded
}

/// A player's `bwncr …` command (API 8): set `ran` for any `bwncr` form, then
/// answer the player (`SoundHandler`'s decisions, made by the policy).
#[allow(private_interfaces)]
pub fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    _player: &crate::game::native_server::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let Some(action) = parse(command) else {
        return;
    };
    *ran = true;
    let text = match action {
        Action::Usage => Text::literal(
            "bwncr: /bwncr status|check <sound>|listen|heard [n]|silence <sound>|allow <sound>|reload",
        ),
        Action::Status => {
            let config = config();
            let listen = with_core(|core| core.listen);
            Text::literal(format!("bwncr: {}", summary(&config, listen)))
        }
        Action::Check(name) => {
            let name = name.trim();
            if name.is_empty() {
                Text::literal("bwncr: usage: /bwncr check <sound>")
            } else {
                let config = config();
                let decision = decide(&config, name);
                if config.debug_mode {
                    log::info!("[bwncr] check {name}: {decision:?}");
                }
                Text::literal(format!(
                    "bwncr: {} -> {}",
                    name,
                    match decision {
                        Decision::Silence(rule) => format!("SILENCE ({})", rule.key()),
                        Decision::Allow => "ALLOW".to_owned(),
                    }
                ))
            }
        }
        Action::Silence(name) => {
            let name = name.trim();
            if name.is_empty() {
                Text::literal("bwncr: usage: /bwncr silence <sound>")
            } else {
                let mut config = config();
                if set_custom(&mut config, name) {
                    let saved = save(&config).is_ok();
                    let count = config.silence_us.iter().filter(|e| !e.is_empty()).count();
                    with_core(|core| core.config = config);
                    Text::literal(format!(
                        "bwncr: silenceUs += {name} (now {count}{})",
                        if saved { "" } else { ", not saved" }
                    ))
                } else {
                    Text::literal(format!("bwncr: {name} is already in silenceUs"))
                }
            }
        }
        Action::Allow(name) => {
            let name = name.trim();
            if name.is_empty() {
                Text::literal("bwncr: usage: /bwncr allow <sound>")
            } else {
                let mut config = config();
                if remove_custom(&mut config, name) {
                    let saved = save(&config).is_ok();
                    let count = config.silence_us.iter().filter(|e| !e.is_empty()).count();
                    with_core(|core| core.config = config);
                    Text::literal(format!(
                        "bwncr: silenceUs -= {name} (now {count}{})",
                        if saved { "" } else { ", not saved" }
                    ))
                } else {
                    Text::literal(format!("bwncr: {name} is not in silenceUs"))
                }
            }
        }
        Action::Listen => {
            let listen = with_core(|core| {
                core.listen = !core.listen;
                core.listen
            });
            Text::literal(format!("bwncr: listen {}", on(listen)))
        }
        Action::Heard(count) => {
            let (heard, empty, total) = with_core(|core| {
                (
                    core.feed.recent(count.clamp(1, 32)),
                    core.feed.is_empty(),
                    core.feed.len(),
                )
            });
            if empty {
                Text::literal("bwncr: heard nothing")
            } else {
                Text::literal(format!(
                    "bwncr: heard {} of {}: {}",
                    heard.len(),
                    total,
                    heard.join(", ")
                ))
            }
        }
        Action::Reload => {
            let config = reload(game);
            let listen = with_core(|core| core.listen);
            Text::literal(format!("bwncr: reloaded {}", summary(&config, listen)))
        }
    };
    game.native_feedback(text);
}

/// Records a sound name the client heard while `/bwncr listen` is on.
fn record(name: String) {
    with_core(|core| {
        if core.listen {
            if core.config.debug_mode {
                log::info!("[bwncr] heard {name}");
            }
            core.feed.push(name);
        }
    });
}

/// The client hears the sound packets the server sends (`client_packet`, API
/// 408): name the sound as the client is about to play it, for `/bwncr listen`
/// and `/bwncr heard`.
#[allow(private_interfaces)]
pub fn on_client_packet(
    game: &mut crate::game::Game<'_>,
    packet: &Clientbound,
) {
    match packet {
        Clientbound::Sound(sound) => {
            if let Some(name) = game.remote_sound_name(&sound.sound) {
                record(name);
            }
        }
        Clientbound::SoundEntity(sound) => {
            if let Some(name) = game.remote_sound_name(&sound.sound) {
                record(name);
            }
        }
        Clientbound::LevelEvent(event) => {
            let name = global_event_sound(event.kind)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("minecraft:level_event/{}", event.kind));
            record(name);
        }
        _ => {}
    }
}

/// The policy for a sound name, as the code that will run on the missing hook
/// asks it (kept so the runtime's `Rule` stays used and the decision is one
/// place).
#[expect(dead_code, reason = "the suppression hook is not stamped yet (notes.md)")]
pub fn decision_for(name: &str) -> Decision {
    let decision = decide(&config(), name);
    if config().debug_mode {
        log::info!("[bwncr] {name}: {decision:?}");
    }
    decision
}

