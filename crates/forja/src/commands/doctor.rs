use forja_core::{run_checks, CheckStatus, DoctorReport, SystemCommandRunner};

/// `forja doctor` — run environment diagnostics (RF-05). Returns the report
/// rather than a `Result`: every individual check failure is data, not a
/// control-flow error — `main` decides the exit code from the report.
pub fn run(json: bool) -> DoctorReport {
    let runner = SystemCommandRunner;
    let report = run_checks(&runner);

    if json {
        let rendered =
            serde_json::to_string_pretty(&report).expect("DoctorReport serialization cannot fail");
        println!("{rendered}");
    } else {
        for check in &report.checks {
            let marker = match check.status {
                CheckStatus::Ok => "✓",
                CheckStatus::Warning => "⚠",
                CheckStatus::Failed => "✗",
            };
            println!("  {marker} {}: {}", check.name, check.detail);
        }
    }

    report
}
