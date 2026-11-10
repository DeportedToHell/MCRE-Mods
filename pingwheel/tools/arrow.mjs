// Makes mixins/pingwheel/data/arrow32.rgba from the original's textures/arrow.png (64 x 64,
// copied to tools/assets/): boxed down to 32 x 32 raw RGBA for the 32 rows of the mod-image
// slot Ping Wheel has (rows 976..1008). It is drawn 2.5 to 7.5 GUI pixels wide, so 32 is plenty.
// `node tools/arrow.mjs`
import { writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { decode } from "./png.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const png = decode(join(root, "tools", "assets", "arrow.png"));
const SIDE = 32;
const k = png.width / SIDE;
if (png.width !== png.height || !Number.isInteger(k)) throw new Error(`arrow.png: ${png.width}x${png.height}`);
const out = Buffer.alloc(SIDE * SIDE * 4);
for (let y = 0; y < SIDE; y++) {
  for (let x = 0; x < SIDE; x++) {
    const sum = [0, 0, 0, 0];
    for (let dy = 0; dy < k; dy++) {
      for (let dx = 0; dx < k; dx++) {
        const [r, g, b, a] = png.rgba[(y * k + dy) * png.width + x * k + dx];
        sum[0] += r * a; sum[1] += g * a; sum[2] += b * a; sum[3] += a;
      }
    }
    const o = (y * SIDE + x) * 4;
    if (sum[3] > 0) for (let c = 0; c < 3; c++) out[o + c] = Math.round(sum[c] / sum[3]);
    out[o + 3] = Math.round(sum[3] / (k * k));
  }
}
writeFileSync(join(root, "mixins", "pingwheel", "data", "arrow32.rgba"), out);
console.log(`arrow32.rgba: ${SIDE}x${SIDE}`);
