// Packs the mod into <out>/<id>-<version>.hnmod: `node tools/package.mjs [out]`.
// The bundle is horizon-native.json and mixins/<id>/ (its .rs and .toml files,
// LICENSE, NOTICE and the data/ folder), zipped with fixed dates so the same
// source always gives the same file.
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync, statSync } from "node:fs";
import { deflateRawSync } from "node:zlib";
import { join, dirname } from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(join(root, "horizon-native.json"), "utf8"));
const out = process.argv[2] ?? join(root, "..", "..", "mods");
const id = manifest.id;
const mixinToml = readFileSync(join(root, "mixins", id, "horizon-mixin.toml"), "utf8");
const version = /^version\s*=\s*"([^"]+)"/m.exec(mixinToml)?.[1];
if (version !== manifest.version) throw new Error(`horizon-mixin.toml ${version} != horizon-native.json ${manifest.version}`);
const revision = /^horizon_revision\s*=\s*"([^"]+)"/m.exec(mixinToml)?.[1];
if (revision !== manifest.horizon) throw new Error(`horizon_revision ${revision} != horizon ${manifest.horizon}`);

const files = [["horizon-native.json", readFileSync(join(root, "horizon-native.json"))]];
const allowed = /\.(rs|toml)$|^(LICENSE|NOTICE)/;
for (const name of readdirSync(join(root, "mixins", id)).sort()) {
  if (allowed.test(name)) files.push([`mixins/${id}/${name}`, readFileSync(join(root, "mixins", id, name))]);
}
// data/: the files the mod compiles in with native_data! (its icon).
const data = (rel) => {
  const dir = join(root, "mixins", id, rel);
  if (!existsSync(dir)) return;
  for (const name of readdirSync(dir).sort()) {
    const path = `${rel}/${name}`;
    if (statSync(join(dir, name)).isDirectory()) data(path);
    else files.push([`mixins/${id}/${path}`, readFileSync(join(dir, name))]);
  }
};
data("data");

const table = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = table[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
// 2026-01-01 00:00 in DOS time.
const DOS_TIME = 0;
const DOS_DATE = ((2026 - 1980) << 9) | (1 << 5) | 1;

const locals = [];
const centrals = [];
let offset = 0;
for (const [name, data] of files) {
  const nameBuf = Buffer.from(name, "utf8");
  const packed = deflateRawSync(data, { level: 9 });
  const crc = crc32(data);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(0x0800, 6); // UTF-8 names
  local.writeUInt16LE(8, 8); // deflate
  local.writeUInt16LE(DOS_TIME, 10);
  local.writeUInt16LE(DOS_DATE, 12);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(packed.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(nameBuf.length, 26);
  local.writeUInt16LE(0, 28);
  locals.push(local, nameBuf, packed);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(0x0314, 4); // made by Unix, 2.0
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(0x0800, 8);
  central.writeUInt16LE(8, 10);
  central.writeUInt16LE(DOS_TIME, 12);
  central.writeUInt16LE(DOS_DATE, 14);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(packed.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(nameBuf.length, 28);
  central.writeUInt32LE((0o100644 << 16) >>> 0, 38); // -rw-r--r--
  central.writeUInt32LE(offset, 42);
  centrals.push(central, nameBuf);
  offset += 30 + nameBuf.length + packed.length;
}
const centralSize = centrals.reduce((n, b) => n + b.length, 0);
const end = Buffer.alloc(22);
end.writeUInt32LE(0x06054b50, 0);
end.writeUInt16LE(files.length, 8);
end.writeUInt16LE(files.length, 10);
end.writeUInt32LE(centralSize, 12);
end.writeUInt32LE(offset, 16);
const zip = Buffer.concat([...locals, ...centrals, end]);
mkdirSync(out, { recursive: true });
const file = join(out, `${id}-${version}.hnmod`);
writeFileSync(file, zip);
console.log(JSON.stringify({ file, files: files.length, size: zip.length, sha256: createHash("sha256").update(zip).digest("hex") }, null, 2));
