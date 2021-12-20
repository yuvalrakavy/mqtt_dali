use std::time::Duration;
use rumqttc::{MqttOptions, AsyncClient, EventLoop, QoS, Event, Packet, Publish};
use std::error::Error;
use crate::{dali_manager::DaliManager, command_payload::DaliCommand};
use crate::config_payload::Config;


pub struct MqttDali<'a> {
    config: &'a Config,
    mqtt_client: AsyncClient,
    mqtt_events: EventLoop,
    dali_manager: &'a mut DaliManager<'a>,
}

impl <'a> MqttDali<'a> {
    pub fn new(dali_manager: &'a mut DaliManager<'a>, config: &'a Config, mqtt_broker: &str) -> MqttDali<'a> {
        let mut mqtt_options = MqttOptions::new(&config.name, mqtt_broker, 1883);
        mqtt_options.set_keep_alive(Duration::from_secs(5));

        let (mqtt_client, mqtt_events) = AsyncClient::new(mqtt_options, 10);

        MqttDali {
             config,
             mqtt_client,
             mqtt_events, 
             dali_manager,
         }
    }

    fn get_command_topic(&self) -> String {
        let mut topic = "DALI/Controllers/".to_owned();
    
        topic.push_str(&self.config.name);
        topic.push_str("/Command");
    
        topic
    }

    fn get_config_topic(&self) -> String {
        let mut topic = "DALI/Config/".to_owned();

        topic.push_str(&self.config.name);

        topic
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let config_topic = &self.get_config_topic();

        self.mqtt_client.publish(config_topic, QoS::AtLeastOnce, true, serde_json::to_vec(self.config)?).await?;

        let command_topic = &self.get_command_topic();
        self.mqtt_client.subscribe(command_topic, QoS::AtLeastOnce).await?;

        loop {
            let event = self.mqtt_events.poll().await?;

            if let Event::Incoming(Packet::Publish(Publish { ref topic, payload, ..})) = event {
                if topic == command_topic {
                    match serde_json::from_slice(payload.as_ref()) as serde_json::Result<DaliCommand> {
                        Ok(command) => {
                            println!("Got command {:?}", command);
                            let _ = match command {
                                DaliCommand::SetLightBrightness { bus, channel, value} => self.dali_manager.set_light_brightness_async(bus, channel, value).await,
                                DaliCommand::SetGroupBrightness { bus, group, value } => self.dali_manager.set_group_brightness_async(bus, group, value).await,
                            };
                        },
                        Err(e) => println!("Invalid payload received on {}: {}", command_topic, e),
                    }
                        
                }
                else {
                    println!("Got publish on unexpected topic {}", topic);
                }
            }
        }
    }
}
