mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_sqlite_in_memory_crud() {
    let code = r#"
    import sql

    // 1. Open in-memory SQLite database
    let (db, err) = sql.open("sqlite", ":memory:")
    let open_ok = err == nil and db != nil

    // 2. Create table
    let (create_res, c_err) = db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, age INTEGER, active BOOLEAN)")
    let create_ok = c_err == nil

    // 3. Insert rows with parameterized query
    let (ins1, i_err1) = db.exec("INSERT INTO users (name, age, active) VALUES (?, ?, ?)", ["Alice", 30, true])
    let (ins2, i_err2) = db.exec("INSERT INTO users (name, age, active) VALUES (?, ?, ?)", ["Bob", 25, false])
    let (ins3, i_err3) = db.exec("INSERT INTO users (name, age, active) VALUES (?, ?, ?)", ["Charlie", 35, true])

    let ins_ok = i_err1 == nil and i_err2 == nil and i_err3 == nil
    let last_id = ins3["last_insert_id"]
    let rows_aff = ins3["rows_affected"]

    // 4. Query all rows
    let (rows, q_err) = db.query("SELECT id, name, age, active FROM users ORDER BY id ASC")
    let q_ok = q_err == nil
    let row_count = rows.len()
    let first_user = rows[0]
    let first_name = first_user["name"]
    let first_age = first_user["age"]

    // 5. Query single row with parameter
    let (user_bob, r_err) = db.query_row("SELECT id, name, age FROM users WHERE name = ?", ["Bob"])
    let r_ok = r_err == nil and user_bob != nil
    let bob_name = user_bob["name"]
    let bob_age = user_bob["age"]

    // 6. Update row
    let (upd_res, u_err) = db.exec("UPDATE users SET age = ? WHERE name = ?", [31, "Alice"])
    let upd_ok = u_err == nil and upd_res["rows_affected"] == 1

    let (alice_updated, _) = db.query_row("SELECT age FROM users WHERE name = ?", ["Alice"])
    let alice_new_age = alice_updated["age"]

    // 7. Delete row
    let (del_res, d_err) = db.exec("DELETE FROM users WHERE name = ?", ["Bob"])
    let del_ok = d_err == nil and del_res["rows_affected"] == 1

    let (rows_after, _) = db.query("SELECT id FROM users")
    let count_after = rows_after.len()

    // 8. Close db
    let close_ok = db.close()
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("open_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("create_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("ins_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("last_id"), Some(&Value::Int(3)));
    assert_eq!(vm.globals.get("rows_aff"), Some(&Value::Int(1)));

    assert_eq!(vm.globals.get("q_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("row_count"), Some(&Value::Int(3)));
    assert_eq!(
        vm.globals.get("first_name"),
        Some(&Value::Str(Arc::new("Alice".to_string())))
    );
    assert_eq!(vm.globals.get("first_age"), Some(&Value::Int(30)));

    assert_eq!(vm.globals.get("r_ok"), Some(&Value::Bool(true)));
    assert_eq!(
        vm.globals.get("bob_name"),
        Some(&Value::Str(Arc::new("Bob".to_string())))
    );
    assert_eq!(vm.globals.get("bob_age"), Some(&Value::Int(25)));

    assert_eq!(vm.globals.get("upd_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("alice_new_age"), Some(&Value::Int(31)));

    assert_eq!(vm.globals.get("del_ok"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("count_after"), Some(&Value::Int(2)));
    assert_eq!(vm.globals.get("close_ok"), Some(&Value::Bool(true)));
}

#[test]
fn test_sqlite_error_handling() {
    let code = r#"
    import sql

    let (db, _) = sql.open("sqlite", ":memory:")

    // Syntax error in query
    let (rows, q_err) = db.query("SELECT * FROM non_existent_table")
    let has_q_err = q_err != nil

    // Invalid driver
    let (bad_db, driver_err) = sql.open("unknown_db", "test")
    let has_driver_err = driver_err != nil
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("has_q_err"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("has_driver_err"), Some(&Value::Bool(true)));
}

#[test]
fn test_sql_postgres_and_mysql_connection_handling() {
    let code = r#"
    import sql

    // 1. Test postgres driver rejection with invalid host/connection string
    let (pg_db, pg_err) = sql.open("postgres", "postgresql://invalid_user:invalid_pass@127.0.0.1:54329/invalid_db")
    let pg_failed = pg_err != nil and pg_db == nil

    // 2. Test mysql driver rejection with invalid host/connection string
    let (my_db, my_err) = sql.open("mysql", "mysql://invalid_user:invalid_pass@127.0.0.1:33069/invalid_db")
    let my_failed = my_err != nil and my_db == nil
    "#;

    let vm = run_code(code);
    assert_eq!(vm.globals.get("pg_failed"), Some(&Value::Bool(true)));
    assert_eq!(vm.globals.get("my_failed"), Some(&Value::Bool(true)));
}

#[test]
fn test_sql_query_translation_for_postgres() {
    use whalli::stdlib::sql::translate_query_for_postgres;

    let q1 = "SELECT * FROM users WHERE id = ? AND age >= ? AND name = ?";
    let t1 = translate_query_for_postgres(q1);
    assert_eq!(t1, "SELECT * FROM users WHERE id = $1 AND age >= $2 AND name = $3");

    // Ensure '?' inside string literals is preserved
    let q2 = "SELECT * FROM users WHERE note = 'What?' AND id = ?";
    let t2 = translate_query_for_postgres(q2);
    assert_eq!(t2, "SELECT * FROM users WHERE note = 'What?' AND id = $1");
}
