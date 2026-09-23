mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_time_after_channel_fires() {
    let code = r#"
    import time

    let t = time.after(0.05)
    let stamp = <- t
    let got_stamp = stamp > 0.0
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("got_stamp"), Some(&Value::Bool(true)));
}

#[test]
fn test_time_after_in_select_timeout_fires() {
    let code = r#"
    import time

    let empty_ch = new(chan, 1)

    let result = select {
        msg <- empty_ch => "from_chan",
        <- time.after(0.05) => "timeout"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("result"), Some(&Value::Str(Arc::new("timeout".to_string()))));
}

#[test]
fn test_time_after_in_select_channel_wins() {
    let code = r#"
    import time

    let data_ch = new(chan, 1)
    data_ch <- "fast_data"

    let result = select {
        msg <- data_ch => msg,
        <- time.after(1.0) => "timeout"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("result"), Some(&Value::Str(Arc::new("fast_data".to_string()))));
}

#[test]
fn test_time_after_with_task_race() {
    let code = r#"
    import time

    func slow_job() -> str {
        time.sleep(0.3)
        return "job_done"
    }

    let task = wo slow_job()

    let outcome = select {
        item <- task => item[0],
        <- time.after(0.05) => "cancelled_by_timeout"
    }
    "#;
    let vm = run_code(code);
    assert_eq!(vm.globals.get("outcome"), Some(&Value::Str(Arc::new("cancelled_by_timeout".to_string()))));
}
