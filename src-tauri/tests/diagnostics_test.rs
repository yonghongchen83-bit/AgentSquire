use squirecli_lib::commands::diagnostics::{get_output_impl, get_errors_impl};

#[test]
fn get_output_returns_empty_list_by_default() {
    let out = get_output_impl("chat".to_string()).expect("get_output should succeed");
    assert!(out.is_empty());
}

#[test]
fn get_errors_returns_empty_list_by_default() {
    let out = get_errors_impl().expect("get_errors should succeed");
    assert!(out.is_empty());
}
