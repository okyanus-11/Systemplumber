use std::process::Command;

fn run_placeholder(command: &str, args: &[&str]) {
    let root = std::env::temp_dir().join(format!(
        "systemplumber-placeholder-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temporary root");

    let output = Command::new(env!("CARGO_BIN_EXE_flush"))
        .arg("--root")
        .arg(&root)
        .arg(command)
        .args(args)
        .output()
        .expect("run flush");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not implemented yet"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains("no system changes were made"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn remove_is_explicitly_unavailable() {
    run_placeholder("remove", &["example-package"]);
}

#[test]
fn update_is_explicitly_unavailable() {
    run_placeholder("update", &[]);
}

#[test]
fn adopt_is_explicitly_unavailable() {
    run_placeholder("adopt", &[]);
}

#[test]
fn forge_is_explicitly_unavailable() {
    run_placeholder("forge", &[]);
}
