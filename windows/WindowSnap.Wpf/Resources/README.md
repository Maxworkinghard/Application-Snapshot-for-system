# Resources

这里的图标资源需要在 Windows 上构建时补全：

- `camera-viewfinder.ico` — 托盘图标 + 宠物 fallback 图标，建议 256x256 多尺寸 .ico
  - 可以从 macOS 版的 SF Symbol "camera.viewfinder" 截图转换
  - 或者直接用一个相机/快门的 PNG 转换为 .ico（在线工具即可）
- 应用图标 `WindowSnap.ico`（可选，作为发布版的 EXE 图标）

构建前放进去即可，缺资源时 WPF 会用默认图标。
