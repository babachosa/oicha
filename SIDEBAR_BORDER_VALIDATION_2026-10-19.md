# Manuscript top separator — 2026-10-19

The main action bar already draws a 1px bottom border. The manuscript sidebar header also drew a 1px top border directly below it, producing a 2px separator over the left column. The sidebar's redundant top border is removed; the action bar owns the single continuous separator across the window.

The previously changed sidebar width, header padding, title size and spacing are restored to their original values (248px width, 18px header padding, 17px title). The user's requested change is the separator thickness, not those dimensions. Project actions and storage behavior are unchanged.

Verification: rustfmt and Clippy passed. Computer Use inspected an isolated 1040×660 native window and confirmed the single thin separator above MANUSCRIPT matches the other separators. The owned preview was saved and closed; the user's existing windows were left running. No new unit test was added for this styling-only adjustment.

The prior release EXE is preserved as `target/release/oicha-before-border-fix-2026-10-19.exe`; the running user instances were not terminated.

Final optimized EXE: `target/release/oicha.exe`, 16,806,400 bytes; SHA-256 `0A383367B8EB12E0C62AD0E6ACA4A322B0FA9F68FA12EC4CE2C285218498C187`.
Both complete native renderer/load/queued-close scenarios passed on this binary, including shaped wrapping, navigation and scroll geometry with the restored sidebar width. Evidence: `artifacts/sidebar-border-release-2026-10-19/results.json` and `acceptance.json`.
The binary is a local test build; no public release or installer was published.
