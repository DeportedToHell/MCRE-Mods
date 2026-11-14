// The death message of a dying player: their grave's death cause.
fn universalgraves_died_message(player: u128, message: &horizon_client::text::Text) {
    crate::game::universalgraves::death_message(player, message);
}
