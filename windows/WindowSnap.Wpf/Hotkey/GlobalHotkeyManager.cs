using System;
using System.Windows.Interop;
using WindowSnap.Wpf.PInvoke;
using WindowSnap.Wpf.Services;

namespace WindowSnap.Wpf.Hotkey;

/// <summary>
/// 用 Win32 RegisterHotKey 注册全局热键。需要把消息泵子接到一个隐藏窗口上。
/// 通过 HwndSource.AddHook 拦截 WM_HOTKEY。
/// </summary>
public sealed class GlobalHotkeyManager : IDisposable
{
    private const string HotkeyId = "WindowSnap.Hotkey";
    private static int _nextId = 0xC000;

    private readonly HwndSource _source;
    private readonly HwndSourceHook _hook;
    private int _currentId = -1;
    private Action? _action;
    private ShortcutConfiguration _current;

    public GlobalHotkeyManager(HwndSource source)
    {
        _source = source;
        _hook = WndProc;
        _source.AddHook(_hook);
    }

    public bool Register(ShortcutConfiguration configuration, Action action)
    {
        Unregister();
        var id = _nextId++;
        var modifiers = (uint)configuration.Modifiers | User32.MOD_NOREPEAT;
        if (!User32.RegisterHotKey(_source.Handle, id, modifiers, configuration.VirtualKey))
        {
            return false;
        }
        _currentId = id;
        _action = action;
        _current = configuration;
        return true;
    }

    public void Unregister()
    {
        if (_currentId != -1)
        {
            User32.UnregisterHotKey(_source.Handle, _currentId);
            _currentId = -1;
        }
        _action = null;
    }

    private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (msg == User32.WM_HOTKEY && wParam.ToInt32() == _currentId)
        {
            _action?.Invoke();
            handled = true;
        }
        return IntPtr.Zero;
    }

    public void Dispose()
    {
        Unregister();
        _source.RemoveHook(_hook);
    }
}
