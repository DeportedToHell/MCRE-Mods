// Makes mixins/shulkerboxtooltip/lang.rs from ShulkerBoxTooltip's own language
// files: `node tools/lang.mjs <ShulkerBoxTooltip checkout> [out.rs]`.
// en_us.json and ru_ru.json are in common/src/main/resources/assets/
// shulkerboxtooltip/lang (MIT, as the rest of ShulkerBoxTooltip). ru_ru.json
// lacks a few of the newer lines: their Russian is below (this port's own).
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [checkout, out = new URL("../mixins/shulkerboxtooltip/lang.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(checkout, "common/src/main/resources/assets/shulkerboxtooltip/lang");
const en = JSON.parse(readFileSync(join(dir, "en_us.json"), "utf8"));
const ru = JSON.parse(readFileSync(join(dir, "ru_ru.json"), "utf8"));

const MISSING_RU = {
  "shulkerboxtooltip.colors.shulkerboxtooltip.default.generic_container": "Обычный контейнер",
  "shulkerboxtooltip.colors.shulkerboxtooltip.bundles": "Мешки",
  "shulkerboxtooltip.config.option.preview.compact_preview_order": "Порядок предметов в компактном предпросмотре",
  "shulkerboxtooltip.config.option.preview.compact_preview_order.tooltip":
    "Порядок предметов в компактном предпросмотре.\n • Размер стопки:\n сначала самые большие стопки.\n • Порядок предпросмотра:\n предметы в том же порядке, что и в предпросмотре контейнера.",
  "shulkerboxtooltip.config.compact_preview_order.stack_size": "Размер стопки",
  "shulkerboxtooltip.config.compact_preview_order.preview_order": "Порядок предпросмотра",
  "shulkerboxtooltip.config.option.preview.generic_container_preview": "Предпросмотр других контейнеров",
  "shulkerboxtooltip.config.option.preview.generic_container_preview.tooltip":
    "Если включено, предметы с компонентом контейнера,\nкоторые мод не поддерживает явно, всё равно показывают предпросмотр,\nкак шалкеровый ящик.",
};

const rust = (s) => JSON.stringify(s).replace(/\\u([0-9a-f]{4})/g, (_, h) => `\\u{${h}}`);
let rs = `// ShulkerBoxTooltip's texts (MIT, https://github.com/MisterPeModder/ShulkerBoxTooltip,
// common/src/main/resources/assets/shulkerboxtooltip/lang/en_us.json and ru_ru.json):
// (key, English, Russian). Made by tools/lang.mjs; don't edit by hand. The
// Russian of the lines ru_ru.json lacks is this port's.

pub const LANG: &[(&str, &str, &str)] = &[\n`;
let missing = [];
for (const key of Object.keys(en).sort()) {
  let r = ru[key];
  if (r === undefined) {
    r = MISSING_RU[key];
    missing.push(key);
    if (r === undefined) throw new Error(`no Russian for ${key}`);
  }
  rs += `    (${rust(key)}, ${rust(en[key])}, ${rust(r)}),\n`;
}
rs += "];\n";
writeFileSync(out, rs);
console.log(`${Object.keys(en).length} lines, ${missing.length} translated here: ${missing.join(", ")}`);
