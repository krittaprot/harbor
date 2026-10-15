use crate::model::ProcInfo;
use chrono::DateTime;
use std::collections::{BTreeMap, BTreeSet};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Default)]
pub struct ProcessScanner {
    system: System,
    previous: BTreeMap<u32, u64>,
}

#[allow(dead_code)] // Descendant index is consumed by phase 2 grouping.
pub struct ProcessTable {
    pub by_pid: BTreeMap<u32, ProcInfo>,
    pub children: BTreeMap<u32, Vec<u32>>,
}

impl ProcessScanner {
    pub fn scan(&mut self) -> ProcessTable {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::Always)
                .with_cmd(UpdateKind::Always)
                .with_cwd(UpdateKind::Always),
        );
        let mut by_pid = BTreeMap::new();
        let mut current = BTreeMap::new();
        for (pid, process) in self.system.processes() {
            let pid = pid.as_u32();
            let started = process.start_time();
            let cpu = (self.previous.get(&pid) == Some(&started) && started != 0)
                .then(|| process.cpu_usage())
                .filter(|value| value.is_finite());
            current.insert(pid, started);
            by_pid.insert(
                pid,
                ProcInfo {
                    pid,
                    ppid: process.parent().map(|p| p.as_u32()).unwrap_or(0),
                    uid: process.user_id().map(|uid| **uid),
                    name: process.name().to_string_lossy().into_owned(),
                    argv: process
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy().into_owned())
                        .collect(),
                    cwd: process.cwd().map(|p| p.to_string_lossy().into_owned()),
                    cpu,
                    mem_bytes: process.memory(),
                    started_at: (started != 0)
                        .then(|| DateTime::from_timestamp(started as i64, 0))
                        .flatten(),
                    launchd_label: None,
                },
            );
        }
        self.previous = current;
        ProcessTable::new(by_pid)
    }
}

impl ProcessTable {
    pub fn new(by_pid: BTreeMap<u32, ProcInfo>) -> Self {
        let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for process in by_pid.values() {
            children.entry(process.ppid).or_default().push(process.pid);
        }
        Self { by_pid, children }
    }

    /// Cycle-safe because parent links can race with process exit/PID reuse.
    #[allow(dead_code)]
    pub fn descendants(&self, root: u32) -> Vec<u32> {
        let mut seen = BTreeSet::from([root]);
        let mut pending = vec![root];
        let mut result = Vec::new();
        while let Some(pid) = pending.pop() {
            for &child in self.children.get(&pid).into_iter().flatten() {
                if seen.insert(child) {
                    result.push(child);
                    pending.push(child);
                }
            }
        }
        result.sort_unstable();
        result
    }

    pub fn assign_launchd_labels(&mut self, jobs: &BTreeMap<u32, String>) {
        // Find the nearest job ancestor, including the process itself.
        for pid in self.by_pid.keys().copied().collect::<Vec<_>>() {
            let mut cursor = pid;
            let mut seen = BTreeSet::new();
            while cursor > 1 && seen.insert(cursor) {
                if let Some(label) = jobs.get(&cursor) {
                    self.by_pid.get_mut(&pid).unwrap().launchd_label = Some(label.clone());
                    break;
                }
                let Some(process) = self.by_pid.get(&cursor) else {
                    break;
                };
                cursor = process.ppid;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, ppid: u32) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            uid: Some(501),
            name: "fixture".into(),
            argv: vec![],
            cwd: None,
            cpu: None,
            mem_bytes: 0,
            started_at: None,
            launchd_label: None,
        }
    }

    #[test]
    fn descendants_and_nearest_launchd_ancestor_are_cycle_safe() {
        let mut table = ProcessTable::new(
            [(10, 1), (11, 10), (12, 11), (20, 21), (21, 20), (30, 999)]
                .into_iter()
                .map(|(pid, ppid)| (pid, process(pid, ppid)))
                .collect(),
        );
        assert_eq!(table.descendants(10), vec![11, 12]);
        assert_eq!(table.descendants(20), vec![21]);
        table.assign_launchd_labels(&BTreeMap::from([
            (10, "outer".into()),
            (11, "inner".into()),
        ]));
        assert_eq!(table.by_pid[&10].launchd_label.as_deref(), Some("outer"));
        assert_eq!(table.by_pid[&12].launchd_label.as_deref(), Some("inner"));
        assert_eq!(table.by_pid[&20].launchd_label, None);
        assert_eq!(table.by_pid[&30].launchd_label, None);
    }
}
