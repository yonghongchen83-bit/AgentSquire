use async_trait::async_trait;
use serde_json::Value;

use super::{Tool, ToolDanger, ToolResult};

/// A terminal command-execution tool whose description is injected with the
/// host OS and shell so the AI knows exactly what environment it is targeting.
pub struct TerminalTool {
    description: String,
}

impl TerminalTool {
    /// Build the tool with a compile-time OS-aware description.  Because the
    /// binary is compiled for a single target, `cfg!()` and `std::env::consts`
    /// give us the exact platform the AI's commands will run on.
    pub fn new() -> Self {
        let os = std::env::consts::OS;       // e.g. "windows", "macos", "linux"
        let family = std::env::consts::FAMILY; // e.g. "windows", "unix"

        let (shell, shell_hint) = if cfg!(target_os = "windows") {
            ("PowerShell 5.1", "Use PowerShell syntax: semicolons between commands (`a; b`), `$?` for last exit status, `$env:VAR` for env vars. Pipeline operators `&&`/`||` are NOT available — chain with `; if ($?) { ... }`. Backtick (`` ` ``) is the escape character, not backslash.")
        } else if cfg!(target_os = "macos") {
            ("zsh (macOS default)", "Standard POSIX shell syntax. `&&`/`||` for chaining. `$VAR` for env vars. Backslash escape.")
        } else {
            ("bash", "Standard POSIX shell syntax. `&&`/`||` for chaining. `$VAR` for env vars. Backslash escape.")
        };

        // Sanity check in case shell detection is off.
        let _ = (shell, shell_hint);

        let description = format!(
            "Execute a shell command. Requires user approval.\n\
             Host OS: {os} ({family})\n\
             Shell: {shell}\n\
             Shell guidance: {shell_hint}\n\
             Returns stdout, stderr, and exit code.",
        );

        Self { description }
    }
}

impl Default for TerminalTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TerminalTool {
    fn name(&self) -> &str {
        "run_terminal"
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute (e.g. 'cargo', 'npm', 'python')"
                },
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Command arguments"
                },
                "workdir": {
                    "type": "string",
                    "description": "Working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    fn danger(&self) -> ToolDanger {
        ToolDanger::Destructive
    }

    async fn execute(&self, call_id: &str, args: Value) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    call_id: call_id.to_string(),
                    output: "Missing required argument: command".to_string(),
                    is_error: true,
                }
            }
        };

        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let workdir = args.get("workdir").and_then(|v| v.as_str());

        match crate::shell::exec::execute(command, &cmd_args, workdir) {
            Ok(result) => {
                let mut output = String::new();
                if !result.stdout.is_empty() {
                    output.push_str(&result.stdout);
                }
                if !result.stderr.is_empty() {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&format!("stderr:\n{}", result.stderr));
                }
                if !result.success {
                    output.push_str(&format!("\n(exit code: {})", result.exit_code));
                }
                if output.is_empty() {
                    output = format!("Command completed with exit code {}", result.exit_code);
                }
                ToolResult {
                    call_id: call_id.to_string(),
                    output,
                    is_error: !result.success,
                }
            }
            Err(e) => ToolResult {
                call_id: call_id.to_string(),
                output: e.to_string(),
                is_error: true,
            },
        }
    }
}
