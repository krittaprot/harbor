//! Migrate only the old startup file created for this installed Harbor bundle.
use std::{fs, os::unix::fs::MetadataExt, path::Path};

pub fn migrate_legacy() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    migrate_in(&crate::plist_util::get_user_agents_dir(), &executable).map(|_| ())
}

fn migrate_in(agents: &Path, executable: &Path) -> Result<bool, String> {
    let source = agents.join("launchd-ui.plist");
    let metadata = match fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(format!(
                "Could not inspect the old Harbor startup entry: {e}"
            ));
        }
    };
    if !metadata.is_file() || metadata.uid() != unsafe { libc::getuid() } {
        return Ok(false);
    }
    let value = match plist::Value::from_file(&source) {
        Ok(value) => value,
        Err(_) => return Ok(false), // An unfamiliar file is not ours to migrate.
    };
    let Some(dict) = value.as_dictionary() else {
        return Ok(false);
    };
    let Some(args) = dict.get("ProgramArguments").and_then(|v| v.as_array()) else {
        return Ok(false);
    };
    let Some(old_executable) = args.first().and_then(|v| v.as_string()).map(Path::new) else {
        return Ok(false);
    };
    let same_directory = old_executable
        .parent()
        .zip(executable.parent())
        .is_some_and(|(old, new)| {
            old == new
                || old
                    .canonicalize()
                    .ok()
                    .zip(new.canonicalize().ok())
                    .is_some_and(|(a, b)| a == b)
        });
    if dict.get("Label").and_then(|v| v.as_string()) != Some("launchd-ui")
        || args.len() != 1
        || old_executable.file_name().and_then(|s| s.to_str()) != Some("launchd-ui")
        || !same_directory
        || dict.contains_key("Program")
    {
        return Ok(false);
    }
    let archive = agents.join(".harbor-archive");
    if fs::symlink_metadata(&archive).is_ok_and(|m| !m.is_dir()) {
        return Err("Harbor's startup archive is not a regular directory".into());
    }
    fs::create_dir_all(&archive).map_err(|e| e.to_string())?;
    let destination = archive.join("launchd-ui-before-harbor-name.plist");
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(
            "An archived launchd-ui startup entry already exists; review it before migrating"
                .into(),
        );
    }
    // Do not bootout: this entry may own the currently running app. Removing the
    // startup file avoids duplicate launches at the next login without killing it.
    fs::rename(source, destination).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_only_this_bundles_entry_and_preserves_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let agents = dir.path().join("LaunchAgents");
        fs::create_dir(&agents).unwrap();
        let old = dir.path().join("Harbor.app/Contents/MacOS/launchd-ui");
        let new = old.with_file_name("harbor-app");
        let xml = format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>launchd-ui</string><key>ProgramArguments</key><array><string>{}</string></array><key>RunAtLoad</key><true/></dict></plist>",
            old.display()
        );
        let source = agents.join("launchd-ui.plist");
        fs::write(&source, &xml).unwrap();
        assert!(
            !migrate_in(
                &agents,
                Path::new("/Applications/Other.app/Contents/MacOS/harbor-app")
            )
            .unwrap()
        );
        assert!(source.exists());
        assert!(migrate_in(&agents, &new).unwrap());
        let archive = agents.join(".harbor-archive/launchd-ui-before-harbor-name.plist");
        assert_eq!(fs::read_to_string(archive).unwrap(), xml);
        assert!(!source.exists());
        assert!(!migrate_in(&agents, &new).unwrap());
        fs::write(&source, &xml).unwrap();
        assert!(migrate_in(&agents, &new).is_err());
        assert_eq!(fs::read_to_string(source).unwrap(), xml);
    }

    #[test]
    fn never_migrates_symlinks_or_custom_commands() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("custom.plist");
        fs::write(&target, "<plist><dict/></plist>").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("launchd-ui.plist")).unwrap();
        assert!(
            !migrate_in(
                dir.path(),
                Path::new("/Applications/Harbor.app/Contents/MacOS/harbor-app")
            )
            .unwrap()
        );
        assert!(target.exists());
        let source = dir.path().join("launchd-ui.plist");
        fs::remove_file(&source).unwrap();
        let custom = "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>launchd-ui</string><key>ProgramArguments</key><array><string>/Applications/Harbor.app/Contents/MacOS/launchd-ui</string><string>--custom</string></array></dict></plist>";
        fs::write(&source, custom).unwrap();
        assert!(
            !migrate_in(
                dir.path(),
                Path::new("/Applications/Harbor.app/Contents/MacOS/harbor-app")
            )
            .unwrap()
        );
        assert_eq!(fs::read_to_string(source).unwrap(), custom);
    }
}
