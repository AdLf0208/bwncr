#!/usr/bin/env bash
# In-game check for bwncr slice 1, campaign pin 1d63e0a5, Minecraft 26.3.
#
# The mod is a sound-silencing policy whose *suppression* half has no hook at
# the pin (notes.md, "gaps"). What this scenario can prove:
#   1. the `/bwncr` command exists only with the mod (control below), and the
#      loaded defaults match upstream (all four switches on, empty custom list);
#   2. the policy silences exactly the upstream names, with their case quirks,
#      and leaves everything else alone;
#   3. `/bwncr silence` / `/bwncr allow` edit the custom list and rewrite
#      config/bwncr-common.toml;
#   4. `/bwncr listen` + `/bwncr heard` really observe the server's sound
#      packets (a `/playsound` round trip), which is the upstream `/listen`
#      discovery command;
#   5. the control run on the *unmodded* shared game sees no `bwncr` command,
#      so the replies above come from the port and not from the game.
#
# NEGATIVE=1 keeps the scenario but expects the wither spawn to be ALLOWED; the
# run MUST fail, proving the assertion is not vacuous.
# SELFTEST=1 runs the assertions against a wrong transcript; it MUST fail too.
#
# Run through the registry:
#   ports/tools/build-slot.sh e2e --job bwncr-1d63e0a --port ports/bwncr -- tools/e2e.sh
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/bwncr
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
SHARED_GAME=${SHARED_GAME:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/bwncr-e2e.sock}
LOG=${LOG:-/tmp/opencode/bwncr-e2e.log}
CONTENT=${CONTENT:-$PORT/content}
NEGATIVE=${NEGATIVE:-0}
SELFTEST=${SELFTEST:-0}
# EMBEDDED=1 launches the game with **only the native bundle** (no `--dev-mod`):
# the content comes from the bundle's embedded copy (`mixins/bwncr/data/`, the
# AE2 pattern). CONTROL=1 (default with EMBEDDED) first launches the same binary
# with `.horizon-cache` unusable, where no content mod may load.
EMBEDDED=${EMBEDDED:-0}
CONTROL=${CONTROL:-$EMBEDDED}
WORLD=${WORLD:-BwncrE2E}
CONTROL_WORLD=BwncrControlEmbedded

mkdir -p "$(dirname "$LOG")"
exec > >(tee "$LOG") 2>&1

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

ctl() { "$CTL" --socket "$SOCK" "$@"; }

SCRATCH=""
SCRATCH2=""
PIDFILE=/tmp/opencode/bwncr-e2e.pid
# Launch the game and remember its pid: the game keeps writing to this log
# while it shuts down, and EXIT:<status> must be the log's last line.
launch_game() { # launch_game <game binary> [extra args...]
    local game=$1
    shift
    local json
    json=$("$CTL" --socket "$SOCK" launch --game "$game" "$@")
    jq -r '.launch.pid // empty' <<<"$json" >"$PIDFILE"
}
cleanup() {
    local rc=$? pid
    ctl quit >/dev/null 2>&1 || true
    pid=$(cat "$PIDFILE" 2>/dev/null || true)
    if [ -n "${pid:-}" ]; then
        for _ in $(seq 1 100); do
            kill -0 "$pid" 2>/dev/null || break
            sleep 0.2
        done
    fi
    sleep 1
    [ -n "$SCRATCH" ] && rm -rf "$SCRATCH" 2>/dev/null || true
    [ -n "$SCRATCH2" ] && rm -rf "$SCRATCH2" 2>/dev/null || true
    rm -f "$PIDFILE" 2>/dev/null || true
    echo "EXIT:$rc"
}
trap cleanup EXIT

# The last N chat lines, one per line.
chat_lines() { ctl messages 40 | jq -r '.chat[]'; }

# Runs a chat line and waits for the tick that answers it.
say() {
    ctl chat "$1" >/dev/null
    ctl wait 4 >/dev/null
}

check_text() { # check_text <transcript> <substring> <why>
    if ! grep -qF -- "$2" <<<"$1"; then
        echo "--- text was ---" >&2
        echo "$1" >&2
        fail "$3: missing '$2'"
    fi
    echo "OK: $3"
}

expect_chat() { # expect_chat <substring> <why>
    check_text "$(chat_lines)" "$1" "$2"
}

if [ "$SELFTEST" = 1 ]; then
    echo "== selftest: the assertion must reject a wrong transcript =="
    check_text "bwncr: something else" "bwncr: this line is not in the chat" "selftest"
    echo "selftest did not fail"
    exit 1
fi

[ -x "$CTL" ] || fail "missing horizon-ctl: $CTL"
[ -x "$GAME" ] || fail "missing horizon-client: $GAME"
[ -x "$SHARED_GAME" ] || fail "missing unmodded game: $SHARED_GAME"
[ -d "$RES/assets" ] || fail "missing prepared resources: $RES"
[ -f "$CONTENT/horizon-mod.toml" ] || fail "missing content mod: $CONTENT"
command -v jq >/dev/null || fail "missing jq"
[ -f "$PORT/bwncr.hnmod" ] || fail "missing package: $PORT/bwncr.hnmod"

