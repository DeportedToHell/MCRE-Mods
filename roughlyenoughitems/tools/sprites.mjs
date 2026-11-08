// Makes mixins/roughlyenoughitems/sprites.rs from REI's GUI textures:
//   node tools/sprites.mjs <RoughlyEnoughItems checkout> [out.rs]
// REI's textures (runtime/src/main/resources/assets/roughlyenoughitems/
// textures/gui) are MIT, as the rest of REI. The mod draws them pixel by pixel
// with the game's fills, so it needs no texture of its own in the game's atlas.
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [rei, out = new URL("../mixins/roughlyenoughitems/sprites.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(rei, "runtime/src/main/resources/assets/roughlyenoughitems/textures/gui");

// The parts the mod draws: name, file, region (u, v, w, h) or the whole
// file, and the nine-slice border where REI stretches it (cut down to its
// corners and one middle row and column).
const RC = "recipecontainer.png";
const SPRITES = [
  ["PANEL", "sprites/widget/panel_default.png", null, 4],
  ["PANEL_SLOT", "sprites/widget/panel_slot.png", null, 4],
  ["ARROW_LEFT", "arrow_left.png"],
  ["ARROW_RIGHT", "arrow_right.png"],
  ["GEAR", RC, [0, 0, 14, 14]],
  ["WEATHER_CLEAR", RC, [0, 14, 14, 14]],
  ["WEATHER_RAIN", RC, [14, 14, 14, 14]],
  ["WEATHER_THUNDER", RC, [28, 14, 14, 14]],
  ["TIME_MORNING", RC, [42, 14, 14, 14]],
  ["TIME_NOON", RC, [56, 14, 14, 14]],
  ["TIME_EVENING", RC, [70, 14, 14, 14]],
  ["TIME_NIGHT", RC, [84, 14, 14, 14]],
  // The submenu arrow (always the light one, as REI draws it).
  ["SUBMENU", RC, [0, 28, 18, 18]],
  ["TAB_UNSELECTED", RC, [0, 166, 24, 22]],
  ["TAB_SELECTED", RC, [24, 166, 24, 26]],
  ["SLOT", RC, [0, 222, 18, 18]],
  ["ARROW_EMPTY", "display.png", [106, 91, 24, 17]],
  ["ARROW_FULL", "display.png", [82, 91, 24, 17]],
  ["FIRE_UNLIT", "display.png", [1, 74, 14, 14]],
  ["FIRE_LIT", "display.png", [82, 77, 14, 14]],
  ["BREWING", "display.png", [0, 108, 103, 59]],
  ["BREWING_FUEL", "display.png", [103, 163, 18, 4]],
  ["SHAPELESS", "shapeless_icon_4x.png"],
  ["EXPAND", "expand.png"],
  // The dark theme (appearance.theme = DARK): REI's dark twins, the buttons kept whole (their middles are not plain).
  ["PANEL_DARK", "sprites/widget/panel_default_dark.png", null, 4],
  ["PANEL_SLOT_DARK", "sprites/widget/panel_slot_dark.png", null, 4],
  ["BUTTON_DARK", "sprites/widget/button_dark.png", null, 0, 3],
  ["BUTTON_DISABLED_DARK", "sprites/widget/button_disabled_dark.png", null, 0, 1],
  ["BUTTON_HIGHLIGHTED_DARK", "sprites/widget/button_highlighted_dark.png", null, 0, 3],
  ["TAB_UNSELECTED_DARK", "recipecontainer_dark.png", [0, 166, 24, 22]],
  ["TAB_SELECTED_DARK", "recipecontainer_dark.png", [24, 166, 24, 26]],
  ["SLOT_DARK", "recipecontainer_dark.png", [0, 222, 18, 18]],
  ["ARROW_EMPTY_DARK", "display_dark.png", [106, 91, 24, 17]],
  ["ARROW_FULL_DARK", "display_dark.png", [82, 91, 24, 17]],
  ["FIRE_UNLIT_DARK", "display_dark.png", [1, 74, 14, 14]],
  ["FIRE_LIT_DARK", "display_dark.png", [82, 77, 14, 14]],
  ["BREWING_DARK", "display_dark.png", [0, 108, 103, 59]],
  ["BREWING_FUEL_DARK", "display_dark.png", [103, 163, 18, 4]],
  ["SHRINK", "shrink.png"],
  ["CLIPBOARD", "clipboard.png"],
  // appearance.recipe_border = LIGHTER (NONE draws nothing).
  ["PANEL_LIGHTER", "sprites/widget/panel_lighter.png", null, 4],
  ["PANEL_LIGHTER_DARK", "sprites/widget/panel_lighter_dark.png", null, 4],
  // accessibility.larger_tabs and the compact tab buttons.
  ["TAB_LARGE_UNSELECTED", RC, [0, 192, 28, 26]],
  ["TAB_LARGE_SELECTED", RC, [28, 192, 28, 30]],
  ["TAB_LARGE_UNSELECTED_DARK", "recipecontainer_dark.png", [0, 192, 28, 26]],
  ["TAB_LARGE_SELECTED_DARK", "recipecontainer_dark.png", [28, 192, 28, 30]],
  ["ARROW_LEFT_SMALL", "arrow_left_small.png"],
  ["ARROW_RIGHT_SMALL", "arrow_right_small.png"],
  // The config screen: the categories' icons, the search button's, the values' arrow.
  ["CFG_APPEARANCE", "config/appearance.png"],
  ["CFG_INPUT", "config/input.png"],
  ["CFG_LAYOUT", "config/layout.png"],
  ["CFG_ACCESSIBILITY", "config/accessibility.png"],
  ["CFG_FILTERING", "config/filtering.png"],
  ["CFG_LIST", "config/list.png"],
  ["CFG_FAVORITES", "config/favorites.png"],
  ["CFG_SEARCH", "config/search.png"],
  ["CFG_CHEATS", "config/cheats.png"],
  ["CFG_PERFORMANCE", "config/performance.png"],
  ["CFG_DEBUG", "config/debug.png"],
  ["CFG_RESET", "config/reset.png"],
  ["CFG_SEARCH_OPTIONS", "config/search_options.png"],
  ["CFG_SELECTOR", "config/selector.png"],
];

function decode(file) {
  const buf = readFileSync(file);
  let pos = 8;
  let width = 0, height = 0, depth = 0, type = 0;
  const idat = [];
  let palette = null, trns = null;
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const kind = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (kind === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      depth = data[8];
      type = data[9];
      if (data[12] !== 0) throw new Error(`${file}: interlaced`);
    } else if (kind === "PLTE") palette = data;
    else if (kind === "tRNS") trns = data;
    else if (kind === "IDAT") idat.push(data);
    pos += 12 + len;
  }
  const channels = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[type];
  const raw = inflateSync(Buffer.concat(idat));
  const bits = channels * depth;
  const bpp = Math.max(1, bits >> 3);
  const stride = Math.ceil((width * bits) / 8);
  const bytes = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? bytes[y * stride + x - bpp] : 0;
      const b = y > 0 ? bytes[(y - 1) * stride + x] : 0;
      const c = x >= bpp && y > 0 ? bytes[(y - 1) * stride + x - bpp] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      bytes[y * stride + x] = v & 255;
    }
  }
  // One sample of channel ch of pixel x on row y, scaled to 0..255.
  const sample = (x, y, ch) => {
    const index = x * channels + ch;
    if (depth === 8) return bytes[y * stride + index];
    if (depth === 16) return bytes[y * stride + index * 2];
    const bit = index * depth;
    const byte = bytes[y * stride + (bit >> 3)];
    const v = (byte >> (8 - depth - (bit & 7))) & ((1 << depth) - 1);
    return type === 3 ? v : Math.round((v * 255) / ((1 << depth) - 1));
  };
  const rgba = [];
  for (let i = 0; i < width * height; i++) {
    const p = Array.from({ length: channels }, (_, ch) => sample(i % width, Math.floor(i / width), ch));
    let r, g, b, a = 255;
    if (type === 6) [r, g, b, a] = p;
    else if (type === 2) [r, g, b] = p;
    else if (type === 4) [r, a] = p, g = b = r;
    else if (type === 0) r = g = b = p[0];
    else if (type === 3) {
      [r, g, b] = [palette[p[0] * 3], palette[p[0] * 3 + 1], palette[p[0] * 3 + 2]];
      a = trns && p[0] < trns.length ? trns[p[0]] : 255;
    }
    rgba.push([r, g, b, a]);
  }
  return { width, height, rgba };
}

