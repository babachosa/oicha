# Signed Windows updates

Open **Cài đặt / Settings** in the title bar to choose Tiếng Việt or English,
check for a release, download it, and restart to update. UI language is persisted
separately from manuscripts. Existing titles, chapter names and text are never
translated. New manuscripts/chapters receive default names in the selected language.

The updater uses the latest stable GitHub Release in `babachosa/oicha`. Publish
`oicha-update.json` and the exact versioned Windows installer as release assets.
Each envelope contains a base64 payload and an Ed25519 signature. The app embeds
only `crates/oicha-update/update-public-key.txt`, verifies the signature before
trusting metadata, requires the Windows x64 platform and a newer stable SemVer,
and restricts download URLs to this repository's exact versioned release asset.
HTTPS, timeouts, size limits and SHA-256 checks cover the download. The helper
checks the signature and installer hash again immediately before running Setup.

The downloaded package runs only after the app's existing save/I/O gate succeeds.
A helper copy waits for the original process to exit; it never kills the writer.
Setup runs per-user, preserves its component selections, manuscripts and app data,
and rejects downgrades. The helper reopens the manuscript afterward and records
success/failure for the next startup. Portable/development copies must use Setup.
Downloads that fail verification are deleted. Prepared updates are held in a
temporary directory until restart; helper diagnostics remain in the user's update
cache. There is no delta patching or automatic rollback of the entire payload.

Windows Authenticode is separate. The current executable/installer are unsigned;
the update envelope still has a real cryptographic signature. A code-signing
certificate can be added to packaging later without changing this trust model.

## Release signing

The production private key is outside this repository, under the maintainer's
local application data `Oicha/release-keys/update.pk8`, with a restricted Windows
ACL. Back it up securely. Never commit, log or upload that file as a release asset.
The GitHub Actions secret `OICHA_UPDATE_PRIVATE_KEY` contains its base64 encoding.
Only trusted release workflows should receive that secret.

For a release, bump the workspace version, update `packaging/RELEASE_NOTES.md`,
commit, and push the matching `vX.Y.Z` tag. The Windows release workflow checks,
builds, packages, signs metadata and publishes the installer plus metadata. The
signer refuses a private key that does not match the committed public key.
Do not overwrite an existing release/version; publish a newer one.

Local signing (PowerShell 7; use an existing private key):

```powershell
cargo run -p oicha-update --bin oicha-release --release --locked -- sign "$env:LOCALAPPDATA/Oicha/release-keys/update.pk8" 0.2.1 dist/Oicha-0.2.1-Windows-x64-Setup.exe packaging/RELEASE_NOTES.md dist/oicha-update.json
```

To create keys for a separate fork, `oicha-release keygen PRIVATE.pk8 PUBLIC.txt`
creates a new key without overwriting an existing private file. Replace the fork's
repository/URL constants and public key, build, then sign with the matching key.
Changing the production public key breaks trust for older installed clients;
key rotation requires a deliberate migration release.

## Verification

`cargo test --workspace --locked` covers invalid signatures, modified metadata,
wrong platforms/URLs, truncated/corrupt/oversized downloads, cleanup, HTTPS-only
transport, semantic version comparisons and preferences persistence.

`tools/test-windows-update.ps1 -BaselineInstaller OLD.exe -UpdateInstaller NEW.exe`
installs an isolated baseline, runs the real installed app and helper, deliberately
causes a save conflict to verify restart is blocked, restores the test document,
then upgrades. The restarted executable verifies its new version, unsaved
Vietnamese/emoji text and English preference. The script checks all installed
payload hashes, registry version, preserved shortcut choices and user files,
then uninstalls its own fixture. It refuses to touch an existing user installation.

The `--smoke-test`, `--update-fixture`, `--update-installer` and `--update-verify`
flags are explicit QA entry points, not production feed overrides. QA still uses
the embedded production public key and signed installer. Normal sessions use
only the hard-coded HTTPS GitHub stable feed.

After publication, `oicha-release probe 0.2.0 artifacts/live-update-cache` uses the
production network feed and download path, verifies the real release and deletes
the temporary download when it exits; it does not install it.
