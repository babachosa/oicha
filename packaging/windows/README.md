# Windows packaging

Build on Windows x64 with PowerShell 7.2+, the pinned Rust toolchain and a licensed
Visual Studio C++ installation including its x64 redistributable CRT directory:

```powershell
./tools/build-windows-installer.ps1
# Already built the app:
./tools/build-windows-installer.ps1 -SkipBuild
# Explicit tool/runtime locations are also supported:
./tools/build-windows-installer.ps1 -NsisPath C:/Tools/NSIS/makensis.exe -VcRuntimeDirectory C:/path/to/x64/Microsoft.VC145.CRT
```

The script verifies x64 PE/version resources, verifies Microsoft's signatures on
the app-local CRT DLLs, creates a fresh staging folder, generates a whitelist for
uninstall, builds the installer, and writes SHA-256/payload manifests beside it.
Source licenses are copied directly from tracked source files; no preexisting
`dist` folder is required. The CI workflow builds and uploads the same artifacts.
CI desktop/GPU tests have to run separately on an interactive Windows desktop.

`tools/get-nsis.ps1` fetches NSIS **3.13** using Tauri's unmodified binary mirror:
https://github.com/tauri-apps/binary-releases/releases/tag/nsis-3.13

Archive SHA-256:
`BA63DFFC4410EE89193E1CB5A41989991BD77C61068DA17E3156D136B7B0B3D8`.
This matches the release asset digest; its SHA-1 also matches the NSIS download
constant in the Tauri bundler source. The initial SourceForge mirrors returned
HTML instead of archives and were rejected before execution. Only the verified
archive is used. NSIS's applicable licenses are shipped as `NSIS-COPYING.txt`.

Production installer policy:

- Current-user install, no elevation; Windows x64, build 19041+.
- Stable location `%LOCALAPPDATA%/Programs/Oicha`, stable uninstall/upgrade identity.
- Start Menu shortcut by default; Desktop and `.oicha` Open with entry are optional.
- File associations never overwrite Windows UserChoice or assign a new default.
- Setup/uninstall serialize with a per-user mutex; a running installed app must
  close/save first. No process is force-killed. Silent refusal returns exit 2.
- Newer registered versions cannot be overwritten with older packages.
- Unmanaged destinations are rejected; the executable is staged before replacement.
- Uninstall checks the install marker, deletes only its generated file whitelist,
  and uses nonrecursive directory removal. App data and manuscripts are preserved.
- Local VC runtime servicing is Oicha's responsibility; refresh it from the licensed
  Redist directory for subsequent releases. Versions/hashes are in the payload manifest.

Command line:

```powershell
./Oicha-0.1.0-Windows-x64-Setup.exe /S
# Optional components; /D, when used, must be the final unquoted argument:
./Oicha-0.1.0-Windows-x64-Setup.exe /S /DESKTOP /ASSOC /D=C:\Users\me\Apps\Oicha
# Uninstall without removing user data:
& "$env:LOCALAPPDATA/Programs/Oicha/Uninstall.exe" /S
```

Interactive lifecycle regression (refuses to run over an existing registered Oicha
installation or existing Oicha shortcuts):

```powershell
./tools/test-windows-installer.ps1
```

The application remains Rust/GPUI. NSIS is only the installer tool, and Sharp/Node
is used only to regenerate committed branding assets; neither is a runtime dependency.
The icon SVG, 9-size ICO, PNG preview and wizard artwork are in
`crates/oicha-app/assets/branding`. To regenerate them, install Sharp for the
maintainer script and run `node tools/generate-icon.cjs`.

This release is **unsigned at the user's request**. No self-signed publisher or
trusted signing identity is fabricated. A future signed release must sign the
application, installer and generated uninstaller with the publisher's real
code-signing certificate and a trusted timestamp, then re-run packaging QA.
