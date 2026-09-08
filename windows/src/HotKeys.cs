using System;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 全局快捷键（RegisterHotKey）：Alt+Shift+2 截取，Alt+Shift+R 录制开关。
    /// 通过消息专用窗口接收 WM_HOTKEY，对应 macOS 端的 GlobalHotKey。
    /// </summary>
    internal sealed class HotKeyManager : NativeWindow, IDisposable
    {
        private const int HotKeyIdCapture = 1;
        private const int HotKeyIdRecord = 2;

        internal event Action CapturePressed;
        internal event Action RecordPressed;

        internal HotKeyManager()
        {
            var parameters = new CreateParams
            {
                Caption = "AppSnapshotHotKeys",
                Parent = new IntPtr(-3) // HWND_MESSAGE
            };
            CreateHandle(parameters);
        }

        internal void RegisterAll()
        {
            bool captureOk = NativeMethods.RegisterHotKey(
                Handle, HotKeyIdCapture,
                NativeMethods.ModAlt | NativeMethods.ModShift | NativeMethods.ModNoRepeat, 0x32 /* '2' */);

            bool recordOk = NativeMethods.RegisterHotKey(
                Handle, HotKeyIdRecord,
                NativeMethods.ModAlt | NativeMethods.ModShift | NativeMethods.ModNoRepeat, 0x52 /* 'R' */);

            if (!captureOk)
            {
                App.Toast.Show("快捷键 Alt+Shift+2 注册失败，请从托盘菜单截取", ToastKind.Warning);
            }
            if (!recordOk)
            {
                App.Toast.Show("快捷键 Alt+Shift+R 注册失败，请从托盘菜单录制", ToastKind.Warning);
            }
        }

        protected override void WndProc(ref Message message)
        {
            if (message.Msg == NativeMethods.WmHotkey)
            {
                int id = message.WParam.ToInt32();
                if (id == HotKeyIdCapture)
                {
                    Action handler = CapturePressed;
                    if (handler != null)
                    {
                        handler();
                    }
                }
                else if (id == HotKeyIdRecord)
                {
                    Action handler = RecordPressed;
                    if (handler != null)
                    {
                        handler();
                    }
                }
            }
            base.WndProc(ref message);
        }

        public void Dispose()
        {
            NativeMethods.UnregisterHotKey(Handle, HotKeyIdCapture);
            NativeMethods.UnregisterHotKey(Handle, HotKeyIdRecord);
            ReleaseHandle();
        }
    }
}
