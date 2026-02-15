
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum BusStatus {
    Active,
    NoPower,
    Overloaded,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub short_address: u8,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub group_address: u8,     // Group number
    pub description: String,
    pub members: Vec<u8>,      // Members list (short addresses of lights in this group)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BusConfig {
    pub description: String,
    pub status: BusStatus,
    pub bus: usize,                // Bus number
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaliConfig {
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
                        "status": "Active",
                        "bus": 0,
                        "channels": [
                            {
                                "short_address": 1,
                                "description": "main light"
                            }
                        ]
                    }
                ]
            }
        "#);

    let config: DaliConfig = serde_json::from_str(&config_json).unwrap();

    println!("Config {:#?}", config);
}
