//! GitHub release discovery and installation. All network and disk work runs off the UI thread.
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use self_update::{Release, backends::github};

pub const RELEASES_URL: &str = "https://github.com/marinoandrea/bitman/releases";
const VERSION: &str = env!("CARGO_PKG_VERSION");
static INSTALL_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn install_in_progress() -> bool {
    INSTALL_IN_PROGRESS.load(Ordering::SeqCst)
}

struct InstallGuard;

impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALL_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    Current,
    Available(Release),
    Installing,
    Installed,
    Failed(String),
    InstallFailed(String),
}

impl UpdateState {
    pub fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Installing)
    }

    pub fn message(&self) -> String {
        match self {
            Self::Idle => format!("Bitman {VERSION}"),
            Self::Checking => "Checking for updates…".into(),
            Self::Current => "Bitman is up to date.".into(),
            Self::Available(release) => format!("Bitman {} is available.", release.version()),
            Self::Installing => "Installing update… Keep Bitman open until it finishes.".into(),
            Self::Installed => "Update installed. Restart Bitman to use it.".into(),
            Self::Failed(message) | Self::InstallFailed(message) => {
                format!("Update failed: {message}")
            }
        }
    }
}

fn target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        _ => Err("No release package is available for this platform.".into()),
    }
}

fn archive_name(version: &str, target: &str) -> String {
    let extension = if target.ends_with("linux-gnu") {
        "tar.gz"
    } else {
        "zip"
    };
    format!("bitman-{version}-{target}.{extension}")
}

fn builder() -> Result<github::UpdateBuilder, String> {
    let mut builder = github::Update::configure();
    builder
        .repo_owner("marinoandrea")
        .repo_name("bitman")
        .bin_name("bitman")
        .target(target()?)
        .current_version(VERSION)
        .tag_prefix("v")
        .timeout(Duration::from_secs(120))
        .retries(0)
        .show_output(false)
        .no_confirm(true);
    Ok(builder)
}

// Release-please publishes the release before CI uploads its packages. Never offer
// an install until both the exact native archive and its checksum are present.
fn select_release(releases: &[Release], current: &str, target: &str) -> Option<Release> {
    let current = semver::Version::parse(current).ok()?;
    releases
        .iter()
        .filter_map(|release| {
            let version = semver::Version::parse(release.version()).ok()?;
            let archive = archive_name(release.version(), target);
            let sums = format!("{archive}.sha256");
            (version.pre.is_empty()
                && version > current
                && release.assets().iter().any(|asset| asset.name() == archive)
                && release.assets().iter().any(|asset| asset.name() == sums))
            .then_some((version, release))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, release)| release.clone())
}

pub fn check() -> Result<Option<Release>, String> {
    let releases = builder()?
        .build()
        .map_err(|e| e.to_string())?
        .get_newer_releases()
        .map_err(|e| e.to_string())?;
    Ok(select_release(releases.all(), VERSION, target()?))
}

fn install_location_reason(executable: &Path, os: &str) -> Option<&'static str> {
    if os == "macos"
        && !executable
            .ancestors()
            .any(|path| path.extension().is_some_and(|ext| ext == "app"))
    {
        return Some("Install the released Bitman.app to use in-app updates.");
    }
    if os == "linux"
        && ["/usr", "/bin", "/sbin", "/opt", "/snap", "/nix", "/app"]
            .iter()
            .any(|root| executable.starts_with(root))
    {
        return Some(
            "Update this system installation using its package manager or the release download.",
        );
    }
    None
}

pub fn manual_update_reason() -> Option<String> {
    if cfg!(debug_assertions) {
        return Some("Development builds do not install updates. Download a release build to use in-app updates.".into());
    }
    match std::env::current_exe().and_then(|path| path.canonicalize()) {
        Ok(path) => install_location_reason(&path, std::env::consts::OS).map(str::to_owned),
        Err(error) => Some(format!("Could not locate this installation: {error}")),
    }
}

