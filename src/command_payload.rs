
use serde::Deserialize;

/// Payload  for controller command topic

#[derive(Debug, Deserialize)]
#[serde(tag="command")]
pub enum DaliCommand {
    SetLightBrightness{bus: usize, address: u8, value: u8},    
    SetGroupBrightness{bus: usize, group: u8, value: u8},

    UpdateBusStatus,
    RenameBus {bus: usize, name: String},
    RenameLight {bus: usize, address: u8, name: String},
    RenameGroup {bus: usize, group: u8, name: String},
    NewGroup { bus: usize, group: u8},
    AddToGroup {bus: usize, group: u8, address: u8},
    RemoveGroup { bus: usize, group: u8},
    RemoveFromGroup {bus: usize, group: u8, address: u8},
    FindAllLights {bus: usize},
    FindNewLights {bus: usize},
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

