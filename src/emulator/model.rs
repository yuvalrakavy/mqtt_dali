use super::scenario::{Input, Output, Scenario};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    None,
    Value(u8),
    Collision,
}
#[derive(Serialize)]
struct Gear {
    #[serde(flatten)]
    config: Output,
    dtr: [u8; 3],
    initialize_until: Option<u64>,
    withdrawn: bool,
    search: u32,
    min_level: u8,
    max_level: u8,
    power_on_level: u8,
    failure_level: u8,
    fade: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    colour_state: Option<super::colour::State>,
}
#[derive(Serialize)]
struct Bus {
    number: u8,
    powered: bool,
    outputs: Vec<Gear>,
    inputs: Vec<Input>,
    #[serde(skip)]
    repeat: Option<(u16, u64)>,
    #[serde(skip)]
    enabled_type: Option<u8>,
}
pub struct Model {
    buses: Vec<Bus>,
    rng: u64,
    now_ms: u64,
    unsupported: u64,
}
fn random(rng: &mut u64) -> u32 {
    *rng = rng.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *rng;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((value ^ (value >> 31)) & 0xffffff) as u32
}
impl Model {
    pub fn new(scenario: Scenario) -> Result<Self, String> {
        scenario.validate()?;
        let mut rng = scenario.seed;
        let buses = scenario
            .buses
            .into_iter()
            .map(|bus| Bus {
                number: bus.number,
                powered: bus.powered,
                repeat: None,
                enabled_type: None,
                inputs: bus.inputs,
                outputs: bus
                    .outputs
                    .into_iter()
                    .map(|mut config| {
                        config
                            .random_address
                            .get_or_insert_with(|| random(&mut rng));
                        Gear {
                            colour_state: config.colour.clone().map(super::colour::State::new),
                            config,
                            dtr: [0; 3],
                            initialize_until: None,
                            withdrawn: false,
                            search: 0xffffff,
                            min_level: 1,
                            max_level: 254,
                            power_on_level: 254,
                            failure_level: 254,
                            fade: 7,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(Self {
            buses,
            rng,
            now_ms: 0,
            unsupported: 0,
        })
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"time_ms":self.now_ms,"unsupported_commands":self.unsupported,"buses":self.buses})
    }
    pub fn advance_to(&mut self, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
        for bus in &mut self.buses {
            for gear in &mut bus.outputs {
                if gear
                    .initialize_until
                    .is_some_and(|until| self.now_ms >= until)
                {
                    gear.initialize_until = None;
                }
            }
        }
    }
    pub fn set_address(&mut self, id: &str, address: Option<u8>) -> Result<(), String> {
        if address.is_some_and(|a| a > 63) {
            return Err("short address exceeds 63".into());
        }
        for bus in &mut self.buses {
            if let Some(gear) = bus.outputs.iter_mut().find(|g| g.config.id == id) {
                gear.config.short_address = address;
                return Ok(());
            }
            if let Some(input) = bus.inputs.iter_mut().find(|i| i.id == id) {
                input.short_address = address;
                return Ok(());
            }
        }
        Err("fixture ID does not exist".into())
    }
    pub fn set_power(&mut self, number: u8, powered: bool) -> Result<(), String> {
        let bus = self
            .buses
            .iter_mut()
            .find(|b| b.number == number)
            .ok_or("bus does not exist")?;
        bus.powered = powered;
        Ok(())
    }
    pub fn powered(&self, number: u8) -> bool {
        self.buses.iter().any(|b| b.number == number && b.powered)
    }
    pub fn bus_count(&self) -> u8 {
        self.buses.len() as u8
    }
    pub fn intervening_frame(&mut self, number: u8) {
        if let Some(bus) = self.buses.iter_mut().find(|b| b.number == number) {
            bus.repeat = None;
        }
    }
    pub fn transmit(&mut self, number: u8, frame: u16, now_ms: u64) -> Reply {
        self.advance_to(now_ms);
        let Some(bus) = self
            .buses
            .iter_mut()
            .find(|bus| bus.number == number && bus.powered)
        else {
            return Reply::None;
        };
        let [address, opcode] = frame.to_be_bytes();
        // One shared broadcast latch suffices: all gear on a modeled bus hears
        // the same C1 special. Consume standard commands before repeat/selection.
        let enabled_type = if address <= 0x9f || address >= 0xfc {
            bus.enabled_type.take()
        } else {
            None
        };
        let repeat_required = matches!(address, 0xa5 | 0xa7)
            || ((address <= 0x9f || address >= 0xfc)
                && address & 1 == 1
                && matches!(opcode, 0x20..=0x30 | 0x40..=0x80));
        let repeated = bus
            .repeat
            .take()
            .is_some_and(|(old, at)| old == frame && self.now_ms.saturating_sub(at) <= 100);
        if repeat_required && !repeated {
            bus.repeat = Some((frame, self.now_ms));
            return Reply::None;
        }
        if address == 0xc1 {
            bus.enabled_type = Some(opcode);
        }
        let special = matches!(
            address,
            0xa1 | 0xa3
                | 0xa5
                | 0xa7
                | 0xa9
                | 0xab
                | 0xb1
                | 0xb3
                | 0xb5
                | 0xb7
                | 0xb9
                | 0xbb
                | 0xc1
                | 0xc3
                | 0xc5
        );
        let mut replies = Vec::new();
        let mut known = false;
        for gear in &mut bus.outputs {
            if special {
                let eligible = gear.initialize_until.is_some() && !gear.withdrawn;
                let exact = eligible && gear.config.random_address == Some(gear.search);
                let value = match address {
                    0xa1 => {
                        gear.initialize_until = None;
                        None
                    }
                    0xa3 => {
                        gear.dtr[0] = opcode;
                        None
                    }
                    0xc3 => {
                        gear.dtr[1] = opcode;
                        None
                    }
                    0xc5 => {
                        gear.dtr[2] = opcode;
                        None
                    }
                    0xb1 => {
                        gear.search = (gear.search & 0x00ffff) | ((opcode as u32) << 16);
                        None
                    }
                    0xb3 => {
                        gear.search = (gear.search & 0xff00ff) | ((opcode as u32) << 8);
                        None
                    }
                    0xb5 => {
                        gear.search = (gear.search & 0xffff00) | opcode as u32;
                        None
                    }
                    0xa5 => {
                        if opcode == 0
                            || (opcode == 255 && gear.config.short_address.is_none())
                            || (opcode & 1 == 1
                                && opcode != 255
                                && gear.config.short_address == Some(opcode >> 1))
                        {
                            gear.initialize_until = Some(self.now_ms.saturating_add(900_000));
                            gear.withdrawn = false;
                        }
                        None
                    }
                    0xa7 => {
                        if gear.initialize_until.is_some() {
                            gear.config.random_address = Some(random(&mut self.rng));
                            gear.withdrawn = false;
                        }
                        None
                    }
                    0xa9 => {
                        if eligible && gear.config.random_address.unwrap() <= gear.search {
                            Some(255)
                        } else {
                            None
                        }
                    }
                    0xab => {
                        if exact {
                            gear.withdrawn = true;
                        }
                        None
                    }
                    0xb7 => {
                        if exact && (opcode == 255 || opcode & 0x81 == 1) {
                            gear.config.short_address = if opcode == 255 {
                                None
                            } else {
                                Some(opcode >> 1)
                            };
                        }
                        None
                    }
                    0xb9 => {
                        if eligible
                            && gear.config.short_address == Some(opcode >> 1)
                            && opcode & 0x81 == 1
                        {
                            Some(255)
                        } else {
                            None
                        }
                    }
                    0xbb => {
                        if exact {
                            Some(gear.config.short_address.map_or(255, |a| (a << 1) | 1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                known = true;
                if let Some(value) = value {
                    replies.push(value);
                }
                continue;
            }
            let selected = match address {
                0x00..=0x7f => gear.config.short_address == Some(address >> 1),
                0x80..=0x9f => gear.config.groups & (1 << ((address >> 1) & 15)) != 0,
                0xfc..=0xff => address >= 0xfe || gear.config.short_address.is_none(),
                _ => false,
            };
            if !selected {
                continue;
            }
            if address & 1 == 1 && opcode >= 0xe0 {
                if enabled_type == Some(8) {
                    if let Some(colour) = &mut gear.colour_state {
                        if let Some(reply) = colour.command(opcode, &mut gear.dtr) {
                            known = true;
                            if let Some(value) = reply {
                                replies.push(value);
                            }
                            continue;
                        }
                    }
                }
                self.unsupported += 1;
                continue;
            }
            if address & 1 == 0 {
                if let Some(colour) = &mut gear.colour_state {
                    colour.power_command();
                }
                if opcode != 255 {
                    gear.config.level = if opcode == 0 {
                        0
                    } else {
                        opcode.clamp(gear.min_level, gear.max_level)
                    };
                }
                known = true;
                continue;
            }
            if matches!(opcode, 0 | 3..=8) {
                if let Some(colour) = &mut gear.colour_state {
                    colour.power_command();
                }
            }
            let value = match opcode {
                0 => {
                    gear.config.level = 0;
                    None
                }
                3 | 8 => {
                    if gear.config.level > 0 || opcode == 8 {
                        gear.config.level = gear
                            .config
                            .level
                            .saturating_add(1)
                            .clamp(gear.min_level, gear.max_level);
                    }
                    None
                }
                4 => {
                    if gear.config.level > 0 {
                        gear.config.level = gear.config.level.saturating_sub(1).max(gear.min_level);
                    }
                    None
                }
                5 => {
                    gear.config.level = gear.max_level;
                    None
                }
                6 => {
                    gear.config.level = gear.min_level;
                    None
                }
                7 => {
                    gear.config.level = if gear.config.level <= gear.min_level {
                        0
                    } else {
                        gear.config.level - 1
                    };
                    None
                }
                0x10..=0x1f => {
                    let level = gear.config.scenes[(opcode & 15) as usize];
                    if level != 255 {
                        gear.config.level = if level == 0 {
                            0
                        } else {
                            level.clamp(gear.min_level, gear.max_level)
                        };
                    }
                    None
                }
                0x20 => {
                    gear.config.groups = 0;
                    gear.config.scenes.fill(255);
                    gear.min_level = 1;
                    gear.max_level = 254;
                    gear.config.level = 254;
                    gear.config.random_address = Some(0xffffff);
                    gear.search = 0xffffff;
                    gear.power_on_level = 254;
                    gear.failure_level = 254;
                    gear.fade = 7;
                    None
                }
                0x21 => {
                    gear.dtr[0] = gear.config.level;
                    None
                }
                0x2a => {
                    gear.max_level = gear.dtr[0].clamp(gear.min_level, 254);
                    gear.config.level = gear.config.level.min(gear.max_level);
                    None
                }
                0x2b => {
                    gear.min_level = gear.dtr[0].clamp(1, gear.max_level);
                    if gear.config.level > 0 {
                        gear.config.level = gear.config.level.max(gear.min_level);
                    }
                    None
                }
                0x2c => {
                    gear.failure_level = gear.dtr[0];
                    None
                }
                0x2d => {
                    gear.power_on_level = gear.dtr[0];
                    None
                }
                0x2e => {
                    if gear.dtr[0] <= 15 {
                        gear.fade = (gear.fade & 15) | (gear.dtr[0] << 4);
                    }
                    None
                }
                0x2f => {
                    if (1..=15).contains(&gear.dtr[0]) {
                        gear.fade = (gear.fade & 0xf0) | gear.dtr[0];
                    }
                    None
                }
                0x40..=0x4f => {
                    gear.config.scenes[(opcode & 15) as usize] = gear.dtr[0];
                    None
                }
                0x50..=0x5f => {
                    gear.config.scenes[(opcode & 15) as usize] = 255;
                    None
                }
                0x60..=0x6f => {
                    gear.config.groups |= 1 << (opcode & 15);
                    None
                }
                0x70..=0x7f => {
                    gear.config.groups &= !(1 << (opcode & 15));
                    None
                }
                0x80 => {
                    if gear.dtr[0] == 255 || gear.dtr[0] & 0x81 == 1 {
                        gear.config.short_address = if gear.dtr[0] == 255 {
                            None
                        } else {
                            Some(gear.dtr[0] >> 1)
                        };
                    }
                    None
                }
                0x90 => Some(if gear.config.level > 0 { 4 } else { 0 }),
                0x91 => Some(255),
                0x97 => Some(8),
                0x98 => Some(gear.dtr[0]),
                0x99 => Some(gear.config.device_type),
                0x9c => Some(gear.dtr[1]),
                0x9d => Some(gear.dtr[2]),
                0xa0 => Some(gear.config.level),
                0xa1 => Some(gear.max_level),
                0xa2 => Some(gear.min_level),
                0xa3 => Some(gear.power_on_level),
                0xa4 => Some(gear.failure_level),
                0xa5 => Some(gear.fade),
                0xb0..=0xbf => Some(gear.config.scenes[(opcode & 15) as usize]),
                0xc0 => Some(gear.config.groups as u8),
                0xc1 => Some((gear.config.groups >> 8) as u8),
                0xc2 => Some((gear.config.random_address.unwrap() >> 16) as u8),
                0xc3 => Some((gear.config.random_address.unwrap() >> 8) as u8),
                0xc4 => Some(gear.config.random_address.unwrap() as u8),
                _ => {
                    self.unsupported += 1;
                    continue;
                }
            };
            known = true;
            if let Some(value) = value {
                replies.push(value);
            }
        }
        if !known && special {
            self.unsupported += 1;
        }
        match replies.first() {
            None => Reply::None,
            Some(value) if replies.iter().all(|r| r == value) => Reply::Value(*value),
            Some(_) => Reply::Collision,
        }
    }
}
#[cfg(test)]
mod colour_tests;
#[cfg(test)]
mod tests;
