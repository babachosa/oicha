//! Signed Windows updates. Network work is blocking and must run off the UI thread.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::signature::{ED25519, UnparsedPublicKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;

pub const REPOSITORY: &str = "babachosa/oicha";
pub const PUBLIC_KEY: &str = include_str!("../update-public-key.txt");
const FEED: &str = "https://github.com/babachosa/oicha/releases/latest/download/oicha-update.json";
const MAX_METADATA: u64 = 64 * 1024;
const MAX_INSTALLER: u64 = 128 * 1024 * 1024;
const MARKER: &str = "Oicha-8c8c69a3-7cc9-4f10-88da-25b2597bc46a";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub platform: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    pub notes: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub payload: String,
    pub signature: String,
}

#[derive(Clone)]
pub struct Available {
    pub manifest: Manifest,
    envelope: Vec<u8>,
}

pub struct Prepared {
    pub available: Available,
    directory: TempDir,
}

pub fn installer_url(version: &str) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/download/v{version}/Oicha-{version}-Windows-x64-Setup.exe"
    )
}

pub fn verify(envelope: &[u8], public_key: &str) -> Result<Manifest> {
    ensure!(
        envelope.len() as u64 <= MAX_METADATA,
        "Update metadata is too large"
    );
    let envelope: Envelope = serde_json::from_slice(envelope).context("Invalid update envelope")?;
    let key = STANDARD
        .decode(public_key.trim())
        .context("Invalid update public key")?;
    ensure!(key.len() == 32, "Invalid update public key length");
    let payload = STANDARD
        .decode(&envelope.payload)
        .context("Invalid signed payload")?;
    let signature = STANDARD
        .decode(&envelope.signature)
        .context("Invalid update signature")?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&payload, &signature)
        .map_err(|_| anyhow::anyhow!("Update signature verification failed"))?;
    let manifest: Manifest = serde_json::from_slice(&payload).context("Invalid update manifest")?;
    let version = Version::parse(&manifest.version)?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        "Expected a stable release version"
    );
    ensure!(
        version.to_string() == manifest.version,
        "Noncanonical release version"
    );
    ensure!(
        manifest.schema == 1 && manifest.platform == "windows-x86_64",
        "Unsupported update platform or schema"
    );
    ensure!(
        manifest.url == installer_url(&manifest.version),
        "Unexpected update download location"
    );
    ensure!(
        manifest.bytes > 0 && manifest.bytes <= MAX_INSTALLER,
        "Invalid installer size"
    );
    ensure!(
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid installer checksum"
    );
    ensure!(
        manifest.notes.len() <= 8 * 1024,
        "Release notes are too large"
    );
    Ok(manifest)
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent("Oicha-Updater/1")
        .build()
        .into()
}

pub fn check(current: &str) -> Result<Option<Available>> {
    let mut response = match agent(Duration::from_secs(30)).get(FEED).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error).context("Could not check GitHub Releases"),
    };
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)?;
    available(&bytes, current)
}

pub fn available(envelope: &[u8], current: &str) -> Result<Option<Available>> {
    available_with_key(envelope, current, PUBLIC_KEY)
}

fn available_with_key(
    envelope: &[u8],
    current: &str,
    public_key: &str,
) -> Result<Option<Available>> {
    let manifest = verify(envelope, public_key)?;
    if Version::parse(&manifest.version)? <= Version::parse(current)? {
        return Ok(None);
    }
    Ok(Some(Available {
        manifest,
        envelope: envelope.to_vec(),
    }))
}

pub fn download(
    update: Available,
    cache: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<Prepared> {
    ensure!(
        verify(&update.envelope, PUBLIC_KEY)? == update.manifest,
        "Update metadata changed"
    );
    let mut response = agent(Duration::from_secs(600))
        .get(&update.manifest.url)
        .call()
        .context("Could not download the update")?;
    stage(
        update,
        cache,
        response.body_mut().as_reader(),
        &mut progress,
    )
}

// Also used by the isolated native QA to stage a real, signed installer from disk.
// It has exactly the same signature, length and hash verification as a download.
pub fn stage(
    update: Available,
    cache: &Path,
    mut reader: impl Read,
    mut progress: impl FnMut(u64, u64),
) -> Result<Prepared> {
    stage_with_key(update, cache, &mut reader, &mut progress, PUBLIC_KEY)
}

fn stage_with_key(
    update: Available,
    cache: &Path,
    mut reader: impl Read,
    mut progress: impl FnMut(u64, u64),
    key: &str,
) -> Result<Prepared> {
    let verified = verify(&update.envelope, key)?;
    ensure!(verified == update.manifest, "Update metadata changed");
    fs::create_dir_all(cache)?;
    let directory = tempfile::Builder::new()
        .prefix("oicha-update-")
        .tempdir_in(cache)?;
    let mut output = File::create(directory.path().join("setup.exe"))?;
    let mut total = 0;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(total <= verified.bytes, "Installer exceeds signed size");
        output.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        progress(total, verified.bytes);
    }
    ensure!(total == verified.bytes, "Update download is incomplete");
    ensure!(
        format!("{:x}", digest.finalize()).eq_ignore_ascii_case(&verified.sha256),
        "Installer checksum verification failed"
    );
    output.sync_all()?;
    fs::write(directory.path().join("oicha-update.json"), &update.envelope)?;
    Ok(Prepared {
        available: update,
        directory,
    })
}

