# Single-file multi-project library — 2026-10-19

Local, unpublished development change after 0.2.7. No release tag or public update feed has been changed.

## Behavior

- A `.oicha` file is now a redb 4.4.0 database, schema 2, containing independent projects, the project index and internal crash checkpoints. This deliberately rejects previous JSON manuscripts; they are never converted or overwritten.
- Project creation no longer creates a new manuscript file. Home lists the current library's projects; returning Home and switching projects wait for outstanding changes to save. A transition blocks editing the previous document while the storage selection changes.
- Save library as copies all projects, includes the active unsaved snapshot, and changes to the copy only after success. Existing destination files are refused using `create_new`.
- The application opens Home on ordinary startup and remembers `last-library`, independently of the old `last-session` pointer. First launch creates an empty `Library.oicha` in app data. Language preferences remain application settings.
- The main action toolbar is 38px (previously 50px), with a Home action and compact project/library actions. The sidebar session goal, sidebar branding block, and inspector quotation are removed in Vietnamese and English.
- Save / Don't Save / Cancel remains in place. Don't Save does not require a database write when there is no checkpoint to remove. The deletion confirmation no longer promises a nonexistent `.bak` file.

## Storage guarantees and checks

redb provides the database file lock and durable transactions within the single file. The integration uses its default immediate durability, not asynchronous commits. See [redb](https://github.com/cberner/redb) and its [design](https://github.com/cberner/redb/blob/master/docs/design.md). Its MIT license is retained in `third_party/redb` and added to installer packaging.

Core tests cover independent projects and Unicode, Save As of the whole library with current unsaved changes, internal recovery/discard isolation, an exclusive second-open failure, refusing an existing destination, preserving rejected old-format bytes, aborted/invalid writes, and abrupt subprocess exit both before and after commit. A dedicated directory contains exactly one file after these operations. Each project is limited to 64 MiB of serialized content and each library to 10,000 projects.

## Verification

- `cargo test --workspace --locked`: 75 passed; two opt-in diagnostics ignored in the default run. The storage crash test terminates a child process without destructors, then reopens the same file.
- `cargo test -p oicha-core --test vim_reference --locked -- --ignored`: passed against the installed unconfigured Vim.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- Dependency gate: 739 registry package versions checked; no unreviewed advisory. Existing explicitly tracked informational notices remain visible. Evidence: `artifacts/library-dependency-audit.json`.
- Debug native Home scenario: `artifacts/library-debug-2026-10-19-b/results.json`. Empty Home, hidden editor input isolation, creating two projects in the same file, Vietnamese text, save-before-switch, Home requested during an active save, English selection, reopening and inspecting both projects all passed. The first external checker incorrectly assumed the Home sort order was project-ID order; it was corrected to look up IDs. The application scenario itself passed in that run as well.
- Debug close scenarios: `artifacts/library-close-debug-2026-10-19/results.json`: save, discard, no-store and failed save passed, with closed-file inspection.
- Debug full native renderer scenarios: `artifacts/library-full-debug-2026-10-19/results.json`: load success/failure, input during I/O, queued close, Vietnamese shaping/IME adapter, Vim, display-row dd, page/scroll/caret and DOCX regressions passed.
- Computer Use visual check on an isolated 1040×660 window: Home list, creating a second project by clicking, compact toolbar, returning Home and displaying both projects passed. The three removed panels are absent. The fixture was closed with its own caption button; the user's installed app/manuscript was left open.
- Additional Computer Use check: used the native Save As dialog to save a two-project library as `copy.oicha`, saw Home switch to the new path, changed the UI to English, dismissed Settings back to English Home, and closed cleanly using Alt+F4. Closed-file inspection confirms both projects' complete manuscript values match the source. Evidence: `artifacts/library-copy-ui-2026-10-19/results.json`. The native dialog's accessibility set-value action was unavailable; selecting File name with Alt+N and typing succeeded.

Final release-binary verification passed:

- Optimized build: `target/release/oicha.exe`, 16,550,912 bytes.
- SHA-256: `BD4803C18C5F1F6A00F75FC8D685736E471BBF857F84C6F0A53D94E7B4E1F242`.
- Home/library/reopen: `artifacts/library-release-2026-10-19/results.json`.
- Four close flows: `artifacts/library-close-release-2026-10-19/results.json`.
- Two full native renderer/load/queued-close flows: `artifacts/library-full-release-2026-10-19/results.json`.
- Combined acceptance and binary fingerprint: `artifacts/library-release-2026-10-19/acceptance.json`.
- Final workspace test log: `artifacts/library-final-unit-tests.txt`; 75 passed. Format and Clippy passed after the final source edits. The existing upstream `proc-macro-error2` future-compatibility warning remains.

The EXE recorded above is the initial local library candidate. The later menu/sidebar candidate is recorded in [PROJECT_ACTIONS_VALIDATION_2026-10-19.md](PROJECT_ACTIONS_VALIDATION_2026-10-19.md). No installer, version tag, Git push or public update manifest was published for the initial library change.

## Limits

These are local regressions, not certification of defect-free operation. Abrupt process termination was tested; physical power loss and disk-full injection into every redb commit were not. Uncommitted text since the last two-second autosave can still be lost on process termination. Undo is retained while returning Home to the same project, and rebuilt when switching to another project. No external backup is created automatically. Earlier `.bak`/`.recovery` validation notes describe the superseded storage implementation.
