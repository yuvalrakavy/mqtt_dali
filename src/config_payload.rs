
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub short_address: u8,            // Channel number
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub group_address: u8,              // Group number
    pub description: String,
    pub channels: Vec<u8>,      // Channel list
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BusConfig {
    pub description: String,
    pub bus: usize,                // Bus number
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub buses: Vec<BusConfig>,
}

#[test]
fn test_parse_config() {
    println!("Testing config");
    
    let config_json = String::from(
        r#"{ 
                "name": "Kitchen",
                "buses": [
                    {
                        "description": "lights",
                        "bus": 0,
                        "channels": [
                            {
                                "channel": 1,
                                "description": "main light"
                            }
                        ]
                    }
                ]
            }
        "#);

    let config: Config = serde_json::from_str(&config_json).unwrap();

    println!("Config {:#?}", config);
}
