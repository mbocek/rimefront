use std::path::{Path, PathBuf};
use std::process::Command;

/// The bundle's name inside the macOS disk image (tools/package_dmg.sh).
const APP_NAME: &str = "Iron Tide.app";
const STAGING_IN_FOLDER: &str = ".update";
const STAGING_BESIDE_APP: &str = ".iron-tide-update";
const UNPACKED: &str = "unpacked";
const PREVIOUS: &str = "previous";
const DISK_IMAGE_MOUNT: &str = "mount";

/// Where this copy of the game is installed, laid out as a release package lays it out.
#[derive(Clone, PartialEq, Debug)]
pub enum Installation {
    /// `Iron Tide.app`, from the macOS disk image.
    MacApp(PathBuf),
    /// A folder holding the executable beside `assets`, `docs` and the README, from an archive.
    Folder { root: PathBuf, executable: String },
}

impl Installation {
    /// `None` for a development build, which has no assets beside its executable.
    pub fn locate(executable: &Path) -> Option<Self> {
        let folder = executable.parent()?;
        let named = |path: Option<&Path>, name: &str| {
            path.and_then(Path::file_name)
                .is_some_and(|found| found == name)
        };
        let contents = folder.parent();
        let bundle = contents.and_then(Path::parent);
        let in_bundle = named(Some(folder), "MacOS")
            && named(contents, "Contents")
            && bundle.is_some_and(|bundle| bundle.extension().is_some_and(|ext| ext == "app"));
        if in_bundle {
            return bundle.map(|bundle| Installation::MacApp(bundle.to_path_buf()));
        }
        let executable = executable.file_name()?.to_str()?.to_string();
        folder
            .join("assets")
            .is_dir()
            .then(|| Installation::Folder {
                root: folder.to_path_buf(),
                executable,
            })
    }

    pub fn current() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        Self::locate(&executable.canonicalize().unwrap_or(executable))
    }

    /// The end of the release package name that installs this copy, matching what CI publishes.
    pub fn package_suffix(&self) -> String {
        let extension = match self {
            Installation::MacApp(_) => "dmg",
            Installation::Folder { .. } if cfg!(target_os = "windows") => "zip",
            Installation::Folder { .. } => "tar.gz",
        };
        format!("{}.{extension}", platform())
    }

    /// Where the package is downloaded and unpacked and the replaced files wait; it must be writable and on
    /// the same volume as the installation, so files move into place by renaming.
    pub fn staging(&self) -> PathBuf {
        match self {
            Installation::MacApp(bundle) => bundle.with_file_name(STAGING_BESIDE_APP),
            Installation::Folder { root, .. } => root.join(STAGING_IN_FOLDER),
        }
    }

    /// Empties the staging folder; failing here means this copy cannot update itself.
    pub fn prepare_staging(&self) -> std::io::Result<()> {
        let staging = self.staging();
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)
    }

    /// Removes what a finished update left behind, the copy it replaced among it.
    pub fn forget_finished_update(&self) {
        let _ = std::fs::remove_dir_all(self.staging());
    }

    /// Replaces this copy with the one in `package`; the running game keeps working until it quits.
    pub fn install(&self, package: &Path) -> Result<(), String> {
        let staging = self.staging();
        let fresh = self.unpack(package, &staging.join(UNPACKED))?;
        self.put_in_place(&fresh, &staging.join(PREVIOUS))
    }

    pub fn relaunch(&self) -> std::io::Result<()> {
        match self {
            Installation::MacApp(bundle) => Command::new("open").arg("-n").arg(bundle).spawn(),
            Installation::Folder { root, executable } => Command::new(root.join(executable))
                .current_dir(root)
                .spawn(),
        }
        .map(drop)
    }

    fn unpack(&self, package: &Path, into: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(into).map_err(|error| error.to_string())?;
        match self {
            Installation::MacApp(_) => copy_app_from_disk_image(package, into),
            Installation::Folder { executable, .. } => {
                run(Command::new("tar")
                    .arg("-xf")
                    .arg(package)
                    .arg("-C")
                    .arg(into))?;
                packaged_folder(into, executable)
            }
        }
    }

    fn put_in_place(&self, fresh: &Path, previous: &Path) -> Result<(), String> {
        std::fs::create_dir_all(previous).map_err(|error| error.to_string())?;
        match self {
            Installation::MacApp(bundle) => {
                let old = previous.join(APP_NAME);
                rename(bundle, &old)?;
                rename(fresh, bundle).inspect_err(|_| {
                    let _ = std::fs::rename(&old, bundle);
                })
            }
            Installation::Folder { root, .. } => replace_folder_contents(root, fresh, previous),
        }
    }
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "macos-arm64"
    } else if cfg!(target_os = "macos") {
        "macos-x86_64"
    } else if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    }
}

