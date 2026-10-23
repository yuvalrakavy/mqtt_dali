use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn group_scalar_literal_ignored_write_unknowns_and_pre_send_cut_are_truthful() {
    let fixture = Fixture::new();
    let pty = crate::service::hat::tests::Pty::new();
    let path = pty.path.clone();
    let mode = Arc::new(AtomicU8::new(0));
    let native_mode = mode.clone();
    let owner: Arc<std::sync::Mutex<Option<ServiceHandle>>> = Arc::new(std::sync::Mutex::new(None));
    let native_owner = owner.clone();
    let cut = Arc::new(AtomicBool::new(false));
    let native_cut = cut.clone();
    let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
    let native_commands = commands.clone();
    let peer = tokio::spawn(async move {
        let mut pty = pty;
        let (read, mut write) = tokio::io::split(&mut pty.peer);
        let mut read = BufReader::new(read);
        let mut line = Vec::new();
        loop {
            if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                break;
            }
            native_commands.lock().unwrap().push(line.clone());
            let mode = native_mode.load(Ordering::SeqCst);
            let response: &[u8] = match line.as_slice() {
                b"v\n" => b"V013204\n",
                b"d\n" => b"D24\n",
                b"h01C0\n" if mode == 5 => b"N\n",
                b"h01C0\n" if mode == 4 => b"J00\n",
                b"h01C0\n" => b"JFF\n",
                b"h01C1\n" if mode == 1 || mode == 5 => b"N\n",
                b"h01C1\n" if mode == 4 => b"J00\n",
                b"h01C1\n" => b"JFF\n",
                b"h01A0\n" if mode == 2 => b"JFF\n",
                b"h01A0\n" => b"J7F\n",
                b"h0190\n" if mode == 3 => b"N\n",
                b"h0190\n" => b"JFF\n",
                b"h0199\n" => b"J00\n",
                b"h8100\n" => b"N\n", // Ignored native group OFF: actual remains127.
                b"h00FE\n" => {
                    let stale = native_owner
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .snapshot()
                        .unwrap()
                        .group_outputs[TARGET]
                        .stale;
                    native_cut.store(stale, Ordering::SeqCst);
                    b"N\n" // Clamp maximum request to the same actual127.
                }
                other => panic!("unexpected group scalar literal wire command: {other:?}"),
            };
            write.write_all(response).await.unwrap();
            line.clear();
        }
    });
    let broker = FakeBroker::start().await;
    let (service, handle) = fixture.service();
    *owner.lock().unwrap() = Some(handle.clone());
    let running = start(
        service.with_uart(path, RepeatPolicy::Unverified),
        &handle,
        &broker,
    )
    .await;
    wait_for(&handle, |s| matches!(s.hardware, Hardware::Ready { .. })).await;
    assert!(
        broker
            .wait_for_subscription("DALI/Desired/Controller/+/+", Duration::from_secs(2))
            .await
    );
    let options =
        SendOptions::default().user_property("target-generation", context(1).generation().unwrap());
    assert!(broker.send_with(
        &format!("DALI/Desired/Controller/{TARGET}/PowerState"),
        "false",
        &options
    ));
    let state = values(&broker, |v| v["Available"] == true).await;
    assert_eq!(
        state["PowerState"], true,
        "ignored group OFF echoed requested power"
    );
    assert_eq!(state["Brightness"], 0.5);
    assert_eq!(
        commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_slice() == b"h8100\n")
            .count(),
        1,
        "group scalar command was not a single native send"
    );
    handle
        .apply_output(
            "Bus1.Outputs.Light1",
            crate::dali::gear::Intent::new(None, Some(1.0)).unwrap(),
            context(1),
            deadline(),
        )
        .await
        .unwrap();
    assert!(
        cut.load(Ordering::SeqCst),
        "group actual was not invalidated before the individual native send"
    );
    let stale = values(&broker, |v| {
        v["Available"] == false && v["Quality"] == "stale"
    })
    .await;
    for field in [
        "PowerState",
        "Brightness",
        "PowerMixed",
        "BrightnessMixed",
        "KnownMembers",
        "MembershipMatches",
    ] {
        assert!(
            stale[field].is_null(),
            "stale merged group retained old evidence: {field}"
        );
    }
    for (fault, quality) in [
        (1, "partial"),
        (2, "partial"),
        (3, "partial"),
        (4, "empty"),
        (5, "unavailable"),
    ] {
        mode.store(fault, Ordering::SeqCst);
        assert!(
            refresh(&broker, &format!("test/group-literal/{fault}"), None)
                .await
                .get("error")
                .is_none()
        );
        let state = values(&broker, |v| {
            v["Available"] == false && v["Quality"] == quality
        })
        .await;
        assert!(
            state["PowerState"].is_null() && state["Brightness"].is_null(),
            "unknown/empty group became Off or maximum"
        );
        let errors = broker.received_on("DALI/Error/Controller").len();
        assert!(broker.send_with(
            &format!("DALI/Desired/Controller/{TARGET}/PowerState"),
            "false",
            &options
        ));
        assert!(
            broker
                .wait_until(Duration::from_secs(2), |b| b
                    .received_on("DALI/Error/Controller")
                    .len()
                    > errors)
                .await,
            "incomplete/empty scalar Set was acknowledged as confirmed"
        );
        assert!(refresh(
            &broker,
            &format!("test/group-literal/settled/{fault}"),
            None
        )
        .await
        .get("error")
        .is_none());
    }
    assert!(running.stop().await);
    peer.abort();
    let _ = peer.await;
}
