using System;
using System.Windows;
using System.Windows.Threading;

namespace WindowSnap.Wpf.Toast;

/// <summary>
/// 右上角 HUD 提示，1.8 秒后自动关闭。对应 macOS 端 ToastController。
/// </summary>
public partial class ToastWindow : Window
{
    private DispatcherTimer? _timer;

    public ToastWindow()
    {
        InitializeComponent();
    }

    public void ShowToast(string message, string symbol)
    {
        Message.Text = message;
        Symbol.Text = symbol;
        PositionTopRight();
        Show();
        _timer?.Stop();
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1.8) };
        _timer.Tick += (_, _) =>
        {
            Hide();
            _timer.Stop();
        };
        _timer.Start();
    }

    private void PositionTopRight()
    {
        var work = SystemParameters.WorkArea; // 主屏工作区，DIP 单位，别用 WinForms 的物理像素
        Left = work.Right - Width - 16;
        Top = work.Top + 16;
    }
}
