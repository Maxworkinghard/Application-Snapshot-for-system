# 应用快照

一个轻量的 Windows 悬浮窗口截图工具，交互参考 [Maxworkinghard/quick](https://github.com/Maxworkinghard/quick) 的 macOS「应用快照」。悬浮圆形图标显示上一个前台应用，点击后截取该应用窗口并自动复制到 Windows 剪贴板。

## 使用

1. 启动 `dist\AppSnapshot.exe`。
2. 在应用之间切换，悬浮图标会显示上一个使用过的应用。
3. 点击悬浮图标，截取该应用窗口。
4. 在微信、文档或其他支持图片的输入框中按 `Ctrl+V` 粘贴。

截图仅进入剪贴板，**不会保存到桌面或其他目录，也不会上传**。60 秒后，如果剪贴板仍然是本次截图，程序会自动清空；如果期间复制了其他内容，则不会清除新内容。

悬浮图标默认位于右下角，可拖动调整位置；右键图标可退出。

截图优先通过 `PrintWindow` 获取独立窗口内容，即使目标被其他普通窗口遮挡也会尝试完整捕获；不支持该接口的 GPU/浏览器窗口会自动回退到屏幕像素截图。

## 构建

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

输出文件：`dist\AppSnapshot.exe`。程序使用 Windows 自带的 .NET Framework 4.0+ 和 WinForms，不需要 Electron 或额外依赖。
