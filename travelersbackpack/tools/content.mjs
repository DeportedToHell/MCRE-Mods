// Builds the content mod (content/: horizon-mod.toml, assets/, data/) from a
// Traveler's Backpack checkout: `node tools/content.mjs <checkout>`, the
// checkout being https://github.com/Tiviacz1337/Travelers-Backpack (branch
// 26.1-fabric). The original draws its backpacks with a model loader of its own
// (`BackpackDynamicModel`: the base, the tanks, the sleeping bag and a design's
// nose, put together in code); Horizon reads vanilla models, so each design's
// parts are merged here into plain models, with and without tanks. Its recipes
// are of its own type (`backpack_shaped`, which carries the backpack's contents
// over); they become vanilla's shaped recipes, and the native mod carries the
// contents over when one is crafted.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, copyFileSync, rmSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const [checkout] = process.argv.slice(2);
if (!checkout) throw new Error("usage: node tools/content.mjs <Traveler's Backpack checkout>");
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "content");
const src = join(checkout, "src/main");
const assets = join(src, "resources/assets/travelersbackpack");
const generated = join(src, "generated/data/travelersbackpack");
const NS = "travelersbackpack";

const json = (p) => JSON.parse(readFileSync(p, "utf8"));
const write = (p, v) => {
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, typeof v === "string" ? v : JSON.stringify(v, null, 2) + "\n");
};
const copyDir = (from, to, filter = () => true) => {
  for (const e of readdirSync(from, { withFileTypes: true })) {
    if (e.isDirectory()) copyDir(join(from, e.name), join(to, e.name), filter);
    else if (filter(e.name)) {
      mkdirSync(to, { recursive: true });
      copyFileSync(join(from, e.name), join(to, e.name));
    }
  }
};

// ---- the designs (ModBlocks: name, map colour, sound, light) --------------------------------

const MAP_COLORS = {
  NONE: 0, GRASS: 1, SAND: 2, WOOL: 3, FIRE: 4, ICE: 5, METAL: 6, PLANT: 7, SNOW: 8, CLAY: 9, DIRT: 10,
  STONE: 11, WATER: 12, WOOD: 13, QUARTZ: 14, COLOR_ORANGE: 15, COLOR_MAGENTA: 16, COLOR_LIGHT_BLUE: 17,
  COLOR_YELLOW: 18, COLOR_LIGHT_GREEN: 19, COLOR_PINK: 20, COLOR_GRAY: 21, COLOR_LIGHT_GRAY: 22, COLOR_CYAN: 23,
  COLOR_PURPLE: 24, COLOR_BLUE: 25, COLOR_BROWN: 26, COLOR_GREEN: 27, COLOR_RED: 28, COLOR_BLACK: 29, GOLD: 30,
  DIAMOND: 31, LAPIS: 32, EMERALD: 33, PODZOL: 34, NETHER: 35, TERRACOTTA_WHITE: 36, TERRACOTTA_ORANGE: 37,
  TERRACOTTA_MAGENTA: 38, TERRACOTTA_LIGHT_BLUE: 39, TERRACOTTA_YELLOW: 40, TERRACOTTA_LIGHT_GREEN: 41,
  TERRACOTTA_PINK: 42, TERRACOTTA_GRAY: 43, TERRACOTTA_LIGHT_GRAY: 44, TERRACOTTA_CYAN: 45,
  TERRACOTTA_PURPLE: 46, TERRACOTTA_BLUE: 47, TERRACOTTA_BROWN: 48, TERRACOTTA_GREEN: 49, TERRACOTTA_RED: 50,
  TERRACOTTA_BLACK: 51,
};
// A vanilla block with each `SoundType`.
const SOUNDS = {
  WOOL: "minecraft:white_wool", METAL: "minecraft:iron_block", NETHERITE_BLOCK: "minecraft:netherite_block",
  STONE: "minecraft:stone", WOOD: "minecraft:oak_planks", GLASS: "minecraft:glass",
  NETHER_BRICKS: "minecraft:nether_bricks", SNOW: "minecraft:snow_block", GRASS: "minecraft:grass_block",
  SLIME_BLOCK: "minecraft:slime_block", BONE_BLOCK: "minecraft:bone_block",
  SCULK_SHRIEKER: "minecraft:sculk_shrieker",
};
const modBlocks = readFileSync(join(src, "java/com/tiviacz/travelersbackpack/init/ModBlocks.java"), "utf8");
const designs = [];
for (const m of modBlocks.matchAll(
  /MODID, "([a-z_]+)"\), new TravelersBackpackBlock\(Block\.Properties\.of\(\)\.mapColor\(MapColor\.([A-Z_]+)\)\.sound\(SoundType\.([A-Z_]+)\)([^;]*)/g,
)) {
  const light = /lightLevel\(f -> (\d+)\)/.exec(m[4]);
  designs.push({ name: m[1], map: MAP_COLORS[m[2]], sound: SOUNDS[m[3]], light: light ? Number(light[1]) : 0 });
}
for (const d of designs) {
  if (d.map === undefined || d.sound === undefined) throw new Error(`unknown colour or sound for ${d.name}`);
}

