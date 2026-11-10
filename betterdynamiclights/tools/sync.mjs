// node sync.mjs <from> <to>: makes <to> a copy of <from>, writing only the
// files whose bytes differ (unchanged files keep their times, so cargo keeps
// what it compiled) and removing the ones <from> no longer has.
import { readdirSync, readFileSync, writeFileSync, mkdirSync, rmSync, statSync, existsSync } from "node:fs";
import { join } from "node:path";

const [from, to] = process.argv.slice(2);
let written = 0;
let removed = 0;

function walk(dir, rel = "") {
  const out = new Map();
  for (const e of readdirSync(join(dir, rel), { withFileTypes: true })) {
    const r = rel ? `${rel}/${e.name}` : e.name;
    if (e.isDirectory()) for (const [k, v] of walk(dir, r)) out.set(k, v);
    else out.set(r, true);
  }
  return out;
}

const source = walk(from);
const target = existsSync(to) ? walk(to) : new Map();
for (const rel of source.keys()) {
  const a = join(from, rel);
  const b = join(to, rel);
  const bytes = readFileSync(a);
  if (target.has(rel) && statSync(b).size === bytes.length && readFileSync(b).equals(bytes)) continue;
  mkdirSync(join(b, ".."), { recursive: true });
  writeFileSync(b, bytes);
  written++;
}
for (const rel of target.keys()) {
  if (!source.has(rel)) {
    rmSync(join(to, rel));
    removed++;
  }
}
console.log(`stage: ${written} written, ${removed} removed, ${source.size} files`);
