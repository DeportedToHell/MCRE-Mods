// Regenerates content/ (the content mod, `accessories.hmod`) from a checkout of
// the original (wisp-forest/accessories, branch 1.21.11):
//   node tools/content.mjs <checkout>
// It copies the textures the screen draws, the slot, group and entity files,
// the tags, and the languages. owo's rich-text values (objects and arrays with
// colours and `{"index": n}` arguments) become plain strings with `%s` / `%n$s`,
// since Horizon's lang loader, like vanilla's, takes strings only. The port's
// own keys (lang/port/*.json) are merged on top; the mod's menu type is written
// beside the data.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, copyFileSync, rmSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const checkout = process.argv[2];
if (!checkout) throw new Error("usage: node tools/content.mjs <accessories checkout>");
const res = join(checkout, "common", "src", "main", "resources");
const out = join(root, "content");

const copyTree = (from, to, filter = () => true) => {
  for (const name of readdirSync(from).sort()) {
    const src = join(from, name);
    const dst = join(to, name);
    if (statSync(src).isDirectory()) copyTree(src, dst, filter);
    else if (filter(src)) {
      mkdirSync(dirname(dst), { recursive: true });
      copyFileSync(src, dst);
    }
  }
};

for (const dir of ["assets", "data"]) rmSync(join(out, dir), { recursive: true, force: true });

// Textures the screen uses (the legacy ones nothing draws are left out).
const textures = join(res, "assets", "accessories", "textures");
const used = [
  /^gui[\\/]accessories_(open|back)_icon(_hovered)?\.png$/,
  /^gui[\\/](charm|cosmetic|crafting)_toggle_icon(_hovered)?\.png$/,
  /^gui[\\/](reset|settings)_icon(_hovered)?\.png$/,
  /^gui[\\/]sprites[\\/]container[\\/]slot[\\/][a-z_]+\.png$/,
  /^gui[\\/]sprites[\\/]container[\\/]slot[\\/]group[\\/][a-z_]+\.png$/,
  /^gui[\\/]theme[\\/]all_arrow_directions\.png$/,
  /^gui[\\/]theme[\\/](light|dark)[\\/].+\.png$/,
];
copyTree(textures, join(out, "assets", "accessories", "textures"), (src) => {
  const rel = src.slice(textures.length + 1);
  return used.some((r) => r.test(rel)) && !/(charm_icon|crafting_grid|toggle_background)\.png$/.test(rel);
});
// The nine-patch definitions (sizes, corners), read by the native mod.
copyTree(join(res, "assets", "accessories", "nine_patch_textures"), join(out, "assets", "accessories", "nine_patch_textures"));

// Slots, groups, entity bindings and tags.
copyTree(join(res, "data", "accessories", "accessories"), join(out, "data", "accessories", "accessories"));
copyTree(join(res, "data", "accessories", "tags"), join(out, "data", "accessories", "tags"));

// owo rich text -> a plain string. `{"index": n}` is the n-th argument.
const rich = (v) => {
  if (typeof v === "string") return v.replaceAll("%", "%%");
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  if (Array.isArray(v)) return v.map(rich).join("");
  if (v && typeof v === "object") {
    let s = "";
    if (typeof v.text === "string") s += v.text.replaceAll("%", "%%");
    if (typeof v.translate === "string") s += v.translate;
    if (typeof v.index === "number") s += `%${v.index + 1}$s`;
    if (Array.isArray(v.extra)) s += v.extra.map(rich).join("");
    return s;
  }
  return "";
};
// A plain string value is Java format text already (its own %s); rich text's
// literal parts are escaped.
const plain = (v) => (typeof v === "string" ? v : rich(v));
const langIn = join(res, "assets", "accessories", "lang");
const langOut = join(out, "assets", "accessories", "lang");
const portLang = join(root, "lang");
mkdirSync(langOut, { recursive: true });
const codes = new Set(readdirSync(langIn).map((n) => n.replace(/\.json$/, "")));
if (existsSync(portLang)) for (const n of readdirSync(portLang)) codes.add(n.replace(/\.json$/, ""));
for (const code of [...codes].sort()) {
  const src = join(langIn, `${code}.json`);
  const original = existsSync(src) ? JSON.parse(readFileSync(src, "utf8")) : {};
  const map = {};
  for (const [k, v] of Object.entries(original)) map[k] = plain(v);
  const port = join(portLang, `${code}.json`);
  if (existsSync(port)) Object.assign(map, JSON.parse(readFileSync(port, "utf8")));
  writeFileSync(join(langOut, `${code}.json`), JSON.stringify(map, null, 2) + "\n");
}

// The accessories screen's menu type: its slots are the accessories' (and
// their cosmetics'); Horizon adds the player's inventory after them.
const menu = join(out, "data", "accessories", "horizon", "menu");
mkdirSync(menu, { recursive: true });
writeFileSync(join(menu, "accessories_menu.json"), JSON.stringify({ slots: 256, data_slots: 16 }, null, 2) + "\n");

console.log("content/ written from", checkout);
