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
    if row.hit_mode == crate::world::HitMode::Projectile {
        if row.projectile_speed_mps <= 0.0 {
            return Err(fail(format!(
                "Projectile Speed must be > 0 for Projectile hit mode (got {})",
                row.projectile_speed_mps
            )));
        }
    }
    if !row.damage.is_finite()
        || !row.range_meters.is_finite()
        || !row.attacks_per_second.is_finite()
        || !row.windup_seconds.is_finite()
        || !row.recovery_seconds.is_finite()
    {
        return Err(fail("numeric fields must be finite".to_string()));
    }

    if row.animation_key.trim().is_empty() {
        return Err(fail(format!(
            "Weapon `{}` row {}: Animation Key must be non-empty",
            row.weapon_id, row.row_number
        )));
    }
    let key = row.animation_key.trim();
    if key.chars().any(char::is_whitespace) {
        return Err(fail(format!(
            "Weapon `{}` row {}: Animation Key must not contain whitespace (`{key}`)",
            row.weapon_id, row.row_number
        )));
    }
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(fail(format!(
            "Weapon `{}` row {}: Animation Key has invalid characters (`{key}`)",
            row.weapon_id, row.row_number
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_import::weapon::schema::WeaponImportRow;
    use crate::world::{AttackPlaybackPolicy, DamageType, HitMode, TargetFilter};

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
            projectile_speed_mps: 0.0,
            animation_key: "attack_fists".to_string(),
            attack_playback_policy: AttackPlaybackPolicy::default(),
            normalized_strike_time:
                crate::data_import::weapon::schema::DEFAULT_NORMALIZED_STRIKE_TIME,
            attack_blend_in_ms: crate::data_import::weapon::schema::DEFAULT_ATTACK_BLEND_MS,
            attack_blend_out_ms: crate::data_import::weapon::schema::DEFAULT_ATTACK_BLEND_MS,
            attack_variant: None,
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
    fn projectile_speed_required_for_projectile_hit_mode() {
        let mut row = sample_row(1.5);
        row.hit_mode = HitMode::Projectile;
        row.projectile_speed_mps = 0.0;
        assert!(validate_row(&row).is_err());
        row.projectile_speed_mps = 12.0;
        assert!(validate_row(&row).is_ok());
    }

    #[test]
    fn blank_animation_key_rejected() {
        let mut row = sample_row(1.5);
        row.animation_key = "   ".to_string();
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("Animation Key"));
        assert!(err.message.contains("weapon_fists"));
    }

    #[test]
    fn malformed_animation_key_rejected() {
        let mut row = sample_row(1.5);
        row.animation_key = "attack clip".to_string();
        let err = validate_row(&row).unwrap_err();
        assert!(err.message.contains("whitespace"));
    }

    #[test]
    fn valid_animation_key_accepted() {
        let mut row = sample_row(1.5);
        row.animation_key = "attack_bite".to_string();
        assert!(validate_row(&row).is_ok());
    }
}
