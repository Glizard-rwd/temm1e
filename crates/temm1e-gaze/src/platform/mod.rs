//! Platform-specific utilities for key mapping and input handling.

use enigo::Key;
use temm1e_core::types::error::Temm1eError;

/// Parse a key combination string like "cmd+c", "ctrl+shift+a", "enter", "tab"
/// into a sequence of enigo Keys (modifiers first, then the main key).
pub fn parse_key_combo(combo: &str) -> Result<Vec<Key>, Temm1eError> {
    let parts: Vec<String> = combo.split('+').map(|s| s.trim().to_lowercase()).collect();

    let mut keys = Vec::new();
    for part in &parts {
        let key = map_key_name(part)?;
        keys.push(key);
    }

    if keys.is_empty() {
        return Err(Temm1eError::Tool(format!("Empty key combo: '{}'", combo)));
    }

    Ok(keys)
}

/// Map a human-readable key name to an enigo Key.
fn map_key_name(name: &str) -> Result<Key, Temm1eError> {
    match name {
        // Modifiers
        "cmd" | "command" | "meta" | "super" | "win" => Ok(Key::Meta),
        "ctrl" | "control" => Ok(Key::Control),
        "alt" | "option" | "opt" => Ok(Key::Alt),
        "shift" => Ok(Key::Shift),

        // Special keys
        "enter" | "return" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "escape" | "esc" => Ok(Key::Escape),
        "backspace" | "delete" => Ok(Key::Backspace),
        "del" | "forwarddelete" => Ok(Key::Delete),
        "space" => Ok(Key::Space),
        "up" => Ok(Key::UpArrow),
        "down" => Ok(Key::DownArrow),
        "left" => Ok(Key::LeftArrow),
        "right" => Ok(Key::RightArrow),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),

        // Function keys
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),

        // Single character
        s if s.len() == 1 => {
            let ch = s.chars().next().unwrap();
            Ok(Key::Unicode(ch))
        }

        other => Err(Temm1eError::Tool(format!(
            "Unknown key name: '{}'. Supported: cmd, ctrl, alt, shift, enter, tab, \
             escape, backspace, del, space, up/down/left/right, home, end, \
             pageup/pagedown, f1-f12, or a single character.",
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_key() {
        let keys = parse_key_combo("enter").unwrap();
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn parse_modifier_combo() {
        let keys = parse_key_combo("cmd+c").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn parse_triple_combo() {
        let keys = parse_key_combo("ctrl+shift+a").unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn parse_unknown_key_fails() {
        let result = parse_key_combo("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn parse_case_insensitive() {
        let keys = parse_key_combo("CMD+C").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn parse_with_spaces() {
        let keys = parse_key_combo("ctrl + shift + a").unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn map_all_modifiers() {
        assert!(map_key_name("cmd").is_ok());
        assert!(map_key_name("ctrl").is_ok());
        assert!(map_key_name("alt").is_ok());
        assert!(map_key_name("shift").is_ok());
        assert!(map_key_name("option").is_ok());
        assert!(map_key_name("meta").is_ok());
    }

    #[test]
    fn map_all_special_keys() {
        for name in &[
            "enter",
            "return",
            "tab",
            "escape",
            "backspace",
            "space",
            "up",
            "down",
            "left",
            "right",
            "home",
            "end",
            "pageup",
            "pagedown",
        ] {
            assert!(map_key_name(name).is_ok(), "Key '{}' should be valid", name);
        }
    }

    #[test]
    fn map_function_keys() {
        for i in 1..=12 {
            let name = format!("f{}", i);
            assert!(
                map_key_name(&name).is_ok(),
                "Key '{}' should be valid",
                name
            );
        }
    }

    #[test]
    fn map_single_char() {
        assert!(map_key_name("a").is_ok());
        assert!(map_key_name("z").is_ok());
        assert!(map_key_name("1").is_ok());
    }
}
