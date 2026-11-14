#!/usr/bin/env bash
# Builds the mod into the pinned Horizon source the way the launcher does, and
# `cargo check`s the game with it: `tools/check.sh` (add `--with-installed` to
# stage the player's other installed native mods beside it, `--build` for a
# full `fast` build instead of a check).
#
# Uses the launcher's own Rust, horizon-mixins and crate cache (read only) and
# works in $FALLINGLEAVES_BUILD (default: %TEMP%/fallingleaves-build), never in the launcher's folders.
# The Horizon source is the launcher's export of the pinned revision, or $HORIZON_SOURCE (with its
# crate cache in $HORIZON_CARGO_HOME).
set -euo pipefail

here=$(cd "$(dirname "$0")/.." && pwd)
launcher="$APPDATA/horizon-launcher"
rev=9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0
source="${HORIZON_SOURCE:-$launcher/native/source/$rev}"
mixins="$launcher/native/tools/5a64af938ec8/bin/horizon-mixins.exe"
rust="$launcher/rust/1.99.1-x86_64-pc-windows-gnu"
out="${FALLINGLEAVES_BUILD:-$TEMP/fallingleaves-build}"

with_installed=0
mode=check
for arg in "$@"; do
  case "$arg" in
    --with-installed) with_installed=1 ;;
    --build) mode=build ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

mkdir -p "$out"
export PATH="$rust/bin:$rust/mingw64/bin:$PATH"
export CARGO_HOME="${HORIZON_CARGO_HOME:-$launcher/cargo}"
export CARGO_TARGET_DIR="$out/target"
rm -rf "${out:?}/mods" "${out:?}/stage.new"
mkdir -p "$out/mods"
cp -r "$here/mixins/fallingleaves" "$out/mods/"
if [ "$with_installed" = 1 ]; then
  for dir in "$launcher"/native/installed.incoming/*/mixins/*; do
    [ "$(basename "$dir")" = fallingleaves ] || cp -r "$dir" "$out/mods/"
  done
fi

"$mixins" apply --workspace "$source" --mods "$out/mods" --out "$out/stage.new" > "$out/apply.json"
grep -q '"status": "ok"' "$out/apply.json" || { cat "$out/apply.json"; exit 1; }

# Copy only the files that changed, so cargo keeps what it compiled before.
node "$here/tools/sync.mjs" "$out/stage.new" "$out/stage"

if [ "$mode" = build ]; then
  cargo build --manifest-path "$out/stage/Cargo.toml" --locked --offline --profile fast \
    -p horizon-client --bin horizon-client --message-format short 2>&1 | tee "$out/cargo.log"
else
  cargo check --manifest-path "$out/stage/Cargo.toml" --locked --offline \
    -p horizon-client --bin horizon-client --message-format short 2>&1 | tee "$out/cargo.log"
fi
status=${PIPESTATUS[0]}
echo "--- warnings and errors in the mod:"
# Any error is the mod's (Horizon builds clean); its warnings are those in its
# modules (spliced into game.rs, particles.rs and animate_tick.rs) and helpers.
{
  grep -E '^crates.*: error' "$out/cargo.log"
  grep -E '^crates.*: warning' "$out/cargo.log" | grep -E 'fallingleaves|game.rs:(1[0-9]{5}|[2-9][0-9]{4})' 
} | sort -u | grep . || echo "(none)"
exit "$status"