SCRATCH=$(mktemp -d /tmp/opencode/bwncr-home.XXXXXX)
mkdir -p "$SCRATCH"
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
export HORIZON_LAUNCHER_NO_SELF_UPDATE=1
mkdir -p "$(dirname "$SOCK")"
rm -f "$SOCK"
cd "$SCRATCH"

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod, and that content mod must be the one loaded.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/bwncr-embedded-content.hmod
    local log=$SCRATCH/game/logs/latest.log
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/bwncr.hmod" \
        || fail "the embedded content differs from content/bwncr.hmod"
    grep -q "Horizon: content from 1 mod(s)" "$log" \
        || fail "the embedded content mod did not load: $(grep -i 'content' "$log" | tail -3)"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/bwncr.hmod, loaded as 1 content mod"
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. This port's `.hmod` carries only lang files, so it has no block or
# item id to leave unknown; the positive signal is the loader's own line, which
# must be absent, while `/bwncr` still answers from the code.
embedded_control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch_game "$GAME" \
        --world "$CONTROL_WORLD" --seed 20261109 --creative --headless --wait --resources "$RES"
    ctl wait-ready 120
    ctl wait 10 >/dev/null
    local log=$SCRATCH/game/logs/latest.log
    if grep -q "Horizon: content from 1 mod(s)" "$log"; then
        fail "the control loaded a content mod, so the content did not come from the embedded file"
    fi
    [ -f "$SCRATCH/.horizon-cache" ] \
        || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    say "/bwncr status"
    expect_chat "bwncr: silenceWither=on" "the code still answers in the control"
    echo "  without the embedded file: no content mod loaded, /bwncr still answers from the code"
    ctl quit >/dev/null 2>&1 || true
    sleep 2
    rm -f "$SCRATCH/.horizon-cache"
}

echo "== launch (world $WORLD, $([ "$EMBEDDED" = 1 ] && echo 'one bundle, no --dev-mod' || echo 'content --dev-mod content/')) =="
if [ "$EMBEDDED" = 1 ]; then
    if [ "$CONTROL" = 1 ]; then embedded_control_leg; fi
    launch_game "$GAME" \
        --world "$WORLD" --seed 20261109 --creative --headless --wait --resources "$RES"
else
    launch_game "$GAME" \
        --world "$WORLD" --seed 20261109 --creative --headless --wait \
        --resources "$RES" --dev-mod "$CONTENT"
fi
ctl wait-ready 120
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi

echo "== 1. the defaults are upstream's =="
say "/bwncr status"
expect_chat "bwncr: silenceWither=on silenceTrader=on silenceDragon=on silenceLightning=on custom=0 debug=off listen=off" \
    "status reports upstream's defaults"

echo "== 2. the policy silences the upstream names and only them =="
say "/bwncr check entity.wither.spawn"
expect_chat "bwncr: entity.wither.spawn -> SILENCE (silenceWither)" "wither spawn"
if [ "$NEGATIVE" = 1 ]; then
    # A scenario that cannot fail is not evidence: this run MUST exit 1.
    expect_chat "bwncr: entity.wither.spawn -> ALLOW" "NEGATIVE=1 expects an allow"
    echo "NEGATIVE CONTROL DID NOT FAIL"
    exit 1
fi
say "/bwncr check minecraft:entity.wither.death"
expect_chat "bwncr: minecraft:entity.wither.death -> SILENCE (silenceWither)" "wither death, prefixed"
say "/bwncr check ENTITY.WITHER.DEATH"
expect_chat "bwncr: ENTITY.WITHER.DEATH -> SILENCE (silenceWither)" "wither death, case-insensitive"
say "/bwncr check minecraft:entity.llama.ambient"
expect_chat "bwncr: minecraft:entity.llama.ambient -> SILENCE (silenceTrader)" "llama ambient"
say "/bwncr check entity.ender_dragon.death"
expect_chat "bwncr: entity.ender_dragon.death -> SILENCE (silenceDragon)" "dragon death"
say "/bwncr check minecraft:entity.lightning_bolt.thunder"
expect_chat "bwncr: minecraft:entity.lightning_bolt.thunder -> SILENCE (silenceLightning)" "thunder"
say "/bwncr check minecraft:entity.cow.ambient"
expect_chat "bwncr: minecraft:entity.cow.ambient -> ALLOW" "an unrelated sound is allowed"

