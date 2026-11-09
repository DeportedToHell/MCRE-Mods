// ShulkerBoxTooltip for Horizon: the configuration (ShulkerBoxTooltip's
// `Configuration` and `ClientConfiguration`, its option tree and its
// `config/shulkerboxtooltip.json5` file) and the small helpers of the preview,
// with no game types, so the same file builds and tests on its own
// (`cargo test` at the repository root) and is what the bundle adds to the
// game as `crate::game::shulkerboxtooltip`.

pub const ID: &str = "shulkerboxtooltip";
/// `fabric.mod.json`'s name.
pub const NAME: &str = "Shulker Box Tooltip";
pub const VERSION: &str = "1.0.1";
/// The file ShulkerBoxTooltip keeps its configuration in, in `config/`.
pub const CONFIG_FILE: &str = "shulkerboxtooltip.json5";
/// A key that is not bound (`InputConstants.UNKNOWN`).
pub const UNBOUND: &str = "key.keyboard.unknown";

// ---- the option tree ---------------------------------------------------------------------

/// The tabs of the config screen, in their order (`@ConfigCategory(ordinal)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Preview,
    Tooltip,
    Colors,
    Controls,
    Server,
}

impl Category {
    pub const ALL: [Category; 5] = [Category::Preview, Category::Tooltip, Category::Colors, Category::Controls, Category::Server];

    /// The field's name in the file.
    pub fn name(self) -> &'static str {
        match self {
            Category::Preview => "preview",
            Category::Tooltip => "tooltip",
            Category::Colors => "colors",
            Category::Controls => "controls",
            Category::Server => "server",
        }
    }

    /// `shulkerboxtooltip.config.category.<name>`.
    pub fn title_key(self) -> String {
        format!("shulkerboxtooltip.config.category.{}", self.name())
    }
}

/// The colour groups of the Colors tab (`ColorRegistry` categories, in the
/// order the config tree lists them: the default one first, then the others
/// as their `HashMap` gives them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorGroup {
    Default,
    ShulkerBoxes,
    Bundles,
}

impl ColorGroup {
    pub const ALL: [ColorGroup; 3] = [ColorGroup::Default, ColorGroup::ShulkerBoxes, ColorGroup::Bundles];

    pub fn id(self) -> &'static str {
        match self {
            ColorGroup::Default => "shulkerboxtooltip:default",
            ColorGroup::ShulkerBoxes => "shulkerboxtooltip:shulker_boxes",
            ColorGroup::Bundles => "shulkerboxtooltip:bundles",
        }
    }

    /// `shulkerboxtooltip.colors.<namespace>.<path>`.
    pub fn title_key(self) -> String {
        format!("shulkerboxtooltip.colors.{}", self.id().replace(':', "."))
    }
}

/// The dye colours, in `DyeColor` order, with `getTextureDiffuseColor`.
pub const DYES: [(&str, u32); 16] = [
    ("white", 0xF9FFFE),
    ("orange", 0xF9801D),
    ("magenta", 0xC74EBD),
    ("light_blue", 0x3AB3DA),
    ("yellow", 0xFED83D),
    ("lime", 0x80C71F),
    ("pink", 0xF38BAA),
    ("gray", 0x474F52),
    ("light_gray", 0x9D9D97),
    ("cyan", 0x169C9C),
    ("purple", 0x8932B8),
    ("blue", 0x3C44AA),
    ("brown", 0x835432),
    ("green", 0x5E7C16),
    ("red", 0xB02E26),
    ("black", 0x1D1D21),
];

