mod sha256;
mod signature;

use std::fmt;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
pub use sha256::{parse_sha256sums, verify_file_sha256};
pub use signature::{verify_checksums_if_configured, RELEASE_SIGNING_PUBKEY_HEX};

pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/Byakatris/ShotCraft/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/Byakatris/ShotCraft/releases/latest";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Typed errors emitted by the ShotCraft auto-updater pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdaterError {
    Network(String),
    Parse(String),
    UntrustedUrl(String),
    InvalidFilename(String),
    Verification(String),
    Io(String),
    UnsupportedPlatform(String),
}

impl fmt::Display for UpdaterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg)
            | Self::Parse(msg)
            | Self::UntrustedUrl(msg)
            | Self::InvalidFilename(msg)
            | Self::Verification(msg)
            | Self::Io(msg)
            | Self::UnsupportedPlatform(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for UpdaterError {}

impl From<UpdaterError> for String {
    fn from(err: UpdaterError) -> Self {
        err.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub html_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub asset_size: Option<u64>,
    pub checksums_url: Option<String>,
    pub checksums_sig_url: Option<String>,
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
    MacOsArm64,
    MacOsX64,
    WindowsX64,
    WindowsArm64,
    LinuxDeb,
    LinuxTarGz,
    Unsupported,
}

/// Detect the target release package kind for the running OS, CPU architecture, and install layout.
pub fn current_platform_target() -> PlatformTarget {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        PlatformTarget::MacOsArm64
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        PlatformTarget::MacOsX64
    }
    #[cfg(all(target_os = "macos", not(any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        PlatformTarget::Unsupported
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        PlatformTarget::WindowsArm64
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        PlatformTarget::WindowsX64
    }
    #[cfg(all(target_os = "windows", not(any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        PlatformTarget::Unsupported
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        if std::env::current_exe()
            .ok()
            .is_some_and(|p| p == Path::new("/usr/bin/shotcraft"))
        {
            PlatformTarget::LinuxDeb
        } else {
            PlatformTarget::LinuxTarGz
        }
    }
    #[cfg(all(target_os = "linux", not(target_arch = "x86_64")))]
    {
        PlatformTarget::Unsupported
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        PlatformTarget::Unsupported
    }
}

/// Parse semantic version tags such as `"v0.6.0"`, `"0.7"`, or `"v1.2.3-rc1"`, returning `((maj, min, pat), is_stable)`.
fn parse_semver_full(raw: &str) -> Option<((u32, u32, u32), bool)> {
    let trimmed = raw.trim();
    let cleaned = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    if cleaned.is_empty() {
        return None;
    }
    let without_build = cleaned.split('+').next().unwrap_or(cleaned);
    let (core, is_stable) = match without_build.split_once('-') {
        Some((c, pre)) if !pre.is_empty() => (c, false),
        Some(_) => return None,
        None => (without_build, true),
    };
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
    Some(((major, minor, patch), is_stable))
}

/// Parse semantic version tags into `(major, minor, patch)`.
pub fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    parse_semver_full(raw).map(|(v, _)| v)
}

/// Return `true` iff `latest` parses to a strictly higher semver than `current` (with stable > prerelease).
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver_full(current), parse_semver_full(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

fn has_unsafe_url_chars(url: &str) -> bool {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.is_empty()
        || trimmed.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(
                    c,
                    '\\' | '&' | '|' | '<' | '>' | '^' | '!' | '"' | '\'' | '`' | ';' | '$'
                )
        })
        || lower.contains("..")
        || lower.contains("%2e")
}

/// Allow downloads strictly over HTTPS from this repository's official GitHub Releases downloads path.
/// (`curl --proto =https --proto-redir =https` follows GitHub's signed CDN redirect over HTTPS.)
pub fn is_trusted_download_url(url: &str) -> bool {
    let trimmed = url.trim();
    !has_unsafe_url_chars(trimmed)
        && trimmed.starts_with("https://github.com/Byakatris/ShotCraft/releases/download/")
}

/// Allow browser release pages only on the canonical repository releases URL.
pub fn is_trusted_release_page_url(url: &str) -> bool {
    let trimmed = url.trim();
    !has_unsafe_url_chars(trimmed)
        && trimmed.starts_with("https://github.com/Byakatris/ShotCraft/")
}

