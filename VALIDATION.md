# Validation — 2026-10-18

Character-find changes in 0.2.6: see [VIM_FIND_VALIDATION_2026-10-18.md](VIM_FIND_VALIDATION_2026-10-18.md) for `f/F`, repeat/reverse, counted operators, Unicode handling and native dispatch checks.

**Audit findings resolved in 0.2.4.** See [AUDIT_FIX_VALIDATION_2026-10-17.md](AUDIT_FIX_VALIDATION_2026-10-17.md) for final hashes, 62 passing tests, native regressions, dependency gate and the investigation of test timing/clipboard contention. The [original 0.2.3 review](AUDIT_REVIEW_2026-10-17.md) is historical.

Latest candidate: **0.2.5**. See [VIM_IME_WRITING_VALIDATION_2026-10-17.md](VIM_IME_WRITING_VALIDATION_2026-10-17.md) for the UniKey/Vim input and writing-area fixes. [CURSOR_NAVIGATION_VALIDATION.md](CURSOR_NAVIGATION_VALIDATION.md) records the earlier 0.2.3 cursor fix. [SCROLL_EDGE_VALIDATION.md](SCROLL_EDGE_VALIDATION.md) records the actual signed 0.2.1 → 0.2.2 update test. Language/updater baseline evidence is in [LANGUAGE_UPDATE_VALIDATION.md](LANGUAGE_UPDATE_VALIDATION.md). Earlier evidence below is historical.

Windows x64, Rust 1.98.1 MSVC, GPUI 0.2.2. This records the candidate after fixing all six findings from the earlier production review and updating popup presentation/lifecycle, zz, end scroll space and Enter-at-EOF page advance, fixed A4 display pages, automatic soft-wrap follow, smooth typing/scroll motion, bundled Vietnamese Literata typography with clean page surfaces, whole-word Vietnamese wrapping and an integrated titlebar.

Latest UI cleanup removes the manuscript/autosave banner and its entire 34px row above the paper. Autosave behavior is unchanged. Fresh native renderer evidence for this cleanup is at artifacts/clean-canvas-release/results.json; the earlier unit/Clippy/caption checks below cover the unchanged logic and window controls.

Current Windows distribution adds embedded icon/version resources and the NSIS installer. The application passed 49 tests, Clippy, release build, and installed-app native renderer/caption tests. Installer lifecycle checks verify fresh install, same-version repair, downgrade/running-app/unmanaged-folder refusal, Unicode shortcuts, bundled runtime loading and uninstall with user data preserved. See WINDOWS_INSTALLER_VALIDATION.md and artifacts/windows-installer-qa-results.json. The build dependency embed-resource was already present in the pinned registry graph; only the app's dependency edge was added.

| Check | Result |
| --- | --- |
| cargo fmt --all --check | PASS |
| cargo test --workspace --locked | PASS: 38 core + 2 pagination + 1 glyph-layout + 2 security + 3 motion + 3 prose-wrap tests |
| Vim reference test --ignored | PASS: 67 cases match Vim 9.2 |
| cargo clippy --workspace --all-targets --locked -- -D warnings | PASS |
| cargo build -p oicha --release --locked | PASS |
| Security regressions --release --locked | PASS: row/column overflow rejects before mutation; valid growth; both memory orders |
| Native release: load succeeds | PASS, exit 0, queued close marker and both edits verified in saved files |
| Native release: load fails | PASS, exit 0, queued close marker and both edits verified in saved files |
| DOCX export | Both native outputs have 5 XML parts; all parse successfully |
| Dependency check (2026-10-14; lockfile unchanged) | PASS: 735 registry package versions scanned; local grid backport hash verified; no unreviewed advisory |

The native test exercises the actual UTF-16 adapter with accent-only replacement inside NFD text after an emoji, marked range, selection, commit and undo. It also checks short typing undo with Vim ON/OFF, IME across idle, toolbar history, Vietnamese baseline stability at 16/20/28px, A/0/j/k, wrapping/reflow, formatting, search, focus mode, saving and DOCX.

The load regression uses a deterministic worker barrier. Input entered into the old editor while a new book is being read must be present in the old file before the new book replaces it. Closing while loading is tested for both success and failure; input entered while that close is queued must be present in the current book before the process exits. The runner requires PASS reports, actual persisted content, a final-window-close marker and exit code 0.

Native tests use isolated manuscripts and skip the normal last-session pointer. Visible windows are required for real Windows/GPUI reflow frames: an initial hidden-window run suspended repainting and was discarded. tools/native-smoke.ps1 now explicitly runs visible renderer test windows.