/// `ColorKey.ofDye`: each channel at least 0.15, as `componentsToRgb` turns
/// the floats back into an int.
pub fn dye_window_color(diffuse: u32) -> u32 {
    let channel = |shift: u32| -> u32 {
        let c = ((diffuse >> shift) & 0xFF) as f32 / 255.0;
        (255.0f32 * c.max(0.15)) as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// One window colour: its group, its id in the file, its name's language
/// key and its default.
#[derive(Clone, Debug)]
pub struct ColorKey {
    pub group: ColorGroup,
    pub id: String,
    pub title_key: String,
    pub default: u32,
}

/// The window colours `ShulkerBoxTooltip.registerColors` registers, in order.
pub fn color_keys() -> Vec<ColorKey> {
    let mut keys = vec![
        ColorKey {
            group: ColorGroup::Default,
            id: "default".into(),
            title_key: "shulkerboxtooltip.colors.shulkerboxtooltip.default.default".into(),
            default: 0xFFFFFF,
        },
        ColorKey { group: ColorGroup::Default, id: "ender_chest".into(), title_key: "block.minecraft.ender_chest".into(), default: 0x0B4B41 },
        ColorKey {
            group: ColorGroup::Default,
            id: "generic_container".into(),
            title_key: "shulkerboxtooltip.colors.shulkerboxtooltip.default.generic_container".into(),
            default: 0xFFFFFF,
        },
        ColorKey { group: ColorGroup::ShulkerBoxes, id: "shulker_box".into(), title_key: "block.minecraft.shulker_box".into(), default: 0x976797 },
    ];
    for (dye, diffuse) in DYES {
        keys.push(ColorKey {
            group: ColorGroup::ShulkerBoxes,
            id: format!("{dye}_shulker_box"),
            title_key: format!("block.minecraft.{dye}_shulker_box"),
            default: dye_window_color(diffuse),
        });
    }
    keys.push(ColorKey { group: ColorGroup::Bundles, id: "bundle".into(), title_key: "item.minecraft.bundle".into(), default: 0x844B2B });
    for (dye, diffuse) in DYES {
        keys.push(ColorKey {
            group: ColorGroup::Bundles,
            id: format!("{dye}_bundle"),
            title_key: format!("item.minecraft.{dye}_bundle"),
            default: dye_window_color(diffuse),
        });
    }
    keys
}

/// The enums of the options, each with its values in declaration order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnumKind {
    /// `ItemStackMergingStrategy`.
    Merging,
    /// `CompactPreviewOrder`.
    Order,
    /// `Theme`.
    Theme,
    /// `BundleTheme`.
    BundleTheme,
    /// `PreviewPosition`.
    Position,
    /// `ShulkerBoxTooltipType`.
    TooltipType,
    /// `LootTableInfoType`.
    LootInfo,
    /// `EnderChestSyncType`.
    EnderSync,
}

impl EnumKind {
    /// The values' names, as the file writes them.
    pub fn values(self) -> &'static [&'static str] {
        match self {
            EnumKind::Merging => &["IGNORE", "FIRST_ITEM", "SEPARATE"],
            EnumKind::Order => &["STACK_SIZE", "PREVIEW_ORDER"],
            EnumKind::Theme => &["SHULKERBOXTOOLTIP", "VANILLA"],
            EnumKind::BundleTheme => &["SHULKERBOXTOOLTIP", "VANILLA", "VANILLA_PLUS"],
            EnumKind::Position => &["INSIDE", "OUTSIDE", "OUTSIDE_TOP", "OUTSIDE_BOTTOM"],
            EnumKind::TooltipType => &["VANILLA", "MOD", "NONE"],
            EnumKind::LootInfo => &["HIDE", "SIMPLE", "ADVANCED"],
            EnumKind::EnderSync => &["NONE", "ACTIVE", "PASSIVE"],
        }
    }

    /// The enum's `toString`: the language key of value `i`.
    pub fn value_key(self, i: usize) -> String {
        let prefix = match self {
            EnumKind::Merging => "item_stack_merging_strategy",
            EnumKind::Order => "compact_preview_order",
            EnumKind::Theme | EnumKind::BundleTheme => "theme",
            EnumKind::Position => "preview_position",
            EnumKind::TooltipType => "tooltip_type",
            EnumKind::LootInfo => "loot_table_info_type",
            EnumKind::EnderSync => "ender_chest_sync_type",
        };
        let name = self.values().get(i).copied().unwrap_or("").to_lowercase();
        format!("shulkerboxtooltip.config.{prefix}.{name}")
    }
}

/// What an option holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bool,
    Enum(EnumKind),
    /// An int greater than zero (`@Validator(GreaterThanZero.class)`).
    PositiveInt,
    /// A key (`Key`), by its name.
    Key,
    /// A window colour (`ColorKey`): RGB, no alpha.
    Color,
}

/// An option's value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Value {
    Bool(bool),
    Enum(usize),
    Int(i32),
    Key(String),
    /// The colour as an int, which may hold alpha bits while it is being
    /// edited (`ColorValueConfigEntry` sets 0xFFFFFFFF for what it can't read),
    /// and is then invalid.
    Color(i32),
}

impl Value {
    pub fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }
    pub fn as_enum(&self) -> usize {
        match self {
            Value::Enum(i) => *i,
            _ => 0,
        }
    }
    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(i) | Value::Color(i) => *i,
            _ => 0,
        }
    }
    pub fn as_key(&self) -> &str {
        match self {
            Value::Key(k) => k,
            _ => UNBOUND,
        }
    }
}

/// An option of the tree (`ValueConfigNode`), or a colour group's heading
/// (`CategoryConfigNode` inside the Colors tab).
#[derive(Clone, Debug)]
pub struct Node {
    pub category: Category,
    /// The field's name in the file (`swapModes`), or the colour's id.
    pub name: String,
    pub kind: Kind,
    pub default: Value,
    /// `@RequiresRestart`.
    pub restart: bool,
    /// The colour's group, for a colour.
    pub group: Option<ColorGroup>,
    /// The title's language key.
    pub title_key: String,
}

