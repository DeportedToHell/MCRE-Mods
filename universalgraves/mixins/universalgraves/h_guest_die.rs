// A player is dying (`ServerPlayer.die`): who, with what, by what blow.
fn universalgraves_guest_die(game: &mut crate::game::Game<'_>, host: &mut LanHost, gi: usize, conn: u32) {
    let Some(g) = host.hub.guests.get(gi) else {
        return;
    };
    if g.health > 0.0 || g.dead {
        return;
    }
    let (damage_type, attacker) = match host.play.get(&conn).and_then(|p| p.death_cause.as_deref()) {
        Some("death.attack.genericKill") => ("minecraft:generic_kill".to_owned(), 0),
        _ => game
            .guest_combat
            .get(&conn)
            .and_then(|tracker| tracker.last_blow())
            .map(|(d, a)| (d.to_owned(), a))
            .unwrap_or_else(|| ("minecraft:generic".to_owned(), 0)),
    };
    crate::game::universalgraves::dying(crate::game::universalgraves::Dying {
        uuid: g.uuid,
        name: g.name.clone(),
        textures: g.properties.iter().find(|p| p.name == "textures").map(|p| (p.value.clone(), p.signature.clone())),
        xp_level: g.xp_level,
        xp_progress: g.xp_progress,
        dimension: g.dimension.clone(),
        position: g.position,
        damage_type,
        attacker,
        skin_parts: g.info.model_customisation,
        main_arm: g.info.main_hand as u8,
        language: g.info.language.to_ascii_lowercase(),
        death_cause: None,
    });
}
