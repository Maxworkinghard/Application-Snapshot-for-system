using System;
using System.Collections.Generic;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 悬浮球点击后的操作面板（对应 macOS 端的菜单页 + 应用快照列表页）。
    /// 一级菜单：应用快照 / 录制 / 润色 Prompt；
    /// 二级页：窗口列表，点击即截取该窗口。
    /// </summary>
    internal sealed class ActionPanelController
    {
        private static readonly int MenuWidth = 248;
        private static readonly Size ListPageSize = new Size(328, 420);

        private ActionPanelForm form;

        internal void Toggle()
        {
            if (form != null && !form.IsDisposed)
            {
                Close();
                return;
            }
            ShowMenu();
        }

        internal void Close()
        {
            if (form != null)
            {
                if (!form.IsDisposed)
                {
                    form.Close();
                }
                form = null;
            }
        }

        internal void ShowMenu()
        {
            Close();
            form = new ActionPanelForm();
            form.BuildMenuPage(MenuWidth);
            ShowForm();
        }

        internal void ShowList()
        {
            Close();
            form = new ActionPanelForm();
            form.BuildListPage(ListPageSize);
            ShowForm();
        }

        /// <summary>悬浮球被拖动时调用，让打开的面板跟随锚定位置。</summary>
        internal void Reposition()
        {
            if (form != null && !form.IsDisposed)
            {
                form.PositionNextToBubble();
            }
        }

        private void ShowForm()
        {
            var panel = form;
            if (panel == null)
            {
                return;
            }
            panel.Closed += delegate
            {
                if (form == panel)
                {
                    form = null;
                }
            };
            panel.ShowPanel();
        }
    }

    internal sealed class ActionPanelForm : Form
    {
        private sealed class WindowEntry
        {
            public IntPtr Handle;
            public string Title;
        }

        private const int CsDropShadow = 0x00020000;

        internal ActionPanelForm()
        {
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            TopMost = true;
            BackColor = Color.White;
            Font = new Font("Microsoft YaHei UI", 9F);
            AutoScaleMode = AutoScaleMode.Dpi;

            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.UserPaint,
                true);
        }

        protected override bool ShowWithoutActivation
        {
            get { return true; }
        }

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams parameters = base.CreateParams;
                parameters.ClassStyle |= CsDropShadow;
                parameters.ExStyle |= NativeMethods.WsExToolWindow | NativeMethods.WsExNoActivate;
                return parameters;
            }
        }

        internal void BuildMenuPage(int width)
        {
            SuspendLayout();
            Controls.Clear();

            var buttons = new List<Button>();

            var snapshotButton = MakeButton("应用快照");
            snapshotButton.Click += delegate
            {
                App.Panels.ShowList();
            };
            buttons.Add(snapshotButton);

            var recordButton = MakeButton(BuildRecordButtonText());
            recordButton.Enabled = App.Recording.RecordButtonEnabled;
            recordButton.Click += delegate
            {
                Close();
                App.ToggleRecording();
            };
            buttons.Add(recordButton);

            bool polishing = App.Polish.IsBusy;
            var polishButton = MakeButton(polishing ? "停止润色" : "润色 Prompt");
            polishButton.Click += delegate
            {
                Close();
                App.Polish.ToggleFromMenu();
            };
            buttons.Add(polishButton);

            int top = 16;
            foreach (Button button in buttons)
            {
                button.SetBounds(16, top, width - 32, 40);
                Controls.Add(button);
                top += 48;
            }

            int height = top + 8;
            SetBoundsCore(width, height);
            ResumeLayout();
        }

        private static string BuildRecordButtonText()
        {
            string title = App.Recording.RecordButtonTitle;
            string subtitle = App.Recording.RecordButtonSubtitle;
            if (subtitle != null)
            {
                return title + "\n" + subtitle;
            }
            return title;
        }

        internal void BuildListPage(Size size)
        {
            SuspendLayout();
            Controls.Clear();

            var header = new Label
            {
                Text = "选择要截取的窗口",
                AutoSize = false,
                Bounds = new Rectangle(12, 10, 190, 24),
                Font = new Font("Microsoft YaHei UI", 10.5F, FontStyle.Bold),
                TextAlign = ContentAlignment.MiddleLeft
            };

            var backButton = new Button
            {
                Text = "返回",
                AutoSize = false,
                Bounds = new Rectangle(size.Width - 84, 8, 72, 28),
                FlatStyle = FlatStyle.Standard
            };
            backButton.Click += delegate
            {
                App.Panels.ShowMenu();
            };

            var list = new WindowListBox
            {
                Bounds = new Rectangle(12, 44, size.Width - 24, size.Height - 56),
                BackColor = Color.White
            };

            list.ItemActivated += delegate(WindowListItem item)
            {
                Close();
                App.CaptureWindow(item.Handle);
            };

            Controls.Add(header);
            Controls.Add(backButton);
            Controls.Add(list);

            SetBoundsCore(size.Width, size.Height);
            ResumeLayout();

            // 后台线程枚举窗口和加载图标，避免阻塞 UI
            var panel = this;
            System.Threading.ThreadPool.QueueUserWorkItem(delegate
            {
                var entries = EnumerateCapturableWindows();

                // 按进程分组：组内窗口缩进，进程名只显示一次
                var groups = new Dictionary<uint, List<WindowEntry>>();
                foreach (WindowEntry entry in entries)
                {
                    uint pid;
                    NativeMethods.GetWindowThreadProcessId(entry.Handle, out pid);
                    if (!groups.ContainsKey(pid))
                    {
                        groups[pid] = new List<WindowEntry>();
                    }
                    groups[pid].Add(entry);
                }

                var items = new List<WindowListItem>();
                foreach (var pair in groups)
                {
                    uint pid = pair.Key;
                    List<WindowEntry> windows = pair.Value;
                    string processName = "";
                    try
                    {
                        processName = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName;
                    }
                    catch { }

                    Bitmap icon = windows.Count > 0
                        ? WindowIconLoader.LoadWindowIcon(windows[0].Handle, (int)pid, 32)
                        : null;

                    for (int i = 0; i < windows.Count; i++)
                    {
                        items.Add(new WindowListItem
                        {
                            Handle = windows[i].Handle,
                            Title = windows[i].Title,
                            ProcessName = i == 0 ? processName : "",
                            Icon = i == 0 ? icon : null,
                            IsGroupChild = i > 0
                        });
                    }
                }

                panel.Invoke(new Action(delegate
                {
                    foreach (WindowListItem item in items)
                    {
                        list.Items.Add(item);
                    }
                    list.Invalidate();
                }));
            });
        }

        private static string TruncateTitle(string title)
        {
            if (title != null && title.Length > 60)
            {
                return title.Substring(0, 60) + "…";
            }
            return title;
        }

        private void SetBoundsCore(int width, int height)
        {
            ClientSize = new Size(width, height);
            PositionNextToBubble();
        }

        internal void PositionNextToBubble()
        {
            Form bubble = App.MainForm;
            Rectangle bubbleRect = bubble != null
                ? bubble.Bounds
                : new Rectangle(Cursor.Position, Size.Empty);
            Screen screen = Screen.FromPoint(new Point(bubbleRect.Left + bubbleRect.Width / 2, bubbleRect.Top + bubbleRect.Height / 2));

            const int Inset = 8;
            int x = bubbleRect.Left + (bubbleRect.Width - Width) / 2;
            int y = bubbleRect.Top - Height - Inset;
            if (y < screen.WorkingArea.Top + Inset)
            {
                y = bubbleRect.Bottom + Inset;
            }

            Rectangle working = screen.WorkingArea;
            x = Math.Max(working.Left + Inset, Math.Min(x, working.Right - Width - Inset));
            y = Math.Max(working.Top + Inset, Math.Min(y, working.Bottom - Height - Inset));
            Location = new Point(x, y);
        }

        internal void ShowPanel()
        {
            Show();
        }

        private static List<WindowEntry> EnumerateCapturableWindows()
        {
            var entries = new List<WindowEntry>();
            var ownProcessId = (uint)System.Diagnostics.Process.GetCurrentProcess().Id;

            // 先收集有 GUI 的进程 ID（MainWindowHandle != 0），排除纯后台进程
            var guiProcessIds = new HashSet<uint>();
            try
            {
                foreach (System.Diagnostics.Process process in System.Diagnostics.Process.GetProcesses())
                {
                    try
                    {
                        if (process.MainWindowHandle != IntPtr.Zero)
                        {
                            guiProcessIds.Add((uint)process.Id);
                        }
                    }
                    catch { }
                }
            }
            catch { }

            NativeMethods.EnumWindowsDelegate callback = delegate(IntPtr window, IntPtr lParam)
            {
                if (!NativeMethods.IsWindowVisible(window) || NativeMethods.IsIconic(window))
                {
                    return true;
                }

                // 只保留真正的顶层窗口（GetAncestor(GA_ROOT) == 自身）
                if (NativeMethods.GetAncestor(window, 2) != window)
                {
                    return true;
                }

                uint processId;
                NativeMethods.GetWindowThreadProcessId(window, out processId);
                if (processId == 0 || processId == ownProcessId || !guiProcessIds.Contains(processId))
                {
                    return true;
                }

                long extended = NativeMethods.GetWindowLongPtr(window, NativeMethods.GwlExStyle).ToInt64();
                if ((extended & NativeMethods.WsExToolWindowCheck) != 0)
                {
                    return true;
                }

                int length = NativeMethods.GetWindowTextLength(window);
                if (length <= 0)
                {
                    return true;
                }
                var text = new StringBuilder(length + 1);
                NativeMethods.GetWindowText(window, text, text.Capacity);
                string title = text.ToString();
                if (title.Length == 0)
                {
                    return true;
                }

                entries.Add(new WindowEntry { Handle = window, Title = title });
                return true;
            };

            NativeMethods.EnumWindows(callback, IntPtr.Zero);
            entries.Sort(delegate(WindowEntry left, WindowEntry right)
            {
                return string.Compare(left.Title, right.Title, StringComparison.CurrentCultureIgnoreCase);
            });
            return entries;
        }

        private static Button MakeButton(string text)
        {
            return new Button
            {
                Text = text,
                FlatStyle = FlatStyle.Standard,
                TextAlign = ContentAlignment.MiddleCenter,
                ForeColor = SystemColors.ControlText,
                Font = new Font("Microsoft YaHei UI", 9.5F)
            };
        }
    }

    /// <summary>窗口列表项数据。</summary>
    internal sealed class WindowListItem
    {
        public IntPtr Handle;
        public string Title;
        public string ProcessName;
        public Bitmap Icon;
        public bool IsGroupChild;
    }

    /// <summary>自绘窗口列表：大图标 + 标题 + 进程名两行，悬停高亮，点击回调。</summary>
    internal sealed class WindowListBox : Control
    {
        internal delegate void ItemActivatedHandler(WindowListItem item);
        internal event ItemActivatedHandler ItemActivated;

        internal readonly List<WindowListItem> Items = new List<WindowListItem>();

        private const int ItemHeight = 48;
        private const int IconSize = 32;
        private const int IconPadding = 8;

        private int _hoverIndex = -1;
        private int _scrollOffset;
        private readonly Font _titleFont = new Font("Microsoft YaHei UI", 9.5F);
        private readonly Font _subFont = new Font("Microsoft YaHei UI", 8F);
        private readonly SolidBrush _titleBrush = new SolidBrush(Color.FromArgb(30, 30, 30));
        private readonly SolidBrush _subBrush = new SolidBrush(Color.FromArgb(120, 120, 120));
        private readonly SolidBrush _hoverBrush = new SolidBrush(Color.FromArgb(240, 244, 252));
        private readonly Pen _separatorPen = new Pen(Color.FromArgb(230, 230, 230));

        internal WindowListBox()
        {
            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.UserPaint
                | ControlStyles.ResizeRedraw
                | ControlStyles.Selectable,
                true);
            TabStop = true;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            e.Graphics.Clear(BackColor);

            int y = -_scrollOffset;
            for (int i = 0; i < Items.Count; i++)
            {
                var bounds = new Rectangle(0, y, Width, ItemHeight);
                if (bounds.Bottom >= 0 && bounds.Top <= Height)
                {
                    if (i == _hoverIndex)
                    {
                        e.Graphics.FillRectangle(_hoverBrush, bounds);
                    }

                    WindowListItem item = Items[i];
                    if (item.Icon != null && !item.IsGroupChild)
                    {
                        e.Graphics.DrawImage(
                            item.Icon,
                            IconPadding,
                            y + (ItemHeight - IconSize) / 2,
                            IconSize,
                            IconSize);
                    }

                    int textX = IconPadding + IconSize + 10;
                    int textWidth = Width - textX - 8;

                    if (item.IsGroupChild)
                    {
                        // 子窗口缩进显示，不画图标和进程名
                        textX += 20;
                        textWidth -= 20;
                        var titleRect = new RectangleF(textX, y + 7, textWidth, 20);
                        e.Graphics.DrawString(
                            "  " + Truncate(item.Title, 32),
                            _titleFont,
                            _titleBrush,
                            titleRect);
                    }
                    else
                    {
                        var titleRect = new RectangleF(textX, y + 7, textWidth, 20);
                        var subRect = new RectangleF(textX, y + 27, textWidth, 16);
                        e.Graphics.DrawString(
                            Truncate(item.Title, 36),
                            _titleFont,
                            _titleBrush,
                            titleRect);
                        e.Graphics.DrawString(
                            item.ProcessName,
                            _subFont,
                            _subBrush,
                            subRect);
                    }

                    if (i < Items.Count - 1)
                    {
                        e.Graphics.DrawLine(
                            _separatorPen,
                            textX,
                            y + ItemHeight - 1,
                            Width - 8,
                            y + ItemHeight - 1);
                    }
                }
                y += ItemHeight;
            }
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            int index = HitTest(e.Y);
            if (index != _hoverIndex)
            {
                _hoverIndex = index;
                Invalidate();
            }
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hoverIndex = -1;
            Invalidate();
        }

        protected override void OnMouseClick(MouseEventArgs e)
        {
            base.OnMouseClick(e);
            int index = HitTest(e.Y);
            if (index >= 0 && index < Items.Count && ItemActivated != null)
            {
                ItemActivated(Items[index]);
            }
        }

        protected override void OnMouseWheel(MouseEventArgs e)
        {
            base.OnMouseWheel(e);
            int delta = e.Delta > 0 ? -ItemHeight : ItemHeight;
            _scrollOffset = Math.Max(0,
                Math.Min(_scrollOffset + delta, Math.Max(0, Items.Count * ItemHeight - Height)));
            Invalidate();
        }

        private int HitTest(int mouseY)
        {
            int index = (mouseY + _scrollOffset) / ItemHeight;
            return index >= 0 && index < Items.Count ? index : -1;
        }

        private static string Truncate(string text, int max)
        {
            if (text != null && text.Length > max)
            {
                return text.Substring(0, max) + "…";
            }
            return text;
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _titleFont.Dispose();
                _subFont.Dispose();
                _titleBrush.Dispose();
                _subBrush.Dispose();
                _hoverBrush.Dispose();
                _separatorPen.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
