use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/Byakatris/ShotCraft/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/Byakatris/ShotCraft/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const DETACHED_PROCESS: u32 = 0x0000_0008;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub html_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub asset_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading(String),
    Installed(String),
    OpenedManual(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallAction {
    ExitToComplete(String),
    OpenedManual(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformTarget {
    MacOs,
    Windows,
    LinuxDeb,
    LinuxTarGz,
}

/// Detect the target release package kind for the running OS / install layout.
pub fn current_platform_target() -> PlatformTarget {
    #[cfg(target_os = "macos")]
    {
        PlatformTarget::MacOs
    }
    #[cfg(target_os = "windows")]
    {
        PlatformTarget::Windows
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::current_exe()
            .ok()
            .is_some_and(|p| p.starts_with("/usr/"))
        {
            PlatformTarget::LinuxDeb
        } else {
            PlatformTarget::LinuxTarGz
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PlatformTarget::LinuxTarGz
    }
}

/// Parse semantic version tags such as `"v0.6.0"`, `"0.7"`, or `"v1.2.3-rc1"`.
pub fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let cleaned = raw.trim().trim_start_matches(['v', 'V']);
    if cleaned.is_empty() {
        return None;
    }
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = match parts.next() {
        Some(p) if !p.is_empty() => p.parse::<u32>().ok()?,
        Some(_) => return None,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Return `true` iff `latest` parses to a strictly higher semver than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

/// Allow downloads only over HTTPS from trusted GitHub release origins for this repository.
pub fn is_trusted_download_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return false;
    }
    trimmed.starts_with("https://github.com/Byakatris/ShotCraft/")
        || trimmed.starts_with("https://objects.githubusercontent.com/")
        || trimmed.starts_with("https://release-assets.githubusercontent.com/")
}

fn sanitize_asset_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(format!("Unsafe asset filename: {name}"));
    }
    Ok(trimmed.to_string())
}

// ── Minimal zero-dependency JSON parser for GitHub Releases API responses ──

#[derive(Debug)]
enum JsonVal {
    Null,
    Bool,
    Num(u64),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(Vec<(String, JsonVal)>),
}

impl JsonVal {
    fn field(&self, key: &str) -> Option<&JsonVal> {
        let JsonVal::Obj(entries) = self else {
            return None;
        };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            JsonVal::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&[JsonVal]> {
        match self {
            JsonVal::Arr(v) => Some(v.as_slice()),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            src: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(b) if b == byte => {
                self.pos += 1;
                Ok(())
            }
            other => Err(format!(
                "JSON parse error at {}: expected '{}', found {:?}",
                self.pos, byte as char, other.map(|c| c as char)
            )),
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonVal, String> {
        if depth > 64 {
            return Err("JSON nesting depth exceeded".to_string());
        }
        self.skip_ws();
        let b = self
            .peek()
            .ok_or_else(|| "Unexpected end of JSON".to_string())?;
        match b {
            b'"' => Ok(JsonVal::Str(self.parse_string()?)),
            b'{' => self.parse_object(depth + 1),
            b'[' => self.parse_array(depth + 1),
            b't' => self.consume_literal(b"true", JsonVal::Bool),
            b'f' => self.consume_literal(b"false", JsonVal::Bool),
            b'n' => self.consume_literal(b"null", JsonVal::Null),
            b'-' | b'0'..=b'9' => self.parse_number(),
            other => Err(format!("Unexpected JSON byte '{}' at {}", other as char, self.pos)),
        }
    }

    fn consume_literal(&mut self, lit: &[u8], val: JsonVal) -> Result<JsonVal, String> {
        if self.src.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(val)
        } else {
            Err(format!("Invalid JSON literal at {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).map_err(|e| e.to_string())?;
        let n = s
            .parse::<u64>()
            .or_else(|_| s.parse::<f64>().map(|f| f.max(0.0) as u64))
            .unwrap_or(0);
        Ok(JsonVal::Num(n))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        while self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = *self
                        .src
                        .get(self.pos)
                        .ok_or_else(|| "Unterminated JSON escape".to_string())?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .src
                                .get(self.pos..self.pos + 4)
                                .ok_or_else(|| "Truncated JSON unicode escape".to_string())?;
                            self.pos += 4;
                            let code = u16::from_str_radix(
                                std::str::from_utf8(hex).map_err(|e| e.to_string())?,
                                16,
                            )
                            .map_err(|e| e.to_string())?;
                            if let Some(ch) = char::from_u32(u32::from(code)) {
                                out.push(ch);
                            }
                        }
                        other => out.push(other as char),
                    }
                }
                _ if b < 0x80 => out.push(b as char),
                first => {
                    let start = self.pos - 1;
                    let width = if first & 0b1110_0000 == 0b1100_0000 {
                        2
                    } else if first & 0b1111_0000 == 0b1110_0000 {
                        3
                    } else {
                        4
                    };
                    let end = (start + width).min(self.src.len());
                    let s = std::str::from_utf8(&self.src[start..end]).map_err(|e| e.to_string())?;
                    out.push_str(s);
                    self.pos = end;
                }
            }
        }
        Err("Unterminated JSON string".to_string())
    }

    fn parse_array(&mut self, depth: usize) -> Result<JsonVal, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse_value(depth)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonVal::Arr(items));
                }
                _ => return Err(format!("Expected ',' or ']' at {}", self.pos)),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonVal, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Obj(entries));
        }
        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            let val = self.parse_value(depth)?;
            entries.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonVal::Obj(entries));
                }
                _ => return Err(format!("Expected ',' or '}}' at {}", self.pos)),
            }
        }
    }
}