impl Node {
    fn option(category: Category, name: &str, kind: Kind, default: Value) -> Node {
        Node {
            category,
            name: name.into(),
            kind,
            default,
            restart: false,
            group: None,
            title_key: format!("shulkerboxtooltip.config.option.{}.{}", category.name(), snake_case(name)),
        }
    }

    /// The tooltip's language key (`<title>.tooltip`); colours have none.
    pub fn tooltip_key(&self) -> Option<String> {
        (self.kind != Kind::Color).then(|| format!("{}.tooltip", self.title_key))
    }

    /// The prefix text's language key (`<title>.prefix`), shown above the
    /// option when the language has it.
    pub fn prefix_key(&self) -> Option<String> {
        (self.kind != Kind::Color).then(|| format!("{}.prefix", self.title_key))
    }

    /// `ValueConfigNode.validate`: the error's language key.
    pub fn validate(&self, v: &Value) -> Option<&'static str> {
        match (self.kind, v) {
            (Kind::PositiveInt, Value::Int(i)) if *i <= 0 => Some("shulkerboxtooltip.config.validator.greater_than_zero"),
            (Kind::Color, Value::Color(c)) if (*c as u32) & 0xFF00_0000 != 0 => Some("shulkerboxtooltip.config.validator.invalid_color"),
            _ => None,
        }
    }
}

/// The options, in the order the config screen lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Opt {
    Enable,
    SwapModes,
    AlwaysOn,
    Merging,
    Order,
    MaxRowSize,
    ServerIntegration,
    Theme,
    BundleTheme,
    Position,
    ShortCounts,
    GenericPreview,
    ShowKeyHints,
    TooltipType,
    LootInfo,
    HideLore,
    ColoredPreview,
    PreviewKey,
    FullPreviewKey,
    LockKey,
    ClientIntegration,
    EnderSync,
}

/// The first colour's index in `nodes()`: the colours follow the options.
pub const FIRST_COLOR: usize = Opt::ColoredPreview as usize + 1;
/// How many window colours there are (`color_keys().len()`).
pub const COLOR_COUNT: usize = 3 + 17 + 17;

/// Every option and colour, in `Opt` order with the colours after
/// `ColoredPreview` (as the Colors tab lists them).
pub fn nodes() -> Vec<Node> {
    use Category as C;
    let b = |c: C, n: &str, d: bool| Node::option(c, n, Kind::Bool, Value::Bool(d));
    let e = |c: C, n: &str, k: EnumKind, d: usize| Node::option(c, n, Kind::Enum(k), Value::Enum(d));
    let key = |n: &str, d: &str| Node::option(C::Controls, n, Kind::Key, Value::Key(d.into()));
    let mut v = vec![
        b(C::Preview, "enable", true),
        b(C::Preview, "swapModes", false),
        b(C::Preview, "alwaysOn", false),
        e(C::Preview, "compactPreviewNbtBehavior", EnumKind::Merging, 2),
        e(C::Preview, "compactPreviewOrder", EnumKind::Order, 0),
        Node::option(C::Preview, "defaultMaxRowSize", Kind::PositiveInt, Value::Int(9)),
        Node { restart: true, ..b(C::Preview, "serverIntegration", true) },
        e(C::Preview, "theme", EnumKind::Theme, 0),
        e(C::Preview, "themeBundle", EnumKind::BundleTheme, 1),
        e(C::Preview, "position", EnumKind::Position, 0),
        b(C::Preview, "shortItemCounts", true),
        b(C::Preview, "genericContainerPreview", true),
        b(C::Tooltip, "showKeyHints", true),
        e(C::Tooltip, "type", EnumKind::TooltipType, 1),
        e(C::Tooltip, "lootTableInfoType", EnumKind::LootInfo, 0),
        b(C::Tooltip, "hideShulkerBoxLore", false),
        b(C::Colors, "coloredPreview", true),
    ];
    for k in color_keys() {
        v.push(Node {
            category: C::Colors,
            name: k.id.clone(),
            kind: Kind::Color,
            default: Value::Color(k.default as i32),
            restart: false,
            group: Some(k.group),
            title_key: k.title_key,
        });
    }
    v.extend([
        key("previewKey", "key.keyboard.left.shift"),
        key("fullPreviewKey", "key.keyboard.left.alt"),
        key("lockTooltipKey", "key.keyboard.left.control"),
        Node { restart: true, ..b(C::Server, "clientIntegration", true) },
        Node { restart: true, ..e(C::Server, "enderChestSyncType", EnumKind::EnderSync, 1) },
    ]);
    v
}

