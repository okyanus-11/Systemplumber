use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Root {
    path: PathBuf,
}

impl Root {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn systemplumber_config(&self) -> PathBuf {
        self.etc().join("systemplumber")
    }

    pub fn bootstrap_config(&self) -> PathBuf {
        self.systemplumber_config().join("bootstrap.toml")
    }
    pub fn host() -> Self {
        Self::new("/")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Join a path beneath this root without allowing lexical parent traversal.
    ///
    /// RootDir and platform prefixes are ignored so paths such as /etc are
    /// interpreted relative to the selected target root. ParentDir components
    /// may only remove components added by this call; they can never pop the
    /// configured root itself.
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        let mut relative = PathBuf::new();

        for component in path.as_ref().components() {
            match component {
                Component::Normal(part) => relative.push(part),
                Component::ParentDir => {
                    relative.pop();
                }
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            }
        }

        self.path.join(relative)
    }

    pub fn etc(&self) -> PathBuf {
        self.join("/etc")
    }

    pub fn usr(&self) -> PathBuf {
        self.join("/usr")
    }

    pub fn var(&self) -> PathBuf {
        self.join("/var")
    }

    pub fn boot(&self) -> PathBuf {
        self.join("/boot")
    }

    pub fn package_db(&self) -> PathBuf {
        self.var().join("lib/systemplumber")
    }
    pub fn mirrorlist(&self) -> PathBuf {
        self.systemplumber_config().join("mirrorlist.toml")
    }
}


#[cfg(test)]
mod tests {
    use super::Root;
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
}