struct ParsedAsset<'a> {
    name: &'a str,
    download_url: &'a str,
    size: Option<u64>,
}

fn select_asset<'a>(assets: &'a [ParsedAsset<'a>], target: PlatformTarget) -> Option<&'a ParsedAsset<'a>> {
    let find_suffix = |suffix: &str| -> Option<&'a ParsedAsset<'a>> {
        assets
            .iter()
            .find(|a| a.name.to_ascii_lowercase().ends_with(suffix))
    };

    match target {
        PlatformTarget::MacOs => find_suffix(".dmg"),
        PlatformTarget::Windows => find_suffix("-setup.exe").or_else(|| find_suffix(".exe")),
        PlatformTarget::LinuxDeb => find_suffix(".deb").or_else(|| find_suffix(".tar.gz")),
        PlatformTarget::LinuxTarGz => find_suffix(".tar.gz").or_else(|| find_suffix(".deb")),
    }
}

/// Parse a GitHub `/releases/latest` JSON payload and select the installer asset for `target`.
pub fn parse_github_release_json(json: &str, target: PlatformTarget) -> Result<ReleaseInfo, String> {
    let root = JsonParser::new(json).parse_value(0)?;

    let tag_name = root
        .field("tag_name")
        .and_then(JsonVal::as_str)
        .ok_or_else(|| "Release JSON missing tag_name".to_string())?
        .to_string();

    let (maj, min, pat) = parse_semver(&tag_name)
        .ok_or_else(|| format!("Invalid release tag semver: {tag_name}"))?;
    let version = format!("{maj}.{min}.{pat}");

    let html_url = root
        .field("html_url")
        .and_then(JsonVal::as_str)
        .filter(|u| is_trusted_download_url(u))
        .unwrap_or(RELEASES_PAGE_URL)
        .to_string();

    let assets_json = root
        .field("assets")
        .and_then(JsonVal::as_array)
        .ok_or_else(|| "Release JSON missing assets array".to_string())?;

    let mut parsed_assets = Vec::with_capacity(assets_json.len());
    for item in assets_json {
        let Some(name) = item.field("name").and_then(JsonVal::as_str) else {
            continue;
        };
        let Some(download_url) = item.field("browser_download_url").and_then(JsonVal::as_str) else {
            continue;
        };
        parsed_assets.push(ParsedAsset {
            name,
            download_url,
            size: item.field("size").and_then(JsonVal::as_u64),
        });
    }

    let chosen = select_asset(&parsed_assets, target)
        .ok_or_else(|| format!("No compatible installer asset found for {target:?}"))?;

    if !is_trusted_download_url(chosen.download_url) {
        return Err(format!(
            "Untrusted download URL rejected: {}",
            chosen.download_url
        ));
    }

    let asset_name = sanitize_asset_filename(chosen.name)?;

    Ok(ReleaseInfo {
        version,
        tag_name,
        html_url,
        asset_name,
        download_url: chosen.download_url.to_string(),
        asset_size: chosen.size,
    })
}

fn curl_binary() -> &'static str {
    if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    }
}

fn http_get_text(url: &str) -> Result<String, String> {
    let ua = format!("ShotCraft/{CURRENT_VERSION}");
    let mut cmd = Command::new(curl_binary());
    cmd.args([
        "-fsSL",
        "--connect-timeout",
        "10",
        "--max-time",
        "25",
        "-H",
        "Accept: application/vnd.github+json",
        "-A",
        &ua,
        url,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {e}", curl_binary()))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("HTTP request failed with status {}", out.status)
        } else {
            stderr
        });
    }

    String::from_utf8(out.stdout).map_err(|e| format!("Invalid UTF-8 response: {e}"))
}

