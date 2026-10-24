use super::*;

#[tokio::test]
async fn canonical_daemon_requires_property_expiry_within_three_minutes() {
    let fixture = Fixture::new();
    let broker = FakeBroker::start().await;
    broker.cap_connections(8);
    let mut child = fixture.command(&broker).spawn().unwrap();
    let initial = inventory(&broker).await;
    let generation = format!(
        "{}.{}",
        initial["inventory_epoch"].as_str().unwrap(),
        initial["revision"]
    );
    let topic = "DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness";
    let errors = "DALI/Error/Controller";
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(3))
            .await
    );
    for expiry in [None, Some(181)] {
        let before = broker.received_on(errors).len();
        let options = SendOptions {
            message_expiry: expiry,
            ..SendOptions::default()
        }
        .user_property("target-generation", &generation)
        .retained();
        assert!(broker.publish(topic, "0.4", &options));
        assert!(broker.wait_until(Duration::from_secs(3),|b|b.received_on(errors).iter().skip(before).any(|message| {
            serde_json::from_slice::<Value>(&message.payload).is_ok_and(|v| {
                v["reason"]=="malformed" && v["error"].as_str().is_some_and(|e|e.contains("finite MQTT message expiry"))
            })
        })).await,"canonical daemon did not enforce the three-minute property lifetime policy; expiry={expiry:?}");
    }
    // A normal, nonretained metadata Command may omit expiry and still completes.
    rename(&broker, &initial).await;
    assert!(
        broker
            .wait_until(Duration::from_secs(3), |b| !b
                .received_on("test/reply")
                .is_empty())
            .await
    );
    assert!(stop(&mut child).await.success());
    assert_eq!(
        fixture.config()["buses"][0]["outputs"][0]["name"],
        "Kitchen"
    );
}
