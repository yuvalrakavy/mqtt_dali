//! Opt-in real-browser acceptance through the managed service and PTY/TCP model.
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
    let mut scenario = json!({"version":1,"buses":[{"number":0,"outputs":[
        {"id":"white","short_address":0,"level":91,"groups":8,"device_type":8,"colour":{"active":"temperature","temperature":{"value":300,"coolest":200,"warmest":500,"physical_coolest":150,"physical_warmest":550}}},
        {"id":"plain","short_address":1,"level":120,"scenes":[255,42,255,255,255,255,255,255,255,255,255,255,255,255,255,255]},
        {"id":"new","short_address":null,"level":0,"groups":4,"device_type":8,"colour":{"active":"rgbwaf","xy":[10000,20000],"rgbwaf":[80,90,100,110]}}
    ],"inputs":[{"id":"buttons","short_address":0,"instances":[1,1,2,4,3],"feedback":[{"instance":null},{"instance":0},{"instance":2}],"values":[{"instance":0,"resolution":1,"bytes":[0]},{"instance":1,"resolution":1,"bytes":[0]},{"instance":2,"resolution":12,"bytes":[255,255]},{"instance":3,"resolution":64,"bytes":[254,220,186,152,118,84,50,16]},{"instance":4,"resolution":2,"bytes":[170]}]}]},
    {"number":1,"outputs":[{"id":"relay","short_address":0,"device_type":7,"level":254,"switching":{"adjustable_thresholds":true}},{"id":"fixed-relay","short_address":1,"device_type":7,"level":0}]}
    ]});
    scenario["buses"][0]["outputs"][0]["memory"] = json!({"version":8,"banks":[
        {"bank":0,"bytes":[26,0,255,0,1,2,3,4,5,6,7,254,220,186,152,118,84,50,16,2,3,0,0,0,0,0,0]},
        {"bank":255,"bytes":[255,null,0,127]}]});
    scenario["buses"][0]["outputs"][1]["memory"] = json!({"version":1,"banks":[
        {"bank":0,"bytes":[14,255,0,0,1,2,3,4,5,6,7,137,171,205,239]}]});
    scenario["buses"][0]["inputs"][0]["memory"] = json!({"version":7,"banks":[
        {"bank":0,"bytes":[26,0,0,0,1,2,3,4,5,6,7]}]});
    let luminaire: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/emulator/luminaire-i4.json")).unwrap();
    let mut bank = luminaire["buses"][0]["outputs"][0]["memory"]["banks"][1].clone();
    bank["bytes"][32] = serde_json::Value::Null;
    bank["bytes"][33] = json!(255);
    bank["bytes"][34] = json!(254);
    for item in &mut bank["bytes"].as_array_mut().unwrap()[60..120] {
        *item = json!(0);
    }
    for (i, byte) in b"Fixture <b>OEM</b>".iter().enumerate() {
        bank["bytes"][60 + i] = json!(*byte);
    }
    scenario["buses"][0]["outputs"][0]["memory"]["banks"]
        .as_array_mut()
        .unwrap()
        .push(bank.clone());
    bank["bytes"][18] = json!(4);
    scenario["buses"][0]["outputs"][1]["memory"]["banks"]
        .as_array_mut()
        .unwrap()
        .push(bank);
    let scenario = serde_json::from_value(scenario).unwrap();
    let tcp_mode = std::env::var_os("DALI_BROWSER_HAT_TCP").is_some();
    let (emulator, tcp, pty_owner) = if tcp_mode {
        let (emulator, address) = crate::emulator::session::Emulator::listen_tcp(
            "127.0.0.1:0".parse().unwrap(),
            scenario,
        )
        .await
        .unwrap();
        (emulator, Some(address), None)
    } else {
        let pty = pty::spawn(scenario).await.unwrap();
        (pty.emulator.clone(), None, Some(pty))
    };
    let mut emulator_web = if tcp_mode {
        Some(
            crate::emulator::web::Server::bind("127.0.0.1:0".parse().unwrap(), emulator.clone())
                .await
                .unwrap(),
        )
    } else {
        None
    };

    let socket = path.join("control.sock");
    let endpoint = Endpoint::bind(socket.clone()).await.unwrap();
    let (stop, receive) = tokio::sync::watch::channel(false);
    let controls = tokio::spawn(endpoint.run(emulator.clone(), receive));
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
    let service = service.with_config_test_hooks(hooks);
    let service = if let Some(address) = tcp {
        service.with_hat_tcp(address, RepeatPolicy::OneTerminal)
    } else {
        service.with_uart(&pty_owner.as_ref().unwrap().path, RepeatPolicy::OneTerminal)
    };
    let running = Bridge::new("DALI")
        .instance("BrowserController")
        .broker(&broker.address())
        // This is a sequential operator-workflow oracle. Periodic All polling
        // can legitimately return Busy to a competing command and has separate
        // native integration coverage; do not let its wall-clock phase choose
        // which DOM/native assertions this test reaches.
        .timing(mqtt_bridge_kit::TimingOverrides {
            poll: Some(None),
            ..Default::default()
        })
        .start_managed(service.with_web(Settings::new(address, &origin, "browser-secret").unwrap()))
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
        .arg(
            emulator_web
                .as_ref()
                .map(|web| format!("http://{}", web.address))
                .unwrap_or_default(),
        )
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
    if let Some(web) = &mut emulator_web {
        web.close().await.unwrap();
    }
    emulator.close().await.unwrap();
    drop(pty_owner);
    assert!(
        status.is_ok_and(|s| s.is_ok_and(|s| s.success())),
        "browser acceptance failed; artifacts {}\n{result}",
        path.display()
    );
    println!("browser artifacts: {}\n{result}", path.display());
}
