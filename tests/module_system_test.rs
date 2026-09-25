mod common;
use common::run_code;
use whalli::value::Value;
use std::sync::Arc;

#[test]
fn test_module_import_with_alias() {
    let sub_code = r#"
    let private_secret = 42

    pub func compute(x: int) -> int {
        return x + private_secret
    }

    pub let version = "1.0.0"
    "#;
    std::fs::write("/tmp/test_submodule.wh", sub_code).unwrap();

    let main_code = r#"
    import "/tmp/test_submodule.wh" as my_calc

    let res = my_calc.compute(10)
    let ver = my_calc.version
    let priv = my_calc["private_secret"]
    "#;
    let vm = run_code(main_code);

    assert_eq!(vm.globals.get("res"), Some(&Value::Int(52)));
    assert_eq!(vm.globals.get("ver"), Some(&Value::Str(Arc::new("1.0.0".to_string()))));
    assert_eq!(vm.globals.get("priv"), Some(&Value::Nil));

    std::fs::remove_file("/tmp/test_submodule.wh").ok();
}

#[test]
fn test_directory_import_with_mod_wh() {
    let pkg_dir = "/tmp/test_whalli_pkg";
    std::fs::create_dir_all(pkg_dir).unwrap();
    let mod_code = r#"
    pub func greet(name: str) -> str {
        return f"Hello, {name}!"
    }
    pub let count = 100
    "#;
    std::fs::write(format!("{}/mod.wh", pkg_dir), mod_code).unwrap();

    let main_code = r#"
    import "/tmp/test_whalli_pkg" as pkg

    let msg = pkg.greet("World")
    let c = pkg.count
    "#;
    let vm = run_code(main_code);

    assert_eq!(vm.globals.get("msg"), Some(&Value::Str(Arc::new("Hello, World!".to_string()))));
    assert_eq!(vm.globals.get("c"), Some(&Value::Int(100)));

    std::fs::remove_dir_all(pkg_dir).ok();
}

#[test]
fn test_default_stem_import_name() {
    let math_code = r#"
    pub func square(n: int) -> int {
        return n * n
    }
    "#;
    std::fs::write("/tmp/my_math_helper.wh", math_code).unwrap();

    let main_code = r#"
    import "/tmp/my_math_helper.wh"

    let sq = my_math_helper.square(7)
    "#;
    let vm = run_code(main_code);

    assert_eq!(vm.globals.get("sq"), Some(&Value::Int(49)));

    std::fs::remove_file("/tmp/my_math_helper.wh").ok();
}

#[test]
fn test_stdlib_import_with_alias() {
    let main_code = r#"
    import math as m

    let pi_val = m.pi
    let s = m.sin(0.0)
    "#;
    let vm = run_code(main_code);

    assert_eq!(vm.globals.get("s"), Some(&Value::Float(0.0)));
}

#[test]
fn test_module_caching_side_effects_run_once() {
    let counter_code = r#"
    pub let loaded = 1
    "#;
    std::fs::write("/tmp/cached_mod.wh", counter_code).unwrap();

    let main_code = r#"
    import "/tmp/cached_mod.wh" as m1
    import "/tmp/cached_mod.wh" as m2

    let same_obj = m1 == m2
    "#;
    let vm = run_code(main_code);

    assert_eq!(vm.globals.get("same_obj"), Some(&Value::Bool(true)));

    std::fs::remove_file("/tmp/cached_mod.wh").ok();
}
