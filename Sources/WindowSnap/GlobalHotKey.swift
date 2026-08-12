import Carbon
import Foundation
import AppKit

struct ShortcutConfiguration: Equatable {
    let keyCode: UInt32
    let modifiers: UInt32

    static let `default` = ShortcutConfiguration(
        keyCode: UInt32(kVK_ANSI_2),
        modifiers: UInt32(optionKey | shiftKey)
    )

    var displayString: String {
        var result = ""
        if modifiers & UInt32(cmdKey) != 0 { result += "⌘" }
        if modifiers & UInt32(optionKey) != 0 { result += "⌥" }
        if modifiers & UInt32(controlKey) != 0 { result += "⌃" }
        if modifiers & UInt32(shiftKey) != 0 { result += "⇧" }
        return result + keyName
    }

    var menuModifierMask: NSEvent.ModifierFlags {
        var result: NSEvent.ModifierFlags = []
        if modifiers & UInt32(cmdKey) != 0 { result.insert(.command) }
        if modifiers & UInt32(optionKey) != 0 { result.insert(.option) }
        if modifiers & UInt32(controlKey) != 0 { result.insert(.control) }
        if modifiers & UInt32(shiftKey) != 0 { result.insert(.shift) }
        return result
    }

    var menuKeyEquivalent: String {
        switch keyCode {
        case UInt32(kVK_ANSI_A): return "a"
        case UInt32(kVK_ANSI_B): return "b"
        case UInt32(kVK_ANSI_C): return "c"
        case UInt32(kVK_ANSI_D): return "d"
        case UInt32(kVK_ANSI_E): return "e"
        case UInt32(kVK_ANSI_F): return "f"
        case UInt32(kVK_ANSI_G): return "g"
        case UInt32(kVK_ANSI_H): return "h"
        case UInt32(kVK_ANSI_I): return "i"
        case UInt32(kVK_ANSI_J): return "j"
        case UInt32(kVK_ANSI_K): return "k"
        case UInt32(kVK_ANSI_L): return "l"
        case UInt32(kVK_ANSI_M): return "m"
        case UInt32(kVK_ANSI_N): return "n"
        case UInt32(kVK_ANSI_O): return "o"
        case UInt32(kVK_ANSI_P): return "p"
        case UInt32(kVK_ANSI_Q): return "q"
        case UInt32(kVK_ANSI_R): return "r"
        case UInt32(kVK_ANSI_S): return "s"
        case UInt32(kVK_ANSI_T): return "t"
        case UInt32(kVK_ANSI_U): return "u"
        case UInt32(kVK_ANSI_V): return "v"
        case UInt32(kVK_ANSI_W): return "w"
        case UInt32(kVK_ANSI_X): return "x"
        case UInt32(kVK_ANSI_Y): return "y"
        case UInt32(kVK_ANSI_Z): return "z"
        case UInt32(kVK_ANSI_0): return "0"
        case UInt32(kVK_ANSI_1): return "1"
        case UInt32(kVK_ANSI_2): return "2"
        case UInt32(kVK_ANSI_3): return "3"
        case UInt32(kVK_ANSI_4): return "4"
        case UInt32(kVK_ANSI_5): return "5"
        case UInt32(kVK_ANSI_6): return "6"
        case UInt32(kVK_ANSI_7): return "7"
        case UInt32(kVK_ANSI_8): return "8"
        case UInt32(kVK_ANSI_9): return "9"
        case UInt32(kVK_ANSI_Minus): return "-"
        case UInt32(kVK_ANSI_Equal): return "="
        case UInt32(kVK_ANSI_LeftBracket): return "["
        case UInt32(kVK_ANSI_RightBracket): return "]"
        case UInt32(kVK_ANSI_Semicolon): return ";"
        case UInt32(kVK_ANSI_Quote): return "'"
        case UInt32(kVK_ANSI_Comma): return ","
        case UInt32(kVK_ANSI_Period): return "."
        case UInt32(kVK_ANSI_Slash): return "/"
        case UInt32(kVK_ANSI_Backslash): return "\\"
        case UInt32(kVK_ANSI_Grave): return "`"
        case UInt32(kVK_Return): return "\r"
        case UInt32(kVK_Space): return " "
        case UInt32(kVK_Tab): return "\t"
        case UInt32(kVK_Delete): return "\u{8}"
        case UInt32(kVK_Escape): return "\u{1b}"
        default: return ""
        }
    }

