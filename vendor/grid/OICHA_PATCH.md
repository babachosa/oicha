# grid 0.18.0 security backport

Source: crates.io grid 0.18.0; original MIT license and copyright retained.
Only expand_rows/expand_cols dimension calculations are changed: checked_add
and checked_mul run before resizing or modifying dimensions. Overflow panics
before the grid changes, rather than wrapping its allocation size in release.
This backports the dimension checks from upstream commit:
https://github.com/becheran/grid/commit/be213bd3528727148bef2d523c89e95d1fd9c072

Advisory: GHSA-38c5-483c-4qqp / CVE-2026-42199.
The upstream advisory fixes grid 1.0.1; Taffy 0.9.0 requires the 0.18 API, so this
workspace uses the reviewed local backport through [patch.crates-io]. Upstream
0.18 does not have the prepend methods introduced in the later API.

Regressions in crates/oicha-core/tests/security_regressions.rs exercise both
memory orders, row/column overflow, unchanged state after panic, and valid growth
in debug AND release. A registry package/version scan alone can still flag 0.18:
the version string is retained for API compatibility, not a claim of unpatched
registry code. CI checks the vendored-source hash and runs these release tests.

Six iterator return lifetimes are also made explicit (no behavior change) to
avoid mismatched_lifetime_syntaxes warnings on Rust 1.98.