pub fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let count = std::io::copy(&mut input, &mut DigestWriter(&mut digest))?;
    Ok((count, format!("{:x}", digest.finalize())))
}
struct DigestWriter<'a>(&'a mut Sha256);
impl Write for DigestWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Plan {
    parent_pid: u32,
    target: PathBuf,
    result: PathBuf,
    manuscript: Option<PathBuf>,
    smoke: bool,
}

pub fn managed_installation() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Oicha", KEY_READ | KEY_WOW64_64KEY)
            .context("Install Oicha using Setup before updating it")?;
        let path: String = key.get_value("InstallDir")?;
        let directory = PathBuf::from(path);
        ensure!(
            fs::read_to_string(directory.join("oicha-install-id.txt"))? == MARKER,
            "Unknown Oicha installation"
        );
        Ok(directory)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("In-app updates currently support Windows only")
    }
}

impl Prepared {
    /// Start the helper before closing. It waits for this process to exit.
    /// On failure the caller keeps its window and manuscript open.
    pub fn launch(self, manuscript: Option<PathBuf>, result: PathBuf) -> Result<()> {
        let target = managed_installation()?;
        ensure!(
            std::env::current_exe()?.canonicalize()? == target.join("Oicha.exe").canonicalize()?,
            "Update the installed Oicha app, not a portable/development copy"
        );
        let helper = self.directory.path().join("update-helper.exe");
        fs::copy(std::env::current_exe()?, &helper)?;
        // The helper is the same native app; carry its app-local CRT with it.
        for entry in fs::read_dir(&target)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
                && path.is_file()
            {
                fs::copy(&path, self.directory.path().join(path.file_name().unwrap()))?;
            }
        }
        let plan = self.directory.path().join("plan.json");
        let smoke = std::env::args_os()
            .nth(1)
            .is_some_and(|s| s == "--smoke-test");
        fs::write(
            &plan,
            serde_json::to_vec(&Plan {
                parent_pid: std::process::id(),
                target,
                result,
                manuscript,
                smoke,
            })?,
        )?;
        let mut command = std::process::Command::new(helper);
        command.arg("--apply-update").arg(plan);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .spawn()
            .context("Could not start the update helper")?;
        let _ = self.directory.keep();
        Ok(())
    }
}

pub fn apply(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let plan: Plan = serde_json::from_slice(&fs::read(path)?)?;
        let directory = path.parent().context("Missing helper directory")?;
        ensure!(
            std::env::current_exe()?.canonicalize()?
                == directory.join("update-helper.exe").canonicalize()?,
            "Update helper location mismatch"
        );
        ensure!(
            managed_installation()?.canonicalize()? == plan.target.canonicalize()?,
            "Installation location changed"
        );
        // If the parent is still running, never start a second copy or install
        // over it. Returning also leaves the signed staged package for diagnosis.
        wait_for_parent(plan.parent_pid)?;
        let result = (|| -> Result<()> {
            let manifest = verify(&fs::read(directory.join("oicha-update.json"))?, PUBLIC_KEY)?;
            let installer = directory.join("setup.exe");
            let (bytes, sha256) = hash_file(&installer)?;
            ensure!(
                bytes == manifest.bytes && sha256.eq_ignore_ascii_case(&manifest.sha256),
                "Staged installer was changed"
            );
            let target = plan.target.to_str().context("Non-Unicode install path")?;
            ensure!(
                !target.contains(['"', '\r', '\n']),
                "Invalid installation path"
            );
            let mut exit = None;
            // The parent's image mapping can outlive its signaled process briefly.
            for _ in 0..20 {
                let status = std::process::Command::new(&installer)
                    .arg("/S")
                    .raw_arg(format!("/D={target}"))
                    .creation_flags(0x08000000)
                    .status()?;
                if status.code() != Some(2) {
                    exit = Some(status);
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            ensure!(
                exit.is_some_and(|s| s.success()),
                "The installer could not complete; your manuscript is preserved"
            );
            let installed: serde_json::Value =
                serde_json::from_slice(&fs::read(plan.target.join("package-manifest.json"))?)?;
            ensure!(
                installed["version"] == manifest.version,
                "Installed version mismatch"
            );
            Ok(())
        })();
        if let Some(parent) = plan.result.parent() {
            fs::create_dir_all(parent)?;
        }
        let report = match &result {
            Ok(()) => "OK".to_string(),
            Err(error) => format!("ERROR: {error:#}"),
        };
        fs::write(&plan.result, report)?;
        let mut restart = std::process::Command::new(plan.target.join("Oicha.exe"));
        if plan.smoke {
            restart.arg("--update-verify");
        }
        if let Some(manuscript) = &plan.manuscript {
            restart.arg(manuscript);
        }
        restart.spawn().context("Could not reopen Oicha")?;
        result
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        anyhow::bail!("Windows updates only")
    }
}

#[cfg(windows)]
fn wait_for_parent(pid: u32) -> Result<()> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    ensure!(
        pid != std::process::id(),
        "Cannot wait on the updater itself"
    );
    // SAFETY: the process handle is only waited on, then closed exactly once.
    // No process memory is read/written and the parent is never terminated.
    unsafe {
        match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(handle) => {
                let result = WaitForSingleObject(handle, 120_000);
                let _ = CloseHandle(handle);
                ensure!(result.0 == 0, "Oicha did not close; update was cancelled");
            }
            Err(error) if error.code() == windows::core::HRESULT::from_win32(87) => {}
            Err(error) => return Err(error).context("Could not wait for Oicha to close"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
