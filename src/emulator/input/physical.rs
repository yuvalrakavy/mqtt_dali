//! Semantic physical stimuli. This model owns gestures; the browser sends no
//! native event codes. Realtime and deterministic stepping use the same clock.
use super::{Settings, State};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Stimulus {
    Button { pressed: bool },
    Value { raw_10: u16 },
    Occupancy { occupied: bool, movement: bool },
}
#[derive(Default, Serialize)]
pub(super) struct Physical {
    pressed: bool,
    raw_10: u16,
    occupied: bool,
    movement: bool,
    #[serde(skip)]
    down: Option<u64>,
    #[serde(skip)]
    long: bool,
    #[serde(skip)]
    stuck: bool,
    #[serde(skip)]
    next_repeat: Option<u64>,
    #[serde(skip)]
    pending_short: Option<u64>,
    #[serde(skip)]
    hold_until: Option<u64>,
    #[serde(skip)]
    report_at: Option<u64>,
    #[serde(skip)]
    last_event: Option<u64>,
    #[serde(skip)]
    reported: u16,
    #[serde(skip)]
    pending: Option<(u16, u32)>,
}
impl Physical {
    pub fn capture(&self) -> crate::emulator::scenario::PhysicalStart {
        crate::emulator::scenario::PhysicalStart {
            pressed: self.pressed,
            raw_10: self.raw_10,
            occupied: self.occupied,
            movement: self.movement,
        }
    }
    pub fn from_config(config: &super::Input, index: usize) -> Self {
        let mut state = Self::default();
        if let Some(initial) = config.initial_physical.as_ref().and_then(|v| v.get(index)) {
            state.pressed = initial.pressed;
            state.raw_10 = initial.raw_10;
            state.occupied = initial.occupied;
            state.movement = initial.movement;
            state.reported = initial.raw_10;
            state.down = initial.pressed.then_some(0);
            return state;
        }
        if let Some(value) = config
            .values
            .iter()
            .find(|v| usize::from(v.instance) == index)
        {
            let bits = u32::from(value.resolution);
            if bits <= 32 {
                let encoded = value
                    .bytes
                    .iter()
                    .fold(0_u64, |v, b| (v << 8) | u64::from(*b));
                let raw = encoded >> (value.bytes.len() * 8 - bits as usize);
                state.raw_10 = (raw * 1023 / ((1_u64 << bits) - 1)) as u16;
                state.reported = state.raw_10;
                state.pressed = config.instances[index] == 1 && raw != 0;
                state.occupied = config.instances[index] == 3 && raw != 0;
            }
        }
        state
    }
}
fn frame(state: &State, index: usize, info: u16, mask: u32) -> Option<u32> {
    let s = &state.settings[index];
    if !s.enabled || s.filter & mask == 0 {
        return None;
    }
    let kind = u32::from(state.config.instances[index]);
    let instance = index as u32;
    let source = match s.scheme {
        0 => 0x800000 | (kind << 17) | 0x8000 | (instance << 10),
        1 => (u32::from(state.config.short_address?) << 17) | (kind << 10),
        2 => (u32::from(state.config.short_address?) << 17) | 0x8000 | (instance << 10),
        3 if s.groups[0] <= 31 => 0x800000 | (u32::from(s.groups[0]) << 17) | (kind << 10),
        4 if s.groups[0] <= 31 => 0xc00000 | (u32::from(s.groups[0]) << 17) | (kind << 10),
        _ => return None,
    };
    Some(source | u32::from(info))
}
fn emit(state: &State, index: usize, info: u16, mask: u32, events: &mut Vec<u32>) {
    if let Some(frame) = frame(state, index, info, mask) {
        events.push(frame);
    }
}
fn sensor_times(s: &Settings) -> (u64, u64, u64, u16, u16) {
    use super::sensor::Profile::*;
    match &s.sensor_settings {
        Some(Absolute { report, deadtime }) => (
            u64::from(*report) * 1000,
            u64::from(*deadtime) * 50,
            0,
            0,
            0,
        ),
        Some(Occupancy {
            report,
            deadtime,
            hold,
        }) => (
            u64::from(*report) * 1000,
            u64::from(*deadtime) * 50,
            if *hold == 0 {
                1000
            } else {
                u64::from(*hold) * 10000
            },
            0,
            0,
        ),
        Some(Light {
            report,
            deadtime,
            hysteresis,
            hysteresis_min,
        }) => (
            u64::from(*report) * 1000,
            u64::from(*deadtime) * 50,
            0,
            u16::from(*hysteresis),
            u16::from(*hysteresis_min),
        ),
        _ => (0, 0, 0, 0, 0),
    }
}
impl State {
    pub fn stimulate(
        &mut self,
        instance: u8,
        stimulus: Stimulus,
        now: u64,
    ) -> Result<Vec<u32>, String> {
        let i = usize::from(instance);
        let kind = *self
            .config
            .instances
            .get(i)
            .ok_or("instance does not exist")?;
        match (&stimulus, kind) {
            (Stimulus::Button { .. }, 1)
            | (Stimulus::Value { raw_10: 0..=1023 }, 2 | 4)
            | (Stimulus::Occupancy { .. }, 3) => {}
            _ => return Err("stimulus does not match supported instance type/range".into()),
        }
        if self
            .config
            .values
            .iter()
            .any(|v| v.instance == instance && v.resolution > 32)
        {
            return Err("semantic stimuli support queried resolutions through 32 bits; use explicit set_input_value bytes for wider profiles".into());
        }
        let mut events = self.advance_physical(now);
        match stimulus {
            Stimulus::Button { pressed } => {
                if self.physical[i].pressed == pressed {
                    return Ok(events);
                }
                self.physical[i].pressed = pressed;
                let timers = self.settings[i].button_timers.as_ref().unwrap();
                let double = u64::from(timers.double) * 20;
                if pressed {
                    let p = &mut self.physical[i];
                    p.down = Some(now);
                    p.long = false;
                    p.stuck = false;
                    p.next_repeat = None;
                    emit(self, i, 1, 2, &mut events);
                } else {
                    emit(self, i, 0, 1, &mut events);
                    if self.physical[i].stuck {
                        emit(self, i, 14, 128, &mut events);
                    } else if self.physical[i].long {
                        emit(self, i, 12, 64, &mut events);
                    } else if double == 0 {
                        emit(self, i, 2, 4, &mut events);
                    } else if self.physical[i]
                        .pending_short
                        .take()
                        .is_some_and(|at| now <= at)
                    {
                        emit(self, i, 5, 8, &mut events);
                    } else {
                        self.physical[i].pending_short = Some(now.saturating_add(double));
                    }
                    let p = &mut self.physical[i];
                    p.down = None;
                    p.next_repeat = None;
                }
                self.update_query_value(instance, if pressed { 1023 } else { 0 });
            }
            Stimulus::Value { raw_10 } => {
                let (_, _, _, percent, minimum) = sensor_times(&self.settings[i]);
                let p = &mut self.physical[i];
                p.raw_10 = raw_10;
                let threshold = minimum
                    .max(((u32::from(p.reported) * u32::from(percent)) / 100) as u16)
                    .max(1);
                if raw_10.abs_diff(p.reported) >= threshold {
                    p.pending = Some((raw_10, 1));
                } else {
                    p.pending = None;
                }
                self.update_query_value(instance, raw_10);
            }
            Stimulus::Occupancy { occupied, movement } => {
                let (_, _, hold, _, _) = sensor_times(&self.settings[i]);
                let p = &mut self.physical[i];
                if occupied {
                    p.occupied = true;
                    p.hold_until = None;
                } else if p.occupied {
                    p.hold_until = Some(now.saturating_add(hold));
                }
                p.movement = movement;
                p.pending = Some((
                    u16::from(p.occupied) * 2 + u16::from(movement),
                    if p.occupied { 1 } else { 2 } | if movement { 8 } else { 16 },
                ));
                self.update_query_value(instance, if self.physical[i].occupied { 1023 } else { 0 });
            }
        }
        events.extend(self.advance_physical(now));
        Ok(events)
    }
    fn update_query_value(&mut self, instance: u8, raw: u16) {
        let Some(value) = self
            .config
            .values
            .iter_mut()
            .find(|v| v.instance == instance)
        else {
            return;
        };
        // Normalize semantic raw10 onto the configured queried resolution. For
        // resolutions above32 use the existing explicit byte control instead.
        if value.resolution > 32 {
            return;
        }
        let bits = u32::from(value.resolution);
        let scaled = u64::from(raw) * ((1_u64 << bits) - 1) / 1023;
        for (byte_index, byte) in value.bytes.iter_mut().enumerate() {
            *byte = 0;
            for bit in 0..8 {
                let index = (byte_index * 8 + bit) % bits as usize;
                *byte |= (((scaled >> (bits as usize - 1 - index)) & 1) as u8) << (7 - bit);
            }
        }
    }
    pub fn advance_physical(&mut self, now: u64) -> Vec<u32> {
        let mut events = Vec::new();
        for i in 0..self.physical.len() {
            let mut emissions = Vec::new();
            let mut clear_occupancy = false;
            if let Some(t) = &self.settings[i].button_timers {
                let p = &mut self.physical[i];
                if let Some(down) = p.down {
                    if !p.long && now >= down.saturating_add(u64::from(t.short) * 20) {
                        p.long = true;
                        p.next_repeat = Some(now.saturating_add(u64::from(t.repeat) * 20));
                        emissions.push((9, 16));
                    }
                    if !p.stuck && now >= down.saturating_add(u64::from(t.stuck) * 1000) {
                        p.stuck = true;
                        p.next_repeat = None;
                        emissions.push((15, 128));
                    }
                    if p.next_repeat.is_some_and(|at| now >= at) {
                        p.next_repeat = Some(now.saturating_add(u64::from(t.repeat) * 20));
                        emissions.push((11, 32));
                    }
                }
                if p.pending_short.is_some_and(|at| now >= at) {
                    p.pending_short = None;
                    emissions.push((2, 4));
                }
            } else if self.settings[i].sensor_settings.is_some() {
                let (report, deadtime, _, _, _) = sensor_times(&self.settings[i]);
                let p = &mut self.physical[i];
                if report == 0 {
                    p.report_at = None;
                }
                if p.hold_until.is_some_and(|at| now >= at) {
                    p.hold_until = None;
                    p.occupied = false;
                    clear_occupancy = true;
                    p.pending = Some((u16::from(p.movement), 2));
                }
                if p.report_at.is_none() && report > 0 {
                    p.report_at = Some(now.saturating_add(report));
                }
                if p.report_at.is_some_and(|at| now >= at) {
                    p.report_at = if report > 0 {
                        Some(now.saturating_add(report))
                    } else {
                        None
                    };
                    p.pending = Some(if self.config.instances[i] == 3 {
                        (u16::from(p.occupied) * 2 + u16::from(p.movement) + 4, 4)
                    } else {
                        (p.raw_10, 1)
                    });
                }
                if p.pending.is_some()
                    && p.last_event
                        .is_none_or(|at| now.saturating_sub(at) >= deadtime)
                {
                    let (info, mask) = p.pending.take().unwrap();
                    p.reported = info;
                    p.last_event = Some(now);
                    emissions.push((info, mask));
                }
            }
            if clear_occupancy {
                self.update_query_value(i as u8, 0);
            }
            for (info, mask) in emissions {
                emit(self, i, info, mask, &mut events);
            }
        }
        events
    }
}
