use std::path::{Path, PathBuf};
use std::process::Command;

/// Ordered list of browser binary names to probe via PATH (used with `which`/`where`).
const BROWSER_CANDIDATES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
];

/// Known absolute paths to check on Linux after PATH lookup fails.
#[cfg(target_os = "linux")]
const LINUX_KNOWN_PATHS: &[&str] = &[
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/local/bin/google-chrome",
    "/usr/local/bin/chromium",
    "/snap/bin/chromium",
    "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
    // NixOS system profile
    "/run/current-system/sw/bin/google-chrome-stable",
    "/run/current-system/sw/bin/chromium",
    // Nix home-manager
    "/home/user/.nix-profile/bin/chromium",
];

/// Known absolute paths to check on macOS after PATH lookup fails.
#[cfg(target_os = "macos")]
const MACOS_KNOWN_PATHS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/local/bin/chromium",
    "/opt/homebrew/bin/chromium",         // Apple Silicon Homebrew
    "/usr/local/opt/chromium/bin/chromium", // Intel Homebrew
];

/// Known fixed absolute paths to check on Windows after PATH lookup fails.
/// Note: %LOCALAPPDATA% (user-local install) is checked dynamically in find_browser().
#[cfg(target_os = "windows")]
const WINDOWS_KNOWN_PATHS: &[&str] = &[
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files\Chromium\Application\chrome.exe",
    r"C:\Program Files (x86)\Chromium\Application\chrome.exe",
];

// ---------------------------------------------------------------------------
// Path where we store a previously-downloaded Chromium binary path
// ---------------------------------------------------------------------------

/// Returns the directory used to download Chromium via chromiumoxide_fetcher.
fn browser_download_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".local/share/ra-cl/browser")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join("Library/Application Support/ra-cl/browser")
    }
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".into());
        PathBuf::from(local).join(r"ra-cl\browser")
    }
}

/// Path to the small text file that caches the downloaded browser executable path.
fn cached_browser_path_file() -> PathBuf {
    browser_download_dir().join("browser_path.txt")
}

