//! Standalone local ATX HAT emulator. Never opens a hardware UART or broker.
use mqtt_dali::emulator::{control::Endpoint, pty, scenario::Scenario};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::fd::FromRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use tokio::sync::watch;

struct Arguments {
    scenario: PathBuf,
    socket: Option<PathBuf>,
    hat_listen: Option<SocketAddr>,
    web_listen: Option<SocketAddr>,
}
fn arguments() -> Result<Option<Arguments>, String> {
    let mut scenario = None;
    let mut socket = None;
    let mut hat_listen = None;
    let mut web_listen = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(None),
            Some("--scenario") => {
                scenario = Some(args.next().ok_or("--scenario requires a path")?.into())
            }
            Some("--control-socket") => {
                socket = Some(
                    args.next()
                        .ok_or("--control-socket requires a path")?
                        .into(),
                )
            }
            Some("--hat-listen") => {
                hat_listen = Some(
                    args.next()
                        .ok_or("--hat-listen requires numeric IP:port")?
                        .to_str()
                        .ok_or("invalid endpoint")?
                        .parse()
                        .map_err(|_| "invalid --hat-listen numeric IP:port")?,
                );
            }
            Some("--web-listen") => {
                web_listen = Some(
                    args.next()
                        .ok_or("--web-listen requires numeric IP:port")?
                        .to_str()
                        .ok_or("invalid endpoint")?
                        .parse::<SocketAddr>()
                        .map_err(|_| "invalid --web-listen numeric IP:port")?,
                );
            }
            _ => return Err(format!("unknown option {}", arg.to_string_lossy())),
        }
    }
    Ok(Some(Arguments {
        scenario: scenario.ok_or("--scenario is required")?,
        socket,
        hat_listen,
        web_listen,
    }))
}
fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            output(libc::STDERR_FILENO, error.to_string());
            return ExitCode::FAILURE;
        }
    };
    // WAIT: emulator-supervise
    let result = runtime.block_on(run());
    // WAIT: emulator-runtime-shutdown
    runtime.shutdown_timeout(Duration::from_millis(500));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            output(libc::STDERR_FILENO, format!("dali_hat_emulator: {error}"));
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<(), String> {
    // Install both stop handlers before logging, scenario loading or PTY creation.
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| e.to_string())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| e.to_string())?;
    let startup = async {
        let Some(args) = arguments()? else {
            output(libc::STDOUT_FILENO,"dali_hat_emulator --scenario scenario.json [--hat-listen 127.0.0.1:5580] [--web-listen 127.0.0.1:5581] [--control-socket /private/directory/control.sock]\n\nPrints JSON ready with the actual endpoints. Omitting --hat-listen retains PTY mode. Web is loopback-only, with numeric host matching. No MQTT or real HAT.\nThe optional control directory must belong to this user and have mode 0700.".into());
            return Ok(None);
        };
        let logging = worker(|| {
            tracing_init::TracingInit::builder("dali_hat_emulator")
                .on_destination_error(tracing_init::types::OnDestinationError::Skip)
                .init()
                .ok()
        })
        .await?;
        let scenario = worker(move || load(args.scenario)).await??;
        let (emulator, pty_path, hat_address) = match args.hat_listen {
            Some(address) => {
                let (emulator, bound) =
                    mqtt_dali::emulator::session::Emulator::listen_tcp(address, scenario).await?;
                (emulator, None, Some(bound))
            }
            None => {
                // WAIT: emulator-pty-open
                let pty = pty::spawn(scenario).await?;
                (pty.emulator, Some(pty.path), None)
            }
        };
        let web = match args.web_listen {
            Some(address) => {
                Some(mqtt_dali::emulator::web::Server::bind(address, emulator.clone()).await?)
            }
            None => None,
        };
        let endpoint = match args.socket {
            Some(path) => Some(
                Endpoint::bind(path.clone())
                    .await
                    .map(|endpoint| (path, endpoint))?,
            ),
            None => None,
        };
        output(libc::STDOUT_FILENO,serde_json::json!({"ready":true,"pty":pty_path,"hat_listen":hat_address,"web_listen":web.as_ref().map(|w|w.address),"control_socket":endpoint.as_ref().map(|(path,_)|path),"profile":"emulated-i4-v000004"}).to_string());
        Ok::<_, String>(Some((emulator, endpoint, web, logging)))
    };
    // WAIT: emulator-startup
    let Some((emulator, endpoint, mut web, logging)) = (tokio::select! {
        _ = term.recv()=>return Ok(()),
        _ = interrupt.recv()=>return Ok(()),
        result = startup=>result?,
    }) else {
        return Ok(());
    };
    tracing::info!("emulator ready");
    let (stop, stopping) = watch::channel(false);
    let endpoint_emulator = emulator.clone();
    let mut endpoint = tokio::spawn(async move {
        match endpoint {
            Some((_, endpoint)) => {
                // WAIT: emulator-control-serve
                endpoint.run(endpoint_emulator, stopping).await
            }
            None => {
                // WAIT: emulator-control-serve
                std::future::pending().await
            }
        }
    });
    let _logging = logging;
    let mut status = emulator.observe_status();
    // WAIT: emulator-supervise
    let result = tokio::select! {
        _ = term.recv()=>Ok(()),
        _ = interrupt.recv()=>Ok(()),
        result = &mut endpoint=>result.map_err(|e|e.to_string())?,
        result = status.wait_for(Option::is_some)=>result.map_err(|e|e.to_string())?.clone().unwrap(),
    };
    stop.send_replace(true);
    endpoint.abort();
    if let Some(web) = &mut web {
        // WAIT: emulator-web-close
        web.close().await?;
    }
    // WAIT: emulator-close
    result.and(emulator.close().await)
}
async fn worker<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    let worker = std::thread::Builder::new()
        .name("dali-emulator-startup".into())
        .spawn(move || {
            let _ = send.send(work());
        })
        .map_err(|e| e.to_string())?;
    drop(worker);
    // WAIT: emulator-startup-worker
    tokio::time::timeout(Duration::from_secs(3), receive)
        .await
        .map_err(|_| "emulator startup worker deadline".into())
        .and_then(|r| r.map_err(|_| "emulator startup worker disappeared".into()))
}
fn load(path: PathBuf) -> Result<Scenario, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("scenario must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("scenario exceeds 4 MiB".into());
    }
    let scenario: Scenario = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    scenario.validate()?;
    Ok(scenario)
}
fn output(descriptor: libc::c_int, mut line: String) {
    line.push('\n');
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    if let Ok(worker) = std::thread::Builder::new()
        .name("dali-emulator-output".into())
        .spawn(move || {
            let fd = unsafe { libc::dup(descriptor) };
            if fd >= 0 {
                let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
                let _ = file.write_all(line.as_bytes());
            }
            let _ = send.send(());
        })
    {
        drop(worker);
        // WAIT: emulator-output
        let _ = receive.recv_timeout(Duration::from_millis(200));
    }
}