let rs = `// REI's GUI textures (MIT, https://github.com/shedaniel/RoughlyEnoughItems,
// runtime/src/main/resources/assets/roughlyenoughitems/textures/gui), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) height: u16,
    /// Nine-slice borders (left, top, right, bottom) where REI stretches it.
    pub(super) border: Option<[u16; 4]>,
    pub(super) runs: &'static [(u8, u8, u8, u32)],
    /// Every pixel, row by row (for stretching and partial draws).
    pub(super) pixels: &'static [u32],
}
`;
for (const [name, file, region, nine, tiled] of SPRITES) {
  const png = decode(join(dir, file));
  let [u, v, width, height] = region ?? [0, 0, png.width, png.height];
  let at = (x, y) => png.rgba[(v + y) * png.width + u + x];
  if (nine) {
    // Corners as they are, one middle column and row: REI's panels are plain
    // colours between their borders.
    const [w0, h0] = [width, height];
    const pick = (i, size) => (i < nine ? i : i === nine ? nine : size - (2 * nine + 1 - i));
    const src = at;
    at = (x, y) => src(pick(x, w0), pick(y, h0));
    width = height = 2 * nine + 1;
  }
  const packed = [];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [r, g, b, a] = at(x, y);
      packed.push(((a << 24) | (b << 16) | (g << 8) | r) >>> 0);
    }
  }
  const runs = [];
  for (let y = 0; y < height; y++) {
    let x = 0;
    while (x < width) {
      const c = packed[y * width + x];
      if (c >>> 24 === 0) { x++; continue; }
      let len = 1;
      while (x + len < width && packed[y * width + x + len] === c && len < 255) len++;
      runs.push(`(${x},${y},${len},0x${c.toString(16).padStart(8, "0")})`);
      x += len;
    }
  }
  const pixels = packed.map((c) => `0x${c.toString(16).padStart(8, "0")}`);
  const where = region ? ` (${region.join(", ")})` : "";
  rs += `\n/// ${file}${where}, ${width}x${height}${nine ? `, nine-slice ${nine}` : ""}.
pub(super) const ${name}: Sprite = Sprite {
    width: ${width},
    height: ${height},
    border: ${nine || tiled ? `Some([${nine || tiled}, ${nine || tiled}, ${nine || tiled}, ${nine || tiled}])` : "None"},
    runs: &[${runs.join(",")}],
    pixels: &[${pixels.join(",")}],
};
`;
  console.log(`${name}: ${width}x${height}, ${runs.length} runs`);
}
writeFileSync(out, rs);
