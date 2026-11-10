// Draws the mod's icon, a pixel-art lantern in its own glow, into
// mixins/dynamiclights/data/icon.png: `node tools/icon.mjs`.
// 32x32 art pixels, each drawn 2x2, so the file is 64x64.
import { writeFileSync, mkdirSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const N = 32;
const SCALE = 2;
const px = new Array(N * N).fill(null).map(() => [0, 0, 0, 0]);

const hex = (h, a = 255) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16), a];
// "Over" blending of a colour onto a pixel.
function put(x, y, c) {
  if (x < 0 || y < 0 || x >= N || y >= N) return;
  const d = px[y * N + x];
  const a = c[3] / 255;
  const da = d[3] / 255;
  const oa = a + da * (1 - a);
  if (oa === 0) return;
  for (let i = 0; i < 3; i++) d[i] = Math.round((c[i] * a + d[i] * da * (1 - a)) / oa);
  d[3] = Math.round(oa * 255);
}
const rect = (x0, y0, x1, y1, c) => {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) put(x, y, c);
};

// The glow: warm rings around the flame, fading out in steps.
const [gx, gy] = [15.5, 17.5];
for (let y = 0; y < N; y++) {
  for (let x = 0; x < N; x++) {
    const d = Math.hypot(x + 0.5 - gx - 0.5, y + 0.5 - gy - 0.5);
    const band = Math.floor(d / 2.5);
    const alpha = [0, 0, 150, 115, 85, 60, 38, 20][band] ?? 0;
    if (alpha) put(x, y, [255, 196, 92, alpha]);
  }
}

const iron = hex("#26262e");
const ironMid = hex("#44444f");
const ironLight = hex("#6c6c7a");
const glass = hex("#ffcf6e", 170);

// Hanging ring.
rect(14, 3, 17, 3, iron);
rect(13, 4, 13, 6, iron);
rect(18, 4, 18, 6, iron);
rect(14, 4, 17, 4, ironLight);
rect(15, 7, 16, 8, iron);
// Cap.
rect(11, 9, 20, 9, iron);
rect(10, 10, 21, 11, ironMid);
rect(11, 10, 20, 10, ironLight);
rect(10, 12, 21, 12, iron);
// Glass case and its frame.
rect(11, 13, 20, 23, glass);
rect(11, 13, 11, 23, iron);
rect(20, 13, 20, 23, iron);
rect(15, 13, 16, 13, iron);
// The flame.
const flame = [
  [15, 14, 16, 14, "#ff9a2e"],
  [14, 15, 17, 15, "#ffb03c"],
  [14, 16, 17, 16, "#ffc94f"],
  [13, 17, 18, 17, "#ffb03c"],
  [13, 18, 18, 18, "#ffd866"],
  [13, 19, 18, 19, "#ffc94f"],
  [14, 20, 17, 20, "#ff9a2e"],
  [14, 21, 17, 21, "#e8741c"],
];
for (const [x0, y, x1, , c] of flame.map((f) => [f[0], f[1], f[2], f[3], f[4]])) rect(x0, y, x1, y, hex(c));
rect(15, 16, 16, 19, hex("#fff2b0"));
rect(15, 17, 16, 18, hex("#fffbe6"));
// Candle stub under the flame.
rect(15, 22, 16, 23, hex("#e9dcc0"));
// Glass glints.
rect(12, 14, 12, 16, hex("#fff6d8", 200));
rect(19, 20, 19, 21, hex("#fff6d8", 140));
// Base.
rect(10, 24, 21, 24, iron);
rect(10, 25, 21, 26, ironMid);
rect(11, 25, 20, 25, ironLight);
rect(11, 27, 20, 27, iron);

// PNG out.
const W = N * SCALE;
const raw = Buffer.alloc((W * 4 + 1) * W);
for (let y = 0; y < W; y++) {
  raw[y * (W * 4 + 1)] = 0;
  for (let x = 0; x < W; x++) {
    const p = px[Math.floor(y / SCALE) * N + Math.floor(x / SCALE)];
    raw.set(p, y * (W * 4 + 1) + 1 + x * 4);
  }
}
const table = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = table[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const c = Buffer.alloc(4);
  c.writeUInt32BE(crc(td));
  return Buffer.concat([len, td, c]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(W, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);
const out = join(root, "mixins", "dynamiclights", "data", "icon.png");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log(`${out}: ${png.length} bytes`);
