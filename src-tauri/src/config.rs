use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, sync::Mutex};

static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceConfig {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub icon: Option<String>,
    pub tint: Option<String>,
    pub labels: Vec<String>,
    pub depends_on: Vec<String>,
    pub health: Option<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub service: BTreeMap<String, ServiceConfig>,
    pub ignored: Vec<String>,
    pub notifications: bool,
    pub start_at_login: bool,
    pub login_paths: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut service = BTreeMap::new();
        service.insert(
            "crystal-blood".into(),
            ServiceConfig {
                name: Some("crystal-blood".into()),
                kind: Some("data".into()),
                icon: Some("drop".into()),
                tint: Some("red".into()),
                labels: vec!["com.ode.crystalblood.*".into()],
                depends_on: vec!["/Volumes/ProVault".into()],
                health: Some("http://127.0.0.1:5050/".into()),
                pinned: true,
            },
        );
        Self {
            service,
            ignored: vec![],
            notifications: true,
            start_at_login: true,
            login_paths: BTreeMap::new(),
        }
    }
}

pub fn directory() -> PathBuf {
    dirs::home_dir()
        .expect("home directory")
        .join("Library/Application Support/Harbor")
}

pub fn load() -> Result<Config, String> {
    let path = directory().join("services.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    toml::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("services.toml: {e}"))
}

fn write(config: &Config) -> Result<(), String> {
    fs::create_dir_all(directory()).map_err(|e| e.to_string())?;
    let path = directory().join("services.toml.tmp");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(
        toml::to_string_pretty(config)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(path, directory().join("services.toml")).map_err(|e| e.to_string())
}

pub fn update(change: impl FnOnce(&mut Config)) -> Result<Config, String> {
    let _lock = CONFIG_LOCK.lock().map_err(|e| e.to_string())?;
    let mut config = load()?;
    change(&mut config);
    write(&config)?;
    Ok(config)
}

pub fn seed(services: &[crate::model::Service]) -> Result<(), String> {
    let _lock = CONFIG_LOCK.lock().map_err(|e| e.to_string())?;
    if directory().join("services.toml").exists() {
        return Ok(());
    }
    let mut config = Config::default();
    for service in services.iter().filter(|s| !s.apple) {
        config
            .service
            .entry(service.id.clone())
            .or_insert_with(|| ServiceConfig {
                name: Some(service.name.clone()),
                kind: Some(service.kind.clone()),
                icon: Some(service.icon.clone()),
                tint: Some(service.tint.clone()),
                labels: service.jobs.iter().map(|j| j.label.clone()).collect(),
                pinned: service.pinned,
                ..Default::default()
            });
    }
    write(&config)
}

pub fn label_valid(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 200
        && label != "."
        && label != ".."
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn user_plist(path: &str) -> Result<PathBuf, String> {
    let base = crate::plist_util::get_user_agents_dir()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let candidate = PathBuf::from(path);
    let parent = candidate
        .parent()
        .ok_or("missing parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if parent != base || candidate.extension().and_then(|s| s.to_str()) != Some("plist") {
        return Err("Only direct user LaunchAgent plist files can be changed".into());
    }
    if candidate.exists()
        && candidate
            .canonicalize()
            .map_err(|e| e.to_string())?
            .parent()
            != Some(base.as_path())
    {
        return Err("Plist symlink leaves LaunchAgents".into());
    }
    if fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Editing a symlinked plist is not supported".into());
    }
    Ok(candidate)
}
