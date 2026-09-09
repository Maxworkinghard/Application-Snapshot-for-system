using System;
using System.Drawing;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>提示词编辑对话框：名称 + 全文；「保存」校验通过后以 DialogResult.OK 结束，由调用方写库。</summary>
    internal sealed class PolishPromptEditorForm : Form
    {
        private readonly TextBox _nameBox;
        private readonly TextBox _textBox;
        private readonly string _originalName;

        internal PolishPromptLibrary.CustomPrompt Prompt
        {
            get
            {
                return new PolishPromptLibrary.CustomPrompt
                {
                    Name = _nameBox.Text,
                    Text = _textBox.Text
                };
            }
        }

        internal PolishPromptEditorForm(PolishPromptLibrary.CustomPrompt original, string prefill)
        {
            _originalName = original == null ? null : original.Name;

            Text = original == null ? "新建润色提示词" : "编辑润色提示词";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            StartPosition = FormStartPosition.CenterParent;
            ShowInTaskbar = false;
            Font = new Font("Microsoft YaHei UI", 9F);
            ClientSize = new Size(520, 420);
            AutoScaleMode = AutoScaleMode.Dpi;

            var nameLabel = new Label
            {
                Text = "名称",
                AutoSize = true,
                TextAlign = ContentAlignment.MiddleRight,
                Location = new Point(12, 20),
                Size = new Size(72, 20)
            };

            _nameBox = new TextBox
            {
                Text = original == null ? "" : original.Name,
                Location = new Point(96, 17),
                Size = new Size(400, 25)
            };

            var textLabel = new Label
            {
                Text = "提示词全文",
                AutoSize = true,
                ForeColor = SystemColors.GrayText,
                Location = new Point(12, 56)
            };

            _textBox = new TextBox
            {
                Multiline = true,
                ScrollBars = ScrollBars.Vertical,
                AcceptsReturn = true,
                WordWrap = false,
                Text = prefill,
                Font = new Font("Consolas", 9F),
                Location = new Point(12, 76),
                Size = new Size(496, 288)
            };

            var cancelButton = new Button
            {
                Text = "取消",
                AutoSize = true,
                Location = new Point(360, 380)
            };
            cancelButton.Click += delegate { DialogResult = DialogResult.Cancel; };

            var saveButton = new Button
            {
                Text = "保存",
                AutoSize = true,
                Location = new Point(440, 380)
            };
            saveButton.Click += Save;

            Controls.Add(nameLabel);
            Controls.Add(_nameBox);
            Controls.Add(textLabel);
            Controls.Add(_textBox);
            Controls.Add(cancelButton);
            Controls.Add(saveButton);
            AcceptButton = saveButton;
            CancelButton = cancelButton;
        }

        private void Save(object sender, EventArgs e)
        {
            string name = _nameBox.Text.Trim();
            if (name.Length == 0)
            {
                MessageBox.Show(this, "名称不能为空", "润色提示词", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
            if (name == PolishPromptLibrary.BuiltinName)
            {
                MessageBox.Show(this, "名称不能与「" + PolishPromptLibrary.BuiltinName + "」相同",
                    "润色提示词", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
            foreach (PolishPromptLibrary.CustomPrompt prompt in PolishPromptLibrary.Custom)
            {
                if (prompt.Name == name && prompt.Name != _originalName)
                {
                    MessageBox.Show(this, "已存在同名提示词「" + name + "」",
                        "润色提示词", MessageBoxButtons.OK, MessageBoxIcon.Warning);
                    return;
                }
            }
            DialogResult = DialogResult.OK;
        }
    }
}
