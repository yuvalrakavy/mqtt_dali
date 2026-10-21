//! mqtt_dali v2, on the shared BridgeKit process runtime.
fn main() -> std::process::ExitCode {
    mqtt_dali::daemon::run()
}
