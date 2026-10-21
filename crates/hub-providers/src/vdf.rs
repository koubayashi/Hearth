//! Minimal parser for Valve's KeyValues text format (`.vdf`, `.acf`).

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Map(BTreeMap<String, Vdf>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            // Keys are case-insensitive in practice ("AppState" vs "appstate").
            Vdf::Map(m) => m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s),
            Vdf::Map(_) => None,
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &Vdf)> {
        match self {
            Vdf::Map(m) => Some(m.iter()),
            Vdf::Str(_) => None,
        }
        .into_iter()
        .flatten()
    }
}

/// Parses a whole document. Malformed trailing input is ignored rather than
/// failing, since a half-written manifest should not hide every other game.
pub fn parse(text: &str) -> Vdf {
    let mut tokens = tokenize(text).into_iter().peekable();
    Vdf::Map(parse_map(&mut tokens))
}

#[derive(Debug, PartialEq)]
enum Tok {
    Str(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        _ => s.push(c),
                    }
                }
                out.push(Tok::Str(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_map(tokens: &mut std::iter::Peekable<std::vec::IntoIter<Tok>>) -> BTreeMap<String, Vdf> {
    let mut map = BTreeMap::new();
    while let Some(tok) = tokens.next() {
        let key = match tok {
            Tok::Str(k) => k,
            Tok::Close => break,
            Tok::Open => continue,
        };
        match tokens.next() {
            Some(Tok::Str(v)) => {
                map.insert(key, Vdf::Str(v));
            }
            Some(Tok::Open) => {
                map.insert(key, Vdf::Map(parse_map(tokens)));
            }
            _ => break,
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_maps_and_escapes() {
        let doc = parse(
            r#""libraryfolders"
            {
                "0" { "path" "C:\\program files (x86)\\steam" "apps" { "228980" "241762497" } }
                // comment
                "1" { "path" "D:\\SteamLibrary" }
            }"#,
        );
        let lf = doc.get("libraryfolders").unwrap();
        let paths: Vec<_> = lf.entries().filter_map(|(_, v)| v.str("path")).collect();
        assert_eq!(paths, [r"C:\program files (x86)\steam", r"D:\SteamLibrary"]);
        assert!(lf.get("0").unwrap().get("apps").unwrap().str("228980").is_some());
    }

    #[test]
    fn keys_are_case_insensitive() {
        let doc = parse(r#""AppState" { "appid" "620" "name" "Portal 2" }"#);
        assert_eq!(doc.get("appstate").unwrap().str("Name"), Some("Portal 2"));
    }
}