// `BackpackDynamicModel.addExtras`: a design's nose or horns.
const EXTRAS = {
  fox: "fox_nose", warden: "warden_horns", wolf: "wolf_nose", ocelot: "ocelot_nose",
  pig: "pig_nose", horse: "pig_nose", villager: "villager_nose", iron_golem: "villager_nose",
};

// `TravelersBackpackBlock`'s shapes, facing north (MainBody, Top, Bottom,
// PocketFace, LeftStrap, RightStrap), in pixels; turned for the others.
const SHAPE_NORTH = [
  [4.1, 0.8, 7.1, 11.9, 7.8, 11.0],
  [4.1, 7.8, 7.1, 11.9, 10.1, 11.0],
  [4.1, 0.0, 7.9, 11.9, 0.8, 11.0],
  [4.9, 2.4, 5.5, 11.1, 7.1, 7.1],
  [10.4, 1.6, 11.0, 11.2, 7.8, 11.8],
  [4.8, 1.6, 11.0, 5.6, 7.8, 11.8],
];
// Turned clockwise seen from above: (x, z) -> (16 - z, x).
const turn = ([x0, y0, z0, x1, y1, z1]) => [16 - z1, y0, x0, 16 - z0, y1, x1];
// The template (glazed terracotta) faces the way the placer looks, the
// original the other way: each of our facings takes the opposite's shape
// and model turn.
const OPPOSITE = { north: "south", south: "north", east: "west", west: "east" };
const shapes = {};
let s = SHAPE_NORTH;
for (const facing of ["north", "east", "south", "west"]) {
  shapes[OPPOSITE[facing]] = s.map((b) => b.map((v) => Math.round((v / 16) * 10000) / 10000));
  s = s.map(turn);
}
// The blockstates' turn of the model (the original's variants).
// `DyeColor`: a backpack's sleeping bag, red as it starts, or none.
const BAG_COLOURS = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
  "purple", "blue", "brown", "green", "red", "black"];
const BAG_STATES = ["red", ...BAG_COLOURS.filter((c) => c !== "red"), "none"];
const MODEL_Y = { north: 0, west: 270, east: 90, south: 180 };

// ---- models ---------------------------------------------------------------------------------

const part = (name) => json(join(assets, "models/block", `backpack_${name}.json`)).elements;
const BASE = part("base");
const TANKS = part("tanks");
const BAG = part("sleeping_bag");
const BAG_EXTRAS = part("sleeping_bag_extras");
// The dyed standard backpack (`BASE_DYED`, `EXTRAS`): its body tinted by its
// dye (the item's first tint), its buckles not.
const retexture = (elements, texture) => elements.map((e) => ({ ...e,
  faces: Object.fromEntries(Object.entries(e.faces).map(([k, f]) => [k, { ...f, texture }])) }));
const BASE_DYED = retexture(part("base_dyed"), "#dyed");
const EXTRAS_DYED = retexture(part("extras"), "#dyed_extras").map((e) => ({ ...e,
  faces: Object.fromEntries(Object.entries(e.faces).map(([k, f]) => { const { tintindex, ...rest } = f; return [k, rest]; })) }));
