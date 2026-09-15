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
        // 加宽到 380：标题省略号按像素截断后，更宽的面板能放下更多有效字符。
        // 常量是 96 DPI 设计值，UiScale 负责换算成当前屏幕的物理像素。
        private static int MenuWidth { get { return UiScale.Px(248); } }
        private static Size ListPageSize { get { return UiScale.Px(380, 460); } }

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
            // Controls.Clear() 只移除不 Dispose，旧列表的图标 GDI 句柄会泄漏
            foreach (Control control in Controls)
            {
                control.Dispose();
            }
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

            int top = UiScale.Px(16);
            foreach (Button button in buttons)
            {
                button.SetBounds(UiScale.Px(16), top, width - UiScale.Px(32), UiScale.Px(40));
                Controls.Add(button);
                top += UiScale.Px(48);
            }

            int height = top + UiScale.Px(8);
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
            // Controls.Clear() 只移除不 Dispose，旧列表的图标 GDI 句柄会泄漏
            foreach (Control control in Controls)
            {
                control.Dispose();
            }
            Controls.Clear();

            var header = new Label
            {
                Text = "选择要截取的窗口",
                AutoSize = false,
                Bounds = new Rectangle(UiScale.Px(12), UiScale.Px(10), UiScale.Px(190), UiScale.Px(24)),
                Font = new Font("Microsoft YaHei UI", 10.5F, FontStyle.Bold),
                TextAlign = ContentAlignment.MiddleLeft
            };

            var backButton = new Button
            {
                Text = "返回",
                AutoSize = false,
                Bounds = new Rectangle(size.Width - UiScale.Px(84), UiScale.Px(8), UiScale.Px(72), UiScale.Px(28)),
                FlatStyle = FlatStyle.Standard
            };
            backButton.Click += delegate
            {
                App.Panels.ShowMenu();
            };

            var list = new WindowListBox
            {
                Bounds = new Rectangle(UiScale.Px(12), UiScale.Px(44), size.Width - UiScale.Px(24), size.Height - UiScale.Px(56)),
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

                Action populate = delegate
                {
                    if (panel.IsDisposed || list.IsDisposed || !panel.IsHandleCreated)
                    {
                        // 面板已关闭：释放后台加载的图标，避免泄漏
                        var released = new HashSet<Bitmap>();
                        foreach (WindowListItem item in items)
                        {
                            if (item.Icon != null && released.Add(item.Icon))
                            {
                                item.Icon.Dispose();
                            }
                        }
                        return;
                    }
                    foreach (WindowListItem item in items)
                    {
                        list.Items.Add(item);
                    }
                    list.Invalidate();
                };

                if (panel.IsDisposed || !panel.IsHandleCreated)
                {
                    populate();
                    return;
                }
                try
                {
                    panel.Invoke(populate);
                }
                catch (InvalidOperationException)
                {
                    // 句柄在 Invoke 途中销毁
                    populate();
                }
            });
        }

        private void SetBoundsCore(int width, int height)
        {
            ClientSize = new Size(width, height);
            PositionNextToBubble();
        }

        internal void PositionNextToBubble()
        {
            // 桌宠模式下锚定到猫的位置,面板跟随宿主形态而非隐藏中的悬浮球
            Rectangle anchorRect;
            if (App.IsPetMode && App.Pet != null && !App.Pet.CurrentBounds.IsEmpty)
            {
                anchorRect = App.Pet.CurrentBounds;
            }
            else
            {
                Form bubble = App.MainForm;
                anchorRect = bubble != null
                    ? bubble.Bounds
                    : new Rectangle(Cursor.Position, Size.Empty);
            }
            Screen screen = Screen.FromPoint(new Point(anchorRect.Left + anchorRect.Width / 2, anchorRect.Top + anchorRect.Height / 2));

            const int Inset = 8;
            int x = anchorRect.Left + (anchorRect.Width - Width) / 2;
            int y = anchorRect.Top - Height - Inset;
            if (y < screen.WorkingArea.Top + Inset)
            {
                y = anchorRect.Bottom + Inset;
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

        /// <summary>
        /// 枚举用户可截取的窗口，对齐任务栏口径：
        /// 可见顶层窗口 + 非 cloaked（UWP 挂起幽灵窗口）+ 有标题；
        /// 最小化窗口保留（任务栏也显示它们），由调用方决定还原后再截取。
        /// </summary>
        private static List<WindowEntry> EnumerateCapturableWindows()
        {
            var entries = new List<WindowEntry>();
            var ownProcessId = (uint)System.Diagnostics.Process.GetCurrentProcess().Id;

            NativeMethods.EnumWindowsDelegate callback = delegate(IntPtr window, IntPtr lParam)
            {
                // 回调内任何异常都会沿 EnumWindows 栈崩溃进程，必须整体兜底
                try
                {
                    if (!NativeMethods.IsWindowVisible(window))
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
                    if (processId == 0 || processId == ownProcessId)
                    {
                        return true;
                    }

                    // UWP 挂起/壳托管的幽灵窗口（如 Windows 输入体验）会被 DWM 标记 cloaked，任务栏同样不显示
                    int cloaked;
                    if (NativeMethods.DwmGetWindowAttributeInt(window, NativeMethods.DwmwaCloaked, out cloaked, 4) == 0
                        && cloaked != 0)
                    {
                        return true;
                    }

                    // 无标题窗口（如 Program Manager）没有截取意义
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

                    // 任务栏口径：有 WS_EX_APPWINDOW 的窗口强制显示；
                    // 否则排除工具窗口和有主窗口（owner）的弹窗。
                    // 注意不能用 Process.MainModule 过滤后台进程——对提权进程会抛访问被拒，误杀正常应用。
                    long extended = NativeMethods.GetWindowLongPtr(window, NativeMethods.GwlExStyle).ToInt64();
                    bool appWindow = (extended & NativeMethods.WsExAppWindow) != 0;
                    if (!appWindow)
                    {
                        if ((extended & NativeMethods.WsExToolWindowCheck) != 0)
                        {
                            return true;
                        }
                        if (NativeMethods.GetWindow(window, NativeMethods.GwOwner) != IntPtr.Zero)
                        {
                            return true;
                        }
                    }

                    entries.Add(new WindowEntry
                    {
                        Handle = window,
                        Title = title
                    });
                }
                catch
                {
                    // 单个窗口属性读取失败（如窗口恰好销毁），跳过继续枚举
                }
                return true;
            };

            NativeMethods.EnumWindows(callback, IntPtr.Zero);
            // 按标题排序；最小化与否只是截取时的技术细节，不在列表里区分
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

        private readonly int _itemHeight = UiScale.Px(48);
        private readonly int _iconSize = UiScale.Px(32);
        private readonly int _iconPadding = UiScale.Px(8);
        private readonly int _textMargin = UiScale.Px(8);

        private int _hoverIndex = -1;
        private int _scrollOffset;
        private readonly Font _titleFont = new Font("Microsoft YaHei UI", 9.5F);
        private readonly Font _subFont = new Font("Microsoft YaHei UI", 8F);
        private readonly SolidBrush _titleBrush = new SolidBrush(Color.FromArgb(30, 30, 30));
        private readonly SolidBrush _subBrush = new SolidBrush(Color.FromArgb(120, 120, 120));
        private readonly SolidBrush _hoverBrush = new SolidBrush(Color.FromArgb(240, 244, 252));
        private readonly Pen _separatorPen = new Pen(Color.FromArgb(230, 230, 230));
        // 单行 + 像素级省略号：默认 DrawString 会换行，长标题折行后与进程名叠在一起
        private readonly StringFormat _lineFormat = new StringFormat
        {
            Trimming = StringTrimming.EllipsisCharacter,
            FormatFlags = StringFormatFlags.NoWrap | StringFormatFlags.LineLimit
        };

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
            e.Graphics.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;

            for (int i = 0; i < Items.Count; i++)
            {
                int itemTop = i * _itemHeight - _scrollOffset;
                if (itemTop + _itemHeight < 0 || itemTop > Height)
                {
                    continue;
                }

                var bounds = new Rectangle(0, itemTop, Width, _itemHeight);
                WindowListItem item = Items[i];

                if (i == _hoverIndex)
                {
                    e.Graphics.FillRectangle(_hoverBrush, bounds);
                }

                int textX;
                if (item.IsGroupChild)
                {
                    // 子窗口：缩进，单行显示标题
                    textX = _iconPadding + _iconSize + UiScale.Px(30);
                    var titleRect = new RectangleF(textX, itemTop + UiScale.Px(14), Width - textX - _textMargin, UiScale.Px(20));
                    e.Graphics.DrawString(
                        item.Title,
                        _titleFont,
                        _titleBrush,
                        titleRect,
                        _lineFormat);
                }
                else
                {
                    // 首窗口：图标 + 标题 + 进程名两行
                    if (item.Icon != null)
                    {
                        e.Graphics.DrawImage(
                            item.Icon,
                            _iconPadding,
                            itemTop + (_itemHeight - _iconSize) / 2,
                            _iconSize,
                            _iconSize);
                    }
                    textX = _iconPadding + _iconSize + UiScale.Px(10);
                    int textWidth = Width - textX - _textMargin;
                    var titleRect = new RectangleF(textX, itemTop + UiScale.Px(7), textWidth, UiScale.Px(20));
                    var subRect = new RectangleF(textX, itemTop + UiScale.Px(27), textWidth, UiScale.Px(16));
                    e.Graphics.DrawString(
                        item.Title,
                        _titleFont,
                        _titleBrush,
                        titleRect,
                        _lineFormat);
                    e.Graphics.DrawString(
                        item.ProcessName,
                        _subFont,
                        _subBrush,
                        subRect,
                        _lineFormat);
                }

                if (i < Items.Count - 1)
                {
                    e.Graphics.DrawLine(
                        _separatorPen,
                        textX,
                        itemTop + _itemHeight - 1,
                        Width - _textMargin,
                        itemTop + _itemHeight - 1);
                }
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
            int delta = e.Delta > 0 ? -_itemHeight : _itemHeight;
            _scrollOffset = Math.Max(0,
                Math.Min(_scrollOffset + delta, Math.Max(0, Items.Count * _itemHeight - Height)));
            Invalidate();
        }

        private int HitTest(int mouseY)
        {
            int index = (mouseY + _scrollOffset) / _itemHeight;
            return index >= 0 && index < Items.Count ? index : -1;
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                ReleaseItemIcons();
                _titleFont.Dispose();
                _subFont.Dispose();
                _titleBrush.Dispose();
                _subBrush.Dispose();
                _hoverBrush.Dispose();
                _separatorPen.Dispose();
            }
            base.Dispose(disposing);
        }

        /// <summary>释放所有列表项的图标 GDI 句柄，重新填充或销毁前必须调用。</summary>
        internal void ReleaseItemIcons()
        {
            var released = new HashSet<Bitmap>();
            foreach (WindowListItem item in Items)
            {
                if (item.Icon != null && released.Add(item.Icon))
                {
                    item.Icon.Dispose();
                }
                item.Icon = null;
            }
            Items.Clear();
        }
    }
}
