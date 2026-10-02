use log::info;
use rustop::opts;

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;

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
async fn main()  {
    let (args, _) = opts! {
        synopsis "MQTT Dali Controller";
        param mqtt:String, desc: "MQTT broker to connect";
        opt emulation:bool = false, desc: "Use hardware emulation (for debugging)";
        opt setup:bool=false, desc: "Setup mode";
        opt log : bool = false, desc: "Enable logging";
        opt console: bool = false, desc: "Enable console logging";
        opt filter: String = String::from("mqtt_dali"), desc: "Filter for logging";
        opt config: String = String::from("dali.json"), desc: "Configuration filename (dali.json)";
    }.parse_or_exit();
    
    // Keep the guard for all of main: dropping it shuts down tracing-init's OpenTelemetry providers
    // (guard.rs), so spans and OTLP logs would stop right after startup.
    let _logging = if args.log {
        let mut logging_builder = {
            let mut builder = tracing_init::TracingInit::builder("mqtt_dali");

            builder
                .log_to_file(true)
                .log_to_gelf_server(true)
                .file_prefix("dali")
                .file_path("logs")
                .log_to_console(args.console)
                .level("*", tracing::Level::INFO);

            if !args.filter.is_empty() {
                builder.filter("*", &args.filter);
            }

            builder
        };

        let guard = logging_builder.init().unwrap();
        println!("Logging: {guard}");
        Some(guard)
    } else {
        None
    };

    let config = Config {
        config_filename: args.config.clone(),
    };

    info!("Loading configuration from {config_filename}", config_filename = args.config.clone());

    let mut dali_config = if !std::path::Path::new(&args.config).exists() {
        DaliConfig::interactive_new().unwrap()
    }
    else {
        config.load().unwrap()
    };

    info!("Configuration: loaded");

    let mut controller = if args.emulation {
        DaliControllerEmulator::try_new(&mut dali_config)
    } else {
        hardware_controller(&mut dali_config)
    }.expect("Error when initializing DALI controller - is serial port enabled? (enable using raspi-config)");

    let mut dali_manager = dali_manager::DaliManager::new(&mut *controller);

    if args.setup {
        let setup_result = Setup::interactive_setup(&config, dali_config, &mut dali_manager).expect("Setup failed");

        match setup_result {
            setup::SetupAction::Quit => std::process::exit(0),
            setup::SetupAction::Start(c) =>{
                dali_config = c;
                config.save(&dali_config).unwrap();
            }
        }
    }

    mqtt::MqttDali::run(&config, &mut dali_manager, &mut dali_config, &args.mqtt).await.unwrap();
}

#[cfg(target_os = "linux")]
fn hardware_controller(dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn dali_manager::DaliController>> {
    DaliAtx::try_new(dali_config)
}

/// The DALI hardware is the Pi's UART: elsewhere, only `--emulation` runs.
#[cfg(not(target_os = "linux"))]
fn hardware_controller(_dali_config: &mut DaliConfig) -> dali_manager::Result<Box<dyn dali_manager::DaliController>> {
    eprintln!("the DALI hardware needs Linux (the Pi's UART); run with --emulation here");
    std::process::exit(2);
}

pub fn get_version() -> String {
    format!("mqtt_dali: {} (built at {})", built_info::PKG_VERSION, built_info::BUILT_TIME_UTC)
}

// Include the generated-file as a separate module
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}