/// The index in `nodes()` of option `o`.
pub fn index(o: Opt) -> usize {
    let i = o as usize;
    if i < FIRST_COLOR { i } else { i + COLOR_COUNT }
}

/// `ShulkerBoxTooltipUtil.snakeCase`.
pub fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ---- the configuration -------------------------------------------------------------------

/// The configuration: one value per node of `nodes()`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Config {
    pub values: Vec<Value>,
}

impl Default for Config {
    fn default() -> Self {
        Config { values: nodes().into_iter().map(|n| n.default).collect() }
    }
}

impl Config {
    pub fn get(&self, o: Opt) -> &Value {
        &self.values[index(o)]
    }
    pub fn on(&self, o: Opt) -> bool {
        self.get(o).as_bool()
    }
    pub fn choice(&self, o: Opt) -> usize {
        self.get(o).as_enum()
    }
    pub fn max_row_size(&self) -> i32 {
        self.get(Opt::MaxRowSize).as_int()
    }
    pub fn key(&self, o: Opt) -> &str {
        self.get(o).as_key()
    }
    /// Window colour `i` of `color_keys()` (`ColorKey.rgb()`).
    pub fn color(&self, i: usize) -> u32 {
        self.values.get(FIRST_COLOR + i).map_or(0xFF_FFFF, |v| v.as_int() as u32 & 0xFF_FFFF)
    }

    /// `RootConfigNode.validate` on every node: the first error, or the
    /// "Multiple errors" key when more than one node has one.
    pub fn validate(&self) -> Option<&'static str> {
        let nodes = nodes();
        let mut error = None;
        for (n, v) in nodes.iter().zip(&self.values) {
            if let Some(e) = n.validate(v) {
                if error.is_some() {
                    return Some("shulkerboxtooltip.config.validator.multiple_errors");
                }
                error = Some(e);
            }
        }
        error
    }

    /// The configuration in the file's format (Jankson's, as ShulkerBoxTooltip
    /// writes it: the options with their comments, keys as `{ "code": ... }`,
    /// colours in hexadecimal).
    pub fn to_json5(&self) -> String {
        let nodes = nodes();
        let mut out = String::from("{\n");
        let cats: Vec<Category> = Category::ALL.to_vec();
        for (ci, cat) in cats.iter().enumerate() {
            out.push_str(&format!("\t\"{}\": {{\n", cat.name()));
            let mut lines: Vec<String> = Vec::new();
            for (i, n) in nodes.iter().enumerate().filter(|(_, n)| n.category == *cat && n.kind != Kind::Color) {
                let mut s = String::new();
                if let Some(c) = comment(&n.name) {
                    s.push_str("\t\t/* ");
                    s.push_str(&c.replace('\n', "\n\t\t   "));
                    s.push_str("\n\t\t*/\n");
                }
                s.push_str(&format!("\t\t\"{}\": {}", n.name, self.value_text(i)));
                lines.push(s);
            }
            if *cat == Category::Colors {
                let mut s = String::from("\t\t\"colors\": {\n");
                let groups: Vec<String> = ColorGroup::ALL
                    .iter()
                    .map(|g| {
                        let mut gs = format!("\t\t\t\"{}\": {{\n", g.id());
                        let entries: Vec<String> = nodes
                            .iter()
                            .zip(&self.values)
                            .filter(|(n, _)| n.group == Some(*g))
                            .map(|(n, v)| {
                                format!(
                                    "\t\t\t\t// (default value: {:#x})\n\t\t\t\t\"{}\": {:#x}",
                                    n.default.as_int() as u32,
                                    n.name,
                                    v.as_int() as u32
                                )
                            })
                            .collect();
                        gs.push_str(&entries.join(",\n"));
                        gs.push_str("\n\t\t\t}");
                        gs
                    })
                    .collect();
                s.push_str(&groups.join(",\n"));
                s.push_str("\n\t\t}");
                lines.push(s);
            }
            out.push_str(&lines.join(",\n"));
            out.push_str(if ci + 1 == cats.len() { "\n\t}\n" } else { "\n\t},\n" });
        }
        out.push_str("}");
        out
    }

    /// The configuration of a file: what it doesn't hold, or holds wrongly,
    /// keeps its default (Jankson's `fromJson` over a fresh `Configuration`);
    /// None when it can't be read at all, or fails validation (both make
    /// ShulkerBoxTooltip use the defaults).
    pub fn from_json5(text: &str) -> Option<Config> {
        let root = json5::parse(text)?;
        let nodes = nodes();
        let mut config = Config::default();
        for (i, n) in nodes.iter().enumerate() {
            let cat = root.get(n.category.name());
            let found = if let Some(g) = n.group {
                cat.and_then(|c| c.get("colors")).and_then(|c| c.get(g.id())).and_then(|c| c.get(&n.name))
            } else {
                cat.and_then(|c| c.get(&n.name))
            };
            let Some(found) = found else { continue };
            let value = match n.kind {
                Kind::Bool => found.as_bool().map(Value::Bool),
                Kind::Enum(k) => found.as_str().and_then(|s| k.values().iter().position(|v| *v == s)).map(Value::Enum),
                Kind::PositiveInt => found.as_i64().map(|v| Value::Int(v as i32)),
                // Jankson reads a key from `{ "code": ... }` or a bare string;
                // one the game doesn't know is unbound.
                Kind::Key => found.get("code").or(Some(found)).and_then(|c| c.as_str()).map(|s| Value::Key(s.to_owned())),
                // `deserializeColorCategory`: a value past an int resets to the default.
                Kind::Color => found.as_i64().map(|v| {
                    if v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX) {
                        Value::Color(v as i32)
                    } else {
                        n.default.clone()
                    }
                }),
            };
            if let Some(v) = value {
                config.values[i] = v;
            }
        }
        config.validate().is_none().then_some(config)
    }
}

