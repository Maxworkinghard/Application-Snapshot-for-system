using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 统一设置窗口：全局快捷键绑定（可选，未绑定即不启用）+ 润色 LLM Provider。
    /// 悬浮球右键「设置…」与托盘菜单「设置…」都打开此窗口。
    /// API Key 保存在 Windows 凭据管理器中（输入框留空表示沿用已保存的 Key）。
    /// </summary>
    internal sealed class SettingsForm : Form
    {
        private static readonly string[] ShortcutNames =
        {
            "截取当前应用窗口",
            "录制当前应用窗口",
            "截取上一个应用窗口",
            "润色提示词"
        };

        private readonly HotKeyBox[] _boxes = new HotKeyBox[4];
        private ComboBox _kindBox;
        private TextBox _baseUrlBox;
        private TextBox _modelBox;
        private TextBox _apiKeyBox;
        private Label _errorLabel;
        private ComboBox _promptBox;
        private Button _deletePromptButton;

        public SettingsForm()
        {
            Text = "应用快照设置";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;
            ShowInTaskbar = false;
            Font = new Font("Microsoft YaHei UI", 9F);
            ClientSize = new Size(500, 702);
            AutoScaleMode = AutoScaleMode.Dpi;

            var titleLabel = new Label
            {
                Text = "设置",
                Font = new Font("Microsoft YaHei UI", 13.5F, FontStyle.Bold),
                AutoSize = true,
                Location = new Point(24, 20)
            };

            // ---- 快捷键区块 ----

            var shortcutHeader = new Label
            {
                Text = "快捷键",
                Font = new Font("Microsoft YaHei UI", 10.5F, FontStyle.Bold),
                AutoSize = true,
                Location = new Point(24, 56)
            };

            var shortcutHint = new Label
            {
                Text = "点击组合键框后按下新组合；「×」清除绑定（留空即不启用）",
                AutoSize = false,
                Size = new Size(452, 18),
                Location = new Point(24, 78),
                ForeColor = SystemColors.GrayText
            };

            int top = 102;
            for (int i = 0; i < ShortcutNames.Length; i++)
            {
                var label = new Label
                {
                    Text = ShortcutNames[i],
                    AutoSize = true,
                    TextAlign = ContentAlignment.MiddleRight,
                    Size = new Size(150, 20),
                    Location = new Point(12, top + 5)
                };

                var box = new HotKeyBox
                {
                    Location = new Point(170, top),
                    Size = new Size(268, 25)
                };
                _boxes[i] = box;

                var clearButton = new Button
                {
                    Text = "×",
                    Font = new Font("Microsoft YaHei UI", 11F),
                    AutoSize = false,
                    Size = new Size(32, 25),
                    Location = new Point(444, top - 1),
                    FlatStyle = FlatStyle.Flat,
                    TabStop = false
                };
                clearButton.Click += delegate { box.ClearSpec(); };

                Controls.Add(label);
                Controls.Add(box);
                Controls.Add(clearButton);
                top += 38;
            }

            var divider = new Label
            {
                AutoSize = false,
                Size = new Size(452, 2),
                BackColor = SystemColors.ControlLight,
                Location = new Point(24, top + 4)
            };

            // ---- 润色服务区块 ----

            int polishTop = top + 18;
            var polishHeader = new Label
            {
                Text = "润色服务（LLM Provider）",
                Font = new Font("Microsoft YaHei UI", 10.5F, FontStyle.Bold),
                AutoSize = true,
                Location = new Point(24, polishTop)
            };

            var polishHint = new Label
            {
                Text = "三项都填写后，「润色」才会调用所配置的模型；未配置时润色会提示先完成配置。",
                AutoSize = false,
                Size = new Size(452, 18),
                Location = new Point(24, polishTop + 22),
                ForeColor = SystemColors.GrayText
            };

            // ---- 润色提示词（内置 + 自定义，可切换不替换）----

            int row = polishTop + 46;
            _promptBox = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Location = new Point(170, row - 3),
                Size = new Size(150, 25)
            };
            _promptBox.SelectedIndexChanged += delegate
            {
                string selected = _promptBox.SelectedItem as string;
                if (selected != null)
                {
                    PolishPromptLibrary.SetActive(selected);
                    UpdatePromptButtons();
                }
            };
            Controls.Add(MakeRowLabel("提示词", row));
            Controls.Add(_promptBox);

            var newPromptButton = new Button
            {
                Text = "新建",
                AutoSize = false,
                Size = new Size(48, 25),
                Location = new Point(326, row - 3),
                FlatStyle = FlatStyle.Flat
            };
            newPromptButton.Click += delegate { EditPrompt(null, ""); };
            Controls.Add(newPromptButton);

            var editPromptButton = new Button
            {
                Text = "编辑",
                AutoSize = false,
                Size = new Size(48, 25),
                Location = new Point(378, row - 3),
                FlatStyle = FlatStyle.Flat
            };
            editPromptButton.Click += delegate { EditSelectedPrompt(); };
            Controls.Add(editPromptButton);

            _deletePromptButton = new Button
            {
                Text = "删除",
                AutoSize = false,
                Size = new Size(48, 25),
                Location = new Point(430, row - 3),
                FlatStyle = FlatStyle.Flat
            };
            _deletePromptButton.Click += delegate { DeleteSelectedPrompt(); };
            Controls.Add(_deletePromptButton);

            var promptHint = new Label
            {
                Text = "切换即生效。选「内置」点「编辑」可基于内置文本另存自定义版本。",
                AutoSize = false,
                Size = new Size(452, 18),
                Location = new Point(24, row + 26),
                ForeColor = SystemColors.GrayText
            };
            Controls.Add(promptHint);

            row += 56;
            _kindBox = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Location = new Point(170, row - 3),
                Size = new Size(306, 25)
            };
            _kindBox.Items.Add("OpenAI 兼容");
            _kindBox.Items.Add("Anthropic");
            _kindBox.SelectedIndex = 0;
            Controls.Add(MakeRowLabel("协议", row));
            Controls.Add(_kindBox);

            row += 38;
            _baseUrlBox = new TextBox
            {
                Location = new Point(170, row - 3),
                Size = new Size(306, 25)
            };
            Controls.Add(MakeRowLabel("Base URL", row));
            Controls.Add(_baseUrlBox);

            row += 38;
            _modelBox = new TextBox
            {
                Location = new Point(170, row - 3),
                Size = new Size(306, 25)
            };
            Controls.Add(MakeRowLabel("模型", row));
            Controls.Add(_modelBox);

            row += 38;
            _apiKeyBox = new TextBox
            {
                Location = new Point(170, row - 3),
                Size = new Size(306, 25),
                UseSystemPasswordChar = true
            };
            Controls.Add(MakeRowLabel("API Key", row));
            Controls.Add(_apiKeyBox);

            row += 36;
            _errorLabel = new Label
            {
                Text = "",
                AutoSize = false,
                Size = new Size(452, 18),
                Location = new Point(24, row),
                ForeColor = Color.Firebrick
            };

            int buttonTop = row + 26;
            var clearPolishButton = new Button
            {
                Text = "清除润色配置",
                AutoSize = true,
                Location = new Point(24, buttonTop)
            };
            clearPolishButton.Click += delegate
            {
                PolishConfiguration.Clear();
                RefreshPolishFields();
                _errorLabel.Text = "润色配置已清除";
            };

            var cancelButton = new Button
            {
                Text = "取消",
                AutoSize = true,
                Location = new Point(320, buttonTop)
            };
            cancelButton.Click += delegate { Close(); };

            var saveButton = new Button
            {
                Text = "保存",
                AutoSize = true,
                Location = new Point(400, buttonTop)
            };
            saveButton.Click += Save;

            Controls.Add(titleLabel);
            Controls.Add(shortcutHeader);
            Controls.Add(shortcutHint);
            Controls.Add(divider);
            Controls.Add(polishHeader);
            Controls.Add(polishHint);
            Controls.Add(_errorLabel);
            Controls.Add(clearPolishButton);
            Controls.Add(cancelButton);
            Controls.Add(saveButton);
            AcceptButton = saveButton;
            CancelButton = cancelButton;

            RefreshShortcutFields();
            RefreshPolishFields();
            RefreshPromptFields();
        }

        private static Label MakeRowLabel(string text, int top)
        {
            return new Label
            {
                Text = text,
                AutoSize = true,
                TextAlign = ContentAlignment.MiddleRight,
                Location = new Point(12, top),
                Size = new Size(150, 20)
            };
        }

        private void Save(object sender, EventArgs e)
        {
            HotKeySpec capture = _boxes[0].Spec;
            HotKeySpec record = _boxes[1].Spec;
            HotKeySpec previousApp = _boxes[2].Spec;
            HotKeySpec polish = _boxes[3].Spec;

            var seen = new List<string>();
            var specs = new[] { capture, record, previousApp, polish };
            for (int i = 0; i < specs.Length; i++)
            {
                if (specs[i] == null)
                {
                    continue;
                }
                if (seen.Contains(specs[i].DisplayName))
                {
                    _errorLabel.Text = "存在重复的快捷键组合，请调整";
                    return;
                }
                seen.Add(specs[i].DisplayName);
            }

            int[] failures = App.HotKeys.Apply(capture, record, previousApp, polish);
            if (failures != null)
            {
                var occupied = new List<string>();
                foreach (int index in failures)
                {
                    occupied.Add(ShortcutNames[index] + "（" + specs[index].DisplayName + "）");
                }
                _errorLabel.Text = string.Join("；", occupied.ToArray()) + " 已被其他应用占用";
                return;
            }

            HotKeyPreferences.Save(capture, record, previousApp, polish);
            App.Tray.RefreshShortcuts();

            string error = ValidateAndSavePolish();
            if (error != null)
            {
                _errorLabel.Text = error;
                return;
            }
            Close();
        }

        /// <summary>润色配置校验：全空视为「不启用」直接放行；填了部分则要求完整。</summary>
        private string ValidateAndSavePolish()
        {
            var kind = _kindBox.SelectedIndex == 1
                ? PolishProtocolKind.Anthropic
                : PolishProtocolKind.OpenAICompatible;
            string baseUrl = _baseUrlBox.Text.Trim();
            string model = _modelBox.Text.Trim();
            string enteredKey = _apiKeyBox.Text;

            PolishConfiguration current = PolishConfiguration.Load();
            string existingKey = PolishApiKeyStore.Load();
            bool providerConfigured = current != null && !string.IsNullOrEmpty(current.BaseUrl);
            bool allEmpty = baseUrl.Length == 0 && model.Length == 0
                && enteredKey.Length == 0
                && existingKey.Length == 0 && !providerConfigured;
            if (allEmpty)
            {
                return null;
            }

            Uri parsed;
            if (baseUrl.Length == 0
                || !Uri.TryCreate(baseUrl, UriKind.Absolute, out parsed)
                || (parsed.Scheme != Uri.UriSchemeHttp && parsed.Scheme != Uri.UriSchemeHttps))
            {
                return "请输入有效的 Base URL（以 http/https 开头）";
            }
            if (model.Length == 0)
            {
                return "请输入模型名称";
            }
            // 已保存过 Key 时，输入框留空表示「沿用原值」；清空配置一律走「清除润色配置」按钮。
            string apiKey = enteredKey.Length == 0 ? existingKey : enteredKey;
            if (apiKey.Length == 0)
            {
                return "请输入 API Key";
            }

            new PolishConfiguration
            {
                Kind = kind,
                BaseUrl = baseUrl,
                Model = model,
                ApiKey = apiKey
            }.Save();
            return null;
        }

        private void RefreshShortcutFields()
        {
            _boxes[0].Spec = HotKeyPreferences.Capture;
            _boxes[1].Spec = HotKeyPreferences.Record;
            _boxes[2].Spec = HotKeyPreferences.PreviousApp;
            _boxes[3].Spec = HotKeyPreferences.Polish;
        }

        private void RefreshPolishFields()
        {
            PolishConfiguration configuration = PolishConfiguration.Load();
            _kindBox.SelectedIndex = configuration != null && configuration.Kind == PolishProtocolKind.Anthropic ? 1 : 0;
            _baseUrlBox.Text = configuration != null ? (configuration.BaseUrl ?? "") : "";
            _modelBox.Text = configuration != null ? (configuration.Model ?? "") : "";
            _apiKeyBox.Text = "";
            _errorLabel.Text = "";
        }

        // ---- 润色提示词管理（切换即时生效，不依赖「保存」按钮）----

        private void RefreshPromptFields()
        {
            _promptBox.Items.Clear();
            _promptBox.Items.Add(PolishPromptLibrary.BuiltinName);
            foreach (PolishPromptLibrary.CustomPrompt prompt in PolishPromptLibrary.Custom)
            {
                _promptBox.Items.Add(prompt.Name);
            }
            _promptBox.SelectedItem = PolishPromptLibrary.ActiveName;
            UpdatePromptButtons();
        }

        private void UpdatePromptButtons()
        {
            string selected = _promptBox.SelectedItem as string;
            _deletePromptButton.Enabled = selected != null && selected != PolishPromptLibrary.BuiltinName;
        }

        private void EditSelectedPrompt()
        {
            string name = _promptBox.SelectedItem as string;
            if (name == null)
            {
                return;
            }
            if (name == PolishPromptLibrary.BuiltinName)
            {
                EditPrompt(null, PolishPrompt.SystemPrompt);
                return;
            }
            PolishPromptLibrary.CustomPrompt original = PolishPromptLibrary.Custom.Find(
                delegate (PolishPromptLibrary.CustomPrompt p) { return p.Name == name; });
            if (original != null)
            {
                EditPrompt(original, original.Text);
            }
        }

        private void EditPrompt(PolishPromptLibrary.CustomPrompt original, string prefill)
        {
            using (var editor = new PolishPromptEditorForm(original, prefill))
            {
                if (editor.ShowDialog(this) != DialogResult.OK)
                {
                    return;
                }
                bool isNew = original == null;
                if (PolishPromptLibrary.Save(editor.Prompt, original == null ? null : original.Name) && isNew)
                {
                    PolishPromptLibrary.SetActive(editor.Prompt.Name.Trim());
                }
                RefreshPromptFields();
            }
        }

        private void DeleteSelectedPrompt()
        {
            string name = _promptBox.SelectedItem as string;
            if (name == null || name == PolishPromptLibrary.BuiltinName)
            {
                return;
            }
            if (MessageBox.Show(this,
                "删除提示词「" + name + "」？\n删除后不可恢复；若它是当前使用的提示词，将切回内置。",
                "应用快照", MessageBoxButtons.YesNo, MessageBoxIcon.Question) == DialogResult.Yes)
            {
                PolishPromptLibrary.Delete(name);
                RefreshPromptFields();
            }
        }

        /// <summary>快捷键录入框：聚焦后按下组合键即记录；无修饰键的组合忽略。</summary>
        private sealed class HotKeyBox : TextBox
        {
            internal HotKeySpec Spec;

            internal HotKeyBox()
            {
                ShowUnbound();
            }

            internal void ClearSpec()
            {
                Spec = null;
                ShowUnbound();
            }

            protected override void OnGotFocus(EventArgs e)
            {
                base.OnGotFocus(e);
                Text = Spec != null ? Spec.DisplayName : "按下组合键…";
                ForeColor = SystemColors.WindowText;
            }

            protected override void OnLostFocus(EventArgs e)
            {
                base.OnLostFocus(e);
                if (Spec != null)
                {
                    Text = Spec.DisplayName;
                    ForeColor = SystemColors.WindowText;
                }
                else
                {
                    ShowUnbound();
                }
            }

            protected override void OnKeyDown(KeyEventArgs e)
            {
                base.OnKeyDown(e);
                Keys key = e.KeyCode;
                bool modifierOnly = key == Keys.ControlKey || key == Keys.Menu || key == Keys.ShiftKey;
                if (modifierOnly)
                {
                    return;
                }

                uint modifiers = 0;
                if (e.Alt)
                {
                    modifiers |= NativeMethods.ModAlt;
                }
                if (e.Control)
                {
                    modifiers |= NativeMethods.ModControl;
                }
                if (e.Shift)
                {
                    modifiers |= NativeMethods.ModShift;
                }
                if (modifiers == 0)
                {
                    return; // Tab / Enter 等无修饰键交还窗体导航
                }

                e.SuppressKeyPress = true;
                e.Handled = true;
                Spec = HotKeySpec.Create(modifiers, (uint)key);
                Text = Spec.DisplayName;
                ForeColor = SystemColors.WindowText;
            }

            private void ShowUnbound()
            {
                Text = "未设置";
                ForeColor = SystemColors.GrayText;
            }
        }
    }
}