const DYED_TEXTURES = { dyed: `${NS}:block/backpack/dyed`, dyed_extras: `${NS}:block/backpack/dyed_extras` };
// `DyedItemColor`'s default: the dyed model untinted.
const UNDYED = -1;
// `BackpackItemModel.ITEM_TRANSFORMS`, in degrees and pixels.
const DISPLAY = {
  thirdperson_lefthand: { rotation: [60, -180, 0], translation: [0, 1.5, 0.5], scale: [0.7, 0.7, 0.7] },
  thirdperson_righthand: { rotation: [60, -180, 0], translation: [0, 1.5, 0.5], scale: [0.7, 0.7, 0.7] },
  firstperson_lefthand: { rotation: [0, -90, 12.5], translation: [1.13, 6, 2], scale: [0.68, 0.68, 0.68] },
  firstperson_righthand: { rotation: [0, -90, 12.5], translation: [1.13, 6, 2], scale: [0.68, 0.68, 0.68] },
  head: { rotation: [0, 180, 0], translation: [0, 14.5, 0], scale: [1, 1, 1] },
  gui: { rotation: [30, -38, 0], translation: [-0.25, 2.25, 0], scale: [1, 1, 1] },
  ground: { rotation: [0, 0, 0], translation: [0, 2, 0], scale: [0.5, 0.5, 0.5] },
  fixed: { rotation: [0, 180, 0], translation: [0, 2.25, 0], scale: [1, 1, 1] },
  on_shelf: { rotation: [0, 0, 0], translation: [0, 2.25, 0], scale: [1, 1, 1] },
};

for (const p of ["content/assets", "content/data"]) rmSync(join(root, "..", p), { recursive: true, force: true });

const out = (...p) => join(root, ...p);
// The item's tank fluids (`BackpackDynamicModel.addFluid`): the native mod
// puts each tank's sprite in the custom model data's strings (1: left,
// 2: right), its fill in its floats and its colour in its colors. A fluid is
// a box of its still sprite inside the tank, as high as the tank is full, in
// 16 steps.
const FLUID_STEPS = 16;
const FLUID_SPRITES = { water: "minecraft:block/water_still", lava: "minecraft:block/lava_still",
  milk: "minecraft:block/white_concrete" };
