use super::*;

#[test]
fn starter_catalog_has_distinct_origins() {
    let catalog = starter_origin_catalog();
    assert_eq!(catalog.definitions().len(), 2);
    let exile = catalog.get(&OriginId::new("exile_pair")).unwrap();
    let lone = catalog.get(&OriginId::new("lone_survivor")).unwrap();
    assert_eq!(exile.roster_size(), 2);
    assert_eq!(lone.roster_size(), 1);
    assert_ne!(exile.id, lone.id);
}

#[test]
fn roster_members_keep_independent_definition_ids() {
    let catalog = starter_origin_catalog();
    let exile = catalog.get(&OriginId::new("exile_pair")).unwrap();
    assert_ne!(
        exile.roster[0].definition_id.as_str(),
        exile.roster[1].definition_id.as_str()
    );
}
