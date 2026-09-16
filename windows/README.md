# 应用快照（Windows 版）

Windows 实现。全局快捷键截取应用窗口、悬浮球或桌宠、窗口录制、提示词润色。总览见 [README](../README.md)（英文）或 [简体中文](../README.zh-CN.md)。

运行要求：Windows 10 或 Windows 11。x64 与 ARM64 各有一份原生自包含 exe，不必安装 .NET。不支持 32 位 Windows。

## 功能

- **`Alt+Shift+2`**（默认，可改）：立即截取当前应用窗口并复制到剪贴板，成功时播放快门音效（内置合成，无需外部文件）
- **`Alt+Shift+R`**（默认，可改）：录制当前应用窗口为 MP4（带浮动控制条，开始 / 停止）
- **可选绑定**：截取上一个前台应用窗口、润色当前剪切板提示词——默认不绑定，在设置中自行决定
- **悬浮球**：圆形悬浮图标，显示上一个前台应用的图标，点击弹出操作面板，可拖动、位置持久化；右键打开「设置…」
- **桌宠模式**（可选）：设置里把桌面形式切为桌宠，GIF 形象替代悬浮球；可换肤、事件动画、拖拽跑动；素材不随应用分发
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
- **桌面形式**：悬浮窗或 GIF 桌宠，以及桌宠形象（素材缺失时拒绝切换，见下节）
- **润色服务（LLM Provider）**：协议 / Base URL / 模型名 / API Key（保存在 Windows 凭据管理器，输入框留空表示沿用已保存的 Key）
  - **OpenAI 兼容**：Base URL 填服务根地址，如 DeepSeek `https://api.deepseek.com`（自动拼 `/v1/chat/completions`；URL 已含 `/v1` 则只拼 `/chat/completions`），模型如 `deepseek-chat`
  - **Anthropic**：Base URL 填 `https://api.anthropic.com`（自动拼 `/v1/messages`），模型如 `claude-sonnet-4-5`，Key 为 Anthropic Console 的 API Key；请求自动携带 `x-api-key` 与 `anthropic-version: 2023-06-01` 头

润色请求参数：`max_tokens = 16384`、`temperature = 0.3`、超时 180 秒。

## 使用

1. 按芯片启动对应产物：x64 用 `dist\win-x64\AppSnapshot.exe`，ARM64 用 `dist\win-arm64\AppSnapshot.exe`。
2. 在应用之间切换，悬浮图标会显示上一个使用过的应用。
3. 按 `Alt+Shift+2` 或点击悬浮图标截取，在微信、文档或其他支持图片的输入框中按 `Ctrl+V` 粘贴。

## 构建

需要 [.NET 10 SDK](https://aka.ms/dotnet/download)。在 x64 机器上一次打出两个原生包（ARM64 为交叉编译，不需要 ARM 设备）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

| 产物 | 架构 |
|---|---|
| `dist\win-x64\AppSnapshot.exe` | 原生 x64（Intel / AMD） |
| `dist\win-arm64\AppSnapshot.exe` | 原生 ARM64（骁龙本等） |

均为自包含单文件，用户不必安装 .NET 运行时。不提供 32 位（`win-x86`）。构建脚本会读取 PE 头核对架构，切片不对会失败。

发行命名（GitHub Release 附件）再跑一次：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\package-release.ps1
```

会把两份 exe 复制到仓库 `dist\release\`，改名为 `Application-Snapshot-{版本}-windows-x64.exe` / `windows-arm64.exe`，并写一份仅含这两文件的 `SHA256SUMS.txt`。

代码签名：`.\sign.ps1` 默认会递归签署 `dist` 下两份 `AppSnapshot.exe`。签发后再跑 `package-release.ps1 -SkipBuild`，避免把未签名副本覆盖进去。

## 桌宠模式（可选）

初始桌面形式为悬浮窗；在「设置… → 桌面形式」中可切换为桌宠并选择形象，选择会写进设置。

桌宠素材**不随应用内置或分发**（素材并非本项目制作，避免版权问题），由用户自行放置：

- 目录：`%APPDATA%\AppSnapshot\pet\<形象>\`（子目录名即形象名，可放多套）
- 状态文件（源码加载）：`idle` / `waving` / `jumping` / `failed` / `waiting` / `running-left` / `running-right`（`.gif`）
- 目录为空或素材缺失时，设置中的桌宠选项会提示不可用，应用回退悬浮窗

桌宠交互：单击打开功能面板（与悬浮窗一致），拖动移动（面板跟随），右键菜单 = 设置 / 退出；截图与录制期间自动离场避镜。

## 代码签名（Smart App Control）

若系统启用了**智能应用控制（Smart App Control）**，未签名的 `AppSnapshot.exe` 会被代码完整性策略直接拒绝启动，报「应用程序控制策略已阻止此文件」。SAC 没有单程序白名单，也不提供「仍要运行」按钮，只能三选一：

1. 用代码签名证书签名产物
2. 在 Windows 安全中心 → 应用和浏览器控制 → 智能应用控制中关闭 SAC（Windows 11 24H2 / 25H2 起该开关可以重新打开，不再需要重装系统）
3. 在未启用 SAC 的机器或虚拟机里运行

### 用 `sign.ps1` 签名

```powershell
.\sign.ps1 -SelfTest                                    # 用一次性自签证书验证签名链路，自动清理
.\sign.ps1 -PfxPath C:\certs\codesign.pfx -PfxPassword YOUR_PFX_PASSWORD
.\sign.ps1 -Thumbprint <证书指纹>
```

`-SelfTest` 会临时创建一张自签证书、对 `dist` 内文件的副本签名并校验，随后删除证书和副本，不会改动真实产物。

### 签名要求

- **必须是 RSA**：SAC 的签名检查不支持 ECC
- 微软文档要求证书由 **Microsoft Trusted Root Program 内的 CA** 签发，自签名不在其列
- 建议始终启用 RFC 3161 时间戳（默认 `http://timestamp.digicert.com`），证书过期后签名依然有效

自签证书不能作为对外分发方案：SAC 是否放行会随云端信誉与策略变化，同一自签证书也可能前后结果不一致。本机调试可关 SAC 或只用 `-SelfTest` 验证签名链路；发给他人的 exe 需要 Trusted Root 内 CA 签发的 RSA 代码签名证书。出问题时看事件日志 `Microsoft-Windows-CodeIntegrity/Operational`。
