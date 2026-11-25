use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Portuguese,
    Spanish,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Portuguese => "pt-BR",
            Self::Spanish => "es",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let c = code.to_ascii_lowercase();
        if c.starts_with("pt") {
            Some(Self::Portuguese)
        } else if c.starts_with("es") {
            Some(Self::Spanish)
        } else if c.starts_with("en") {
            Some(Self::English)
        } else {
            None
        }
    }

    /// Each language is shown in its own name so anyone can find theirs.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Portuguese => "Português (Brasil)",
            Self::Spanish => "Español",
        }
    }
}

static TABLE: OnceLock<(Language, HashMap<String, String>)> = OnceLock::new();

/// Fixed at launch, like the Swift app: changing the language asks for a restart.
pub fn init(preferred: Option<&str>) {
    let lang = preferred
        .and_then(Language::from_code)
        .or_else(|| {
            crate::platform::preferred_languages()
                .iter()
                .find_map(|c| Language::from_code(c))
        })
        .unwrap_or(Language::English);
    let source = match lang {
        Language::English => include_str!("../Localization/en.lproj/Localizable.strings"),
        Language::Portuguese => {
            include_str!("../Localization/pt-BR.lproj/Localizable.strings")
        }
        Language::Spanish => include_str!("../Localization/es.lproj/Localizable.strings"),
    };
    TABLE.set((lang, parse(source))).ok();
}

pub fn current() -> Language {
    TABLE.get().map(|t| t.0).unwrap_or(Language::English)
}

/// Keys are the English text, so a missing translation falls back to English.
pub fn l(key: &str) -> String {
    TABLE
        .get()
        .and_then(|t| t.1.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// `l` for strings with one `%d` or `%@`.
pub fn lf(key: &str, arg: impl std::fmt::Display) -> String {
    let s = l(key);
    let arg = arg.to_string();
    for token in ["%d", "%@", "%ld", "%lld"] {
        if s.contains(token) {
            return s.replacen(token, &arg, 1);
        }
    }
    s
}

fn parse(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in src.lines() {
        let line = line.trim();
        if !line.starts_with('"') {
            continue;
        }
        let mut parts = Vec::new();
        let mut cur = String::new();
        let mut in_str = false;
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            match (in_str, c) {
                (false, '"') => in_str = true,
                (true, '"') => {
                    in_str = false;
                    parts.push(std::mem::take(&mut cur));
                }
                (true, '\\') => match chars.next() {
                    Some('n') => cur.push('\n'),
                    Some('t') => cur.push('\t'),
                    Some(other) => cur.push(other),
                    None => {}
                },
                (true, c) => cur.push(c),
                _ => {}
            }
        }
        if let [k, v, ..] = parts.as_slice() {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_strings_file() {
        let t = super::parse(
            "/* c */\n\"A \\\"b\\\"\" = \"X \\\"y\\\"\";\n\"%d items\" = \"%d itens\";",
        );
        assert_eq!(t["A \"b\""], "X \"y\"");
        assert_eq!(t["%d items"], "%d itens");
    }
}
