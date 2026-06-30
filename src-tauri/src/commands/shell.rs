use crate::shell::exec::{self, CommandResult};

pub fn execute_command_impl(
    command: String,
    args: Vec<String>,
    workdir: Option<String>,
) -> Result<CommandResult, String> {
    exec::execute(&command, &args, workdir.as_deref()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_command_impl_reports_missing_binary() {
        let result = execute_command_impl(
            "definitely_not_a_real_command_xyz".to_string(),
            vec![],
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn execute_command_impl_runs_simple_echo() {
        let result = execute_command_impl(
            "cmd.exe".to_string(),
            vec!["/C".to_string(), "echo".to_string(), "hello".to_string()],
            None,
        )
        .expect("echo command should run");

        assert!(result.success);
        assert!(result.stdout.to_lowercase().contains("hello"));
    }
}
