//! Asymptotic armor damage mitigation (Slice 4).

/// Diminishing-returns constant for armor mitigation.
pub const ARMOR_MITIGATION_K: f64 = 100.0;

/// Damage multiplier after armor: `K / (K + armor_rating)`.
pub fn damage_multiplier_for_armor(armor_rating: u32) -> f64 {
    let armor = armor_rating as f64;
    ARMOR_MITIGATION_K / (ARMOR_MITIGATION_K + armor)
}

/// Raw weapon damage after armor, before minimum-damage rules.
pub fn resolve_damage_after_armor(raw_damage: f32, armor_rating: u32) -> u32 {
    if raw_damage <= 0.0 {
        return 0;
    }
    let mitigated = raw_damage as f64 * damage_multiplier_for_armor(armor_rating);
    mitigated.floor() as u32
}

/// Final integer combat damage applied to HP.
///
/// Positive raw hits always deal at least 1 damage so armor cannot grant immunity.
pub fn resolve_applied_combat_damage(raw_damage: f32, armor_rating: u32) -> u32 {
    if raw_damage <= 0.0 {
        return 0;
    }
    let mitigated = resolve_damage_after_armor(raw_damage, armor_rating);
    mitigated.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn multiplier_at_zero_armor_is_one() {
        approx_eq(damage_multiplier_for_armor(0), 1.0);
        assert_eq!(resolve_damage_after_armor(14.0, 0), 14);
    }

    #[test]
    fn multiplier_at_twenty_five_armor() {
        approx_eq(damage_multiplier_for_armor(25), 0.8);
        assert_eq!(resolve_damage_after_armor(10.0, 25), 8);
    }

    #[test]
    fn multiplier_at_fifty_armor() {
        approx_eq(damage_multiplier_for_armor(50), 2.0 / 3.0);
    }

    #[test]
    fn multiplier_at_one_hundred_armor() {
        approx_eq(damage_multiplier_for_armor(100), 0.5);
        assert_eq!(resolve_damage_after_armor(14.0, 100), 7);
    }

    #[test]
    fn multiplier_at_three_hundred_armor() {
        approx_eq(damage_multiplier_for_armor(300), 0.25);
    }

    #[test]
    fn multiplier_at_nine_hundred_armor() {
        approx_eq(damage_multiplier_for_armor(900), 0.1);
    }

    #[test]
    fn multiplier_at_ninety_nine_thousand_nine_hundred_armor() {
        approx_eq(damage_multiplier_for_armor(99_900), 100.0 / 100_000.0);
        let reduction = 1.0 - damage_multiplier_for_armor(99_900);
        approx_eq(reduction, 0.999);
    }

    #[test]
    fn finite_armor_never_reaches_immunity() {
        for armor in [0, 1, 25, 50, 100, 300, 900, 99_900, u32::MAX] {
            let multiplier = damage_multiplier_for_armor(armor);
            assert!(multiplier > 0.0);
            assert!(multiplier <= 1.0);
            assert!(resolve_applied_combat_damage(1.0, armor) >= 1);
        }
    }

    #[test]
    fn full_starter_armor_reduces_iron_sword_to_ten() {
        assert_eq!(resolve_damage_after_armor(14.0, 40), 10);
        assert_eq!(resolve_applied_combat_damage(14.0, 40), 10);
    }

    #[test]
    fn enormous_armor_still_deals_minimum_one_damage() {
        assert_eq!(resolve_applied_combat_damage(5.0, u32::MAX), 1);
    }
}