/// An option's `@Comment` in the file.
fn comment(name: &str) -> Option<&'static str> {
    Some(match name {
        "enable" => "Toggles the shulker box preview.\n(default value: true)",
        "swapModes" => "Swaps the preview modes.\nIf true, pressing the preview key will show the full preview instead.\n(default value: false)",
        "alwaysOn" => "If on, the preview is always displayed, regardless of the preview key being pressed.\n(default value: false)",
        "compactPreviewNbtBehavior" => {
            "In compact mode, how should items with the same ID but different component data be compacted?\nIGNORE: Ignores component data\nFIRST_ITEM: Items are displayed as all having the same component as the first item\nSEPARATE: Separates items with different component data\n(default value: SEPARATE)"
        }
        "compactPreviewOrder" => {
            "How compact preview items should be ordered.\nSTACK_SIZE: Sort stacks by size, largest first.\nPREVIEW_ORDER: Preserve the order of items as they appear in the container preview.\n(default value: STACK_SIZE)"
        }
        "defaultMaxRowSize" => "The max number of items in a row.\nMay not affect modded containers.\n(default value: 9)",
        "serverIntegration" => {
            "If on, the client will try to send packets to servers to allow extra preview information such as ender chest previews.\n(default value: true)\n"
        }
        "theme" => "The theme to use for preview windows.\nSHULKERBOXTOOLTIP: ShulkerBoxTooltip's default look and feel.\nVANILLA: Mimics the style of vanilla bundle previews.\n(default value: SHULKERBOXTOOLTIP)",
        "themeBundle" => {
            "The theme to use for bundle preview windows.\nVANILLA: Fully disable ShulkerBoxTooltip's changes to bundles.\nVANILLA_PLUS: Mimics vanilla theme, with enhancements like no size limit and preview locking.\nSHULKERBOXTOOLTIP: ShulkerBoxTooltip's custom look and feel.\n(default value: VANILLA)"
        }
        "position" => {
            "The position of the preview window.\nINSIDE: Inside the item's tooltip.\nOUTSIDE: Outside the item's tooltip, moves depending on the screen borders.\nOUTSIDE_TOP: Always at the top of the item's tooltip.\nOUTSIDE_BOTTOM: Always at the bottom of the item's tooltip.\n(default value: INSIDE)"
        }
        "shortItemCounts" => "If on, large item counts in compact previews will be shortened.\n(default value: true)",
        "genericContainerPreview" => {
            "If on, items with the container data component that are not explicitly supported\nwill still show a preview, as if they were a shulker box.\n(default value: true)"
        }
        "showKeyHints" => "Controls whether the key hints in the container's tooltip should be displayed.\n(default value: true)",
        "type" => "The tooltip to use.\nVANILLA: The vanilla tooltip (shows the first 5 items)\nMOD: The mod's tooltip\nNONE: No tooltip\n(default value: MOD)",
        "lootTableInfoType" => {
            "Shows info about the current loot table of the item if present.\nVisible only when Tooltip Type is set to Modded.\nHIDE: No loot table info, default.\nSIMPLE: Displays whether the stack uses a loot table.\nADVANCED: Shows the loot table used by the item.\n(default value: HIDE)"
        }
        "hideShulkerBoxLore" => {
            "If on, the mod hides the custom text on shulker box tooltips.\nUse this option when a server-side preview data pack clashes with the mod.\n(default value: false)"
        }
        "coloredPreview" => "Controls whether the preview window should be colored.\n(default value: true)",
        "previewKey" => "Press this key when hovering a container stack to open the preview window.\n(default value: key.keyboard.left.shift)",
        "fullPreviewKey" => "Press this key when hovering a container stack to open the full preview window.\n(default value: key.keyboard.left.alt)",
        "lockTooltipKey" => "Hold this key when previewing a stack to lock the tooltip.\n(default value: key.keyboard.left.control)",
        "clientIntegration" => {
            "If on, the server will be able to provide extra information about containers to the clients with the mod installed.\nDisabling this option will disable all the options below.\n(default value: true)\n"
        }
        "enderChestSyncType" => {
            "Changes the way the ender chest content preview is synchronized.\nNONE: No synchronization, prevents clients from seeing a preview of their ender chest.\nACTIVE: Ender chest contents are synchronized when changed.\nPASSIVE: Ender chest contents are synchronized when the client opens a preview.\n(default value: ACTIVE)"
        }
        _ => return None,
    })
}

