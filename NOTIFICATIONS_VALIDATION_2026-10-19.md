# Notifications — 2026-10-19

Previously the UI concatenated localized prefixes with raw `anyhow`/Windows errors. File paths and English backend messages appeared in the main banner, success notices shared the same error colors, and existing notices retained the language used at creation.

`crates/oicha-app/src/notifications.rs` now defines 19 notification kinds with separate Vietnamese/English titles and short recovery guidance. Copy is selected at render time. Backend diagnostics are retained separately, bounded to 16 KiB on a UTF-8 boundary, and displayed only after expanding Details. They can be copied for troubleshooting and are explicitly labelled as the original technical message.

The shared card covers startup/open/save/save-as, project listing/open/delete, export, clipboard, Vim commands, chapter validation, close failures, language preferences and update check/download/install/results. Cards distinguish warning, error, information and success with text, symbols and colors. Dismissing an error preserves the notice waiting behind it. Autosave success continues using the existing status without spawning a new banner on every save.

The modal shell now caps height to its available viewport and scrolls long content. This fixes the small-window case where expanding notification details in Settings previously pushed controls beyond the window.

## Verification

- Unit tests cover all 19 kinds in both languages, absence of backend data in public copy, render-time translation, typed missing-file classification, severity and UTF-8-safe bounded diagnostics.
- `cargo test --workspace --locked`: 85 passed, two optional diagnostics ignored. Final-source log: `artifacts/notifications-unit-tests-2026-10-19.txt`. Rustfmt and Clippy (`--workspace --all-targets --locked -- -D warnings`) passed. The existing upstream `proc-macro-error2` future-compatibility notice remains.
- Manual Windows Computer Use QA on the debug binary at 1040 × 660: Vietnamese compact missing-library card; expand Details; switch to English while the notification is active; verify title/body/buttons/technical label translate; dismiss the error and retain the green success notice; settings with simultaneous language/update failures; expand details and scroll to the footer; switch the remaining update failure to English. All passed. Only explicitly created fixtures under `artifacts/notifications-visual-2026-10-19` were used; real user windows and preferences were preserved.
- The initial visual run exposed a Settings height overflow. The final modal cap/scroll fix was built and verified in a fresh fixture (`settings-scroll.oicha`), including access to footer actions.

## Final optimized binary

- `cargo build --release -p oicha --locked`: passed. EXE: `target/release/oicha.exe`, 16,869,376 bytes; SHA-256 `712C09A5B38FCC77F9FB98E002CF9E31F629522C981976D6C621EC8126DCBAF1`.
- Deleted-library startup/autosave/normal-close fixture passed: `artifacts/notifications-missing-release-2026-10-19/results.json`.
- All four native close scenarios passed (save, discard, no store, failed save retains unsaved edits and window): `artifacts/notifications-close-release-2026-10-19/results.json`.
- Both full native editor scenarios passed, including modal lifecycle/focus, Vietnamese IME, Vim, wrapping/reflow, scroll motion, background save, DOCX export and queued close with persisted concurrent edits: `artifacts/notifications-full-release-2026-10-19/results.json`.
- Consolidated acceptance record: `artifacts/notifications-acceptance-2026-10-19.json`. `git diff --check` passed.

The preceding binary is preserved at `target/release/oicha-before-notifications-2026-10-19.exe`; real user app instances were not closed. No installer or release feed is published by this task.
