# Cursor navigation validation — 2026-10-16

Candidate: Oicha 0.2.3, Windows x64, Rust 1.98.1 MSVC, GPUI 0.2.2.

GUI Arrow Up/Down inherited Vim's persistent end-of-line goal after End or `$`.
In the native reproduction, Arrow Up moved from x=132.46 to x=508.02 on a longer
preceding row. GUI arrows now replace that infinite goal with the current finite
horizontal position. They retain the finite goal while passing through short
rows. Explicit Vim `$` followed by j/k continues to honor Vim's end-of-line goal.

| Check | Result |
| --- | --- |
| Workspace tests | PASS: 58 tests |
| Real Vim reference comparison | PASS |
| Clippy, all targets, warnings denied | PASS |
| Release build and NSIS packaging | PASS |
| Native cursor matrix, release binary | PASS: 7 cases |
| Existing native scroll boundary/first-row regressions | PASS |
| Native input, layout, persistence and queued-close regressions | PASS: load success and failure |

The cursor matrix sends real Windows key events to an isolated GPUI window and
checks cursor positions without document mutations. It covers arrows and k from
the middle of a row, arrows after End and `$`, Vim disabled, and long/short/long
row round trips with Vim enabled and disabled. After the fix, End then Arrow Up
moves from x=132.46 to x=131.68 (nearest glyph position). Both round trips restore
x=287.94 exactly. Local evidence:

- `artifacts/cursor-before/cursor-results.json`
- `artifacts/cursor-release-023/cursor-results.json`
- `artifacts/cursor-scroll-regression-023/results.json`
- `artifacts/cursor-native-023/results.json`

Installer: `dist/Oicha-0.2.3-Windows-x64-Setup.exe`, 4,706,599 bytes.
SHA-256: `F2CDC709D268B329AEF8ABC82E29CB64922A13299B817D7419A3D3718C2ECC9B`.

Application: `dist/Oicha.exe`.
SHA-256: `9B56EC7A280F917264F2FCA5A2703F7AC2EE0B63565A82F94BCEE7D2D6F62A44`.

Update metadata is signed with the existing Ed25519 release key. The installer
remains unsigned with Authenticode, as requested. Installer lifecycle and actual
restart/update were previously exercised for 0.2.1 → 0.2.2; they were not repeated
against the user's running installation for this cursor-only change. The user's
installed application and manuscripts were left untouched. These checks cover
the reproduced cursor bug, not every possible navigation sequence.
