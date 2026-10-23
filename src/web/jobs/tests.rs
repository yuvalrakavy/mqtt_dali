use super::*;
use crate::service::{Hardware, Phase, Snapshot};
use mqtt_bridge_kit::{CommandCall, RequestContext};

fn prepared() -> PreparedCommand {
    let model = crate::inventory::Model::new(
        serde_json::from_value(json!({
            "schema_version":2,"revision":1,"inventory_epoch":"0123456789abcdef0123456789abcdef",
            "name":"Controller","buses":[]
        }))
        .unwrap(),
    )
    .unwrap();
    let snapshot = Snapshot {
        identification: None,
        model: Arc::new(model),
        phase: Phase::Ready,
        hardware: Hardware::NotStarted,
        outputs: Default::default(),
        group_outputs: Default::default(),
        inputs: Default::default(),
        feedbacks: Default::default(),
        input_contexts: Default::default(),
        input_listeners: Default::default(),
        version: "test",
        build_utc: "test",
    };
    PreparedCommand::new(
        &snapshot,
        None,
        &CommandCall {
            command: "describe".into(),
            params: serde_json::from_value(json!({"expected_revision":1,"description":null}))
                .unwrap(),
        },
        RequestContext::new("0123456789abcdef0123456789abcdef.1")
            .unwrap()
            .with_traceparent(None),
    )
    .ok()
    .unwrap()
}
fn admit(jobs: &Jobs) -> Option<Record> {
    jobs.admit("describe".into(), None, prepared())
}
#[test]
fn web_jobs_active_capacity_includes_mailbox_and_closed_owner_preserves_history() {
    let (jobs, receiver) = Jobs::new();
    for _ in 0..4 {
        assert!(admit(&jobs).is_some());
    }
    assert!(
        admit(&jobs).is_none(),
        "HTTP active job bound ignored queued requests"
    );
    assert_eq!(jobs.list().len(), 4);
    drop(receiver);
    assert_eq!(
        jobs.slots.available_permits(),
        4,
        "dropping job owner retained pending request permits"
    );
    assert!(
        admit(&jobs).is_none(),
        "retained HTTP State admitted work after owner ended"
    );
    assert_eq!(
        jobs.list().len(),
        4,
        "closed admission changed retained history"
    );
}

#[test]
fn web_jobs_history_evicts_oldest_completed_without_evicting_running_or_closed_admission() {
    let (jobs, mut receiver) = Jobs::new();
    let running = admit(&jobs).unwrap();
    let held = receiver.0.try_recv().unwrap();
    {
        let mut records = jobs.records.lock().unwrap();
        // Reverse lexical order deliberately: FIFO is admission order, not ID.
        for i in (1..32).rev() {
            records.push_back(Record {
                id: format!("{i:032x}"),
                command: "describe".into(),
                target: None,
                status: "succeeded",
                result: Some(json!({"revision":i})),
                error: None,
            });
        }
    }
    let newest = admit(&jobs).unwrap();
    let list = jobs.list();
    assert_eq!(list.len(), 32, "HTTP result history exceeded its bound");
    assert_eq!(list[0].id, running.id, "HTTP history evicted a running job");
    assert_eq!(
        list[1].id,
        format!("{:032x}", 30),
        "HTTP history did not evict oldest completed admission"
    );
    assert_eq!(list[31].id, newest.id);
    drop(receiver);
    let before = serde_json::to_value(jobs.list()).unwrap();
    assert!(admit(&jobs).is_none());
    assert_eq!(
        serde_json::to_value(jobs.list()).unwrap(),
        before,
        "closed full admission erased a previous result"
    );
    jobs.finish(held.id.clone(), Ok(json!({"revision":2})));
    drop(held);
    let finished = jobs.get(&running.id).unwrap();
    assert_eq!(finished.status, "succeeded");
    assert_eq!(finished.result, Some(json!({"revision":2})));
}

#[test]
fn web_jobs_results_preserve_partial_outcomes_but_hide_private_reasons_and_limit_size() {
    let private = "/private/install/dali.v2.json: sensitive save failure";
    let report = crate::service::commissioning::Report {
        space: crate::service::discovery::Space::Inputs,
        outcome: crate::service::commissioning::AssignmentOutcome::ReservedButUnconfirmed,
        bus: 0,
        address: Some(3),
        target: Some("Bus1.Inputs.Input1".into()),
        revision: 7,
        reason: Some(private.into()),
    };
    let (result, error) = public_result(Ok(serde_json::to_value(report).unwrap()));
    assert!(error.is_none());
    let result = result.unwrap();
    assert_eq!(
        result["outcome"], "reserved_but_unconfirmed",
        "HTTP report hid partial physical outcome"
    );
    assert_eq!(result["address"], 3);
    assert!(
        !result.to_string().contains(private),
        "HTTP report leaked a private save reason"
    );
    let (result, error) = public_result(Err(ServiceError::Uncertain(private.into())));
    assert!(result.is_none());
    assert_eq!(error.as_ref().unwrap()["code"], "uncertain");
    assert!(
        !error.unwrap().to_string().contains(private),
        "HTTP job leaked a private service error"
    );
    let oversized = json!({"target":"x".repeat(RESULT_BYTES)});
    let (result, error) = public_result(Ok(oversized));
    assert!(result.is_none(), "HTTP job retained an oversized result");
    assert_eq!(error.unwrap()["code"], "result_unavailable");
}

