use std::path::Path;
use std::time::{Duration, SystemTime};

pub(super) fn project_filter_looks_like_path(filter: &str) -> bool {
    Path::new(filter).components().count() > 1
}

pub(super) fn codex_project_filter_matches(cwd: &str, filter: &str) -> bool {
    if project_filter_looks_like_path(filter) {
        let cwd_path = Path::new(cwd);
        let filter_path = Path::new(filter);
        cwd_path == filter_path || cwd_path.starts_with(filter_path)
    } else {
        Path::new(cwd)
            .components()
            .any(|component| component.as_os_str() == filter)
    }
}

pub(super) fn cutoff_time(since_days: Option<u64>) -> Option<SystemTime> {
    since_days.map(|days| {
        SystemTime::now()
            .checked_sub(Duration::from_secs(days * 86400))
            .unwrap_or(SystemTime::UNIX_EPOCH)
    })
}
