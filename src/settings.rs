use std::fs;
use std::path::PathBuf;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use muda::accelerator::{Accelerator, Code as MudaCode, Modifiers as MudaModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Uk,
    Es,
    Fr,
    De,
    Pl,
    It,
    Pt,
    Tr,
    Nl,
    Zh,
    Ja,
    Ko,
    Ar,
    Hi,
}

impl Language {
    pub const ALL: [Self; 15] = [
        Self::En,
        Self::Uk,
        Self::Es,
        Self::Fr,
        Self::De,
        Self::Pl,
        Self::It,
        Self::Pt,
        Self::Tr,
        Self::Nl,
        Self::Zh,
        Self::Ja,
        Self::Ko,
        Self::Ar,
        Self::Hi,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Uk => "uk",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Pl => "pl",
            Self::It => "it",
            Self::Pt => "pt",
            Self::Tr => "tr",
            Self::Nl => "nl",
            Self::Zh => "zh",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Ar => "ar",
            Self::Hi => "hi",
        }
    }

    pub fn from_code(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|l| l.code() == s.trim())
            .unwrap_or(Self::En)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Uk => "Українська",
            Self::Es => "Español",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Pl => "Polski",
            Self::It => "Italiano",
            Self::Pt => "Português",
            Self::Tr => "Türkçe",
            Self::Nl => "Nederlands",
            Self::Zh => "Chinese (ZH)",
            Self::Ja => "Japanese (JA)",
            Self::Ko => "Korean (KO)",
            Self::Ar => "Arabic (AR)",
            Self::Hi => "Hindi (HI)",
        }
    }
}

pub struct Strings {
    pub menu_full: &'static str,
    pub menu_area: &'static str,
    pub menu_window: &'static str,
    pub menu_live_draw: &'static str,
    pub menu_open_folder: &'static str,
    pub menu_settings: &'static str,
    pub menu_quit: &'static str,
    pub hint_area: &'static str,
    pub hint_window: &'static str,
    pub settings_title: &'static str,
    pub settings_lang: &'static str,
    pub settings_folder: &'static str,
    pub settings_choose: &'static str,
    pub settings_desktop: &'static str,
    pub settings_hotkeys: &'static str,
    pub hk_full: &'static str,
    pub hk_area: &'static str,
    pub hk_window: &'static str,
    pub hk_live_draw: &'static str,
    pub hk_press: &'static str,
}