const FLUID_TANKS = [["left", 1.8], ["right", 12.7]];
for (const [side, x] of FLUID_TANKS) {
  for (const [name, texture] of Object.entries(FLUID_SPRITES)) {
    for (let k = 1; k <= FLUID_STEPS; k++) {
      const top = 0.8 + (6.2 * k) / FLUID_STEPS;
      const face = (v) => ({ uv: [0, 0, 3, v], texture: "#fluid", tintindex: 0 });
      write(out("assets", NS, "models/item", `fluid_${side}_${name}_${k}.json`), {
        textures: { fluid: texture, particle: texture },
        elements: [{ name: "Fluid", from: [x, 0.8, 6.3], to: [x + 1.5, top, 7.8],
          faces: { up: face(3), down: face(3), north: face((12 * k) / FLUID_STEPS), south: face((12 * k) / FLUID_STEPS),
            west: face((12 * k) / FLUID_STEPS), east: face((12 * k) / FLUID_STEPS) } }],
        display: DISPLAY,
      });
    }
  }
}
const tankFluid = (i, side) => ({
  type: "minecraft:select", property: "minecraft:custom_model_data", index: i + 1,
  fallback: { type: "minecraft:empty" },
  cases: Object.keys(FLUID_SPRITES).map((name) => ({ when: name, model: {
    type: "minecraft:range_dispatch", property: "minecraft:custom_model_data", index: i,
    fallback: { type: "minecraft:empty" },
    entries: Array.from({ length: FLUID_STEPS }, (_, j) => ({ threshold: (j + 0.5) / FLUID_STEPS, model: {
      type: "minecraft:model", model: `${NS}:item/fluid_${side}_${name}_${j + 1}`,
      tints: [{ type: "minecraft:custom_model_data", index: i, default: 0xffffff }] } })),
  } })),
});
const withFluids = (model) => ({ type: "minecraft:composite", models: [model, ...FLUID_TANKS.map(([side], i) => tankFluid(i, side))] });
for (const d of designs) {
  const own = json(join(assets, "models/block", `${d.name}.json`));
  const textures = {
    0: own.backpackTexture,
    bag: `${NS}:block/bag/red_sleeping_bag`,
    particle: own.particle,
  };
  const extras = EXTRAS[d.name] ? part(EXTRAS[d.name]) : [];
  const body = [...BASE, ...BAG, ...BAG_EXTRAS, ...extras];
  write(out("assets", NS, "models/block", `${d.name}.json`), { textures, elements: body });
  write(out("assets", NS, "models/block", `${d.name}_tanks.json`), { textures, elements: [...body, ...TANKS] });
  // The bag's colour (`BackpackDynamicModel`'s sleeping bag sprite): the red
  // models' children with its texture; none, the models without it.
  const bare = [...BASE, ...extras];
  write(out("assets", NS, "models/block", `${d.name}_none.json`), { textures, elements: bare });
  write(out("assets", NS, "models/block", `${d.name}_tanks_none.json`), { textures, elements: [...bare, ...TANKS] });
  for (const colour of BAG_COLOURS.filter((c) => c !== "red")) {
    for (const t of ["", "_tanks"]) {
      write(out("assets", NS, "models/block", `${d.name}${t}_${colour}.json`), {
        parent: `${NS}:block/${d.name}${t}`,
        textures: { ...textures, bag: `${NS}:block/bag/${colour}_sleeping_bag` },
      });
    }
  }
  // The item (`BackpackItemModel`): its tanks and its bag's colour as its
  // custom model data says (the native mod keeps it: a flag, then a string).
  write(out("assets", NS, "models/item", `${d.name}.json`), { textures, elements: body, display: DISPLAY });
  write(out("assets", NS, "models/item", `${d.name}_tanks.json`), { textures, elements: [...body, ...TANKS], display: DISPLAY });
  write(out("assets", NS, "models/item", `${d.name}_none.json`), { textures, elements: bare, display: DISPLAY });
  write(out("assets", NS, "models/item", `${d.name}_tanks_none.json`), { textures, elements: [...bare, ...TANKS], display: DISPLAY });
  for (const colour of BAG_COLOURS.filter((c) => c !== "red")) {
    for (const t of ["", "_tanks"]) {
      write(out("assets", NS, "models/item", `${d.name}${t}_${colour}.json`), {
        parent: `${NS}:item/${d.name}${t}`,
        textures: { ...textures, bag: `${NS}:block/bag/${colour}_sleeping_bag` },
      });
    }
  }
  const bagged = (t) => ({
    type: "minecraft:select", property: "minecraft:custom_model_data", index: 0,
    fallback: { type: "minecraft:model", model: `${NS}:item/${d.name}${t}` },
    cases: BAG_STATES.filter((c) => c !== "red").map((c) => ({ when: c, model: { type: "minecraft:model", model: `${NS}:item/${d.name}${t}_${c}` } })),
  });
  const undyed = {
    type: "minecraft:condition", property: "minecraft:custom_model_data", index: 0,
    on_true: bagged("_tanks"), on_false: bagged(""),
  };
  if (d.name !== "standard") {
    write(out("assets", NS, "items", `${d.name}.json`), { model: withFluids(undyed) });
  } else {
    // Dyed (`DataComponents.DYED_COLOR`): the dyed body, tinted.
    const dyedBody = [...BASE_DYED, ...EXTRAS_DYED];
    const dyedTextures = { ...textures, ...DYED_TEXTURES };
    for (const tanks of ["", "_tanks"]) {
      for (const bag of BAG_STATES) {
        const elements = [...dyedBody, ...(bag === "none" ? [] : [...BAG, ...BAG_EXTRAS]), ...(tanks ? TANKS : [])];
        const bagTexture = bag === "none" || bag === "red" ? textures.bag : `${NS}:block/bag/${bag}_sleeping_bag`;
        write(out("assets", NS, "models/item", `standard_dyed${tanks}_${bag}.json`),
          { textures: { ...dyedTextures, bag: bagTexture }, elements, display: DISPLAY });
      }
    }
    const tint = [{ type: "minecraft:dye", default: UNDYED }];
    const dyedBagged = (t) => ({
      type: "minecraft:select", property: "minecraft:custom_model_data", index: 0,
      fallback: { type: "minecraft:model", model: `${NS}:item/standard_dyed${t}_red`, tints: tint },
      cases: BAG_STATES.filter((c) => c !== "red").map((c) => ({ when: c,
        model: { type: "minecraft:model", model: `${NS}:item/standard_dyed${t}_${c}`, tints: tint } })),
    });
    // The creative tab's button (`createTabStack`, `RenderInfo.createCreativeTabInfo`):
    // the tanks full, water in the left and lava in the right, as
    // `BackpackDynamicModel.addFluid` puts a fluid's still sprite in a tank.
    const fluid = (x, texture, tintindex) => {
      const side = (u) => ({ uv: [0, 0, u, 12], texture, ...(tintindex === undefined ? {} : { tintindex }) });
      return {
        name: "Fluid", from: [x, 0.8, 6.3], to: [x + 1.5, 7, 7.8],
        faces: { up: { ...side(3), uv: [0, 0, 3, 3] }, north: side(3), south: side(3), west: side(3), east: side(3) },
      };
    };
    write(out("assets", NS, "models/item", "standard_creative_tab.json"), {
      textures: { ...textures, water: "minecraft:block/water_still", lava: "minecraft:block/lava_still" },
      elements: [...body, ...TANKS, fluid(1.8, "#water", 0), fluid(12.7, "#lava")],
      display: DISPLAY,
    });
    const tankCases = undyed.on_true.cases;
    tankCases.push({ when: "creative_tab", model: { type: "minecraft:model", model: `${NS}:item/standard_creative_tab`,
      // Water's colour (`FluidVariantRendering.getColor`), its sprite being grey.
      tints: [{ type: "minecraft:constant", value: 0x3f76e4 }] } });
    write(out("assets", NS, "items", "standard.json"), { model: withFluids({
      type: "minecraft:condition", property: "minecraft:has_component", component: "minecraft:dyed_color",
      on_true: { type: "minecraft:condition", property: "minecraft:custom_model_data", index: 0,
        on_true: dyedBagged("_tanks"), on_false: dyedBagged("") },
      on_false: undyed,
    }) });
    // Placed or worn and dyed: the buckles, bag and tanks; the native mod
    // draws the body in its dye.
    for (const tanks of ["", "_tanks"]) {
      for (const bag of BAG_STATES) {
        // A speck inside the body, so the body's texture is in the block atlas for the native mod.
        const speck = { from: [7.9, 4.0, 6.9], to: [8.0, 4.1, 7.0], faces: { north: { uv: [0, 0, 0.1, 0.1], texture: "#dyed" } } };
        const elements = [speck, ...EXTRAS_DYED, ...(bag === "none" ? [] : [...BAG, ...BAG_EXTRAS]), ...(tanks ? TANKS : [])];
        const bagTexture = bag === "none" || bag === "red" ? textures.bag : `${NS}:block/bag/${bag}_sleeping_bag`;
        write(out("assets", NS, "models/block", `standard_dyed${tanks}_${bag}.json`),
          { textures: { ...textures, ...DYED_TEXTURES, bag: bagTexture, particle: DYED_TEXTURES.dyed }, elements });
      }
    }
  }
  const variants = {};
  for (const [facing, y] of Object.entries(MODEL_Y)) {
    for (const tanks of [false, true]) {
      for (const bag of BAG_STATES) {
        const v = { model: `${NS}:block/${d.name}${tanks ? "_tanks" : ""}${bag === "red" ? "" : `_${bag}`}` };
        if (y) v.y = y;
        if (d.name !== "standard") {
          variants[`bag=${bag},facing=${facing},tanks=${tanks}`] = v;
          continue;
        }
        variants[`bag=${bag},dyed=false,facing=${facing},tanks=${tanks}`] = v;
        const dyed = { model: `${NS}:block/standard_dyed${tanks ? "_tanks" : ""}_${bag}` };
        if (y) dyed.y = y;
        variants[`bag=${bag},dyed=true,facing=${facing},tanks=${tanks}`] = dyed;
      }
    }
  }
  write(out("assets", NS, "blockstates", `${d.name}.json`), { variants });
  write(out("data", NS, "horizon/block", `${d.name}.json`), {
    template: "minecraft:white_glazed_terracotta",
    properties: {
      hardness: 1.0,
      explosion_resistance: 3600000.0,
      sounds: d.sound,
      map_color: d.map,
      light: d.light,
      requires_tool: false,
      occludes: false,
      render_shape: "MODEL",
    },
    state_properties: { tanks: { type: "boolean" }, bag: { type: "enum", values: BAG_STATES },
      ...(d.name === "standard" ? { dyed: { type: "boolean" } } : {}) },
    shapes: { property: "facing", values: shapes },
    item: true,
    components: { "minecraft:max_stack_size": 1 },
  });
  write(out("data", NS, "loot_table/blocks", `${d.name}.json`), {
    type: "minecraft:block",
    pools: [{ rolls: 1, bonus_rolls: 0, conditions: [{ condition: "minecraft:survives_explosion" }],
      entries: [{ type: "minecraft:item", name: `${NS}:${d.name}` }] }],
  });
}