fn json_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Color(c) => format!("{:#x}", *c as u32),
        Value::Key(k) => format!("{{\n\t\t\t\"code\": \"{}\"\n\t\t}}", json5::escape(k)),
        Value::Enum(i) => i.to_string(),
    }
}

impl Config {
    /// `json_value` for every node, enums by name.
    pub fn value_text(&self, i: usize) -> String {
        let nodes = nodes();
        match (&nodes[i].kind, &self.values[i]) {
            (Kind::Enum(k), Value::Enum(e)) => format!("\"{}\"", k.values().get(*e).copied().unwrap_or("")),
            (_, v) => json_value(v),
        }
    }
}

// ---- reading JSON5 -----------------------------------------------------------------------

/// The little of JSON5 Jankson writes and reads: objects, arrays, strings,
/// numbers (hexadecimal too), booleans, null, `//` and `/* */` comments,
/// unquoted keys and trailing commas.
pub mod json5 {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Number(f64, Option<i64>),
        Str(String),
        Array(Vec<Json>),
        Object(Vec<(String, Json)>),
    }

    impl Json {
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Object(entries) => entries.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn as_bool(&self) -> Option<bool> {
            match self {
                Json::Bool(b) => Some(*b),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Json::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn as_i64(&self) -> Option<i64> {
            match self {
                Json::Number(f, i) => i.or_else(|| (f.fract() == 0.0 && f.abs() < 9.0e18).then_some(*f as i64)),
                _ => None,
            }
        }
    }

    pub fn escape(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out
    }

    struct P<'a> {
        s: &'a [u8],
        i: usize,
    }

    impl P<'_> {
        fn skip(&mut self) {
            loop {
                while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                    self.i += 1;
                }
                if self.s[self.i..].starts_with(b"//") {
                    while self.i < self.s.len() && self.s[self.i] != b'\n' {
                        self.i += 1;
                    }
                } else if self.s[self.i..].starts_with(b"/*") {
                    match self.s[self.i + 2..].windows(2).position(|w| w == b"*/") {
                        Some(p) => self.i += p + 4,
                        None => self.i = self.s.len(),
                    }
                } else {
                    return;
                }
            }
        }
        fn peek(&mut self) -> Option<u8> {
            self.skip();
            self.s.get(self.i).copied()
        }
        fn string(&mut self, quote: u8) -> Option<String> {
            self.i += 1;
            let mut bytes = Vec::new();
            while let Some(&c) = self.s.get(self.i) {
                self.i += 1;
                if c == quote {
                    return String::from_utf8(bytes).ok();
                }
                if c == b'\\' {
                    let e = *self.s.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'n' => bytes.push(b'\n'),
                        b't' => bytes.push(b'\t'),
                        b'r' => bytes.push(b'\r'),
                        b'u' => {
                            let hex = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(u32::from_str_radix(hex, 16).ok()?)?;
                            let mut buf = [0; 4];
                            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        other => bytes.push(other),
                    }
                } else {
                    bytes.push(c);
                }
            }
            None
        }
        fn word(&mut self) -> String {
            let start = self.i;
            while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || b"_+-.$".contains(&self.s[self.i])) {
                self.i += 1;
            }
            String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
        }
        fn value(&mut self) -> Option<Json> {
            match self.peek()? {
                b'{' => {
                    self.i += 1;
                    let mut entries = Vec::new();
                    loop {
                        match self.peek()? {
                            b'}' => {
                                self.i += 1;
                                return Some(Json::Object(entries));
                            }
                            b',' => self.i += 1,
                            q @ (b'"' | b'\'') => {
                                let k = self.string(q)?;
                                if self.peek()? != b':' {
                                    return None;
                                }
                                self.i += 1;
                                entries.push((k, self.value()?));
                            }
                            _ => {
                                let k = self.word();
                                if k.is_empty() || self.peek()? != b':' {
                                    return None;
                                }
                                self.i += 1;
                                entries.push((k, self.value()?));
                            }
                        }
                    }
                }
                b'[' => {
                    self.i += 1;
                    let mut items = Vec::new();
                    loop {
                        match self.peek()? {
                            b']' => {
                                self.i += 1;
                                return Some(Json::Array(items));
                            }
                            b',' => self.i += 1,
                            _ => items.push(self.value()?),
                        }
                    }
                }
                q @ (b'"' | b'\'') => self.string(q).map(Json::Str),
                _ => {
                    let w = self.word();
                    match w.as_str() {
                        "true" => Some(Json::Bool(true)),
                        "false" => Some(Json::Bool(false)),
                        "null" => Some(Json::Null),
                        _ => number(&w),
                    }
                }
            }
        }
    }

    fn number(w: &str) -> Option<Json> {
        let (neg, body) = match w.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, w.strip_prefix('+').unwrap_or(w)),
        };
        if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
            let v = i64::from_str_radix(hex, 16).ok()?;
            let v = if neg { -v } else { v };
            return Some(Json::Number(v as f64, Some(v)));
        }
        let f: f64 = body.parse().ok()?;
        let f = if neg { -f } else { f };
        let i = (!body.contains(['.', 'e', 'E'])).then(|| body.parse::<i64>().ok().map(|v| if neg { -v } else { v })).flatten();
        Some(Json::Number(f, i))
    }

    pub fn parse(text: &str) -> Option<Json> {
        let mut p = P { s: text.as_bytes(), i: 0 };
        let v = p.value()?;
        (p.peek().is_none()).then_some(v)
    }
}

