//! Versioned inventory and read-only import of legacy DALI configuration.
//!
//! Filesystem operations are synchronous. Daemon callers must run them off async
//! workers under a deadline; the check-only entry point also bounds process exit.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_CONFIG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema_version: u32,
    pub revision: u64,
    pub inventory_epoch: String,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub revision_high_water: Option<u64>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub identity_ledger: Option<crate::inventory::Ledger>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub buses: Vec<Bus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bus {
    pub bus: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub enabled: bool,
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub short_address: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub instances: Vec<InputInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InputInstance {
    pub number: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Native type, including unrecognized types. Not a capability claim.
    pub instance_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub short_address: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Public component contract, independently of observed hardware support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_representation: Option<ColorRepresentation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorRepresentation {
    Temperature,
    Xy,
    Rgbwaf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub group_address: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub members: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub legacy: PathBuf,
    pub current: PathBuf,
}

impl ConfigPaths {
    pub fn in_directory(directory: impl AsRef<Path>) -> Self {
        Self {
            legacy: directory.as_ref().join("dali.json"),
            current: directory.as_ref().join("dali.v2.json"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Existing,
    Migrated,
    Created,
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub inventory: Inventory,
    pub origin: Origin,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration I/O at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid JSON configuration at {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("invalid configuration at {path}: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error("configuration at {path} exceeds {MAX_CONFIG_BYTES} bytes")]
    TooLarge { path: PathBuf },
    #[error("configuration at {path} must be a regular file, not a symlink or special file")]
    NotRegular { path: PathBuf },
    #[error("v1 import and v2 configuration paths must identify different files")]
    AliasedPaths,
}

fn io_error(path: &Path, source: io::Error) -> ConfigError {
    ConfigError::Io {
        path: path.to_owned(),
        source,
    }
}

fn invalid(path: &Path, message: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        path: path.to_owned(),
        message: message.into(),
    }
}

/// Select v2 first, or create it by importing v1 or initializing a new controller.
///
/// This function never opens v1 for writing and never performs hardware/network
/// operations. Filesystem work must run on a caller-owned bounded blocking worker.
pub fn load_or_migrate(paths: &ConfigPaths, new_name: &str) -> Result<LoadedConfig, ConfigError> {
    check_distinct_paths(paths)?;
    if let Some(inventory) = load_current(&paths.current)? {
        return Ok(LoadedConfig {
            inventory,
            origin: Origin::Existing,
        });
    }

    let (inventory, origin) = match read_regular(&paths.legacy)? {
        Some(bytes) => {
            let old: LegacyInventory =
                serde_json::from_slice(&bytes).map_err(|source| ConfigError::Json {
                    path: paths.legacy.clone(),
                    source,
                })?;
            (old.into_current(), Origin::Migrated)
        }
        None => (Inventory::empty(new_name.to_owned()), Origin::Created),
    };
    let source = if origin == Origin::Migrated {
        &paths.legacy
    } else {
        &paths.current
    };
    validate(&inventory).map_err(|message| invalid(source, message))?;
    let bytes = serde_json::to_vec_pretty(&inventory).map_err(|source| ConfigError::Json {
        path: paths.current.clone(),
        source,
    })?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            path: paths.current.clone(),
        });
    }
    if publish_new(&paths.current, &bytes)? {
        Ok(LoadedConfig { inventory, origin })
    } else {
        // A concurrent creator wins without being overwritten, even when its
        // document is invalid. Invalid means error, never fallback to our import.
        let inventory = load_current(&paths.current)?.ok_or_else(|| {
            invalid(
                &paths.current,
                "concurrently created v2 disappeared; retry startup",
            )
        })?;
        Ok(LoadedConfig {
            inventory,
            origin: Origin::Existing,
        })
    }
}

impl Inventory {
    fn empty(name: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: 1,
            inventory_epoch: format!("{:032x}", rand::random::<u128>()),
            revision_high_water: None,
            identity_ledger: None,
            name,
            description: None,
            buses: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyInventory {
    name: String,
    buses: Vec<LegacyBus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBus {
    bus: u8,
    description: String,
    #[serde(rename = "status")]
    _status: LegacyStatus,
    channels: Vec<LegacyOutput>,
    #[serde(default)]
    groups: Vec<LegacyGroup>,
}

#[derive(Deserialize)]
enum LegacyStatus {
    Active,
    NoPower,
    Overloaded,
    Unknown,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyOutput {
    short_address: u8,
    description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGroup {
    group_address: u8,
    description: String,
    members: Vec<u8>,
}

impl LegacyInventory {
    fn into_current(self) -> Inventory {
        let mut inventory = Inventory::empty(self.name);
        inventory.buses = self
            .buses
            .into_iter()
            .map(|bus| Bus {
                bus: bus.bus,
                name: bus.description,
                description: None,
                enabled: true,
                inputs: Vec::new(),
                outputs: bus
                    .channels
                    .into_iter()
                    .map(|output| Output {
                        short_address: output.short_address,
                        name: output.description,
                        description: None,
                        color_representation: None,
                    })
                    .collect(),
                groups: bus
                    .groups
                    .into_iter()
                    .map(|group| Group {
                        group_address: group.group_address,
                        name: group.description,
                        description: None,
                        members: group.members,
                    })
                    .collect(),
            })
            .collect();
        inventory
    }
}

fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        return Err("name is empty or whitespace-only");
    }
    if name.chars().any(char::is_control) {
        return Err("name contains a control character");
    }
    if name.contains('/') {
        return Err("name contains a path separator");
    }
    if matches!(name, "." | "..") {
        return Err("name is a path step");
    }
    Ok(())
}

fn validate_controller_name(name: &str) -> Result<(), &'static str> {
    validate_name(name)?;
    // Mirrors BridgeKit wire_id_problem until the P3 runtime dependency lands.
    // Component names have separate encoding and need not obey this restriction.
    if name.contains(['+', '#']) {
        return Err("controller name contains an MQTT wildcard");
    }
    if name.starts_with(['@', '*']) || (name.starts_with('{') && name.ends_with('}')) {
        return Err("controller name uses Store path syntax");
    }
    if name.ends_with('@') {
        return Err("controller name ends in a generated-name marker");
    }
    let chars: Vec<_> = name.chars().collect();
    if chars.len() >= 2 && chars[chars.len() - 1] == '_' && chars[chars.len() - 2].is_ascii_digit()
    {
        return Err("controller name ends in a reserved generated-name suffix");
    }
    if chars.len() >= 3 && chars[0] == '_' && chars[1] != '_' && chars[2..].contains(&'_') {
        return Err("controller name has a reserved sort prefix");
    }
    Ok(())
}

fn deserialize_present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn validate(inventory: &Inventory) -> Result<(), String> {
    crate::inventory::Model::new(inventory.clone()).map(|_| ())
}

pub(crate) fn validate_structure(inventory: &Inventory) -> Result<(), String> {
    if inventory.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {}; expected {SCHEMA_VERSION}",
            inventory.schema_version
        ));
    }
    if inventory.revision == 0 {
        return Err("revision must be positive".into());
    }
    if inventory
        .revision_high_water
        .is_some_and(|high| high < inventory.revision)
    {
        return Err("revision_high_water is below current revision".into());
    }
    if inventory.inventory_epoch.len() != 32
        || !inventory
            .inventory_epoch
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("inventory_epoch must contain 32 lowercase hex characters".into());
    }
    validate_controller_name(&inventory.name).map_err(|why| format!("name: {why}"))?;
    let mut buses = HashSet::new();
    let mut names = HashSet::new();
    for (index, bus) in inventory.buses.iter().enumerate() {
        let at = format!("buses[{index}]");
        validate_name(&bus.name).map_err(|why| format!("{at}.name: {why}"))?;
        if bus.bus >= 4 {
            return Err(format!("{at}.bus must be in 0..3"));
        }
        if !buses.insert(bus.bus) {
            return Err(format!("{at}.bus duplicates another bus index"));
        }
        if !names.insert(&bus.name) {
            return Err(format!("{at}.name duplicates another bus name"));
        }
        let mut addresses = HashSet::new();
        let mut output_names = HashSet::new();
        for (i, output) in bus.outputs.iter().enumerate() {
            let at = format!("{at}.outputs[{i}]");
            validate_name(&output.name).map_err(|why| format!("{at}.name: {why}"))?;
            if output.short_address >= 64 {
                return Err(format!("{at}.short_address must be in 0..63"));
            }
            if !addresses.insert(output.short_address) {
                return Err(format!("{at}.short_address duplicates another output"));
            }
            if !output_names.insert(&output.name) {
                return Err(format!("{at}.name duplicates another output name"));
            }
        }
        let mut groups = HashSet::new();
        let mut input_addresses = HashSet::new();
        let mut input_names = HashSet::new();
        for (i, input) in bus.inputs.iter().enumerate() {
            let at = format!("{at}.inputs[{i}]");
            validate_name(&input.name).map_err(|why| format!("{at}.name: {why}"))?;
            if input.short_address >= 64 || !input_addresses.insert(input.short_address) {
                return Err(format!(
                    "{at}.short_address must be unique in inputs and in 0..63"
                ));
            }
            if !input_names.insert(&input.name) {
                return Err(format!("{at}.name duplicates another input name"));
            }
            let mut numbers = HashSet::new();
            let mut instance_names = HashSet::new();
            for (j, instance) in input.instances.iter().enumerate() {
                let at = format!("{at}.instances[{j}]");
                validate_name(&instance.name).map_err(|why| format!("{at}.name: {why}"))?;
                if instance.number >= 32 || !numbers.insert(instance.number) {
                    return Err(format!(
                        "{at}.number must be unique in its module and in 0..31"
                    ));
                }
                if !instance_names.insert(&instance.name) {
                    return Err(format!("{at}.name duplicates another instance name"));
                }
            }
        }
        let mut group_names = HashSet::new();
        for (i, group) in bus.groups.iter().enumerate() {
            let at = format!("{at}.groups[{i}]");
            validate_name(&group.name).map_err(|why| format!("{at}.name: {why}"))?;
            if group.group_address >= 16 {
                return Err(format!("{at}.group_address must be in 0..15"));
            }
            if !groups.insert(group.group_address) {
                return Err(format!("{at}.group_address duplicates another group"));
            }
            if !group_names.insert(&group.name) {
                return Err(format!("{at}.name duplicates another group name"));
            }
            let mut members = HashSet::new();
            for member in &group.members {
                if !addresses.contains(member) {
                    return Err(format!("{at}.members references absent output {member}"));
                }
                if !members.insert(member) {
                    return Err(format!("{at}.members repeats output {member}"));
                }
            }
        }
    }
    Ok(())
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub(crate) fn check_distinct_paths(paths: &ConfigPaths) -> Result<(), ConfigError> {
    if paths.current == paths.legacy {
        return Err(ConfigError::AliasedPaths);
    }
    let resolved = |path: &Path| {
        let directory = fs::canonicalize(parent(path)).ok()?;
        Some(directory.join(path.file_name()?))
    };
    if let (Some(current), Some(legacy)) = (resolved(&paths.current), resolved(&paths.legacy)) {
        if current == legacy {
            return Err(ConfigError::AliasedPaths);
        }
    }
    if let (Ok(current), Ok(legacy)) = (
        fs::symlink_metadata(&paths.current),
        fs::symlink_metadata(&paths.legacy),
    ) {
        if current.dev() == legacy.dev() && current.ino() == legacy.ino() {
            return Err(ConfigError::AliasedPaths);
        }
    }
    Ok(())
}

pub(crate) fn load_current(path: &Path) -> Result<Option<Inventory>, ConfigError> {
    let Some(bytes) = read_regular(path)? else {
        return Ok(None);
    };
    let inventory = serde_json::from_slice(&bytes).map_err(|source| ConfigError::Json {
        path: path.to_owned(),
        source,
    })?;
    validate(&inventory).map_err(|message| invalid(path, message))?;
    Ok(Some(inventory))
}

fn read_regular(path: &Path) -> Result<Option<Vec<u8>>, ConfigError> {
    read_regular_with(path, || {})
}

fn read_regular_with(
    path: &Path,
    before_open: impl FnOnce(),
) -> Result<Option<Vec<u8>>, ConfigError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(path, error)),
    };
    if !metadata.is_file() {
        return Err(ConfigError::NotRegular {
            path: path.to_owned(),
        });
    }
    before_open();
    // O_NOFOLLOW defeats a symlink swapped after the initial metadata lookup.
    // O_NONBLOCK also prevents a replacement FIFO holding the blocking worker.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| io_error(path, error))?;
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() {
        return Err(ConfigError::NotRegular {
            path: path.to_owned(),
        });
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(ConfigError::TooLarge {
            path: path.to_owned(),
        });
    }
    let mut bytes = Vec::new();
    file.take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            path: path.to_owned(),
        });
    }
    Ok(Some(bytes))
}

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // An interrupted process may leave this name behind; readers ignore it.
        let _ = fs::remove_file(&self.0);
    }
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<bool, ConfigError> {
    publish_new_with(path, bytes, || {})
}

