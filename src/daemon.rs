//! Canonical v2 composition; BridgeKit owns the process lifecycle.
use crate::{atx::transactions::RepeatPolicy, config::ConfigPaths, service::DaliService};
use mqtt_bridge_kit::{
    config::{self, Problem, Settings},
    Bridge,
};
use std::process::ExitCode;
#[allow(dead_code)]
mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

/// Run the canonical daemon or explicitly requested offline configuration check.
/// All resource ownership begins inside the shared managed process runtime.
pub fn run() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if offline_check(&arguments) {
        return crate::config_check::run();
    }
    let declared: Vec<(String, String)> = [
        ("config", "v2 configuration file (default dali.v2.json)"),
        (
            "legacy-config",
            "read-only v1 import file (default dali.json)",
        ),
        (
            "uart",
            "UART/PTY path (default /dev/serial0; none disables hardware)",
        ),
        (
            "uart-repeat-terminals",
            "known repeated-command completion profile: unverified, 1 or 2 (default unverified)",
        ),
        (
            "uart-quiet-ms",
            "quiet recovery interval, 1..5000 ms (default 250)",
        ),
    ]
    .into_iter()
    .map(|(key, help)| (key.into(), help.into()))
    .collect();
    let arguments: Result<Vec<_>, _> = arguments
        .into_iter()
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| Problem::Usage("arguments must be valid UTF-8".into()))
        })
        .collect();
    let mut settings = Settings::new("DALI");
    // Use the shared grammar, rendering its errors through the bounded offline
    // output helper before any process/resource starts. Valid arguments are
    // passed unchanged to Bridge; no bridge-specific copy of the grammar exists.
    let parsed = arguments
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|args| config::parse(args.clone(), &mut settings, &declared));
    let parsed = parsed.and_then(|()| {
        for (option, _) in &declared {
            if settings.options.get(option).is_some_and(String::is_empty) {
                return Err(Problem::Usage(format!("--{option} must not be empty")));
            }
        }
        uart_quiet(settings.options.get("uart-quiet-ms").map(String::as_str))?;
        uart_repeat(
            settings
                .options
                .get("uart-repeat-terminals")
                .map(String::as_str),
        )
        .map(|_| ())
    });
    if let Err(problem) = parsed {
        let usage=format!("{}\nOffline: --check-config [--config <file>] [--legacy-config <file>] [--name <controller>]\n",config::usage("mqtt_dali","DALI",&declared));
        let (descriptor, text, code) = match problem {
            Problem::Help => (libc::STDOUT_FILENO, usage, ExitCode::SUCCESS),
            Problem::Usage(message) => (
                libc::STDERR_FILENO,
                format!("mqtt_dali: {message}\n\n{usage}"),
                ExitCode::from(2),
            ),
        };
        crate::config_check::output(descriptor, text);
        return code;
    }
    let version = format!(
        "mqtt_dali {} (built {})",
        env!("CARGO_PKG_VERSION"),
        build_info::BUILT_TIME_UTC
    );
    let mut bridge = Bridge::new("DALI").app_name("mqtt_dali").version(&version);
    for (key, help) in &declared {
        bridge = bridge.option(key, help);
    }
    let bridge = bridge.args(arguments.expect("validated arguments"));
    let paths = ConfigPaths {
        current: bridge.value("config").unwrap_or("dali.v2.json").into(),
        legacy: bridge.value("legacy-config").unwrap_or("dali.json").into(),
    };
    let instance = bridge
        .settings()
        .instance
        .clone()
        .expect("validated required instance");
    let (mut service, _handle) = DaliService::new(paths, instance);
    let uart = bridge.value("uart").unwrap_or("/dev/serial0");
    if uart != "none" {
        service = service.with_uart(
            uart,
            uart_repeat(bridge.value("uart-repeat-terminals")).expect("validated repeat profile"),
        );
        service = service
            .with_uart_quiet_period(
                uart_quiet(bridge.value("uart-quiet-ms")).expect("validated quiet interval"),
            )
            .expect("validated interval and enabled UART");
    }
    // WAIT: v2-daemon-run
    bridge.run_managed(service)
}

fn uart_repeat(value: Option<&str>) -> Result<RepeatPolicy, Problem> {
    match value.unwrap_or("unverified") {
        "unverified" => Ok(RepeatPolicy::Unverified),
        "1" => Ok(RepeatPolicy::OneTerminal),
        "2" => Ok(RepeatPolicy::TwoTerminals),
        _ => Err(Problem::Usage(
            "--uart-repeat-terminals must be unverified, 1 or 2".into(),
        )),
    }
}
fn uart_quiet(value: Option<&str>) -> Result<std::time::Duration, Problem> {
    value
        .unwrap_or("250")
        .parse::<u64>()
        .ok()
        .filter(|value| (1..=5000).contains(value))
        .map(std::time::Duration::from_millis)
        .ok_or_else(|| {
            Problem::Usage("--uart-quiet-ms must be an integer between 1 and 5000".into())
        })
}

fn offline_check(arguments: &[std::ffi::OsString]) -> bool {
    // Shared CLI options take one value (possibly inline), except help. Only
    // recognize our offline mode in flag position; a value can have that name.
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--check-config") => return true,
            Some("--help" | "-h") | None => return false,
            Some(option) if option.starts_with("--") && !option.contains('=') => {
                arguments.next();
            }
            _ => {}
        }
    }
    false
}