fn http_download_file(url: &str, dest: &Path) -> Result<(), String> {
    if !is_trusted_download_url(url) {
        return Err(format!("Untrusted download URL: {url}"));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create temp update dir: {e}"))?;
    }

    let part_path = dest.with_extension("part");
    let _ = fs::remove_file(&part_path);

    let ua = format!("ShotCraft/{CURRENT_VERSION}");
    let mut cmd = Command::new(curl_binary());
    cmd.args([
        "-fsSL",
        "--connect-timeout",
        "12",
        "--max-time",
        "180",
        "-A",
        &ua,
        "-o",
    ])
    .arg(&part_path)
    .arg(url);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd
        .output()
        .map_err(|e| format!("Download failed to start {}: {e}", curl_binary()))?;
    if !out.status.success() {
        let _ = fs::remove_file(&part_path);
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Download failed with status {}", out.status)
        } else {
            stderr
        });
    }

    let size = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    if size < 1024 {
        let _ = fs::remove_file(&part_path);
        return Err("Downloaded update file is unexpectedly small".to_string());
    }

    let _ = fs::remove_file(dest);
    fs::rename(&part_path, dest).map_err(|e| format!("Failed to finalize download: {e}"))?;
    Ok(())
}

/// Check GitHub Releases (`latest`) and return `Some(ReleaseInfo)` if a newer version exists.
pub fn check_for_updates() -> Result<Option<ReleaseInfo>, String> {
    let json = http_get_text(RELEASES_API_URL)?;
    let info = parse_github_release_json(&json, current_platform_target())?;
    if is_newer_version(CURRENT_VERSION, &info.version) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Download the asset in `info` to a temporary directory and trigger OS-native installation.
pub fn download_and_install(info: &ReleaseInfo) -> Result<InstallAction, String> {
    if !is_trusted_download_url(&info.download_url) {
        return Err("Untrusted release download URL".to_string());
    }
    let clean_name = sanitize_asset_filename(&info.asset_name)?;
    let update_dir = std::env::temp_dir().join("shotcraft-update");
    let download_path = update_dir.join(&clean_name);

    http_download_file(&info.download_url, &download_path)?;

    #[cfg(target_os = "macos")]
    {
        install_macos_dmg(&download_path, &update_dir, &info.version)
    }
    #[cfg(target_os = "windows")]
    {
        install_windows_exe(&download_path, &update_dir, &info.version)
    }
    #[cfg(target_os = "linux")]
    {
        install_linux_pkg(&download_path, &update_dir, &info.version)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(InstallAction::OpenedManual(download_path.display().to_string()))
    }
}

#[cfg(target_os = "macos")]
fn is_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".shotcraft_upd_probe_{}", std::process::id()));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
fn running_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos_dir = exe.parent()?;
    if macos_dir.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let app_dir = contents_dir.parent()?;
    if app_dir.extension()?.to_str()? != "app" {
        return None;
    }
    Some(app_dir.to_path_buf())
}

#[cfg(target_os = "macos")]
fn install_macos_dmg(
    dmg_path: &Path,
    update_dir: &Path,
    version: &str,
) -> Result<InstallAction, String> {
    let Some(target_app) = running_app_bundle() else {
        let _ = Command::new("open").arg(dmg_path).spawn();
        return Ok(InstallAction::OpenedManual(version.to_string()));
    };
    let Some(app_parent) = target_app.parent() else {
        let _ = Command::new("open").arg(dmg_path).spawn();
        return Ok(InstallAction::OpenedManual(version.to_string()));
    };
    if !is_dir_writable(app_parent) {
        let _ = Command::new("open").arg(dmg_path).spawn();
        return Ok(InstallAction::OpenedManual(version.to_string()));
    }

    let mount_point = update_dir.join(format!("mnt_{}", std::process::id()));
    let _ = fs::remove_dir_all(&mount_point);
    fs::create_dir_all(&mount_point).map_err(|e| e.to_string())?;

    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet", "-mountpoint"])
        .arg(&mount_point)
        .arg(dmg_path)
        .output()
        .map_err(|e| format!("hdiutil failed: {e}"))?;
    if !attach.status.success() {
        let _ = Command::new("open").arg(dmg_path).spawn();
        return Ok(InstallAction::OpenedManual(version.to_string()));
    }

    let preferred = mount_point.join("ShotCraft.app");
    let src_app = if preferred.is_dir() {
        Some(preferred)
    } else {
        fs::read_dir(&mount_point).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|ext| ext == "app") && p.is_dir())
        })
    };

    let Some(src_app) = src_app else {
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&mount_point)
            .status();
        let _ = Command::new("open").arg(dmg_path).spawn();
        return Ok(InstallAction::OpenedManual(version.to_string()));
    };

    let script = r#"