fn publish_new_with(
    path: &Path,
    bytes: &[u8],
    before_publish: impl FnOnce(),
) -> Result<bool, ConfigError> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            path: path.to_owned(),
        });
    }
    let name = path
        .file_name()
        .ok_or_else(|| invalid(path, "destination must name a file"))?;
    let mut temporary_name = std::ffi::OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(
        ".{}.{:032x}.tmp",
        std::process::id(),
        rand::random::<u128>()
    ));
    let temporary_path = parent(path).join(temporary_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|error| io_error(&temporary_path, error))?;
    let temporary = TemporaryFile(temporary_path);
    file.set_permissions(Permissions::from_mode(0o600))
        .map_err(|error| io_error(&temporary.0, error))?;
    file.write_all(bytes)
        .map_err(|error| io_error(&temporary.0, error))?;
    file.sync_all()
        .map_err(|error| io_error(&temporary.0, error))?;
    before_publish();
    // Unlike rename, link never replaces an existing destination. The complete,
    // synced inode becomes visible in one operation on local Unix filesystems.
    match fs::hard_link(&temporary.0, path) {
        Ok(()) => {
            fs::remove_file(&temporary.0).map_err(|error| io_error(&temporary.0, error))?;
            File::open(parent(path))
                .and_then(|directory| directory.sync_all())
                .map_err(|error| io_error(parent(path), error))?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

#[cfg(test)]
mod tests;
