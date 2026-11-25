//! A `PreToolUse` command hook that denies `rm -rf` in Bash and lets everything else through.
//!
//! Register it in `settings.json`:
//! `{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "/path/to/guard"}]}]}}`

use std::io::Read;
use std::process::ExitCode;

use claudehooks::output::PermissionDecision;
use claudehooks::{Event, HookOutput, SpecificOutput, parse_input};

fn main() -> ExitCode {
    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        eprintln!("guard: read stdin: {e}");
        return ExitCode::FAILURE;
    }
    let input = match parse_input(&raw) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("guard: parse hook input: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Event::PreToolUse { tool_input, .. } = input.event else {
        return ExitCode::SUCCESS;
    };
    let command = tool_input.get("command").and_then(|c| c.as_str()).unwrap_or_default();
    if !command.contains("rm -rf") {
        return ExitCode::SUCCESS;
    }
    let out = HookOutput {
        hook_specific_output: Some(SpecificOutput::PreToolUse {
            permission_decision: Some(PermissionDecision::Deny),
            permission_decision_reason: Some("guard: rm -rf is not allowed".into()),
            updated_input: None,
            additional_context: None,
        }),
        ..HookOutput::default()
    };
    match serde_json::to_string(&out) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("guard: write hook output: {e}");
            ExitCode::FAILURE
        }
    }
}
