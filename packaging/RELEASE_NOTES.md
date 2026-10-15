Oicha now includes Vietnamese and English interface languages. Open Settings in
the title bar, choose Tiếng Việt or English, and keep writing without restarting.
The choice is saved locally; changing it never translates or modifies a manuscript.

Signed in-app updates are available from Settings. Oicha checks GitHub Releases,
verifies the Ed25519 signature and SHA-256 of the installer, saves the current
manuscript, and uses a separate Rust helper to upgrade and restart the installed app.
Network and verification failures leave the current installation in place. A failed
manuscript save blocks restart. Portable/development copies require Setup first.

The Windows installer remains unsigned with Authenticode; update metadata has its
own cryptographic signature. The first upgrade from 0.1.0 requires running Setup.

This release also updates rustls to 0.23.46 to resolve newly published TLS advisories.
