using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Web.Script.Serialization;

namespace AppSnapshot
{
    /// <summary>
    /// 润色提示词库：内置提示词常驻可选，用户自定义提示词存
    /// %APPDATA%\AppSnapshot\prompts.json，可切换不替换；润色调用时实时读取。
    /// </summary>
    internal static class PolishPromptLibrary
    {
        public const string BuiltinName = "内置";

        private static readonly object SyncRoot = new object();

        private static readonly string DirectoryPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "AppSnapshot");

        private static readonly string FilePath = Path.Combine(DirectoryPath, "prompts.json");

        public sealed class CustomPrompt
        {
            public string Name { get; set; }
            public string Text { get; set; }
        }

        private sealed class PromptFile
        {
            public string Active { get; set; }
            public List<CustomPrompt> Custom { get; set; }
        }

        public static List<CustomPrompt> Custom
        {
            get
            {
                lock (SyncRoot)
                {
                    return LoadFile().Custom ?? new List<CustomPrompt>();
                }
            }
        }

        /// <summary>当前使用的名称（激活项被删后回退「内置」）。</summary>
        public static string ActiveName
        {
            get
            {
                lock (SyncRoot)
                {
                    PromptFile file = LoadFile();
                    string active = file.Active ?? "";
                    if (active.Length > 0 && file.Custom != null
                        && file.Custom.Exists(delegate (CustomPrompt p) { return p.Name == active; }))
                    {
                        return active;
                    }
                    return BuiltinName;
                }
            }
        }

        /// <summary>当前生效的系统提示词：激活的自定义项，缺失时回退内置。</summary>
        public static string ActiveText
        {
            get
            {
                lock (SyncRoot)
                {
                    PromptFile file = LoadFile();
                    string active = file.Active ?? "";
                    if (active.Length > 0 && file.Custom != null)
                    {
                        CustomPrompt match = file.Custom.Find(delegate (CustomPrompt p) { return p.Name == active; });
                        if (match != null)
                        {
                            return match.Text ?? "";
                        }
                    }
                    return PolishPrompt.SystemPrompt;
                }
            }
        }

        public static void SetActive(string name)
        {
            lock (SyncRoot)
            {
                PromptFile file = LoadFile();
                file.Active = name == BuiltinName ? "" : name;
                WriteFile(file);
            }
        }

        /// <summary>新增或按原名更新。返回 false 表示名称为空、与内置重名或与其他自定义重名。</summary>
        public static bool Save(CustomPrompt prompt, string originalName)
        {
            lock (SyncRoot)
            {
                string name = (prompt.Name ?? "").Trim();
                if (name.Length == 0 || name == BuiltinName)
                {
                    return false;
                }

                PromptFile file = LoadFile();
                if (file.Custom == null)
                {
                    file.Custom = new List<CustomPrompt>();
                }

                int originalIndex = originalName == null ? -1
                    : file.Custom.FindIndex(delegate (CustomPrompt p) { return p.Name == originalName; });
                if (originalIndex >= 0)
                {
                    int conflict = file.Custom.FindIndex(delegate (CustomPrompt p) { return p.Name == name; });
                    if (name != originalName && conflict >= 0)
                    {
                        return false;
                    }
                    bool wasActive = (file.Active ?? "") == originalName;
                    file.Custom[originalIndex] = new CustomPrompt { Name = name, Text = prompt.Text };
                    if (wasActive)
                    {
                        file.Active = name;
                    }
                    WriteFile(file);
                    return true;
                }

                if (file.Custom.Exists(delegate (CustomPrompt p) { return p.Name == name; }))
                {
                    return false;
                }
                file.Custom.Add(new CustomPrompt { Name = name, Text = prompt.Text });
                WriteFile(file);
                return true;
            }
        }

        public static void Delete(string name)
        {
            lock (SyncRoot)
            {
                PromptFile file = LoadFile();
                if (file.Custom == null)
                {
                    return;
                }
                bool wasActive = (file.Active ?? "") == name;
                file.Custom.RemoveAll(delegate (CustomPrompt p) { return p.Name == name; });
                if (wasActive)
                {
                    file.Active = "";
                }
                WriteFile(file);
            }
        }

        private static PromptFile LoadFile()
        {
            try
            {
                if (File.Exists(FilePath))
                {
                    string json = File.ReadAllText(FilePath, Encoding.UTF8);
                    PromptFile file = new JavaScriptSerializer().Deserialize<PromptFile>(json);
                    if (file != null)
                    {
                        if (file.Custom == null)
                        {
                            file.Custom = new List<CustomPrompt>();
                        }
                        return file;
                    }
                }
            }
            catch
            {
            }
            return new PromptFile { Active = "", Custom = new List<CustomPrompt>() };
        }

        private static void WriteFile(PromptFile file)
        {
            try
            {
                Directory.CreateDirectory(DirectoryPath);
                string json = new JavaScriptSerializer().Serialize(file);
                File.WriteAllText(FilePath, json, Encoding.UTF8);
            }
            catch
            {
            }
        }
    }
}
