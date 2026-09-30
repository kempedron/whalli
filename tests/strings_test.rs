mod common;
use common::run_code;
use whalli::value::Value;

#[test]
fn test_multiline_string() {
    let code = r#"
    let sql_query = """
    SELECT id, name, email
    FROM users
    WHERE active = true
    """
    let has_newlines = sql_query.contains("\n")
    let has_select = sql_query.contains("SELECT")
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("has_newlines"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_select"), Some(&Value::Bool(true)));
}

#[test]
fn test_raw_string() {
    // Rust raw string r"C:\Users\name\new\test\d+" length:
    // C : \ U s e r s \ n a m e \ n e w \ t e s t \ d +
    // 1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1 = 25
    let code = r#"
    let regex_pat = r"C:\Users\name\new\test\d+"
    let has_backslashes = regex_pat.contains(r"\new")
    let l = regex_pat.len()
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("has_backslashes"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("l"), Some(&Value::Int(25)));
}

#[test]
fn test_multiline_fstring() {
    let code = r#"
    let table = "tasks"
    let status_filter = "done"
    let query = f"""
    SELECT * FROM {table}
    WHERE status = '{status_filter}'
    """
    let has_tasks = query.contains("tasks")
    let has_done = query.contains("done")
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("has_tasks"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_done"), Some(&Value::Bool(true)));
}

#[test]
fn test_raw_bytes() {
    // Rust raw string r"hello\x00world\n" length:
    // h e l l o \ x 0 0 w o r l d \ n = 16
    let code = r#"
    let raw = br"hello\x00world\n"
    let l = raw.len()
    let (s, _) = raw.decode()
    let contains_slash_x = s.contains(r"\x00")
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("l"), Some(&Value::Int(16)));
    assert_eq!(vm.globals.get("contains_slash_x"), Some(&Value::Bool(true)));
}
