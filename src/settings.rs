use std::fs;
use std::path::PathBuf;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use muda::accelerator::{Accelerator, Code as MudaCode, Modifiers as MudaModifiers};

use crate::annotations::Tool;
use crate::diagnostics;

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
}

impl Language {
    pub const ALL: [Self; 10] = [
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
    pub settings_text_font: &'static str,
    pub settings_toolbar_layout: &'static str,
    pub layout_horizontal: &'static str,
    pub layout_vertical: &'static str,
    pub settings_bg_color: &'static str,
    pub settings_accent_color: &'static str,
    pub settings_shift_angle: &'static str,
    pub settings_angle_hint: &'static str,
    pub settings_folder: &'static str,
    pub settings_choose: &'static str,
    pub settings_desktop: &'static str,
    pub settings_hotkeys: &'static str,
    pub hk_full: &'static str,
    pub hk_area: &'static str,
    pub hk_window: &'static str,
    pub hk_live_draw: &'static str,
    pub hk_press: &'static str,
    pub hotkey_conflict: &'static str,
    pub hotkey_need_modifier: &'static str,
    pub tool_move: &'static str,
    pub tool_pen: &'static str,
    pub tool_marker: &'static str,
    pub tool_line: &'static str,
    pub tool_arrow: &'static str,
    pub tool_text: &'static str,
    pub tool_ellipse: &'static str,
    pub tool_rect: &'static str,
    pub tool_step: &'static str,
    pub tool_blur: &'static str,
    pub act_undo: &'static str,
    pub act_redo: &'static str,
    pub act_copy: &'static str,
    pub act_save: &'static str,
    pub act_close: &'static str,
    pub slider_color: &'static str,
    pub slider_size: &'static str,
    pub err_save_failed: &'static str,
    pub err_copy_failed: &'static str,
    pub err_capture_failed: &'static str,
    pub menu_check_updates: &'static str,
    pub menu_update_available: &'static str,
    pub settings_updates: &'static str,
    pub update_check: &'static str,
    pub update_checking: &'static str,
    pub update_up_to_date: &'static str,
    pub update_available: &'static str,
    pub update_install: &'static str,
    pub update_downloading: &'static str,
    pub update_opened: &'static str,
    pub update_failed: &'static str,
    pub settings_open_log: &'static str,
    pub wayland_hotkeys_note: &'static str,
}

impl Strings {
    pub fn primary_mod_label() -> &'static str {
        if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        }
    }

    pub fn with_mod(template: &str) -> String {
        template.replace("{MOD}", Self::primary_mod_label())
    }

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
                settings_text_font: "Шрифт (інструмент Текст)",
                settings_toolbar_layout: "Панель інструментів",
                layout_horizontal: "Горизонт.",
                layout_vertical: "Вертик.",
                settings_bg_color: "Колір фону",
                settings_accent_color: "Колір акценту",
                settings_shift_angle: "Кут Shift (1–90°)",
                settings_angle_hint: "Клікніть щоб ввести (1–90) або потягніть вліво/вправо",
                settings_folder: "Тека збереження скріншотів",
                settings_choose: "Обрати теку...",
                settings_desktop: "Робочий стіл",
                settings_hotkeys: "Гарячі клавіші",
                hk_full: "Весь екран",
                hk_area: "Область",
                hk_window: "Вікно",
                hk_live_draw: "Поверх екрану",
                hk_press: "Натисніть клавіші...",
                hotkey_conflict: "Конфлікт комбінації клавіш",
                hotkey_need_modifier: "Додайте модифікатор (Ctrl/Alt/Shift)",
                tool_move: "Переміщення / Розмір [1]",
                tool_pen: "Олівець [2]",
                tool_marker: "Маркер [3]",
                tool_line: "Лінія [4] (Shift — крок кута)",
                tool_arrow: "Стрілка [5] (Shift — крок кута)",
                tool_text: "Текст / Виноска [6]",
                tool_ellipse: "Еліпс [7] ({MOD} — заливка, Shift — коло)",
                tool_rect: "Прямокутник [8] ({MOD} — заливка, Shift — квадрат)",
                tool_step: "Нумерація кроків [9]",
                tool_blur: "Розмиття [0] ({MOD} — пікселізація)",
                act_undo: "Скасувати ({MOD}+Z)",
                act_redo: "Повернути ({MOD}+Shift+Z)",
                act_copy: "Копіювати ({MOD}+C / Enter)",
                act_save: "Зберегти ({MOD}+S)",
                act_close: "Закрити (Esc)",
                slider_color: "Колір",
                slider_size: "Товщина",
                err_save_failed: "Не вдалося зберегти скріншот",
                err_copy_failed: "Не вдалося скопіювати в буфер",
                err_capture_failed: "Помилка захоплення екрана",
                menu_check_updates: "Перевірити оновлення...",
                menu_update_available: "Встановити оновлення",
                settings_updates: "Оновлення",
                update_check: "Перевірити",
                update_checking: "Перевірка...",
                update_up_to_date: "Актуальна версія",
                update_available: "Доступно",
                update_install: "Оновити",
                update_downloading: "Завантаження...",
                update_opened: "Інсталятор відкрито",
                update_failed: "Помилка перевірки",
                settings_open_log: "Лог",
                wayland_hotkeys_note: "Wayland: користуйтесь меню в треї, якщо гарячі клавіші заблоковані",
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
                settings_text_font: "Fuente de texto",
                settings_toolbar_layout: "Barra de herramientas",
                layout_horizontal: "Horizontal",
                layout_vertical: "Vertical",
                settings_bg_color: "Color de fondo",
                settings_accent_color: "Color de acento",
                settings_shift_angle: "Ángulo Shift (1–90°)",
                settings_angle_hint: "Haz clic para escribir (1–90) o arrastra izq./der.",
                settings_folder: "Carpeta de capturas",
                settings_choose: "Elegir carpeta...",
                settings_desktop: "Escritorio",
                settings_hotkeys: "Atajos globales",
                hk_full: "Pantalla completa",
                hk_area: "Capturar área",
                hk_window: "Capturar ventana",
                hk_live_draw: "Dibujar en pantalla",
                hk_press: "Pulsa teclas...",
                hotkey_conflict: "El atajo ya está asignado",
                hotkey_need_modifier: "Usa al menos una tecla modificadora",
                tool_move: "Mover / Redimensionar [1]",
                tool_pen: "Lápiz [2]",
                tool_marker: "Resaltador [3]",
                tool_line: "Línea [4] (Shift — ajustar ángulo)",
                tool_arrow: "Flecha [5] (Shift — ajustar ángulo)",
                tool_text: "Texto / Llamada [6]",
                tool_ellipse: "Elipse [7] ({MOD} — relleno, Shift — círculo)",
                tool_rect: "Rectángulo [8] ({MOD} — relleno, Shift — cuadrado)",
                tool_step: "Numeración [9]",
                tool_blur: "Desenfoque [0] ({MOD} — pixelar)",
                act_undo: "Deshacer ({MOD}+Z)",
                act_redo: "Rehacer ({MOD}+Shift+Z)",
                act_copy: "Copiar ({MOD}+C / Enter)",
                act_save: "Guardar ({MOD}+S)",
                act_close: "Cerrar (Esc)",
                slider_color: "Color",
                slider_size: "Tamaño",
                err_save_failed: "Error al guardar la captura",
                err_copy_failed: "Error al copiar al portapapeles",
                err_capture_failed: "Error al capturar la pantalla",
                menu_check_updates: "Buscar actualizaciones...",
                menu_update_available: "Instalar actualización",
                settings_updates: "Actualizaciones",
                update_check: "Buscar",
                update_checking: "Buscando...",
                update_up_to_date: "Actualizado",
                update_available: "Disponible",
                update_install: "Actualizar",
                update_downloading: "Descargando...",
                update_opened: "Instalador abierto",
                update_failed: "Error al buscar",
                settings_open_log: "Registro",
                wayland_hotkeys_note: "Wayland: usa el menú de la bandeja si los atajos globales fallan",
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
                settings_text_font: "Police du texte",
                settings_toolbar_layout: "Barre d'outils",
                layout_horizontal: "Horizontale",
                layout_vertical: "Verticale",
                settings_bg_color: "Couleur de fond",
                settings_accent_color: "Couleur d'accent",
                settings_shift_angle: "Pas d'angle Shift (1–90°)",
                settings_angle_hint: "Cliquer pour saisir (1–90) ou glisser gauche/droite",
                settings_folder: "Dossier d'enregistrement",
                settings_choose: "Choisir...",
                settings_desktop: "Bureau",
                settings_hotkeys: "Raccourcis globaux",
                hk_full: "Plein écran",
                hk_area: "Capturer une zone",
                hk_window: "Capturer une fenêtre",
                hk_live_draw: "Dessiner à l'écran",
                hk_press: "Appuyez...",
                hotkey_conflict: "Raccourci déjà utilisé",
                hotkey_need_modifier: "Ajoutez un modificateur (Ctrl/Alt/Shift)",
                tool_move: "Déplacer / Taille [1]",
                tool_pen: "Crayon [2]",
                tool_marker: "Surligneur [3]",
                tool_line: "Ligne [4] (Shift — aimanter l'angle)",
                tool_arrow: "Flèche [5] (Shift — aimanter l'angle)",
                tool_text: "Texte / Bulle [6]",
                tool_ellipse: "Ellipse [7] ({MOD} — plein, Shift — cercle)",
                tool_rect: "Rectangle [8] ({MOD} — plein, Shift — carré)",
                tool_step: "Numérotation [9]",
                tool_blur: "Flou [0] ({MOD} — pixeliser)",
                act_undo: "Annuler ({MOD}+Z)",
                act_redo: "Rétablir ({MOD}+Shift+Z)",
                act_copy: "Copier ({MOD}+C / Entrée)",
                act_save: "Enregistrer ({MOD}+S)",
                act_close: "Fermer (Échap)",
                slider_color: "Couleur",
                slider_size: "Épaisseur",
                err_save_failed: "Échec de l'enregistrement",
                err_copy_failed: "Échec de la copie",
                err_capture_failed: "Échec de la capture",
                menu_check_updates: "Vérifier les mises à jour...",
                menu_update_available: "Installer la mise à jour",
                settings_updates: "Mises à jour",
                update_check: "Vérifier",
                update_checking: "Recherche...",
                update_up_to_date: "À jour",
                update_available: "Disponible",
                update_install: "Mettre à jour",
                update_downloading: "Téléchargement...",
                update_opened: "Installateur ouvert",
                update_failed: "Échec",
                settings_open_log: "Journal",
                wayland_hotkeys_note: "Wayland : utilisez le menu de la barre si les raccourcis sont bloqués",
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
                settings_text_font: "Textschriftart",
                settings_toolbar_layout: "Werkzeugleiste",
                layout_horizontal: "Horizontal",
                layout_vertical: "Vertikal",
                settings_bg_color: "Hintergrundfarbe",
                settings_accent_color: "Akzentfarbe",
                settings_shift_angle: "Shift-Winkel (1–90°)",
                settings_angle_hint: "Klicken zum Eingeben (1–90) oder links/rechts ziehen",
                settings_folder: "Speicherordner",
                settings_choose: "Ordner wählen...",
                settings_desktop: "Schreibtisch",
                settings_hotkeys: "Tastenkürzel",
                hk_full: "Vollbild",
                hk_area: "Bereich aufnehmen",
                hk_window: "Fenster aufnehmen",
                hk_live_draw: "Auf Bildschirm zeichnen",
                hk_press: "Taste drücken...",
                hotkey_conflict: "Tastenkürzel bereits belegt",
                hotkey_need_modifier: "Mindestens eine Zusatztaste wählen",
                tool_move: "Verschieben / Größe [1]",
                tool_pen: "Stift [2]",
                tool_marker: "Textmarker [3]",
                tool_line: "Linie [4] (Shift — Winkel einrasten)",
                tool_arrow: "Pfeil [5] (Shift — Winkel einrasten)",
                tool_text: "Text / Callout [6]",
                tool_ellipse: "Ellipse [7] ({MOD} — gefüllt, Shift — Kreis)",
                tool_rect: "Rechteck [8] ({MOD} — gefüllt, Shift — Quadrat)",
                tool_step: "Nummerierung [9]",
                tool_blur: "Weichzeichnen [0] ({MOD} — verpixeln)",
                act_undo: "Rückgängig ({MOD}+Z)",
                act_redo: "Wiederholen ({MOD}+Shift+Z)",
                act_copy: "Kopieren ({MOD}+C / Enter)",
                act_save: "Speichern ({MOD}+S)",
                act_close: "Schließen (Esc)",
                slider_color: "Farbe",
                slider_size: "Stärke",
                err_save_failed: "Speichern fehlgeschlagen",
                err_copy_failed: "Kopieren fehlgeschlagen",
                err_capture_failed: "Bildschirmaufnahme fehlgeschlagen",
                menu_check_updates: "Nach Updates suchen...",
                menu_update_available: "Update installieren",
                settings_updates: "Updates",
                update_check: "Prüfen",
                update_checking: "Prüfe...",
                update_up_to_date: "Aktuell",
                update_available: "Verfügbar",
                update_install: "Aktualisieren",
                update_downloading: "Lade herunter...",
                update_opened: "Installer geöffnet",
                update_failed: "Prüfung fehlgeschlagen",
                settings_open_log: "Log",
                wayland_hotkeys_note: "Wayland: Nutzen Sie das Tray-Menü, falls globale Kürzel blockiert sind",
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
                settings_text_font: "Czcionka tekstu",
                settings_toolbar_layout: "Pasek narzędzi",
                layout_horizontal: "Poziomy",
                layout_vertical: "Pionowy",
                settings_bg_color: "Kolor tła",
                settings_accent_color: "Kolor akcentu",
                settings_shift_angle: "Kąt Shift (1–90°)",
                settings_angle_hint: "Kliknij, aby wpisać (1–90) lub przeciągnij lewo/prawo",
                settings_folder: "Folder zapisu zrzutów",
                settings_choose: "Wybierz folder...",
                settings_desktop: "Pulpit",
                settings_hotkeys: "Skróty klawiszowe",
                hk_full: "Pełny ekran",
                hk_area: "Zaznacz obszar",
                hk_window: "Przechwyć okno",
                hk_live_draw: "Rysuj na ekranie",
                hk_press: "Naciśnij klawisze...",
                hotkey_conflict: "Konflikt skrótu klawiszowego",
                hotkey_need_modifier: "Wymagany klawisz modyfikujący",
                tool_move: "Przesuń / Rozmiar [1]",
                tool_pen: "Ołówek [2]",
                tool_marker: "Zakreślacz [3]",
                tool_line: "Linia [4] (Shift — przyciąganie kąta)",
                tool_arrow: "Strzałka [5] (Shift — przyciąganie kąta)",
                tool_text: "Tekst / Odnośnik [6]",
                tool_ellipse: "Elipsa [7] ({MOD} — wypełnienie, Shift — koło)",
                tool_rect: "Prostokąt [8] ({MOD} — wypełnienie, Shift — kwadrat)",
                tool_step: "Numeracja [9]",
                tool_blur: "Rozmycie [0] ({MOD} — pikselizacja)",
                act_undo: "Cofnij ({MOD}+Z)",
                act_redo: "Ponów ({MOD}+Shift+Z)",
                act_copy: "Kopiuj ({MOD}+C / Enter)",
                act_save: "Zapisz ({MOD}+S)",
                act_close: "Zamknij (Esc)",
                slider_color: "Kolor",
                slider_size: "Grubość",
                err_save_failed: "Nie udało się zapisać zrzutu",
                err_copy_failed: "Nie udało się skopiować do schowka",
                err_capture_failed: "Błąd przechwytywania ekranu",
                menu_check_updates: "Sprawdź aktualizacje...",
                menu_update_available: "Zainstaluj aktualizację",
                settings_updates: "Aktualizacje",
                update_check: "Sprawdź",
                update_checking: "Sprawdzanie...",
                update_up_to_date: "Aktualna wersja",
                update_available: "Dostępna",
                update_install: "Aktualizuj",
                update_downloading: "Pobieranie...",
                update_opened: "Instalator otwarty",
                update_failed: "Błąd sprawdzania",
                settings_open_log: "Logi",
                wayland_hotkeys_note: "Wayland: użyj menu w zasobniku, jeśli skróty globalne nie działają",
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
                settings_text_font: "Carattere testo",
                settings_toolbar_layout: "Barra strumenti",
                layout_horizontal: "Orizzont.",
                layout_vertical: "Verticale",
                settings_bg_color: "Colore sfondo",
                settings_accent_color: "Colore accento",
                settings_shift_angle: "Angolo Shift (1–90°)",
                settings_angle_hint: "Clicca per scrivere (1–90) o trascina sinistra/destra",
                settings_folder: "Cartella di salvataggio",
                settings_choose: "Scegli cartella...",
                settings_desktop: "Scrivania",
                settings_hotkeys: "Scorciatoie globali",
                hk_full: "Schermo intero",
                hk_area: "Cattura area",
                hk_window: "Cattura finestra",
                hk_live_draw: "Disegna sullo schermo",
                hk_press: "Premi tasti...",
                hotkey_conflict: "Scorciatoia già in uso",
                hotkey_need_modifier: "Usa almeno un tasto modificatore",
                tool_move: "Sposta / Ridimensiona [1]",
                tool_pen: "Penna [2]",
                tool_marker: "Evidenziatore [3]",
                tool_line: "Linea [4] (Shift — aggancia angolo)",
                tool_arrow: "Freccia [5] (Shift — aggancia angolo)",
                tool_text: "Testo / Richiamo [6]",
                tool_ellipse: "Ellisse [7] ({MOD} — pieno, Shift — cerchio)",
                tool_rect: "Rettangolo [8] ({MOD} — pieno, Shift — quadrato)",
                tool_step: "Numerazione [9]",
                tool_blur: "Sfocatura [0] ({MOD} — pixel)",
                act_undo: "Annulla ({MOD}+Z)",
                act_redo: "Ripeti ({MOD}+Shift+Z)",
                act_copy: "Copia ({MOD}+C / Invio)",
                act_save: "Salva ({MOD}+S)",
                act_close: "Chiudi (Esc)",
                slider_color: "Colore",
                slider_size: "Spessore",
                err_save_failed: "Salvataggio non riuscito",
                err_copy_failed: "Copia negli appunti non riuscita",
                err_capture_failed: "Cattura schermo non riuscita",
                menu_check_updates: "Verifica aggiornamenti...",
                menu_update_available: "Installa aggiornamento",
                settings_updates: "Aggiornamenti",
                update_check: "Verifica",
                update_checking: "Verifica...",
                update_up_to_date: "Aggiornato",
                update_available: "Disponibile",
                update_install: "Aggiorna",
                update_downloading: "Download...",
                update_opened: "Installer aperto",
                update_failed: "Errore verifica",
                settings_open_log: "Log",
                wayland_hotkeys_note: "Wayland: usa il menu nella tray se le scorciatoie sono bloccate",
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
                settings_text_font: "Fonte do texto",
                settings_toolbar_layout: "Barra de ferramentas",
                layout_horizontal: "Horizontal",
                layout_vertical: "Vertical",
                settings_bg_color: "Cor de fundo",
                settings_accent_color: "Cor de destaque",
                settings_shift_angle: "Ângulo Shift (1–90°)",
                settings_angle_hint: "Clique para digitar (1–90) ou arraste esq./dir.",
                settings_folder: "Pasta de capturas",
                settings_choose: "Escolher pasta...",
                settings_desktop: "Área de Trabalho",
                settings_hotkeys: "Atalhos globais",
                hk_full: "Tela cheia",
                hk_area: "Capturar área",
                hk_window: "Capturar janela",
                hk_live_draw: "Desenhar na tela",
                hk_press: "Pressione teclas...",
                hotkey_conflict: "Atalho já está em uso",
                hotkey_need_modifier: "Use pelo menos uma tecla modificadora",
                tool_move: "Mover / Redimensionar [1]",
                tool_pen: "Caneta [2]",
                tool_marker: "Marcador [3]",
                tool_line: "Linha [4] (Shift — ajustar ângulo)",
                tool_arrow: "Seta [5] (Shift — ajustar ângulo)",
                tool_text: "Texto / Balão [6]",
                tool_ellipse: "Elipse [7] ({MOD} — preenchido, Shift — círculo)",
                tool_rect: "Retângulo [8] ({MOD} — preenchido, Shift — quadrado)",
                tool_step: "Numeração [9]",
                tool_blur: "Desfoque [0] ({MOD} — pixelizar)",
                act_undo: "Desfazer ({MOD}+Z)",
                act_redo: "Refazer ({MOD}+Shift+Z)",
                act_copy: "Copiar ({MOD}+C / Enter)",
                act_save: "Salvar ({MOD}+S)",
                act_close: "Fechar (Esc)",
                slider_color: "Cor",
                slider_size: "Tamanho",
                err_save_failed: "Falha ao salvar captura",
                err_copy_failed: "Falha ao copiar para a área de transferência",
                err_capture_failed: "Falha ao capturar a tela",
                menu_check_updates: "Verificar atualizações...",
                menu_update_available: "Instalar atualização",
                settings_updates: "Atualizações",
                update_check: "Verificar",
                update_checking: "Verificando...",
                update_up_to_date: "Atualizado",
                update_available: "Disponível",
                update_install: "Atualizar",
                update_downloading: "Baixando...",
                update_opened: "Instalador aberto",
                update_failed: "Falha ao verificar",
                settings_open_log: "Log",
                wayland_hotkeys_note: "Wayland: use o menu da bandeja se os atalhos globais falharem",
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
                settings_text_font: "Metin Yazı Tipi",
                settings_toolbar_layout: "Araç Çubuğu Yönü",
                layout_horizontal: "Yatay",
                layout_vertical: "Dikey",
                settings_bg_color: "Arka Plan Rengi",
                settings_accent_color: "Vurgu Rengi",
                settings_shift_angle: "Shift Açı Adımı (1–90°)",
                settings_angle_hint: "Yazmak için tıklayın (1–90) veya sağa/sola sürükleyin",
                settings_folder: "Kayıt Klasörü",
                settings_choose: "Klasör Seç...",
                settings_desktop: "Masaüstü",
                settings_hotkeys: "Kısayollar",
                hk_full: "Tam Ekran",
                hk_area: "Alan Yakala",
                hk_window: "Pencere Yakala",
                hk_live_draw: "Ekran Üzerine Çiz",
                hk_press: "Tuşlara basın...",
                hotkey_conflict: "Kısayol çakışması",
                hotkey_need_modifier: "En az bir değiştirici tuş ekleyin",
                tool_move: "Taşı / Boyutlandır [1]",
                tool_pen: "Kalem [2]",
                tool_marker: "Fosforlu Kalem [3]",
                tool_line: "Çizgi [4] (Shift — açıya yasla)",
                tool_arrow: "Ok [5] (Shift — açıya yasla)",
                tool_text: "Metin / Açıklama [6]",
                tool_ellipse: "Elips [7] ({MOD} — dolu, Shift — daire)",
                tool_rect: "Dikdörtgen [8] ({MOD} — dolu, Shift — kare)",
                tool_step: "Numaralandırma [9]",
                tool_blur: "Bulanıklaştır [0] ({MOD} — pikselleştir)",
                act_undo: "Geri Al ({MOD}+Z)",
                act_redo: "Yinele ({MOD}+Shift+Z)",
                act_copy: "Kopyala ({MOD}+C / Enter)",
                act_save: "Kaydet ({MOD}+S)",
                act_close: "Kapat (Esc)",
                slider_color: "Renk",
                slider_size: "Boyut",
                err_save_failed: "Ekran görüntüsü kaydedilemedi",
                err_copy_failed: "Panoya kopyalanamadı",
                err_capture_failed: "Ekran yakalama başarısız",
                menu_check_updates: "Güncellemeleri Denetle...",
                menu_update_available: "Güncellemeyi Yükle",
                settings_updates: "Güncellemeler",
                update_check: "Denetle",
                update_checking: "Denetleniyor...",
                update_up_to_date: "Güncel",
                update_available: "Mevcut",
                update_install: "Güncelle",
                update_downloading: "İndiriliyor...",
                update_opened: "Yükleyici açıldı",
                update_failed: "Denetim hatası",
                settings_open_log: "Günlük",
                wayland_hotkeys_note: "Wayland: genel kısayollar engellenirse tepsi menüsünü kullanın",
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
                settings_text_font: "Tekstlettertype",
                settings_toolbar_layout: "Werkbalkrichting",
                layout_horizontal: "Horizont.",
                layout_vertical: "Verticaal",
                settings_bg_color: "Achtergrondkleur",
                settings_accent_color: "Accentkleur",
                settings_shift_angle: "Shift-hoekstap (1–90°)",
                settings_angle_hint: "Klik om te typen (1–90) of sleep links/rechts",
                settings_folder: "Opslagmap",
                settings_choose: "Map kiezen...",
                settings_desktop: "Bureaublad",
                settings_hotkeys: "Sneltoetsen",
                hk_full: "Volledig scherm",
                hk_area: "Gebied vastleggen",
                hk_window: "Venster vastleggen",
                hk_live_draw: "Teken op scherm",
                hk_press: "Druk op toetsen...",
                hotkey_conflict: "Sneltoets is al in gebruik",
                hotkey_need_modifier: "Gebruik minstens één modificatietoets",
                tool_move: "Verplaatsen / Grootte [1]",
                tool_pen: "Pen [2]",
                tool_marker: "Markeerstift [3]",
                tool_line: "Lijn [4] (Shift — hoek uitlijnen)",
                tool_arrow: "Pijl [5] (Shift — hoek uitlijnen)",
                tool_text: "Tekst / Callout [6]",
                tool_ellipse: "Ellips [7] ({MOD} — gevuld, Shift — cirkel)",
                tool_rect: "Rechthoek [8] ({MOD} — gevuld, Shift — vierkant)",
                tool_step: "Nummering [9]",
                tool_blur: "Vervagen [0] ({MOD} — pixelen)",
                act_undo: "Ongedaan maken ({MOD}+Z)",
                act_redo: "Opnieuw ({MOD}+Shift+Z)",
                act_copy: "Kopiëren ({MOD}+C / Enter)",
                act_save: "Opslaan ({MOD}+S)",
                act_close: "Sluiten (Esc)",
                slider_color: "Kleur",
                slider_size: "Dikte",
                err_save_failed: "Opslaan van schermafbeelding mislukt",
                err_copy_failed: "Kopiëren naar klembord mislukt",
                err_capture_failed: "Scherm vastleggen mislukt",
                menu_check_updates: "Controleren op updates...",
                menu_update_available: "Update installeren",
                settings_updates: "Updates",
                update_check: "Controleren",
                update_checking: "Controleren...",
                update_up_to_date: "Up-to-date",
                update_available: "Beschikbaar",
                update_install: "Nu updaten",
                update_downloading: "Downloaden...",
                update_opened: "Installatie geopend",
                update_failed: "Controle mislukt",
                settings_open_log: "Logboek",
                wayland_hotkeys_note: "Wayland: gebruik het systeemvakmenu als sneltoetsen zijn geblokkeerd",
            },
            Language::En => Self {
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
                settings_text_font: "Text Tool Font",
                settings_toolbar_layout: "Toolbar Orientation",
                layout_horizontal: "Horizontal",
                layout_vertical: "Vertical",
                settings_bg_color: "Background Color",
                settings_accent_color: "Accent Color",
                settings_shift_angle: "Shift Angle Step (1–90°)",
                settings_angle_hint: "Click to type (1–90) or drag left/right",
                settings_folder: "Screenshots Save Folder",
                settings_choose: "Choose Folder...",
                settings_desktop: "Desktop",
                settings_hotkeys: "Global Hotkeys",
                hk_full: "Fullscreen",
                hk_area: "Area",
                hk_window: "Window",
                hk_live_draw: "Screen Draw",
                hk_press: "Press keys...",
                hotkey_conflict: "Hotkey already assigned",
                hotkey_need_modifier: "Include a modifier (Ctrl/Alt/Shift)",
                tool_move: "Move / Resize [1]",
                tool_pen: "Pen [2]",
                tool_marker: "Marker [3]",
                tool_line: "Line [4] (Shift — snap angle)",
                tool_arrow: "Arrow [5] (Shift — snap angle)",
                tool_text: "Text / Callout [6]",
                tool_ellipse: "Ellipse [7] ({MOD} — filled, Shift — circle)",
                tool_rect: "Rectangle [8] ({MOD} — filled, Shift — square)",
                tool_step: "Numbering [9]",
                tool_blur: "Blur [0] ({MOD} — pixelate)",
                act_undo: "Undo ({MOD}+Z)",
                act_redo: "Redo ({MOD}+Shift+Z)",
                act_copy: "Copy ({MOD}+C / Enter)",
                act_save: "Save ({MOD}+S)",
                act_close: "Close (Esc)",
                slider_color: "Color",
                slider_size: "Size",
                err_save_failed: "Failed to save screenshot",
                err_copy_failed: "Failed to copy to clipboard",
                err_capture_failed: "Screen capture failed",
                menu_check_updates: "Check for Updates...",
                menu_update_available: "Install Update",
                settings_updates: "Updates",
                update_check: "Check",
                update_checking: "Checking...",
                update_up_to_date: "Up to date",
                update_available: "Available",
                update_install: "Update now",
                update_downloading: "Downloading...",
                update_opened: "Installer opened",
                update_failed: "Check failed",
                settings_open_log: "Log",
                wayland_hotkeys_note: "Wayland: use the tray menu if global hotkeys are blocked",
            },
        }
    }
}

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

    pub fn has_modifier(self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key
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
        Self::Roboto,
        Self::GostTypeAU,
        Self::TimesNewRoman,
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
            .unwrap_or(Self::Roboto)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GostTypeAU => "GOST Type AU (System)",
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

    /// Candidate system font paths across macOS, Windows, and Linux.
    pub fn system_paths(self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if self == Self::Roboto {
            return paths;
        }

        if self == Self::GostTypeAU {
            #[cfg(target_os = "macos")]
            {
                if let Some(home) = dirs::home_dir() {
                    paths.push(home.join("Library/Fonts/GOST_Type_AU.ttf"));
                    paths.push(home.join("Library/Fonts/OpenGostTypeA-Regular.ttf"));
                }
                paths.push(PathBuf::from("/Library/Fonts/GOST_Type_AU.ttf"));
                paths.push(PathBuf::from("/Library/Fonts/OpenGostTypeA-Regular.ttf"));
            }
            #[cfg(target_os = "windows")]
            {
                paths.push(PathBuf::from("C:\\Windows\\Fonts\\GOST_AU.TTF"));
                paths.push(PathBuf::from("C:\\Windows\\Fonts\\GOST_A.TTF"));
                paths.push(PathBuf::from("C:\\Windows\\Fonts\\GOST_Type_AU.ttf"));
                if let Some(local) = dirs::data_local_dir() {
                    paths.push(local.join("Microsoft\\Windows\\Fonts\\GOST_Type_AU.ttf"));
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                if let Some(data_dir) = dirs::data_local_dir() {
                    paths.push(data_dir.join("fonts/GOST_Type_AU.ttf"));
                    paths.push(data_dir.join("fonts/OpenGostTypeA-Regular.ttf"));
                }
                paths.push(PathBuf::from(
                    "/usr/share/fonts/truetype/opengost/OpenGostTypeA-Regular.ttf",
                ));
                paths.push(PathBuf::from(
                    "/usr/share/fonts/TTF/OpenGostTypeA-Regular.ttf",
                ));
            }
            return paths;
        }

        #[cfg(target_os = "macos")]
        {
            let p = match self {
                Self::TimesNewRoman => "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
                Self::SanFrancisco => "/System/Library/Fonts/SFNS.ttf",
                Self::Arial => "/System/Library/Fonts/Supplemental/Arial.ttf",
                Self::ArialBlack => "/System/Library/Fonts/Supplemental/Arial Black.ttf",
                Self::Georgia => "/System/Library/Fonts/Supplemental/Georgia.ttf",
                Self::Verdana => "/System/Library/Fonts/Supplemental/Verdana.ttf",
                Self::Tahoma => "/System/Library/Fonts/Supplemental/Tahoma.ttf",
                Self::TrebuchetMS => "/System/Library/Fonts/Supplemental/Trebuchet MS.ttf",
                Self::CourierNew => "/System/Library/Fonts/Supplemental/Courier New.ttf",
                Self::Impact => "/System/Library/Fonts/Supplemental/Impact.ttf",
                Self::ComicSansMS => "/System/Library/Fonts/Supplemental/Comic Sans MS.ttf",
                Self::SFRounded => "/System/Library/Fonts/SFNSRounded.ttf",
                Self::SFMono => "/System/Library/Fonts/SFNSMono.ttf",
                Self::GostTypeAU | Self::Roboto => "",
            };
            if !p.is_empty() {
                paths.push(PathBuf::from(p));
            }
        }
        #[cfg(target_os = "windows")]
        {
            let p = match self {
                Self::TimesNewRoman => "C:\\Windows\\Fonts\\times.ttf",
                Self::SanFrancisco => "C:\\Windows\\Fonts\\segoeui.ttf",
                Self::Arial => "C:\\Windows\\Fonts\\arial.ttf",
                Self::ArialBlack => "C:\\Windows\\Fonts\\ariblk.ttf",
                Self::Georgia => "C:\\Windows\\Fonts\\georgia.ttf",
                Self::Verdana => "C:\\Windows\\Fonts\\verdana.ttf",
                Self::Tahoma => "C:\\Windows\\Fonts\\tahoma.ttf",
                Self::TrebuchetMS => "C:\\Windows\\Fonts\\trebuc.ttf",
                Self::CourierNew => "C:\\Windows\\Fonts\\cour.ttf",
                Self::Impact => "C:\\Windows\\Fonts\\impact.ttf",
                Self::ComicSansMS => "C:\\Windows\\Fonts\\comic.ttf",
                Self::SFRounded => "C:\\Windows\\Fonts\\calibri.ttf",
                Self::SFMono => "C:\\Windows\\Fonts\\consola.ttf",
                Self::GostTypeAU | Self::Roboto => "",
            };
            if !p.is_empty() {
                paths.push(PathBuf::from(p));
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let candidates: &[&str] = match self {
                Self::TimesNewRoman => &[
                    "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf",
                ],
                Self::Georgia => &[
                    "/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf",
                    "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
                ],
                Self::SanFrancisco => &[
                    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                ],
                Self::Arial | Self::Verdana | Self::Tahoma | Self::TrebuchetMS => &[
                    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
                ],
                Self::ArialBlack | Self::Impact => &[
                    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
                ],
                Self::CourierNew | Self::SFMono => &[
                    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
                    "/usr/share/fonts/truetype/freefont/FreeMono.ttf",
                ],
                Self::ComicSansMS => &[
                    "/usr/share/fonts/truetype/comic-neue/ComicNeue-Regular.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                ],
                Self::SFRounded => &[
                    "/usr/share/fonts/truetype/crosextra/Carlito-Regular.ttf",
                    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                ],
                Self::GostTypeAU | Self::Roboto => &[],
            };
            for &c in candidates {
                paths.push(PathBuf::from(c));
            }
        }

        paths
    }

    /// Read the first available system font file for this variant.
    pub fn resolve_font_bytes(self) -> Option<Vec<u8>> {
        self.system_paths()
            .into_iter()
            .find_map(|p| fs::read(p).ok())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarLayout {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
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
            text_font: TextFont::Roboto,
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
    pub fn hotkey_conflict(&self, slot: HotkeySlot, spec: HotkeySpec) -> Option<HotkeySlot> {
        let pairs = [
            (HotkeySlot::Fullscreen, self.hk_fullscreen),
            (HotkeySlot::Area, self.hk_area),
            (HotkeySlot::Window, self.hk_window),
            (HotkeySlot::LiveDraw, self.hk_live_draw),
        ];
        pairs
            .into_iter()
            .find(|(s, existing)| *s != slot && *existing == spec)
            .map(|(s, _)| s)
    }

    pub fn load() -> Self {
        let path = config_path();
        let content = fs::read_to_string(&path).or_else(|_| {
            let legacy = dirs::config_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("LightCraft")
                .join("settings.conf");
            fs::read_to_string(legacy)
        });
        match content {
            Ok(text) => Self::parse_from_str(&text),
            Err(_) => Self::default(),
        }
    }

    pub fn parse_from_str(content: &str) -> Self {
        let mut s = Self::default();
        for line in content.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "language" => {
                    s.language = Language::from_code(v);
                }
                "save_dir" => {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        s.save_dir = PathBuf::from(trimmed);
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
                        if p.is_finite() {
                            s.bg_color_pos = p.clamp(0.0, 1.0);
                        }
                    }
                }
                "accent_color_pos" => {
                    if let Ok(p) = v.trim().parse::<f32>() {
                        if p.is_finite() {
                            s.accent_color_pos = p.clamp(0.0, 1.0);
                        }
                    }
                }
                "draw_color_pos" => {
                    if let Ok(p) = v.trim().parse::<f32>() {
                        if p.is_finite() {
                            s.draw_color_pos = p.clamp(0.0, 1.0);
                        }
                    }
                }
                "draw_thickness" => {
                    if let Ok(t) = v.trim().parse::<f32>() {
                        if t.is_finite() {
                            s.draw_thickness = t.clamp(1.5, 20.0);
                        }
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

    pub fn serialize_to_string(&self) -> String {
        let layout_str = match self.toolbar_layout {
            ToolbarLayout::Horizontal => "horizontal",
            ToolbarLayout::Vertical => "vertical",
        };
        format!(
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
        )
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                diagnostics::log_error("settings.save", &format!("mkdir failed: {e}"));
                return;
            }
        }
        let data = self.serialize_to_string();
        let tmp = path.with_extension("conf.tmp");
        if let Err(e) = fs::write(&tmp, &data) {
            diagnostics::log_error("settings.save", &format!("write tmp failed: {e}"));
            return;
        }
        if let Err(rename_err) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            if let Err(write_err) = fs::write(&path, &data) {
                diagnostics::log_error(
                    "settings.save",
                    &format!("rename ({rename_err}) & fallback write ({write_err}) failed"),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_roundtrip_and_fallback() {
        assert_eq!(Language::ALL.len(), 10);
        for lang in Language::ALL {
            assert_eq!(Language::from_code(lang.code()), lang);
            let s = Strings::for_lang(lang);
            assert!(!s.settings_title.is_empty());
            assert!(!s.err_save_failed.is_empty());
        }
        assert_eq!(Language::from_code("zh"), Language::En);
        assert_eq!(Language::from_code("unknown"), Language::En);
    }

    #[test]
    fn hotkey_spec_roundtrip_and_validation() {
        let spec = HotkeySpec {
            ctrl: true,
            alt: false,
            shift: true,
            super_key: false,
            key: HotkeyCode::KeyS,
        };
        let s = spec.serialize();
        assert_eq!(HotkeySpec::deserialize(&s), Some(spec));
        assert!(spec.has_modifier());

        let bare = HotkeySpec {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            key: HotkeyCode::Digit1,
        };
        assert!(!bare.has_modifier());

        assert!(HotkeySpec::deserialize("1,0,1,0").is_none());
        assert!(HotkeySpec::deserialize("2,0,1,0,A").is_none());
        assert!(HotkeySpec::deserialize("1,0,1,0,UNKNOWN").is_none());
    }

    #[test]
    fn settings_serialize_parse_roundtrip_and_clamping() {
        let mut s = AppSettings::default();
        s.language = Language::Uk;
        s.save_dir = PathBuf::from("/tmp/shotcraft_shots");
        s.bg_color_pos = 0.75;
        s.accent_color_pos = 0.125;
        s.draw_color_pos = 0.5;
        s.draw_thickness = 12.5;
        s.snap_angle_deg = 45;
        s.text_font = TextFont::CourierNew;
        s.toolbar_layout = ToolbarLayout::Vertical;
        s.last_tool = Tool::Blur;

        let serialized = s.serialize_to_string();
        let parsed = AppSettings::parse_from_str(&serialized);
        assert_eq!(parsed.language, s.language);
        assert_eq!(parsed.save_dir, s.save_dir);
        assert_eq!(parsed.snap_angle_deg, 45);
        assert_eq!(parsed.text_font, TextFont::CourierNew);
        assert_eq!(parsed.toolbar_layout, ToolbarLayout::Vertical);
        assert_eq!(parsed.last_tool, Tool::Blur);
        assert!((parsed.draw_thickness - 12.5).abs() < 0.01);

        let clamped = AppSettings::parse_from_str(
            "unknown_key=123\nbg_color_pos=-5.0\ndraw_thickness=999.0\nsnap_angle_deg=0\nbad_line\n",
        );
        assert_eq!(clamped.bg_color_pos, 0.0);
        assert_eq!(clamped.draw_thickness, 20.0);
        assert_eq!(clamped.snap_angle_deg, 1);
    }

    #[test]
    fn hotkey_conflict_detection() {
        let s = AppSettings::default();
        assert_eq!(
            s.hotkey_conflict(HotkeySlot::Area, s.hk_fullscreen),
            Some(HotkeySlot::Fullscreen)
        );
        assert_eq!(
            s.hotkey_conflict(HotkeySlot::Fullscreen, s.hk_fullscreen),
            None
        );
        let unique = HotkeySpec::opt_shift(HotkeyCode::KeyZ);
        assert_eq!(s.hotkey_conflict(HotkeySlot::Area, unique), None);
    }
}