// ---- textures, languages, menus, recipes, tags ------------------------------------------------

copyDir(join(assets, "textures"), out("assets", NS, "textures"), (n) => n.endsWith(".png") || n.endsWith(".mcmeta"));
copyDir(join(assets, "lang"), out("assets", NS, "lang"));
// The lantern lights through Horizon's own dynamic lights or Better Dynamic
// Lights, not LambDynamicLights: its line when neither is on says so.
for (const [lang, lines] of Object.entries({
  en_us: {
    "item.travelersbackpack.lantern_upgrade_missing":
      "§4Dynamic Lighting Inactive - turn on Horizon's Dynamic Lights or Better Dynamic Lights",
  },
  ru_ru: {
    "item.travelersbackpack.lantern_upgrade_tooltip": "Даёт рюкзаку источник света",
    "item.travelersbackpack.lantern_upgrade_active": "§2Динамический свет активен",
    "item.travelersbackpack.lantern_upgrade_missing":
      "§4Динамический свет неактивен - включите Dynamic Lights в Horizon или Better Dynamic Lights",
  },
})) {
  const file = out("assets", NS, "lang", `${lang}.json`);
  write(file, { ...json(file), ...lines });
}
// `Tiers`: each tier's menu has 256 slots, the most Horizon allows: the
// backpack's storage, upgrade and tool slots (sizes from the config), then
// `TAB_SLOTS` (10) for each upgrade slot; the rest hide (`MENU_SLOTS`).
for (const tier of ["leather", "iron", "gold", "diamond", "netherite"]) {
  // Placed, in a hand (the equip button) and worn (the unequip button).
  for (const opened of ["", "_held", "_worn"]) {
    write(out("data", NS, "horizon/menu", `${tier}${opened}.json`), { slots: 256 });
  }
}
const names = new Set(designs.map((d) => d.name));
for (const f of readdirSync(join(generated, "recipe"))) {
  const r = json(join(generated, "recipe", f));
  if (r.type !== `${NS}:backpack_shaped` || !names.has(r.result.id.split(":")[1])) continue;
  write(out("data", NS, "recipe", f), { ...r, type: "minecraft:crafting_shaped" });
}
copyDir(join(src, "resources/data/travelersbackpack/tags"), out("data", NS, "tags"), (n) => n.endsWith(".json"));

