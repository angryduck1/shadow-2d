//! Тексты игры: разбор файла assets/dialogs.txt.
//!
//! Формат: строка `@ключ` начинает запись, `@--` начинает комментарий.
//! Копия файла вшита в программу; файл на диске перекрывает её по ключам.

use std::collections::HashMap;

const EMBEDDED_DIALOGS: &str = include_str!("../assets/dialogs.txt");

/// Что показать, если ключа нет ни в файле, ни во встроенной копии.
pub const MISSING: &str = "[нет текста]";

/// Набор записей «ключ → текст».
pub struct Catalog {
    embedded: HashMap<String, String>,
    overrides: HashMap<String, String>,
}

impl Catalog {
    /// Встроенные тексты плюс правки из файла рядом с игрой, если он есть.
    pub fn load() -> Self {
        let overrides = std::fs::read_to_string("assets/dialogs.txt")
            .map(|source| parse_sections(&source))
            .unwrap_or_default();
        Self { embedded: parse_sections(EMBEDDED_DIALOGS), overrides }
    }

    /// Только встроенные тексты: для тестов, которые не зависят от папки запуска.
    #[cfg(test)]
    pub fn embedded_only() -> Self {
        Self { embedded: parse_sections(EMBEDDED_DIALOGS), overrides: HashMap::new() }
    }

    pub fn get(&self, key: &str) -> &str {
        self.overrides
            .get(key)
            .or_else(|| self.embedded.get(key))
            .map_or(MISSING, String::as_str)
    }
}

/// Разбирает текст на записи и обрезает пустые строки по краям каждой.
pub fn parse_sections(source: &str) -> HashMap<String, String> {
    let mut sections: HashMap<String, Vec<&str>> = HashMap::new();
    let mut current: Option<String> = None;

    for line in source.lines() {
        let line = line.trim_end(); // убирает и `\r` из файлов Windows
        if line.starts_with("@--") {
            current = None;
        } else if let Some(key) = line.strip_prefix('@') {
            let key = key.trim().to_string();
            sections.entry(key.clone()).or_default();
            current = Some(key);
        } else if let Some(key) = &current {
            if let Some(lines) = sections.get_mut(key) {
                lines.push(line);
            }
        }
    }

    sections
        .into_iter()
        .map(|(key, lines)| {
            let first = lines.iter().position(|l| !l.is_empty()).unwrap_or(lines.len());
            let last = lines.iter().rposition(|l| !l.is_empty()).map_or(first, |i| i + 1);
            (key, lines[first..last].join("\n"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_section_files() {
        let parsed = parse_sections("@-- заметка\nпропуск\n@a\n\nраз\n  два\n\n@b\r\nтри\r\n");
        assert_eq!(parsed["a"], "раз\n  два");
        assert_eq!(parsed["b"], "три");
        assert_eq!(parsed.len(), 2);
    }

    /// Повторный ключ склеил бы две записи в одну: такого быть не должно.
    #[test]
    fn embedded_keys_are_unique() {
        let mut keys = std::collections::HashSet::new();
        for line in EMBEDDED_DIALOGS.lines().filter(|l| l.starts_with('@') && !l.starts_with("@--")) {
            assert!(keys.insert(line.trim()), "ключ {line} встречается дважды");
        }
    }

    #[test]
    fn missing_key_gives_placeholder() {
        let catalog = Catalog::embedded_only();
        assert_eq!(catalog.get("нет.такого"), MISSING);
        assert_ne!(catalog.get("help"), MISSING);
    }
}
