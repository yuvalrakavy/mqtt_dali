//! Canonical single-segment physical targets for the BridgeKit lanes.
use std::{fmt, str::FromStr};
pub const MAX_TARGET_BYTES: usize = 4096;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    Outputs,
    Inputs,
    Groups,
    Scenes,
}
impl Namespace {
    fn name(self) -> &'static str {
        match self {
            Self::Outputs => "Outputs",
            Self::Inputs => "Inputs",
            Self::Groups => "Groups",
            Self::Scenes => "Scenes",
        }
    }
    fn parse(name: &str) -> Result<Self, TargetError> {
        match name {
            "Outputs" => Ok(Self::Outputs),
            "Inputs" => Ok(Self::Inputs),
            "Groups" => Ok(Self::Groups),
            "Scenes" => Ok(Self::Scenes),
            _ => Err(TargetError("unknown address space")),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Route {
    Inventory,
    Item {
        bus: String,
        space: Namespace,
        name: String,
        instance: Option<String>,
    },
}
/// Validated construction prevents alternate spellings and invalid public values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    encoded: String,
    route: Route,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid DALI target: {0}")]
pub struct TargetError(&'static str);
impl Target {
    pub fn item(bus: &str, space: Namespace, name: &str) -> Result<Self, TargetError> {
        Self::build(bus, space, name, None)
    }
    pub fn input_instance(bus: &str, module: &str, instance: &str) -> Result<Self, TargetError> {
        Self::build(bus, Namespace::Inputs, module, Some(instance))
    }
    fn build(
        bus: &str,
        space: Namespace,
        name: &str,
        instance: Option<&str>,
    ) -> Result<Self, TargetError> {
        let mut encoded = format!("{}.{}.{}", encode(bus)?, space.name(), encode(name)?);
        if let Some(instance) = instance {
            encoded.push_str(".Instances.");
            encoded.push_str(&encode(instance)?);
        }
        if encoded.len() > MAX_TARGET_BYTES {
            return Err(TargetError("encoded target exceeds 4096 bytes"));
        }
        Ok(Self {
            encoded,
            route: Route::Item {
                bus: bus.into(),
                space,
                name: name.into(),
                instance: instance.map(str::to_owned),
            },
        })
    }
    pub fn inventory() -> Self {
        Self {
            encoded: "__inventory".into(),
            route: Route::Inventory,
        }
    }
    pub fn parse(text: &str) -> Result<Self, TargetError> {
        if text.len() > MAX_TARGET_BYTES {
            return Err(TargetError("encoded target exceeds 4096 bytes"));
        }
        if text == "__inventory" {
            return Ok(Self::inventory());
        }
        let parts: Vec<_> = text.split('.').collect();
        if !matches!(parts.len(), 3 | 5) {
            return Err(TargetError("wrong target depth"));
        }
        let space = Namespace::parse(parts[1])?;
        let bus = decode(parts[0])?;
        let name = decode(parts[2])?;
        let target = if parts.len() == 5 {
            if space != Namespace::Inputs || parts[3] != "Instances" {
                return Err(TargetError("instances require an Inputs module"));
            }
            Self::input_instance(&bus, &name, &decode(parts[4])?)?
        } else {
            Self::item(&bus, space, &name)?
        };
        if target.as_str() != text {
            return Err(TargetError("noncanonical spelling"));
        }
        Ok(target)
    }
    pub fn as_str(&self) -> &str {
        &self.encoded
    }
    pub fn is_inventory(&self) -> bool {
        matches!(self.route, Route::Inventory)
    }
    pub fn bus(&self) -> Option<&str> {
        match &self.route {
            Route::Item { bus, .. } => Some(bus),
            Route::Inventory => None,
        }
    }
    pub fn namespace(&self) -> Option<Namespace> {
        match &self.route {
            Route::Item { space, .. } => Some(*space),
            Route::Inventory => None,
        }
    }
    pub fn name(&self) -> Option<&str> {
        match &self.route {
            Route::Item { name, .. } => Some(name),
            Route::Inventory => None,
        }
    }
    pub fn instance(&self) -> Option<&str> {
        match &self.route {
            Route::Item { instance, .. } => instance.as_deref(),
            Route::Inventory => None,
        }
    }
}
impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for Target {
    type Err = TargetError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}
fn encode(name: &str) -> Result<String, TargetError> {
    if name.len() > MAX_TARGET_BYTES
        || name.trim().is_empty()
        || name.chars().any(char::is_control)
        || name.contains('/')
        || matches!(name, "." | "..")
    {
        return Err(TargetError("invalid path component"));
    }
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
            output.push(byte as char);
        } else {
            output.push('%');
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 15) as usize] as char);
        }
    }
    Ok(output)
}
fn decode(text: &str) -> Result<String, TargetError> {
    let mut bytes = Vec::new();
    let mut input = text.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or(TargetError("invalid percent escape"))?;
            let low = input
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or(TargetError("invalid percent escape"))?;
            bytes.push(((high << 4) | low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|_| TargetError("invalid UTF-8 component"))
}
#[cfg(test)]
mod tests;
