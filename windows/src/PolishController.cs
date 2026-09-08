using System;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 润色流程：剪切板文字 → 用户确认 → 服务润色 → 结果写回剪切板（替换原文）。
    /// 处理中再次点击视为取消；写回前校验剪切板未被外部改动（与 macOS 端一致）。
    /// 所有入口方法都必须在 UI 线程调用。
    /// </summary>
    internal sealed class PolishController
    {
        private const int MaxPolishInputLength = 12000;
        private const int ConfirmPreviewLength = 200;

        private readonly PolishService service = new PolishService();
        private readonly object syncRoot = new object();
        private bool busy;

        private string undoOriginalText;
        private uint undoChangeCountAfterWrite;

        /// <summary>是否有一次可撤销的润色（面板按钮显示用，粗略状态）。</summary>
        internal bool HasUndoState
        {
            get { return undoOriginalText != null; }
        }

        internal bool IsBusy
        {
            get
            {
                lock (syncRoot)
                {
                    return busy;
                }
            }
        }

        /// <summary>面板/托盘入口：处理中点击 = 停止润色，空闲时点击 = 开始润色。</summary>
        internal void ToggleFromMenu()
        {
            if (IsBusy)
            {
                service.CancelActive();
                App.Toast.Show("已停止润色，剪切板未改动", ToastKind.Error);
                return;
            }
            Start();
        }

        private void Start()
        {
            string text = "";
            try
            {
                if (Clipboard.ContainsText())
                {
                    text = Clipboard.GetText();
                }
            }
            catch
            {
            }
            text = (text ?? "").Trim();

            if (text.Length == 0)
            {
                App.Toast.Show("剪切板没有文字，请先复制 Prompt", ToastKind.Warning);
                return;
            }
            if (text.Length > MaxPolishInputLength)
            {
                App.Toast.Show(
                    "剪切板内容过长（上限 " + MaxPolishInputLength + " 字）", ToastKind.Warning);
                return;
            }

            uint baselineSequence = NativeMethods.GetClipboardSequenceNumber();
            if (!ConfirmPolish(text))
            {
                return;
            }
            if (NativeMethods.GetClipboardSequenceNumber() != baselineSequence)
            {
                App.Toast.Show("剪切板内容已变化，请重新点击润色", ToastKind.Warning);
                return;
            }

            App.Toast.Show("正在润色…", ToastKind.Success);
            lock (syncRoot)
            {
                busy = true;
            }

            string requestText = text;
            ThreadPool.QueueUserWorkItem(delegate
            {
                PolishOutcome outcome = service.Polish(requestText);
                RunOnUi(delegate { Finish(outcome, requestText, baselineSequence); });
            });
        }

        private void Finish(PolishOutcome outcome, string requestText, uint baselineSequence)
        {
            lock (syncRoot)
            {
                busy = false;
            }

            if (outcome.Cancelled)
            {
                return;
            }
            if (outcome.ErrorMessage != null)
            {
                App.Toast.Show(outcome.ErrorMessage, ToastKind.Error);
                return;
            }

            if (NativeMethods.GetClipboardSequenceNumber() != baselineSequence)
            {
                App.Toast.Show("剪切板内容已变化，润色结果未写入", ToastKind.Warning);
                return;
            }

            try
            {
                Clipboard.SetText(outcome.PolishedText);
            }
            catch
            {
                App.Toast.Show("写入剪切板失败", ToastKind.Error);
                return;
            }
            undoOriginalText = requestText;
            undoChangeCountAfterWrite = NativeMethods.GetClipboardSequenceNumber();
            App.Toast.Show("润色完成，结果已替换剪切板", ToastKind.Success);
        }

        /// <summary>撤销上一次润色：仅当剪切板自写回后未被外部改动时才恢复原文。</summary>
        internal void Undo()
        {
            if (undoOriginalText == null)
            {
                App.Toast.Show("没有可撤销的润色", ToastKind.Warning);
                return;
            }
            string originalText = undoOriginalText;
            undoOriginalText = null;

            if (NativeMethods.GetClipboardSequenceNumber() != undoChangeCountAfterWrite)
            {
                App.Toast.Show("剪切板内容已变化，无法撤销", ToastKind.Warning);
                return;
            }

            try
            {
                Clipboard.SetText(originalText);
            }
            catch
            {
                App.Toast.Show("恢复剪切板失败", ToastKind.Error);
                return;
            }
            App.Toast.Show("已撤销，剪切板已恢复原文", ToastKind.Success);
        }

        /// <summary>系统确认弹窗：用户确认后才会覆盖剪切板内容。</summary>
        private static bool ConfirmPolish(string previewText)
        {
            string preview = previewText.Length > ConfirmPreviewLength
                ? previewText.Substring(0, ConfirmPreviewLength) + "…"
                : previewText;
            DialogResult result = MessageBox.Show(
                "确认后将用润色结果替换剪切板内容：\n\n" + preview,
                "润色剪切板中的 Prompt？",
                MessageBoxButtons.YesNo,
                MessageBoxIcon.Information,
                MessageBoxDefaultButton.Button1);
            return result == DialogResult.Yes;
        }

        private static void RunOnUi(MethodInvoker action)
        {
            Form mainForm = App.MainForm;
            if (mainForm != null && !mainForm.IsDisposed && mainForm.IsHandleCreated)
            {
                try
                {
                    mainForm.BeginInvoke(action);
                }
                catch
                {
                }
            }
        }
    }
}