    private var keyName: String {
        switch keyCode {
        case UInt32(kVK_ANSI_A): return "A"
        case UInt32(kVK_ANSI_B): return "B"
        case UInt32(kVK_ANSI_C): return "C"
        case UInt32(kVK_ANSI_D): return "D"
        case UInt32(kVK_ANSI_E): return "E"
        case UInt32(kVK_ANSI_F): return "F"
        case UInt32(kVK_ANSI_G): return "G"
        case UInt32(kVK_ANSI_H): return "H"
        case UInt32(kVK_ANSI_I): return "I"
        case UInt32(kVK_ANSI_J): return "J"
        case UInt32(kVK_ANSI_K): return "K"
        case UInt32(kVK_ANSI_L): return "L"
        case UInt32(kVK_ANSI_M): return "M"
        case UInt32(kVK_ANSI_N): return "N"
        case UInt32(kVK_ANSI_O): return "O"
        case UInt32(kVK_ANSI_P): return "P"
        case UInt32(kVK_ANSI_Q): return "Q"
        case UInt32(kVK_ANSI_R): return "R"
        case UInt32(kVK_ANSI_S): return "S"
        case UInt32(kVK_ANSI_T): return "T"
        case UInt32(kVK_ANSI_U): return "U"
        case UInt32(kVK_ANSI_V): return "V"
        case UInt32(kVK_ANSI_W): return "W"
        case UInt32(kVK_ANSI_X): return "X"
        case UInt32(kVK_ANSI_Y): return "Y"
        case UInt32(kVK_ANSI_Z): return "Z"
        case UInt32(kVK_ANSI_0): return "0"
        case UInt32(kVK_ANSI_1): return "1"
        case UInt32(kVK_ANSI_2): return "2"
        case UInt32(kVK_ANSI_3): return "3"
        case UInt32(kVK_ANSI_4): return "4"
        case UInt32(kVK_ANSI_5): return "5"
        case UInt32(kVK_ANSI_6): return "6"
        case UInt32(kVK_ANSI_7): return "7"
        case UInt32(kVK_ANSI_8): return "8"
        case UInt32(kVK_ANSI_9): return "9"
        case UInt32(kVK_ANSI_Minus): return "-"
        case UInt32(kVK_ANSI_Equal): return "="
        case UInt32(kVK_ANSI_LeftBracket): return "["
        case UInt32(kVK_ANSI_RightBracket): return "]"
        case UInt32(kVK_ANSI_Semicolon): return ";"
        case UInt32(kVK_ANSI_Quote): return "'"
        case UInt32(kVK_ANSI_Comma): return ","
        case UInt32(kVK_ANSI_Period): return "."
        case UInt32(kVK_ANSI_Slash): return "/"
        case UInt32(kVK_ANSI_Backslash): return "\\"
        case UInt32(kVK_ANSI_Grave): return "`"
        case UInt32(kVK_Return): return "↩"
        case UInt32(kVK_Space): return "Space"
        case UInt32(kVK_Tab): return "⇥"
        case UInt32(kVK_Delete): return "⌫"
        case UInt32(kVK_Escape): return "Esc"
        case UInt32(kVK_LeftArrow): return "←"
        case UInt32(kVK_RightArrow): return "→"
        case UInt32(kVK_UpArrow): return "↑"
        case UInt32(kVK_DownArrow): return "↓"
        case UInt32(kVK_F1): return "F1"
        case UInt32(kVK_F2): return "F2"
        case UInt32(kVK_F3): return "F3"
        case UInt32(kVK_F4): return "F4"
        case UInt32(kVK_F5): return "F5"
        case UInt32(kVK_F6): return "F6"
        case UInt32(kVK_F7): return "F7"
        case UInt32(kVK_F8): return "F8"
        case UInt32(kVK_F9): return "F9"
        case UInt32(kVK_F10): return "F10"
        case UInt32(kVK_F11): return "F11"
        case UInt32(kVK_F12): return "F12"
        case UInt32(kVK_F13): return "F13"
        case UInt32(kVK_F14): return "F14"
        case UInt32(kVK_F15): return "F15"
        case UInt32(kVK_F16): return "F16"
        case UInt32(kVK_F17): return "F17"
        case UInt32(kVK_F18): return "F18"
        case UInt32(kVK_F19): return "F19"
        case UInt32(kVK_F20): return "F20"
        default: return "键"
        }
    }
}