impl Strings {
    pub fn for_lang(lang: Language) -> Self {
        match lang {
            Language::Uk => Self {
                menu_full: "Весь екран",
                menu_area: "Область",
                menu_window: "Вікно",
                menu_live_draw: "Поверх екрану",
                menu_open_folder: "Відкрити теку збереження",
                menu_settings: "Налаштування...",
                menu_quit: "Вийти з ShotCraft",
                hint_area: "Потягніть для виділення  •  Esc — скасувати",
                hint_window: "Наведіть на вікно та клікніть  •  Esc — скасувати",
                settings_title: "Налаштування ShotCraft",
                settings_lang: "Мова",
                settings_folder: "Тека збереження скріншотів",
                settings_choose: "Обрати теку...",
                settings_desktop: "Робочий стіл",
                settings_hotkeys: "Гарячі клавіші",
                hk_full: "Весь екран",
                hk_area: "Область",
                hk_window: "Вікно",
                hk_live_draw: "Поверх екрану",
                hk_press: "Натисніть клавіші...",
            },
            Language::Es => Self {
                menu_full: "Pantalla completa",
                menu_area: "Capturar área",
                menu_window: "Capturar ventana",
                menu_live_draw: "Dibujar en pantalla",
                menu_open_folder: "Abrir carpeta",
                menu_settings: "Ajustes...",
                menu_quit: "Salir de ShotCraft",
                hint_area: "Arrastra para seleccionar  •  Esc para cancelar",
                hint_window: "Pasa sobre una ventana y haz clic  •  Esc para cancelar",
                settings_title: "Ajustes de ShotCraft",
                settings_lang: "Idioma",
                settings_folder: "Carpeta de capturas",
                settings_choose: "Elegir carpeta...",
                settings_desktop: "Escritorio",
                settings_hotkeys: "Atajos globales",
                hk_full: "Pantalla completa",
                hk_area: "Capturar área",
                hk_window: "Capturar ventana",
                hk_live_draw: "Dibujar en pantalla",
                hk_press: "Pulsa teclas...",
            },
            Language::Fr => Self {
                menu_full: "Plein écran",
                menu_area: "Capturer une zone",
                menu_window: "Capturer une fenêtre",
                menu_live_draw: "Dessiner à l'écran",
                menu_open_folder: "Ouvrir le dossier",
                menu_settings: "Réglages...",
                menu_quit: "Quitter ShotCraft",
                hint_area: "Glissez pour sélectionner  •  Échap pour annuler",
                hint_window: "Survolez une fenêtre et cliquez  •  Échap pour annuler",
                settings_title: "Réglages ShotCraft",
                settings_lang: "Langue",
                settings_folder: "Dossier d'enregistrement",
                settings_choose: "Choisir...",
                settings_desktop: "Bureau",
                settings_hotkeys: "Raccourcis globaux",
                hk_full: "Plein écran",
                hk_area: "Capturer une zone",
                hk_window: "Capturer une fenêtre",
                hk_live_draw: "Dessiner à l'écran",
                hk_press: "Appuyez...",
            },
            Language::De => Self {
                menu_full: "Vollbild",
                menu_area: "Bereich aufnehmen",
                menu_window: "Fenster aufnehmen",
                menu_live_draw: "Auf Bildschirm zeichnen",
                menu_open_folder: "Ordner öffnen",
                menu_settings: "Einstellungen...",
                menu_quit: "ShotCraft beenden",
                hint_area: "Zum Auswählen ziehen  •  Esc zum Abbrechen",
                hint_window: "Fenster anvisieren & klicken  •  Esc zum Abbrechen",
                settings_title: "ShotCraft Einstellungen",
                settings_lang: "Sprache",
                settings_folder: "Speicherordner",
                settings_choose: "Ordner wählen...",
                settings_desktop: "Schreibtisch",
                settings_hotkeys: "Tastenkürzel",
                hk_full: "Vollbild",
                hk_area: "Bereich aufnehmen",
                hk_window: "Fenster aufnehmen",
                hk_live_draw: "Auf Bildschirm zeichnen",
                hk_press: "Taste drücken...",
            },
            Language::Pl => Self {
                menu_full: "Pełny ekran",
                menu_area: "Zaznacz obszar",
                menu_window: "Przechwyć okno",
                menu_live_draw: "Rysuj na ekranie",
                menu_open_folder: "Otwórz folder",
                menu_settings: "Ustawienia...",
                menu_quit: "Zakończ ShotCraft",
                hint_area: "Przeciągnij, aby zaznaczyć  •  Esc — anuluj",
                hint_window: "Najedź na okno i kliknij  •  Esc — anuluj",
                settings_title: "Ustawienia ShotCraft",
                settings_lang: "Język",
                settings_folder: "Folder zapisu zrzutów",
                settings_choose: "Wybierz folder...",
                settings_desktop: "Pulpit",
                settings_hotkeys: "Skróty klawiszowe",
                hk_full: "Pełny ekran",
                hk_area: "Zaznacz obszar",
                hk_window: "Przechwyć okno",
                hk_live_draw: "Rysuj na ekranie",
                hk_press: "Naciśnij klawisze...",
            },
            Language::It => Self {
                menu_full: "Schermo intero",
                menu_area: "Cattura area",
                menu_window: "Cattura finestra",
                menu_live_draw: "Disegna sullo schermo",
                menu_open_folder: "Apri cartella",
                menu_settings: "Impostazioni...",
                menu_quit: "Esci da ShotCraft",
                hint_area: "Trascina per selezionare  •  Esc per annullare",
                hint_window: "Passa su una finestra e clicca  •  Esc per annullare",
                settings_title: "Impostazioni ShotCraft",
                settings_lang: "Lingua",
                settings_folder: "Cartella di salvataggio",
                settings_choose: "Scegli cartella...",
                settings_desktop: "Scrivania",
                settings_hotkeys: "Scorciatoie globali",
                hk_full: "Schermo intero",
                hk_area: "Cattura area",
                hk_window: "Cattura finestra",
                hk_live_draw: "Disegna sullo schermo",
                hk_press: "Premi tasti...",
            },
            Language::Pt => Self {
                menu_full: "Tela cheia",
                menu_area: "Capturar área",
                menu_window: "Capturar janela",
                menu_live_draw: "Desenhar na tela",
                menu_open_folder: "Abrir pasta",
                menu_settings: "Configurações...",
                menu_quit: "Sair do ShotCraft",
                hint_area: "Arraste para selecionar  •  Esc para cancelar",
                hint_window: "Passe sobre uma janela e clique  •  Esc para cancelar",
                settings_title: "Configurações ShotCraft",
                settings_lang: "Idioma",
                settings_folder: "Pasta de capturas",
                settings_choose: "Escolher pasta...",
                settings_desktop: "Mesa",
                settings_hotkeys: "Atalhos globais",
                hk_full: "Tela cheia",
                hk_area: "Capturar área",
                hk_window: "Capturar janela",
                hk_live_draw: "Desenhar na tela",
                hk_press: "Pressione teclas...",
            },
            Language::Tr => Self {
                menu_full: "Tam Ekran",
                menu_area: "Alan Yakala",
                menu_window: "Pencere Yakala",
                menu_live_draw: "Ekran Üzerine Çiz",
                menu_open_folder: "Klasörü Aç",
                menu_settings: "Ayarlar...",
                menu_quit: "ShotCraft'tan Çık",
                hint_area: "Seçmek için sürükleyin  •  İptal: Esc",
                hint_window: "Pencerenin üzerine gelip tıklayın  •  İptal: Esc",
                settings_title: "ShotCraft Ayarları",
                settings_lang: "Dil",
                settings_folder: "Kayıt Klasörü",
                settings_choose: "Klasör Seç...",
                settings_desktop: "Masaüstü",
                settings_hotkeys: "Kısayollar",
                hk_full: "Tam Ekran",
                hk_area: "Alan Yakala",
                hk_window: "Pencere Yakala",
                hk_live_draw: "Ekran Üzerine Çiz",
                hk_press: "Tuşlara basın...",
            },
            Language::Nl => Self {
                menu_full: "Volledig scherm",
                menu_area: "Gebied vastleggen",
                menu_window: "Venster vastleggen",
                menu_live_draw: "Teken op scherm",
                menu_open_folder: "Map openen",
                menu_settings: "Instellingen...",
                menu_quit: "ShotCraft afsluiten",
                hint_area: "Sleep om te selecteren  •  Esc om te annuleren",
                hint_window: "Wijs een venster aan en klik  •  Esc om te annuleren",
                settings_title: "ShotCraft Instellingen",
                settings_lang: "Taal",
                settings_folder: "Opslagmap",
                settings_choose: "Map kiezen...",
                settings_desktop: "Bureaublad",
                settings_hotkeys: "Sneltoetsen",
                hk_full: "Volledig scherm",
                hk_area: "Gebied vastleggen",
                hk_window: "Venster vastleggen",
                hk_live_draw: "Teken op scherm",
                hk_press: "Druk op toetsen...",
            },
            _ => Self {
                menu_full: "Fullscreen",
                menu_area: "Area",
                menu_window: "Window",
                menu_live_draw: "Screen Draw",
                menu_open_folder: "Open Save Folder",
                menu_settings: "Settings...",
                menu_quit: "Quit ShotCraft",
                hint_area: "Drag to select area  •  Esc to cancel",
                hint_window: "Hover a window and click  •  Esc to cancel",
                settings_title: "ShotCraft Settings",
                settings_lang: "Language",
                settings_folder: "Screenshots Save Folder",
                settings_choose: "Choose Folder...",
                settings_desktop: "Desktop",
                settings_hotkeys: "Global Hotkeys",
                hk_full: "Fullscreen",
                hk_area: "Area",
                hk_window: "Window",
                hk_live_draw: "Screen Draw",
                hk_press: "Press keys...",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub key: HotkeyCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyCode {
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Digit0,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
}

impl HotkeyCode {
    pub fn to_ghk(self) -> Code {
        match self {
            Self::Digit1 => Code::Digit1,
            Self::Digit2 => Code::Digit2,
            Self::Digit3 => Code::Digit3,
            Self::Digit4 => Code::Digit4,
            Self::Digit5 => Code::Digit5,
            Self::Digit6 => Code::Digit6,
            Self::Digit7 => Code::Digit7,
            Self::Digit8 => Code::Digit8,
            Self::Digit9 => Code::Digit9,
            Self::Digit0 => Code::Digit0,
            Self::KeyA => Code::KeyA,
            Self::KeyB => Code::KeyB,
            Self::KeyC => Code::KeyC,
            Self::KeyD => Code::KeyD,
            Self::KeyE => Code::KeyE,
            Self::KeyF => Code::KeyF,
            Self::KeyG => Code::KeyG,
            Self::KeyH => Code::KeyH,
            Self::KeyI => Code::KeyI,
            Self::KeyJ => Code::KeyJ,
            Self::KeyK => Code::KeyK,
            Self::KeyL => Code::KeyL,
            Self::KeyM => Code::KeyM,
            Self::KeyN => Code::KeyN,
            Self::KeyO => Code::KeyO,
            Self::KeyP => Code::KeyP,
            Self::KeyQ => Code::KeyQ,
            Self::KeyR => Code::KeyR,
            Self::KeyS => Code::KeyS,
            Self::KeyT => Code::KeyT,
            Self::KeyU => Code::KeyU,
            Self::KeyV => Code::KeyV,
            Self::KeyW => Code::KeyW,
            Self::KeyX => Code::KeyX,
            Self::KeyY => Code::KeyY,
            Self::KeyZ => Code::KeyZ,
        }
    }

    pub fn to_muda(self) -> MudaCode {
        match self {
            Self::Digit1 => MudaCode::Digit1,
            Self::Digit2 => MudaCode::Digit2,
            Self::Digit3 => MudaCode::Digit3,
            Self::Digit4 => MudaCode::Digit4,
            Self::Digit5 => MudaCode::Digit5,
            Self::Digit6 => MudaCode::Digit6,
            Self::Digit7 => MudaCode::Digit7,
            Self::Digit8 => MudaCode::Digit8,
            Self::Digit9 => MudaCode::Digit9,
            Self::Digit0 => MudaCode::Digit0,
            Self::KeyA => MudaCode::KeyA,
            Self::KeyB => MudaCode::KeyB,
            Self::KeyC => MudaCode::KeyC,
            Self::KeyD => MudaCode::KeyD,
            Self::KeyE => MudaCode::KeyE,
            Self::KeyF => MudaCode::KeyF,
            Self::KeyG => MudaCode::KeyG,
            Self::KeyH => MudaCode::KeyH,
            Self::KeyI => MudaCode::KeyI,
            Self::KeyJ => MudaCode::KeyJ,
            Self::KeyK => MudaCode::KeyK,
            Self::KeyL => MudaCode::KeyL,
            Self::KeyM => MudaCode::KeyM,
            Self::KeyN => MudaCode::KeyN,
            Self::KeyO => MudaCode::KeyO,
            Self::KeyP => MudaCode::KeyP,
            Self::KeyQ => MudaCode::KeyQ,
            Self::KeyR => MudaCode::KeyR,
            Self::KeyS => MudaCode::KeyS,
            Self::KeyT => MudaCode::KeyT,
            Self::KeyU => MudaCode::KeyU,
            Self::KeyV => MudaCode::KeyV,
            Self::KeyW => MudaCode::KeyW,
            Self::KeyX => MudaCode::KeyX,
            Self::KeyY => MudaCode::KeyY,
            Self::KeyZ => MudaCode::KeyZ,
        }
    }

    pub fn from_winit(k: winit::keyboard::KeyCode) -> Option<Self> {
        use winit::keyboard::KeyCode as W;
        match k {
            W::Digit1 => Some(Self::Digit1),
            W::Digit2 => Some(Self::Digit2),
            W::Digit3 => Some(Self::Digit3),
            W::Digit4 => Some(Self::Digit4),
            W::Digit5 => Some(Self::Digit5),
            W::Digit6 => Some(Self::Digit6),
            W::Digit7 => Some(Self::Digit7),
            W::Digit8 => Some(Self::Digit8),
            W::Digit9 => Some(Self::Digit9),
            W::Digit0 => Some(Self::Digit0),
            W::KeyA => Some(Self::KeyA),
            W::KeyB => Some(Self::KeyB),
            W::KeyC => Some(Self::KeyC),
            W::KeyD => Some(Self::KeyD),
            W::KeyE => Some(Self::KeyE),
            W::KeyF => Some(Self::KeyF),
            W::KeyG => Some(Self::KeyG),
            W::KeyH => Some(Self::KeyH),
            W::KeyI => Some(Self::KeyI),
            W::KeyJ => Some(Self::KeyJ),
            W::KeyK => Some(Self::KeyK),
            W::KeyL => Some(Self::KeyL),
            W::KeyM => Some(Self::KeyM),
            W::KeyN => Some(Self::KeyN),
            W::KeyO => Some(Self::KeyO),
            W::KeyP => Some(Self::KeyP),
            W::KeyQ => Some(Self::KeyQ),
            W::KeyR => Some(Self::KeyR),
            W::KeyS => Some(Self::KeyS),
            W::KeyT => Some(Self::KeyT),
            W::KeyU => Some(Self::KeyU),
            W::KeyV => Some(Self::KeyV),
            W::KeyW => Some(Self::KeyW),
            W::KeyX => Some(Self::KeyX),
            W::KeyY => Some(Self::KeyY),
            W::KeyZ => Some(Self::KeyZ),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::Digit0 => "0",
            Self::KeyA => "A",
            Self::KeyB => "B",
            Self::KeyC => "C",
            Self::KeyD => "D",
            Self::KeyE => "E",
            Self::KeyF => "F",
            Self::KeyG => "G",
            Self::KeyH => "H",
            Self::KeyI => "I",
            Self::KeyJ => "J",
            Self::KeyK => "K",
            Self::KeyL => "L",
            Self::KeyM => "M",
            Self::KeyN => "N",
            Self::KeyO => "O",
            Self::KeyP => "P",
            Self::KeyQ => "Q",
            Self::KeyR => "R",
            Self::KeyS => "S",
            Self::KeyT => "T",
            Self::KeyU => "U",
            Self::KeyV => "V",
            Self::KeyW => "W",
            Self::KeyX => "X",
            Self::KeyY => "Y",
            Self::KeyZ => "Z",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        match s.trim() {
            "1" => Some(Self::Digit1),
            "2" => Some(Self::Digit2),
            "3" => Some(Self::Digit3),
            "4" => Some(Self::Digit4),
            "5" => Some(Self::Digit5),
            "6" => Some(Self::Digit6),
            "7" => Some(Self::Digit7),
            "8" => Some(Self::Digit8),
            "9" => Some(Self::Digit9),
            "0" => Some(Self::Digit0),
            "A" => Some(Self::KeyA),
            "B" => Some(Self::KeyB),
            "C" => Some(Self::KeyC),
            "D" => Some(Self::KeyD),
            "E" => Some(Self::KeyE),
            "F" => Some(Self::KeyF),
            "G" => Some(Self::KeyG),
            "H" => Some(Self::KeyH),
            "I" => Some(Self::KeyI),
            "J" => Some(Self::KeyJ),
            "K" => Some(Self::KeyK),
            "L" => Some(Self::KeyL),
            "M" => Some(Self::KeyM),
            "N" => Some(Self::KeyN),
            "O" => Some(Self::KeyO),
            "P" => Some(Self::KeyP),
            "Q" => Some(Self::KeyQ),
            "R" => Some(Self::KeyR),
            "S" => Some(Self::KeyS),
            "T" => Some(Self::KeyT),
            "U" => Some(Self::KeyU),
            "V" => Some(Self::KeyV),
            "W" => Some(Self::KeyW),
            "X" => Some(Self::KeyX),
            "Y" => Some(Self::KeyY),
            "Z" => Some(Self::KeyZ),
            _ => None,
        }
    }
}

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

    pub fn to_hotkey(self) -> HotKey {
        let mut mods = Modifiers::empty();
        if self.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if self.alt {
            mods |= Modifiers::ALT;
        }
        if self.shift {
            mods |= Modifiers::SHIFT;
        }
        if self.super_key {
            mods |= Modifiers::SUPER;
        }
        let m = if mods.is_empty() { None } else { Some(mods) };
        HotKey::new(m, self.key.to_ghk())
    }

    pub fn to_accelerator(self) -> Accelerator {
        let mut mods = MudaModifiers::empty();
        if self.ctrl {
            mods |= MudaModifiers::CONTROL;
        }
        if self.alt {
            mods |= MudaModifiers::ALT;
        }
        if self.shift {
            mods |= MudaModifiers::SHIFT;
        }
        if self.super_key {
            mods |= MudaModifiers::SUPER;
        }
        let m = if mods.is_empty() { None } else { Some(mods) };
        Accelerator::new(m, self.key.to_muda())
    }

    /// Clean ASCII-safe display so Roboto renders zero missing-glyph boxes.
    pub fn display(self) -> String {
        let mut parts: Vec<&str> = Vec::new();
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
            #[cfg(not(target_os = "macos"))]
            parts.push("Win");
        }
        parts.push(self.key.label());
        parts.join(" + ")
    }

    fn serialize(self) -> String {
        format!(
            "{},{},{},{},{}",
            self.ctrl as u8,
            self.alt as u8,
            self.shift as u8,
            self.super_key as u8,
            self.key.label()
        )
    }

    fn deserialize(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split(',').collect();
        if parts.len() != 5 {
            return None;
        }
        Some(Self {
            ctrl: parts[0] == "1",
            alt: parts[1] == "1",
            shift: parts[2] == "1",
            super_key: parts[3] == "1",
            key: HotkeyCode::from_label(parts[4])?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TextFont {
    GostTypeAU = 0,
    TimesNewRoman = 1,
    Roboto = 2,
    SanFrancisco = 3,
    Arial = 4,
    ArialBlack = 5,
    Georgia = 6,
    Verdana = 7,
    Tahoma = 8,
    TrebuchetMS = 9,
    CourierNew = 10,
    Impact = 11,
    ComicSansMS = 12,
    SFRounded = 13,
    SFMono = 14,
}

impl TextFont {
    pub const ALL: [Self; 15] = [
        Self::GostTypeAU,
        Self::TimesNewRoman,
        Self::Roboto,
        Self::SanFrancisco,
        Self::Arial,
        Self::ArialBlack,
        Self::Georgia,
        Self::Verdana,
        Self::Tahoma,
        Self::TrebuchetMS,
        Self::CourierNew,
        Self::Impact,
        Self::ComicSansMS,
        Self::SFRounded,
        Self::SFMono,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::GostTypeAU => "gost_type_au",
            Self::TimesNewRoman => "times_new_roman",
            Self::Roboto => "roboto",
            Self::SanFrancisco => "sf",
            Self::Arial => "arial",
            Self::ArialBlack => "arial_black",
            Self::Georgia => "georgia",
            Self::Verdana => "verdana",
            Self::Tahoma => "tahoma",
            Self::TrebuchetMS => "trebuchet_ms",
            Self::CourierNew => "courier_new",
            Self::Impact => "impact",
            Self::ComicSansMS => "comic_sans_ms",
            Self::SFRounded => "sf_rounded",
            Self::SFMono => "sf_mono",
        }
    }

    pub fn from_code(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|f| f.code() == s.trim())
            .unwrap_or(Self::GostTypeAU)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GostTypeAU => "GOST Type AU",
            Self::TimesNewRoman => "Times New Roman",
            Self::Roboto => "Roboto (Google)",
            Self::SanFrancisco => "San Francisco (Apple)",
            Self::Arial => "Arial",
            Self::ArialBlack => "Arial Black",
            Self::Georgia => "Georgia",
            Self::Verdana => "Verdana",
            Self::Tahoma => "Tahoma",
            Self::TrebuchetMS => "Trebuchet MS",
            Self::CourierNew => "Courier New",
            Self::Impact => "Impact",
            Self::ComicSansMS => "Comic Sans MS",
            Self::SFRounded => "SF Rounded / Calibri",
            Self::SFMono => "SF Mono / Consolas",
        }
    }

    pub fn egui_family(self) -> &'static str {
        match self {
            Self::GostTypeAU => "text_gost_au",
            Self::TimesNewRoman => "text_times",
            Self::Roboto => "text_roboto",
            Self::SanFrancisco => "text_sf",
            Self::Arial => "text_arial",
            Self::ArialBlack => "text_arial_black",
            Self::Georgia => "text_georgia",
            Self::Verdana => "text_verdana",
            Self::Tahoma => "text_tahoma",
            Self::TrebuchetMS => "text_trebuchet",
            Self::CourierNew => "text_courier",
            Self::Impact => "text_impact",
            Self::ComicSansMS => "text_comic",
            Self::SFRounded => "text_sf_rounded",
            Self::SFMono => "text_sf_mono",
        }
    }

    pub fn system_path(self) -> Option<&'static str> {
        #[cfg(target_os = "macos")]
        {
            match self {
                Self::GostTypeAU | Self::Roboto => None,
                Self::TimesNewRoman => Some("/System/Library/Fonts/Supplemental/Times New Roman.ttf"),
                Self::SanFrancisco => Some("/System/Library/Fonts/SFNS.ttf"),
                Self::Arial => Some("/System/Library/Fonts/Supplemental/Arial.ttf"),
                Self::ArialBlack => Some("/System/Library/Fonts/Supplemental/Arial Black.ttf"),
                Self::Georgia => Some("/System/Library/Fonts/Supplemental/Georgia.ttf"),
                Self::Verdana => Some("/System/Library/Fonts/Supplemental/Verdana.ttf"),
                Self::Tahoma => Some("/System/Library/Fonts/Supplemental/Tahoma.ttf"),
                Self::TrebuchetMS => Some("/System/Library/Fonts/Supplemental/Trebuchet MS.ttf"),
                Self::CourierNew => Some("/System/Library/Fonts/Supplemental/Courier New.ttf"),
                Self::Impact => Some("/System/Library/Fonts/Supplemental/Impact.ttf"),
                Self::ComicSansMS => Some("/System/Library/Fonts/Supplemental/Comic Sans MS.ttf"),
                Self::SFRounded => Some("/System/Library/Fonts/SFNSRounded.ttf"),
                Self::SFMono => Some("/System/Library/Fonts/SFNSMono.ttf"),
            }
        }
        #[cfg(target_os = "windows")]
        {
            match self {
                Self::GostTypeAU | Self::Roboto => None,
                Self::TimesNewRoman => Some("C:\\Windows\\Fonts\\times.ttf"),
                Self::SanFrancisco => Some("C:\\Windows\\Fonts\\segoeui.ttf"),
                Self::Arial => Some("C:\\Windows\\Fonts\\arial.ttf"),
                Self::ArialBlack => Some("C:\\Windows\\Fonts\\ariblk.ttf"),
                Self::Georgia => Some("C:\\Windows\\Fonts\\georgia.ttf"),
                Self::Verdana => Some("C:\\Windows\\Fonts\\verdana.ttf"),
                Self::Tahoma => Some("C:\\Windows\\Fonts\\tahoma.ttf"),
                Self::TrebuchetMS => Some("C:\\Windows\\Fonts\\trebuc.ttf"),
                Self::CourierNew => Some("C:\\Windows\\Fonts\\cour.ttf"),
                Self::Impact => Some("C:\\Windows\\Fonts\\impact.ttf"),
                Self::ComicSansMS => Some("C:\\Windows\\Fonts\\comic.ttf"),
                Self::SFRounded => Some("C:\\Windows\\Fonts\\calibri.ttf"),
                Self::SFMono => Some("C:\\Windows\\Fonts\\consola.ttf"),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = self;
            None
        }
    }
}

use crate::annotations::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarLayout {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub language: Language,
    pub save_dir: PathBuf,
    pub hk_fullscreen: HotkeySpec,
    pub hk_area: HotkeySpec,
    pub hk_window: HotkeySpec,
    pub hk_live_draw: HotkeySpec,
    pub bg_color_pos: f32,
    pub accent_color_pos: f32,
    pub draw_color_pos: f32,
    pub draw_thickness: f32,
    pub snap_angle_deg: u16,
    pub text_font: TextFont,
    pub toolbar_layout: ToolbarLayout,
    pub last_tool: Tool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::En,
            save_dir: default_desktop_dir(),
            hk_fullscreen: HotkeySpec::opt_shift(HotkeyCode::Digit1),
            hk_area: HotkeySpec::opt_shift(HotkeyCode::Digit2),
            hk_window: HotkeySpec::opt_shift(HotkeyCode::Digit3),
            hk_live_draw: HotkeySpec::opt_shift(HotkeyCode::Digit4),
            bg_color_pos: 0.28,
            accent_color_pos: 0.56,
            draw_color_pos: 0.56,
            draw_thickness: 5.0,
            snap_angle_deg: 90,
            text_font: TextFont::GostTypeAU,
            toolbar_layout: ToolbarLayout::Horizontal,
            last_tool: Tool::Arrow,
        }
    }
}

pub fn default_desktop_dir() -> PathBuf {
    dirs::desktop_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ShotCraft").join("settings.conf")
}

impl AppSettings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let path = config_path();
        let content = fs::read_to_string(&path).or_else(|_| {
            let legacy = dirs::config_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("LightCraft")
                .join("settings.conf");
            fs::read_to_string(legacy)
        });
        let Ok(content) = content else {
            return s;
        };
        for line in content.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "language" => {
                    s.language = Language::from_code(v);
                }
                "save_dir" => {
                    let p = PathBuf::from(v.trim());
                    if !v.trim().is_empty() {
                        s.save_dir = p;
                    }
                }
                "hk_fullscreen" => {
                    if let Some(hk) = HotkeySpec::deserialize(v) {
                        s.hk_fullscreen = hk;
                    }
                }
                "hk_area" => {
                    if let Some(hk) = HotkeySpec::deserialize(v) {
                        s.hk_area = hk;
                    }
                }
                "hk_window" => {
                    if let Some(hk) = HotkeySpec::deserialize(v) {
                        s.hk_window = hk;
                    }
                }
                "hk_live_draw" => {
                    if let Some(hk) = HotkeySpec::deserialize(v) {
                        s.hk_live_draw = hk;
                    }
                }
                "bg_color_pos" => {
                    if let Ok(p) = v.trim().parse::<f32>() {
                        s.bg_color_pos = p.clamp(0.0, 1.0);
                    }
                }
                "accent_color_pos" => {
                    if let Ok(p) = v.trim().parse::<f32>() {
                        s.accent_color_pos = p.clamp(0.0, 1.0);
                    }
                }
                "draw_color_pos" => {
                    if let Ok(p) = v.trim().parse::<f32>() {
                        s.draw_color_pos = p.clamp(0.0, 1.0);
                    }
                }
                "draw_thickness" => {
                    if let Ok(t) = v.trim().parse::<f32>() {
                        s.draw_thickness = t.clamp(1.5, 20.0);
                    }
                }
                "snap_angle_deg" => {
                    if let Ok(deg) = v.trim().parse::<u16>() {
                        s.snap_angle_deg = deg.clamp(1, 90);
                    }
                }
                "text_font" => {
                    s.text_font = TextFont::from_code(v);
                }
                "toolbar_layout" => {
                    s.toolbar_layout = if v.trim() == "vertical" {
                        ToolbarLayout::Vertical
                    } else {
                        ToolbarLayout::Horizontal
                    };
                }
                "last_tool" => {
                    s.last_tool = Tool::from_code(v);
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let layout_str = match self.toolbar_layout {
            ToolbarLayout::Horizontal => "horizontal",
            ToolbarLayout::Vertical => "vertical",
        };
        let data = format!(
            "language={}\nsave_dir={}\nhk_fullscreen={}\nhk_area={}\nhk_window={}\nhk_live_draw={}\nbg_color_pos={:.3}\naccent_color_pos={:.3}\ndraw_color_pos={:.3}\ndraw_thickness={:.2}\nsnap_angle_deg={}\ntext_font={}\ntoolbar_layout={}\nlast_tool={}\n",
            self.language.code(),
            self.save_dir.display(),
            self.hk_fullscreen.serialize(),
            self.hk_area.serialize(),
            self.hk_window.serialize(),
            self.hk_live_draw.serialize(),
            self.bg_color_pos,
            self.accent_color_pos,
            self.draw_color_pos,
            self.draw_thickness,
            self.snap_angle_deg,
            self.text_font.code(),
            layout_str,
            self.last_tool.code(),
        );
        let _ = fs::write(path, data);
    }
}
