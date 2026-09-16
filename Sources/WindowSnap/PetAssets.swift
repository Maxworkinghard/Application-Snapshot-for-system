import AppKit

/// 桌宠动画状态；running / review 两个动作预留，待接入更多事件钩子。
/// 动作名与 Windows / Linux 端一致，同一套素材目录三端可直接复用。
enum PetPose: String {
    case idle = "idle"
    case waving = "waving"
    case jumping = "jumping"
    case failed = "failed"
    case waiting = "waiting"
    case runningLeft = "running-left"
    case runningRight = "running-right"

    static let all: [PetPose] = [.idle, .waving, .jumping, .failed, .waiting, .runningLeft, .runningRight]

    /// idle 与跑动持续循环，其余单次动画播完回 idle。
    var loops: Bool {
        switch self {
        case .idle, .runningLeft, .runningRight:
            return true
        case .waving, .jumping, .failed, .waiting:
            return false
        }
    }
}

/// 一个动作的 GIF。NSBitmapImageRep 自带 GIF 逐帧合成（含 disposal 处理），
/// 这里只记录每帧延迟，绘制前把 rep 定位到对应帧即可。
final class PetClip {
    let rep: NSBitmapImageRep
    let delays: [TimeInterval]
    let loops: Bool
    let pixelSize: NSSize

    var frameCount: Int { delays.count }

    init?(url: URL, loops: Bool) {
        guard let data = try? Data(contentsOf: url),
              let rep = NSBitmapImageRep(data: data) else {
            return nil
        }
        let count = (rep.value(forProperty: .frameCount) as? NSNumber)?.intValue ?? 1
        guard count > 0, rep.pixelsWide > 0, rep.pixelsHigh > 0 else {
            return nil
        }

        var delays: [TimeInterval] = []
        for index in 0..<count {
            rep.setProperty(.currentFrame, withValue: NSNumber(value: index))
            let raw = (rep.value(forProperty: .currentFrameDuration) as? NSNumber)?.doubleValue ?? 0.1
            // GIF 里 0 或过小的延迟按浏览器惯例抬到 20ms，与 Windows 端同规则
            delays.append(max(0.02, raw))
        }
        rep.setProperty(.currentFrame, withValue: NSNumber(value: 0))

        self.rep = rep
        self.delays = delays
        self.loops = loops
        self.pixelSize = NSSize(width: rep.pixelsWide, height: rep.pixelsHigh)
    }

    func seek(to index: Int) {
        guard index >= 0, index < frameCount else { return }
        rep.setProperty(.currentFrame, withValue: NSNumber(value: index))
    }
}

/// 桌宠素材目录：~/Library/Application Support/AppSnapshot/pet/<形象>/idle.gif 等。
/// 素材不随应用内置或分发（素材并非本项目制作，避免版权问题），由用户自行放入；
/// 目录为空时桌宠模式不可用，启动与切换都回退悬浮窗。
enum PetAssets {
    static var root: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support")
        return base
            .appendingPathComponent("AppSnapshot", isDirectory: true)
            .appendingPathComponent("pet", isDirectory: true)
    }

    /// 可用形象 = pet 目录下含至少一个 gif 的子目录，按名称排序。
    static func availableSkins() -> [String] {
        let manager = FileManager.default
        guard let entries = try? manager.contentsOfDirectory(
            at: root,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return entries
            .filter { url in
                var isDirectory: ObjCBool = false
                guard manager.fileExists(atPath: url.path, isDirectory: &isDirectory), isDirectory.boolValue else {
                    return false
                }
                let files = (try? manager.contentsOfDirectory(atPath: url.path)) ?? []
                return files.contains { $0.lowercased().hasSuffix(".gif") }
            }
            .map(\.lastPathComponent)
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    static func poseURL(skin: String, pose: PetPose) -> URL {
        root
            .appendingPathComponent(skin, isDirectory: true)
            .appendingPathComponent(pose.rawValue + ".gif")
    }

    /// 载入一个形象的全部动作；缺失的动作留空，调用方自行退化。
    static func loadClips(skin: String) -> [PetPose: PetClip] {
        var clips: [PetPose: PetClip] = [:]
        for pose in PetPose.all {
            if let clip = PetClip(url: poseURL(skin: skin, pose: pose), loops: pose.loops) {
                clips[pose] = clip
            }
        }
        return clips
    }
}

/// 桌面呈现形式：悬浮窗（默认）或桌宠，两者互斥。
enum DesktopForm: String {
    case bubble
    case pet
}

/// 形式与形象的持久化，键名与 Windows 端 settings.txt 的 ui.mode / pet.skin 保持一致。
enum DesktopFormStore {
    private static let modeKey = "ui.mode"
    private static let skinKey = "pet.skin"

    static var form: DesktopForm {
        get { DesktopForm(rawValue: UserDefaults.standard.string(forKey: modeKey) ?? "") ?? .bubble }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: modeKey) }
    }

    static var skin: String? {
        get { UserDefaults.standard.string(forKey: skinKey) }
        set { UserDefaults.standard.set(newValue, forKey: skinKey) }
    }

    /// 可用形象中解析当前选择；没有任何素材时返回 nil（桌宠不可用）。
    static func resolvedSkin() -> String? {
        let skins = PetAssets.availableSkins()
        if let skin, skins.contains(skin) {
            return skin
        }
        return skins.first
    }
}
