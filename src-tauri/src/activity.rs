//! 活动记录：截了什么、录了什么、润色了什么、哪里失败了。
//!
//! 主窗口的「时间线」「今日流水」和侧栏里那只猫的播报都靠它。只记结果，不记过程；
//! 最多留 300 条，落在配置目录的 activity.json。

use super::*;
use serde::Deserialize;

const ACTIVITY_LIMIT: usize = 300;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityEntry {
    pub(crate) id: String,
    pub(crate) at: u64,
    /// capture | record | polish | error
    pub(crate) kind: String,
    pub(crate) title: String,
    /// 一小段数字信息，如「1920 × 1080」「58 → 427 字」「02:13」
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) meta: Option<String>,
    /// 较长的补充：润色结果开头、错误原文、录像文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot_id: Option<String>,
}

pub(crate) struct ActivityLog {
    path: PathBuf,
    entries: Vec<ActivityEntry>,
    counter: u64,
}

impl ActivityLog {
    pub(crate) fn load(path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<ActivityEntry>>(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            entries,
            counter: 0,
        }
    }

    fn push(&mut self, mut entry: ActivityEntry) -> ActivityEntry {
        self.counter += 1;
        entry.id = format!("act-{}-{}", entry.at, self.counter);
        self.entries.insert(0, entry.clone());
        self.entries.truncate(ACTIVITY_LIMIT);
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(payload) = serde_json::to_vec(&self.entries) {
            let _ = fs::write(&self.path, payload);
        }
        entry
    }
}

/// 一条活动的内容；id 与时间由 [`record`] 补上
pub(crate) struct Activity<'a> {
    pub(crate) kind: &'a str,
    pub(crate) title: String,
    pub(crate) meta: Option<String>,
    pub(crate) detail: Option<String>,
    pub(crate) snapshot_id: Option<String>,
}

impl<'a> Activity<'a> {
    pub(crate) fn new(kind: &'a str, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            meta: None,
            detail: None,
            snapshot_id: None,
        }
    }

    pub(crate) fn meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    pub(crate) fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(crate) fn snapshot(mut self, id: Option<String>) -> Self {
        self.snapshot_id = id;
        self
    }
}

/// 记一条并广播 `activity-added`。记录失败不影响调用方。
pub(crate) fn record(app: &AppHandle, activity: Activity<'_>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let entry = state.activity.lock().push(ActivityEntry {
        id: String::new(),
        at: now_millis(),
        kind: activity.kind.to_string(),
        title: activity.title,
        meta: activity.meta,
        detail: activity.detail,
        snapshot_id: activity.snapshot_id,
    });
    let _ = app.emit("activity-added", &entry);
}

/// 失败也是一条活动：界面上的猫会播报它
pub(crate) fn record_error(app: &AppHandle, title: impl Into<String>, error: &str) {
    record(
        app,
        Activity::new("error", title).detail(truncate(error, 200)),
    );
}

#[tauri::command]
pub(crate) fn list_activity(state: State<'_, AppState>) -> Vec<ActivityEntry> {
    state.activity.lock().entries.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str) -> ActivityEntry {
        ActivityEntry {
            id: String::new(),
            at: 1,
            kind: "capture".into(),
            title: title.into(),
            meta: None,
            detail: None,
            snapshot_id: None,
        }
    }

    #[test]
    fn newest_first_and_capped() {
        let path = std::env::temp_dir().join("snapshot-activity-test.json");
        let _ = fs::remove_file(&path);
        let mut log = ActivityLog::load(path.clone());
        for index in 0..(ACTIVITY_LIMIT + 5) {
            log.push(entry(&index.to_string()));
        }
        assert_eq!(log.entries.len(), ACTIVITY_LIMIT);
        assert_eq!(log.entries[0].title, (ACTIVITY_LIMIT + 4).to_string());
        // 同一毫秒里的多条也得有不同的 id
        assert_ne!(log.entries[0].id, log.entries[1].id);

        let reloaded = ActivityLog::load(path.clone());
        assert_eq!(reloaded.entries.len(), ACTIVITY_LIMIT);
        let _ = fs::remove_file(&path);
    }
}
