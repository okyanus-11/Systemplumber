use std::io::Cursor;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tar::{Archive, EntryType};

use crate::mirror::MirrorClient;
use crate::mirror_selector::MirrorSelector;
use crate::mirrorlist::MirrorList;
use crate::package::Package;
use crate::root::Root;

pub struct ArchBackend {
    mirrorlist: MirrorList,
    repository: String,
    architecture: String,
}

impl ArchBackend {
    pub fn new(
        mirrorlist: MirrorList,
        repository: impl Into<String>,
        architecture: impl Into<String>,
    ) -> Self {
        Self {
            mirrorlist,
            repository: repository.into(),
            architecture: architecture.into(),
        }
    }

    pub fn install(&self, root: &Root, package: &Package) -> anyhow::Result<()> {
        let filename = package
            .filename
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("package {} has no payload filename", package.name))?;

        let expected_checksum = package
            .checksum
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("package {} has no SHA-256 checksum", package.name))?;

        let mirror = MirrorSelector::select(&self.mirrorlist)?;
        println!(
            "Selected mirror: {}
            ",
            mirror.name
        );

        let path = format!("{}/os/{}/{}", self.repository, self.architecture, filename);
        println!(
            "Downloading package: {path}
            "
        );

        let client = MirrorClient::new()?;
        let data = client.fetch_bytes(mirror, &path)?;

        println!(
            "Downloaded {} bytes
            ",
            data.len()
        );

        verify_sha256(&data, expected_checksum)?;
        println!(
            "SHA-256 verified.
            "
        );

        extract_package(root, &data)?;

        Ok(())
    }
}

