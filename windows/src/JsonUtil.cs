using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace AppSnapshot
{
    /// <summary>
    /// 替代 .NET Framework 的 JavaScriptSerializer。
    /// 读入时大小写不敏感，写出时保留中文、省略 null，便于沿用已有 prompts.json。
    /// </summary>
    internal static class JsonUtil
    {
        private static readonly JsonSerializerOptions Options = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
        };

        public static string Serialize<T>(T value)
        {
            return JsonSerializer.Serialize(value, Options);
        }

        public static T Deserialize<T>(string json) where T : class
        {
            if (string.IsNullOrWhiteSpace(json))
            {
                return null;
            }
            return JsonSerializer.Deserialize<T>(json, Options);
        }

        public static JsonDocument Parse(string json)
        {
            return JsonDocument.Parse(json);
        }
    }
}
