//! Independent immediate Part208 profile; never imports the bridge codec.
use super::scenario::Switching;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct State {
    #[serde(flatten)]
    profile: Switching,
    virtual_level: u8,
    on: bool,
    last_threshold: Option<u8>,
    #[serde(skip)]
    pending: Option<(u16, u64)>,
}
impl State {
    pub fn new(profile: Switching, actual: u8) -> Self {
        Self {
            profile,
            virtual_level: actual,
            on: actual != 0,
            last_threshold: None,
            pending: None,
        }
    }
    pub fn level(&self) -> u8 {
        self.virtual_level
    }
    pub fn actual(&self) -> u8 {
        if self.on {
            254
        } else {
            0
        }
    }
    pub fn adjustable(&self) -> bool {
        self.profile.adjustable_thresholds
    }
    pub fn drive(&mut self, level: u8) {
        if level == self.virtual_level {
            return;
        }
        let up = level > self.virtual_level;
        self.virtual_level = level;
        let on_index = if up { 0 } else { 2 };
        let off_index = on_index + 1;
        let on = self.profile.thresholds[on_index];
        let off = self.profile.thresholds[off_index];
        let applies = |threshold| {
            threshold != 255
                && if up {
                    level >= threshold
                } else {
                    level <= threshold
                }
        };
        let on_applies = applies(on);
        let off_applies = applies(off);
        // Last applicable threshold in the direction of travel wins; ties are On.
        // Evaluate the new direction even when its threshold lies before old VAPL.
        if on_applies && (!off_applies || if up { on >= off } else { on <= off }) {
            self.on = true;
            self.last_threshold = Some(on_index as u8);
        } else if off_applies {
            self.on = false;
            self.last_threshold = Some(off_index as u8);
        }
    }
    pub fn reset(&mut self) {
        self.profile.thresholds = [1, 255, 255, 0];
        self.virtual_level = 254;
        self.on = true;
        self.pending = None;
        // SWITCH STATUS is preserved by RESET in this raw Part208 profile.
    }
    /// Called for every frame addressed to this gear, before outer base repeat
    /// admission can return early. Another address does not cancel this pair.
    pub fn observe(&mut self, frame: u16) {
        if self.pending.is_some_and(|(old, _)| old != frame) {
            self.pending = None;
        }
    }
    /// Outer None is unsupported or not enabled. Inner None has no bus reply.
    pub fn command(
        &mut self,
        frame: u16,
        dtr0: u8,
        now_ms: u64,
        enabled: bool,
    ) -> Option<Option<u8>> {
        let opcode = frame as u8;
        if (0xe1..=0xe4).contains(&opcode) {
            let repeated = self
                .pending
                .take()
                .is_some_and(|(old, at)| old == frame && now_ms.saturating_sub(at) <= 100);
            if !repeated {
                if !enabled {
                    return None;
                }
                self.pending = Some((frame, now_ms));
                return Some(None);
            }
            if self.adjustable() {
                let index = usize::from(opcode - 0xe1);
                self.profile.thresholds[index] = if index < 2 { dtr0.max(1) } else { dtr0 };
            }
            return Some(None);
        }
        if !enabled {
            return None;
        }
        Some(Some(match opcode {
            0xf0 => {
                if self.adjustable() {
                    8
                } else {
                    0
                }
            }
            0xf1 => self.last_threshold.unwrap_or(0) << 2,
            0xf2..=0xf5 => self.profile.thresholds[usize::from(opcode - 0xf2)],
            0xf7 => self.profile.gear_type,
            0xff => 1, // Raw version of this declared emulated profile.
            _ => return None,
        }))
    }
}
