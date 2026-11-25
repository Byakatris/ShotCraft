use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use muda::accelerator::{Accelerator, Code as MudaCode, Modifiers as MudaModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeySlot {
    Fullscreen,
    Area,
    Window,
    LiveDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub key: HotkeyCode,
}

macro_rules! define_hotkey_codes {
    ($(($variant:ident, $label:literal)),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum HotkeyCode {
            $($variant,)*
        }

        impl HotkeyCode {
            #[cfg(test)]
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn to_ghk(self) -> Code {
                match self {
                    $(Self::$variant => Code::$variant,)*
                }
            }

            pub fn to_muda(self) -> MudaCode {
                match self {
                    $(Self::$variant => MudaCode::$variant,)*
                }
            }

            pub fn from_winit(k: winit::keyboard::KeyCode) -> Option<Self> {
                use winit::keyboard::KeyCode as W;
                match k {
                    $(W::$variant => Some(Self::$variant),)*
                    _ => None,
                }
            }

            pub fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)*
                }
            }

            pub fn from_label(s: &str) -> Option<Self> {
                match s.trim() {
                    $($label => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

define_hotkey_codes!(
    (Digit1, "1"),
    (Digit2, "2"),
    (Digit3, "3"),
    (Digit4, "4"),
    (Digit5, "5"),
    (Digit6, "6"),
    (Digit7, "7"),
    (Digit8, "8"),
    (Digit9, "9"),
    (Digit0, "0"),
    (KeyA, "A"),
    (KeyB, "B"),
    (KeyC, "C"),
    (KeyD, "D"),
    (KeyE, "E"),
    (KeyF, "F"),
    (KeyG, "G"),
    (KeyH, "H"),
    (KeyI, "I"),
    (KeyJ, "J"),
    (KeyK, "K"),
    (KeyL, "L"),
    (KeyM, "M"),
    (KeyN, "N"),
    (KeyO, "O"),
    (KeyP, "P"),
    (KeyQ, "Q"),
    (KeyR, "R"),
    (KeyS, "S"),
    (KeyT, "T"),
    (KeyU, "U"),
    (KeyV, "V"),
    (KeyW, "W"),
    (KeyX, "X"),
    (KeyY, "Y"),
    (KeyZ, "Z"),
);

impl HotkeySpec {
    pub fn opt_shift(key: HotkeyCode) -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: true,
            super_key: false,
            key,
        }
    }

    pub fn has_modifier(self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key
    }

    fn map_mods<M: Default + std::ops::BitOrAssign>(
        self,
        ctrl: M,
        alt: M,
        shift: M,
        super_key: M,
    ) -> (M, bool) {
        let mut mods = M::default();
        let mut any = false;
        for (enabled, bit) in [
            (self.ctrl, ctrl),
            (self.alt, alt),
            (self.shift, shift),
            (self.super_key, super_key),
        ] {
            if enabled {
                mods |= bit;
                any = true;
            }
        }
        (mods, any)
    }

    pub fn to_hotkey(self) -> HotKey {
        let (mods, any) = self.map_mods(
            Modifiers::CONTROL,
            Modifiers::ALT,
            Modifiers::SHIFT,
            Modifiers::SUPER,
        );
        HotKey::new(any.then_some(mods), self.key.to_ghk())
    }

    pub fn to_accelerator(self) -> Accelerator {
        let (mods, any) = self.map_mods(
            MudaModifiers::CONTROL,
            MudaModifiers::ALT,
            MudaModifiers::SHIFT,
            MudaModifiers::SUPER,
        );
        Accelerator::new(any.then_some(mods), self.key.to_muda())
    }

    /// Clean ASCII-safe display so Roboto renders zero missing-glyph boxes.
    pub fn display(self) -> String {
        let mut parts: Vec<&str> = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            #[cfg(target_os = "macos")]
            parts.push("Opt");
            #[cfg(not(target_os = "macos"))]
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.super_key {
            #[cfg(target_os = "macos")]
            parts.push("Cmd");
            #[cfg(target_os = "windows")]
            parts.push("Win");
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            parts.push("Super");
        }
        parts.push(self.key.label());
        parts.join(" + ")
    }

    pub fn serialize(self) -> String {
        format!(
            "{},{},{},{},{}",
            self.ctrl as u8,
            self.alt as u8,
            self.shift as u8,
            self.super_key as u8,
            self.key.label()
        )
    }

    pub fn deserialize(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split(',').collect();
        if parts.len() != 5 {
            return None;
        }
        let parse_flag = |f: &str| match f {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        };
        Some(Self {
            ctrl: parse_flag(parts[0])?,
            alt: parse_flag(parts[1])?,
            shift: parse_flag(parts[2])?,
            super_key: parse_flag(parts[3])?,
            key: HotkeyCode::from_label(parts[4])?,
        })
    }
}
