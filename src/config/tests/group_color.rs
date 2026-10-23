use super::*;

#[test]
fn group_color_optional_representation_survives_config_and_identity_projection() {
    for representation in [
        json!("temperature"),
        json!("xy"),
        json!("rgbwaf"),
        Value::Null,
    ] {
        let group = json!({"group_address":0,"name":"All","members":[0],"color_representation":representation});
        let parsed: Group = serde_json::from_value(group.clone())
            .expect("saved group color representation is not supported");
        let serialized = serde_json::to_value(&parsed).unwrap();
        if representation.is_null() {
            assert!(serialized.get("color_representation").is_none());
        } else {
            assert_eq!(serialized["color_representation"], representation);
        }
        let inventory: Inventory = serde_json::from_value(json!({
            "schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef",
            "name":"Controller","buses":[{"bus":0,"name":"Bus1","enabled":true,
                "outputs":[{"short_address":0,"name":"Light1"}],"groups":[group]}]
        }))
        .unwrap();
        let model = crate::inventory::Model::new(inventory).unwrap();
        let projected = model
            .projections()
            .into_iter()
            .find(|p| p.target == "Bus1.Groups.All")
            .unwrap();
        assert_eq!(
            serde_json::to_value(projected.color_representation).unwrap(),
            representation,
            "group representation is missing from the binding contract"
        );
        let reloaded = crate::inventory::Model::new(
            serde_json::from_slice(&serde_json::to_vec(model.inventory()).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reloaded.state_bytes().unwrap(),
            model.state_bytes().unwrap()
        );
    }
    let old: Group =
        serde_json::from_value(json!({"group_address":0,"name":"All","members":[]})).unwrap();
    assert!(serde_json::to_value(old)
        .unwrap()
        .get("color_representation")
        .is_none());
    for invalid in [
        json!("primary_n"),
        json!({"xy":null}),
        json!(true),
        json!(1),
    ] {
        assert!(serde_json::from_value::<Group>(
            json!({"group_address":0,"name":"All","members":[],"color_representation":invalid})
        )
        .is_err());
    }
}
