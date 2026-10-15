# Language and updater validation — 2026-10-16

Candidate: **Oicha 0.2.1, Windows x64**, unsigned with Authenticode. The update
envelope has a real Ed25519 signature using the public key committed in source.

- Installer: `dist/Oicha-0.2.1-Windows-x64-Setup.exe`, 4,696,244 bytes.
- Installer SHA-256: `2204380DEC0BA44AC1C042AC5D648653228EE2628E5C951F96EBAF4453643D1D`.
- App SHA-256: `D6F5CF4D228FCE1E39E4BC2C2CA5E395E7A25DA9E183770CF02344676B3C3FEF`.

Passed `cargo fmt --all --check`, workspace tests (**57 passed**, the existing
external Vim oracle remains explicitly ignored), Clippy with warnings denied,
and release-mode overflow regressions. Dependency scan checked **741** registry
versions with no unreviewed/blocking advisory. Rustls was upgraded from 0.23.45
to **0.23.46** for RUSTSEC-2026-0346/0347/0348/0349. The existing seven accepted
unmaintained advisory IDs remain recorded by the dependency policy.

The native GPUI smoke tests switch Vietnamese/English, check the persisted
preference and prompt/input language, and assert the serialized manuscript is
unchanged. Both installed release scenarios passed, including Vietnamese IME,
undo, wrapped-row Vim motions, scrolling, A4 pages, modal focus, I/O concurrency,
export, and graceful close. English settings were visually checked at 1040×660.
Desktop/GPU/IME fixtures run serially: an overlapping visual-preview capture
invalidated one artificial IME test; the complete serial installer/native suite
then passed.

Actual upgrade: installed the unpublished **0.2.0 QA baseline**, opened its real
native app, staged the signed 0.2.1 installer, and requested restart. An external
write conflict deliberately made save fail: the app retained its window, unsaved
text and pending update. After restoring the fixture, the app saved and closed,
the separate Rust helper installed 0.2.1, and the new executable restarted.
The restarted process verified its own version, saved Vietnamese/emoji text and
the English preference. All installed payload hashes, registry version, user-added
files and unselected shortcut options were preserved. The test uninstalled its
own installation afterward; no existing user installation was touched.

Final installer lifecycle passed: native GPU/IME and caption tests on the installed
0.2.1 binary, same-version repair, downgrade refusal, running-app protection,
unmanaged-folder protection, bundled runtime use, Unicode shortcuts/associations,
and data-preserving uninstall. Evidence is local and ignored by Git:

- `artifacts/windows-update-qa-results.json`
- `artifacts/windows-installer-qa-results.json`
- `artifacts/update-dependency-audit-final.json`
- `artifacts/ui-preview-03f60ab178d84bc786aa13fb35b0f599/settings-en.png`

The private signing key is outside the repository with a restricted ACL, and its
base64 value was configured as the repository's `OICHA_UPDATE_PRIVATE_KEY` Actions
secret. Only the public key is committed. A staged secret scan passed. A fresh
checkout preserves the exact vendored grid source bytes and recorded SHA-256 via
`.gitattributes`; its source code is unchanged when ignoring line endings.

The 0.2.0 fixture is only for local upgrade QA and is not a public release. Users
of 0.1.0 must run the 0.2.1 installer once because 0.1.0 has no updater.