// ---- the tier upgrades (ModItems: `TierUpgrade`) ----------------------------------------------

// The blank upgrade stacks to 64, the tier upgrades to 16.
const UPGRADES = [["blank_upgrade", 64], ["iron_tier_upgrade", 16], ["gold_tier_upgrade", 16],
  ["diamond_tier_upgrade", 16], ["netherite_tier_upgrade", 16]];
// The backpack's own upgrades (`ModItems`: `UpgradeItem`, stacks of 16), put in its upgrade slots.
const BACKPACK_UPGRADES = ["tanks", "crafting", "furnace", "smoker", "blast_furnace", "pickup", "magnet", "jukebox",
  "void", "feeding", "refill", "lantern"].map((u) => [`${u}_upgrade`, 16]);
for (const [name, stack] of [...UPGRADES, ...BACKPACK_UPGRADES]) {
  write(out("data", NS, "horizon/item", `${name}.json`), { template: "minecraft:paper", max_stack_size: stack });
  write(out("assets", NS, "items", `${name}.json`), json(join(assets, "items", `${name}.json`)));
  write(out("assets", NS, "models/item", `${name}.json`), json(join(assets, "models/item", `${name}.json`)));
  write(out("data", NS, "recipe", `${name}.json`), json(join(generated, "recipe", `${name}.json`)));
}
// The tank (`BackpackTankItem`, an ingredient of the tanks upgrade and the
// standard backpack), the hose's nozzle and the hose (`HoseItem`: its three
// modes are its models, picked by its custom model data's first string).
for (const [name, stack] of [["backpack_tank", 16], ["hose_nozzle", 64]]) {
  write(out("data", NS, "horizon/item", `${name}.json`), { template: "minecraft:paper", max_stack_size: stack });
  write(out("assets", NS, "items", `${name}.json`), json(join(assets, "items", `${name}.json`)));
  write(out("assets", NS, "models/item", `${name}.json`), json(join(assets, "models/item", `${name}.json`)));
  write(out("data", NS, "recipe", `${name}.json`), json(join(generated, "recipe", `${name}.json`)));
}
write(out("data", NS, "horizon/item", "hose.json"), { template: "minecraft:paper", max_stack_size: 1 });
for (const mode of ["suck", "spill", "drink"]) {
  write(out("assets", NS, "models/item", `hose_${mode}.json`), json(join(assets, "models/item", `hose_${mode}.json`)));
}
write(out("assets", NS, "items", "hose.json"), { model: {
  type: "minecraft:select", property: "minecraft:custom_model_data", index: 0,
  fallback: { type: "minecraft:model", model: `${NS}:item/hose_suck` },
  cases: ["spill", "drink"].map((m) => ({ when: m, model: { type: "minecraft:model", model: `${NS}:item/hose_${m}` } })),
} });
write(out("data", NS, "recipe", "hose.json"), json(join(generated, "recipe", "hose.json")));

