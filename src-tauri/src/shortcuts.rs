//! 全局快捷键。
//!
//! 注册放在 Rust 侧：按下后在这里直接执行动作，不经过网页往返；注册只在启动和
//! 保存时各发生一次。原先由主窗口网页注册，保存一次会连带触发好几轮设置更新，
//! 几轮并发注册同一个键，应用自己和自己撞车，却被报成「被其他程序占用」。

use super::settings::ShortcutBinding;
use super::*;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 插件本体。所有键共用一个回调：按下哪个键，就去设置里查它绑的是哪个动作。
pub(crate) fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if !matches!(event.state, ShortcutState::Pressed) {
                return;
            }
            let Some(action) = bound_action(app, shortcut) else {
                return;
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = actions::perform(&app, &action).await {
                    eprintln!("snapshot: shortcut action {action} failed: {error}");
                    activity::record_error(&app, actions::action_failure_title(&action), &error);
                }
            });
        })
        .build()
}

fn bound_action(app: &AppHandle, pressed: &Shortcut) -> Option<String> {
    let state = app.try_state::<AppState>()?;
    let settings = state.settings.lock();
    settings
        .shortcuts
        .iter()
        .find(|binding| parse(binding).as_ref() == Some(pressed))
        .map(|binding| binding.action.clone())
}

fn parse(binding: &ShortcutBinding) -> Option<Shortcut> {
    binding.accelerator.as_deref()?.parse().ok()
}

/// 保存前的校验：每个键都得认得出，且不能两个动作绑同一个键。
/// 按解析后的键比较，所以「Alt+Shift+2」和「Shift+Alt+2」也算重复。
pub(crate) fn validate(bindings: &[ShortcutBinding]) -> Result<(), String> {
    let mut seen = Vec::new();
    for binding in bindings {
        let Some(accelerator) = binding.accelerator.as_deref() else {
            continue;
        };
        let shortcut: Shortcut = accelerator
            .parse()
            .map_err(|_| format!("「{accelerator}」不是有效的快捷键"))?;
        if seen.contains(&shortcut) {
            return Err("快捷键不能重复".into());
        }
        seen.push(shortcut);
    }
    Ok(())
}

/// 用一组绑定整体替换系统里的注册，返回没能注册上的键。
pub(crate) fn register_all(app: &AppHandle, bindings: &[ShortcutBinding]) -> Vec<String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    bindings
        .iter()
        .filter_map(|binding| binding.accelerator.as_deref())
        .filter(|accelerator| manager.register(*accelerator).is_err())
        .map(str::to_string)
        .collect()
}

pub(crate) fn conflict_message(failed: &[String]) -> String {
    format!(
        "这些快捷键没能注册，可能已被其他程序占用：{}",
        failed.join("、")
    )
}

/// 启动时没注册上的键。启动那一刻网页还没加载，没法当场提示，由主窗口加载后来取。
#[tauri::command]
pub(crate) fn get_shortcut_conflicts(state: State<'_, AppState>) -> Vec<String> {
    state.shortcut_conflicts.lock().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(action: &str, accelerator: Option<&str>) -> ShortcutBinding {
        ShortcutBinding {
            action: action.into(),
            accelerator: accelerator.map(str::to_string),
        }
    }

    #[test]
    fn unbound_actions_pass_validation() {
        assert!(validate(&[binding("snapshot", None), binding("record", None)]).is_ok());
    }

    #[test]
    fn same_key_in_different_modifier_order_is_a_duplicate() {
        let result = validate(&[
            binding("snapshot", Some("Alt+Shift+2")),
            binding("record", Some("Shift+Alt+2")),
        ]);
        assert_eq!(result, Err("快捷键不能重复".to_string()));
    }

    #[test]
    fn unrecognized_key_is_rejected_with_its_text() {
        let result = validate(&[binding("snapshot", Some("Alt+NotAKey"))]);
        assert!(result.unwrap_err().contains("Alt+NotAKey"));
    }

    #[test]
    fn keys_recorded_by_the_settings_page_parse() {
        // 设置页录键时拼出的格式：修饰键 + normalizeKey 之后的主键
        for accelerator in [
            "CommandOrControl+Shift+S",
            "Alt+Shift+2",
            "CommandOrControl+Alt+F1",
            "Alt+Shift+Space",
        ] {
            assert!(
                accelerator.parse::<Shortcut>().is_ok(),
                "{accelerator} 应能解析"
            );
        }
    }
}
