use crate::shell::exec::{self, CommandResult};

pub fn execute_command_impl(
    command: String,
    args: Vec<String>,
    workdir: Option<String>,
) -> Result<CommandResult, String> {
    exec::execute(&command, &args, workdir.as_deref()).map_err(|e| e.to_string())
}