fn sanitize_asset_filename(name: &str) -> Result<String, UpdaterError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(UpdaterError::InvalidFilename(format!(
            "Unsafe asset filename: {name}"
        )));
    }
    Ok(trimmed.to_string())
}

fn append_part_suffix(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_os_string();
    os.push(".part");
    PathBuf::from(os)
}

fn private_update_dir() -> Result<PathBuf, UpdaterError> {
    let base = dirs::cache_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("ShotCraft").join("update");
    fs::create_dir_all(&dir)
        .map_err(|e| UpdaterError::Io(format!("Cannot create update cache dir: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

#[derive(Deserialize)]
struct GitHubAsset {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    browser_download_url: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    #[serde(default)]
    tag_name: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Option<Vec<GitHubAsset>>,
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
        PlatformTarget::MacOsArm64 => assets
            .iter()
            .find(|a| {
                let n = a.name.to_ascii_lowercase();
                n.ends_with(".dmg") && !n.contains("x86_64") && !n.contains("intel")
            })
            .or_else(|| find_suffix(".dmg")),
        // Intentionally require an x86_64/Intel/Universal DMG instead of offering an arm64 binary.
        PlatformTarget::MacOsX64 => assets.iter().find(|a| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".dmg")
                && (n.contains("x86_64")
                    || n.contains("x64")
                    || n.contains("intel")
                    || n.contains("universal"))
        }),
        PlatformTarget::WindowsArm64 => find_suffix("-arm64-setup.exe")
            .or_else(|| {
                assets.iter().find(|a| {
                    let n = a.name.to_ascii_lowercase();
                    n.ends_with(".exe") && n.contains("arm64")
                })
            })
            .or_else(|| find_suffix("-setup.exe")),
        PlatformTarget::WindowsX64 => assets
            .iter()
            .find(|a| {
                let n = a.name.to_ascii_lowercase();
                n.ends_with("-setup.exe") && !n.contains("arm64")
            })
            .or_else(|| {
                assets.iter().find(|a| {
                    let n = a.name.to_ascii_lowercase();
                    n.ends_with(".exe") && !n.contains("arm64")
                })
            }),
        PlatformTarget::LinuxDeb => find_suffix(".deb").or_else(|| find_suffix(".tar.gz")),
        PlatformTarget::LinuxTarGz => find_suffix(".tar.gz").or_else(|| find_suffix(".deb")),
        PlatformTarget::Unsupported => None,
    }
}

/// Parse a GitHub `/releases/latest` JSON payload and select the installer asset for `target`.
pub fn parse_github_release_json(
    json: &str,
    target: PlatformTarget,
) -> Result<ReleaseInfo, UpdaterError> {
    let root: GitHubRelease = serde_json::from_str(json)
        .map_err(|e| UpdaterError::Parse(format!("Invalid release JSON: {e}")))?;

    if root.draft {
        return Err(UpdaterError::Parse("Draft release ignored".to_string()));
    }

    let tag_name = root
        .tag_name
        .ok_or_else(|| UpdaterError::Parse("Release JSON missing tag_name".to_string()))?;

    let (maj, min, pat) = parse_semver(&tag_name)
        .ok_or_else(|| UpdaterError::Parse(format!("Invalid release tag semver: {tag_name}")))?;
    let version = format!("{maj}.{min}.{pat}");

    let html_url = root
        .html_url
        .as_deref()
        .filter(|u| is_trusted_release_page_url(u))
        .unwrap_or(RELEASES_PAGE_URL)
        .to_string();

    let assets_json = root
        .assets
        .ok_or_else(|| UpdaterError::Parse("Release JSON missing assets array".to_string()))?;

    let mut parsed_assets = Vec::with_capacity(assets_json.len());
    for item in &assets_json {
        let Some(name) = item.name.as_deref() else {
            continue;
        };
        let Some(download_url) = item.browser_download_url.as_deref() else {
            continue;
        };
        parsed_assets.push(ParsedAsset {
            name,
            download_url,
            size: item.size,
        });
    }

    let checksums_url = parsed_assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .map(|a| a.download_url)
        .filter(|u| is_trusted_download_url(u))
        .map(str::to_string);

    let checksums_sig_url = parsed_assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt.sig"))
        .map(|a| a.download_url)
        .filter(|u| is_trusted_download_url(u))
        .map(str::to_string);

    let chosen = select_asset(&parsed_assets, target).ok_or_else(|| {
        UpdaterError::UnsupportedPlatform(format!("No compatible installer asset found for {target:?}"))
    })?;

    if !is_trusted_download_url(chosen.download_url) {
        return Err(UpdaterError::UntrustedUrl(format!(
            "Untrusted download URL rejected: {}",
            chosen.download_url
        )));
    }

    let asset_name = sanitize_asset_filename(chosen.name)?;

    Ok(ReleaseInfo {
        version,
        tag_name,
        html_url,
        asset_name,
        download_url: chosen.download_url.to_string(),
        asset_size: chosen.size,
        checksums_url,
        checksums_sig_url,
    })
}

fn curl_binary() -> &'static str {
    if cfg!(target_os = "windows") {
        "curl.exe"
    } else {
        "curl"
    }
}