final class ShortcutStore {
    private let keyCodeKey = "shortcut.keyCode"
    private let modifiersKey = "shortcut.modifiers"

    var configuration: ShortcutConfiguration {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: keyCodeKey) != nil,
              defaults.object(forKey: modifiersKey) != nil else {
            return .default
        }

        return ShortcutConfiguration(
            keyCode: UInt32(defaults.integer(forKey: keyCodeKey)),
            modifiers: UInt32(defaults.integer(forKey: modifiersKey))
        )
    }

    func save(_ configuration: ShortcutConfiguration) {
        let defaults = UserDefaults.standard
        defaults.set(Int(configuration.keyCode), forKey: keyCodeKey)
        defaults.set(Int(configuration.modifiers), forKey: modifiersKey)
    }
}

private let hotKeySignature = fourCharacterCode("WSNP")
private var nextHotKeyID: UInt32 = 1

private let hotKeyEventHandler: EventHandlerUPP = { _, event, userData in
    guard let event, let userData else { return OSStatus(eventNotHandledErr) }

    let hotKey = Unmanaged<GlobalHotKey>.fromOpaque(userData).takeUnretainedValue()
    var identifier = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &identifier
    )
    guard status == noErr, hotKey.matches(identifier) else {
        return OSStatus(eventNotHandledErr)
    }

    DispatchQueue.main.async {
        hotKey.performAction()
    }
    return noErr
}

final class GlobalHotKey {
    private var eventHandler: EventHandlerRef?
    private var hotKey: EventHotKeyRef?
    private let identifier: EventHotKeyID
    private let action: () -> Void

    init?(configuration: ShortcutConfiguration, action: @escaping () -> Void) {
        self.action = action
        self.identifier = EventHotKeyID(signature: hotKeySignature, id: nextHotKeyID)
        nextHotKeyID &+= 1

        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )

        let handlerStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            hotKeyEventHandler,
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &eventHandler
        )
        guard handlerStatus == noErr else { return nil }

        let registrationStatus = RegisterEventHotKey(
            configuration.keyCode,
            configuration.modifiers,
            identifier,
            GetApplicationEventTarget(),
            0,
            &hotKey
        )

        guard registrationStatus == noErr else {
            if let eventHandler {
                RemoveEventHandler(eventHandler)
            }
            return nil
        }
    }

    deinit {
        if let hotKey {
            UnregisterEventHotKey(hotKey)
        }
        if let eventHandler {
            RemoveEventHandler(eventHandler)
        }
    }

    fileprivate func performAction() {
        action()
    }

    fileprivate func matches(_ identifier: EventHotKeyID) -> Bool {
        self.identifier.signature == identifier.signature && self.identifier.id == identifier.id
    }
}

private func fourCharacterCode(_ value: String) -> OSType {
    value.utf8.reduce(0) { result, character in
        (result << 8) + OSType(character)
    }
}
