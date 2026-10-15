use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use oicha_update::{Envelope, Manifest, PUBLIC_KEY, hash_file, installer_url};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use std::{fs, io::Write, path::Path};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("probe") if args.len() == 3 => {
            if let Some(update) = oicha_update::check(&args[1])? {
                let version = update.manifest.version.clone();
                let _ready = oicha_update::download(update, Path::new(&args[2]), |_, _| {})?;
                println!(
                    "PASS: GitHub stable feed, signature, download and checksum verified for {version}"
                );
            } else {
                println!("No newer published release");
            }
        }
        Some("keygen") if args.len() == 3 => {
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .map_err(|_| anyhow::anyhow!("Could not generate signing key"))?;
            let key = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
                .map_err(|_| anyhow::anyhow!("Invalid signing key"))?;
            let secret = Path::new(&args[1]);
            if let Some(parent) = secret.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(secret)
                .context("Key already exists or cannot be created")?;
            file.write_all(pkcs8.as_ref())?;
            file.sync_all()?;
            fs::write(&args[2], STANDARD.encode(key.public_key().as_ref()) + "\n")?;
            println!(
                "Created private signing key and public verification key. Keep the private file outside the repository."
            );
        }
        Some("sign") if args.len() == 6 => {
            let key = Ed25519KeyPair::from_pkcs8(&fs::read(&args[1])?)
                .map_err(|_| anyhow::anyhow!("Invalid signing key"))?;
            ensure!(
                STANDARD.encode(key.public_key().as_ref()) == PUBLIC_KEY.trim(),
                "Signing key does not match the app's public key"
            );
            let version = &args[2];
            let (bytes, sha256) = hash_file(Path::new(&args[3]))?;
            let payload = serde_json::to_vec(&Manifest {
                schema: 1,
                version: version.clone(),
                platform: "windows-x86_64".into(),
                url: installer_url(version),
                bytes,
                sha256,
                notes: fs::read_to_string(&args[4])?,
            })?;
            let envelope = serde_json::to_vec_pretty(&Envelope {
                payload: STANDARD.encode(&payload),
                signature: STANDARD.encode(key.sign(&payload).as_ref()),
            })?;
            oicha_update::verify(&envelope, PUBLIC_KEY)?;
            fs::write(&args[5], envelope)?;
            println!("Signed update metadata for {version}");
        }
        _ => anyhow::bail!(
            "Usage: oicha-release keygen PRIVATE.pk8 PUBLIC.txt | sign PRIVATE.pk8 VERSION SETUP.exe NOTES.txt OUTPUT.json"
        ),
    }
    Ok(())
}