fn http_get_text(url: &str) -> Result<String, UpdaterError> {
    let ua = format!("ShotCraft/{CURRENT_VERSION}");
    let mut cmd = Command::new(curl_binary());
    cmd.args([
        "-fsSL",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
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
        .map_err(|e| UpdaterError::Network(format!("Failed to run {}: {e}", curl_binary())))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(UpdaterError::Network(if stderr.is_empty() {
            format!("HTTP request failed with status {}", out.status)
        } else {
            stderr
        }));
    }

    String::from_utf8(out.stdout)
        .map_err(|e| UpdaterError::Parse(format!("Invalid UTF-8 response: {e}")))
}

fn http_download_file(
    url: &str,
    dest: &Path,
    expected_size: Option<u64>,
    expected_sha256: Option<&str>,
) -> Result<(), UpdaterError> {
    if !is_trusted_download_url(url) {
        return Err(UpdaterError::UntrustedUrl(format!(
            "Untrusted download URL: {url}"
        )));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| UpdaterError::Io(format!("Cannot create temp update dir: {e}")))?;
    }

    let part_path = append_part_suffix(dest);
    let _ = fs::remove_file(&part_path);

    let ua = format!("ShotCraft/{CURRENT_VERSION}");
    let mut cmd = Command::new(curl_binary());
    cmd.args([
        "-fsSL",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
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
        .map_err(|e| UpdaterError::Network(format!("Download failed to start {}: {e}", curl_binary())))?;
    if !out.status.success() {
        let _ = fs::remove_file(&part_path);
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(UpdaterError::Network(if stderr.is_empty() {
            format!("Download failed with status {}", out.status)
        } else {
            stderr
        }));
    }

    let size = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    if size < 1024 {
        let _ = fs::remove_file(&part_path);
        return Err(UpdaterError::Verification(
            "Downloaded update file is unexpectedly small".to_string(),
        ));
    }
    if let Some(expected) = expected_size.filter(|&s| s > 0)
        && size != expected
    {
        let _ = fs::remove_file(&part_path);
        return Err(UpdaterError::Verification(format!(
            "Downloaded file size mismatch (expected {expected} bytes, got {size})"
        )));
    }
    if let Some(expected_hex) = expected_sha256
        && let Err(err) = verify_file_sha256(&part_path, expected_hex)
    {
        let _ = fs::remove_file(&part_path);
        return Err(UpdaterError::Verification(err));
    }

    let _ = fs::remove_file(dest);
    fs::rename(&part_path, dest)
        .map_err(|e| UpdaterError::Io(format!("Failed to finalize download: {e}")))?;
    Ok(())
}

/// Check GitHub Releases (`latest`) and return `Some(ReleaseInfo)` if a newer version exists.
pub fn check_for_updates() -> Result<Option<ReleaseInfo>, UpdaterError> {
    let json = http_get_text(RELEASES_API_URL)?;
    let info = parse_github_release_json(&json, current_platform_target())?;
    if is_newer_version(CURRENT_VERSION, &info.version) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Download the asset in `info`, verify its signature (when configured) and SHA-256 against `SHA256SUMS.txt`, and trigger installation.
pub fn download_and_install(info: &ReleaseInfo) -> Result<InstallAction, UpdaterError> {
    if !is_trusted_download_url(&info.download_url) {
        return Err(UpdaterError::UntrustedUrl(
            "Untrusted release download URL".to_string(),
        ));
    }
    let Some(checksums_url) = info.checksums_url.as_deref() else {
        return Err(UpdaterError::Verification(
            "Release is missing SHA256SUMS.txt — please update manually from GitHub Releases"
                .to_string(),
        ));
    };
    if !is_trusted_download_url(checksums_url) {
        return Err(UpdaterError::UntrustedUrl(
            "Untrusted release checksums URL".to_string(),
        ));
    }

    let clean_name = sanitize_asset_filename(&info.asset_name)?;
    let sums_text = http_get_text(checksums_url)?;

    if let Some(pk) = RELEASE_SIGNING_PUBKEY_HEX
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let sig_text = match info.checksums_sig_url.as_deref() {
            Some(sig_url) => {
                if !is_trusted_download_url(sig_url) {
                    return Err(UpdaterError::UntrustedUrl(
                        "Untrusted release signature URL".to_string(),
                    ));
                }
                Some(http_get_text(sig_url)?)
            }
            None => None,
        };
        verify_checksums_if_configured(sums_text.as_bytes(), sig_text.as_deref(), Some(pk))
            .map_err(UpdaterError::Verification)?;
    }

    let expected_sha = parse_sha256sums(&sums_text, &clean_name).ok_or_else(|| {
        UpdaterError::Verification(format!(
            "SHA256SUMS.txt missing checksum for {clean_name}"
        ))
    })?;

    let update_dir = private_update_dir()?;
    let download_path = update_dir.join(&clean_name);

    http_download_file(
        &info.download_url,
        &download_path,
        info.asset_size,
        Some(expected_sha.as_str()),
    )?;

    #[cfg(target_os = "macos")]
    {
        install_macos_dmg(&download_path, &update_dir, &info.version)
            .map_err(UpdaterError::Io)
    }
    #[cfg(target_os = "windows")]
    {
        install_windows_exe(&download_path, &update_dir, &info.version)
            .map_err(UpdaterError::Io)
    }
    #[cfg(target_os = "linux")]
    {
        install_linux_pkg(&download_path, &update_dir, &info.version)
            .map_err(UpdaterError::Io)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(InstallAction::OpenedManual(download_path.display().to_string()))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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
fn open_dmg_manual(dmg_path: &Path, version: &str) -> Result<InstallAction, String> {
    Command::new("open")
        .arg(dmg_path)
        .spawn()
        .map_err(|e| format!("Failed to open DMG manually: {e}"))?;
    Ok(InstallAction::OpenedManual(version.to_string()))
}

#[cfg(target_os = "macos")]
fn cleanup_macos_mount(mount_point: &Path) {
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(mount_point)
        .status();
    if mount_point.exists() {
        let _ = Command::new("hdiutil")
            .args(["detach", "-force", "-quiet"])
            .arg(mount_point)
            .status();
    }
    let _ = fs::remove_dir_all(mount_point);
}

#[cfg(target_os = "macos")]
fn install_macos_dmg(
    dmg_path: &Path,
    update_dir: &Path,
    version: &str,
) -> Result<InstallAction, String> {
    let Some(target_app) = running_app_bundle() else {
        return open_dmg_manual(dmg_path, version);
    };
    let Some(app_parent) = target_app.parent() else {
        return open_dmg_manual(dmg_path, version);
    };
    if !is_dir_writable(app_parent) {
        return open_dmg_manual(dmg_path, version);
    }

    let mount_point = update_dir.join(format!("mnt_{}", std::process::id()));
    let _ = fs::remove_dir_all(&mount_point);
    fs::create_dir_all(&mount_point).map_err(|e| e.to_string())?;

    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet", "-mountpoint"])
        .arg(&mount_point)
        .arg(dmg_path)
        .output()
        .map_err(|e| {
            let _ = fs::remove_dir_all(&mount_point);
            format!("hdiutil failed: {e}")
        })?;
    if !attach.status.success() {
        let _ = fs::remove_dir_all(&mount_point);
        return open_dmg_manual(dmg_path, version);
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
        cleanup_macos_mount(&mount_point);
        return open_dmg_manual(dmg_path, version);
    };

    let script = r#"
pid="$1"
src_app="$2"
dst_app="$3"
mnt="$4"
dmg="$5"
exited=0
for _ in $(seq 1 40); do
  if ! kill -0 "$pid" 2>/dev/null; then
    exited=1
    break
  fi
  sleep 0.5
done
if [ "$exited" -eq 1 ]; then
  bak="${dst_app}.bak"
  rm -rf "$bak"
  if mv "$dst_app" "$bak"; then
    if ditto "$src_app" "$dst_app"; then
      rm -rf "$bak"
      # curl downloads do not attach com.apple.quarantine; preserve Gatekeeper / signature attributes intact.
      open "$dst_app"
    else
      mv "$bak" "$dst_app" 2>/dev/null || true
      open "$dst_app" 2>/dev/null || true
    fi
  fi
fi
hdiutil detach -quiet "$mnt" 2>/dev/null || hdiutil detach -force -quiet "$mnt" 2>/dev/null || true
rmdir "$mnt" 2>/dev/null || true
rm -f "$dmg" 2>/dev/null || true
"#;

    if let Err(e) = Command::new("/bin/sh")
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
    {
        cleanup_macos_mount(&mount_point);
        return Err(format!("Failed to launch macOS update helper: {e}"));
    }

    Ok(InstallAction::ExitToComplete(version.to_string()))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn escape_cmd_percent(s: &str) -> String {
    s.replace('%', "%%")
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn validate_windows_cmd_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.chars().any(|c| matches!(c, '\r' | '\n' | '"' | '\0')) {
        return Err("Unsafe characters in Windows installer path".to_string());
    }
    Ok(())
}

/// Build the UTF-8 (`chcp 65001`) Windows update `.cmd` script with disabled delayed expansion
/// so non-ASCII user profile paths and special shell characters stay literal inside quotes.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn build_windows_update_cmd(
    pid: u32,
    installer_path: &str,
    fallback_exe: &str,
    current_exe: &str,
) -> Result<String, String> {
    validate_windows_cmd_path(installer_path)?;
    validate_windows_cmd_path(fallback_exe)?;
    validate_windows_cmd_path(current_exe)?;

    let inst_esc = escape_cmd_percent(installer_path);
    let fallback_esc = escape_cmd_percent(fallback_exe);
    let cur_esc = escape_cmd_percent(current_exe);

    Ok(format!(
        "@echo off\r\n\
         chcp 65001 >nul\r\n\
         setlocal DisableDelayedExpansion\r\n\
         set /a _tries=0\r\n\
         :wait_exit\r\n\
         tasklist /FI \"PID eq {pid}\" 2>nul | find \"{pid}\" >nul\r\n\
         if not errorlevel 1 (\r\n\
           set /a _tries+=1\r\n\
           if %_tries% LSS 40 (\r\n\
             ping -n 2 127.0.0.1 >nul\r\n\
             goto wait_exit\r\n\
           )\r\n\
         )\r\n\
         \"{inst_esc}\" /S\r\n\
         if exist \"{fallback_esc}\" (\r\n\
           start \"\" \"{fallback_esc}\"\r\n\
         ) else if exist \"{cur_esc}\" (\r\n\
           start \"\" \"{cur_esc}\"\r\n\
         )\r\n\
         del \"{inst_esc}\" >nul 2>&1\r\n\
         del \"%~f0\" >nul 2>&1\r\n"
    ))
}

#[cfg(target_os = "windows")]
fn install_windows_exe(
    installer_path: &Path,
    update_dir: &Path,
    version: &str,
) -> Result<InstallAction, String> {
    let exe_fallback = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ShotCraft")
        .join("ShotCraft.exe");
    let current_exe = std::env::current_exe().unwrap_or_else(|_| exe_fallback.clone());
    let pid = std::process::id();

    let cmd_path = update_dir.join(format!("apply_update_{pid}.cmd"));
    let script = build_windows_update_cmd(
        pid,
        &installer_path.display().to_string(),
        &exe_fallback.display().to_string(),
        &current_exe.display().to_string(),
    )?;
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

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn resolve_linux_tar_target(
    current_exe: Option<&Path>,
    writable_check: impl Fn(&Path) -> bool,
    home_dir: Option<&Path>,
) -> PathBuf {
    if let Some(exe) = current_exe
        && exe != Path::new("/usr/bin/shotcraft")
        && exe.parent().is_some_and(writable_check)
    {
        return exe.to_path_buf();
    }
    home_dir
        .unwrap_or_else(|| Path::new("."))
        .join(".local")
        .join("bin")
        .join("shotcraft")
}

#[cfg(target_os = "linux")]
fn open_pkg_manual_linux(pkg_path: &Path, version: &str) -> Result<InstallAction, String> {
    Command::new("xdg-open")
        .arg(pkg_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to open update package via xdg-open: {e}"))?;
    Ok(InstallAction::OpenedManual(version.to_string()))
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
            .map_err(|e| {
                let _ = fs::remove_dir_all(&extract_dir);
                format!("Failed to extract tarball: {e}")
            })?;
        if !untar.status.success() {
            let _ = fs::remove_dir_all(&extract_dir);
            return open_pkg_manual_linux(pkg_path, version);
        }

        let install_sh = fs::read_dir(&extract_dir).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|e| e.path().join("install.sh"))
                .find(|p| p.is_file())
        });

        let Some(install_sh) = install_sh else {
            let _ = fs::remove_dir_all(&extract_dir);
            return open_pkg_manual_linux(pkg_path, version);
        };

        let cur_exe = std::env::current_exe().ok();
        let home_dir = dirs::home_dir();
        let target_bin =
            resolve_linux_tar_target(cur_exe.as_deref(), is_dir_writable, home_dir.as_deref());

        let script = r#"
pid="$1"
inst="$2"
ext_dir="$3"
pkg="$4"
target_bin="$5"
exited=0
for _ in $(seq 1 40); do
  if ! kill -0 "$pid" 2>/dev/null; then
    exited=1
    break
  fi
  sleep 0.5
done
if [ "$exited" -eq 1 ]; then
  if command -v bash >/dev/null 2>&1; then
    SHOTCRAFT_INSTALL_BIN="$target_bin" bash "$inst" >/dev/null 2>&1
  else
    SHOTCRAFT_INSTALL_BIN="$target_bin" sh "$inst" >/dev/null 2>&1
  fi
  status=$?
  if [ "$status" -eq 0 ] && [ -x "$target_bin" ]; then
    nohup "$target_bin" >/dev/null 2>&1 &
  fi
fi
rm -rf "$ext_dir" "$pkg" 2>/dev/null || true
"#;

        if let Err(e) = Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .arg("shotcraft-updater")
            .arg(std::process::id().to_string())
            .arg(&install_sh)
            .arg(&extract_dir)
            .arg(pkg_path)
            .arg(&target_bin)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            let _ = fs::remove_dir_all(&extract_dir);
            return Err(format!("Failed to spawn Linux updater: {e}"));
        }

        return Ok(InstallAction::ExitToComplete(version.to_string()));
    }

    open_pkg_manual_linux(pkg_path, version)
}

/// Open a release URL (validated to the repository origin) in the user's default browser.
pub fn open_url_in_browser(url: &str) {
    let target = if is_trusted_release_page_url(url) {
        url.trim()
    } else {
        RELEASES_PAGE_URL
    };
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(target).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", target])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = Command::new("xdg-open").arg(target).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RELEASE_JSON: &str = r#"{
  "tag_name": "v0.7.1",
  "draft": false,
  "html_url": "https://github.com/Byakatris/ShotCraft/releases/tag/v0.7.1",
  "body": "Release notes \uD83D\uDE80 with \"quotes\", braces {}, and nested URLs.",
  "assets": [
    {
      "name": "ShotCraft-v0.7.1.dmg",
      "size": 3012450,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1.dmg"
    },
    {
      "name": "ShotCraft-v0.7.1-Setup.exe",
      "size": 1350120,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1-Setup.exe"
    },
    {
      "name": "ShotCraft-v0.7.1-arm64-Setup.exe",
      "size": 1310400,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1-arm64-Setup.exe"
    },
    {
      "name": "ShotCraft-v0.7.1-linux-amd64.deb",
      "size": 2410000,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1-linux-amd64.deb"
    },
    {
      "name": "ShotCraft-v0.7.1-linux-x86_64.tar.gz",
      "size": 2405000,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1-linux-x86_64.tar.gz"
    },
    {
      "name": "SHA256SUMS.txt",
      "size": 512,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/SHA256SUMS.txt"
    },
    {
      "name": "SHA256SUMS.txt.sig",
      "size": 129,
      "browser_download_url": "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/SHA256SUMS.txt.sig"
    }
  ]
}"#;

    #[test]
    fn semver_parsing_and_ordering() {
        assert_eq!(parse_semver("v0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_semver("0.7"), Some((0, 7, 0)));
        assert_eq!(parse_semver("V1.2.3-rc1+build9"), Some((1, 2, 3)));
        assert_eq!(parse_semver("vv1.0.0"), None);
        assert_eq!(parse_semver("1.2.3.4"), None);
        assert_eq!(parse_semver("1.2."), None);
        assert_eq!(parse_semver("1.2.3-"), None);
        assert_eq!(parse_semver("invalid"), None);
        assert_eq!(parse_semver(""), None);

        assert!(is_newer_version("0.6.0", "v0.6.1"));
        assert!(is_newer_version("0.6.0", "v0.10.0"));
        assert!(is_newer_version("0.7.0-rc1", "v0.7.0"));
        assert!(!is_newer_version("0.7.0", "v0.7.0-rc1"));
        assert!(!is_newer_version("0.6.0", "v0.6.0"));
        assert!(!is_newer_version("0.6.0", "v0.5.9"));
    }

    #[test]
    fn download_and_release_url_allowlists() {
        assert!(is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.0/ShotCraft-v0.7.dmg"
        ));
        assert!(!is_trusted_download_url(
            "https://objects.githubusercontent.com/github-production-release-asset/123"
        ));
        assert!(!is_trusted_download_url(
            "https://release-assets.githubusercontent.com/asset.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/archive/v0.7.0.zip"
        ));
        assert!(!is_trusted_download_url(
            "http://github.com/Byakatris/ShotCraft/releases/download/v0.7.0/ShotCraft.dmg"
        ));
        assert!(!is_trusted_download_url("https://evil.example.com/ShotCraft.exe"));
        assert!(!is_trusted_download_url(
            "https://github.com/OtherUser/ShotCraft/releases/download/v0.7.0/ShotCraft.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/releases/download/../../evil/asset.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/releases/download/%2e%2e/evil.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.0/a.exe&calc"
        ));

        assert!(is_trusted_release_page_url(
            "https://github.com/Byakatris/ShotCraft/releases/tag/v0.7.0"
        ));
        assert!(!is_trusted_release_page_url(
            "https://github.com/Byakatris/ShotCraft/releases/tag/v0.7.0|calc"
        ));
        assert!(!is_trusted_release_page_url(
            "https://objects.githubusercontent.com/some-blob"
        ));
    }

    #[test]
    fn windows_update_cmd_utf8_escaping_and_injection_guard() {
        let cmd = build_windows_update_cmd(
            4242,
            r"C:\Users\Дмитро & Ко\AppData\Local\ShotCraft\update\100%real!Setup.exe",
            r"C:\Users\Дмитро & Ко\AppData\Local\ShotCraft\ShotCraft.exe",
            r"C:\Tools^Dev\ShotCraft.exe",
        )
        .unwrap();
        assert!(cmd.contains("chcp 65001 >nul\r\n"));
        assert!(cmd.contains("setlocal DisableDelayedExpansion\r\n"));
        assert!(cmd.contains("100%%real!Setup.exe"));
        assert!(cmd.contains("PID eq 4242"));

        assert!(build_windows_update_cmd(1, "bad\"path.exe", "b", "c").is_err());
        assert!(build_windows_update_cmd(1, "a", "bad\npath.exe", "c").is_err());
    }

    #[test]
    fn linux_tar_target_prefers_writable_running_binary() {
        let cur = Path::new("/opt/shotcraft/ShotCraft");
        let home = Path::new("/home/dev");
        assert_eq!(
            resolve_linux_tar_target(Some(cur), |_| true, Some(home)),
            PathBuf::from("/opt/shotcraft/ShotCraft")
        );
        assert_eq!(
            resolve_linux_tar_target(Some(cur), |_| false, Some(home)),
            PathBuf::from("/home/dev/.local/bin/shotcraft")
        );
        assert_eq!(
            resolve_linux_tar_target(Some(Path::new("/usr/bin/shotcraft")), |_| true, Some(home)),
            PathBuf::from("/home/dev/.local/bin/shotcraft")
        );
    }

    #[test]
    fn sanitize_filename_and_part_suffix() {
        assert_eq!(
            sanitize_asset_filename("ShotCraft-v0.6.1-linux-x86_64.tar.gz").unwrap(),
            "ShotCraft-v0.6.1-linux-x86_64.tar.gz"
        );
        for bad in ["", ".", "..", "../evil.dmg", "dir/a.dmg", "a\\b.exe", "bad name.dmg"] {
            assert!(sanitize_asset_filename(bad).is_err(), "expected err for {bad}");
        }
        assert_eq!(
            append_part_suffix(Path::new("/tmp/ShotCraft.tar.gz")),
            PathBuf::from("/tmp/ShotCraft.tar.gz.part")
        );
    }

    #[test]
    fn parse_release_selects_correct_asset_for_all_platforms() {
        let draft = r#"{"tag_name":"v0.8.0","draft":true,"assets":[]}"#;
        assert!(matches!(
            parse_github_release_json(draft, PlatformTarget::MacOsArm64),
            Err(UpdaterError::Parse(_))
        ));

        let mac = parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::MacOsArm64).unwrap();
        assert_eq!(mac.version, "0.7.1");
        assert_eq!(mac.asset_name, "ShotCraft-v0.7.1.dmg");
        assert_eq!(mac.asset_size, Some(3012450));
        assert_eq!(
            mac.checksums_url.as_deref(),
            Some("https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/SHA256SUMS.txt")
        );
        assert_eq!(
            mac.checksums_sig_url.as_deref(),
            Some("https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/SHA256SUMS.txt.sig")
        );

        assert!(matches!(
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::MacOsX64),
            Err(UpdaterError::UnsupportedPlatform(_))
        ));
        assert!(matches!(
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::Unsupported),
            Err(UpdaterError::UnsupportedPlatform(_))
        ));

        let win =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::WindowsX64).unwrap();
        assert_eq!(win.asset_name, "ShotCraft-v0.7.1-Setup.exe");

        let win_arm =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::WindowsArm64).unwrap();
        assert_eq!(win_arm.asset_name, "ShotCraft-v0.7.1-arm64-Setup.exe");

        let deb =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::LinuxDeb).unwrap();
        assert_eq!(deb.asset_name, "ShotCraft-v0.7.1-linux-amd64.deb");

        let tar =
            parse_github_release_json(SAMPLE_RELEASE_JSON, PlatformTarget::LinuxTarGz).unwrap();
        assert_eq!(tar.asset_name, "ShotCraft-v0.7.1-linux-x86_64.tar.gz");
    }

    #[test]
    fn download_and_install_requires_checksums_manifest() {
        let info = ReleaseInfo {
            version: "0.7.1".into(),
            tag_name: "v0.7.1".into(),
            html_url: RELEASES_PAGE_URL.into(),
            asset_name: "ShotCraft-v0.7.1.dmg".into(),
            download_url: "https://github.com/Byakatris/ShotCraft/releases/download/v0.7.1/ShotCraft-v0.7.1.dmg".into(),
            asset_size: Some(3012450),
            checksums_url: None,
            checksums_sig_url: None,
        };
        let err = download_and_install(&info).unwrap_err();
        assert!(matches!(err, UpdaterError::Verification(_)));
        assert!(err.to_string().contains("SHA256SUMS.txt"));
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
        let err = parse_github_release_json(tampered, PlatformTarget::MacOsArm64).unwrap_err();
        assert!(matches!(err, UpdaterError::UntrustedUrl(_)));
    }
}
