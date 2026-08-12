using System;
using System.Diagnostics;
using WindowSnap.Wpf.PInvoke;

namespace WindowSnap.Wpf.Services;

/// <summary>
/// 跟踪前台窗口变化，记录"上一个前台过的非自身 app"。
/// 对应 macOS 端 AppDelegate 里的 lastExternalApplication / previousExternalApplication。
/// </summary>
public sealed class TargetAppTracker : IDisposable
{
    public record TargetInfo(IntPtr Hwnd, uint ProcessId, string ProcessName, string Title);

    /// <summary>当前前台过的非自身 app（用于快捷键路径：截当前 app）。</summary>
    public TargetInfo? Current { get; private set; }

    /// <summary>上一个前台过的非自身 app（用于宠物路径：截上一个 app）。</summary>
    public TargetInfo? Previous { get; private set; }

    /// <summary>目标变化时触发。Payload 是 Previous（宠物要显示的那个）。</summary>
    public event EventHandler<TargetInfo?>? PreviousChanged;

    private User32.WinEventDelegate? _hookProc;
    private IntPtr _hook = IntPtr.Zero;
    private int _ownPid;

    public void Start()
    {
        using var proc = Process.GetCurrentProcess();
        _ownPid = proc.Id;

        _hookProc = OnForegroundChanged;
        _hook = User32.SetWinEventHook(
            User32.EVENT_SYSTEM_FOREGROUND,
            User32.EVENT_SYSTEM_FOREGROUND,
            IntPtr.Zero,
            _hookProc,
            0,
            0,
            User32.WINEVENT_OUTOFCONTEXT | User32.WINEVENT_SKIPOWNPROCESS);

        // 立即抓一次当前前台
        RecordCurrent();
    }

    /// <summary>热键触发时实时刷新一次前台窗口（对齐 macOS 版 frontmostApplication 的实时查询语义）。</summary>
    public void RefreshNow() => RecordCurrent();

    private void OnForegroundChanged(
        IntPtr hWinEventHook, uint evt, IntPtr hwnd, int idObject,
        int idChild, uint thread, uint time)
    {
        if (hwnd == IntPtr.Zero) return;
        RecordCurrent();
    }

    private void RecordCurrent()
    {
        var hwnd = User32.GetForegroundWindow();
        if (hwnd == IntPtr.Zero) return;

        User32.GetWindowThreadProcessId(hwnd, out uint pid);
        if (pid == 0 || pid == _ownPid) return;

        string name;
        try
        {
            using var p = Process.GetProcessById((int)pid);
            name = p.ProcessName;
        }
        catch
        {
            name = "未知应用";
        }

        var info = new TargetInfo(hwnd, pid, name, User32.GetWindowTitle(hwnd));
        if (Current != null && info.ProcessId != Current.ProcessId)
        {
            Previous = Current;
            PreviousChanged?.Invoke(this, Previous);
        }
        Current = info;
    }

    public void ClearTerminated(uint pid)
    {
        var changed = false;
        if (Previous?.ProcessId == pid) { Previous = null; changed = true; }
        if (Current?.ProcessId == pid) { Current = null; changed = true; }
        if (changed) PreviousChanged?.Invoke(this, Previous);
    }

    public void Dispose()
    {
        if (_hook != IntPtr.Zero)
        {
            User32.UnhookWinEvent(_hook);
            _hook = IntPtr.Zero;
        }
        _hookProc = null;
    }
}
