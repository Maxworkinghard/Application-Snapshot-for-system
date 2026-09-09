using System;
using System.Collections.Generic;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 快捷键组合：修饰键 + 虚拟键码，可与文本（如 "Alt+Shift+2"）互转。
    /// null 表示未绑定（用户自行决定是否启用）。
    /// </summary>
    internal sealed class HotKeySpec
    {
        internal uint Modifiers;
        internal uint Vk;

        internal string DisplayName
        {
            get
            {
                var parts = new List<string>();
                if ((Modifiers & NativeMethods.ModAlt) != 0)
                {
                    parts.Add("Alt");
                }
                if ((Modifiers & NativeMethods.ModControl) != 0)
                {
                    parts.Add("Ctrl");
                }
                if ((Modifiers & NativeMethods.ModShift) != 0)
                {
                    parts.Add("Shift");
                }
                if ((Modifiers & NativeMethods.ModWin) != 0)
                {
                    parts.Add("Win");
                }
                parts.Add(VkToName(Vk));
                return string.Join("+", parts.ToArray());
            }
        }

        internal static HotKeySpec Create(uint modifiers, uint vk)
        {
            return new HotKeySpec { Modifiers = modifiers, Vk = vk };
        }

        /// <summary>解析 "Alt+Shift+2" 之类的文本；格式非法返回 null。</summary>
        internal static HotKeySpec Parse(string text)
        {
            if (string.IsNullOrEmpty(text))
            {
                return null;
            }
            uint modifiers = 0;
            uint vk = 0;
            foreach (string raw in text.Split('+'))
            {
                string part = raw.Trim();
                if (part.Length == 0)
                {
                    continue;
                }
                switch (part.ToLowerInvariant())
                {
                    case "ctrl":
                    case "control":
                        modifiers |= NativeMethods.ModControl;
                        continue;
                    case "alt":
                        modifiers |= NativeMethods.ModAlt;
                        continue;
                    case "shift":
                        modifiers |= NativeMethods.ModShift;
                        continue;
                    case "win":
                    case "super":
                        modifiers |= NativeMethods.ModWin;
                        continue;
                }
                uint? key = NameToVk(part);
                if (key == null || vk != 0)
                {
                    return null;
                }
                vk = key.Value;
            }
            if (modifiers == 0 || vk == 0)
            {
                return null;
            }
            return Create(modifiers, vk);
        }

        private static string VkToName(uint vk)
        {
            if (vk >= 0x30 && vk <= 0x39)
            {
                return ((char)vk).ToString();
            }
            if (vk >= 0x41 && vk <= 0x5A)
            {
                return ((char)vk).ToString();
            }
            if (vk >= 0x70 && vk <= 0x7B)
            {
                return "F" + (vk - 0x70 + 1);
            }
            return "VK" + vk;
        }

        private static uint? NameToVk(string name)
        {
            if (name.Length == 1)
            {
                char c = char.ToUpperInvariant(name[0]);
                if (c >= '0' && c <= '9')
                {
                    return (uint)c;
                }
                if (c >= 'A' && c <= 'Z')
                {
                    return (uint)c;
                }
                return null;
            }
            if (name.Length >= 2 && (name[0] == 'F' || name[0] == 'f'))
            {
                int number;
                if (int.TryParse(name.Substring(1), out number)
                    && number >= 1 && number <= 12)
                {
                    return (uint)(0x70 + number - 1);
                }
            }
            return null;
        }
    }

    /// <summary>
    /// 快捷键配置存取：%APPDATA%\AppSnapshot\settings.txt 中的
    /// hotkey.capture / hotkey.record / hotkey.previousApp / hotkey.polish。
    /// 行存在且值为空表示「未绑定」；行不存在则用默认值。
    /// 默认仅绑定截取（Alt+Shift+2）与录制（Alt+Shift+R），
    /// 截取上一个应用与润色默认不绑定，由用户在设置窗中自行决定。
    /// </summary>
    internal static class HotKeyPreferences
    {
        private static readonly HotKeySpec DefaultCapture = HotKeySpec.Parse("Alt+Shift+2");
        private static readonly HotKeySpec DefaultRecord = HotKeySpec.Parse("Alt+Shift+R");

        internal static HotKeySpec Capture
        {
            get { return Load("hotkey.capture", DefaultCapture); }
        }

        internal static HotKeySpec Record
        {
            get { return Load("hotkey.record", DefaultRecord); }
        }

        internal static HotKeySpec PreviousApp
        {
            get { return Load("hotkey.previousApp", null); }
        }

        internal static HotKeySpec Polish
        {
            get { return Load("hotkey.polish", null); }
        }

        internal static void Save(HotKeySpec capture, HotKeySpec record, HotKeySpec previousApp, HotKeySpec polish)
        {
            AppSettings.Write("hotkey.capture", capture == null ? "" : capture.DisplayName);
            AppSettings.Write("hotkey.record", record == null ? "" : record.DisplayName);
            AppSettings.Write("hotkey.previousApp", previousApp == null ? "" : previousApp.DisplayName);
            AppSettings.Write("hotkey.polish", polish == null ? "" : polish.DisplayName);
        }

        private static HotKeySpec Load(string key, HotKeySpec fallback)
        {
            string value = AppSettings.Read(key);
            if (value == null)
            {
                return fallback;
            }
            value = value.Trim();
            if (value.Length == 0)
            {
                return null;
            }
            return HotKeySpec.Parse(value) ?? fallback;
        }
    }

    /// <summary>
    /// 全局快捷键（RegisterHotKey）：截取当前应用 / 录制 / 截取上一个应用 / 润色提示词，
    /// 全部可选绑定，通过消息专用窗口接收 WM_HOTKEY，对应 macOS 端的 GlobalHotKey。
    /// </summary>
    internal sealed class HotKeyManager : NativeWindow, IDisposable
    {
        internal event Action CapturePressed;
        internal event Action RecordPressed;
        internal event Action PreviousAppPressed;
        internal event Action PolishPressed;

        private HotKeySpec _capture;
        private HotKeySpec _record;
        private HotKeySpec _previousApp;
        private HotKeySpec _polish;

        internal HotKeyManager()
        {
            var parameters = new CreateParams
            {
                Caption = "AppSnapshotHotKeys",
                Parent = new IntPtr(-3) // HWND_MESSAGE
            };
            CreateHandle(parameters);
        }

        /// <summary>
        /// 整组应用：全部注册成功才生效并记录为新组合；
        /// 任一被占用则注销本次注册并恢复旧组合（保持原快捷键可用）。
        /// 返回失败项下标（0=截取当前应用 1=录制 2=截取上一个应用 3=润色）；null 表示全部成功。
        /// </summary>
        internal int[] Apply(HotKeySpec capture, HotKeySpec record, HotKeySpec previousApp, HotKeySpec polish)
        {
            var specs = new[] { capture, record, previousApp, polish };
            UnregisterAll();

            var failures = new List<int>();
            for (int i = 0; i < specs.Length; i++)
            {
                if (specs[i] == null)
                {
                    continue;
                }
                if (!NativeMethods.RegisterHotKey(
                        Handle, i + 1,
                        specs[i].Modifiers | NativeMethods.ModNoRepeat,
                        specs[i].Vk))
                {
                    failures.Add(i);
                }
            }

            if (failures.Count > 0)
            {
                UnregisterAll();
                var old = new[] { _capture, _record, _previousApp, _polish };
                for (int i = 0; i < old.Length; i++)
                {
                    if (old[i] == null)
                    {
                        continue;
                    }
                    // 恢复旧组合也可能失败（间隙中被其他程序抢占）：
                    // 失败的项置空，保持记录与实际注册状态一致
                    if (!NativeMethods.RegisterHotKey(
                            Handle, i + 1,
                            old[i].Modifiers | NativeMethods.ModNoRepeat,
                            old[i].Vk))
                    {
                        old[i] = null;
                    }
                }
                _capture = old[0];
                _record = old[1];
                _previousApp = old[2];
                _polish = old[3];
                return failures.ToArray();
            }

            _capture = capture;
            _record = record;
            _previousApp = previousApp;
            _polish = polish;
            return null;
        }

        protected override void WndProc(ref Message message)
        {
            if (message.Msg == NativeMethods.WmHotkey)
            {
                int id = message.WParam.ToInt32();
                Action handler;
                switch (id)
                {
                    case 1:
                        handler = CapturePressed;
                        break;
                    case 2:
                        handler = RecordPressed;
                        break;
                    case 3:
                        handler = PreviousAppPressed;
                        break;
                    case 4:
                        handler = PolishPressed;
                        break;
                    default:
                        handler = null;
                        break;
                }
                if (handler != null)
                {
                    handler();
                }
            }
            base.WndProc(ref message);
        }

        private void UnregisterAll()
        {
            for (int id = 1; id <= 4; id++)
            {
                NativeMethods.UnregisterHotKey(Handle, id);
            }
        }

        public void Dispose()
        {
            UnregisterAll();
            ReleaseHandle();
        }
    }
}
