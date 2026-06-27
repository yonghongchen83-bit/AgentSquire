#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let result = execute("cmd.exe", &["/C", "echo", "hello"].map(String::from), None).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn test_exit_code() {
        let result = execute("cmd.exe", &["/C", "exit", "0"].map(String::from), None).unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_with_stdin() {
        let result = execute_with_stdin(
            "cmd.exe",
            &["/C", "findstr", "hello"].map(String::from),
            "hello world\ngoodbye",
            None,
        ).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn test_command_not_found() {
        let result = execute("nonexistent_cmd_xyz", &[], None);
        assert!(result.is_err());
    }

    #[test]
    fn test_workdir() {
        let dir = std::env::current_dir().unwrap();
        let dir_str = dir.to_str().unwrap();
        let result = execute(
            "cmd.exe",
            &["/C", "echo", "%CD%"].map(String::from),
            Some(dir_str),
        ).unwrap();
        assert!(result.success);
    }
}

use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command not found: {0}")]
    NotFound(String),
    #[error("Non-zero exit code: {0}")]
    ExitCode(i32),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub fn execute(command: &str, args: &[String], workdir: Option<&str>) -> Result<CommandResult, ShellError> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = workdir {
        cmd.current_dir(dir);
    }

    let output = cmd.output()?;

    Ok(CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
    })
}

pub fn execute_with_stdin(
    command: &str,
    args: &[String],
    stdin: &str,
    workdir: Option<&str>,
) -> Result<CommandResult, ShellError> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = workdir {
        cmd.current_dir(dir);
    }

    let mut child = cmd.spawn()?;
    use std::io::Write;
    if let Some(mut stdin_writer) = child.stdin.take() {
        stdin_writer.write_all(stdin.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    Ok(CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
    })
}
