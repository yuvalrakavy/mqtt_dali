
use serde::{Deserialize, Serialize};

/// Payload  for controller command topic

#[derive(Debug, Deserialize)]
#[serde(tag="command")]
pub enum DaliCommand {
    SetLightBrightness{bus: usize, address: u8, value: u8 },    
    SetGroupBrightness{bus: usize, group: u8, value: u8 },

    UpdateBusStatus,
    RenameBus   { bus: usize, name: String },
    RenameLight { bus: usize, address: u8, name: String },
    RenameGroup { bus: usize, group: u8, name: String },
    NewGroup    { bus: usize },
    AddToGroup  { bus: usize, group: u8, address: u8 },
    MatchGroup  { bus: usize, group: u8, pattern: String },
    RemoveGroup { bus: usize, group: u8 },
    RemoveFromGroup { bus: usize, group: u8, address: u8 },
    FindAllLights   { bus: usize },
    FindNewLights   { bus: usize },
    QueryLightStatus{ bus: usize, address: u8 },
    RemoveShortAddress { bus: usize, address: u8 },
    SetLightFadeTime { bus: usize, address: u8, fade_time: u8 },
    SetGroupFadeTime { bus: usize, group: u8, fade_time: u8 },
}

#[derive(Debug, Copy, Clone)]
pub struct LightStatus(u8);

impl From<u8> for LightStatus {
    fn from(v: u8) -> Self {
        LightStatus(v)
    }
}

impl From<LightStatus> for u8 {
    fn from(light_status: LightStatus) -> Self {
        light_status.0
    }
}

impl std::fmt::Display for LightStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = String::new();

        if (self.0 & 0x01) != 0 {
            d.push_str(" Not-OK");
        }
        if (self.0 & 0x02) != 0 {
            d.push_str(" Lamp-Failure");
        }
        if (self.0 & 0x04) != 0 {
            d.push_str(" Lamp-ON");
        }
        if (self.0 & 0x08) != 0 {
            d.push_str(" Limit-error");
        }
        if (self.0 & 0x10) != 0 {
            d.push_str(" Fade-In-Progress");
        }
        if (self.0 & 0x20) != 0 {
            d.push_str(" Reset-state");
        }
        if (self.0 & 0x40) != 0 {
            d.push_str(" Missing-short-address");
        }
        if (self.0 & 0x80) != 0 {
            d.push_str(" Power-Failure");
        }

        write!(f, "{:#04x}: {}", self.0, d)
    }
}

#[derive(Serialize)]
pub struct QueryLightReply {
    controller: String,
    bus: usize,
    address: u8,
    failure: bool,
    status: u8,
    description: String,
}

impl QueryLightReply {
    pub fn new(controller: &str, bus: usize, address: u8, status: LightStatus) -> QueryLightReply {
        QueryLightReply {
             controller: controller.to_owned(),
             bus,
             address,
             failure: false,
             status: status.into(),
             description: format!("{}", status)
        }
    }

    pub fn new_failure(controller: &str, bus: usize, address: u8, error: &str) -> QueryLightReply {
        QueryLightReply {
            controller: controller.to_owned(),
            bus,
            address,
            failure: true,
            status: 0,
            description: error.to_string()
       }
    }
}

#[cfg(test)]
mod tests {
    use crate::command_payload::DaliCommand;

    #[test]
    fn test_set_light_brightness() {
        let json = r#"
            {
                "command": "SetLightBrightness",
                "bus": 1,
                "channel": 5,
                "value": 48
            }
        "#;

        let c: DaliCommand = serde_json::from_str(&json).unwrap();
        match c {
            DaliCommand::SetLightBrightness { bus: 1, address: 5, value: 48 } => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_set_group_brightness() {
        let json = r#"
            {
                "command": "SetGroupBrightness",
                "bus": 1,
                "group": 5,
                "value": 48
            }
        "#;

        let c: DaliCommand = serde_json::from_str(&json).unwrap();
        match c {
            DaliCommand::SetGroupBrightness { bus: 1, group: 5, value: 48 } => assert!(true),
            _ => assert!(false),
        }
    }
}