pub fn install(release: &Release) -> Result<(), String> {
    if INSTALL_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("An update is already being installed.".into());
    }
    let _guard = InstallGuard;
    if let Some(reason) = manual_update_reason() {
        return Err(reason);
    }
    if select_release(std::slice::from_ref(release), VERSION, target()?).is_none() {
        return Err(
            "The release is not a complete, newer stable release for this platform.".into(),
        );
    }
    install_builder(release, target()?)?
        .build()
        .map_err(|e| e.to_string())?
        .update()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn install_builder(release: &Release, target: &str) -> Result<github::UpdateBuilder, String> {
    let archive = archive_name(release.version(), target);
    let mut builder = builder()?;
    builder
        .target(target)
        // Pin installation to the version the user accepted, even if a new release appears.
        .release_tag(format!("v{}", release.version()))
        .checksum_from_asset(format!("{archive}.sha256"))
        .asset_matcher(move |assets| assets.iter().find(|asset| asset.name() == archive).cloned())
        .check_install_path_writable(true);
    if target.ends_with("apple-darwin") {
        builder.bundle_path_in_archive("Bitman.app");
        #[cfg(target_os = "macos")]
        builder.verify_binary(|bundle| {
            let output = std::process::Command::new("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(bundle)
                .output()?;
            if !output.status.success() {
                return Err(self_update::Error::verification_rejected(
                    "The downloaded app has an invalid code signature.",
                ));
            }
            Ok(())
        });
    } else {
        let file = if target.ends_with("windows-msvc") {
            "bitman.exe"
        } else {
            "usr/bin/bitman"
        };
        builder.bin_path_in_archive(format!("bitman-{}-{target}/{file}", release.version()));
    }
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use self_update::ReleaseAsset;

    fn release(version: &str, target: &str, checksum: bool) -> Release {
        let archive = archive_name(version, target);
        let mut builder = Release::builder();
        builder
            .version(version)
            .name(version)
            .date("")
            .asset(ReleaseAsset::new(&archive, "https://example.com/package"));
        if checksum {
            builder.asset(ReleaseAsset::new(
                format!("{archive}.sha256"),
                "https://example.com/checksum",
            ));
        }
        builder.build().unwrap()
    }

    #[test]
    fn selects_newest_complete_native_stable_release() {
        let target = "aarch64-apple-darwin";
        let releases = [
            release("1.1.0", target, true),
            release("1.10.0", target, true),
            release("1.11.0", target, false),
            release("2.0.0-beta.1", target, true),
            release("3.0.0", "x86_64-apple-darwin", true),
        ];
        assert_eq!(
            select_release(&releases, "1.0.0", target)
                .unwrap()
                .version(),
            "1.10.0"
        );
        assert!(select_release(&releases, "1.10.0", target).is_none());
        assert!(select_release(&releases, "4.0.0", target).is_none());
    }

    #[test]
    fn matches_packaged_archives_exactly() {
        assert_eq!(
            archive_name("0.2.0", "x86_64-unknown-linux-gnu"),
            "bitman-0.2.0-x86_64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            archive_name("0.2.0", "x86_64-pc-windows-msvc"),
            "bitman-0.2.0-x86_64-pc-windows-msvc.zip"
        );
        assert!(select_release(&[], "0.1.0", "aarch64-apple-darwin").is_none());
    }

    #[test]
    fn protects_system_installations_and_unbundled_macos_builds() {
        assert!(install_location_reason(Path::new("/usr/bin/bitman"), "linux").is_some());
        assert!(
            install_location_reason(Path::new("/opt/bitman/usr/bin/bitman"), "linux").is_some()
        );
        assert!(
            install_location_reason(Path::new("/home/user/bitman/usr/bin/bitman"), "linux")
                .is_none()
        );
        assert!(
            install_location_reason(Path::new("/tmp/target/release/bitman"), "macos").is_some()
        );
        assert!(
            install_location_reason(
                Path::new("/Applications/Bitman.app/Contents/MacOS/bitman"),
                "macos"
            )
            .is_none()
        );
    }

    struct FixtureClient {
        corrupt: bool,
    }
    struct FixtureResponse {
        headers: self_update::http_client::HeaderMap,
        bytes: Vec<u8>,
    }

    impl self_update::http_client::HttpResponse for FixtureResponse {
        fn headers(&self) -> &self_update::http_client::HeaderMap {
            &self.headers
        }
        fn body(self: Box<Self>) -> Box<dyn std::io::Read> {
            Box::new(std::io::Cursor::new(self.bytes))
        }
    }

    impl self_update::http_client::HttpClient for FixtureClient {
        fn get(
            &self,
            url: &str,
            _: &self_update::http_client::HeaderMap,
            _: Option<Duration>,
        ) -> self_update::Result<Box<dyn self_update::http_client::HttpResponse>> {
            let bytes = match url {
                "https://api.github.com/repos/marinoandrea/bitman/releases/tags/v1.2.0" => br#"{
                    "tag_name":"v1.2.0", "created_at":"2026-01-01T00:00:00Z",
                    "assets":[
                        {"name":"bitman-1.2.0-x86_64-pc-windows-msvc.zip", "url":"https://api.github.com/archive"},
                        {"name":"bitman-1.2.0-x86_64-pc-windows-msvc.zip.sha256", "url":"https://api.github.com/checksum"}
                    ]
                }"#.to_vec(),
                "https://api.github.com/checksum" => b"9d815913988903a0679e594ad63152302211223812790adec692cdb1613656fd  bitman-1.2.0-x86_64-pc-windows-msvc.zip\n".to_vec(),
                "https://api.github.com/archive" => {
                    let mut bytes = include_bytes!("../tests/fixtures/updater/bitman-1.2.0-x86_64-pc-windows-msvc.zip").to_vec();
                    if self.corrupt { bytes[0] ^= 1; }
                    bytes
                }
                _ => panic!("Unexpected request: {url}"),
            };
            Ok(Box::new(FixtureResponse {
                headers: Default::default(),
                bytes,
            }))
        }
    }

    #[test]
    fn verifies_download_before_replacing_existing_installation() {
        for corrupt in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let executable = directory.path().join("bitman.exe");
            std::fs::write(&executable, b"old executable").unwrap();
            let target = "x86_64-pc-windows-msvc";
            let release = release("1.2.0", target, true);
            let result = install_builder(&release, target)
                .unwrap()
                .bin_install_path(&executable)
                .http_client(std::sync::Arc::new(FixtureClient { corrupt }))
                .build()
                .unwrap()
                .update();
            if corrupt {
                assert!(result.is_err(), "A corrupt download must be rejected");
                assert_eq!(std::fs::read(&executable).unwrap(), b"old executable");
            } else {
                assert!(result.unwrap().is_updated());
                assert_eq!(
                    std::fs::read(&executable).unwrap(),
                    b"new executable fixture"
                );
            }
        }
    }
}
