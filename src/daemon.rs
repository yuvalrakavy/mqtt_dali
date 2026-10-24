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
            "hat-tcp",
            "raw TCP HAT endpoint at numeric IP:port, instead of UART",
        ),
        (
            "uart-repeat-terminals",
            "known repeated-command completion profile: unverified, 1 or 2 (default unverified)",
        ),
        (
            "uart-quiet-ms",
            "quiet recovery interval, 1..5000 ms (default 250)",
        ),
        (
            "web-bind",
            "enable web access at IP:port (disabled by default)",
        ),
        (
            "web-origin",
            "canonical browser http(s) origin; do not infer proxy headers",
        ),
        ("web-password", "web password (or MQTT_DALI_WEB_PASSWORD)"),
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
    let mut web = None;
    let parsed = parsed.and_then(|()| {
        for (option, _) in &declared {
            if settings.options.get(option).is_some_and(String::is_empty) {
                return Err(Problem::Usage(format!("--{option} must not be empty")));
            }
        }
        hat_endpoint(&settings)?;
        uart_quiet(settings.options.get("uart-quiet-ms").map(String::as_str))?;
        web = web_options(&mut settings)?;
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
    let mut bridge = Bridge::new("DALI")
        .app_name("mqtt_dali")
        .version(&version)
        .request_lifetime_limit(std::time::Duration::from_secs(180));
    for (key, help) in &declared {
        if key != "web-password" {
            bridge = bridge.option(key, help);
        }
    }
    let bridge = bridge.args(runtime_arguments(arguments.expect("validated arguments")));
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
    if let Some(web) = web {
        service = service.with_web(web);
    }
    let endpoint = hat_endpoint(bridge.settings()).expect("validated HAT endpoint");
    if let Some(endpoint) = endpoint {
        let repeat =
            uart_repeat(bridge.value("uart-repeat-terminals")).expect("validated repeat profile");
        service = match endpoint {
            crate::atx::connection::Endpoint::Uart(path) => service.with_uart(path, repeat),
            crate::atx::connection::Endpoint::Tcp(address) => service.with_hat_tcp(address, repeat),
        };
        service = service
            .with_uart_quiet_period(
                uart_quiet(bridge.value("uart-quiet-ms")).expect("validated quiet interval"),
            )
            .expect("validated interval and enabled UART");
    }
    // WAIT: v2-daemon-run
    bridge.run_managed(service)
}

fn hat_endpoint(settings: &Settings) -> Result<Option<crate::atx::connection::Endpoint>, Problem> {
    use crate::atx::connection::Endpoint;
    if let Some(value) = settings.options.get("hat-tcp") {
        if settings.options.contains_key("uart") {
            return Err(Problem::Usage(
                "--hat-tcp and --uart are mutually exclusive".into(),
            ));
        }
        let address = value
            .parse::<std::net::SocketAddr>()
            .ok()
            .filter(|a| a.port() != 0)
            .ok_or_else(|| {
                Problem::Usage("--hat-tcp must be a numeric IP:port with a nonzero port".into())
            })?;
        return Ok(Some(Endpoint::Tcp(address)));
    }
    Ok(
        match settings
            .options
            .get("uart")
            .map(String::as_str)
            .unwrap_or("/dev/serial0")
        {
            "none" => None,
            path => Some(Endpoint::Uart(path.into())),
        },
    )
}

fn web_options(settings: &mut Settings) -> Result<Option<crate::web::Settings>, Problem> {
    let password = settings.options.remove("web-password");
    let Some(bind) = settings.options.get("web-bind") else {
        if password.is_some() || settings.options.contains_key("web-origin") {
            return Err(Problem::Usage("web options require --web-bind".into()));
        }
        return Ok(None);
    };
    let bind: std::net::SocketAddr = bind
        .parse()
        .map_err(|_| Problem::Usage("--web-bind must be IP:port".into()))?;
    if bind.port() == 0 {
        return Err(Problem::Usage("--web-bind requires a nonzero port".into()));
    }
    let origin = settings
        .options
        .get("web-origin")
        .ok_or_else(|| Problem::Usage("web requires --web-origin".into()))?;
    let password = match password {
        Some(password) => password,
        None => match std::env::var("MQTT_DALI_WEB_PASSWORD") {
            Ok(password) => password,
            Err(std::env::VarError::NotPresent) => String::new(),
            Err(_) => return Err(Problem::Usage("web password must be valid UTF-8".into())),
        },
    };
    crate::web::Settings::new(bind, origin, &password)
        .map(Some)
        .map_err(Problem::Usage)
}
/// Privacy transform after the shared parser accepts the grammar. Every ordinary
/// option still keeps its literal value, even if that value looks like a flag.
fn runtime_arguments(arguments: Vec<String>) -> Vec<String> {
    let mut source = arguments.into_iter();
    let mut result = Vec::new();
    while let Some(flag) = source.next() {
        if flag == "--web-password" {
            source.next();
            continue;
        }
        if flag.starts_with("--web-password=") {
            continue;
        }
        let separate = flag.starts_with("--") && !flag.contains('=');
        result.push(flag);
        if separate {
            if let Some(value) = source.next() {
                result.push(value);
            }
        }
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hat_endpoint_selection_preserves_uart_defaults_and_accepts_numeric_ipv6() {
        use crate::atx::connection::Endpoint;
        let mut settings = Settings::new("DALI");
        assert!(
            matches!(hat_endpoint(&settings).unwrap(), Some(Endpoint::Uart(p)) if p == std::path::Path::new("/dev/serial0"))
        );
        settings.options.insert("uart".into(), "none".into());
        assert!(hat_endpoint(&settings).unwrap().is_none());
        settings
            .options
            .insert("uart".into(), "/tmp/test-pty".into());
        assert!(
            matches!(hat_endpoint(&settings).unwrap(), Some(Endpoint::Uart(p)) if p == std::path::Path::new("/tmp/test-pty"))
        );
        settings.options.remove("uart");
        for endpoint in ["127.0.0.1:5580", "[::1]:5580"] {
            settings.options.insert("hat-tcp".into(), endpoint.into());
            assert!(
                matches!(hat_endpoint(&settings).unwrap(), Some(Endpoint::Tcp(a)) if a.to_string() == endpoint)
            );
        }
    }
    #[test]
    fn hat_tcp_selection_rejects_conflicting_uart_even_when_disabled() {
        for uart in ["none", "/dev/serial0"] {
            let mut settings = Settings::new("DALI");
            settings
                .options
                .insert("hat-tcp".into(), "127.0.0.1:5580".into());
            settings.options.insert("uart".into(), uart.into());
            assert!(
                matches!(hat_endpoint(&settings), Err(Problem::Usage(s)) if s == "--hat-tcp and --uart are mutually exclusive"),
                "ambiguous UART/TCP selection was accepted"
            );
        }
    }
    #[test]
    fn web_password_is_consumed_before_runtime_settings_and_literal_values_survive() {
        let secret = "unique-web-secret-9e8c169f91";
        let options = vec![
            ("web-password".into(), "password".into()),
            ("web-bind".into(), "bind".into()),
            ("web-origin".into(), "origin".into()),
        ];
        for password in [
            vec!["--web-password".to_owned(), secret.into()],
            vec![format!("--web-password={secret}")],
        ] {
            let mut args = vec![
                "--instance".into(),
                "--web-password".into(),
                "--broker".into(),
                "127.0.0.1:9".into(),
                "--web-bind".into(),
                "127.0.0.1:8080".into(),
                "--web-origin".into(),
                "http://localhost:8080".into(),
            ];
            args.extend(password);
            let mut preflight = Settings::new("DALI");
            config::parse(args.clone(), &mut preflight, &options).unwrap();
            assert!(web_options(&mut preflight).unwrap().is_some());
            assert!(
                !format!("{preflight:?}").contains(secret),
                "preflight retained web credential in Debug settings"
            );
            let sanitized = runtime_arguments(args);
            let mut runtime = Settings::new("DALI");
            let ordinary: Vec<_> = options
                .iter()
                .filter(|(name, _)| name != "web-password")
                .cloned()
                .collect();
            config::parse(sanitized, &mut runtime, &ordinary).unwrap();
            assert_eq!(runtime.instance.as_deref(), Some("--web-password"));
            assert!(!runtime.options.contains_key("web-password"));
            assert!(
                !format!("{runtime:?}").contains(secret),
                "runtime Debug settings retained web credential"
            );
        }
    }
}