// ---- the preview -------------------------------------------------------------------------

/// `ShulkerBoxTooltipUtil.abbreviateInteger`: 1.2k, 45k, 3M...
pub fn abbreviate(count: i64) -> String {
    let count = count.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    if count == i32::MIN {
        return "-2G".into();
    }
    if count > -1000 && count < 1000 {
        return count.to_string();
    }
    let mut s = String::new();
    let mut c = count;
    if c < 0 {
        s.push('-');
        c = -c;
    }
    let (integral, decimal, unit) = match (c as f64).log10() as i32 {
        3 => (c / 1_000, (c % 1_000) / 100, 'k'),
        4 | 5 => (c / 1_000, 0, 'k'),
        6 => (c / 1_000_000, (c % 1_000_000) / 100_000, 'M'),
        7 | 8 => (c / 1_000_000, 0, 'M'),
        _ => (c / 1_000_000_000, (c % 1_000_000_000) / 100_000_000, 'G'),
    };
    s.push_str(&integral.to_string());
    if decimal > 0 {
        s.push('.');
        s.push_str(&decimal.to_string());
    }
    s.push(unit);
    s
}

/// Which preview the keys ask for (`PreviewType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewType {
    NoPreview,
    Compact,
    Full,
}

/// `ShulkerBoxTooltipClient.getCurrentPreviewType`.
pub fn preview_type(requested: bool, full_key: bool, swap: bool, has_full: bool) -> PreviewType {
    if requested && !has_full {
        return PreviewType::Compact;
    }
    if !requested {
        return PreviewType::NoPreview;
    }
    if full_key != swap { PreviewType::Full } else { PreviewType::Compact }
}

/// A compact preview cell (`MergedItemStack`): the slots it merges, the
/// first of them and the total count.
#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    /// The merged slots, in slot order.
    pub slots: Vec<usize>,
    pub first: usize,
    pub count: u64,
}

/// `MergedItemStack.mergeInventory`: the non-empty slots `counts` (None:
/// empty) grouped by `same(a, b)`, ordered by `order` (0 `STACK_SIZE`,
/// largest first, ties by first slot; 1 `PREVIEW_ORDER`). Slots at or past
/// `max_size` are left out (`MergedItemStack.add`).
pub fn merge(counts: &[Option<u32>], max_size: usize, same: impl Fn(usize, usize) -> bool, order: usize) -> Vec<Merged> {
    let mut groups: Vec<Merged> = Vec::new();
    for (i, c) in counts.iter().enumerate() {
        let Some(c) = c else { continue };
        if let Some(g) = groups.iter_mut().find(|g| same(g.slots[0], i)) {
            if i < max_size {
                g.slots.push(i);
                g.first = g.first.min(i);
                g.count += u64::from(*c);
            }
        } else if i < max_size {
            groups.push(Merged { slots: vec![i], first: i, count: u64::from(*c) });
        } else {
            // A group whose stacks all lie past the end is still listed, empty
            // (`MergedItemStack.add` ignores them but the group exists).
            groups.push(Merged { slots: vec![i], first: usize::MAX, count: 0 });
        }
    }
    groups.retain(|g| g.first != usize::MAX || g.count > 0);
    match order {
        1 => groups.sort_by_key(|g| g.first),
        _ => groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.first.cmp(&b.first))),
    }
    groups
}

