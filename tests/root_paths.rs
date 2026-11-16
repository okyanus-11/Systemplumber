use flush::root::Root;
use std::path::PathBuf;

#[test]
fn absolute_paths_are_interpreted_inside_target_root() {
    let root = Root::new("/tmp/systemplumber-root");
    assert_eq!(
        root.join("/etc/systemplumber/config.toml"),
        PathBuf::from("/tmp/systemplumber-root/etc/systemplumber/config.toml")
    );
}

#[test]
fn parent_components_cannot_escape_target_root() {
    let root = Root::new("/tmp/systemplumber-root");
    assert_eq!(
        root.join("../../etc/passwd"),
        PathBuf::from("/tmp/systemplumber-root/etc/passwd")
    );
    assert_eq!(
        root.join("var/../../outside"),
        PathBuf::from("/tmp/systemplumber-root/outside")
    );
}

#[test]
fn root_path_resolves_to_selected_root() {
    let root = Root::new("/tmp/systemplumber-root");
    assert_eq!(root.join("/"), PathBuf::from("/tmp/systemplumber-root"));
}
