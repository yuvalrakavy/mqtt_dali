use super::*;
use crate::{emulator::session::Emulator, service::tests::deadline};

#[tokio::test]
async fn memory_prepared_job_keeps_admission_revision_before_service_dispatch() {
    for (command, params) in [
        ("read_identity", PropertyMap::new()),
        ("read_luminaire_data", PropertyMap::new()),
        (
            "read_energy_data",
            PropertyMap::from_iter([("bank".into(), json!(202))]),
        ),
        (
            "read_diagnostics",
            PropertyMap::from_iter([("bank".into(), json!(205))]),
        ),
        (
            "read_diagnostics",
            PropertyMap::from_iter([("bank".into(), json!(207))]),
        ),
        (
            "read_memory",
            PropertyMap::from_iter([
                ("bank".into(), json!(255)),
                ("start".into(), json!(0)),
                ("count".into(), json!(1)),
            ]),
        ),
    ] {
        let fixture = Fixture::new();
        let scenario=serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[{"id":"lamp","short_address":0,"memory":{"banks":[{"bank":0,"bytes":[0]},{"bank":255,"bytes":[255]}]}}]}]})).unwrap();
        let (emulator, address) = Emulator::listen_tcp("127.0.0.1:0".parse().unwrap(), scenario)
            .await
            .unwrap();
        let broker = FakeBroker::start().await;
        let (service, handle) = fixture.service();
        let running = start(
            service.with_hat_tcp(address, RepeatPolicy::OneTerminal),
            &handle,
            &broker,
        )
        .await;
        wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
        let call = CommandCall {
            command: command.into(),
            params,
        };
        let target = "Bus1.Outputs.Light1";
        let prepare = || {
            PreparedCommand::new(
                &handle.snapshot().unwrap(),
                Some(target.into()),
                &call,
                context(1),
            )
            .unwrap_or_else(|_| panic!("memory preparation failed"))
        };
        let queued = prepare();
        handle
            .edit(
                1,
                MetadataEdit::Description(Some("metadata changed after HTTP admission".into())),
                context(1),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            routing::generations(&handle.snapshot().unwrap().model).targets[target],
            context(1).generation().unwrap(),
            "fixture token rotated; test no longer exercises continuing generation"
        );
        assert!(
            matches!(
                queued.run(handle.clone()).await,
                Err(ServiceError::Rejected(_))
            ),
            "memory prepared job renewed its admitted revision: {command}"
        );
        let fresh = prepare().run(handle.clone()).await.unwrap();
        assert_eq!(
            fresh["revision"], 2,
            "fresh read did not capture its own admission"
        );
        assert_eq!(fixture.disk().revision, 2);
        assert!(running.stop().await);
        emulator.close().await.unwrap();
    }
}
