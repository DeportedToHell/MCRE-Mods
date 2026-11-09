// Packs Falling Leaves' particle textures (tools/textures/, the original's) into
// mixins/fallingleaves/data/tiles.rgba: one row of 5 by 5 tiles with a clear
// pixel between them, in the order particle.rs counts them, as raw RGBA.
// `node tools/tiles.mjs`
import { writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { decode } from "./png.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const names = [
  "falling_leaf_1", "falling_leaf_2", "falling_leaf_3", "falling_leaf_4", "falling_leaf_5",
  "falling_leaf_conifer_1", "falling_leaf_conifer_2", "falling_leaf_conifer_3",
  "falling_snow",
];
const TILE = 5, STEP = 7;
const width = names.length * STEP, height = TILE;
const out = Buffer.alloc(width * height * 4);
names.forEach((name, i) => {
  const png = decode(join(root, "tools", "textures", `${name}.png`));
  if (png.width !== TILE || png.height !== TILE) throw new Error(`${name}: ${png.width}x${png.height}`);
  png.rgba.forEach(([r, g, b, a], p) => {
    const x = i * STEP + (p % TILE), y = Math.floor(p / TILE);
    out.set([r, g, b, a], (y * width + x) * 4);
  });
});
writeFileSync(join(root, "mixins", "fallingleaves", "data", "tiles.rgba"), out);
console.log(`tiles.rgba: ${width}x${height}`);
