// Writes mixins/emotecraft/engine/src/lang.rs from Emotecraft's en_US.json and ru_RU.json
// (Crowdin): `node tools/lang.mjs <path to emotesAssets/src/main/resources/assets/emotecraft/lang>`.
// The one key ru_RU lacks (the Let's Groove emote's name) is the port's own translation.
import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = process.argv[2];
if (!src) throw new Error("usage: node tools/lang.mjs <lang folder>");
const en = JSON.parse(readFileSync(join(src, "en_US.json"), "utf8"));
const ru = JSON.parse(readFileSync(join(src, "ru_RU.json"), "utf8"));
// The port's translation of the key ru_RU lacks.
const ruExtra = { "emotecraft.emote.lets_groove.name": "Давай зажжём" };
for (const [k, v] of Object.entries(ruExtra)) if (!(k in ru)) ru[k] = v;
const missing = Object.keys(en).filter((k) => !(k in ru));
if (missing.length) console.warn("ru_RU still lacks", missing);
const lit = (s) => JSON.stringify(s);
const table = (name, obj) =>
  `pub const ${name}: &[(&str, &str)] = &[\n` +
  Object.entries(obj).map(([k, v]) => `    (${lit(k)}, ${lit(v)}),`).join("\n") +
  "\n];\n";
const out =
  "//! Emotecraft's texts: `assets/emotecraft/lang/en_US.json` and `ru_RU.json` (Crowdin project\n" +
  "//! 414868, by Emotecraft's translators), written by tools/lang.mjs. The Russian name of the\n" +
  "//! Let's Groove emote, which ru_RU lacks, is the port's.\n//!\n" +
  "//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.\n\n" +
  table("EN_US", en) + "\n" + table("RU_RU", ru) + "\n" +
  "/// A key's text in a language (`ru_ru` or English), none if Emotecraft has no such key.\n" +
  "pub fn get(lang: &str, key: &str) -> Option<&'static str> {\n" +
  "    let find = |t: &'static [(&str, &str)]| t.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);\n" +
  "    if lang.eq_ignore_ascii_case(\"ru_ru\") {\n        if let Some(v) = find(RU_RU) {\n            return Some(v);\n        }\n    }\n" +
  "    find(EN_US)\n}\n";
writeFileSync(join(root, "mixins", "emotecraft", "engine", "src", "lang.rs"), out);
console.log("lang.rs:", Object.keys(en).length, "keys");