/// Read the previously-downloaded browser path from disk (if it still exists).
fn find_downloaded_browser() -> Option<String> {
    let cache_file = cached_browser_path_file();
    let path_str = std::fs::read_to_string(&cache_file).ok()?;
    let path_str = path_str.trim().to_string();
    if !path_str.is_empty() && Path::new(&path_str).exists() {
        log::debug!("Found previously downloaded browser: {}", path_str);
        Some(path_str)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Public browser detection
// ---------------------------------------------------------------------------

/// Resolve a binary name through the system PATH using `which` (Unix) or `where` (Windows).
fn which(name: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let cmd = "where";
    #[cfg(not(target_os = "windows"))]
    let cmd = "which";

    Command::new(cmd)
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Try to locate a usable Chrome or Chromium binary.
///
/// Search order:
/// 1. Probe each candidate name via PATH (`which`/`where`)
/// 2. Check OS-specific hardcoded known paths
/// 3. On Windows: check `%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe`
/// 4. Check for a previously downloaded Chromium (via `download_chromium`)
///
/// Returns the full path as a `String` if found, `None` otherwise.
pub fn find_browser() -> Option<String> {
    // 1. PATH lookup
    for name in BROWSER_CANDIDATES {
        if let Some(path) = which(name) {
            log::debug!("Found browser via PATH: {}", path);
            return Some(path);
        }
    }

    // 2. Hardcoded known paths
    #[cfg(target_os = "linux")]
    let known: &[&str] = LINUX_KNOWN_PATHS;
    #[cfg(target_os = "macos")]
    let known: &[&str] = MACOS_KNOWN_PATHS;
    #[cfg(target_os = "windows")]
    let known: &[&str] = WINDOWS_KNOWN_PATHS;

    for path in known {
        if Path::new(path).exists() {
            log::debug!("Found browser at known path: {}", path);
            return Some(path.to_string());
        }
    }

    // 3. Windows: %LOCALAPPDATA% user-local Chrome install (dynamic because the
    //    env var expands to a per-user path that cannot be a compile-time constant)
    #[cfg(target_os = "windows")]
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let candidates = [
            format!(r"{}\Google\Chrome\Application\chrome.exe", local_app_data),
            format!(r"{}\Chromium\Application\chrome.exe", local_app_data),
        ];
        for path in &candidates {
            if Path::new(path).exists() {
                log::debug!("Found browser in LOCALAPPDATA: {}", path);
                return Some(path.clone());
            }
        }
    }

    // 4. Previously downloaded via chromiumoxide_fetcher
    find_downloaded_browser()
}

// ---------------------------------------------------------------------------
// Auto-install via chromiumoxide_fetcher (cross-platform, no sudo)
// ---------------------------------------------------------------------------

/// Download Chromium using the `chromiumoxide` fetcher crate.
/// Stores the executable path in a cache file so future calls to `find_browser()`
/// can locate it without re-downloading.
async fn download_chromium() -> anyhow::Result<String> {
    use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};

    let download_dir = browser_download_dir();
    std::fs::create_dir_all(&download_dir)?;

    println!("Downloading Chromium to {}…", download_dir.display());
    println!("(This may take a few minutes on first run.)");

    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&download_dir)
            .build()?,
    );

    let info = fetcher.fetch().await?;
    let exe_path = info.executable_path.to_string_lossy().to_string();

    // Cache the path for future find_browser() calls
    let cache_file = cached_browser_path_file();
    if let Err(e) = std::fs::write(&cache_file, &exe_path) {
        log::warn!("Could not cache browser path to {}: {}", cache_file.display(), e);
    }

    println!("Chromium downloaded: {}", exe_path);
    Ok(exe_path)
}

// ---------------------------------------------------------------------------
// System package manager fallback (Linux / macOS)
// ---------------------------------------------------------------------------

/// Detect the Linux distro by reading /etc/os-release.
#[cfg(target_os = "linux")]
fn detect_linux_distro() -> String {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    // Prefer ID_LIKE (family) over ID (specific distro)
    let id_like = content
        .lines()
        .find(|l| l.starts_with("ID_LIKE="))
        .map(|l| l.trim_start_matches("ID_LIKE=").trim_matches('"').to_lowercase())
        .unwrap_or_default();
    let id = content
        .lines()
        .find(|l| l.starts_with("ID="))
        .map(|l| l.trim_start_matches("ID=").trim_matches('"').to_lowercase())
        .unwrap_or_default();
    if !id_like.is_empty() { id_like } else { id }
}

