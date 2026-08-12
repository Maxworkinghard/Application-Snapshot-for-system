using System;
using System.Windows;
using H.NotifyIcon;

namespace WindowSnap.Wpf.Tray;

/// <summary>
/// 系统托盘图标 + 右键菜单。
/// </summary>
public sealed class TrayIconManager : IDisposable
{
    private readonly TaskbarIcon _icon;
    private readonly Action _captureFrontWindow;
    private readonly Action _chooseWindow;
    private readonly Action _openShortcutSettings;
    private readonly Action _openScreenCaptureSettings;
    private readonly Action _quit;

    public TaskbarIcon Icon => _icon;

    public TrayIconManager(
        Action captureFrontWindow,
        Action chooseWindow,
        Action openShortcutSettings,
        Action openScreenCaptureSettings,
        Action quit)
    {
        _captureFrontWindow = captureFrontWindow;
        _chooseWindow = chooseWindow;
        _openShortcutSettings = openShortcutSettings;
        _openScreenCaptureSettings = openScreenCaptureSettings;
        _quit = quit;

        _icon = new TaskbarIcon
        {
            ToolTipText = "应用快照",
            Visibility = Visibility.Visible,
        };
        try
        {
            _icon.IconSource = new System.Windows.Media.Imaging.BitmapImage(
                new Uri("pack://application:,,,/Resources/camera-viewfinder.ico",
                       UriKind.RelativeOrAbsolute));
        }
        catch
        {
            // 图标资源缺失时保持默认外观，不崩溃
        }
        _icon.TrayLeftMouseUp += (_, _) => _captureFrontWindow();
        BuildMenu();
    }

    private void BuildMenu()
    {
        var menu = new System.Windows.Controls.ContextMenu();

        var captureItem = new System.Windows.Controls.MenuItem
        {
            Header = "截取当前应用窗口",
            InputGestureText = "Alt+Shift+2"
        };
        captureItem.Click += (_, _) => _captureFrontWindow();
        menu.Items.Add(captureItem);

        var chooseItem = new System.Windows.Controls.MenuItem { Header = "选择其他窗口…" };
        chooseItem.Click += (_, _) => _chooseWindow();
        menu.Items.Add(chooseItem);

        menu.Items.Add(new System.Windows.Controls.Separator());

        var shortcutItem = new System.Windows.Controls.MenuItem { Header = "设置快捷键…" };
        shortcutItem.Click += (_, _) => _openShortcutSettings();
        menu.Items.Add(shortcutItem);

        var settingsItem = new System.Windows.Controls.MenuItem { Header = "屏幕捕获设置…" };
        settingsItem.Click += (_, _) => _openScreenCaptureSettings();
        menu.Items.Add(settingsItem);

        menu.Items.Add(new System.Windows.Controls.Separator());

        var quitItem = new System.Windows.Controls.MenuItem { Header = "退出应用快照" };
        quitItem.Click += (_, _) => _quit();
        menu.Items.Add(quitItem);

        _icon.ContextMenu = menu;
    }

    public void UpdateCaptureShortcutLabel(string text)
    {
        if (_icon.ContextMenu?.Items[0] is System.Windows.Controls.MenuItem mi)
            mi.InputGestureText = text;
    }

    public void Dispose()
    {
        _icon.Dispose();
    }
}
