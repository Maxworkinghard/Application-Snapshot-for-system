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
                Bounds = new Rectangle(12, 12, 190, 22),
                Font = new Font("Microsoft YaHei UI", 10.5F, FontStyle.Bold),
                TextAlign = ContentAlignment.MiddleLeft
            };

            var backButton = new Button
            {
                Text = "返回",
                AutoSize = false,
                Bounds = new Rectangle(size.Width - 84, 8, 72, 30),
                FlatStyle = FlatStyle.Standard
            };
            backButton.Click += delegate
            {
                App.Panels.ShowMenu();
            };

            var list = new ListView
            {
                Bounds = new Rectangle(12, 46, size.Width - 24, size.Height - 58),
                View = View.Details,
                FullRowSelect = true,
                HeaderStyle = ColumnHeaderStyle.None,
                HideSelection = true,
                BorderStyle = BorderStyle.FixedSingle,
                MultiSelect = false,
                Scrollable = true
            };
            var imageList = new ImageList
            {
                ImageSize = new Size(32, 32),
                ColorDepth = ColorDepth.Depth32Bit
            };
            list.SmallImageList = imageList;

            var column = new ColumnHeader { Width = size.Width - 40 - 34 };
            list.Columns.Add(column);

            int index = 0;
            foreach (WindowEntry entry in EnumerateCapturableWindows())
            {
                uint processId;
                NativeMethods.GetWindowThreadProcessId(entry.Handle, out processId);
                Bitmap icon = WindowIconLoader.LoadWindowIcon(entry.Handle, (int)processId, 32);
                string key = "w" + index;
                imageList.Images.Add(key, icon);
                var item = new ListViewItem(TruncateTitle(entry.Title))
                {
                    ImageKey = key,
                    Tag = entry.Handle,
                    ToolTipText = entry.Title
                };
                list.Items.Add(item);
                index++;
            }

            list.Click += delegate
            {
                ListView.SelectedListViewItemCollection selected = list.SelectedItems;
                if (selected.Count == 0)
                {
                    return;
                }
                IntPtr handle = (IntPtr)selected[0].Tag;
                Close();
                App.CaptureWindow(handle);
            };

            Controls.Add(header);
            Controls.Add(backButton);
            Controls.Add(list);

            SetBoundsCore(size.Width, size.Height);
            ResumeLayout();
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
            var seenProcesses = new HashSet<uint>();
            var ownProcessId = (uint)System.Diagnostics.Process.GetCurrentProcess().Id;

            NativeMethods.EnumWindowsDelegate callback = delegate(IntPtr window, IntPtr lParam)
            {
                if (!NativeMethods.IsWindowVisible(window) || NativeMethods.IsIconic(window))
                {
                    return true;
                }

                uint processId;
                NativeMethods.GetWindowThreadProcessId(window, out processId);
                if (processId == 0 || processId == ownProcessId)
                {
                    return true;
                }

                // 样式值可能含高位标志（如 WS_POPUP=0x80000000），ToInt32 会抛 OverflowException
                long style = NativeMethods.GetWindowLongPtr(window, NativeMethods.GwlStyle).ToInt64();
                long extended = NativeMethods.GetWindowLongPtr(window, NativeMethods.GwlExStyle).ToInt64();
                if ((extended & NativeMethods.WsExToolWindowCheck) != 0
                    || (style & NativeMethods.WsVisible) == 0)
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

                if (seenProcesses.Add(processId))
                {
                    entries.Add(new WindowEntry { Handle = window, Title = title });
                }
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
}