/// The ender chest's slots (`PlayerEnderChestContainer`: 27).
pub const ENDER_SLOTS: usize = 27;

/// A slot read from the server's ender chest message or the cache file: kept
/// only inside the ender chest, so a hostile server can't make the client
/// allocate for a huge index.
pub fn ender_slot(text: &str) -> Option<usize> {
    text.parse::<usize>().ok().filter(|&slot| slot < ENDER_SLOTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ender_slots_outside_the_chest_are_refused() {
        assert_eq!(ender_slot("0"), Some(0));
        assert_eq!(ender_slot("26"), Some(26));
        assert_eq!(ender_slot("27"), None);
        assert_eq!(ender_slot("4294967295"), None);
        assert_eq!(ender_slot("18446744073709551615"), None);
        assert_eq!(ender_slot("99999999999999999999999"), None);
        assert_eq!(ender_slot("-1"), None);
    }

    #[test]
    fn short_counts_as_the_mod_writes_them() {
        assert_eq!(abbreviate(999), "999");
        assert_eq!(abbreviate(1000), "1k");
        assert_eq!(abbreviate(1234), "1.2k");
        assert_eq!(abbreviate(45_678), "45k");
        assert_eq!(abbreviate(1_500_000), "1.5M");
        assert_eq!(abbreviate(-2500), "-2.5k");
    }

    #[test]
    fn dyed_colours_are_clamped_as_color_key_of_dye() {
        assert_eq!(dye_window_color(0x1D1D21), 0x262626);
        assert_eq!(dye_window_color(0xF9801D), 0xF98026);
        assert_eq!(dye_window_color(0xF9FFFE), 0xF9FFFE);
    }

    #[test]
    fn the_file_round_trips() {
        let mut c = Config::default();
        c.values[index(Opt::Position)] = Value::Enum(2);
        c.values[index(Opt::MaxRowSize)] = Value::Int(5);
        c.values[index(Opt::PreviewKey)] = Value::Key("key.keyboard.z".into());
        c.values[FIRST_COLOR + 1] = Value::Color(0x123456);
        let text = c.to_json5();
        assert_eq!(Config::from_json5(&text), Some(c));
    }

    #[test]
    fn a_bad_value_keeps_the_default_and_an_invalid_file_is_refused() {
        let c = Config::from_json5("{ preview: { enable: false, theme: \"NOPE\", }, }").unwrap();
        assert!(!c.on(Opt::Enable));
        assert_eq!(c.choice(Opt::Theme), 0);
        assert_eq!(Config::from_json5("{ \"preview\": { \"defaultMaxRowSize\": 0 } }"), None);
        assert_eq!(Config::from_json5("{ oops"), None);
    }

    #[test]
    fn preview_types() {
        assert_eq!(preview_type(false, false, false, true), PreviewType::NoPreview);
        assert_eq!(preview_type(true, false, false, true), PreviewType::Compact);
        assert_eq!(preview_type(true, true, false, true), PreviewType::Full);
        assert_eq!(preview_type(true, false, true, true), PreviewType::Full);
        assert_eq!(preview_type(true, true, false, false), PreviewType::Compact);
    }

    #[test]
    fn merging_orders_by_size_then_slot() {
        let counts = [Some(1), Some(64), None, Some(1), Some(5)];
        // Slots 0 and 3 hold the same item.
        let same = |a: usize, b: usize| (a == 0 || a == 3) && (b == 0 || b == 3) || a == b;
        let m = merge(&counts, 27, same, 0);
        assert_eq!(m.iter().map(|g| g.first).collect::<Vec<_>>(), vec![1, 4, 0]);
        assert_eq!(m[2].count, 2);
        let m = merge(&counts, 27, same, 1);
        assert_eq!(m.iter().map(|g| g.first).collect::<Vec<_>>(), vec![0, 1, 4]);
    }

    #[test]
    fn titles_follow_the_field_names() {
        let n = nodes();
        assert_eq!(n[index(Opt::SwapModes)].title_key, "shulkerboxtooltip.config.option.preview.swap_modes");
        assert_eq!(n[index(Opt::EnderSync)].title_key, "shulkerboxtooltip.config.option.server.ender_chest_sync_type");
        assert_eq!(n.len(), index(Opt::EnderSync) + 1);
        assert_eq!(color_keys().len(), COLOR_COUNT);
        assert_eq!(color_keys().iter().position(|k| k.id == "black_bundle"), Some(COLOR_COUNT - 1));
    }
}
