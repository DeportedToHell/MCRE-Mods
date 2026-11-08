// Draws the port's own icon (not the original's: its assets are All Rights
// Reserved): a picked-up gem with a green "+" on a notification panel, into
// mixins/pickupnotifier/data/icon.png: `node tools/icon.mjs`.
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
const put = (x, y, c) => {
  if (x >= 0 && y >= 0 && x < N && y < N) px[y * N + x] = c;
};
const rect = (x0, y0, x1, y1, c) => {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) put(x, y, c);
};

// The panel: a rounded dark square with a violet frame, like a tooltip.
rect(2, 1, 29, 30, hex("#2c1f5c"));
rect(1, 2, 30, 29, hex("#2c1f5c"));
rect(3, 2, 28, 29, hex("#160f26"));
rect(2, 3, 29, 28, hex("#160f26"));
rect(3, 3, 28, 3, hex("#4b2fa0"));
rect(3, 3, 3, 28, hex("#3a2580"));

// A dropped-item shadow under the gem.
rect(7, 25, 18, 25, hex("#0b0814"));
rect(9, 26, 16, 26, hex("#0b0814"));

// The gem.
const gem = [
  "....OOOOOO....",
  "...OAABBBCO...",
  "..OAABBBBCCO..",
  ".OAABBBBBBCCO.",
  "OOOOOOOOOOOOOO",
  "OLLMMMMMMMMDDO",
  ".OLLMMMMMMDDO.",
  "..OLLMMMMDDO..",
  "...OLLMMDDO...",
  "....OLMMDO....",
  ".....OLDO.....",
  "......OO......",
];
const pal = { O: hex("#0d3b46"), A: hex("#e6fffc"), B: hex("#8ff3ea"), C: hex("#4fd3cf"), L: hex("#b9fbf5"), M: hex("#3ec6c0"), D: hex("#1f8f93") };
gem.forEach((row, y) => [...row].forEach((ch, x) => ch !== "." && put(5 + x, 11 + y, pal[ch])));

// The "+": picked up.
const plus = hex("#55ff55");
const dark = hex("#1f6b1f");
rect(22, 5, 24, 13, dark);
rect(19, 8, 27, 10, dark);
rect(22, 5, 23, 12, plus);
rect(19, 8, 26, 9, plus);

// Two text lines of the notification.
rect(20, 17, 27, 17, hex("#ffffff"));
rect(20, 20, 25, 20, hex("#aaaaaa"));

const W = N * SCALE;
const raw = Buffer.alloc((W * 4 + 1) * W);
for (let y = 0; y < W; y++) for (let x = 0; x < W; x++) raw.set(px[Math.floor(y / SCALE) * N + Math.floor(x / SCALE)], y * (W * 4 + 1) + 1 + x * 4);
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
ihdr[8] = 8;
ihdr[9] = 6;
const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0))]);
const out = join(root, "mixins", "pickupnotifier", "data", "icon.png");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, png);
console.log(`${out}: ${png.length} bytes`);
