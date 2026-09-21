//! The set of non-modifier keys a hotkey binding may include one of,
//! beyond the four modifiers (see `settings::HotkeyCombo`), and the
//! platform-specific identifiers needed to detect one being held: a
//! Windows virtual-key code for `GetAsyncKeyState`
//! (`windows_integration::poll_held_modifiers`), and a Linux/X11 keycode
//! for `XQueryKeymap` (`linux_integration::poll_held_modifiers`).
//!
//! Keyed by the same identifier the frontend already has to hand —
//! `KeyboardEvent.code` — which names a *physical* key position
//! independent of keyboard layout (e.g. `"KeyP"` is always the key at the
//! P position on a QWERTY-shaped board, even on a layout where it types a
//! different character), matching how both `GetAsyncKeyState` and X11
//! keycodes are also physical-position-based rather than character-based.
//!
//! The Linux keycodes follow the "evdev" XKB ruleset's universal
//! convention (X11 keycode = Linux kernel evdev scancode + 8), which is
//! effectively standard across every mainstream distribution — confirmed
//! against a real `xmodmap -pm` listing during development, where every
//! modifier keycode matched this table exactly.
#[rustfmt::skip]
const KEY_TABLE: &[(&str, u16, u8)] = &[
    // code           windows VK   X11 keycode (evdev + 8)
    ("KeyA",          0x41,        38),
    ("KeyB",          0x42,        56),
    ("KeyC",          0x43,        54),
    ("KeyD",          0x44,        40),
    ("KeyE",          0x45,        26),
    ("KeyF",          0x46,        41),
    ("KeyG",          0x47,        42),
    ("KeyH",          0x48,        43),
    ("KeyI",          0x49,        31),
    ("KeyJ",          0x4A,        44),
    ("KeyK",          0x4B,        45),
    ("KeyL",          0x4C,        46),
    ("KeyM",          0x4D,        58),
    ("KeyN",          0x4E,        57),
    ("KeyO",          0x4F,        32),
    ("KeyP",          0x50,        33),
    ("KeyQ",          0x51,        24),
    ("KeyR",          0x52,        27),
    ("KeyS",          0x53,        39),
    ("KeyT",          0x54,        28),
    ("KeyU",          0x55,        30),
    ("KeyV",          0x56,        55),
    ("KeyW",          0x57,        25),
    ("KeyX",          0x58,        53),
    ("KeyY",          0x59,        29),
    ("KeyZ",          0x5A,        52),
    ("Digit0",        0x30,        19),
    ("Digit1",        0x31,        10),
    ("Digit2",        0x32,        11),
    ("Digit3",        0x33,        12),
    ("Digit4",        0x34,        13),
    ("Digit5",        0x35,        14),
    ("Digit6",        0x36,        15),
    ("Digit7",        0x37,        16),
    ("Digit8",        0x38,        17),
    ("Digit9",        0x39,        18),
    ("F1",            0x70,        67),
    ("F2",            0x71,        68),
    ("F3",            0x72,        69),
    ("F4",            0x73,        70),
    ("F5",            0x74,        71),
    ("F6",            0x75,        72),
    ("F7",            0x76,        73),
    ("F8",            0x77,        74),
    ("F9",            0x78,        75),
    ("F10",           0x79,        76),
    ("F11",           0x7A,        95),
    ("F12",           0x7B,        96),
    ("Space",         0x20,        65),
    ("Tab",           0x09,        23),
    ("Enter",         0x0D,        36),
    ("Escape",        0x1B,        9),
    ("Backspace",     0x08,        22),
    ("Delete",        0x2E,        119),
    ("Insert",        0x2D,        118),
    ("Home",          0x24,        110),
    ("End",           0x23,        115),
    ("PageUp",        0x21,        112),
    ("PageDown",      0x22,        117),
    ("ArrowUp",       0x26,        111),
    ("ArrowDown",     0x28,        116),
    ("ArrowLeft",     0x25,        113),
    ("ArrowRight",    0x27,        114),
    ("Minus",         0xBD,        20),
    ("Equal",         0xBB,        21),
    ("BracketLeft",   0xDB,        34),
    ("BracketRight",  0xDD,        35),
    ("Semicolon",     0xBA,        47),
    ("Quote",         0xDE,        48),
    ("Backslash",     0xDC,        51),
    ("Comma",         0xBC,        59),
    ("Period",        0xBE,        60),
    ("Slash",         0xBF,        61),
    ("Backquote",     0xC0,        49),
];

