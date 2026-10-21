# Dependency policy

The application and editor remain Rust/GPUI. The PowerShell scripts are build
and QA tools; the desktop runtime does not execute them.

Oicha 0.3.1 updates the locked TLS dependencies to `rustls-webpki 0.103.16`
and `webpki-roots 1.0.10`, fixing
[RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) and
[RUSTSEC-2026-0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html),
published on 2026-10-20. Neither vulnerability is excepted. Release validation
must check the signed production update feed and actual installer download
through the patched TLS stack as well as rerunning the dependency scan.

Oicha 0.2.4 upgrades the DOCX writer's `zip` dependency from 4.6.1 to 9.0.4.
This removes RUSTSEC-2026-0367, RUSTSEC-2026-0368 and RUSTSEC-2026-0369 from
the locked graph. These extraction advisories have not been added to any
exception list. DOCX writing and native export regressions must pass after the
upgrade; the runtime still does not import or extract ZIP archives.

CI queries OSV for every registry package/version in Cargo.lock. Incomplete
responses, pagination that was not consumed, network failure after retries and
unreviewed advisory IDs fail the check. Passing this check covers published
advisories available from that service at scan time, not all possible defects.

`grid 0.18.0` is a local security backport because GPUI 0.2.2/Taffy 0.9.0 use
its 0.18 API. Registry grid 0.18.0 is affected by
[GHSA-38c5-483c-4qqp](https://github.com/becheran/grid/security/advisories/GHSA-38c5-483c-4qqp).
The local source adds checked dimension addition/multiplication before resizing,
based on the [upstream fix](https://github.com/becheran/grid/commit/be213bd3528727148bef2d523c89e95d1fd9c072).
Original license/source provenance is retained in vendor/grid. CI verifies the
source hash and runs release tests for row/column overflows, both memory orders,
unchanged state after overflow and valid growth. A version-only audit may still
report 0.18.0; use the source backport and test evidence when assessing this case.

The following explicitly tracked advisories are INFO Unmaintained, rather than
published exploitable vulnerabilities. They belong to the pinned GPUI dependency
graph (the lockfile also includes non-Windows targets). They are reported on
every scan and temporarily allowed. Any new advisory ID blocks CI until review.
They have not been represented as fixed or silently excluded from scan results.

| Crate | Advisory | Follow-up |
| --- | --- | --- |
| async-std | [RUSTSEC-2025-0052](https://rustsec.org/advisories/RUSTSEC-2025-0052.html) | Track GPUI runtime migration |
| instant | [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384.html) | Track migration to web-time |
| paste | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) | Track upstream macro replacement |
| proc-macro-error2 | [RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173.html) | Track GPUI macro migration |
| rustls-pemfile | [RUSTSEC-2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134.html) | Track upstream PEM parser migration |
| rustybuzz | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html) | Track upstream shaping migration |
| ttf-parser | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) | Track upstream font parser migration |
| zvariant | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) | Track migration to zbus 6 wire module |
| zbus_names | [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | Track migration to zbus 6 names module |
| zvariant_derive | [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) | Track migration to zbus 6 derive macros |

Reviewed for 0.2.6 on 2026-10-18: the three new zbus-family advisories above
are explicitly INFO Unmaintained, with no patched standalone versions. The
upstream crates moved into zbus 6; this is a future upstream API migration,
not a vulnerability version patch. `cargo tree --target x86_64-pc-windows-msvc
--locked --offline -i zvariant` and the equivalent `zbus_names` query both
have no dependency paths. These crates remain in the cross-platform lockfile,
and their advisories remain visible in every scan. No new vulnerability advisory
was added to the informational list and no dependency version was changed.

Reassess these exceptions on every dependency upgrade. Renderer upgrades must
pass the real native wrapping/IME/undo smoke tests; do not blindly replace the
renderer or break its APIs to remove an informational warning.
