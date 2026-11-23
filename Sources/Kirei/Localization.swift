import Foundation

/// Looks up a user-facing string. Keys are the English text, so a missing
/// translation (or running outside the app bundle) falls back to English.
func L(_ key: String) -> String {
    Bundle.main.localizedString(forKey: key, value: key, table: nil)
}

enum AppLanguage: String, CaseIterable, Identifiable {
    case system, en, ptBR = "pt-BR", es

    var id: String { rawValue }

    /// Each language is shown in its own name so anyone can find theirs.
    var title: String {
        switch self {
        case .system: return L("System default")
        case .en: return "English"
        case .ptBR: return "Português (Brasil)"
        case .es: return "Español"
        }
    }

    /// Per-app override read by Bundle at launch.
    static var current: AppLanguage {
        get {
            guard let langs = UserDefaults.standard.persistentDomain(forName: Bundle.main.bundleIdentifier ?? "")?["AppleLanguages"] as? [String],
                  let first = langs.first else { return .system }
            return AppLanguage(rawValue: first) ?? .system
        }
        set {
            if newValue == .system {
                UserDefaults.standard.removeObject(forKey: "AppleLanguages")
            } else {
                UserDefaults.standard.set([newValue.rawValue], forKey: "AppleLanguages")
            }
        }
    }
}
