// Draws the port's own icon (the original's assets are All Rights Reserved):
// the picked-up list, an apple, a gem and gold with green pluses and counts, into
// mixins/pickupnotifier/data/icon.png (64x64): `node tools/icon.mjs`.
import { writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";
const N = 64;
const hex = (h, a = 255) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16), a];
function C() { const px = new Array(N * N).fill(null).map(() => [0, 0, 0, 0]);
  const blend = (x, y, c) => { if (x < 0 || y < 0 || x >= N || y >= N) return; const d = px[y * N + x], a = c[3] / 255, da = d[3] / 255, oa = a + da * (1 - a); if (!oa) return;
    for (let i = 0; i < 3; i++) d[i] = Math.round((c[i] * a + d[i] * da * (1 - a)) / oa); d[3] = Math.round(oa * 255); };
  const inRound = (x, y, x0, y0, w, h, r) => { const cx = Math.min(Math.max(x, x0 + r), x0 + w - 1 - r), cy = Math.min(Math.max(y, y0 + r), y0 + h - 1 - r); return (x - cx) ** 2 + (y - cy) ** 2 <= r * r + r * 0.6; };
  return { px, blend,
    rr(x0, y0, w, h, r, c, c2) { for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) if (inRound(x, y, x0, y0, w, h, r)) { const t = (y - y0) / Math.max(1, h - 1); blend(x, y, c2 ? c.map((v, i) => Math.round(v + (c2[i] - v) * t)) : c); } },
    rect(x0, y0, x1, y1, c) { for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) blend(x, y, c); },
    art(rows, pal, ox, oy, s = 1) { rows.forEach((r, y) => [...r].forEach((ch, x) => { if (ch === ".") return; for (let j = 0; j < s; j++) for (let i = 0; i < s; i++) blend(ox + x * s + i, oy + y * s + j, pal[ch]); })); },
    plus(cx, cy, len, th, c, sh) { const h = Math.floor(len / 2), t = Math.floor(th / 2); if (sh) { this.rect(cx - h + 1, cy - t + 1, cx + h + 1, cy - t + th, sh); this.rect(cx - t + 1, cy - h + 1, cx - t + th, cy + h + 1, sh); } this.rect(cx - h, cy - t, cx + h, cy - t + th - 1, c); this.rect(cx - t, cy - h, cx - t + th - 1, cy + h, c); } }; }
const DIG = { "1": ["010", "110", "010", "010", "010", "111"], "3": ["110", "001", "010", "001", "001", "110"], "6": ["011", "100", "110", "101", "101", "010"], "4": ["101", "101", "111", "001", "001", "001"], "+": ["000", "010", "111", "010", "000", "000"] };
function txt(g, s, x, y, c, sc = 2, sh) { for (const ch of s) { const d = DIG[ch]; d.forEach((r, j) => [...r].forEach((v, i) => { if (v !== "1") return; if (sh) g.rect(x + i * sc + 1, y + j * sc + 1, x + i * sc + sc, y + j * sc + sc, sh); g.rect(x + i * sc, y + j * sc, x + i * sc + sc - 1, y + j * sc + sc - 1, c); })); x += (d[0].length + 1) * sc; } }
const GEM = [["...OOOOOO...","..OAABBBCO..",".OAABBBBCCO.","OAABBBBBBCCO","OOOOOOOOOOOO","OLLMMMMMMDDO",".OLLMMMMDDO.","..OLLMMDDO..","...OLMMDO...","....OLDO....",".....OO.....","............"],{O:hex("#0b3640"),A:hex("#f2fffd"),B:hex("#9ff5ed"),C:hex("#55d8d2"),L:hex("#c4fcf6"),M:hex("#3cc4be"),D:hex("#1b858a")}];
const APPLE = [[".....OO.....",".....OGG....","...OOOGOO...","..ORRRRRWRO.",".ORRRRRRRWRO",".ORRRRRRRRRO",".ORRRRRRRRRO",".OKRRRRRRRRO",".OKKRRRRRRRO","..OKKRRRRRO.","...OOORROO..","......OO...."],{O:hex("#3d0a06"),R:hex("#e3261b"),W:hex("#ffb0a2"),K:hex("#a5150c"),G:hex("#3b9a22")}];
const GOLD = [["............","............","....OOOOOOO.","...OYYYWWYO.","..OYYYYYYYO.",".OYYYYYYYO..","OBBBBBBBBO..","OOOOOOOOO...","............","............","............","............"],{O:hex("#5a3d00"),Y:hex("#ffd84a"),W:hex("#fff6b8"),B:hex("#c99212")}];

// I: hand-picked list on a dark rounded panel, big crisp items, green pluses and counts.
function I() { const g = C();
  g.rr(2, 2, 60, 60, 10, hex("#2b2f3a"), hex("#161920"));
  g.rr(3, 3, 58, 58, 9, hex("#000000", 0));
  const rows = [[APPLE, "3"], [GEM, "1"], [GOLD, "6"]];
  rows.forEach(([it, n], i) => { const y = 7 + i * 18;
    g.rr(6, y - 2, 52, 16, 4, hex("#ffffff", i === 2 ? 34 : 18));
    g.art(it[0], it[1], 8, y, 1); g.art(it[0], it[1], 8, y - 0, 1);
    g.art(it[0], it[1], 7, y - 1, 1);
    g.plus(30, y + 6, 7, 3, hex("#55ff55"), hex("#1d6b1d"));
    txt(g, n, 37, y + 1, hex("#ffffff"), 2, hex("#000000", 160)); });
  return g.px; }
function png(px, w, h, file, sc = 1) { const W = w * sc, H = h * sc, raw = Buffer.alloc((W * 4 + 1) * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) raw.set(px[Math.floor(y / sc) * w + Math.floor(x / sc)], y * (W * 4 + 1) + 1 + x * 4);
  const T = new Uint32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (b) => { let c = 0xffffffff; for (const v of b) c = T[(c ^ v) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const ch = (t, d) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const hd = Buffer.alloc(13); hd.writeUInt32BE(W, 0); hd.writeUInt32BE(H, 4); hd[8] = 8; hd[9] = 6;
  writeFileSync(file, Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), ch("IHDR", hd), ch("IDAT", deflateSync(raw)), ch("IEND", Buffer.alloc(0))])); }
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const file = join(root, "mixins", "pickupnotifier", "data", "icon.png");
mkdirSync(dirname(file), { recursive: true });
png(I(), N, N, file);
console.log(file);