// The creative tab (`ModItemGroups`), in the original's order: the standard
// backpack five times (with tanks, then a tier each: the native mod gives them
// their components), every design with tanks, the sleeping bags.
write(out("data", NS, "horizon/creative_tab", "travelers_backpack.json"), {
  title: "itemGroup.travelersbackpack",
  icon: `${NS}:standard`,
  items: [
    "backpack_tank", "hose_nozzle", "hose",
    ...UPGRADES.map(([n]) => n),
    ...["tanks", "crafting", "furnace", "smoker", "blast_furnace", "feeding", "refill", "pickup", "magnet", "void",
      "jukebox", "lantern"].map((u) => `${u}_upgrade`),
    "standard", "standard", "standard", "standard", "standard",
    ...["netherite", "diamond", "gold", "emerald", "iron", "lapis", "redstone", "coal", "quartz", "bookshelf", "end",
      "nether", "sandstone", "snow", "sponge", "cake", "cactus", "hay", "melon", "pumpkin", "creeper", "dragon",
      "enderman", "blaze", "ghast", "magma_cube", "skeleton", "spider", "wither", "warden", "bat", "bee", "wolf", "fox",
      "ocelot", "horse", "cow", "pig", "sheep", "chicken", "squid", "villager", "iron_golem"],
    ...["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple",
      "blue", "brown", "green", "red", "black"].map((c) => `${c}_sleeping_bag`),
  ].map((n) => `${NS}:${n}`),
});
// Every design, for the smithing table's base slot.
write(out("data", NS, "tags/item/backpacks.json"), { replace: false, values: designs.map((d) => `${NS}:${d.name}`) });
// `BackpackUpgradeRecipe` (a smithing table: leather, a backpack, a tier
// upgrade): the slots accept these; the native mod makes the result, the
// backpack one tier up with what it holds.
write(out("data", NS, "recipe", "tier_upgrade.json"), {
  type: "minecraft:smithing_transform",
  template: "#c:leathers",
  base: `#${NS}:backpacks`,
  addition: UPGRADES.slice(1).map(([name]) => `${NS}:${name}`),
  result: { id: `${NS}:standard` },
});

// ---- the sleeping bags (`SleepingBagBlock`, a `BedBlock`; `SleepingBagItem`, a `BedItem`) ----------

// Each dye colour's, a bed of that colour underneath: its two halves, sleeping,
// the night skipped; drawn by the original's models (no bed block entity's).
const COLOURS = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
  "purple", "blue", "brown", "green", "red", "black"];
for (const colour of COLOURS) {
  const name = `${colour}_sleeping_bag`;
  write(out("data", NS, "horizon/block", `${name}.json`), {
    template: `minecraft:${colour}_bed`,
    properties: { hardness: 0.2, explosion_resistance: 0.2, sounds: "minecraft:white_wool", occludes: false, render_shape: "MODEL" },
    // `SLEEPING_BAG`: two pixels high (the head's half-pixel pillow left out).
    shapes: { property: "part", values: { foot: [[0, 0, 0, 1, 0.125, 1]], head: [[0, 0, 0, 1, 0.125, 1]] } },
    item: true,
  });
  write(out("assets", NS, "blockstates", `${name}.json`), json(join(assets, "blockstates", `${name}.json`)));
  for (const part of ["foot", "head"]) {
    write(out("assets", NS, "models/block", `${name}_${part}.json`), json(join(assets, "models/block", `${name}_${part}.json`)));
  }
  write(out("assets", NS, "models/item", `${name}.json`), json(join(assets, "models/item", `${name}.json`)));
  write(out("assets", NS, "items", `${name}.json`), json(join(assets, "items", `${name}.json`)));
  write(out("data", NS, "recipe", `${name}.json`), json(join(generated, "recipe", `${name}.json`)));
  write(out("data", NS, "recipe", `dye_${name}.json`), json(join(generated, "recipe", `dye_${name}.json`)));
}