Earlier fix-candidate release measurements for the same layout+navigation-mapping operation, Georgia 20px, width 580px, one ASCII paragraph and one Vietnamese insertion:

| Paragraph size | Before | After, success/failure runs |
| --- | ---: | ---: |
| 2,000 characters | 1.038 ms | 0.428 / 0.454 ms |
| 10,000 characters | 15.179 ms | 1.859 / 2.231 ms |
| 40,000 characters | 264.648 ms | 8.343 / 8.740 ms |

These are local observations, not percentile benchmarks or whole input-to-paint latency. Glyph position lookup preserves GPUI's first-matching-glyph behavior even for repeated clusters/non-monotone glyph order; unit tests compare against the former linear lookup. Paragraph row/position metadata is shared and reused when shaping and width are unchanged.

The registry grid 0.18.0 advisory is resolved in the local source backport; the 0.18 API/version is retained for Taffy compatibility. See vendor/grid/OICHA_PATCH.md, its SHA-256 record and DEPENDENCY_POLICY.md. Seven INFO Unmaintained advisories are explicitly tracked, logged on every scan and temporarily allowed in the pinned GPUI graph. They are not represented as fixed vulnerabilities. Any unreviewed advisory or incomplete scan fails the checker.

Executable: dist/Oicha.exe (12,221,440 bytes).
SHA-256: 906FC8A2D9ED0DCDED8C98295D2AE41DE69601A50C74F6F1B871F0A04562128F

Installer: dist/Oicha-0.1.0-Windows-x64-Setup.exe (3,873,263 bytes).
SHA-256: 6E71BD4351F2F0B18949442F2A13F639674FB1B9BDCD0BF3CCD97E13A6872F00.
Authenticode status: NotSigned, explicitly requested by the user. Hash/manifest sidecars accompany the installer.

Current prose/titlebar update: see PROSE_WINDOW_VALIDATION.md, artifacts/prose-titlebar-release/results.json, artifacts/prose-titlebar-window-release/results.json and artifacts/prose-titlebar-window-modal-release/results.json. The user's Vietnamese word-wrap example is verified against real shaping, and native caption hit tests/actions cover maximize/restore, minimize and closing with saved content, including with an open popup. All previous input/page/motion/persistence checks are retained.

Earlier typography update: see TYPOGRAPHY_VALIDATION.md and artifacts/literata-release/results.json. Native checks verify four bundled Literata faces shape Vietnamese at 16/20/28px without fallback, plus the existing input, page, motion and persistence regressions. The first-page title and page-count footer have been removed, and all text starts at the same top margin.

Earlier motion update: see MOTION_VALIDATION.md and artifacts/motion-release/results.json. Native regression coverage includes intermediate and settled caret/scroll positions, precision-scroll interruption, and the earlier page/input/persistence checks.

Earlier page update: see PAGINATION_VALIDATION.md and artifacts/pages-release/results.json. Soft wraps and Enter share the writing-area behavior. Fixed-size sheets, page-margin hit tests, j across page boundaries, empty paragraphs, wrapping, zz and resizing are exercised on the native renderer.

Earlier popup update: see POPUP_VALIDATION.md. The popup candidate passed both native scenarios at artifacts/popup-release/results.json, including cancel/reopen focus, input blocked during closing, explicit chapter deletion applied once, and Ctrl+Enter notes submission. Search, help and chapter confirmation were visually checked in isolated native windows.

Enter-at-EOF update: see SCROLL_VALIDATION.md and artifacts/enter-page-release/results.json. EOF Enter opens a fresh writing area with Vim ON/OFF, including immediate Vietnamese IME input; upper/middle edits keep their viewport, second Enter does not repeat the jump, newline undo works, and a small viewport/large font and Vim o are covered.

Scrolling update: see SCROLL_VALIDATION.md. Earlier native evidence at artifacts/zz-release/results.json includes last-row centering at 28px, interrupted z prefix, extra scroll space without document edits, and viewport resizing to 1040 × 660. Both native load/close scenarios retain the earlier regression coverage.

Earlier fix evidence: artifacts/fixes-native-release/results.json and artifacts/fixes-dependency-audit-final.json. The initial review is historical; see FIX_VALIDATION.md for each finding's resolution.

Remaining verification limits: actual UniKey/EVKey/Windows Telex implementations beyond the exercised native adapter, power-loss/disk-full fault injection of every GUI persistence step, soak/performance percentiles, Microsoft Word's full schema/round-trip validation, accessibility and non-Windows platforms are not certified. The binary is unsigned. Cargo still reports an upstream proc-macro-error2 future-compatibility warning. This candidate passes the specified local regressions; it is not a claim of full Word/Vim compatibility or defect-free production operation.
