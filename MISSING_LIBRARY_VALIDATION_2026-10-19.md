# Deleted library startup — 2026-10-19

The startup constructor combined an explicit path, the remembered `last-library` path and the fresh-profile default into a single `exists ? open : create` branch. Deleting `Tieu-thuyet2.oicha` therefore caused the next launch to create a new empty database with that same filename.

Startup selection is now implemented in `crates/oicha-app/src/library.rs`:

- Explicit and remembered paths are opened only. Missing files, including a remembered `Library.oicha`, are never recreated, and no fallback store or missing parent directory is created.
- Only a profile without `last-library` bootstraps the default file. An existing default remains intact. Unreadable, invalid UTF-8 or empty remembered-path preferences produce an error rather than silently falling back to creation.
- Metadata errors retain their OS error kind and pathname for the UI. A missing library produces a clear deleted/moved-file message and an empty Home with Open library / New project guidance. It does not trigger a save picker or autosave. Creation requires the user's explicit new-library action.
- The missing remembered path is retained until the user chooses a different library, so repeatedly reopening the application cannot recreate the deleted file. No real user library or preferences were modified by QA.

## Verification

- `cargo test --workspace --locked`: 82 passed, two optional diagnostics ignored. Five new startup tests cover repeated launch after deleting a remembered file, deletion of the remembered default, a missing explicit path with a valid remembered alternative, missing parent-directory preservation, fresh-profile creation/existing-default preservation, explicit-path precedence and invalid preferences. Log: `artifacts/missing-library-unit-tests.txt`.
- Rustfmt and Clippy with `-D warnings`: passed.
- Debug native startup: `artifacts/missing-library-debug-2026-10-19-b/results.json` — passed. The CLI fixture uses `create_new` to reserve its own file, closes its handle, removes only that fixture and starts through the production constructor. It waits beyond the two-second autosave interval, verifies that the file remains absent and the app has no live store/project or save dialog, then closes normally. The external driver checks exit 0, the window-close marker and that the file remains absent.
- An initial native run caught the database backend hiding NotFound inside its error wrapper: the file stayed absent, but the UI displayed a generic error. The metadata probe and typed-error assertion fix this; the subsequent native run passed.

- Final optimized EXE: `target/release/oicha.exe`, 16,845,312 bytes, SHA-256 `8EC48B1F2B91193A0A2E584EB7CF1ACBE621CA46DE89270D6D5E132A1D1429F1`.
- Release missing-library fixture passed: `artifacts/missing-library-release-2026-10-19/results.json`. The deleted file remained absent through startup, the autosave interval and normal shutdown; Home had no store/project or save dialog.
- Release healthy-library fixture passed: `artifacts/missing-library-normal-release-2026-10-19/results.json`. Two independent projects persisted in one file, including Vietnamese edits, save-before-switch, Home during save and English UI.
- Both full native editor scenarios (load success and load failure) passed, including rendering, Vietnamese IME, Vim, wrapping, scrolling, background saves, DOCX export and queued close with edits persisted: `artifacts/missing-library-full-release-2026-10-19/results.json`.
- Consolidated final evidence: `artifacts/missing-library-release-2026-10-19/acceptance.json`. `git diff --check` passed.

The prior release EXE is retained as `target/release/oicha-before-missing-library-fix-2026-10-19.exe`. The user's running instance was not closed. No release feed or installer has been published.
