//! Opt-in real-browser acceptance through the managed service and physical PTY model.
use super::*;
use crate::{
    atx::transactions::RepeatPolicy,
    config_store::test_hooks::{Hooks, Stage, Step},
    emulator::{control::Endpoint, pty},
};
use std::{os::unix::fs::PermissionsExt, process::Stdio, sync::Arc};

#[tokio::test]
#[ignore = "requires npm dependencies and a local Chrome; run explicitly for browser changes"]
async fn browser_functional_phone_and_desktop() {
    // macOS's per-user temp prefix can exceed the Unix socket path limit.
    let path = PathBuf::from("/tmp").join(format!("dali-browser-{:032x}", rand::random::<u128>()));
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let emulator=pty::spawn(serde_json::from_value(json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"white","short_address":0,"level":91,"groups":8,"device_type":8,"colour":{"active":"temperature","temperature":{"value":300,"coolest":200,"warmest":500,"physical_coolest":150,"physical_warmest":550}}},
        {"id":"plain","short_address":1,"level":120},
        {"id":"new","short_address":null,"level":0,"groups":4}
    ],"inputs":[{"id":"buttons","short_address":0,"instances":[1,1],"values":[{"instance":0,"resolution":1,"bytes":[0]},{"instance":1,"resolution":1,"bytes":[0]}]}]}]})).unwrap()).await.unwrap();
    let socket = path.join("control.sock");
    let endpoint = Endpoint::bind(socket.clone()).await.unwrap();
    let (stop, receive) = tokio::sync::watch::channel(false);
    let controls = tokio::spawn(endpoint.run(emulator.emulator.clone(), receive));
    let broker = FakeBroker::start().await;
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let origin = format!("http://{address}");
    let failure = path.join("fail-next-save");
    let sentinel = failure.clone();
    let hooks = Hooks {
        callback: Some(Arc::new(move |step| {
            if step == Step::BeforeRename(Stage::Reservation) && sentinel.exists() {
                fs::remove_file(&sentinel).map_err(|e| e.to_string())?;
                return Err("browser injected save failure".into());
            }
            Ok(())
        })),
    };
    let (service, handle) =
        DaliService::new(ConfigPaths::in_directory(&path), "BrowserController".into());
    let running = Bridge::new("DALI")
        .instance("BrowserController")
        .broker(&broker.address())
        .start_managed(
            service
                .with_config_test_hooks(hooks)
                .with_uart(&emulator.path, RepeatPolicy::OneTerminal)
                .with_web(Settings::new(address, &origin, "browser-secret").unwrap()),
        )
        .await
        .unwrap();
    timeout(Duration::from_secs(3), async {
        loop {
            if handle.snapshot().is_some_and(|s| {
                s.phase == Phase::Ready
                    && matches!(s.hardware, crate::service::Hardware::Ready { .. })
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let log = path.join("browser.log");
    let output = fs::File::create(&log).unwrap();
    let mut child = tokio::process::Command::new("node")
        .arg("web/tests/browser.mjs")
        .arg(&origin)
        .arg(&socket)
        .arg(&path)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let status = timeout(Duration::from_secs(240), child.wait()).await;
    if status.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    let result = fs::read_to_string(&log).unwrap_or_default();
    assert!(running.stop().await);
    stop.send_replace(true);
    controls.await.unwrap().unwrap();
    emulator.emulator.close().await.unwrap();
    assert!(
        status.is_ok_and(|s| s.is_ok_and(|s| s.success())),
        "browser acceptance failed; artifacts {}\n{result}",
        path.display()
    );
    println!("browser artifacts: {}\n{result}", path.display());
}
