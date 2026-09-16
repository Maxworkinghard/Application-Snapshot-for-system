import AppKit

/// 桌宠动画状态；waving 预留，暂无触发入口（与 Windows 保持一致）。
enum PetPose: Int, CaseIterable {
    case idle
    case waving
    case jumping
    case failed
    case waiting
    case runningLeft
    case runningRight

    var fileName: String {
        switch self {
        case .idle: return "idle"
        case .waving: return "waving"
        case .jumping: return "jumping"
        case .failed: return "failed"
        case .waiting: return "waiting"
        case .runningLeft: return "running-left"
        case .runningRight: return "running-right"
        }
    }

    /// 循环动画持续播放；一次性动画播完回 idle。
    var loops: Bool {
        switch self {
        case .idle, .runningLeft, .runningRight: return true
        case .waving, .jumping, .failed, .waiting: return false
        }
    }
}

/// 桌宠素材目录：~/Library/Application Support/WindowSnap/pet/<形象>/idle.gif 等。
/// 素材不随应用内置或分发（版权考虑），由用户自行放入；目录为空时桌宠模式不可用，
/// 因此这里只拼路径、从不创建目录。
enum PetAssets {
    static var root: URL {
        let base = try? FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: false
        )
        guard let base else {
            return URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Library/Application Support/WindowSnap/pet")
        }
        return base.appendingPathComponent("WindowSnap/pet")
    }

    /// 可用形象 = pet 目录下含至少一个 gif 的子目录，按名称排序（忽略大小写）。
    static func availableSkins() -> [String] {
        var skins: [String] = []
        let manager = FileManager.default
        guard let entries = try? manager.contentsOfDirectory(
            at: root,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: []
        ) else {
            return skins
        }
        for entry in entries {
            guard (try? entry.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory == true else {
                continue
            }
            let gifs = (try? manager.contentsOfDirectory(at: entry, includingPropertiesForKeys: [.isRegularFileKey]))?
                .contains { $0.pathExtension.lowercased() == "gif" &&
                    (try? $0.resourceValues(forKeys: [.isRegularFileKey]))?.isRegularFile == true } ?? false
            if gifs {
                skins.append(entry.lastPathComponent)
            }
        }
        return skins.sorted { $0.caseInsensitiveCompare($1) == .orderedAscending }
    }

    static func posePath(skin: String, pose: PetPose) -> URL {
        root.appendingPathComponent(skin).appendingPathComponent(pose.fileName + ".gif")
    }
}

/// 桌面呈现形式：悬浮球（默认）或桌宠，与形象一起持久化；键名与 Windows / Linux 一致。
final class DesktopModeStore {
    private let modeKey = "ui.mode"
    private let skinKey = "pet.skin"

    var isPetMode: Bool {
        get { UserDefaults.standard.string(forKey: modeKey) == "pet" }
        set { UserDefaults.standard.set(newValue ? "pet" : "bubble", forKey: modeKey) }
    }

    var skin: String {
        get { UserDefaults.standard.string(forKey: skinKey) ?? "" }
        set { UserDefaults.standard.set(newValue, forKey: skinKey) }
    }

    /// 可用形象中解析当前选择；没有任何素材时返回 nil（桌宠不可用）。
    func resolveSkin() -> String? {
        let skins = PetAssets.availableSkins()
        if skins.contains(skin) {
            return skin
        }
        return skins.first
    }
}
