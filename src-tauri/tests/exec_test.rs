use squirecli_lib::shell::exec::{execute, execute_with_stdin};

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
    )
    .unwrap();
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
    )
    .unwrap();
    assert!(result.success);
}