// ---- common tags (`c:`), which Horizon's game data doesn't have -------------------------------

// The values Fabric's conventional tags give them, vanilla items only.
const C_TAGS = {
  chests: ["minecraft:chest", "minecraft:trapped_chest", "minecraft:ender_chest"],
  "chests/wooden": ["minecraft:chest", "minecraft:trapped_chest"],
  "glass_blocks/colorless": ["minecraft:glass"],
  "player_workstations/crafting_tables": ["minecraft:crafting_table"],
  "crops/carrot": ["minecraft:carrot"],
  "crops/nether_wart": ["minecraft:nether_wart"],
  "crops/wheat": ["minecraft:wheat"],
  "dusts/redstone": ["minecraft:redstone"],
  "dyes/green": ["minecraft:green_dye"],
  eggs: ["minecraft:egg", "minecraft:blue_egg", "minecraft:brown_egg"],
  ender_pearls: ["minecraft:ender_pearl"],
  "gems/diamond": ["minecraft:diamond"],
  "gems/emerald": ["minecraft:emerald"],
  "gems/lapis": ["minecraft:lapis_lazuli"],
  "gems/quartz": ["minecraft:quartz"],
  "ingots/gold": ["minecraft:gold_ingot"],
  "ingots/iron": ["minecraft:iron_ingot"],
  "ingots/netherite": ["minecraft:netherite_ingot"],
  leathers: ["minecraft:leather"],
  "rods/blaze": ["minecraft:blaze_rod"],
  "storage_blocks/lapis": ["minecraft:lapis_block"],
  strings: ["minecraft:string"],
};
for (const [tag, values] of Object.entries(C_TAGS)) {
  write(out("data", "c", "tags/item", `${tag}.json`), { replace: false, values });
}
// Every `#c:` tag a recipe uses is one of these.
for (const f of readdirSync(out("data", NS, "recipe"))) {
  for (const [, tag] of readFileSync(out("data", NS, "recipe", f), "utf8").matchAll(/"#c:([a-z_/]+)"/g)) {
    if (!C_TAGS[tag]) throw new Error(`recipe ${f} uses #c:${tag}, which C_TAGS lacks`);
  }
}

// ---- dyeing (`standard_dyed`, `cauldron_can_remove_dye`) ------------------------------------

write(out("data", NS, "recipe", "standard_dyed.json"), json(join(generated, "recipe", "standard_dyed.json")));
// The bookshelf backpack counts as a bookshelf; the dragon and the wither don't break backpacks.
for (const tag of ["enchantment_power_provider", "dragon_immune", "wither_immune"]) {
  write(out("data", "minecraft", "tags/block", `${tag}.json`), json(join(src, "resources/data/minecraft/tags/block", `${tag}.json`)));
}
write(out("data", "minecraft", "tags/item", "cauldron_can_remove_dye.json"),
  json(join(src, "resources/data/minecraft/tags/item/cauldron_can_remove_dye.json")));

// ---- advancements and trades (`ModAdvancements`, `villager_trade`) ----------------------------

// The mod's tree as it is; its own trigger (`travelersbackpack:action_type`) the native mod fires.
copyDir(join(src, "resources/data/travelersbackpack/advancement"), out("data", NS, "advancement"), (n) => n.endsWith(".json"));
// The recipe book's unlocks, for the recipes the port has.
const recipes = new Set(readdirSync(out("data", NS, "recipe")).map((f) => `${NS}:${f.replace(/\.json$/, "")}`));
for (const category of readdirSync(join(generated, "advancement/recipes"))) {
  for (const f of readdirSync(join(generated, "advancement/recipes", category))) {
    const a = json(join(generated, "advancement/recipes", category, f));
    if ((a.rewards?.recipes ?? []).every((r) => recipes.has(r))) write(out("data", NS, "advancement/recipes", category, f), a);
  }
}
// The wandering trader's trade (`villager_trade`) the native mod contributes: Horizon
// reads a content mod's trades only from the `minecraft` namespace, which a content mod can't write.

const manifest = `id = "travelersbackpack"
name = "Traveler's Backpack"
version = "0.1.0"
description = "Unique and upgradeable backpacks with customisation."
authors = ["Tiviacz1337"]
api = "0.2"
side = "both"
capabilities = ["registries"]
`;
write(out("horizon-mod.toml"), manifest);
console.log(JSON.stringify({ designs: designs.length, names: designs.map((d) => d.name) }));
