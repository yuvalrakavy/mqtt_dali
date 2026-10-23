use super::scenario::{Output, Scenario};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    switching_state: Option<super::switching::State>,
}
impl Gear {
    fn selected(&self, address: u8) -> bool {
        match address {
            0x00..=0x7f => self.config.short_address == Some(address >> 1),
            0x80..=0x9f => self.config.groups & (1 << ((address >> 1) & 15)) != 0,
            0xfc..=0xff => address >= 0xfe || self.config.short_address.is_none(),
            _ => false,
        }
    }
    fn control_level(&self) -> u8 {
        self.switching_state
            .as_ref()
            .map_or(self.config.level, |s| s.level())
    }
    fn drive(&mut self, level: u8) {
        self.config.level = if let Some(switching) = &mut self.switching_state {
            switching.drive(level);
            switching.actual()
        } else {
            level
        };
    }
    fn adjustable_limits(&self) -> bool {
        self.switching_state.as_ref().is_none_or(|s| s.adjustable())
    }
}
#[derive(Serialize)]
struct Bus {
    number: u8,
    powered: bool,
    outputs: Vec<Gear>,
    inputs: Vec<super::input::State>,
    #[serde(skip)]
    repeat: Option<(u16, u64)>,
    #[serde(skip)]
    input_repeat: Option<(u32, u64)>,
    #[serde(skip)]
    enabled_type: Option<u8>,
}
pub struct Model {
    buses: Vec<Bus>,
    rng: u64,
    input_rng: u64,
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
        let mut input_rng = scenario.seed ^ 0x8a5cd789635d2dff;
        let buses = scenario
            .buses
            .into_iter()
            .map(|bus| Bus {
                number: bus.number,
                powered: bus.powered,
                repeat: None,
                input_repeat: None,
                enabled_type: None,
                inputs: bus
                    .inputs
                    .into_iter()
                    .map(|mut config| {
                        config
                            .random_address
                            .get_or_insert_with(|| random(&mut input_rng));
                        super::input::State::new(config)
                    })
                    .collect(),
                outputs: bus
                    .outputs
                    .into_iter()
                    .map(|mut config| {
                        config
                            .random_address
                            .get_or_insert_with(|| random(&mut rng));
                        let switching_state = (config.device_type == 7).then(|| {
                            super::switching::State::new(
                                config.switching.clone().unwrap_or_default(),
                                config.level,
                            )
                        });
                        let min_level = if switching_state.is_some() { 254 } else { 1 };
                        Gear {
                            colour_state: config.colour.clone().map(super::colour::State::new),
                            switching_state,
                            config,
                            dtr: [0; 3],
                            initialize_until: None,
                            withdrawn: false,
                            search: 0xffffff,
                            min_level,
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
            input_rng,
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
            for input in &mut bus.inputs {
                input.advance_to(self.now_ms);
            }
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
            if let Some(input) = bus.inputs.iter_mut().find(|i| i.config.id == id) {
                input.config.short_address = address;
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
    pub fn set_input_value(
        &mut self,
        id: &str,
        instance: u8,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let input = self
            .buses
            .iter_mut()
            .flat_map(|b| b.inputs.iter_mut())
            .find(|i| i.config.id == id)
            .ok_or("input ID does not exist")?;
        input.set_value(instance, bytes)
    }
    pub fn transmit_input(&mut self, number: u8, frame: u32, now_ms: u64) -> Reply {
        self.advance_to(now_ms);
        let Some(bus) = self
            .buses
            .iter_mut()
            .find(|b| b.number == number && b.powered)
        else {
            return Reply::None;
        };
        bus.repeat = None;
        let [high, address, instance, opcode] = frame.to_be_bytes();
        let repeated = bus
            .input_repeat
            .take()
            .is_some_and(|(old, at)| old == frame && self.now_ms.saturating_sub(at) <= 100);
        if high == 0 && address == 0xc1 {
            if (instance > 10 && !matches!(instance, 0x30..=0x32))
                || (matches!(instance, 0 | 2 | 3 | 4 | 10) && opcode != 0)
            {
                self.unsupported += 1;
                return Reply::None;
            }
            if matches!(instance, 1 | 2) && !repeated {
                bus.input_repeat = Some((frame, self.now_ms));
                return Reply::None;
            }
            let replies: Vec<_> = bus
                .inputs
                .iter_mut()
                .filter_map(|input| {
                    let generated = if instance == 2 {
                        random(&mut self.input_rng)
                    } else {
                        0
                    };
                    input.special(instance, opcode, self.now_ms, generated)
                })
                .collect();
            return match replies.first() {
                None => Reply::None,
                Some(value) if replies.iter().all(|r| r == value) => Reply::Value(*value),
                Some(_) => Reply::Collision,
            };
        }
        if high != 0
            || address & 1 == 0
            || (address > 0x7f && !matches!(address, 0xfd | 0xff))
            || (instance > 31 && !matches!(instance, 0x20..=0x3f | 0xfc | 0xfe))
        {
            self.unsupported += 1;
            return Reply::None;
        }
        if ((instance == 0xfe && opcode == 0x14)
            || (instance <= 31 && matches!(opcode, 0x00..=0x03 | 0x61..=0x68))
            || (instance <= 31 && matches!(opcode, 0x10..=0x11 | 0x21..=0x23 | 0x30..=0x33)))
            && !repeated
        {
            bus.input_repeat = Some((frame, self.now_ms));
            return Reply::None;
        }
        let replies: Vec<_> = bus
            .inputs
            .iter_mut()
            .filter(|i| {
                address == 0xff
                    || address == 0xfd && i.config.short_address.is_none()
                    || i.config.short_address == Some(address >> 1)
            })
            .filter_map(|i| i.query(instance, opcode))
            .collect();
        match replies.first() {
            None => Reply::None,
            Some(value) if replies.iter().all(|r| r == value) => Reply::Value(*value),
            Some(_) => Reply::Collision,
        }
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
            bus.input_repeat = None;
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
        bus.input_repeat = None;
        let [address, opcode] = frame.to_be_bytes();
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
        // DT7's per-gear pair sees addressed traffic even when the existing
        // Part102 repeat guard defers this first frame below.
        for gear in &mut bus.outputs {
            if special || gear.selected(address) {
                if let Some(switching) = &mut gear.switching_state {
                    switching.observe(frame);
                }
            }
        }
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
            if !gear.selected(address) {
                continue;
            }
            if address & 1 == 1 && opcode >= 0xe0 {
                if let Some(switching) = &mut gear.switching_state {
                    if let Some(reply) =
                        switching.command(frame, gear.dtr[0], self.now_ms, enabled_type == Some(7))
                    {
                        known = true;
                        if let Some(value) = reply {
                            replies.push(value);
                        }
                        continue;
                    }
                }
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
                    let level = if opcode == 0 {
                        0
                    } else {
                        opcode.clamp(gear.min_level, gear.max_level)
                    };
                    gear.drive(level);
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
                    gear.drive(0);
                    None
                }
                3 | 8 => {
                    if gear.control_level() > 0 || opcode == 8 {
                        let level = gear
                            .control_level()
                            .saturating_add(1)
                            .clamp(gear.min_level, gear.max_level);
                        gear.drive(level);
                    }
                    None
                }
                4 => {
                    if gear.control_level() > 0 {
                        gear.drive(gear.control_level().saturating_sub(1).max(gear.min_level));
                    }
                    None
                }
                5 => {
                    gear.drive(gear.max_level);
                    None
                }
                6 => {
                    gear.drive(gear.min_level);
                    None
                }
                7 => {
                    let level = if gear.control_level() <= gear.min_level {
                        0
                    } else {
                        gear.control_level() - 1
                    };
                    gear.drive(level);
                    None
                }
                0x10..=0x1f => {
                    let level = gear.config.scenes[(opcode & 15) as usize];
                    if let Some(colour) = &mut gear.colour_state {
                        colour.recall_scene((opcode & 15) as usize, level);
                    }
                    if level != 255 {
                        let level = if level == 0 {
                            0
                        } else {
                            level.clamp(gear.min_level, gear.max_level)
                        };
                        gear.drive(level);
                    }
                    None
                }
                0x20 => {
                    gear.config.groups = 0;
                    gear.config.scenes.fill(255);
                    if let Some(colour) = &mut gear.colour_state {
                        colour.reset_scenes();
                    }
                    gear.min_level = if gear.switching_state.is_some() {
                        254
                    } else {
                        1
                    };
                    gear.max_level = 254;
                    gear.config.level = 254;
                    if let Some(switching) = &mut gear.switching_state {
                        switching.reset();
                    }
                    gear.config.random_address = Some(0xffffff);
                    gear.search = 0xffffff;
                    gear.power_on_level = 254;
                    gear.failure_level = 254;
                    gear.fade = 7;
                    None
                }
                0x21 => {
                    gear.dtr[0] = gear.control_level();
                    if let Some(colour) = &mut gear.colour_state {
                        colour.report_actual();
                    }
                    None
                }
                0x2a => {
                    if gear.adjustable_limits() {
                        gear.max_level = gear.dtr[0].clamp(gear.min_level, 254);
                        gear.drive(gear.control_level().min(gear.max_level));
                    }
                    None
                }
                0x2b => {
                    if gear.adjustable_limits() {
                        gear.min_level = gear.dtr[0].clamp(1, gear.max_level);
                        if gear.control_level() > 0 {
                            gear.drive(gear.control_level().max(gear.min_level));
                        }
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
                    if let Some(colour) = &mut gear.colour_state {
                        colour.store_scene((opcode & 15) as usize);
                    }
                    None
                }
                0x50..=0x5f => {
                    gear.config.scenes[(opcode & 15) as usize] = 255;
                    if let Some(colour) = &mut gear.colour_state {
                        colour.remove_scene((opcode & 15) as usize);
                    }
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
                0x9a => Some(if gear.switching_state.is_some() {
                    254
                } else {
                    1
                }),
                0x9c => Some(gear.dtr[1]),
                0x9d => Some(gear.dtr[2]),
                0xa0 => {
                    if let Some(colour) = &mut gear.colour_state {
                        colour.report_actual();
                    }
                    Some(gear.config.level)
                }
                0xa1 => Some(gear.max_level),
                0xa2 => Some(gear.min_level),
                0xa3 => Some(gear.power_on_level),
                0xa4 => Some(gear.failure_level),
                0xa5 => Some(gear.fade),
                0xb0..=0xbf => {
                    if let Some(colour) = &mut gear.colour_state {
                        colour.report_scene((opcode & 15) as usize);
                    }
                    Some(gear.config.scenes[(opcode & 15) as usize])
                }
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
mod addressing_tests;
#[cfg(test)]
mod button_timing_tests;
#[cfg(test)]
mod colour_tests;
#[cfg(test)]
mod feedback_tests;
#[cfg(test)]
mod gear_settings_tests;
#[cfg(test)]
mod input_settings_tests;
#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod sensor_settings_tests;
#[cfg(test)]
mod switching_tests;
#[cfg(test)]
mod tests;
