// Clumps for Horizon: the mod's names and the arithmetic of a clumped
// experience orb. No game types here, so `cargo test` runs it alone.
//
// Clumps keeps, in each orb, how many orbs of each value it took in
// (`clumpedMap`) and makes the orb worth all of them (`clumps$resolve`).
// Horizon's orbs keep a value and a count; the port keeps a clumped orb as
// one orb (count 1) worth all the points it took in, which Horizon saves and
// sends as it does any orb.
//
// Ported from Clumps by Jaredlll08 (MIT, https://github.com/jaredlll08/Clumps,
// branch 26.3).

pub const ID: &str = "clumps";
pub const NAME: &str = "Clumps";
pub const VERSION: &str = "1.0.2";

/// The most points one orb may be worth: the protocol sends an orb's value as
/// an `int`, and `Player.giveExperiencePoints` takes one.
pub const MAX_POINTS: u64 = i32::MAX as u64;

/// The points of an orb worth `value` that counts `count` orbs.
pub fn points(value: u32, count: u32) -> u64 {
    u64::from(value) * u64::from(count)
}

/// The value of the orb that takes in `from` points, being worth `into`:
/// None when the two together would be worth more than one orb can hold (the
/// merge doesn't happen then, so no point is lost).
pub fn merged(into: u64, from: u64) -> Option<u32> {
    let total = into.checked_add(from)?;
    (total > 0 && total <= MAX_POINTS).then(|| total as u32)
}

/// `fabric.mod.json`'s description (`Clumps xp orbs together.`), and the same
/// in Russian.
pub fn description(ru: bool) -> &'static str {
    if ru { "Объединяет шары опыта в один." } else { "Clumps xp orbs together." }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clump_is_worth_every_orb_it_took() {
        // Three orbs of 7 and one of 3: 24 points, as {7: 3, 3: 1} resolves.
        let into = points(7, 3);
        assert_eq!(merged(into, points(3, 1)), Some(24));
        assert_eq!(merged(0, 5), Some(5));
    }

    #[test]
    fn a_clump_never_overflows() {
        assert_eq!(merged(MAX_POINTS, 0), Some(i32::MAX as u32));
        assert_eq!(merged(MAX_POINTS, 1), None);
        assert_eq!(merged(u64::MAX, 1), None);
        assert_eq!(merged(0, 0), None);
        assert_eq!(points(u32::MAX, u32::MAX), u64::from(u32::MAX) * u64::from(u32::MAX));
    }

    #[test]
    fn descriptions() {
        assert_eq!(description(false), "Clumps xp orbs together.");
        assert!(!description(true).is_empty());
    }
}