#[test]
fn scene_saved_view_job_summaries_exclude_results_and_only_reader_gets_large_budget() {
    let (jobs, mut receiver) = Jobs::new();
    let big = json!({"fixtures":"x".repeat(40000)});
    let reader = jobs
        .admit("read_scene_settings".into(), None, prepared())
        .unwrap();
    let request = receiver.0.try_recv().unwrap();
    jobs.finish(request.id, Ok(big.clone()));
    assert_eq!(
        jobs.get(&reader.id).unwrap().result,
        Some(big.clone()),
        "saved-view job lost bounded large reply"
    );
    assert!(
        jobs.list()[0].result.is_none(),
        "job summaries retained full results"
    );
    let ordinary = admit(&jobs).unwrap();
    let request = receiver.0.try_recv().unwrap();
    jobs.finish(request.id, Ok(big));
    assert_eq!(
        jobs.get(&ordinary.id).unwrap().error.unwrap()["code"],
        "result_unavailable",
        "ordinary command gained large result budget"
    );
    let (_, error) = public_result_with_budget(
        Ok(json!({"fixtures":"x".repeat(SCENE_RESULT_BYTES)})),
        SCENE_RESULT_BYTES,
    );
    assert_eq!(
        error.unwrap()["code"],
        "result_unavailable",
        "saved-view result lost its upper bound"
    );
}

#[test]
fn group_output_jobs_keep_complete_dense_rosters_with_a_bounded_result_budget() {
    use crate::{
        service::{
            group_output::{Member, Report, Values},
            scenes::Reading,
        },
        targets::{Namespace, Target},
    };
    let members = (0..64)
        .map(|address| Member {
            target: Target::item(
                "Bus1",
                Namespace::Outputs,
                &format!("{}{:02}", "L".repeat(4000), address),
            )
            .unwrap()
            .to_string(),
            target_generation: "0123456789abcdef0123456789abcdef.1".into(),
            address,
            expected_member: true,
            member: Some(true),
            membership_low: Some(Reading::Value { raw: 255 }),
            membership_high: Some(Reading::Value { raw: 255 }),
            actual_level: Some(Reading::Value { raw: 127 }),
            status: Some(Reading::Value { raw: 4 }),
        })
        .collect();
    let mut report = Report {
        operation: "read_group_output",
        outcome: "observed",
        target: "Bus1.Groups.All".into(),
        target_generation: "0123456789abcdef0123456789abcdef.1".into(),
        revision: 1,
        bus: 0,
        group: 15,
        coverage: "saved_inventory",
        membership_matches: Some(true),
        values: Some(Values {
            quality: "uniform",
            power_state: Some(true),
            brightness: Some(0.5),
            power_mixed: Some(false),
            brightness_mixed: Some(false),
            known_members: 64,
            unresolved: vec![],
        }),
        members,
        reason: None,
        configuration_changed: false,
    };
    for command in [
        "read_group_output",
        "set_group_output",
        "read_group_color",
        "set_group_color",
    ] {
        report.operation = command;
        report.outcome = if command.starts_with("read_") {
            "observed"
        } else {
            "sent"
        };
        let value = serde_json::to_value(&report).unwrap();
        let bytes = serde_json::to_vec(&value).unwrap().len();
        assert!(bytes > RESULT_BYTES && bytes < SCENE_RESULT_BYTES);
        let (jobs, mut receiver) = Jobs::new();
        let record = jobs
            .admit(command.into(), Some(report.target.clone()), prepared())
            .unwrap();
        let request = receiver.0.try_recv().unwrap();
        jobs.finish(request.id, Ok(value.clone()));
        let finished = jobs.get(&record.id).unwrap();
        assert_eq!(
            finished.status, "succeeded",
            "dense group report was discarded after native work"
        );
        assert_eq!(finished.result, Some(value), "group roster was truncated");
        assert!(
            jobs.list()[0].result.is_none(),
            "group job summary included its large result"
        );
        let oversized = jobs.admit(command.into(), None, prepared()).unwrap();
        let request = receiver.0.try_recv().unwrap();
        jobs.finish(
            request.id,
            Ok(json!({"members":"x".repeat(SCENE_RESULT_BYTES)})),
        );
        assert_eq!(
            jobs.get(&oversized.id).unwrap().error.unwrap()["code"],
            "result_unavailable",
            "group result lost its upper bound"
        );
    }
}
