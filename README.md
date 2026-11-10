# bwncr (Horizon port)

A Horizon port of [bwncr](https://gitlab.com/kreezxil/BadWitherNoCookie) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- Analysis clone: `https://gitlab.com/kreezxil/BadWitherNoCookie` branch `1.21.1`
- at `112fa60559f1152141eb38515c9b85ada36b3b9f` (`2026-05-24`, version 3.20.4 —
- the jar's build); the clone was deleted after these notes.
- A sound about to play is replaced with nothing when its name matches
- (`SoundHandler.java:27-59`):
- `entity.wither.spawn` (exact) or `entity.wither.death` (case-insensitive)
- and `silenceWither`;
- `entity.wandering_trader.ambient` or `entity.llama.ambient`

## Not yet ported

- The suppression itself is not carried.** The pin has no client "a sound is
- about to play" hook: `native-hooks.json` has no sound/audio hook, and
- `play_sound_to` is still `@API:play_sound_to@` (pending, unstamped). The
- named hook request is NeoForge's `PlaySoundEvent` — sound id, category,
- position, volume, pitch, **cancellable**. Until it is stamped the port cannot
- silence anything; `/bwncr` reports the decisions the hook will use.
- `/bwncr listen` prints through `/bwncr heard`, not live.** Upstream sends
- each name to the player's chat as it arrives (client-side). The port records

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: https://gitlab.com/kreezxil/BadWitherNoCookie. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
