use rustop::opts;
use std::process::ExitCode;
use tracing::{info, warn};

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;
mod shutdown;

mod dali_emulator;
#[cfg(target_os = "linux")]
mod dali_atx;
// The HAT's protocol is used by `dali_atx` (Linux) and tested everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod hat_line;

use crate::config_payload::DaliConfig;
use crate::dali_emulator::DaliControllerEmulator;
#[cfg(target_os = "linux")]
use crate::dali_atx::DaliAtx;
use crate::setup::Setup;

pub struct Config {
    config_filename: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    let (args, _) = opts! {
        synopsis "MQTT Dali Controller";
        param mqtt:String, desc: "MQTT broker to connect";
        opt emulation:bool = false, desc: "Use hardware emulation (for debugging)";
        opt setup:bool=false, desc: "Setup mode";
        opt log : bool = false, desc: "Also log to a file (logs/dali.<date>.log)";
        opt console: bool = false, desc: "Also log to the console";
        opt filter: String = String::from("warn,mqtt_dali=info"), desc: "Filter for logging";
        opt config: String = String::from("dali.json"), desc: "Configuration filename (dali.json)";
    }.parse_or_exit();

    // Keep the guard for all of main: dropping it shuts down tracing-init's OpenTelemetry providers
    // (guard.rs), so spans and OTLP logs would stop right after startup. Every exit below returns
    // from main, so the guard is dropped, and flushes, on every path (finding C-7).
    let _logging = init_logging(args.log, args.console, &args.filter);

    let config = Config {
        config_filename: args.config.clone(),
    };

    info!("Loading configuration from {config_filename}", config_filename = args.config.clone());

    let loaded = if !std::path::Path::new(&args.config).exists() {
        DaliConfig::interactive_new().map_err(|e| e.to_string())
    }
    else {
        config.load().map_err(|e| e.to_string())
    };
    let mut dali_config = match loaded {
        Ok(dali_config) => dali_config,
        Err(e) => return startup_failed("loading the configuration", &e),
    };

    info!("Configuration: loaded");

    let controller = if args.emulation {
        DaliControllerEmulator::try_new(&mut dali_config)
    } else {
        hardware_controller(&mut dali_config)
    };
    let mut controller = match controller {
        Ok(controller) => controller,
        Err(e) => {
            return startup_failed(
                "initializing the DALI controller (is the serial port enabled? raspi-config)",
                &format!("{e:?}"),
            )
        }
    };

    if args.setup {
        let mut dali_manager = dali_manager::DaliManager::new(&mut *controller);
        match Setup::interactive_setup(&config, dali_config, &mut dali_manager) {
            Ok(setup::SetupAction::Quit) => return ExitCode::SUCCESS,
            Ok(setup::SetupAction::Start(c)) => {
                dali_config = c;
                if let Err(e) = config.save(&dali_config) {
                    return startup_failed("saving the configuration", &e.to_string());
                }
            }
            Err(e) => return startup_failed("setup", &e.to_string()),
        }
    }

    // The session runs on a thread of its own (its DALI calls are synchronous); main waits for a
    // signal, then for the session within shutdown::SHUTDOWN_BOUND.
    let (stop, stop_rx) = tokio::sync::watch::channel(false);
    let bridge = mqtt::Bridge {
        config,
        controller,
        dali_config,
        broker: args.mqtt.clone(),
    };
    let done = match mqtt::spawn(tokio::runtime::Handle::current(), bridge, stop_rx) {
        Ok(done) => done,
        Err(e) => return startup_failed("starting the session thread", &e.to_string()),
    };
    // WAIT: dali-signal
    let signal = shutdown::stop_signal();
    match shutdown::supervise(done, stop, signal, shutdown::SHUTDOWN_BOUND).await {
        shutdown::Ended::Stopped | shutdown::Ended::Overran => ExitCode::SUCCESS,
        shutdown::Ended::Failed | shutdown::Ended::Panicked => ExitCode::FAILURE,
    }
}

/// tracing-init, always (finding C-M6). `logging.toml` (searched upward from the working
/// directory; the systemd units set it) or `LOG_DESTINATION` chooses the destinations; `--console`
/// and `--log` add the console and a file (tracing-init ignores `LOG_DESTINATION` once either is
/// given). A destination that cannot start is skipped, and logging
/// that cannot start at all leaves the bridge running without it: telemetry never stops the
/// bridge, and never panics it (finding C-8).
fn init_logging(file: bool, console: bool, filter: &str) -> Option<tracing_init::TracingGuard> {
    let mut builder = tracing_init::TracingInit::builder("mqtt_dali");
    builder
        .file_prefix("dali")
        .file_path("logs")
        .on_destination_error(tracing_init::types::OnDestinationError::Skip);
    // Only ever switched on: a flag set to false pins its destination off, past logging.toml.
    if console {
        builder.log_to_console(true);
    }
    if file {
        builder.log_to_file(true);
    }
    if !filter.is_empty() {
        builder.filter("*", filter);
    }
    match builder.init() {
        Ok(guard) => {
            println!("Logging: {guard}");
            Some(guard)
        }
        Err(e) => {
            eprintln!("mqtt_dali: logging could not start ({e}); running without it");
            None
        }
    }
}

/// A start-up failure: logged while the logging guard lives, and an exit status systemd restarts on.
fn startup_failed(stage: &str, error: &str) -> ExitCode {
    warn!(kind = "startup_failed", stage, error, "the DALI bridge could not start");
    eprintln!("mqtt_dali: {stage}: {error}");
    ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
fn hardware_controller(dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn dali_manager::DaliController>> {
    DaliAtx::try_new(dali_config)
}

/// The DALI hardware is the Pi's UART: elsewhere, only `--emulation` runs.
#[cfg(not(target_os = "linux"))]
fn hardware_controller(_dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn dali_manager::DaliController>> {
    Err(error_stack::Report::new(dali_manager::DaliManagerError::Context(
        "the DALI hardware needs Linux (the Pi's UART); run with --emulation here".to_owned(),
    )))
}

pub fn get_version() -> String {
    format!("mqtt_dali: {} (built at {})", built_info::PKG_VERSION, built_info::BUILT_TIME_UTC)
}

// Include the generated-file as a separate module
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
