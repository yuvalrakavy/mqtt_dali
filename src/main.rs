use rustop::opts;

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;

mod dali_emulator;

use crate::config_payload::Config;

#[tokio::main]
async fn main()  {
    let (args, _) = opts! {
        synopsis "MQTT Dali Controller";
        param mqtt:String, desc: "MQTT broker to connect";
        opt setup:bool=false, desc: "Setup mode";
        opt config: String = String::from("dali.json"), desc: "Coniguration filename (dali.json)";
        opt update: bool=false, desc: "force update MQTT configuration topic (/DALI/Config/ContollerName)";
    }.parse_or_exit();

    let mut setup = args.setup;

    let mut config = if !std::path::Path::new(&args.config).exists() {
        setup = true;
        Config::interactive_new().unwrap()
    }
    else {
        Config::load(&args.config).unwrap()
    };

    let mut dali_manager = if setup { dali_manager::DaliManager::new() } else { dali_manager::DaliManager::new_with_config(&config) };

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