/// An archive holds one folder named after the version and platform, with the executable in it.
fn packaged_folder(unpacked: &Path, executable: &str) -> Result<PathBuf, String> {
    std::fs::read_dir(unpacked)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|folder| folder.join(executable).is_file())
        .ok_or_else(|| format!("the package has no {executable}"))
}

fn copy_app_from_disk_image(image: &Path, into: &Path) -> Result<PathBuf, String> {
    let mount = into.join(DISK_IMAGE_MOUNT);
    run(Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(image))?;
    let fresh = into.join(APP_NAME);
    let copied = run(Command::new("ditto").arg(mount.join(APP_NAME)).arg(&fresh));
    let _ = run(Command::new("hdiutil").arg("detach").arg(&mount));
    copied.map(|()| fresh)
}

/// Moves each top-level entry of the fresh copy into `root`, the old one aside into `previous`, and puts
/// everything back if any move fails. The running executable may be renamed, even on Windows.
fn replace_folder_contents(root: &Path, fresh: &Path, previous: &Path) -> Result<(), String> {
    let entries: Vec<PathBuf> = std::fs::read_dir(fresh)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    let mut placed: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for entry in entries {
        let Some(name) = entry.file_name() else {
            continue;
        };
        let target = root.join(name);
        let set_aside = previous.join(name);
        let moved = (|| {
            let replaced = target.exists().then(|| set_aside.clone());
            if replaced.is_some() {
                rename(&target, &set_aside)?;
            }
            placed.push((target.clone(), replaced));
            rename(&entry, &target)
        })();
        if let Err(reason) = moved {
            undo_placements(placed);
            return Err(reason);
        }
    }
    Ok(())
}

fn undo_placements(placed: Vec<(PathBuf, Option<PathBuf>)>) {
    for (target, replaced) in placed.into_iter().rev() {
        let Some(replaced) = replaced else {
            let _ = remove_any(&target);
            continue;
        };
        if replaced.exists() {
            let _ = remove_any(&target);
            let _ = std::fs::rename(&replaced, &target);
        }
    }
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn rename(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|error| {
        format!(
            "could not move {} to {}: {error}",
            from.display(),
            to.display()
        )
    })
}