/// Returns `(package_manager_binary, install_args, chromium_package_name)`.
#[cfg(target_os = "linux")]
fn linux_install_method() -> Option<(&'static str, Vec<&'static str>, &'static str)> {
    let distro = detect_linux_distro();

    if distro.contains("nixos") || distro.contains("nix") {
        // NixOS: cannot reliably auto-install with nix-env in all configurations
        return None;
    }
    if (distro.contains("arch") || distro.contains("manjaro")) && which("pacman").is_some() {
        return Some(("pacman", vec!["-S", "--noconfirm"], "chromium"));
    }
    if (distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos"))
        && which("dnf").is_some()
    {
        return Some(("dnf", vec!["install", "-y"], "chromium"));
    }
    if distro.contains("opensuse") && which("zypper").is_some() {
        return Some(("zypper", vec!["install", "-y"], "chromium"));
    }
    // Default: apt-get (Debian / Ubuntu / Mint / Pop!_OS / …)
    if which("apt-get").is_some() {
        return Some(("apt-get", vec!["install", "-y"], "chromium"));
    }
    // Fallback probes (no distro match)
    if which("pacman").is_some() {
        return Some(("pacman", vec!["-S", "--noconfirm"], "chromium"));
    }
    if which("dnf").is_some() {
        return Some(("dnf", vec!["install", "-y"], "chromium"));
    }
    if which("zypper").is_some() {
        return Some(("zypper", vec!["install", "-y"], "chromium"));
    }
    None
}

/// Try to install Chromium via the system package manager.
/// Returns the browser binary path on success.
#[cfg(target_os = "linux")]
fn install_via_package_manager() -> anyhow::Result<String> {
    let (pm, args, pkg) = linux_install_method().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not determine a package manager for your distro.\n\
             Install Chromium manually:\n  \
             Arch:    sudo pacman -S chromium\n  \
             Debian:  sudo apt-get install chromium\n  \
             Fedora:  sudo dnf install chromium\n  \
             NixOS:   nix-env -iA nixpkgs.chromium"
        )
    })?;

    println!("Installing Chromium via `sudo {} {} {}`…", pm, args.join(" "), pkg);
    let status = Command::new("sudo")
        .arg(pm)
        .args(&args)
        .arg(pkg)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run installer: {}", e))?;

    if !status.success() {
        anyhow::bail!("Package manager exited with a non-zero status. Try running the command manually with sudo.");
    }

    find_browser().ok_or_else(|| {
        anyhow::anyhow!(
            "Chromium was installed but could not be located. Check that the binary is in PATH."
        )
    })
}

#[cfg(target_os = "macos")]
fn install_via_package_manager() -> anyhow::Result<String> {
    let brew = which("brew").ok_or_else(|| {
        anyhow::anyhow!(
            "Homebrew not found. Install it from https://brew.sh then run:\n  \
             brew install chromium\n  \
             Or install Google Chrome from https://www.google.com/chrome/"
        )
    })?;

    println!("Installing Chromium via `brew install chromium`…");
    let status = Command::new(&brew)
        .args(["install", "chromium"])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run brew: {}", e))?;

    if !status.success() {
        anyhow::bail!("brew install chromium failed. Try running it manually.");
    }

    find_browser().ok_or_else(|| {
        anyhow::anyhow!(
            "Chromium was installed but could not be located. \
             Ensure /opt/homebrew/bin (Apple Silicon) or /usr/local/bin (Intel) is in PATH."
        )
    })
}

#[cfg(target_os = "windows")]
fn install_via_package_manager() -> anyhow::Result<String> {
    anyhow::bail!(
        "Automatic package-manager install is not supported on Windows.\n\
         Chrome/Chromium was downloaded automatically — see above.\n\
         Alternatively install manually from:\n  \
         https://www.google.com/chrome/\n  \
         https://www.chromium.org/getting-involved/download-chromium/"
    )
}

// ---------------------------------------------------------------------------
// Public install / ensure API
// ---------------------------------------------------------------------------

/// Try to install a browser.
///
/// Strategy:
/// 1. Download Chromium via `chromiumoxide_fetcher` (cross-platform, no sudo required).
/// 2. If that fails, fall back to the system package manager (Linux/macOS).
///
/// Returns the browser binary path on success.
pub async fn install_browser() -> anyhow::Result<String> {
    // Primary: chromiumoxide_fetcher download (works everywhere, no sudo)
    match download_chromium().await {
        Ok(path) => return Ok(path),
        Err(e) => {
            log::warn!("chromiumoxide download failed: {}. Trying system package manager…", e);
            println!("Download failed: {}. Trying system package manager…", e);
        }
    }

    // Fallback: system package manager
    install_via_package_manager()
}

/// Ensure a browser is available, installing Chromium automatically if necessary.
///
/// Called during `--operation setup`. Returns the browser binary path.
pub async fn ensure_browser() -> anyhow::Result<String> {
    if let Some(path) = find_browser() {
        println!("Browser found: {}", path);
        return Ok(path);
    }

    println!("No Chrome or Chromium browser found.");
    install_browser().await
}
