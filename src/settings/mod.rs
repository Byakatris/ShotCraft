pub mod fonts;
pub mod hotkey;
pub mod i18n;

use std::fs;
use std::path::PathBuf;

use crate::annotations::Tool;
use crate::diagnostics;

pub use fonts::TextFont;
pub use hotkey::{HotkeyCode, HotkeySlot, HotkeySpec};
pub use i18n::{Language, Strings};

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

fn app_config_root() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    app_config_root().join("ShotCraft").join("settings.conf")
}

fn parse_clamped_f32(raw: &str, min: f32, max: f32) -> Option<f32> {
    raw.trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(min, max))
}

fn sanitize_path_for_line(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .chars()
        .filter(|&c| c != '\n' && c != '\r')
        .collect()
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
        let content = fs::read_to_string(&path)
            .or_else(|_| fs::read_to_string(app_config_root().join("LightCraft").join("settings.conf")));
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
                    let clean = v.trim_matches(|c| c == '\r' || c == '\n');
                    if !clean.trim().is_empty() {
                        s.save_dir = PathBuf::from(clean);
                    }
                }
                "hk_fullscreen" => {
                    if let Some(hk) = HotkeySpec::deserialize(v).filter(|h| h.has_modifier()) {
                        s.hk_fullscreen = hk;
                    }
                }
                "hk_area" => {
                    if let Some(hk) = HotkeySpec::deserialize(v).filter(|h| h.has_modifier()) {
                        s.hk_area = hk;
                    }
                }
                "hk_window" => {
                    if let Some(hk) = HotkeySpec::deserialize(v).filter(|h| h.has_modifier()) {
                        s.hk_window = hk;
                    }
                }
                "hk_live_draw" => {
                    if let Some(hk) = HotkeySpec::deserialize(v).filter(|h| h.has_modifier()) {
                        s.hk_live_draw = hk;
                    }
                }
                "bg_color_pos" => {
                    if let Some(p) = parse_clamped_f32(v, 0.0, 1.0) {
                        s.bg_color_pos = p;
                    }
                }
                "accent_color_pos" => {
                    if let Some(p) = parse_clamped_f32(v, 0.0, 1.0) {
                        s.accent_color_pos = p;
                    }
                }
                "draw_color_pos" => {
                    if let Some(p) = parse_clamped_f32(v, 0.0, 1.0) {
                        s.draw_color_pos = p;
                    }
                }
                "draw_thickness" => {
                    if let Some(t) = parse_clamped_f32(v, 1.5, 20.0) {
                        s.draw_thickness = t;
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
        s.deduplicate_hotkeys();
        s
    }

    fn deduplicate_hotkeys(&mut self) {
        let def = Self::default();
        let fallback_pool = [
            def.hk_fullscreen,
            def.hk_area,
            def.hk_window,
            def.hk_live_draw,
            HotkeySpec::opt_shift(HotkeyCode::Digit5),
            HotkeySpec::opt_shift(HotkeyCode::Digit6),
            HotkeySpec::opt_shift(HotkeyCode::Digit7),
            HotkeySpec::opt_shift(HotkeyCode::Digit8),
        ];
        if self.hotkey_conflict(HotkeySlot::Area, self.hk_area).is_some()
            && let Some(&cand) = fallback_pool
                .iter()
                .find(|&&c| self.hotkey_conflict(HotkeySlot::Area, c).is_none())
        {
            self.hk_area = cand;
        }
        if self.hotkey_conflict(HotkeySlot::Window, self.hk_window).is_some()
            && let Some(&cand) = fallback_pool
                .iter()
                .find(|&&c| self.hotkey_conflict(HotkeySlot::Window, c).is_none())
        {
            self.hk_window = cand;
        }
        if self
            .hotkey_conflict(HotkeySlot::LiveDraw, self.hk_live_draw)
            .is_some()
            && let Some(&cand) = fallback_pool
                .iter()
                .find(|&&c| self.hotkey_conflict(HotkeySlot::LiveDraw, c).is_none())
        {
            self.hk_live_draw = cand;
        }
    }

    pub fn serialize_to_string(&self) -> String {
        let layout_str = match self.toolbar_layout {
            ToolbarLayout::Horizontal => "horizontal",
            ToolbarLayout::Vertical => "vertical",
        };
        format!(
            "language={}\nsave_dir={}\nhk_fullscreen={}\nhk_area={}\nhk_window={}\nhk_live_draw={}\nbg_color_pos={:.3}\naccent_color_pos={:.3}\ndraw_color_pos={:.3}\ndraw_thickness={:.2}\nsnap_angle_deg={}\ntext_font={}\ntoolbar_layout={}\nlast_tool={}\n",
            self.language.code(),
            sanitize_path_for_line(&self.save_dir),
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
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            diagnostics::log_error("settings.save", &format!("mkdir failed: {e}"));
            return;
        }
        let data = self.serialize_to_string();
        let tmp = path.with_extension(format!("conf.{}.tmp", std::process::id()));
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
            assert!(!s.update_installed.is_empty());
            assert!(!s.hotkey_unsupported_key.is_empty());
            for tpl in [
                s.tool_pen,
                s.tool_marker,
                s.tool_line,
                s.tool_arrow,
                s.tool_ellipse,
                s.tool_rect,
                s.tool_blur,
                s.act_undo,
                s.act_redo,
                s.act_copy,
                s.act_save,
                s.hotkey_need_modifier,
            ] {
                assert!(
                    tpl.contains("{MOD}"),
                    "missing {{MOD}} placeholder in {:?}: {tpl}",
                    lang
                );
                assert!(!Strings::with_mod(tpl).contains("{MOD}"));
            }
        }
        assert_eq!(Language::from_code("zh"), Language::En);
        assert_eq!(Language::from_code("unknown"), Language::En);
    }

    #[test]
    fn hotkey_codes_and_spec_roundtrip() {
        assert_eq!(HotkeyCode::ALL.len(), 36);
        for &code in HotkeyCode::ALL {
            assert_eq!(HotkeyCode::from_label(code.label()), Some(code));
            let _ = code.to_ghk();
            let _ = code.to_muda();
        }
        assert_eq!(HotkeyCode::from_winit(winit::keyboard::KeyCode::KeyQ), Some(HotkeyCode::KeyQ));
        assert_eq!(HotkeyCode::from_winit(winit::keyboard::KeyCode::F1), None);
        assert_eq!(HotkeyCode::from_winit(winit::keyboard::KeyCode::Space), None);

        let spec = HotkeySpec {
            ctrl: true,
            alt: true,
            shift: true,
            super_key: true,
            key: HotkeyCode::KeyS,
        };
        let s = spec.serialize();
        assert_eq!(HotkeySpec::deserialize(&s), Some(spec));
        assert!(spec.has_modifier());
        assert!(spec.display().contains("Ctrl"));
        assert!(spec.display().ends_with(" + S"));
        let _ = spec.to_hotkey();
        let _ = spec.to_accelerator();

        let bare = HotkeySpec {
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
            key: HotkeyCode::Digit1,
        };
        assert!(!bare.has_modifier());
        assert_eq!(bare.display(), "1");

        assert!(HotkeySpec::deserialize("1,0,1,0").is_none());
        assert!(HotkeySpec::deserialize("2,0,1,0,A").is_none());
        assert!(HotkeySpec::deserialize("1,0,1,0,UNKNOWN").is_none());
    }

    #[test]
    fn text_fonts_unique_and_complete() {
        assert_eq!(TextFont::ALL.len(), TextFont::COUNT);
        for (i, tf) in TextFont::ALL.iter().enumerate() {
            assert_eq!(TextFont::from_code(tf.code()), *tf);
            for other in &TextFont::ALL[i + 1..] {
                assert_ne!(tf.code(), other.code());
                assert_ne!(tf.egui_family(), other.egui_family());
            }
        }
        assert_eq!(TextFont::from_code("missing_font"), TextFont::Roboto);
    }

    #[test]
    fn settings_serialize_parse_roundtrip_and_edge_cases() {
        let s = AppSettings {
            language: Language::Uk,
            save_dir: PathBuf::from("/tmp/shotcraft shots/sub=1"),
            bg_color_pos: 0.75,
            accent_color_pos: 0.125,
            draw_color_pos: 0.5,
            draw_thickness: 12.5,
            snap_angle_deg: 45,
            text_font: TextFont::CourierNew,
            toolbar_layout: ToolbarLayout::Vertical,
            last_tool: Tool::Blur,
            ..Default::default()
        };

        let serialized = s.serialize_to_string();
        let parsed = AppSettings::parse_from_str(&serialized);
        assert_eq!(parsed.language, s.language);
        assert_eq!(parsed.save_dir, s.save_dir);
        assert_eq!(parsed.snap_angle_deg, 45);
        assert_eq!(parsed.text_font, TextFont::CourierNew);
        assert_eq!(parsed.toolbar_layout, ToolbarLayout::Vertical);
        assert_eq!(parsed.last_tool, Tool::Blur);
        assert!((parsed.draw_thickness - 12.5).abs() < 0.01);

        // NaN, infinity, bare modifier-less hotkeys, duplicate hotkeys, CRLF
        let edge = AppSettings::parse_from_str(
            "unknown_key=123\r\nbg_color_pos=NaN\r\naccent_color_pos=inf\r\ndraw_thickness=999.0\r\nsnap_angle_deg=0\r\nsave_dir=   \r\nhk_fullscreen=0,0,0,0,A\r\nhk_area=0,1,1,0,1\r\nbad_line\r\n",
        );
        assert_eq!(edge.bg_color_pos, AppSettings::default().bg_color_pos);
        assert_eq!(edge.accent_color_pos, AppSettings::default().accent_color_pos);
        assert_eq!(edge.draw_thickness, 20.0);
        assert_eq!(edge.snap_angle_deg, 1);
        assert_eq!(edge.save_dir, AppSettings::default().save_dir);
        assert!(edge.hk_fullscreen.has_modifier());
        assert_ne!(edge.hk_fullscreen, edge.hk_area);
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
