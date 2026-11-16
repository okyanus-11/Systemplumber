use std::process::Command;

#[test]
fn install_refuses_to_modify_host_root_without_explicit_opt_in() {
    let output = Command::new(env!("CARGO_BIN_EXE_flush"))
        .args(["install", "example-package"])
        .output()
        .expect("run flush");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to modify the host filesystem"));
    assert!(stderr.contains("--allow-host-root"));
}

#[test]
fn sync_refuses_to_modify_host_root_without_explicit_opt_in() {
    let output = Command::new(env!("CARGO_BIN_EXE_flush"))
        .arg("sync")
        .output()
        .expect("run flush");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to modify the host filesystem"));
    assert!(stderr.contains("--allow-host-root"));
}
