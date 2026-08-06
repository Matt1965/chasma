//! Floor and region selector presentation logic (IN-10).

/// Floor selector arrow availability (non-wrapping UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorSelectorState {
    pub floor_id: i32,
    pub label_line: String,
    pub can_prev: bool,
    pub can_next: bool,
}

/// Region selector arrow availability (non-wrapping UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSelectorState {
    pub region_label: String,
    pub region_key: String,
    pub index_one_based: usize,
    pub total: usize,
    pub can_prev: bool,
    pub can_next: bool,
    pub severity_hint: RegionSeverityHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegionSeverityHint {
    #[default]
    None,
    Warning,
    Error,
}

pub fn floor_label_line(floor_id: i32, elevation_meters: Option<f32>) -> String {
    let elev = elevation_meters
        .map(|e| format!("{e:.1}"))
        .unwrap_or_else(|| "-".into());
    if floor_id == 0 {
        format!("Ground · {floor_id} ({elev} m)")
    } else {
        format!("Floor {floor_id} ({elev} m)")
    }
}

pub fn floor_selector_state(
    floor_ids: &[i32],
    current_floor_id: Option<i32>,
    elevation_meters: Option<f32>,
) -> Option<FloorSelectorState> {
    if floor_ids.is_empty() {
        return None;
    }
    let index = current_floor_id
        .and_then(|id| floor_ids.iter().position(|&f| f == id))
        .unwrap_or(0);
    let floor_id = floor_ids[index];
    Some(FloorSelectorState {
        floor_id,
        label_line: floor_label_line(floor_id, elevation_meters),
        can_prev: index > 0,
        can_next: index + 1 < floor_ids.len(),
    })
}

pub fn region_display_label(region_key: &str, room_tag: Option<&str>) -> String {
    if let Some(tag) = room_tag.filter(|t| !t.is_empty()) {
        format!("{tag}")
    } else {
        region_key.to_string()
    }
}

pub fn region_selector_state(
    regions: &[(String, Option<String>)],
    current_key: Option<&str>,
    severity_hint: RegionSeverityHint,
) -> RegionSelectorState {
    let total = regions.len();
    if total == 0 {
        return RegionSelectorState {
            region_label: "No regions".into(),
            region_key: "-".into(),
            index_one_based: 0,
            total: 0,
            can_prev: false,
            can_next: false,
            severity_hint,
        };
    }
    let index = current_key
        .and_then(|key| regions.iter().position(|(k, _)| k == key))
        .unwrap_or(0);
    let (key, tag) = &regions[index];
    RegionSelectorState {
        region_label: region_display_label(key, tag.as_deref()),
        region_key: key.clone(),
        index_one_based: index + 1,
        total,
        can_prev: index > 0,
        can_next: index + 1 < total,
        severity_hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_selector_shows_label_and_id() {
        let state = floor_selector_state(&[0, 1], Some(0), Some(0.0)).unwrap();
        assert_eq!(state.floor_id, 0);
        assert!(state.label_line.contains("Ground"));
        assert!(state.label_line.contains('0'));
    }

    #[test]
    fn floor_upper_moves_to_higher_index_in_list() {
        let ids = vec![-1, 0, 2];
        let state = floor_selector_state(&ids, Some(0), None).unwrap();
        assert!(state.can_prev);
        assert!(state.can_next);
        let at_start = floor_selector_state(&ids, Some(-1), None).unwrap();
        assert!(!at_start.can_prev);
        assert!(at_start.can_next);
        let at_end = floor_selector_state(&ids, Some(2), None).unwrap();
        assert!(at_end.can_prev);
        assert!(!at_end.can_next);
    }

    #[test]
    fn single_floor_disables_both_arrows() {
        let state = floor_selector_state(&[0], Some(0), None).unwrap();
        assert!(!state.can_prev);
        assert!(!state.can_next);
    }

    #[test]
    fn region_selector_shows_label_key_and_count() {
        let regions = vec![
            ("room_a".into(), Some("Room A".into())),
            ("room_b".into(), None),
        ];
        let state = region_selector_state(&regions, Some("room_a"), RegionSeverityHint::None);
        assert_eq!(state.region_label, "Room A");
        assert_eq!(state.region_key, "room_a");
        assert_eq!(state.index_one_based, 1);
        assert_eq!(state.total, 2);
    }

    #[test]
    fn region_boundaries_disable_arrows() {
        let regions = vec![("a".into(), None), ("b".into(), None)];
        let first = region_selector_state(&regions, Some("a"), RegionSeverityHint::None);
        assert!(!first.can_prev);
        assert!(first.can_next);
        let last = region_selector_state(&regions, Some("b"), RegionSeverityHint::None);
        assert!(last.can_prev);
        assert!(!last.can_next);
    }

    #[test]
    fn one_region_floor_is_stable() {
        let regions = vec![("only".into(), Some("Only".into()))];
        let state = region_selector_state(&regions, Some("only"), RegionSeverityHint::None);
        assert_eq!(state.index_one_based, 1);
        assert_eq!(state.total, 1);
        assert!(!state.can_prev);
        assert!(!state.can_next);
    }

    #[test]
    fn no_regions_shows_clear_message() {
        let state = region_selector_state(&[], None, RegionSeverityHint::None);
        assert_eq!(state.region_label, "No regions");
        assert_eq!(state.total, 0);
    }
}
