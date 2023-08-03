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
        opt config: String = String::from("dali.json"), desc: "Configuration filename (dali.json)";
    }.parse_or_exit();

    env_logger::init();
    
    let config = Config {
        config_filename: args.config.clone(),
    };

    let mut dali_config = if !std::path::Path::new(&args.config).exists() {
        DaliConfig::interactive_new().unwrap()
    }
    else {
        config.load().unwrap()
    };

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

    let mut mqtt = mqtt::MqttDali::new(&mut dali_manager, &mut dali_config, &args.mqtt);

    mqtt.run(&config).await.unwrap();
}
