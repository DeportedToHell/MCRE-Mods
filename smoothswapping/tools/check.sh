#!/usr/bin/env bash
# Builds the mod into the pinned Horizon source the way the launcher does, and
# `cargo check`s the game with it: `tools/check.sh` (add `--with-installed` to
# stage the player's other installed native mods beside it, `--build` for a
# full `fast` build instead of a check).
#
# Uses the launcher's own Rust, horizon-mixins and crate cache (read only) and
# works in $SMOOTHSWAPPING_BUILD (default: %TEMP%/smoothswapping-build), never in the launcher's folders.
set -euo pipefail

here=$(cd "$(dirname "$0")/.." && pwd)
launcher="$APPDATA/horizon-launcher"
rev=2ebf00a0281c592f79191e76fce0e7d522c6f425
source="$launcher/native/source/$rev"
mixins="$launcher/native/tools/5a64af938ec8/bin/horizon-mixins.exe"
rust="$launcher/rust/1.99.1-x86_64-pc-windows-gnu"
out="${SMOOTHSWAPPING_BUILD:-$TEMP/smoothswapping-build}"

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
cp -r "$here/mixins/smoothswapping" "$out/mods/"
if [ "$with_installed" = 1 ]; then
  for dir in "$launcher"/native/installed.incoming/*/mixins/*; do
    [ "$(basename "$dir")" = smoothswapping ] || cp -r "$dir" "$out/mods/"
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
grep -E 'smoothswapping' "$out/cargo.log" | grep -E 'error|warning' || echo "(none)"
exit "$status"
