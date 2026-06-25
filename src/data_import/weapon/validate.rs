use super::schema::WeaponImportRow;

pub fn validate_row(row: &WeaponImportRow) -> Result<(), crate::data_import::RowImportError> {
    let fail = |message: String| crate::data_import::RowImportError {
        row_number: row.row_number,
        message,
    };

    if row.weapon_id.trim().is_empty() {
        return Err(fail("Weapon ID must be non-empty".to_string()));
    }
    if row.name.trim().is_empty() {
        return Err(fail("Name must be non-empty".to_string()));
    }
    if row.damage < 0.0 {
        return Err(fail(format!("Damage must be >= 0 (got {})", row.damage)));
    }
    if row.range_meters < 0.0 {
        return Err(fail(format!(
            "Range must be >= 0 (got {})",
            row.range_meters
        )));
    }
    if row.attacks_per_second <= 0.0 {
        return Err(fail(format!(
            "Attacks Per Second must be > 0 (got {})",
            row.attacks_per_second
        )));
    }
    if row.windup_seconds < 0.0 {
        return Err(fail(format!(
            "Windup must be >= 0 (got {})",
            row.windup_seconds
        )));
    }
    if row.recovery_seconds < 0.0 {
        return Err(fail(format!(
            "Recovery must be >= 0 (got {})",
            row.recovery_seconds
        )));
    }
    if !row.damage.is_finite()
        || !row.range_meters.is_finite()
        || !row.attacks_per_second.is_finite()
        || !row.windup_seconds.is_finite()
        || !row.recovery_seconds.is_finite()
    {
        return Err(fail("numeric fields must be finite".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::weapon::schema::WeaponImportRow;
    use crate::world::{DamageType, HitMode, TargetFilter};

    fn sample_row(aps: f32) -> WeaponImportRow {
        WeaponImportRow {
            row_number: 2,
            weapon_id: "weapon_fists".to_string(),
            name: "Fists".to_string(),
            description: String::new(),
            damage: 4.0,
            damage_type: DamageType::Blunt,
            range_meters: 1.0,
            attacks_per_second: aps,
            windup_seconds: 0.1,
            recovery_seconds: 0.1,
            hit_mode: HitMode::Melee,
            projectile_key: None,
            animation_key: "attack_fists".to_string(),
            target_filters: vec![TargetFilter::Enemies],
            stat_scaling: None,
            enabled: true,
            enabled_was_blank: false,
        }
    }

    #[test]
    fn attacks_per_second_must_be_positive() {
        assert!(validate_row(&sample_row(0.0)).is_err());
        assert!(validate_row(&sample_row(1.5)).is_ok());
    }

    #[test]
    fn derived_cooldown_from_row_definition() {
        let def = sample_row(2.0).to_definition();
        assert!((def.attack_cooldown_seconds() - 0.5).abs() < 1e-4);
    }
}