echo "== 3. the custom list edits and rewrites the config =="
say "/bwncr silence entity.cow.ambient"
expect_chat "bwncr: silenceUs += entity.cow.ambient (now 1)" "custom list append"
say "/bwncr check minecraft:entity.cow.ambient"
expect_chat "bwncr: minecraft:entity.cow.ambient -> SILENCE (silenceUs)" "custom rule"
say "/bwncr allow entity.cow.ambient"
expect_chat "bwncr: silenceUs -= entity.cow.ambient (now 0)" "custom list remove"
say "/bwncr check minecraft:entity.cow.ambient"
expect_chat "bwncr: minecraft:entity.cow.ambient -> ALLOW" "custom rule removed"
CONFIG_FILE=$SCRATCH/config/bwncr-common.toml
[ -f "$CONFIG_FILE" ] || fail "the config file was not written: $CONFIG_FILE"
grep -q '^silenceWither = true$' "$CONFIG_FILE" || fail "config lost silenceWither=true"
grep -q '^silenceUs = \[""\]$' "$CONFIG_FILE" || fail "config lost the default silenceUs entry"
echo "OK: config $CONFIG_FILE carries the defaults"

echo "== 4. /bwncr listen observes the sound packets the server sends =="
say "/bwncr listen"
expect_chat "bwncr: listen on" "listen on"
ctl command "playsound minecraft:entity.cow.ambient master @a 0 0 0 1 1" >/dev/null
ctl wait 6 >/dev/null
say "/bwncr heard 8"
expect_chat "minecraft:entity.cow.ambient" "the played sound reached the feed"
say "/bwncr listen"
expect_chat "bwncr: listen off" "listen off"

echo "== 5. the carried .hmod loads on the shared, unmodded game =="
ctl quit >/dev/null 2>&1 || true
sleep 2
rm -f "$SOCK"
if [ ! -f "$CONTENT/bwncr.hmod" ]; then
    fail "missing content package: $CONTENT/bwncr.hmod"
fi
SCRATCH2=$(mktemp -d /tmp/opencode/bwncr-content.XXXXXX)
mkdir -p "$SCRATCH2"
(
    cd "$SCRATCH2"
    export HORIZON_HOME=$SCRATCH2/game
    export HORIZON_LAUNCHER_HOME=$SCRATCH2/launcher
    export XDG_DATA_HOME=$SCRATCH2/data
    export XDG_CONFIG_HOME=$SCRATCH2/config
    export HORIZON_NO_AUDIO=1
    export HORIZON_LAUNCHER_NO_SELF_UPDATE=1
    launch_game "$SHARED_GAME" \
        --world BwncrContent --seed 20261109 --creative --headless --wait \
        --resources "$RES" --dev-mod "$CONTENT/bwncr.hmod"
    "$CTL" --socket "$SOCK" wait-ready 120 >/dev/null
    ctl wait 10 >/dev/null
    log=$SCRATCH2/game/logs/latest.log
    grep -q "content from 1 mod(s)" "$log" || fail "the .hmod did not load as one content mod"
    if grep -q "mod bwncr content not loaded" "$log"; then
        fail "the .hmod failed to load"
    fi
    echo "OK: the .hmod loaded (content from 1 mod(s))"
    ctl chat "/bwncr status" >/dev/null
    ctl wait 4 >/dev/null
    lines=$(chat_lines)
    if grep -qF -- "bwncr:" <<<"$lines"; then
        fail "the unmodded game answered a bwncr command"
    fi
    echo "OK: the shared game (content only) has no bwncr command"
    ctl quit >/dev/null 2>&1 || true
)
rm -rf "$SCRATCH2" 2>/dev/null || true
SCRATCH2=""

echo "== 6. control: the shared game without the content has no bwncr command =="
rm -f "$SOCK"
SCRATCH2=$(mktemp -d /tmp/opencode/bwncr-control.XXXXXX)
mkdir -p "$SCRATCH2"
(
    cd "$SCRATCH2"
    export HORIZON_HOME=$SCRATCH2/game
    export HORIZON_LAUNCHER_HOME=$SCRATCH2/launcher
    export XDG_DATA_HOME=$SCRATCH2/data
    export XDG_CONFIG_HOME=$SCRATCH2/config
    export HORIZON_NO_AUDIO=1
    export HORIZON_LAUNCHER_NO_SELF_UPDATE=1
    launch_game "$SHARED_GAME" \
        --world BwncrControl --seed 20261109 --creative --headless --wait \
        --resources "$RES"
    "$CTL" --socket "$SOCK" wait-ready 120 >/dev/null
    ctl chat "/bwncr status" >/dev/null
    ctl wait 4 >/dev/null
    lines=$(chat_lines)
    echo "--- control chat ---"
    echo "$lines"
    if grep -qF -- "bwncr:" <<<"$lines"; then
        fail "the unmodded game answered a bwncr command"
    fi
    echo "OK: the unmodded game has no bwncr command"
    ctl quit >/dev/null 2>&1 || true
)
rm -rf "$SCRATCH2" 2>/dev/null || true
SCRATCH2=""

if [ "$EMBEDDED" = 1 ]; then
    echo "PASS: one bundle, no --dev-mod: defaults, upstream rules, custom list, config file and listen feed all work, the embedded content loaded as 1 content mod, and the control loaded none while the code still answered"
else
    echo "PASS: defaults, upstream rules, custom list, config file, listen feed, hmod load, unmodded control"
fi
