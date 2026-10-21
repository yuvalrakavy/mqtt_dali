//! Development v2 configuration entry point. P3 will wire the BridgeKit daemon.
//! This check command never initializes hardware, MQTT or terminal setup.

use std::io::Write;
use std::os::fd::FromRawFd;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use mqtt_dali::config::{load_or_migrate, ConfigPaths};

const CONFIG_DEADLINE: Duration = Duration::from_secs(3);

enum Arguments {
    Help,
    Check {
        paths: ConfigPaths,
        new_name: String,
    },
}

fn arguments() -> Result<Arguments, String> {
    let mut paths = ConfigPaths::in_directory(".");
    let mut new_name = "DaliController".to_owned();
    let mut check = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--help" | "-h") => return Ok(Arguments::Help),
            Some("--check-config") => check = true,
            Some("--config") => {
                paths.current = args
                    .next()
                    .ok_or("--config requires a v2 file path")?
                    .into();
            }
            Some("--legacy-config") => {
                paths.legacy = args
                    .next()
                    .ok_or("--legacy-config requires a v1 import path")?
                    .into();
            }
            Some("--name") => {
                new_name = args
                    .next()
                    .ok_or("--name requires a controller name")?
                    .into_string()
                    .map_err(|_| "controller name must be valid UTF-8")?;
            }
            _ => {
                return Err(format!(
                    "unrecognized option {}",
                    argument.to_string_lossy()
                ))
            }
        }
    }
    if !check {
        return Err("v2 daemon integration is pending; use --check-config to initialize or migrate configuration".into());
    }
    Ok(Arguments::Check { paths, new_name })
}

fn main() -> ExitCode {
    let (paths, new_name) = match arguments() {
        Ok(Arguments::Help) => {
            output(libc::STDOUT_FILENO, "mqtt_dali_v2 --check-config [--config dali.v2.json] [--legacy-config dali.json] [--name DaliController]\n\nSelect v2 first, import v1 only if v2 is absent, or initialize a new inventory.\nNo hardware or MQTT connection is made. The v1 file is never written.".into());
            return ExitCode::SUCCESS;
        }
        Ok(Arguments::Check { paths, new_name }) => (paths, new_name),
        Err(error) => {
            output(libc::STDERR_FILENO, format!("mqtt_dali_v2: {error}"));
            return ExitCode::from(2);
        }
    };
    let current = paths.current.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("dali-v2-config".into())
        .spawn(move || {
            configuration_gate();
            let result = load_or_migrate(&paths, &new_name);
            let _ = send.send(result);
        });
    match worker {
        Ok(worker) => drop(worker), // Do not join a filesystem worker at process exit.
        Err(error) => {
            output(
                libc::STDERR_FILENO,
                format!("mqtt_dali_v2: could not start configuration worker: {error}"),
            );
            return ExitCode::FAILURE;
        }
    }
    // WAIT: v2-config-check
    let result = receive.recv_timeout(CONFIG_DEADLINE);
    match result {
        Ok(Ok(loaded)) => {
            output(
                libc::STDOUT_FILENO,
                format!(
                    "{}",
                    serde_json::json!({
                        "origin": loaded.origin,
                        "config": current,
                        "name": loaded.inventory.name,
                        "schema_version": loaded.inventory.schema_version,
                        "revision": loaded.inventory.revision,
                        "buses": loaded.inventory.buses.len(),
                    })
                ),
            );
            ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            output(libc::STDERR_FILENO, format!("mqtt_dali_v2: {error}"));
            ExitCode::FAILURE
        }
        Err(error) => {
            output(libc::STDERR_FILENO, format!("mqtt_dali_v2: configuration did not complete within {CONFIG_DEADLINE:?}: {error}"));
            ExitCode::FAILURE
        }
    }
}

/// Best-effort command output: an undrained pipe must not hold process exit.
/// Duplicate the descriptor and bypass Rust's global stdio locks, which could
/// otherwise also be held by the abandoned writer during stdio cleanup at exit.
fn output(descriptor: libc::c_int, mut line: String) {
    line.push('\n');
    let (send, receive) = mpsc::sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("dali-v2-output".into())
        .spawn(move || {
            let descriptor = unsafe { libc::dup(descriptor) };
            if descriptor >= 0 {
                // The duplicate belongs only to this File, including on error.
                let mut file = unsafe { std::fs::File::from_raw_fd(descriptor) };
                let _ = file.write_all(line.as_bytes());
            }
            let _ = send.send(());
        });
    if let Ok(worker) = worker {
        drop(worker);
        // WAIT: v2-config-output
        let _ = receive.recv_timeout(Duration::from_millis(200));
    }
}

/// Debug-only test seam: hold a real filesystem read inside the bounded worker.
/// A release binary never examines this variable.
#[cfg(debug_assertions)]
fn configuration_gate() {
    if let Some(path) = std::env::var_os("MQTT_DALI_V2_TEST_CONFIG_GATE") {
        let _ = std::fs::read(path);
    }
}

#[cfg(not(debug_assertions))]
fn configuration_gate() {}