pid="$1"
src_app="$2"
dst_app="$3"
mnt="$4"
dmg="$5"
for _ in $(seq 1 30); do
  if ! kill -0 "$pid" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
bak="${dst_app}.bak"
rm -rf "$bak"
if mv "$dst_app" "$bak"; then
  if ditto "$src_app" "$dst_app"; then
    rm -rf "$bak"
    xattr -cr "$dst_app" 2>/dev/null || true
    open "$dst_app"
  else
    mv "$bak" "$dst_app" 2>/dev/null || true
  fi
fi
hdiutil detach -quiet "$mnt" 2>/dev/null || true
rmdir "$mnt" 2>/dev/null || true
rm -f "$dmg" 2>/dev/null || true
"#;

    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg("shotcraft-updater")
        .arg(std::process::id().to_string())
        .arg(&src_app)
        .arg(&target_app)
        .arg(&mount_point)
        .arg(dmg_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch macOS update helper: {e}"))?;

    Ok(InstallAction::ExitToComplete(version.to_string()))
}

#[cfg(target_os = "windows")]
fn install_windows_exe(
    installer_path: &Path,
    update_dir: &Path,
    version: &str,
) -> Result<InstallAction, String> {
    let exe_fallback = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("%LOCALAPPDATA%"))
        .join("ShotCraft")
        .join("ShotCraft.exe");
    let current_exe = std::env::current_exe().unwrap_or_else(|_| exe_fallback.clone());

    let cmd_path = update_dir.join("apply_update.cmd");
    let script = format!(
        "@echo off\r\n\
         ping -n 3 127.0.0.1 >nul\r\n\
         \"{}\" /S\r\n\
         if exist \"{}\" (\r\n\
           start \"\" \"{}\"\r\n\
         ) else if exist \"{}\" (\r\n\
           start \"\" \"{}\"\r\n\
         )\r\n\
         del \"{}\" >nul 2>&1\r\n\
         del \"%~f0\" >nul 2>&1\r\n",
        installer_path.display(),
        current_exe.display(),
        current_exe.display(),
        exe_fallback.display(),
        exe_fallback.display(),
        installer_path.display(),
    );
    fs::write(&cmd_path, script)
        .map_err(|e| format!("Failed to write Windows update script: {e}"))?;

    Command::new("cmd.exe")
        .args(["/C"])
        .arg(&cmd_path)
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn Windows updater: {e}"))?;

    Ok(InstallAction::ExitToComplete(version.to_string()))
}

#[cfg(target_os = "linux")]
fn install_linux_pkg(
    pkg_path: &Path,
    update_dir: &Path,
    version: &str,
) -> Result<InstallAction, String> {
    let name_lower = pkg_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name_lower.ends_with(".tar.gz") && current_platform_target() == PlatformTarget::LinuxTarGz {
        let extract_dir = update_dir.join(format!("extract_{}", std::process::id()));
        let _ = fs::remove_dir_all(&extract_dir);
        fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;

        let untar = Command::new("tar")
            .arg("-xzf")
            .arg(pkg_path)
            .arg("-C")
            .arg(&extract_dir)
            .output()
            .map_err(|e| format!("Failed to extract tarball: {e}"))?;
        if !untar.status.success() {
            let _ = Command::new("xdg-open").arg(pkg_path).spawn();
            return Ok(InstallAction::OpenedManual(version.to_string()));
        }

        let install_sh = fs::read_dir(&extract_dir).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|e| e.path().join("install.sh"))
                .find(|p| p.is_file())
        });

        let Some(install_sh) = install_sh else {
            let _ = Command::new("xdg-open").arg(pkg_path).spawn();
            return Ok(InstallAction::OpenedManual(version.to_string()));
        };

        let script = r#"
pid="$1"
inst="$2"
ext_dir="$3"
pkg="$4"
for _ in $(seq 1 30); do
  if ! kill -0 "$pid" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
sh "$inst" >/dev/null 2>&1 || true
if [ -x "$HOME/.local/bin/shotcraft" ]; then
  nohup "$HOME/.local/bin/shotcraft" >/dev/null 2>&1 &
