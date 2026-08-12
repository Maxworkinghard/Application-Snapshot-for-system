using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using WindowSnap.Wpf.Capture;
using WindowSnap.Wpf.Hotkey;
using WindowSnap.Wpf.Pet;
using WindowSnap.Wpf.Services;
using WindowSnap.Wpf.Settings;
using WindowSnap.Wpf.Toast;
using WindowSnap.Wpf.Tray;
using WinForms = System.Windows.Forms;

namespace WindowSnap.Wpf;

/// <summary>
/// 协调核心：托盘 + 快捷键 + 截图服务 + 宠物 + Toast。
/// 对应 macOS 端 AppDelegate.swift。
/// </summary>
public sealed class AppDelegate : IDisposable
{
    private readonly WindowCaptureService _captureService = new();
    private readonly ShortcutStore _shortcutStore = new();
    private readonly TargetAppTracker _tracker = new();
    private readonly ClipboardAutoClearService _autoClear = new();
    public ClipboardAutoClearService AutoClearService => _autoClear;

    private TrayIconManager? _tray;
    private GlobalHotkeyManager? _hotkey;
    private ToastWindow? _toast;
    private DesktopPetWindow? _pet;
    private ShortcutSettingsWindow? _settings;
    private HiddenMessageWindow? _msgWindow;

    private ShortcutConfiguration _shortcutConfig;
    private bool _isCapturing;

    public void Start()
    {
        _toast = new ToastWindow();
        _tracker.PreviousChanged += OnPreviousChanged;
        _tracker.Start();

        _shortcutConfig = _shortcutStore.Load();

        // 建一个隐藏消息窗口作为热键宿主 + 剪贴板监听挂载点
        _msgWindow = new HiddenMessageWindow();
        _msgWindow.Show();
        _hotkey = new GlobalHotkeyManager(((HwndSource)PresentationSource.FromVisual(_msgWindow)));

        _tray = new TrayIconManager(
            captureFrontWindow: CaptureFrontWindow,
            chooseWindow: ChooseWindow,
            openShortcutSettings: OpenShortcutSettings,
            openScreenCaptureSettings: OpenScreenCaptureSettings,
            quit: Quit);

        if (!RegisterShortcut(_shortcutConfig))
        {
            _shortcutConfig = ShortcutConfiguration.Default;
            _shortcutStore.Save(_shortcutConfig);
            _tray.UpdateCaptureShortcutLabel(_shortcutConfig.DisplayString);

            if (!RegisterShortcut(_shortcutConfig))
                ShowToast("快捷键注册失败，请从托盘截取", "!");
            else
                ShowToast("原快捷键被占用，已恢复默认快捷键", "!");
        }
        else
        {
            _tray.UpdateCaptureShortcutLabel(_shortcutConfig.DisplayString);
        }

        _pet = new DesktopPetWindow(
            onRequestTarget: () => _tracker.Previous,
            onCapture: target => CaptureWindow(target?.Hwnd ?? IntPtr.Zero, target?.ProcessName ?? "应用", target?.Title ?? ""));
        _pet.Show();

        // 首次启动也刷新一次宠物
        OnPreviousChanged(null, _tracker.Previous);
    }

    private void CaptureFrontWindow()
    {
        _tracker.RefreshNow();
        CaptureWindow(_tracker.Current?.Hwnd ?? IntPtr.Zero, _tracker.Current?.ProcessName ?? "应用", _tracker.Current?.Title ?? "");
    }

    private async void CaptureWindow(IntPtr hwnd, string appName, string title)
    {
        if (_isCapturing) return;
        _isCapturing = true;
        try
        {
            if (hwnd == IntPtr.Zero)
            {
                ShowToast("没有找到可截取的应用窗口", "!");
                return;
            }
            var result = await _captureService.CaptureAsync(hwnd, appName, title);
            ShowToast($"已复制 {result.ApplicationName} 窗口，60 秒后自动清空", "✓");
        }
        catch (Exception ex)
        {
            ShowToast(ex.Message, "!");
        }
        finally
        {
            _isCapturing = false;
        }
    }

    private void ChooseWindow()
    {
        // 简化：调用 Windows 自带 Snipping Tool / 或弹提示让用户用 PrintScreen
        // 完整实现需要写一个 picker；先做最小可用：打开 Snipping Tool
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = "ms-screenclip:",
                UseShellExecute = true
            });
        }
        catch { }
    }

    private void OpenShortcutSettings()
    {
        if (_settings != null) { _settings.Activate(); return; }
        _settings = new ShortcutSettingsWindow(_shortcutConfig, ApplyShortcut);
        _settings.Closed += (_, _) => _settings = null;
        _settings.Show();
    }

    private void OpenScreenCaptureSettings()
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "ms-settings:privacy-graphicscaptureprograms",
                UseShellExecute = true
            });
        }
        catch { }
    }

    private void Quit() => Application.Current.Shutdown();

    private bool RegisterShortcut(ShortcutConfiguration config)
    {
        _hotkey?.Unregister();
        return _hotkey?.Register(config, CaptureFrontWindow) ?? false;
    }

    private bool ApplyShortcut(ShortcutConfiguration config)
    {
        if (config == _shortcutConfig) return true;
        if (!RegisterShortcut(config)) return false;
        _shortcutConfig = config;
        _shortcutStore.Save(config);
        _tray?.UpdateCaptureShortcutLabel(config.DisplayString);
        ShowToast($"快捷键已设为 {config.DisplayString}", "✓");
        return true;
    }

    private void OnPreviousChanged(object? sender, TargetAppTracker.TargetInfo? target)
    {
        _pet?.Dispatcher.BeginInvoke(new Action(() => _pet?.UpdateTarget(target)));
    }

    private void ShowToast(string message, string symbol)
    {
        _toast?.Dispatcher.BeginInvoke(new Action(() => _toast?.ShowToast(message, symbol)));
    }

    public void Shutdown()
    {
        Dispose();
    }

    public void Dispose()
    {
        _tracker.Dispose();
        _hotkey?.Dispose();
        _tray?.Dispose();
        _autoClear.Dispose();
        _msgWindow?.Close();
    }
}

internal sealed class HiddenMessageWindow : Window
{
    public HiddenMessageWindow()
    {
        Width = 0;
        Height = 0;
        WindowStyle = WindowStyle.None;
        ShowInTaskbar = false;
        Visibility = Visibility.Hidden;
        AllowsTransparency = true;
        WindowState = WindowState.Normal;
    }
}
