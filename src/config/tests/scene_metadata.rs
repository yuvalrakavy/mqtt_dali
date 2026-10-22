use super::*;

#[test]
fn scene_metadata_valid_json_roundtrips_without_changing_legacy_or_empty_documents() {
    let fixture = Fixture::new();
    let original = fixture.legacy(&legacy());
    let imported = fixture.load().unwrap().inventory;
    let mut value = serde_json::to_value(&imported).unwrap();
    assert!(value["buses"][0].get("scenes").is_none());
    value["buses"][0]["scenes"] = json!([
        {"slot":15,"name":"Evening","description":"Native scene"},
        {"slot":0,"name":"Light1"}
        ,{"slot":3,"name":"Counter"}
    ]);
    let inventory: Inventory =
        serde_json::from_value(value).expect("valid named scene JSON was rejected");
    let model = crate::inventory::Model::new(inventory.clone()).unwrap();
    let state: Value = serde_json::from_slice(&model.state_bytes().unwrap()).unwrap();
    let scene = &state["Inventory"]["buses"][0]["scenes"][0];
    assert_eq!(scene["slot"], 15);
    assert_eq!(
        scene["target"], "KitchenBus.Scenes.Evening",
        "scene target was not published"
    );
    assert_eq!(scene["binding_revision"], 1);
    fs::write(
        &fixture.paths.current,
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
    assert_eq!(fixture.load().unwrap().inventory, inventory);
    assert_eq!(fs::read(&fixture.paths.legacy).unwrap(), original);
}

#[test]
fn scene_metadata_rejects_invalid_slots_names_and_lists() {
    let fixture = Fixture::new();
    fixture.legacy(&legacy());
    let base = serde_json::to_value(fixture.load().unwrap().inventory).unwrap();
    for scenes in [
        json!(null),
        json!({}),
        json!("bad"),
        json!([{"slot":16,"name":"Bad"}]),
        json!([{"slot":-1,"name":"Bad"}]),
        json!([{"slot":1.5,"name":"Bad"}]),
        json!([{"slot":false,"name":"Bad"}]),
        json!([{"slot":0,"name":""}]),
        json!([{"slot":0,"name":"a/b"}]),
        json!([{"slot":0,"name":"A"},{"slot":0,"name":"B"}]),
        json!([{"slot":0,"name":"A"},{"slot":1,"name":"A"}]),
        json!([{"slot":0}]),
        json!([{"name":"A"}]),
        json!([{"slot":0,"name":"A","description":3}]),
        json!([{"slot":0,"name":"A","members":[1]}]),
    ] {
        let mut value = base.clone();
        value["buses"][0]["scenes"] = scenes.clone();
        assert!(
            serde_json::from_value::<Inventory>(value).map_or(true, |inventory| {
                crate::config::validate_structure(&inventory).is_err()
            }),
            "invalid scene metadata was accepted: {scenes}"
        );
    }
}
