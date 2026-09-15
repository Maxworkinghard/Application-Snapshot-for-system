# 应用快照（Windows 版）

macOS 版的 Windows 移植。常驻后台，功能与 macOS 端对齐：全局快捷键截取应用窗口、悬浮球、窗口录制、提示词润色。

## 功能

- **`Alt+Shift+2`**（默认，可改）：立即截取当前应用窗口并复制到剪贴板，成功时播放快门音效（内置合成，无需外部文件）
- **`Alt+Shift+R`**（默认，可改）：录制当前应用窗口为 MP4（带浮动控制条，开始 / 停止）
- **可选绑定**：截取上一个前台应用窗口、润色当前剪切板提示词——默认不绑定，在设置中自行决定
- **悬浮球**：圆形悬浮图标，显示上一个前台应用的图标，点击弹出操作面板，可拖动、位置持久化；右键打开「设置…」
- **操作面板**（点击悬浮球）：
  - 应用快照：列出所有可截取窗口（带图标），点击即截取该窗口
  - 录制当前应用窗口 / 停止录制
  - 润色 Prompt / 停止润色
- **润色 Prompt**：读取剪贴板中的文字草稿 → 系统确认弹窗 → 大模型改写 → 结果写回剪贴板（替换原文），处理中可停止
- **托盘菜单**：截取 / 录制 / 选择其他窗口 / 设置保存目录 / 设置 / 退出
- **Toast 提示**：截图、录制、润色各阶段的结果反馈
- **剪贴板 60 秒自动清空**，期间复制过其他内容则跳过

截图仅进入剪贴板，**不落盘、不上传**；截图优先通过 `PrintWindow` 获取独立窗口内容，即使目标被其他普通窗口遮挡也会尝试完整捕获，不支持该接口的 GPU/浏览器窗口会自动回退到屏幕像素截图。

## 设置

右键悬浮球或托盘菜单 `设置…` 打开统一设置窗口，保存后立即生效：

- **快捷键**：点击组合键框后按下新组合即可记录，「×」清除绑定（留空即不启用）；被其他应用占用的组合会提示并保留原快捷键
- **润色提示词**：内置改写规则常驻；可新建/编辑/删除自定义提示词并随时切换（切换即生效，不替换内置）；选「内置」点「编辑」可基于内置文本另存自定义版本，自定义提示词存 `%APPDATA%\AppSnapshot\prompts.json`
- **润色服务（LLM Provider）**：协议 / Base URL / 模型名 / API Key（保存在 Windows 凭据管理器，输入框留空表示沿用已保存的 Key）
  - **OpenAI 兼容**：Base URL 填服务根地址，如 DeepSeek `https://api.deepseek.com`（自动拼 `/v1/chat/completions`；URL 已含 `/v1` 则只拼 `/chat/completions`），模型如 `deepseek-chat`
  - **Anthropic**：Base URL 填 `https://api.anthropic.com`（自动拼 `/v1/messages`），模型如 `claude-sonnet-4-5`，Key 为 Anthropic Console 的 API Key；请求自动携带 `x-api-key` 与 `anthropic-version: 2023-06-01` 头

参数与 macOS / Linux 端一致：`max_tokens = 16384`、`temperature = 0.3`、超时 180 秒。

## 使用

1. 启动 `dist\AppSnapshot.exe`。
2. 在应用之间切换，悬浮图标会显示上一个使用过的应用。
3. 按 `Alt+Shift+2` 或点击悬浮图标截取，在微信、文档或其他支持图片的输入框中按 `Ctrl+V` 粘贴。

## 构建

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

输出文件：`dist\AppSnapshot.exe`。程序使用 Windows 自带的 .NET Framework 4.0+ 和 WinForms，不需要 Electron 或额外依赖。

## 代码签名（Smart App Control）

若系统启用了**智能应用控制（Smart App Control）**，未签名的 `AppSnapshot.exe` 会被代码完整性策略直接拒绝启动，报「应用程序控制策略已阻止此文件」。SAC 没有单程序白名单，也不提供「仍要运行」按钮，只能三选一：

1. 用代码签名证书签名产物
2. 在 Windows 安全中心 → 应用和浏览器控制 → 智能应用控制中关闭 SAC（Windows 11 24H2 / 25H2 起该开关可以重新打开，不再需要重装系统）
3. 在未启用 SAC 的机器或虚拟机里运行

### 用 `sign.ps1` 签名

```powershell
.\sign.ps1 -SelfTest                                    # 用一次性自签证书验证签名链路，自动清理
.\sign.ps1 -PfxPath C:\certs\codesign.pfx -PfxPassword ***
.\sign.ps1 -Thumbprint <证书指纹>
```

`-SelfTest` 会临时创建一张自签证书、对 `dist` 内文件的副本签名并校验，随后删除证书和副本，不会改动真实产物。

### 签名要求

- **必须是 RSA**：SAC 的签名检查不支持 ECC
- 微软文档要求证书由 **Microsoft Trusted Root Program 内的 CA** 签发，自签名不在其列
- 建议始终启用 RFC 3161 时间戳（默认 `http://timestamp.digicert.com`），证书过期后签名依然有效

### 实测记录（2026-09-14，Windows 11 专业版 25H2，Build 26200，SAC 强制模式）

| 对象 | 结果 |
|---|---|
| 未签名 `AppSnapshot.exe` | 被拒绝，事件 ID 3077 / 3118，策略 ID `{0283ac0f-fff1-49ae-ada1-8a933130cad6}` |
| 用自签证书签名后的同一文件 | **正常启动并常驻运行**，无拦截事件 |

即：在该构建上**自签名即可通过 SAC**，与微软文档不符。因此本仓库当前用自签证书签名 `dist\AppSnapshot.exe`（证书 `CN=AppSnapshot Self-Signed Publisher`，仅存在于本机当前用户证书存储）。

两点提醒：

- 这是该构建的实测行为，可能在 Code Integrity 策略刷新后改变——真出问题时先看事件日志 `Microsoft-Windows-CodeIntegrity/Operational`
- 自签证书只在签名它的那台机器上有效；**要分发给他人的产物，仍需向受信任 CA 申请代码签名证书**
