use super::*;

#[test]
fn scene_saved_view_rechecks_optional_token_ready_phase_and_namespace() {
    let model = Model::new(serde_json::from_value(json!({
        "schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef","name":"Controller",
        "buses":[{"bus":0,"name":"Bus1","enabled":false,"outputs":[{"short_address":0,"name":"Light1"}],"scenes":[{"slot":0,"name":"Evening"}]}]
    })).unwrap()).unwrap();
    let mut snapshot = super::snapshot(model, Phase::Ready);
    let target = "Bus1.Scenes.Evening";
    assert!(crate::service::scene_settings::view::read(
        &snapshot,
        target,
        &RequestContext::default()
    )
    .is_ok());
    assert!(
        crate::service::scene_settings::view::read(&snapshot, target, &context(2)).is_err(),
        "saved view accepted retired supplied token"
    );
    assert!(
        crate::service::scene_settings::view::read(&snapshot, "Bus1.Outputs.Light1", &context(1))
            .is_err(),
        "saved view accepted output namespace"
    );
    snapshot.phase = Phase::Writing;
    assert!(
        crate::service::scene_settings::view::read(&snapshot, target, &context(1)).is_err(),
        "saved view exposed frozen routing"
    );
}

#[tokio::test]
async fn scene_saved_view_reads_private_config_without_uart_and_survives_restart() {
    let fixture = Fixture::new();
    fixture.add_output(1, "Unmanaged");
    let desired = json!({"level":{"kind":"mask"},"color":{"kind":"xy","x":null,"y":20000}});
    let mut disk = serde_json::to_value(fixture.disk()).unwrap();
    disk["buses"][0]["enabled"] = json!(false);
    disk["buses"][0]["scenes"] = json!([{"slot":3,"name":"Evening","contents":[{
        "output_origin":{"target":"Bus1.Outputs.Light1","revision":1},"setting":desired
    }]}]);
    fs::write(&fixture.paths.current, serde_json::to_vec(&disk).unwrap()).unwrap();
    for restart in 0..2 {
        // A fresh broker avoids treating the prior process's historical
        // subscription as proof that this process has subscribed.
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(service, &handle, &broker).await;
        assert!(
            broker
                .wait_for_subscription("DALI/Command/Controller", Duration::from_secs(2))
                .await
        );
        let topic = format!("test/saved-view/{restart}");
        assert!(broker.publish(
            "DALI/Command/Controller",
            json!({"command":"read_scene_settings","target":"Bus1.Scenes.Evening"}).to_string(),
            &SendOptions::default().response_topic(&topic)
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| !b
                    .received_on(&topic)
                    .is_empty())
                .await
        );
        let reply: serde_json::Value =
            serde_json::from_slice(&broker.received_on(&topic)[0].payload).unwrap();
        assert!(
            reply.get("error").is_none(),
            "saved view required hardware or mutation authority: {reply}"
        );
        let view = &reply["value"];
        assert_eq!(view["slot"], 3);
        assert_eq!(view["revision"], 1 + restart);
        assert_eq!(
            view["fixtures"].as_array().unwrap().len(),
            2,
            "saved view truncated configured outputs"
        );
        assert_eq!(view["fixtures"][0]["saved"], desired);
        assert_eq!(view["fixtures"][1]["saved"], serde_json::Value::Null);
        assert_eq!(
            view["fixtures"][0]["fixture_generation"],
            context(1).generation().unwrap()
        );
        let retained: Vec<_> = broker
            .received_on("DALI/State/Controller/__inventory")
            .into_iter()
            .filter(|packet| !packet.payload.is_empty())
            .collect();
        assert!(
            !retained.is_empty(),
            "saved view test missed emitted Inventory"
        );
        for packet in retained {
            let public: serde_json::Value = serde_json::from_slice(&packet.payload).unwrap();
            assert!(
                public["Inventory"]["buses"][0]["scenes"][0]
                    .get("contents")
                    .is_none(),
                "retained Inventory exposed private scene rows"
            );
        }
        if restart == 0 {
            handle
                .edit(
                    1,
                    MetadataEdit::Description(Some("preserve private rows".into())),
                    context(1),
                    deadline(),
                )
                .await
                .unwrap();
        }
        assert!(running.stop().await);
    }
}
