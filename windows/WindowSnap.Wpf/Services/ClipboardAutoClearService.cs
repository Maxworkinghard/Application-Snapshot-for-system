using System;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;

namespace WindowSnap.Wpf.Services;

/// <summary>
/// 复刻 macOS 端 scheduleClipboardAutoClear：写入后 60 秒自动清空，
/// 期间剪贴板被改写过（用户复制了别的）则跳过。
/// 用 Win32 GetClipboardSequenceNumber 对标 NSPasteboard.changeCount，无需消息监听。
/// </summary>
public sealed class ClipboardAutoClearService : IDisposable
{
    [DllImport("user32.dll")]
    private static extern uint GetClipboardSequenceNumber();

    private readonly TimeSpan _interval = TimeSpan.FromSeconds(60);
    private DispatcherTimer? _timer;
    private uint _seqAtWrite;

    /// <summary>写入剪贴板后（UI 线程）调用，启动/重置 60 秒倒计时。</summary>
    public void OnWrite()
    {
        _seqAtWrite = GetClipboardSequenceNumber();
        if (_timer == null)
        {
            _timer = new DispatcherTimer { Interval = _interval };
            _timer.Tick += OnTick;
        }
        _timer.Stop();
        _timer.Start();
    }

    private void OnTick(object? sender, EventArgs e)
    {
        _timer?.Stop();
        // 仅当剪贴板自上次写入后未被覆盖时清空，避免抹掉用户后续复制的东西
        if (GetClipboardSequenceNumber() != _seqAtWrite) return;
        try { Clipboard.Clear(); }
        catch { }
    }

    public void Dispose() => _timer?.Stop();
}