fi
rm -rf "$ext_dir" "$pkg" 2>/dev/null || true
"#;

        Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .arg("shotcraft-updater")
            .arg(std::process::id().to_string())
            .arg(&install_sh)
            .arg(&extract_dir)
            .arg(pkg_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Linux updater: {e}"))?;

        return Ok(InstallAction::ExitToComplete(version.to_string()));
    }

    let _ = Command::new("xdg-open")
        .arg(pkg_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    Ok(InstallAction::OpenedManual(version.to_string()))
}

/// Open the GitHub Releases page in the user's default browser.
pub fn open_releases_page() {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(RELEASES_PAGE_URL).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd.exe")
        .args(["/C", "start", "", RELEASES_PAGE_URL])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = Command::new("xdg-open").arg(RELEASES_PAGE_URL).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RELEASE_JSON: &str = r#"{
  "tag_name": "v0.7.1",
  "html_url": "https://github.com/Byakatris/ShotCraft/releases/tag/v0.7.1",
  "body": "Release notes with \"quotes\", braces {}, and nested URLs.",
  "assets": [
    {
      "name": "ShotCraft-v0.7.dmg",
      "size": 3012450,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.dmg"
    },
    {
      "name": "ShotCraft-v0.7-Setup.exe",
      "size": 1350120,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7-Setup.exe"
    },
    {
      "name": "ShotCraft-v0.7-linux-amd64.deb",
      "size": 2410000,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7-linux-amd64.deb"
    },
    {
      "name": "ShotCraft-v0.7-linux-x86_64.tar.gz",
      "size": 2405000,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7-linux-x86_64.tar.gz"
    }
  ]
}"#;

    #[test]
    fn semver_parsing_and_ordering() {
        assert_eq!(parse_semver("v0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_semver("0.7"), Some((0, 7, 0)));
        assert_eq!(parse_semver("V1.2.3-rc1+build9"), Some((1, 2, 3)));
        assert_eq!(parse_semver("invalid"), None);
        assert_eq!(parse_semver(""), None);

        assert!(is_newer_version("0.6.0", "v0.6.1"));
        assert!(is_newer_version("0.6.0", "v0.10.0"));
        assert!(!is_newer_version("0.6.0", "v0.6.0"));
        assert!(!is_newer_version("0.6.0", "v0.5.9"));
    }

    #[test]
    fn download_url_allowlist_blocks_untrusted_origins() {
        assert!(is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.0/ShotCraft-v0.7.dmg"
        ));
        assert!(is_trusted_download_url(
            "https://objects.githubusercontent.com/github-production-release-asset/123"
        ));
        assert!(is_trusted_download_url(
            "https://release-assets.githubusercontent.com/asset.exe"
        ));

        assert!(!is_trusted_download_url(
            "http://github.com/Byakatris/ShotCraft/releases/download/v0.7.0/ShotCraft.dmg"
        ));
        assert!(!is_trusted_download_url("https://evil.example.com/ShotCraft.exe"));
        assert!(!is_trusted_download_url(
            "https://github.com/OtherUser/ShotCraft/releases/download/v0.7.0/ShotCraft.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/\nrm -rf /"
        ));
    }

    #[test]
    fn parse_release_selects_correct_asset_for_all_platforms() {
        let mac = parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::MacOs).unwrap();
        assert_eq!(mac.version, "0.7.1");
        assert_eq!(mac.asset_name, "ShotCraft-v0.7.dmg");
        assert_eq!(mac.asset_size, Some(3012450));

        let win = parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::Windows).unwrap();
        assert_eq!(win.asset_name, "ShotCraft-v0.7-Setup.exe");

        let deb =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::LinuxDeb).unwrap();
        assert_eq!(deb.asset_name, "ShotCraft-v0.7-linux-amd64.deb");

        let tar =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::LinuxTarGz).unwrap();
        assert_eq!(tar.asset_name, "ShotCraft-v0.7-linux-x86_64.tar.gz");
    }

    #[test]
    fn parse_release_rejects_untrusted_asset_url() {
        let tampered = r#"{
  "tag_name": "v0.8.0",
  "html_url": "https://github.com/Byakatris/ShotCraft/releases/tag/v0.8.0",
  "assets": [
    {
      "name": "ShotCraft-v0.8.dmg",
      "browser_download_url": "https://attacker.invalid/ShotCraft-v0.8.dmg"
    }
  ]
}"#;
        let err = parse_github_release_json(tampered, PlatformTarget::MacOs).unwrap_err();
        assert!(err.contains("Untrusted"));
    }
}
