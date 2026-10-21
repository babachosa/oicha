# Oicha 0.3.1 release review — 2026-10-21

Scope: the Vim character replacement implementation after 0.3.0, plus two
newly published TLS security patches. The `.oicha` library format is unchanged
from 0.3.0. Older 0.2.x files remain intentionally unsupported and preserved.

## Changes

- Normal `r{char}`, counts, Visual replacement, Unicode graphemes and retained
  per-position formatting. Each replacement has one undo/redo transaction;
  redo placement matches Vim. Escape, shortcuts, blur and mouse repositioning
  cancel pending replacement. Literal command targets cannot open prompts or
  trigger open-row scrolling. VI/EN F1 help and README are updated.
- `U`, full `R` mode, dot-repeat, control quoting and digraphs remain outside
  the supported Vim subset. `u` undoes and `Ctrl+R` redoes.
- `rustls-webpki 0.103.15` → `0.103.16` and `webpki-roots 1.0.9` → `1.0.10`
  fix RUSTSEC-2026-0382/0381, published 2026-10-20. The initial release scan
  caught both; neither was added to the exception list. The final scan checks
  739 registry package versions with no blocking advisory. Existing tracked
  INFO Unmaintained exceptions remain visible.

Detailed implementation evidence and earlier candidate checks are in
[VIM_REPLACE_VALIDATION_2026-10-21.md](VIM_REPLACE_VALIDATION_2026-10-21.md).
Final 0.3.1 package evidence below supersedes that candidate's 0.3.0 version
resource and executable hash.

## Preservation and validation boundary

The user's installed app and open manuscript remain untouched. Native QA uses
owned, isolated fixtures. Full installer/uninstaller and helper-driven update
lifecycle tests from 0.3.0 remain historical evidence: they are not repeated
on this occupied desktop profile, and installation-preservation guards are
not bypassed. Installer scripts, helper, registration and storage
behavior have not changed. This release verifies packaging, final executable
native workflows, update signature/download integrity and the public feed.

## Final checks

- Final locked workspace tests: **93 passed**, two optional diagnostics ignored.
  Explicit installed-Vim differential oracle: passed, including 29 new `r`
  cases. Rustfmt, Clippy with warnings denied and diff checks: passed.
  Logs: `artifacts/release-0.3.1-tests.txt`, `-oracle.txt`, `-clippy.txt`.
- Patched TLS optimized workspace build: passed. EXE SHA-256:
  `1183669EBAAFFA8DFA5DF34FA0C4E8D824C9E805F867D7CD47AEC3747F521436`.
- NSIS 3.13 packaging, verified Microsoft runtimes, package payload hashes,
  EXE/installer version resources and production-key metadata signing: passed.
  Installer `dist/Oicha-0.3.1-Windows-x64-Setup.exe`, **5,219,465 bytes**;
  SHA-256 `EF45A2CA0E4DF96977CEAB8CF3AEABCE71802DE55ABFDED3569A928964B45517`.
  `artifacts/release-0.3.1-package-acceptance.json`.
- Both full native scenarios passed on the final unchanged EXE when run
  independently: Vietnamese IME/history, shaping/wrapping, motion, modals,
  saves/exports, concurrent edits during load and queued close. Both dispatch
  13 replacement targets and verify count, undo/redo, cancellation and no
  search/Ex/open-row leakage. `artifacts/release-0.3.1-native-isolated/`.
- The initial native run overlapped packaging and missed its 45 ms wheel-frame
  sample (offset had not advanced yet). Evidence is preserved in
  `artifacts/release-0.3.1-native/`. The isolated repeat passed without changing
  source or relaxing assertions. The failed owned fixture alone was stopped;
  the real installed app was preserved. Timing-sensitive animation sampling
  remains a QA limitation, rather than evidence that every frame deadline is
  guaranteed under desktop load.
- Consolidated acceptance: `artifacts/release-0.3.1-acceptance.json`.

## Production publication

- Published stable [v0.3.1](https://github.com/babachosa/oicha/releases/tag/v0.3.1)
  from commit `ef3a1c8e32255c429d3b30e61593cacd3643ee56`, GitHub release
  `419130100`, at 2026-10-21 03:43:37 UTC. Code and annotated tag were pushed.
- All four uploaded assets (Setup, SHA-256, manifest and signed metadata)
  matched local sizes and GitHub SHA-256 digests before publishing.
  `artifacts/release-0.3.1-upload-verification.json`.
- Anonymous production `oicha-release probe 0.3.0` downloaded **0.3.1** and
  verified the Ed25519 signature and actual installer checksum using the
  patched TLS stack. A probe from **0.3.1** reports no newer release.
  `artifacts/release-0.3.1-live-probe.txt`, `-current-probe.txt`.
- GitHub latest-release API confirms a public, stable 0.3.1 with all assets.
  Actions were still running at publication; the release was uploaded directly
  with `gh`, and its workflow preserves an already published release's assets.

No blocking defect remains in the exercised release workflows. This record
does not claim exhaustive Vim compatibility or a fresh installer-driven
upgrade/uninstall test on the user's live installation.
