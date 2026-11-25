use std::fs;
use std::path::PathBuf;

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
    pub const COUNT: usize = 15;

    pub const ALL: [Self; Self::COUNT] = [
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
                let fonts_dir = windows_fonts_dir();
                paths.push(fonts_dir.join("GOST_AU.TTF"));
                paths.push(fonts_dir.join("GOST_A.TTF"));
                paths.push(fonts_dir.join("GOST_Type_AU.ttf"));
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
            let file = match self {
                Self::TimesNewRoman => "times.ttf",
                Self::SanFrancisco => "segoeui.ttf",
                Self::Arial => "arial.ttf",
                Self::ArialBlack => "ariblk.ttf",
                Self::Georgia => "georgia.ttf",
                Self::Verdana => "verdana.ttf",
                Self::Tahoma => "tahoma.ttf",
                Self::TrebuchetMS => "trebuc.ttf",
                Self::CourierNew => "cour.ttf",
                Self::Impact => "impact.ttf",
                Self::ComicSansMS => "comic.ttf",
                Self::SFRounded => "calibri.ttf",
                Self::SFMono => "consola.ttf",
                Self::GostTypeAU | Self::Roboto => "",
            };
            if !file.is_empty() {
                paths.push(windows_fonts_dir().join(file));
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

#[cfg(target_os = "windows")]
fn windows_fonts_dir() -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("Fonts")
}
