use log::info;
use rustop::opts;

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;

mod dali_emulator;
mod dali_atx;

use crate::config_payload::DaliConfig;
use crate::dali_emulator::DaliControllerEmulator;
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
    
    if args.log {
        let mut logging_builder = {
            let mut builder = tracing_init::TracingInit::builder("mqtt_dali");

            builder
                .log_to_file(true)
                .log_to_server(true)
                .log_file_prefix("dali")
                .log_file_path("logs")
                .log_to_console(args.console)
                .level(tracing::Level::INFO);

            if !args.filter.is_empty() {
                builder.filter(&args.filter);
            }
            
            builder
        };

        let log_description = logging_builder.init().map(|t| format!("{}", t));

        println!("Logging: {}", log_description.unwrap());
    }

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
        DaliAtx::try_new(&mut dali_config)
    }.expect("Error when initializing DALI controller - is serial port enabled? (enable using raspi-config)");

    let mut dali_manager = dali_manager::DaliManager::new(controller.as_mut());

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

pub fn get_version() -> String {
    format!("mqtt_dali: {} (built at {})", built_info::PKG_VERSION, built_info::BUILT_TIME_UTC)
}

// Include the generated-file as a separate module
pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}