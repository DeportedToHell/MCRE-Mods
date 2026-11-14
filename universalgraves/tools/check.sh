#!/usr/bin/env bash
# Builds the mod into the pinned Horizon source the way the launcher does, and
# `cargo check`s the game with it: `tools/check.sh` (add `--with-installed` to
# stage the player's other installed native mods beside it, `--build` for a
# full `fast` build instead of a check, `--fetch` to fetch the crates first).
#
# Uses the launcher's own Rust and horizon-mixins (read only) and works in
# $UG_BUILD (default: %TEMP%/universalgraves-build), never in the launcher's folders.
# The Horizon source is the launcher's export of the pinned revision, or a git
# checkout of horizon-rust/horizon at it in $HORIZON_SOURCE; the crates come
# from the launcher's cache, or from $UG_CARGO_HOME (with such a checkout:
# run once with --fetch).
set -euo pipefail

here=$(cd "$(dirname "$0")/.." && pwd)
launcher="$APPDATA/horizon-launcher"
rev=9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0
source="${HORIZON_SOURCE:-$launcher/native/source/$rev}"
mixins="$launcher/native/tools/5a64af938ec8/bin/horizon-mixins.exe"
rust="$launcher/rust/1.99.1-x86_64-pc-windows-gnu"
out="${UG_BUILD:-$TEMP/universalgraves-build}"

with_installed=0
mode=check
fetch=0
for arg in "$@"; do
  case "$arg" in
    --with-installed) with_installed=1 ;;
    --build) mode=build ;;
    --fetch) fetch=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

[ -f "$source/Cargo.toml" ] || { echo "no Horizon source for $rev at $source (AGENT-GUIDE.md, Setup)"; exit 1; }
mkdir -p "$out"
export PATH="$rust/bin:$rust/mingw64/bin:$PATH"
export CARGO_HOME="${UG_CARGO_HOME:-$launcher/cargo}"
export CARGO_TARGET_DIR="$out/target"
if [ "$fetch" = 1 ]; then
  cargo fetch --manifest-path "$source/Cargo.toml" --locked
fi
rm -rf "${out:?}/mods" "${out:?}/stage.new"
mkdir -p "$out/mods"
cp -r "$here/mixins/universalgraves" "$out/mods/"
if [ "$with_installed" = 1 ]; then
  for dir in "$launcher"/native/installed.incoming/*/mixins/*; do
    [ "$(basename "$dir")" = universalgraves ] || cp -r "$dir" "$out/mods/"
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
# The mod's modules are spliced into game.rs.
grep -aE '^crates[\/]horizon-client[\/]src[\/](game|lan[\/]containers).rs.*(error|warning)' "$out/cargo.log" || echo "(none)"
exit "$status"
