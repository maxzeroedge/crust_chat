use std::path::Path;
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
    // NixOS system profile paths
    "/run/current-system/sw/bin/google-chrome-stable",
    "/run/current-system/sw/bin/chromium",
    // Home manager Nix paths
    "/home/user/.nix-profile/bin/chromium",
];

/// Known absolute paths to check on macOS after PATH lookup fails.
#[cfg(target_os = "macos")]
const MACOS_KNOWN_PATHS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/local/bin/chromium",
    "/opt/homebrew/bin/chromium",
];

/// Known absolute paths to check on Windows after PATH lookup fails.
#[cfg(target_os = "windows")]
const WINDOWS_KNOWN_PATHS: &[&str] = &[
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files\Chromium\Application\chrome.exe",
    r"C:\Program Files (x86)\Chromium\Application\chrome.exe",
];

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
///
/// Returns the path as a `String` if found, `None` otherwise.
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
    let known = LINUX_KNOWN_PATHS;
    #[cfg(target_os = "macos")]
    let known = MACOS_KNOWN_PATHS;
    #[cfg(target_os = "windows")]
    let known = WINDOWS_KNOWN_PATHS;

    for path in known {
        if Path::new(path).exists() {
            log::debug!("Found browser at known path: {}", path);
            return Some(path.to_string());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Auto-install (Chromium only — open-source, no EULA complications)
// ---------------------------------------------------------------------------

/// Detect the Linux distro by reading /etc/os-release.
#[cfg(target_os = "linux")]
fn detect_linux_distro() -> String {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    // Prefer ID_LIKE for family detection, fall back to ID
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

/// Returns (package_manager, install_args, chromium_package_name).
#[cfg(target_os = "linux")]
fn linux_install_method() -> Option<(&'static str, Vec<&'static str>, &'static str)> {
    let distro = detect_linux_distro();

    // Check package manager presence in order of specificity
    if distro.contains("arch") || distro.contains("manjaro") {
        // pacman
        if which("pacman").is_some() {
            return Some(("pacman", vec!["-S", "--noconfirm"], "chromium"));
        }
    }
    if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
        if which("dnf").is_some() {
            return Some(("dnf", vec!["install", "-y"], "chromium"));
        }
        if which("yum").is_some() {
            return Some(("yum", vec!["install", "-y"], "chromium"));
        }
    }
    if distro.contains("opensuse") || distro.contains("suse") {
        if which("zypper").is_some() {
            return Some(("zypper", vec!["install", "-y"], "chromium"));
        }
    }
    if distro.contains("nixos") || distro.contains("nix") {
        // nix-env or nix profile — too complex and stateful; skip auto-install
        return None;
    }
    // Default: try apt-get (Debian/Ubuntu/Mint/Pop!_OS/…)
    if which("apt-get").is_some() {
        return Some(("apt-get", vec!["install", "-y"], "chromium"));
    }
    // Last resort: pacman / dnf / zypper without distro match
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

/// Try to install Chromium using the system package manager.
///
/// On Linux: uses apt-get / pacman / dnf / zypper depending on distro.
/// On macOS: uses Homebrew (`brew install chromium`).
/// On Windows: auto-install is not supported; user must install manually.
///
/// Returns the path to the installed binary on success.
pub fn install_browser() -> anyhow::Result<String> {
    #[cfg(target_os = "linux")]
    {
        let method = linux_install_method().ok_or_else(|| {
            anyhow::anyhow!(
                "Could not determine package manager for your distro. \
                 Please install Chromium manually:\n  \
                 - Arch:    sudo pacman -S chromium\n  \
                 - Debian:  sudo apt-get install chromium\n  \
                 - Fedora:  sudo dnf install chromium\n  \
                 - NixOS:   nix-env -iA nixpkgs.chromium  (or add to configuration.nix)"
            )
        })?;

        let (pm, args, pkg) = method;
        println!("Installing Chromium via `sudo {} {} {}`…", pm, args.join(" "), pkg);

        let status = Command::new("sudo")
            .arg(pm)
            .args(&args)
            .arg(pkg)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run installer: {}", e))?;

        if !status.success() {
            anyhow::bail!(
                "Package manager exited with non-zero status. \
                 Try running the install command manually with sudo."
            );
        }

        // Re-probe after install
        find_browser().ok_or_else(|| {
            anyhow::anyhow!(
                "Chromium was installed but could not be located. \
                 Try running again or check your PATH."
            )
        })
    }

    #[cfg(target_os = "macos")]
    {
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
                 Check that /opt/homebrew/bin (Apple Silicon) or /usr/local/bin (Intel) is in your PATH."
            )
        })
    }

    #[cfg(target_os = "windows")]
    {
        anyhow::bail!(
            "Automatic browser installation is not supported on Windows.\n\
             Please install Google Chrome from https://www.google.com/chrome/\n\
             or Chromium from https://www.chromium.org/getting-involved/download-chromium/"
        )
    }
}

/// Ensure a browser is available, installing Chromium if necessary.
///
/// Called at startup (setup mode). Returns the browser binary path.
pub fn ensure_browser() -> anyhow::Result<String> {
    if let Some(path) = find_browser() {
        println!("Browser found: {}", path);
        return Ok(path);
    }

    println!("No Chrome/Chromium browser found.");
    install_browser()
}
