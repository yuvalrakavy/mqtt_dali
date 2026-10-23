//! Independent immediate DT1 model. Literal opcodes; no production codec imports.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub battery_charge: u8,
    pub features: u8,
    pub mode: u8,
    pub failures: u8,
    pub status: u8,
    pub emergency_level: u8,
    pub minimum_level: u8,
    pub maximum_level: u8,
    pub rated_duration: u8,
    pub duration_result: u8,
    pub emergency_hours: u8,
    pub total_hours: u8,
    pub function_interval_days: u8,
    pub duration_interval_weeks: u8,
    pub execution_timeout_days: u8,
    pub prolong: u8,
    pub function_delay_quarters: Option<u16>,
    pub duration_delay_quarters: Option<u16>,
    pub function_test_seconds: u32,
    pub duration_test_seconds: u32,
    pub function_test_passes: bool,
    pub duration_test_passes: bool,
    pub test_ready: bool,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            battery_charge: 254,
            features: 27,
            mode: 2,
            failures: 0,
            status: 0,
            emergency_level: 254,
            minimum_level: 1,
            maximum_level: 254,
            rated_duration: 90,
            duration_result: 0,
            emergency_hours: 0,
            total_hours: 0,
            function_interval_days: 0,
            duration_interval_weeks: 0,
            execution_timeout_days: 0,
            prolong: 0,
            function_delay_quarters: None,
            duration_delay_quarters: None,
            function_test_seconds: 10,
            duration_test_seconds: 10800,
            function_test_passes: true,
            duration_test_passes: true,
            test_ready: true,
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.minimum_level == 0
            || self.minimum_level > self.maximum_level
            || self.maximum_level > 254
            || self.emergency_level < self.minimum_level
            || self.emergency_level > self.maximum_level
            || self.duration_interval_weeks > 97
            || self.function_delay_quarters == Some(65535)
            || self.duration_delay_quarters == Some(65535)
            || !(1..=86400).contains(&self.function_test_seconds)
            || !(1..=86400).contains(&self.duration_test_seconds)
            || self.mode & 0x30 != 0
            || self.status & 0x30 != 0
        {
            return Err("invalid DT1 level limits, interval, delay or test duration; active/pending flags are runtime state".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivePatch {
    pub battery_charge: Option<u8>,
    pub failures: Option<u8>,
    pub test_ready: Option<bool>,
    pub function_test_passes: Option<bool>,
    pub duration_test_passes: Option<bool>,
}
#[derive(Serialize)]
pub struct State {
    #[serde(flatten)]
    profile: Box<Profile>,
    pub mode_raw: u8,
    pub status_raw: u8,
    pub active_test: Option<&'static str>,
    #[serde(skip)]
    active: Option<(usize, u64)>,
    #[serde(skip)]
    pending_since: [Option<u64>; 2],
    #[serde(skip)]
    next: [Option<u64>; 2],
    #[serde(skip)]
    repeated: Option<(u16, u64)>,
    #[serde(skip)]
    staged_delay: u16,
    #[serde(skip)]
    now_ms: u64,
}
impl State {
    pub fn new(profile: Box<Profile>, now_ms: u64) -> Self {
        let next = [
            profile.function_delay_quarters,
            profile.duration_delay_quarters,
        ]
        .map(|delay| delay.map(|q| now_ms.saturating_add(u64::from(q) * 900000)));
        let mut state = Self {
            mode_raw: profile.mode,
            status_raw: profile.status,
            profile,
            active_test: None,
            active: None,
            pending_since: [None; 2],
            next,
            repeated: None,
            staged_delay: 0,
            now_ms,
        };
        for index in 0..2 {
            if !state.automatic() || state.interval_ms(index) == 0 {
                state.next[index] = None;
            }
        }
        state
    }
    fn automatic(&self) -> bool {
        self.profile.features & 8 != 0
    }
    fn interval_ms(&self, index: usize) -> u64 {
        if index == 0 {
            u64::from(self.profile.function_interval_days) * 86400000
        } else {
            u64::from(self.profile.duration_interval_weeks) * 604800000
        }
    }
    fn remaining(&self, index: usize) -> Option<u16> {
        self.next[index].map(|at| at.saturating_sub(self.now_ms).div_ceil(900000).min(65534) as u16)
    }
    pub fn profile(&self) -> Box<Profile> {
        Box::new(Profile {
            function_delay_quarters: self.remaining(0),
            duration_delay_quarters: self.remaining(1),
            ..self.profile.as_ref().clone()
        })
    }
    pub fn patch(&mut self, patch: LivePatch, now_ms: u64) {
        if let Some(value) = patch.battery_charge {
            self.profile.battery_charge = value;
        }
        if let Some(value) = patch.failures {
            self.profile.failures = value;
        }
        if let Some(value) = patch.test_ready {
            self.profile.test_ready = value;
        }
        if let Some(value) = patch.function_test_passes {
            self.profile.function_test_passes = value;
        }
        if let Some(value) = patch.duration_test_passes {
            self.profile.duration_test_passes = value;
        }
        self.advance_to(now_ms);
    }
    fn request(&mut self, index: usize, now_ms: u64) {
        if self.active.is_some_and(|(kind, _)| kind == index) || self.pending_since[index].is_some()
        {
            return;
        }
        self.pending_since[index] = Some(now_ms);
    }
    pub fn advance_to(&mut self, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
        if let Some((index, _)) = self.active.filter(|(_, until)| self.now_ms >= *until) {
            let passed = if index == 0 {
                self.profile.function_test_passes
            } else {
                self.profile.duration_test_passes
            };
            let failure = 1 << (6 + index);
            self.profile.failures =
                (self.profile.failures & !failure) | if passed { 0 } else { failure };
            self.profile.status |= 1 << (1 + index);
            if index == 1 {
                self.profile.duration_result =
                    (self.profile.duration_test_seconds / 120).min(255) as u8;
            }
            self.active = None;
        }
        // At most one coalesced occurrence per kind, even across a huge jump.
        for index in 0..2 {
            if self.next[index].is_some_and(|at| self.now_ms >= at) {
                self.request(index, self.now_ms);
                let interval = self.interval_ms(index);
                self.next[index] = (interval != 0).then(|| self.now_ms.saturating_add(interval));
            }
            let timeout = if self.profile.execution_timeout_days == 0 {
                900000
            } else {
                u64::from(self.profile.execution_timeout_days) * 86400000
            };
            if self.pending_since[index]
                .is_some_and(|since| self.now_ms.saturating_sub(since) >= timeout)
            {
                self.profile.failures |= 1 << (4 + index);
            }
        }
        if self.active.is_none() && self.profile.test_ready {
            if let Some(index) = self.pending_since.iter().position(Option::is_some) {
                self.pending_since[index] = None;
                let seconds = if index == 0 {
                    self.profile.function_test_seconds
                } else {
                    self.profile.duration_test_seconds
                };
                self.active = Some((index, self.now_ms.saturating_add(u64::from(seconds) * 1000)));
            }
        }
        self.mode_raw = self.active.map_or(self.profile.mode, |(index, _)| {
            (self.profile.mode & !0x3f) | (1 << (4 + index))
        });
        self.status_raw = self.profile.status & !0x30;
        for index in 0..2 {
            if self.pending_since[index].is_some() {
                self.status_raw |= 1 << (4 + index);
            }
        }
        self.active_test = self
            .active
            .map(|(index, _)| if index == 0 { "function" } else { "duration" });
    }
    pub fn observe(&mut self, frame: u16) {
        if self.repeated.is_some_and(|(old, _)| old != frame) {
            self.repeated = None;
        }
    }
    /// A second repeated setter retains the first copy's enable authority.
    pub fn command(
        &mut self,
        frame: u16,
        dtr: &mut [u8; 3],
        now_ms: u64,
        enabled: bool,
    ) -> Option<Option<u8>> {
        let opcode = frame as u8;
        if matches!(opcode,0xe3..=0xe5|0xe9..=0xef) {
            let repeated = self
                .repeated
                .take()
                .is_some_and(|(old, at)| old == frame && now_ms.saturating_sub(at) <= 100);
            if !repeated {
                if !enabled {
                    return None;
                }
                self.repeated = Some((frame, now_ms));
                return Some(None);
            }
            let raw = dtr[0];
            match opcode {
                0xe3 | 0xe4 => self.request(usize::from(opcode - 0xe3), now_ms),
                0xe5 => {
                    self.active = None;
                    self.pending_since = [None; 2];
                }
                0xe9 if self.profile.features & 16 != 0
                    && (self.profile.minimum_level..=self.profile.maximum_level).contains(&raw) =>
                {
                    self.profile.emergency_level = raw
                }
                0xea if self.automatic() => {
                    self.staged_delay = (u16::from(raw) << 8) | (self.staged_delay & 255)
                }
                0xeb if self.automatic() => {
                    self.staged_delay = (self.staged_delay & 0xff00) | u16::from(raw)
                }
                0xec | 0xed if self.automatic() && (opcode == 0xec || raw <= 97) => {
                    let index = usize::from(opcode - 0xec);
                    if index == 0 {
                        self.profile.function_interval_days = raw;
                    } else {
                        self.profile.duration_interval_weeks = raw;
                    }
                    self.next[index] = (raw != 0 && self.staged_delay != 65535)
                        .then(|| now_ms.saturating_add(u64::from(self.staged_delay) * 900000));
                }
                0xee => self.profile.execution_timeout_days = raw,
                0xef => self.profile.prolong = raw,
                _ => {}
            }
            self.advance_to(now_ms);
            return Some(None);
        }
        if !enabled {
            return None;
        }
        let value = match opcode {
            0xf1 => self.profile.battery_charge,
            0xf2 => match dtr[0] {
                selector @ 0..=3 => {
                    let index = usize::from(selector / 2);
                    let word = if self.automatic() {
                        self.remaining(index).unwrap_or(65535)
                    } else {
                        65535
                    };
                    let [high, low] = word.to_be_bytes();
                    if selector % 2 == 0 {
                        dtr[1] = low;
                        high
                    } else {
                        low
                    }
                }
                4 => self.profile.function_interval_days,
                5 => self.profile.duration_interval_weeks,
                6 => self.profile.execution_timeout_days,
                7 => self.profile.prolong,
                _ => return Some(None),
            },
            0xf3 => self.profile.duration_result,
            0xf4 => self.profile.emergency_hours,
            0xf5 => self.profile.total_hours,
            0xf6 => self.profile.emergency_level,
            0xf7 => self.profile.minimum_level,
            0xf8 => self.profile.maximum_level,
            0xf9 => self.profile.rated_duration,
            0xfa => self.mode_raw,
            0xfb => self.profile.features,
            0xfc => self.profile.failures,
            0xfd => self.status_raw,
            0xff => 1,
            _ => return None,
        };
        Some(Some(value))
    }
}
