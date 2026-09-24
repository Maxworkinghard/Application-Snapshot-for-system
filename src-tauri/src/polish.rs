use super::settings::{keyring_entry, DEFAULT_PROMPT};
use super::*;
use regex::Regex;

/// 待润色草稿的长度上限（字符）
const POLISH_INPUT_LIMIT: usize = 12_000;

fn normalize_draft(raw: &str, empty_hint: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(empty_hint.into());
    }
    if trimmed.chars().count() > POLISH_INPUT_LIMIT {
        return Err(format!("内容过长（上限 {POLISH_INPUT_LIMIT} 字）"));
    }
    Ok(trimmed.to_string())
}

/// 真正干活的那一段：拿设置 + 草稿，调模型，返回改写后的文本。
/// 剪贴板命令和界面命令都走这里，避免两份实现漂移。
async fn run_polish(
    state: &State<'_, AppState>,
    original: &str,
    template_id: Option<&str>,
) -> Result<(String, String), String> {
    let settings = state.settings.lock().clone();
    if settings.base_url.is_empty() || settings.model.is_empty() {
        return Err("尚未配置润色服务，请先在 Prompt 页面完成配置".into());
    }
    let api_key = if settings.has_api_key {
        Some(
            keyring_entry()?
                .get_password()
                .map_err(|_| "无法读取已保存的 API Key".to_string())?,
        )
    } else {
        None
    };
    // 输入框里按 Tab 换规则时指定模板；不指定就用当前生效的那条
    let wanted = template_id.unwrap_or(&settings.active_template_id);
    let template = settings
        .templates
        .iter()
        .find(|item| item.id == wanted)
        .or_else(|| {
            settings
                .templates
                .iter()
                .find(|item| item.id == settings.active_template_id)
        });
    // 刚新建、还没写正文的规则按内置规则润色，别把空的系统提示发出去
    let prompt = template
        .map(|item| item.content.as_str())
        .filter(|content| !content.trim().is_empty())
        .unwrap_or(DEFAULT_PROMPT);
    let rule_name = template
        .map(|item| item.name.clone())
        .unwrap_or_else(|| "润色".into());
    let endpoint = make_endpoint(&settings.base_url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.post(endpoint);
    if let Some(api_key) = api_key {
        request = request.bearer_auth(api_key);
    }
    request = request.json(&json!({ "model": settings.model, "max_tokens": 16384, "temperature": 0.3, "messages": [{ "role": "system", "content": prompt }, { "role": "user", "content": original }] }));
    let response = request
        .send()
        .await
        .map_err(|error| format!("润色请求失败：{error}"))?;
    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("润色服务响应无效：{error}"))?;
    if !status.is_success() {
        return Err(format!(
            "润色服务返回错误（{}）：{}",
            status.as_u16(),
            truncate(&payload.to_string(), 240)
        ));
    }
    let polished = payload
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| "润色服务未返回内容".to_string())?;
    Ok((strip_reasoning(polished), rule_name))
}

/// 成功与失败都记一条活动，时间线和猫的播报靠它
fn log_polish(app: &AppHandle, original: &str, result: &Result<(String, String), String>) {
    match result {
        Ok((polished, rule)) => activity::record(
            app,
            activity::Activity::new("polish", rule.clone())
                .meta(format!(
                    "{} → {} 字",
                    original.chars().count(),
                    polished.chars().count()
                ))
                .detail(truncate(polished, 4000)),
        ),
        Err(error) => activity::record_error(app, "润色失败", error),
    }
}

/// 剥掉 R1 一类推理模型吐出的思维链
fn strip_reasoning(raw: &str) -> String {
    Regex::new(r"(?s)<think>.*?</think>")
        .unwrap()
        .replace_all(raw, "")
        .trim()
        .to_string()
}

/// 界面用：收草稿、回改写结果，不碰剪贴板
#[tauri::command]
pub(crate) async fn polish_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    template_id: Option<String>,
) -> Result<String, String> {
    let original = normalize_draft(&text, "请先输入待润色的草稿")?;
    let result = run_polish(&state, &original, template_id.as_deref()).await;
    log_polish(&app, &original, &result);
    result.map(|(polished, _)| polished)
}

/// 快捷键与便携坞用：就地替换剪贴板
#[tauri::command]
pub(crate) async fn polish_clipboard(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let raw = Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .map_err(|_| "剪贴板没有文字，请先复制 Prompt".to_string())?;
    let original = normalize_draft(&raw, "剪贴板没有文字，请先复制 Prompt")?;
    let result = run_polish(&state, &original, None).await;
    log_polish(&app, &original, &result);
    let (polished, _) = result?;
    let current = Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .unwrap_or_default();
    if current.trim() != original {
        return Err("剪贴板内容已变化，润色结果未写入".into());
    }
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(polished))
        .map_err(|error| format!("写入剪贴板失败：{error}"))?;
    Ok("润色完成，结果已替换剪贴板".into())
}

fn make_endpoint(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim_end_matches('/');
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("润色服务 Base URL 无效".into());
    }
    if trimmed.ends_with("/chat/completions") {
        return Ok(trimmed.into());
    }
    let version_suffix = Regex::new(r"/v\d+$").unwrap().is_match(trimmed);
    let path = if version_suffix {
        "chat/completions"
    } else {
        "v1/chat/completions"
    };
    Ok(format!("{trimmed}/{path}"))
}
