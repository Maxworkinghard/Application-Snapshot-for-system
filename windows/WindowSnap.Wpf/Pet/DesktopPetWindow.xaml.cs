using System;
using System.Drawing;
using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media.Imaging;
using System.Windows.Media;
using WindowSnap.Wpf.Services;

namespace WindowSnap.Wpf.Pet;

/// <summary>
/// 桌面悬浮小宠物。圆形悬浮窗，显示上一个前台 app 的图标；
/// 点击弹出气泡 + 截取按钮；可拖动，位置持久化。
/// 注意：Left/Top 等 WPF 坐标一律用 DIP（SystemParameters），不能混用物理像素。
/// </summary>
public partial class DesktopPetWindow : Window
{
    private readonly Action<TargetAppTracker.TargetInfo?> _onCapture;
    private readonly Func<TargetAppTracker.TargetInfo?> _onRequestTarget;
    private TargetAppTracker.TargetInfo? _target;
    private const double DragThreshold = 4.0;

    public DesktopPetWindow(
        Func<TargetAppTracker.TargetInfo?> onRequestTarget,
        Action<TargetAppTracker.TargetInfo?> onCapture)
    {
        _onRequestTarget = onRequestTarget;
        _onCapture = onCapture;
        InitializeComponent();
        PositionFromSettings();
        UpdateTarget(_onRequestTarget());
    }

    public void UpdateTarget(TargetAppTracker.TargetInfo? target)
    {
        _target = target;
        if (target != null)
        {
            TargetName.Text = target.ProcessName;
            CaptureButton.Content = $"截取 {target.ProcessName} 窗口";
            CaptureButton.IsEnabled = true;
            IconImage.Source = TryGetAppIcon(target) ?? FallbackIcon();
        }
        else
        {
            TargetName.Text = "未识别到上一个应用";
            CaptureButton.Content = "截取窗口";
            CaptureButton.IsEnabled = false;
            IconImage.Source = FallbackIcon();
        }
    }

    private BitmapSource? TryGetAppIcon(TargetAppTracker.TargetInfo target)
    {
        try
        {
            using var process = System.Diagnostics.Process.GetProcessById((int)target.ProcessId);
            var exePath = process.MainModule?.FileName;
            if (exePath == null) return null;
            using var icon = System.Drawing.Icon.ExtractAssociatedIcon(exePath);
            if (icon == null) return null;
            return Imaging.CreateBitmapSourceFromHIcon(
                icon.Handle, Int32Rect.Empty, BitmapSizeOptions.FromEmptyOptions());
        }
        catch { return null; }
    }

    private static BitmapSource? FallbackIcon()
    {
        try
        {
            return new BitmapImage(new Uri(
                "pack://application:,,,/Resources/camera-viewfinder.ico",
                UriKind.RelativeOrAbsolute));
        }
        catch
        {
            // 图标资源缺失时显示空圆形，不崩溃
            return null;
        }
    }

    private void PositionFromSettings()
    {
        var s = Properties.Settings.Default;
        var x = s.Get("pet.position.x", double.MinValue);
        var y = s.Get("pet.position.y", double.MinValue);
        if (x == double.MinValue || y == double.MinValue)
        {
            DefaultPosition();
            return;
        }
        // 离屏回退：虚拟屏幕范围 ±200 DIP 容差，对应 macOS 版逻辑
        var left = SystemParameters.VirtualScreenLeft - 200;
        var top = SystemParameters.VirtualScreenTop - 200;
        var right = SystemParameters.VirtualScreenLeft + SystemParameters.VirtualScreenWidth + 200;
        var bottom = SystemParameters.VirtualScreenTop + SystemParameters.VirtualScreenHeight + 200;
        if (x < left || x > right || y < top || y > bottom)
        {
            DefaultPosition();
            return;
        }
        Left = x;
        Top = y;
    }

    private void DefaultPosition()
    {
        var work = SystemParameters.WorkArea; // 主屏工作区，DIP
        Left = work.Right - Width - 16;
        Top = work.Bottom - Height - 16;
    }

    protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonDown(e);
        var beforeLeft = Left;
        var beforeTop = Top;
        try
        {
            DragMove(); // 同步阻塞到松开鼠标；纯点击时立即返回
        }
        catch (InvalidOperationException)
        {
            return;
        }

        if (Math.Abs(Left - beforeLeft) > DragThreshold || Math.Abs(Top - beforeTop) > DragThreshold)
        {
            var s = Properties.Settings.Default;
            s.Set("pet.position.x", Left);
            s.Set("pet.position.y", Top);
        }
        else
        {
            ActionPopup.PlacementTarget = PetBorder;
            ActionPopup.IsOpen = true;
        }
    }

    private void OnCaptureClick(object sender, RoutedEventArgs e)
    {
        ActionPopup.IsOpen = false;
        _onCapture?.Invoke(_onRequestTarget());
    }
}
