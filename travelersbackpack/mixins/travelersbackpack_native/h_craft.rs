// `BackpackShapedRecipe.assemble`: a backpack crafted from another (a design
// from the standard backpack) keeps what that one held and its tier, as
// vanilla's transmute recipes keep the base's components. (Its own copy of
// the designs: this runs in the game's library, away from the mod's other
// modules.)
fn travelersbackpack_craft(
    ctx: &Context<'_>,
    input: &Input<'_>,
    recipe: &Recipe,
) -> core::ops::ControlFlow<Option<Option<ItemStack>>> {
    use core::ops::ControlFlow::{Break, Continue};
    const DESIGNS: [&str; 44] = [
        "standard", "netherite", "diamond", "gold", "emerald", "iron", "lapis", "redstone", "coal", "quartz",
        "bookshelf", "end", "nether", "sandstone", "snow", "sponge", "cake", "cactus", "hay", "melon", "pumpkin",
        "creeper", "dragon", "enderman", "blaze", "ghast", "magma_cube", "skeleton", "spider", "wither", "warden",
        "bat", "bee", "wolf", "fox", "ocelot", "horse", "cow", "pig", "sheep", "squid", "chicken", "villager",
        "iron_golem",
    ];
    let backpack = |id: &str| {
        id.strip_prefix("travelersbackpack:").is_some_and(|d| DESIGNS.contains(&d))
    };
    thread_local! {
        static INSIDE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    }
    if INSIDE.with(|i| i.get()) || !matches!(recipe, Recipe::Shaped { .. }) {
        return Continue(());
    }
    let from = input.stacks().into_iter().find(|s| backpack(&s.item));
    INSIDE.with(|i| i.set(true));
    let made = Recipes::assemble_if_matches(ctx, input, recipe);
    INSIDE.with(|i| i.set(false));
    match (made, from) {
        (Some(Some(stack)), Some(from)) if backpack(&stack.item) => Break(Some(Some(from.transmute(&stack.item, stack.count)))),
        // A new backpack (`ShapedBackpackRecipe.assemble`): the sleeping bag
        // crafted in is its bag (none without one), a tank puts the tanks
        // upgrade in (`STARTER_UPGRADES`).
        (Some(Some(mut stack)), None) if backpack(&stack.item) => {
            const COLOURS: [&str; 16] = [
                "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan",
                "purple", "blue", "brown", "green", "red", "black",
            ];
            let stacks = input.stacks();
            let bag = stacks
                .iter()
                .find_map(|s| {
                    let c = s.item.strip_prefix("travelersbackpack:")?.strip_suffix("_sleeping_bag")?;
                    COLOURS.iter().position(|x| *x == c)
                })
                .map_or(-1, |i| i as i32);
            let tanks = stacks.iter().any(|s| &*s.item == "travelersbackpack:backpack_tank");
            use horizon_world::nbt::{Compound, Tag};
            let data = stack.data_mut();
            let mut custom = match data.other("minecraft:custom_data") {
                Some(Tag::Compound(c)) => c.clone(),
                _ => Compound::new(),
            };
            if bag != 14 {
                custom.insert("TravelersBackpackSleepingBag".into(), Tag::Int(bag));
            }
            if !custom.is_empty() {
                data.set_other("minecraft:custom_data", Some(Tag::Compound(custom)));
            }
            if tanks {
                data.container.push((27, ItemStack::new("travelersbackpack:tanks_upgrade", 1)));
            }
            let name = usize::try_from(bag).ok().and_then(|i| COLOURS.get(i)).copied().unwrap_or("none");
            let mut look = Compound::new();
            look.insert("flags".into(), Tag::List(vec![Tag::Byte(i8::from(tanks))]));
            look.insert("strings".into(), Tag::List(vec![Tag::String(name.into())]));
            data.set_other("minecraft:custom_model_data", Some(Tag::Compound(look)));
            data.wire = Default::default();
            stack.tidy();
            Break(Some(Some(stack)))
        }
        (other, _) => Break(other),
    }
}
