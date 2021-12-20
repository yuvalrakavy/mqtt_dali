use rustop::opts;

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;

mod dali_emulator;

use crate::config_payload::Config;
use crate::dali_emulator::DaliControllerEmulator;

#[tokio::main]
async fn main()  {
    let (args, _) = opts! {
        synopsis "MQTT Dali Controller";
        param mqtt:String, desc: "MQTT broker to connect";
        opt emulation:bool = true, desc: "Use hardware emulation (for debugging)";
        opt setup:bool=false, desc: "Setup mode";
        opt config: String = String::from("dali.json"), desc: "Coniguration filename (dali.json)";
        opt update: bool=false, desc: "force update MQTT configuration topic (/DALI/Config/ContollerName)";
    }.parse_or_exit();

    let mut setup = args.setup;
    let mut new_config = false;

    let mut config = if !std::path::Path::new(&args.config).exists() {
        setup = true;
        new_config = true;
        Config::interactive_new().unwrap()
    }
    else {
        Config::load(&args.config).unwrap()
    };

    let controller = if args.emulation {
        if new_config {
            let lights_count = Config::prompt_for_number("Number of lights to emulate", &Some(3)).unwrap();
            DaliControllerEmulator::new(config.buses.len(), lights_count)
        } else {
            DaliControllerEmulator::new_with_config(&config)
        }
    } else { 
        panic!("Only emulation is supported at this stage");
    };

    let mut dali_manager = dali_manager::DaliManager::new(&controller);

    if setup {
        let setup_result = config.iteractive_setup(&mut dali_manager);

        if let Ok(_) | Err(setup::SetupError::UserQuit) = setup_result {
            config.save(&args.config).unwrap();
        }

        if let Err(setup::SetupError::UserQuit) = setup_result {
            std::process::exit(0);
        }
    }

    let mut mqtt = mqtt::MqttDali::new(&mut dali_manager, &config, &args.mqtt);

    mqtt.run().await.unwrap();
}
