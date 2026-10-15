# Windows installer and application branding — 2026-10-16

Application: Oicha 0.1.0, Rust/GPUI, Windows x64. Setup: Unicode NSIS 3.13,
Vietnamese wizard, current-user install without elevation. Windows 10 build 19041+
or Windows 11 and DirectX 11 GPU/driver are required. Installer/app remain unsigned,
as explicitly requested by the user.

The original book/quill branding is embedded as Windows group-icon resource ID 1,
which GPUI loads for the window/taskbar, with 16/20/24/32/40/48/64/128/256 px frames.
The executable has ProductName, FileDescription and version resources. The setup,
uninstaller, Start Menu/Desktop shortcuts and `.oicha` Open with entry use the icon.
The welcome/finish wizard also uses original Oicha artwork. Branding source and
asset generator are tracked in the repository.

| Check | Evidence/result |
| --- | --- |
| Workspace tests | 49 pass, including the app resource build step |
| Format / Clippy | Pass, warnings denied |
| Release build | Pass; x64 PE and version metadata verified before packaging |
| Icon resources | ID 1, all 9 expected sizes: artifacts/windows-branding/resources.json |
| Windows icon extraction | App and setup icons extracted with Windows APIs and visually inspected |
| Live GPUI icon | Class HICON verified nonzero and extracted: artifacts/windows-branding/runtime-class-icon.png |
| Payload integrity | Every installed file matches the staged SHA-256 manifest |
| Unicode/space paths | Real install under `Bản cài có khoảng trắng`; Windows Shell canonical shortcut targets and Unicode icon fields match |
| Runtime dependency | Microsoft-signed x64 VC CRT DLLs packaged; the installed process actually loads its app-local vcruntime140.dll |
| Reinstall/repair | Same-version install succeeds and preserves a user manuscript in the install folder |
| Downgrade guard | Temporarily registering a newer version makes the installer refuse with exit 2 and leaves the executable unchanged |
| Destination protection | Existing unrelated directory refused; its README preserved |
| Running-app protection | Silent setup and uninstall both refuse with exit 2; native app remains running/intact |
| Native regressions on installed app | Both load-success/load-failure renderer scenarios and caption control test pass |
| Uninstall | App, runtime files, shortcuts and owned registration removed; user manuscript and app-data sentinel retained |

Latest lifecycle evidence: `artifacts/windows-installer-qa-results.json` points to
the isolated test folder, which contains the nested native renderer/caption reports.
Build evidence: `artifacts/windows-installer-build.json`. The installer `.sha256`
and `.manifest.json` sidecars record the exact distribution hash and payload hashes,
runtime versions and unsigned status.

Installer welcome page was visually inspected at
`artifacts/windows-installer-visual/welcome.png`; cancelling that preview exits
without installing or registering the app. The final tested installer is
3,873,263 bytes; SHA-256
`6E71BD4351F2F0B18949442F2A13F639674FB1B9BDCD0BF3CCD97E13A6872F00`.

The release pipeline downloads only the pinned, SHA-256-verified NSIS archive,
compiles with warnings denied, generates its uninstall whitelist from staged files,
and does not consume ignored development artifacts. See packaging/windows/README.md.
The GitHub workflow is updated, but no remote CI run was launched in this session.

These are local Windows install/renderer checks, not a clean-OS VM or a certificate/
SmartScreen reputation certification. Future newer-version migrations, Windows 10
hardware, all DPI/monitor configurations and other Windows accounts require separate
rollout testing. No claim of an authenticated publisher is made for the unsigned files.
