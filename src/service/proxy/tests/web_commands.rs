use super::super::*;
use crate::emulator::{pty, session::Control};
use crate::service::tests::{context, deadline, start, wait_for, Fixture};
use mqtt_test_broker::FakeBroker;
use std::{
    future::{poll_fn, Future},
    task::Poll,
};

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

#[tokio::test]
async fn local_observed_reply_rechecks_mapping_after_a_ready_service_response() {
    for input in [false, true] {
        let fixture = Fixture::new();
        fixture.add_button();
        let emulator=pty::spawn(serde_json::from_value(serde_json::json!({"version":1,"buses":[{"number":0,
            "outputs":[{"id":"lamp","short_address":0,"level":77}],
            "inputs":[{"id":"panel","short_address":0,"instances":[1],"values":[{"instance":0,"resolution":1,"bytes":[255]}]}]}]})).unwrap()).await.unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_uart(
                emulator.path.clone(),
                crate::atx::transactions::RepeatPolicy::Unverified,
            ),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let target = if input {
            "Bus1.Inputs.Panel.Instances.Button"
        } else {
            "Bus1.Outputs.Light1"
        };
        let call = CommandCall {
            command: if input { "read_input" } else { "read_output" }.into(),
            params: PropertyMap::new(),
        };
        let prepared = PreparedCommand::new(
            &handle.snapshot().unwrap(),
            Some(target.into()),
            &call,
            context(1),
        )
        .ok()
        .unwrap();
        let mut reply = Box::pin(prepared.run(handle.clone()));
        poll_fn(|cx| {
            assert!(matches!(reply.as_mut().poll(cx), Poll::Pending));
            Poll::Ready(())
        })
        .await;
        wait_for(&handle, |s| {
            let observed = if input {
                s.inputs.contains_key(target)
            } else {
                s.outputs.contains_key(target)
            };
            observed && handle.permit.available_permits() == 1
        })
        .await;
        let edit = if input {
            MetadataEdit::RenameInputInstance {
                bus: 0,
                address: 0,
                instance: 0,
                name: "Renamed".into(),
            }
        } else {
            MetadataEdit::RenameOutput {
                bus: 0,
                address: 0,
                name: "Renamed".into(),
            }
        };
        handle.edit(1, edit, context(1), deadline()).await.unwrap();
        assert!(reply.await.is_err(),"HTTP completed an observation against a retired mapping after its service reply was ready; input={input}");
        assert!(running.stop().await);
        emulator.emulator.close().await.unwrap();
    }
}

#[tokio::test]
async fn local_input_reply_projects_a_newer_native_value_after_a_ready_query() {
    let fixture = Fixture::new();
    fixture.add_button();
    let emulator=pty::spawn(serde_json::from_value(serde_json::json!({"version":1,"buses":[{"number":0,
        "inputs":[{"id":"panel","short_address":0,"instances":[1],"values":[{"instance":0,"resolution":1,"bytes":[255]}]}]}]})).unwrap()).await.unwrap();
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    let running = start(
        service.with_uart(
            emulator.path.clone(),
            crate::atx::transactions::RepeatPolicy::Unverified,
        ),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    let target = "Bus1.Inputs.Panel.Instances.Button";
    let call = CommandCall {
        command: "read_input".into(),
        params: PropertyMap::new(),
    };
    let prepared = PreparedCommand::new(
        &handle.snapshot().unwrap(),
        Some(target.into()),
        &call,
        context(1),
    )
    .ok()
    .unwrap();
    let mut reply = Box::pin(prepared.run(handle.clone()));
    poll_fn(|cx| {
        assert!(matches!(reply.as_mut().poll(cx), Poll::Pending));
        Poll::Ready(())
    })
    .await;
    wait_for(&handle, |s| {
        s.inputs.contains_key(target) && handle.permit.available_permits() == 1
    })
    .await;
    emulator
        .emulator
        .control(
            Control::InputEvent {
                bus: 0,
                frame: 0x008000,
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    wait_for(&handle, |s| {
        matches!(
            s.inputs.get(target).map(|o| &o.evidence),
            Some(input::Evidence::Event { .. })
        )
    })
    .await;
    let actual = reply.await.unwrap();
    assert_eq!(
        actual["values"]["Status"], false,
        "HTTP input reply overwrote newer native truth with a ready query"
    );
    assert_eq!(actual["values"]["ValueSource"], "event");
    assert!(running.stop().await);
    emulator.emulator.close().await.unwrap();
}