fn run(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Checks the downloaded package against its line in the release's `SHA256SUMS.txt`, written by
/// `sha256sum` in CI.
pub fn verify_checksum(checksums: &str, package_name: &str, package: &Path) -> Result<(), String> {
    let expected = checksums
        .lines()
        .find_map(|line| {
            let (hash, name) = line.split_once(char::is_whitespace)?;
            (name.trim().trim_start_matches('*') == package_name).then(|| hash.to_lowercase())
        })
        .ok_or_else(|| format!("the release lists no checksum for {package_name}"))?;
    let bytes = std::fs::read(package).map_err(|error| error.to_string())?;
    let actual: String = ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{package_name} does not match its checksum"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "iron_tide_self_update_{}_{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn a_bundle_or_a_folder_with_assets_is_an_installation_and_a_development_build_is_not() {
        let bundle = Path::new("/Applications/Iron Tide.app");
        assert_eq!(
            Installation::locate(&bundle.join("Contents/MacOS/iron-tide")),
            Some(Installation::MacApp(bundle.to_path_buf()))
        );
        let root = scratch("locate");
        assert_eq!(Installation::locate(&root.join("iron-tide")), None);
        std::fs::create_dir_all(root.join("assets")).unwrap();
        assert_eq!(
            Installation::locate(&root.join("iron-tide")),
            Some(Installation::Folder {
                root: root.clone(),
                executable: "iron-tide".to_string(),
            })
        );
    }

    #[test]
    fn a_package_must_match_its_published_checksum() {
        let directory = scratch("checksum");
        let package = directory.join("iron-tide-9.0.0-linux-x86_64.tar.gz");
        write(&package, "abc");
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let other = "iron-tide-9.0.0-windows-x86_64.zip";
        let listed = format!("{abc}  iron-tide-9.0.0-linux-x86_64.tar.gz\n{abc}  {other}\n");
        assert_eq!(
            verify_checksum(&listed, "iron-tide-9.0.0-linux-x86_64.tar.gz", &package),
            Ok(())
        );
        let tampered = listed.replacen("ba78", "0000", 1);
        assert!(
            verify_checksum(&tampered, "iron-tide-9.0.0-linux-x86_64.tar.gz", &package).is_err()
        );
        assert!(verify_checksum(&listed, "missing.tar.gz", &package).is_err());
    }

    #[test]
    fn an_archive_replaces_the_folder_and_keeps_the_old_files_aside_until_forgotten() {
        let base = scratch("install");
        let root = base.join("iron-tide-1.0.0-linux-x86_64");
        write(&root.join("iron-tide"), "old game");
        write(&root.join("assets/models/tank.glb"), "old tank");
        write(&root.join("README.md"), "old readme");
        let packaged = base.join("source/iron-tide-9.0.0-linux-x86_64");
        write(&packaged.join("iron-tide"), "new game");
        write(&packaged.join("assets/models/tank.glb"), "new tank");
        write(&packaged.join("docs/rules.md"), "new rules");
        write(&packaged.join("README.md"), "new readme");
        let package = base.join("iron-tide-9.0.0-linux-x86_64.tar.gz");
        run(Command::new("tar")
            .arg("-czf")
            .arg(&package)
            .arg("-C")
            .arg(base.join("source"))
            .arg("iron-tide-9.0.0-linux-x86_64"))
        .unwrap();

        let installation = Installation::locate(&root.join("iron-tide")).unwrap();
        installation.prepare_staging().unwrap();
        installation.install(&package).unwrap();

        assert_eq!(read(&root.join("iron-tide")), "new game");
        assert_eq!(read(&root.join("assets/models/tank.glb")), "new tank");
        assert_eq!(read(&root.join("docs/rules.md")), "new rules");
        let previous = installation.staging().join(PREVIOUS);
        assert_eq!(read(&previous.join("iron-tide")), "old game");
        installation.forget_finished_update();
        assert!(!installation.staging().exists());
        assert_eq!(read(&root.join("README.md")), "new readme");
    }

    #[test]
    fn a_failed_move_puts_the_old_files_back() {
        let base = scratch("rollback");
        let root = base.join("game");
        write(&root.join("iron-tide"), "old game");
        write(&root.join("assets/a.txt"), "old asset");
        let fresh = base.join("fresh");
        write(&fresh.join("iron-tide"), "new game");
        write(&fresh.join("assets/a.txt"), "new asset");
        let previous = base.join("previous");
        std::fs::create_dir_all(&previous).unwrap();
        write(&previous.join("assets"), "a file where a folder has to go");

        assert!(replace_folder_contents(&root, &fresh, &previous).is_err());
        assert_eq!(read(&root.join("iron-tide")), "old game");
        assert_eq!(read(&root.join("assets/a.txt")), "old asset");
    }
}
