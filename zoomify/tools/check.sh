#!/usr/bin/env bash
# Builds the mod into the pinned Horizon source the way the launcher does, and
# `cargo check`s the game with it: `tools/check.sh` (add `--with-installed` to
# stage the player's other installed native mods beside it, `--build` for a
# full `fast` build instead of a check).
#
# Uses the launcher's own Rust, horizon-mixins and crate cache (read only) and
# works in $ZOOMIFY_BUILD (default: %TEMP%/zoomify-build), never in the launcher's folders.
# The Horizon source is the launcher's export of the pinned revision, or $HORIZON_SOURCE.
set -euo pipefail

here=$(cd "$(dirname "$0")/.." && pwd)
launcher="$APPDATA/horizon-launcher"
rev=fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca
source="${HORIZON_SOURCE:-$launcher/native/source/$rev}"
mixins="$launcher/native/tools/5a64af938ec8/bin/horizon-mixins.exe"
rust="$launcher/rust/1.99.1-x86_64-pc-windows-gnu"
out="${ZOOMIFY_BUILD:-$TEMP/zoomify-build}"

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
export CARGO_HOME="$launcher/cargo"
export CARGO_TARGET_DIR="$out/target"
rm -rf "${out:?}/mods" "${out:?}/stage.new"
mkdir -p "$out/mods"
cp -r "$here/mixins/zoomify" "$out/mods/"
if [ "$with_installed" = 1 ]; then
  for dir in "$launcher"/native/installed.incoming/*/mixins/*; do
    [ "$(basename "$dir")" = zoomify ] || cp -r "$dir" "$out/mods/"
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
# The mod's modules are spliced at the end of game.rs: what is reported from
# there, and anything naming one of its items elsewhere (its hook helpers).
first=$(grep -n "^pub(crate) mod zoomify" "$out/stage/crates/horizon-client/src/game.rs" | head -1 | cut -d: -f1)
{
  grep -E '^crates.horizon-client.src.game.rs:[0-9]+:[0-9]+: (warning|error)' "$out/cargo.log" |
    awk -F: -v first="${first:-999999999}" '$2 + 0 >= first + 0'
  grep -E '^crates.*: (warning|error).*zoomify' "$out/cargo.log" | grep -v '^crates.horizon-client.src.game.rs:'
} | grep . || echo "(none)"
exit "$status"