fn reject_symlink_destination(path: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "refusing to write through existing symlink destination: {}",
                path.display()
            );
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_symlink_target(root: &Root, link_path: &Path, target: &Path) -> anyhow::Result<()> {
    use std::path::Component;

    if target.is_absolute() {
        anyhow::bail!(
            "refusing absolute symlink target {} for {}",
            target.display(),
            link_path.display()
        );
    }

    let mut depth = link_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();

    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir if depth > 0 => depth -= 1,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!(
                    "refusing symlink target that escapes target root: {} -> {}",
                    link_path.display(),
                    target.display()
                );
            }
            Component::CurDir => {}
        }
    }

    let target_relative = link_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(target);
    let target_path = root.checked_join(&target_relative)?;
    match std::fs::symlink_metadata(&target_path) {
        Ok(_) => {
            let canonical_root = std::fs::canonicalize(root.path())?;
            let canonical_target = std::fs::canonicalize(&target_path).map_err(|error| {
                anyhow::anyhow!(
                    "refusing unresolved symlink target {} -> {}: {}",
                    link_path.display(),
                    target.display(),
                    error
                )
            })?;
            if !canonical_target.starts_with(&canonical_root) {
                anyhow::bail!(
                    "refusing symlink target outside target root: {} -> {}",
                    link_path.display(),
                    target.display()
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    Ok(())
}

fn verify_sha256(data: &[u8], expected: &str) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(data);

    let digest = hasher.finalize();
    let actual = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!(
            "package checksum mismatch: expected {}, got {}
            ",
            expected,
            actual
        );
    }

    Ok(())
}

fn extract_package(root: &Root, data: &[u8]) -> anyhow::Result<()> {
    use std::fs;

    let decoder = zstd::stream::read::Decoder::new(Cursor::new(data))?;
    let mut archive = Archive::new(decoder);

    println!(
        "Extracting package into {}
            ",
        root.path().display()
    );

    let mut hard_links: Vec<(PathBuf, PathBuf)> = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        println!(
            "DEBUG archive entry: path={} type={:?} size{}",
            path.display(),
            entry.header().entry_type(),
            entry.header().size()?
        );

        validate_archive_path(&path)?;

        let destination = root.checked_join(&path)?;

        match entry.header().entry_type() {
            EntryType::Directory => {
                reject_symlink_destination(&destination)?;
                fs::create_dir_all(&destination)?;
            }

            EntryType::Regular => {
                reject_symlink_destination(&destination)?;
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut file = fs::File::create(&destination).map_err(|error| {
                    anyhow::anyhow!(
                        "failed to create {}: {}
                        ",
                        destination.display(),
                        error
                    )
                })?;

                std::io::copy(&mut entry, &mut file)?;
            }

            EntryType::Symlink => {
                let target = entry.link_name()?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "symlink {} has no target
                        ",
                        path.display()
                    )
                })?;
                validate_symlink_target(root, &path, &target)?;

                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }

                match fs::read_link(&destination) {
                    Ok(existing_target) if existing_target == target => {
                        continue;
                    }
                    Ok(existing_target) => {
                        anyhow::bail!(
                            "symlink conflict at {}: existing target is {}, expected {}",
                            destination.display(),
                            existing_target.display(),
                            target.display()
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(anyhow::anyhow!(
                            "failed to inspect symlink {}: {}
                            ",
                            destination.display(),
                            error
                        ));
                    }
                }

                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &destination).map_err(|error| {
                    anyhow::anyhow!(
                        "failed to create symlink {} -> {}: {}
                        ",
                        destination.display(),
                        target.display(),
                        error
                    )
                })?;

                #[cfg(not(unix))]
                anyhow::bail!("symlinks are not supported on this platform");
            }

            EntryType::Link => {
                let target = entry
                    .link_name()?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "hard link {} has no target
                        ",
                            path.display()
                        )
                    })?
                    .into_owned();

                validate_archive_path(&target)?;
                hard_links.push((path, target));
            }

            entry_type => {
                println!(
                    "Skipping unsupported archive entry {:?}: {}
                    ",
                    entry_type,
                    path.display()
                );
            }
        }
    }

    // Arch tar hard-link targets are archive-root-relative, not
    // relative to the directory containing the destination.
    //
    // Retry pending links so chains of hard links can be resolved.
    while !hard_links.is_empty() {
        let mut unresolved = Vec::new();
        let mut created_any = false;

        for (path, target) in hard_links {
            let destination = root.checked_join(&path)?;
            let target_path = root.checked_join(&target)?;
            reject_symlink_destination(&destination)?;
            reject_symlink_destination(&target_path)?;

            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }

            let target_metadata = match fs::metadata(&target_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    unresolved.push((path, target));
                    continue;
                }
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "failed to inspect hard-link target {}: {}
                        ",
                        target_path.display(),
                        error
                    ));
                }
            };

            match fs::symlink_metadata(&destination) {
                Ok(_) => {
                    let destination_metadata = fs::metadata(&destination).map_err(|error| {
                        anyhow::anyhow!(
                            "failed to inspect existing hard-link destination {}: {}
                            ",
                            destination.display(),
                            error
                        )
                    })?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;

                        if destination_metadata.dev() == target_metadata.dev()
                            && destination_metadata.ino() == target_metadata.ino()
                        {
                            created_any = true;
                            continue;
                        }
                    }

                    println!(
                        "DEBUG hardlink conflict: destination={} ({} bytes), target={} ({} bytes)",
                        destination.display(),
                        destination_metadata.len(),
                        target_path.display(),
                        target_metadata.len()
                    );

                    anyhow::bail!(
                        "hard-link destination already exists and differs from target: {} -> {}
                        ",
                        destination.display(),
                        target_path.display()
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "failed to inspect hard-link destination {}: {}
                        ",
                        destination.display(),
                        error
                    ));
                }
            }

            #[cfg(unix)]
            fs::hard_link(&target_path, &destination).map_err(|error| {
                anyhow::anyhow!(
                    "failed to create hard link {} -> {}: {}
                    ",
                    destination.display(),
                    target_path.display(),
                    error
                )
            })?;

            #[cfg(not(unix))]
            anyhow::bail!(
                "hard links are not supported on this platform
                "
            );

            created_any = true;
        }

        if unresolved.is_empty() {
            break;
        }

        if !created_any {
            let (path, target) = &unresolved[0];
            anyhow::bail!(
                "could not resolve hard link {} -> {}: target was not found after retrying pending links",
                root.join(path).display(),
                root.join(target).display()
            );
        }

        hard_links = unresolved;
    }

    Ok(())
}

fn validate_archive_path(path: &Path) -> anyhow::Result<()> {
    if path.is_absolute() {
        anyhow::bail!(
            "refusing to extract absolute archive path: {}
            ",
            path.display()
        );
    }

    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            anyhow::bail!(
                "refusing to extract unsafe archive path: {}
                ",
                path.display()
            );
        }
    }

    Ok(())
}