/// Whether `code` (a `KeyboardEvent.code` value) is one of the extra keys a
/// hotkey binding can include — used to validate/normalize a binding
/// coming from the frontend before persisting it.
pub fn is_supported_key(code: &str) -> bool {
    KEY_TABLE.iter().any(|(c, _, _)| *c == code)
}

/// The Windows virtual-key code for `code`, if it's a supported extra key.
pub fn windows_vk_for_code(code: &str) -> Option<u16> {
    KEY_TABLE.iter().find(|(c, _, _)| *c == code).map(|(_, vk, _)| *vk)
}

/// The Linux/X11 keycode for `code`, if it's a supported extra key.
pub fn x11_keycode_for_code(code: &str) -> Option<u8> {
    KEY_TABLE.iter().find(|(c, _, _)| *c == code).map(|(_, _, kc)| *kc)
}

/// The `code` for a given Windows virtual-key code, if it's one of the
/// supported extra keys — the reverse of `windows_vk_for_code`, used to
/// report back *which* supported key (if any) is currently held while
/// polling.
pub fn code_for_windows_vk(vk: u16) -> Option<&'static str> {
    KEY_TABLE.iter().find(|(_, v, _)| *v == vk).map(|(c, _, _)| *c)
}

/// The `code` for a given Linux/X11 keycode, if it's one of the supported
/// extra keys — the reverse of `x11_keycode_for_code`.
pub fn code_for_x11_keycode(keycode: u8) -> Option<&'static str> {
    KEY_TABLE.iter().find(|(_, _, kc)| *kc == keycode).map(|(c, _, _)| *c)
}

/// Every supported key as `(code, windows_vk)` pairs — used to scan for
/// which one (if any) is currently held while polling on Windows.
pub fn windows_vks() -> impl Iterator<Item = (&'static str, u16)> {
    KEY_TABLE.iter().map(|(code, vk, _)| (*code, *vk))
}

/// Every supported key as `(code, x11_keycode)` pairs — used to scan for
/// which one (if any) is currently held while polling on Linux.
pub fn x11_keycodes() -> impl Iterator<Item = (&'static str, u8)> {
    KEY_TABLE.iter().map(|(code, _, keycode)| (*code, *keycode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_code_is_unique() {
        let codes: HashSet<&str> = KEY_TABLE.iter().map(|(c, _, _)| *c).collect();
        assert_eq!(codes.len(), KEY_TABLE.len());
    }

    #[test]
    fn every_windows_vk_is_unique() {
        let vks: HashSet<u16> = KEY_TABLE.iter().map(|(_, vk, _)| *vk).collect();
        assert_eq!(vks.len(), KEY_TABLE.len());
    }

    #[test]
    fn every_x11_keycode_is_unique() {
        let keycodes: HashSet<u8> = KEY_TABLE.iter().map(|(_, _, kc)| *kc).collect();
        assert_eq!(keycodes.len(), KEY_TABLE.len());
    }

    #[test]
    fn looks_up_a_letter_key_on_both_platforms() {
        assert_eq!(windows_vk_for_code("KeyP"), Some(0x50));
        assert_eq!(x11_keycode_for_code("KeyP"), Some(33));
    }

    #[test]
    fn round_trips_windows_vk_back_to_code() {
        assert_eq!(code_for_windows_vk(0x50), Some("KeyP"));
    }

    #[test]
    fn round_trips_x11_keycode_back_to_code() {
        assert_eq!(code_for_x11_keycode(33), Some("KeyP"));
    }

    #[test]
    fn unsupported_code_is_none() {
        assert!(!is_supported_key("NumpadEnter"));
        assert_eq!(windows_vk_for_code("NumpadEnter"), None);
        assert_eq!(x11_keycode_for_code("NumpadEnter"), None);
    }

    #[test]
    fn function_key_row_is_sequential_except_f11_f12() {
        for (index, n) in (1..=10).enumerate() {
            let code = format!("F{n}");
            assert_eq!(windows_vk_for_code(&code), Some(0x70 + index as u16));
        }
    }
}
