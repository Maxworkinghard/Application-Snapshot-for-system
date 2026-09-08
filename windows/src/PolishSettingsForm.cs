using System;
using System.Drawing;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 润色服务设置窗口：三项填写完整后「润色 Prompt」才会调用真实模型；
    /// API Key 保存在 Windows 凭据管理器中（输入框留空表示沿用已保存的 Key）。
    /// </summary>
    internal sealed class PolishSettingsForm : Form
    {
        private ComboBox kindBox;
        private TextBox baseUrlBox;
        private TextBox modelBox;
        private TextBox apiKeyBox;
        private Label errorLabel;

        public PolishSettingsForm()
        {
            Text = "润色服务设置";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            ShowInTaskbar = false;
            Font = new Font("Microsoft YaHei UI", 9F);
            ClientSize = new Size(440, 330);
            AutoScaleMode = AutoScaleMode.Dpi;

            var titleLabel = new Label
            {
                Text = "润色服务设置",
                Font = new Font("Microsoft YaHei UI", 13.5F, FontStyle.Bold),
                AutoSize = true,
                Location = new Point(24, 20)
            };

            var description = new Label
            {
                Text = "三项都填写后，「润色 Prompt」将调用所配置的模型润色；未配置或清除配置后，润色会提示先完成配置。API Key 保存在 Windows 凭据管理器中。",
                AutoSize = false,
                Size = new Size(392, 42),
                Location = new Point(24, 50),
                ForeColor = SystemColors.GrayText
            };

            int rowTop = 104;
            var kindLabel = MakeRowLabel("协议", rowTop);
            kindBox = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Location = new Point(110, rowTop - 3),
                Size = new Size(306, 25)
            };
            kindBox.Items.Add("OpenAI 兼容");
            kindBox.Items.Add("Anthropic");
            kindBox.SelectedIndex = 0;

            rowTop += 40;
            var baseUrlLabel = MakeRowLabel("Base URL", rowTop);
            baseUrlBox = new TextBox
            {
                Location = new Point(110, rowTop - 3),
                Size = new Size(306, 25)
            };

            rowTop += 40;
            var modelLabel = MakeRowLabel("模型", rowTop);
            modelBox = new TextBox
            {
                Location = new Point(110, rowTop - 3),
                Size = new Size(306, 25)
            };

            rowTop += 40;
            var apiKeyLabel = MakeRowLabel("API Key", rowTop);
            apiKeyBox = new TextBox
            {
                Location = new Point(110, rowTop - 3),
                Size = new Size(306, 25),
                UseSystemPasswordChar = true
            };

            errorLabel = new Label
            {
                Text = "",
                AutoSize = false,
                Size = new Size(392, 18),
                Location = new Point(24, rowTop + 34),
                ForeColor = Color.Firebrick
            };

            var clearButton = new Button
            {
                Text = "清除配置",
                AutoSize = true,
                Location = new Point(24, 280)
            };
            clearButton.Click += delegate
            {
                PolishConfiguration.Clear();
                Close();
            };

            var cancelButton = new Button
            {
                Text = "取消",
                AutoSize = true,
                Location = new Point(300, 280)
            };
            cancelButton.Click += delegate { Close(); };

            var saveButton = new Button
            {
                Text = "保存",
                AutoSize = true,
                Location = new Point(370, 280)
            };
            saveButton.Click += Save;

            Controls.Add(titleLabel);
            Controls.Add(description);
            Controls.Add(kindLabel);
            Controls.Add(kindBox);
            Controls.Add(baseUrlLabel);
            Controls.Add(baseUrlBox);
            Controls.Add(modelLabel);
            Controls.Add(modelBox);
            Controls.Add(apiKeyLabel);
            Controls.Add(apiKeyBox);
            Controls.Add(errorLabel);
            Controls.Add(clearButton);
            Controls.Add(cancelButton);
            Controls.Add(saveButton);
            AcceptButton = saveButton;
            CancelButton = cancelButton;

            RefreshFields();
        }

        private static Label MakeRowLabel(string text, int top)
        {
            return new Label
            {
                Text = text,
                AutoSize = true,
                TextAlign = ContentAlignment.MiddleRight,
                Location = new Point(12, top),
                Size = new Size(90, 20)
            };
        }

        protected override bool ShowWithoutActivation
        {
            get { return false; }
        }

        private void RefreshFields()
        {
            PolishConfiguration configuration = PolishConfiguration.Load();
            kindBox.SelectedIndex = configuration.Kind == PolishProtocolKind.Anthropic ? 1 : 0;
            baseUrlBox.Text = configuration.BaseUrl ?? "";
            modelBox.Text = configuration.Model ?? "";
            apiKeyBox.Text = "";
            errorLabel.Text = "";
        }

        private void Save(object sender, EventArgs e)
        {
            var kind = kindBox.SelectedIndex == 1
                ? PolishProtocolKind.Anthropic
                : PolishProtocolKind.OpenAICompatible;
            string baseUrl = baseUrlBox.Text.Trim();
            string model = modelBox.Text.Trim();
            string enteredKey = apiKeyBox.Text;
            // 已保存过 Key 时，输入框留空表示「沿用原值」，而不是清空——
            // 清空配置一律走「清除配置」按钮。
            string apiKey = enteredKey.Length == 0 ? PolishApiKeyStore.Load() : enteredKey;

            Uri parsed;
            if (baseUrl.Length == 0
                || !Uri.TryCreate(baseUrl, UriKind.Absolute, out parsed)
                || (parsed.Scheme != Uri.UriSchemeHttp && parsed.Scheme != Uri.UriSchemeHttps))
            {
                errorLabel.Text = "请输入有效的 Base URL（以 http/https 开头）";
                return;
            }
            if (model.Length == 0)
            {
                errorLabel.Text = "请输入模型名称";
                return;
            }
            if (apiKey.Length == 0)
            {
                errorLabel.Text = "请输入 API Key";
                return;
            }

            new PolishConfiguration
            {
                Kind = kind,
                BaseUrl = baseUrl,
                Model = model,
                ApiKey = apiKey
            }.Save();
            Close();
        }
    }
}
