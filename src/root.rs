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

    /// Resolve an archive destination while rejecting symlinked ancestors
    /// that would redirect writes outside the configured target root.
    pub fn checked_join(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let destination = self.join(path);
        let canonical_root = std::fs::canonicalize(&self.path).map_err(|error| {
            anyhow::anyhow!(
                "target root {} must exist before resolving archive paths: {}",
                self.path.display(),
                error
            )
        })?;

        if destination == self.path {
            return Ok(destination);
        }

        let mut ancestor = destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("archive destination has no parent"))?;
        let mut missing = Vec::new();

        while let Err(error) = std::fs::symlink_metadata(ancestor) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.into());
            }
            let name = ancestor.file_name().ok_or_else(|| {
                anyhow::anyhow!("could not resolve archive destination parent")
            })?;
            missing.push(name.to_os_string());
            ancestor = ancestor.parent().ok_or_else(|| {
                anyhow::anyhow!("archive destination is outside the target root")
            })?;
        }

        let canonical_ancestor = std::fs::canonicalize(ancestor)?;
        if !canonical_ancestor.starts_with(&canonical_root) {
            anyhow::bail!(
                "refusing archive destination through a symlink outside target root: {}",
                destination.display()
            );
        }

        let mut resolved_parent = canonical_ancestor;
        for component in missing.iter().rev() {
            resolved_parent.push(component);
        }
        if !resolved_parent.starts_with(&canonical_root) {
            anyhow::bail!(
                "refusing archive destination outside target root: {}",
                destination.display()
            );
        }

        Ok(destination)
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


#[cfg(all(test, unix))]
mod symlink_tests {
    use super::Root;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temp_root() -> (PathBuf, PathBuf) {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "systemplumber-root-check-{}-{id}",
            std::process::id()
        ));
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        (base, root)
    }

    #[test]
    fn checked_join_rejects_symlinked_parent_outside_root() {
        let (base, root_path) = temp_root();
        let outside = base.join("outside");
        symlink(&outside, root_path.join("escape")).unwrap();

        let root = Root::new(&root_path);
        assert!(root.checked_join("escape/payload").is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn checked_join_rejects_broken_symlink_ancestors() {
        let (base, root_path) = temp_root();
        let outside_missing = base.join("outside").join("not-created");
        symlink(&outside_missing, root_path.join("broken")).unwrap();

        let root = Root::new(&root_path);
        assert!(root.checked_join("broken/payload").is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
