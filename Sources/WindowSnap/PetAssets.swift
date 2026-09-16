import AppKit

/// 桌宠素材目录：~/Library/Application Support/AppSnapshot/pet/<形象>/idle.gif 等。
/// 素材不随应用内置或分发（版权考虑），由用户自行放入；目录为空时桌宠模式不可用。
/// 目录契约与 Windows（%APPDATA%\AppSnapshot\pet）和 Linux（~/.local/share/windowsnap/pet）端一致。
enum PetAssets {
    static var root: URL {
        FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("AppSnapshot/pet")
    }

    /// 可用形象 = pet 目录下含至少一个 gif 的子目录，按名称排序。
    static func availableSkins() -> [String] {
        let directories: [URL]
        do {
            directories = try FileManager.default.contentsOfDirectory(
                at: root,
                includingPropertiesForKeys: [.isDirectoryKey]
            )
        } catch {
            return []
        }

        return directories
            .filter { entry in
                let values = try? entry.resourceValues(forKeys: [.isDirectoryKey])
                return values?.isDirectory == true
            }
            .filter { directory in
                let entries = (try? FileManager.default.contentsOfDirectory(
                    at: directory,
                    includingPropertiesForKeys: nil
                )) ?? []
                return entries.contains { $0.pathExtension.lowercased() == "gif" }
            }
            .map { $0.lastPathComponent }
            .sorted { $0.compare($1, options: [.caseInsensitive]) == .orderedAscending }
    }

    static func poseURL(skin: String, pose: String) -> URL {
        root.appendingPathComponent(skin).appendingPathComponent("\(pose).gif")
    }

    /// 已保存形象存在则用之，否则回退第一个可用形象；没有素材返回 nil。
    static func resolveSkin() -> String? {
        let skins = availableSkins()
        let saved = UserDefaults.standard.string(forKey: "pet.skin") ?? ""
        return skins.contains(saved) ? saved : skins.first
    }
}
