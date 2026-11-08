// Makes mixins/justenoughitems/sprites.rs from JEI's GUI sprites:
//   node tools/sprites.mjs <JEI checkout> [out.rs]
// JEI's textures (Common/src/main/resources/assets/jei/textures/gui/sprites)
// are MIT, as the rest of JEI. The mod draws them pixel by pixel with the
// game's fills, so it needs no texture of its own in the game's atlas.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [jei, out = new URL("../mixins/justenoughitems/sprites.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(jei, "Common/src/main/resources/assets/jei/textures/gui/sprites");

// The sprites the mod draws, with their nine-slice border where JEI stretches them.
const SPRITES = [
  ["GUI_BACKGROUND", "gui_background_v2"],
  ["RECIPE_BACKGROUND", "single_recipe_background_v2"],
  ["CATALYST_TAB", "catalyst_tab_v2"],
  ["CATALYST_SLOT", "recipe_catalyst_slot_background_v2"],
  ["TAB_SELECTED", "tab_selected"],
  ["TAB_UNSELECTED", "tab_unselected"],
  ["SLOT", "slot"],
  ["OUTPUT_SLOT", "output_slot"],
  ["RECIPE_ARROW", "recipe_arrow"],
  ["RECIPE_ARROW_FILLED", "recipe_arrow_filled"],
  ["RECIPE_PLUS", "recipe_plus_sign"],
  ["BUTTON_PRESSED", "button_pressed_v2"],
  ["BUTTON_PRESSED_HIGHLIGHT", "button_pressed_highlight_v2"],
  ["BREWING_BACKGROUND", "brewing_stand_background_v2"],
  ["BREWING_ARROW", "brewing_stand_arrow"],
  ["BREWING_BUBBLES", "brewing_stand_bubbles"],
  ["BREWING_HEAT", "brewing_stand_blaze_heat"],
  ["ICON_SHAPELESS", "icons/shapeless_icon_v2"],
  ["ICON_TRANSFER", "icons/recipe_transfer"],
  ["ICON_CONFIG", "icons/config_button"],
  ["ICON_CONFIG_CHEAT", "icons/config_button_cheat"],
  ["ICON_BOOKMARK_ON", "icons/bookmark_button_enabled"],
  ["ICON_BOOKMARK_OFF", "icons/bookmark_button_disabled"],
  ["ICON_FLAME", "icons/flame"],
  ["ICON_FLAME_EMPTY", "icons/flame_empty"],
  ["ICON_LIST", "icons/list_badge"],
  ["ICON_TAG", "icons/tag_badge"],
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

function mcmeta(file) {
  if (!existsSync(file)) return null;
  const meta = JSON.parse(readFileSync(file, "utf8"));
  const s = meta?.gui?.scaling;
  if (!s || s.type !== "nine_slice") return null;
  const b = s.border;
  const [left, top, right, bottom] = typeof b === "number" ? [b, b, b, b] : [b.left, b.top, b.right, b.bottom];
  return { left, top, right, bottom };
}

let rs = `// JEI's GUI sprites (MIT, https://github.com/mezz/JustEnoughItems,
// Common/src/main/resources/assets/jei/textures/gui/sprites), as runs of
// pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) height: u16,
    /// Nine-slice borders (left, top, right, bottom) where JEI stretches it.
    pub(super) border: Option<[u16; 4]>,
    pub(super) runs: &'static [(u8, u8, u8, u32)],
    /// Every pixel, row by row (for stretching).
    pub(super) pixels: &'static [u32],
}
`;
for (const [name, file] of SPRITES) {
  const png = join(dir, `${file}.png`);
  const { width, height, rgba } = decode(png);
  const border = mcmeta(`${png}.mcmeta`);
  const packed = rgba.map(([r, g, b, a]) => ((a << 24) | (b << 16) | (g << 8) | r) >>> 0);
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
  rs += `\n/// ${file}.png, ${width}x${height}${border ? `, nine-slice ${border.left} ${border.top} ${border.right} ${border.bottom}` : ""}.
pub(super) const ${name}: Sprite = Sprite {
    width: ${width},
    height: ${height},
    border: ${border ? `Some([${border.left}, ${border.top}, ${border.right}, ${border.bottom}])` : "None"},
    runs: &[${runs.join(",")}],
    pixels: &[${pixels.join(",")}],
};
`;
  console.log(`${name}: ${width}x${height}${border ? ` border ${JSON.stringify(border)}` : ""}, ${runs.length} runs`);
}
writeFileSync(out, rs);
