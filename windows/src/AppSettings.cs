using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace AppSnapshot
{
    /// <summary>
    /// 应用设置：普通字段以 key=value 行存于 %APPDATA%\AppSnapshot\settings.txt；
    /// 润色 API Key 单独存 Windows 凭据管理器（不落盘），对应 macOS 端的 Keychain。
    /// </summary>
    internal static class AppSettings
    {
        private static readonly object SyncRoot = new object();

        private static readonly string DirectoryPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "AppSnapshot");

        private static readonly string FilePath = Path.Combine(DirectoryPath, "settings.txt");

        /// <summary>应用数据目录(%APPDATA%\AppSnapshot);桌宠等可选素材放在其子目录下。</summary>
        public static string DataDirectory
        {
            get { return DirectoryPath; }
        }

        public static string Read(string key)
        {
            lock (SyncRoot)
            {
                if (!File.Exists(FilePath))
                {
                    return null;
                }
                try
                {
                    foreach (string rawLine in File.ReadAllLines(FilePath))
                    {
                        string line = rawLine == null ? "" : rawLine.Trim();
                        if (line.Length == 0 || line.StartsWith("#"))
                        {
                            continue;
                        }
                        int separator = line.IndexOf('=');
                        if (separator <= 0)
                        {
                            continue;
                        }
                        if (line.Substring(0, separator).Trim() == key)
                        {
                            return line.Substring(separator + 1).Trim();
                        }
                    }
                }
                catch
                {
                }
                return null;
            }
        }

        public static void Write(string key, string value)
        {
            lock (SyncRoot)
            {
                var lines = new System.Collections.Generic.List<string>();
                if (File.Exists(FilePath))
                {
                    try
                    {
                        lines.AddRange(File.ReadAllLines(FilePath));
                    }
                    catch
                    {
                    }
                }

                bool replaced = false;
                for (int i = 0; i < lines.Count; i++)
                {
                    string line = lines[i] == null ? "" : lines[i].Trim();
                    int separator = line.IndexOf('=');
                    if (separator > 0 && line.Substring(0, separator).Trim() == key)
                    {
                        lines[i] = key + "=" + value;
                        replaced = true;
                        break;
                    }
                }
                if (!replaced)
                {
                    lines.Add(key + "=" + value);
                }

                try
                {
                    Directory.CreateDirectory(DirectoryPath);
                    File.WriteAllLines(FilePath, lines.ToArray(), Encoding.UTF8);
                }
                catch
                {
                }
            }
        }

        /// <summary>
        /// 录制文件保存目录：默认「下载」，可被设置覆盖；目录失效时自动回退。
        /// </summary>
        public static string SaveDirectory
        {
            get
            {
                string configured = Read("recording.saveDirectory");
                if (!string.IsNullOrEmpty(configured))
                {
                    try
                    {
                        if (Directory.Exists(configured))
                        {
                            return configured;
                        }
                        Directory.CreateDirectory(configured);
                        return configured;
                    }
                    catch
                    {
                    }
                }
                return DefaultDirectory;
            }
            set
            {
                Write("recording.saveDirectory", value);
            }
        }

        private static string DefaultDirectory
        {
            get
            {
                string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
                if (!string.IsNullOrEmpty(profile))
                {
                    return Path.Combine(profile, "Downloads");
                }
                return Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
            }
        }
    }

    /// <summary>润色 API Key 的凭据管理器存取。</summary>
    internal static class PolishApiKeyStore
    {
        private const string Target = "AppSnapshot.Polish.ApiKey";

        public static string Load()
        {
            IntPtr credentialPtr;
            if (!NativeMethods.CredRead(Target, NativeMethods.CredTypeGeneric, 0, out credentialPtr))
            {
                return "";
            }

            try
            {
                var credential = (NativeMethods.Credential)Marshal.PtrToStructure(
                    credentialPtr, typeof(NativeMethods.Credential));
                if (credential.CredentialBlob == IntPtr.Zero || credential.CredentialBlobSize == 0)
                {
                    return "";
                }
                byte[] blob = new byte[credential.CredentialBlobSize];
                Marshal.Copy(credential.CredentialBlob, blob, 0, blob.Length);
                return Encoding.Unicode.GetString(blob);
            }
            catch
            {
                return "";
            }
            finally
            {
                NativeMethods.CredFree(credentialPtr);
            }
        }

        public static void Save(string apiKey)
        {
            Delete();
            if (string.IsNullOrEmpty(apiKey))
            {
                return;
            }

            byte[] blob = Encoding.Unicode.GetBytes(apiKey);
            IntPtr blobPointer = Marshal.AllocHGlobal(blob.Length);
            try
            {
                Marshal.Copy(blob, 0, blobPointer, blob.Length);
                var credential = new NativeMethods.Credential
                {
                    Flags = 0,
                    Type = (int)NativeMethods.CredTypeGeneric,
                    TargetName = Target,
                    Comment = null,
                    CredentialBlobSize = (uint)blob.Length,
                    CredentialBlob = blobPointer,
                    Persist = (int)NativeMethods.CredPersistLocalMachine,
                    AttributeCount = 0,
                    Attributes = IntPtr.Zero,
                    TargetAlias = null,
                    UserName = Environment.UserName
                };
                NativeMethods.CredWrite(ref credential, 0);
            }
            finally
            {
                Marshal.FreeHGlobal(blobPointer);
            }
        }

        public static void Delete()
        {
            NativeMethods.CredDelete(Target, NativeMethods.CredTypeGeneric, 0);
        }
    }
}
