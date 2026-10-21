use super::*;
#[test]
fn literal_names_roundtrip_without_unicode_case_or_whitespace_normalization() {
    for (bus, namespace, name, literal) in [
        (
            "Bus1",
            Namespace::Outputs,
            "KitchenCeiling",
            "Bus1.Outputs.KitchenCeiling",
        ),
        (
            "Bus.1",
            Namespace::Groups,
            "π%+#",
            "Bus%2E1.Groups.%CF%80%25%2B%23",
        ),
        (" Bus1 ", Namespace::Scenes, "é", "%20Bus1%20.Scenes.%C3%A9"),
        ("Bus1", Namespace::Inputs, "%2E", "Bus1.Inputs.%252E"),
    ] {
        let target = Target::item(bus, namespace, name).unwrap();
        assert_eq!(target.as_str(), literal);
        assert_eq!(Target::parse(literal).unwrap(), target);
    }
    let instance = Target::input_instance("Bus.1", "Wall Panel", "Key.1").unwrap();
    assert_eq!(
        instance.as_str(),
        "Bus%2E1.Inputs.Wall%20Panel.Instances.Key%2E1"
    );
    assert_eq!(Target::parse(instance.as_str()).unwrap(), instance);
    assert_ne!(
        Target::item("Bus1", Namespace::Outputs, "é").unwrap(),
        Target::item("Bus1", Namespace::Outputs, "e\u{301}").unwrap()
    );
}
#[test]
fn namespaces_metadata_and_escaped_names_never_collide() {
    let metadata = Target::inventory();
    assert_eq!(Target::parse("__inventory").unwrap(), metadata);
    assert_ne!(
        metadata,
        Target::item("__inventory", Namespace::Outputs, "__inventory").unwrap()
    );
    let mut seen = std::collections::HashSet::new();
    for name in ["a.b", "a%2Eb", "a b", "a+b", "a#b", "a_b", "a-b"] {
        for namespace in [
            Namespace::Outputs,
            Namespace::Inputs,
            Namespace::Groups,
            Namespace::Scenes,
        ] {
            assert!(seen.insert(
                Target::item("Bus1", namespace, name)
                    .unwrap()
                    .as_str()
                    .to_owned()
            ));
        }
    }
    assert_ne!(
        Target::item("Bus1", Namespace::Inputs, "Wall.Instances.Key").unwrap(),
        Target::input_instance("Bus1", "Wall", "Key").unwrap()
    );
}
#[test]
fn parsing_rejects_noncanonical_aliases_invalid_names_and_structure() {
    for invalid in [
        "",
        "Bus1.Outputs",
        "Bus1.outputs.Lamp",
        "Bus1.Outputs.",
        ".Outputs.Lamp",
        "Bus1.Outputs.A.B",
        "Bus1.Outputs.A.Instances.B",
        "Bus1.Inputs.A.instances.B",
        "Bus1.%4Futputs.A",
        "Bus1.Outputs.%41",
        "Bus1.Outputs.%2e",
        "Bus1.Outputs.%c3%a9",
        "Bus1.Outputs.%",
        "Bus1.Outputs.%GG",
        "Bus1.Outputs.%FF",
        "Bus1.Outputs.A+B",
        "Bus1.Outputs.π",
        "Bus1.Outputs.%2F",
        "Bus1.Outputs.%00",
        "Bus1.Outputs.%20",
        "Bus1.Outputs.%2E",
        "Bus1.Outputs.%2E%2E",
        "Bus1.Outputs.A/B",
        "Bus1.Inputs.A.Instances.%0A",
    ] {
        assert!(
            Target::parse(invalid).is_err(),
            "accepted invalid target {invalid}"
        );
    }
    for invalid in ["", " ", ".", "..", "a/b", "a\n"] {
        assert!(Target::item("Bus1", Namespace::Outputs, invalid).is_err());
        assert!(Target::input_instance("Bus1", "Module", invalid).is_err());
    }
}
#[test]
fn encoded_bound_counts_all_separators_and_namespace_bytes() {
    let target = Target::item("B", Namespace::Outputs, &"x".repeat(4086)).unwrap();
    assert_eq!(target.as_str().len(), 4096);
    assert_eq!(Target::parse(target.as_str()).unwrap(), target);
    assert!(
        Target::item("B", Namespace::Outputs, &"x".repeat(4087)).is_err(),
        "4097-byte target bypassed encoded limit"
    );
    assert!(Target::parse(&format!("{}x", target.as_str())).is_err());
    assert!(
        Target::item("B", Namespace::Outputs, &"π".repeat(700)).is_err(),
        "limit counted decoded instead of encoded bytes"
    );
    let instance = Target::input_instance("B", "M", &"x".repeat(4075)).unwrap();
    assert_eq!(instance.as_str().len(), 4096);
    assert_eq!(Target::parse(instance.as_str()).unwrap(), instance);
    assert!(Target::input_instance("B", "M", &"x".repeat(4076)).is_err());
}
