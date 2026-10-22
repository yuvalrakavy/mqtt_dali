use super::super::*;

#[tokio::test(start_paused = true)]
async fn local_command_keeps_the_original_deadline_before_queued_execution() {
    let snapshot = crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    let call = CommandCall {
        command: "describe".into(),
        params: PropertyMap::from_iter([
            ("expected_revision".into(), Value::from(1)),
            ("description".into(), Value::Null),
        ]),
    };
    let context = RequestContext::new("0123456789abcdef0123456789abcdef.1")
        .unwrap()
        .with_traceparent(None);
    let prepared = PreparedCommand::new(&snapshot, None, &call, context)
        .ok()
        .unwrap();
    tokio::time::advance(Duration::from_secs(2)).await;
    // No owner was opened: renewed work would fail Closed/starting, not Expired.
    let (_, handle) = DaliService::new(
        crate::config::ConfigPaths::in_directory("/tmp/unopened-dali-command-deadline"),
        "Controller".into(),
    );
    assert_eq!(
        prepared.run(handle).await,
        Err(ServiceError::Expired),
        "queued HTTP command renewed its original deadline"
    );
}

#[tokio::test]
async fn local_command_uses_the_existing_service_permit_after_preparation() {
    let snapshot = crate::service::tests::snapshot(crate::service::tests::model(), Phase::Ready);
    let call = CommandCall {
        command: "describe".into(),
        params: serde_json::from_value(
            serde_json::json!({"expected_revision":1,"description":null}),
        )
        .unwrap(),
    };
    let prepared = PreparedCommand::new(
        &snapshot,
        None,
        &call,
        RequestContext::new("0123456789abcdef0123456789abcdef.1").unwrap(),
    )
    .ok()
    .unwrap();
    let (service, handle) = DaliService::new(
        crate::config::ConfigPaths::in_directory("/tmp/unopened-dali-command-permit"),
        "Controller".into(),
    );
    let _held = handle.permit.clone().try_acquire_owned().unwrap();
    assert_eq!(
        prepared.run(handle).await,
        Err(ServiceError::Busy),
        "HTTP command bypassed the real service permit"
    );
    drop(service);
}
