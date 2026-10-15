//! Service mutations shared by the desktop IPC and the headless CLI.
use crate::{
    config, launchctl,
    model::{JobRef, Snapshot},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

impl std::str::FromStr for ServiceAction {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            "restart" => Ok(Self::Restart),
            _ => Err("Unknown service action".into()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ActionPlan {
    pub service_id: String,
    pub action: ServiceAction,
    /// Start in this order; stop in reverse. Already-loaded jobs are skipped on start.
    pub jobs: Vec<JobRef>,
}

pub fn plan(snapshot: &Snapshot, id: &str, action: ServiceAction) -> Result<ActionPlan, String> {
    let service = snapshot
        .services
        .iter()
        .find(|s| s.id == id)
        .ok_or("Service no longer exists")?;
    if service.jobs.is_empty() || service.jobs.iter().any(|j| !j.editable) {
        return Err("This service includes read-only jobs".into());
    }
    for job in &service.jobs {
        let path = job.plist_path.as_ref().ok_or("Job has no plist")?;
        config::user_plist(path)?;
        if !config::label_valid(&job.label)
            || crate::plist_util::parse_plist(path)
                .map_err(|e| e.to_string())?
                .label
                != job.label
        {
            return Err("Job label changed; rescan first".into());
        }
    }
    let mut jobs = service.jobs.clone();
    jobs.sort_by_key(|j| {
        if j.role.contains("postgres") {
            0
        } else if j.role.contains("tunnel") || j.role.contains("cloudflared") {
            2
        } else {
            1
        }
    });
    Ok(ActionPlan {
        service_id: id.into(),
        action,
        jobs,
    })
}

pub fn apply(snapshot: &Snapshot, id: &str, action: ServiceAction) -> Result<(), String> {
    let plan = plan(snapshot, id, action)?;
    if !matches!(action, ServiceAction::Start) {
        let loaded = launchctl::list_loaded().map_err(|e| e.to_string())?;
        for job in plan.jobs.iter().rev() {
            if loaded.iter().any(|l| l.label == job.label) {
                launchctl::bootout(job.plist_path.as_deref().unwrap())
                    .map_err(|e| format!("{}: {e}", job.label))?;
            }
        }
    }
    if !matches!(action, ServiceAction::Stop) {
        let loaded = launchctl::list_loaded().map_err(|e| e.to_string())?;
        for job in &plan.jobs {
            if !loaded.iter().any(|l| l.label == job.label) {
                launchctl::bootstrap(job.plist_path.as_deref().unwrap())
                    .map_err(|e| format!("{}: {e}", job.label))?;
            }
        }
    }
    Ok(())
}
