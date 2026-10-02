use rustop::opts;
use std::process::ExitCode;
use std::time::Duration;
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
#[cfg(test)]
mod test_log;

use crate::config_payload::DaliConfig;
use crate::dali_emulator::DaliControllerEmulator;
#[cfg(target_os = "linux")]
use crate::dali_atx::DaliAtx;
use crate::setup::Setup;

pub struct Config {
    config_filename: String,
}

/// The command line.
struct Args {
    mqtt: String,
    emulation: bool,
    setup: bool,
    log: bool,
    console: bool,
    filter: String,
    config: String,
}

/// How long the process waits, once `run` has returned, for the runtime's blocking threads still
/// running: a configuration write, the logging start or a start-up step stuck on a filesystem
/// that stopped answering, a broker name lookup stuck in the resolver. Past it they are left
/// behind, and the process exits.
const RUNTIME_SHUTDOWN_BOUND: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
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
    let args = Args {
        mqtt: args.mqtt,
        emulation: args.emulation,
        setup: args.setup,
        log: args.log,
        console: args.console,
        filter: args.filter,
        config: args.config,
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            // A direct print, and the one on the lifecycle path (fleet class B2): no logging, no
            // stop handler yet, so a stderr nobody drains holds nothing SIGTERM cannot end.
            eprintln!("mqtt_dali: the async runtime could not start: {e}");
            return ExitCode::FAILURE;
        }
    };
    // WAIT: dali-run
    let code = runtime.block_on(run(args));
    // Dropping a runtime waits, without limit, for every blocking task still running, so a write
    // or a name lookup stuck in the kernel would hold the exit however bounded the shutdown before
    // it (fleet class F1). `run` has returned, its logging guard dropped and flushed.
    // WAIT: dali-runtime-shutdown
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_BOUND);
    code
}

async fn run(args: Args) -> ExitCode {
    // First, before logging, the configuration or the hardware: a stop during start-up takes the
    // bridge's own path, not the signal's default action (fleet class F3).
    let (mut signals, unavailable) = shutdown::StopSignals::install();

    // The logging start is synchronous and can wait: tracing-init gives a destination that will
    // not start 5 s, and reads its logging config with no bound of its own (a hung mount). It runs
    // on the blocking pool, raced with a stop (fleet class B1); a start still stuck is left to the
    // runtime's bounded shutdown.
    //
    // Keep the guard for all of `run`: dropping it shuts down tracing-init's OpenTelemetry providers
    // (guard.rs), so spans and OTLP logs would stop right after startup. Every exit below returns
    // from `run`, so the guard is dropped, and flushes, on every path (finding C-7).
    let (log, console, filter) = (args.log, args.console, args.filter.clone());
    let logging_start = tokio::task::spawn_blocking(move || init_logging(log, console, &filter));
    // WAIT: dali-startup
    let logging = tokio::select! {
        started = logging_start => started.ok().flatten(),
        signal = signals.recv() => {
            note(format!("{signal} during the logging start: stopping"));
            return ExitCode::SUCCESS;
        }
    };
    match &logging {
        Some(guard) => info!("Logging: {guard}"),
        None => {
            for missing in &unavailable {
                note(format!(
                    "no {} handler ({}): that signal ends the bridge without its bounded shutdown",
                    missing.signal, missing.error
                ));
            }
        }
    }
    shutdown::report_unavailable(&unavailable);

    info!("Loading configuration from {config_filename}", config_filename = args.config.clone());

    // Start-up waits: on the configuration's file, on the HAT (its version, up to 11 s), on the
    // operator (an interactive configuration or --setup). It runs on the blocking pool, raced with
    // a stop, which ends the process at once; a step still stuck is left to the runtime's bounded
    // shutdown.
    let (config_filename, emulation, setup) = (args.config.clone(), args.emulation, args.setup);
    let starting = tokio::task::spawn_blocking(move || start_up(config_filename, emulation, setup));
    // WAIT: dali-startup
    let started = tokio::select! {
        started = starting => started,
        signal = signals.recv() => {
            info!(signal, "stop signal during start-up: stopping");
            return ExitCode::SUCCESS;
        }
    };
    let (config, controller, dali_config) = match started {
        Ok(Ok(started)) => started,
        Ok(Err(code)) => return code,
        Err(e) => return startup_failed("start-up", &format!("it ended abnormally: {e}")),
    };

    // The session runs on a thread of its own (its DALI calls are synchronous); `run` waits for a
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
    let signal = async {
        // WAIT: dali-signal
        let signal = signals.recv().await;
        info!(signal, "stop signal: stopping");
    };
    match shutdown::supervise(done, stop, signal, shutdown::SHUTDOWN_BOUND).await {
        shutdown::Ended::Stopped | shutdown::Ended::Overran => ExitCode::SUCCESS,
        shutdown::Ended::Failed | shutdown::Ended::Panicked => ExitCode::FAILURE,
    }
}

/// Start-up's synchronous steps, on the blocking pool: the configuration (an interactive one when
/// its file is missing), the DALI controller, and the interactive setup with --setup. An `Err` is
/// the exit code to end with, its reason already logged.
fn start_up(
    config_filename: String,
    emulation: bool,
    setup: bool,
) -> Result<(Config, Box<dyn dali_manager::DaliController>, DaliConfig), ExitCode> {
    let config = Config { config_filename };
    let loaded = if !std::path::Path::new(&config.config_filename).exists() {
        DaliConfig::interactive_new().map_err(|e| e.to_string())
    }
    else {
        config.load().map_err(|e| e.to_string())
    };
    let mut dali_config = match loaded {
        Ok(dali_config) => dali_config,
        Err(e) => return Err(startup_failed("loading the configuration", &e)),
    };

    info!("Configuration: loaded");

    let controller = if emulation {
        DaliControllerEmulator::try_new(&mut dali_config)
    } else {
        hardware_controller(&mut dali_config)
    };
    let mut controller = match controller {
        Ok(controller) => controller,
        Err(e) => {
            return Err(startup_failed(
                "initializing the DALI controller (is the serial port enabled? raspi-config)",
                &format!("{e:?}"),
            ))
        }
    };

    if setup {
        let mut dali_manager = dali_manager::DaliManager::new(&mut *controller);
        match Setup::interactive_setup(&config, dali_config, &mut dali_manager) {
            Ok(setup::SetupAction::Quit) => return Err(ExitCode::SUCCESS),
            Ok(setup::SetupAction::Start(c)) => {
                dali_config = c;
                if let Err(e) = config.save(&dali_config) {
                    return Err(startup_failed("saving the configuration", &e.to_string()));
                }
            }
            Err(e) => return Err(startup_failed("setup", &e.to_string())),
        }
    }
    Ok((config, controller, dali_config))
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
        Ok(guard) => Some(guard),
        Err(e) => {
            note(format!("logging could not start ({e}); running without it"));
            None
        }
    }
}

/// A start-up failure: logged while the logging guard lives, and an exit status systemd restarts on.
fn startup_failed(stage: &str, error: &str) -> ExitCode {
    warn!(kind = "startup_failed", stage, error, "the DALI bridge could not start");
    note(format!("{stage}: {error}"));
    ExitCode::FAILURE
}

/// A line for stderr, where logging may not be there to carry it, written by a thread of its own:
/// a stderr nobody drains holds that thread, never the bridge's start or stop (fleet class B2).
/// Best effort: the process may end before it is written.
fn note(line: String) {
    let _ = std::thread::Builder::new()
        .name("stderr-note".into())
        .spawn(move || eprintln!("mqtt_dali: {line}"));
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
