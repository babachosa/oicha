# Oicha 0.3.0 release review — 2026-10-19

Scope: all unpublished changes since v0.2.7, plus the existing save/close/load/update gates, installer packaging and signature/download verification. Windows x64 / Rust 1.98.1 MSVC / GPUI 0.2.2. Existing user app instances, libraries and shortcuts are preserved during QA.

## Review fixes

1. **Autosave blocked by unrelated notifications.** The timer previously stopped for any main error, including an unsupported Vim command or chapter warning. Only a failed manuscript write now pauses automatic retry. A successful autosave clears a write failure, but leaves unrelated notices visible. Unit coverage checks all notification kinds; the native two-project fixture edits Vietnamese text after a Vim warning, waits for actual autosave and verifies both the saved revision and retained notice.
2. **Corrupt next-project ID could overwrite an existing project.** `new_project` now rejects an occupied ID before writing anything. The regression deliberately corrupts next-ID metadata and checks that the existing project survives reopening.
3. **Update QA assumed JSON and attempted to replace a live database.** New-format QA now forces validation failure before a transaction and restores only its in-memory fixture state. The driver inspects binary libraries using `--inspect-store`. For the actual public 0.2.7 baseline it explicitly verifies preserved legacy JSON and its rejection by the new storage API. The QA-only `--update-verify` entry point can read that old fixture for preservation assertions; normal startup has no compatibility/import path.
4. **Installer QA encountered existing user shortcuts.** An explicit `-PreserveShortcuts` mode disables shortcut creation for the isolated installation and verifies the original links' hashes afterward. It does not overwrite, move or remove existing links. Shortcut creation remains covered by the earlier installer lifecycle evidence; this run covers preservation and executable resources.
5. **Clean-close installer/caption fixtures assumed the previous automatic-save-on-X behavior.** They now allow the normal autosave interval before testing a clean close; dirty close is exercised separately by the four-choice-path native fixture. The first installed-app run correctly showed the new Save / Don't Save / Cancel modal and therefore timed out under the old test assumption. Its owned manuscript was saved and closed through the UI, its isolated installation cleaned up, and the corrected lifecycle test rerun. This run skips the raw-message caption probe in favor of Windows Computer Use checks.

## Release content and intentional limits

One binary `.oicha` library contains multiple independent projects. Home supports project switching and confirmed deletion. The release includes displayed-row `dd`, retained EOF writing space, flush caption controls, Save / Don't Save / Cancel, compact project UI, deleted-library startup handling and localized notification cards with expandable diagnostics.

Version 0.3.0 is deliberately a breaking storage release: legacy 0.2.x JSON files are kept intact but are not opened or migrated. This is stated in the installer release notes and README in addition to the existing storage documentation. The user explicitly requested no legacy compatibility. Windows executable/installer remain Authenticode-unsigned as previously authorized; update metadata uses the unchanged production Ed25519 key. No private signing key is stored in the repository or uploaded.

## Checks

- Workspace tests: **87 passed**, two opt-in diagnostics ignored. `artifacts/release-0.3.0-tests.txt`.
- Rustfmt and Clippy with warnings denied: passed.
- Release-mode grid security regressions: two passed. Explicit differential test against the installed Git Vim: passed (`artifacts/release-0.3.0-vim-oracle.txt`); the intentionally customized final-row/display-row `dd` behavior is covered by the editor and native cases instead of the Vim oracle.
- Dependency scan: **739 registry package versions**, no unreviewed/blocking advisory; checked local grid backport. `artifacts/release-0.3.0-dependency-audit.json`. Previously documented upstream INFO Unmaintained advisories remain visible and accepted by policy; the existing proc-macro-error2 future-compatibility notice is unchanged.
- The public v0.2.7 installer used as update baseline matches its GitHub asset SHA-256: `3058BF3D1B20FF852F5C262312464CF60FDCF3AA77AB2C4AEDED22BE57EB20D5`.

## Release artifact acceptance

- Optimized workspace build and NSIS package: passed. Executable SHA-256: `8D6364336CA5355D7912D13480D6CE88F2E27AEDD6973A00DEDB63950853E44D`.
- Installer: `dist/Oicha-0.3.0-Windows-x64-Setup.exe`, 5,206,253 bytes; SHA-256 `8DAA4E9BD161EB6BFAE4520BB6BE277FD02C9373523F9831084B75E0D2D2E802`. Bundled Microsoft runtime signatures were checked during packaging. Correct executable/icon/version resources and redb license are included.
- New-library/project-switching/autosave-after-Vim-notice, missing/deleted-library, project deletion and four close-choice scenarios: passed (`artifacts/release-0.3.0-library`, `-missing`, `-project-actions`, `-close`).
- Installed-app lifecycle: passed. Same-version repair, downgrade rejection, unmanaged-folder rejection, running-app protection, bundled CRT loading, quoted file association, original shortcut preservation, uninstall with user files/app data preserved. `artifacts/windows-installer-qa-results.json`; final isolated run `artifacts/release-0.3.0-installer-retry-5f89b9031e20481fbccc7fda74d4829f`. New shortcut creation was intentionally not repeated over existing user links.
- Both full native editor scenarios passed inside the installed app, including Vietnamese IME, Vim, displayed-row `dd`, wrapping/reflow, page/scroll motion, modal focus/lifecycle, background save, DOCX export and concurrent edits across load/queued close.
- Windows Computer Use on the final EXE: maximize, restore, minimize, activate and X-close on a clean isolated Home all passed. Controls fit the top edge. The close marker is at `artifacts/release-0.3.0-caption/preview.smoke-closed.txt`.
- Production-key metadata signing passed. Actual signed helper/installer update **0.2.7 → 0.3.0** passed: failed save blocks restart, pending Vietnamese/emoji text is saved by the old app, new binary is installed/restarted, payload hashes and registry version match, English preference and shortcut options survive, and the legacy JSON document remains intact and is rejected by the new storage API. `artifacts/windows-update-qa-results.json`; `artifacts/update-qa-9d90913ea87c4f839de5a156521630c8`.
- Consolidated record: `artifacts/release-0.3.0-acceptance.json`. PowerShell syntax and `git diff --check` passed. Secret-pattern scan of publishable sources found no private keys or GitHub tokens.

## Production publication

- Published stable [v0.3.0](https://github.com/babachosa/oicha/releases/tag/v0.3.0), release commit `5c4ce723325e68ba3a82bc2edcaa06d2a830ad0a`, on 2026-10-19 at 07:07:39 UTC.
- All four uploaded assets were checked against local byte counts and GitHub SHA-256 digests before publishing: Setup, checksum, package manifest and signed `oicha-update.json`. Existing releases were preserved.
- `oicha-release probe 0.2.7 artifacts/release-0.3.0-live-feed`: passed using the anonymous production stable feed, real download, embedded public key and installer checksum. `artifacts/release-0.3.0-live-probe.txt`.
- Checking from version 0.3.0 reports no newer release, so it is not offered again. `artifacts/release-0.3.0-current-probe.txt`.

No blocking defect remains in the reviewed/tested Windows workflows. This review does not claim absence of every possible defect or validation of other operating systems.
