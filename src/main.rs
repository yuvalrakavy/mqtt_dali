use rustop::opts;

mod command_payload;
mod config_payload;
mod mqtt;
mod dali_manager;
mod dali_commands;
mod setup;

mod dali_emulator;
mod dali_atx;

use crate::config_payload::Config;
use crate::dali_emulator::DaliControllerEmulator;
use crate::dali_atx::DaliAtx;

#[tokio::main]
async fn main()  {
    let (args, _) = opts! {
        synopsis "MQTT Dali Controller";
        param mqtt:String, desc: "MQTT broker to connect";
        opt emulation:bool = false, desc: "Use hardware emulation (for debugging)";
        opt setup:bool=false, desc: "Setup mode";
        opt config: String = String::from("dali.json"), desc: "Configuration filename (dali.json)";
        opt debug: bool=false, desc: "Generate debug output";
    }.parse_or_exit();

    let mut setup = args.setup;

    let mut config = if !std::path::Path::new(&args.config).exists() {
        setup = true;
        Config::interactive_new().unwrap()
    }
    else {
        Config::load(&args.config).unwrap()
    };

    let mut controller = if args.emulation {
        DaliControllerEmulator::try_new(&mut config, args.debug)
    } else { 
        DaliAtx::try_new(&mut config, args.debug)
    }.expect("Error when initializing DALI controller");

    let mut dali_manager = dali_manager::DaliManager::new(controller.as_mut(), args.debug);

    if setup {
        let setup_result = config.interactive_setup(& mut dali_manager).expect("Setup failed");

        config.save(&args.config).unwrap();

        if let setup::SetupAction::Quit = setup_result {
            std::process::exit(0);
        }
    }

    let mut mqtt = mqtt::MqttDali::new(&mut dali_manager, &config, &args.mqtt);

    mqtt.run().await.unwrap();
}
