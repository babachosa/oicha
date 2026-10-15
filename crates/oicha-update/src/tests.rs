use super::*;
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};

fn fixture() -> (Ed25519KeyPair, String, Manifest, Vec<u8>) {
    let key = Ed25519KeyPair::from_pkcs8(
        Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .unwrap()
            .as_ref(),
    )
    .unwrap();
    let public = STANDARD.encode(key.public_key().as_ref());
    let bytes = b"MZ signed installer fixture".to_vec();
    let manifest = Manifest {
        schema: 1,
        version: "0.2.1".into(),
        platform: "windows-x86_64".into(),
        url: installer_url("0.2.1"),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        notes: "Language selection and signed updates".into(),
    };
    (key, public, manifest, bytes)
}
fn signed(key: &Ed25519KeyPair, manifest: &Manifest) -> Vec<u8> {
    let payload = serde_json::to_vec(manifest).unwrap();
    serde_json::to_vec(&Envelope {
        signature: STANDARD.encode(key.sign(&payload).as_ref()),
        payload: STANDARD.encode(payload),
    })
    .unwrap()
}

#[test]
fn verifies_signature_and_rejects_forged_or_modified_metadata() {
    let (key, public, manifest, _) = fixture();
    let bytes = signed(&key, &manifest);
    assert_eq!(verify(&bytes, &public).unwrap(), manifest);
    let (_, other_public, _, _) = fixture();
    assert!(verify(&bytes, &other_public).is_err());
    let mut envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
    let mut modified = manifest.clone();
    modified.version = "9.0.0".into();
    envelope.payload = STANDARD.encode(serde_json::to_vec(&modified).unwrap());
    assert!(verify(&serde_json::to_vec(&envelope).unwrap(), &public).is_err());
    assert!(verify(&vec![b'x'; MAX_METADATA as usize + 1], &public).is_err());
}

#[test]
fn rejects_wrong_platform_url_schema_and_oversize_installer() {
    let (key, public, manifest, _) = fixture();
    for changed in [
        Manifest {
            platform: "windows-aarch64".into(),
            ..manifest.clone()
        },
        Manifest {
            url: "https://evil.example/setup.exe".into(),
            ..manifest.clone()
        },
        Manifest {
            url: installer_url("0.2.0"),
            ..manifest.clone()
        },
        Manifest {
            schema: 2,
            ..manifest.clone()
        },
        Manifest {
            bytes: MAX_INSTALLER + 1,
            ..manifest.clone()
        },
        Manifest {
            bytes: 0,
            ..manifest.clone()
        },
        Manifest {
            version: "0.3.0-beta.1".into(),
            ..manifest.clone()
        },
    ] {
        assert!(
            verify(&signed(&key, &changed), &public).is_err(),
            "Accepted {changed:?}"
        );
    }
}

#[test]
fn partial_corrupt_oversize_downloads_are_removed_and_never_executable() {
    let (key, public, manifest, bytes) = fixture();
    let cache = tempfile::tempdir().unwrap();
    for input in [
        bytes[..bytes.len() - 1].to_vec(),
        vec![b'X'; bytes.len()],
        [bytes.clone(), vec![0]].concat(),
    ] {
        let update = Available {
            envelope: signed(&key, &manifest),
            manifest: manifest.clone(),
        };
        assert!(stage_with_key(update, cache.path(), &input[..], |_, _| {}, &public).is_err());
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }
}

#[test]
fn valid_download_reports_progress_and_rechecks_changed_metadata() {
    let (key, public, manifest, bytes) = fixture();
    let cache = tempfile::tempdir().unwrap();
    let mut progress = 0;
    let mut update = Available {
        envelope: signed(&key, &manifest),
        manifest: manifest.clone(),
    };
    let prepared = stage_with_key(
        update.clone(),
        cache.path(),
        &bytes[..],
        |done, total| {
            assert!(done <= total);
            progress = done;
        },
        &public,
    )
    .unwrap();
    assert_eq!(progress, bytes.len() as u64);
    assert_eq!(
        fs::read(prepared.directory.path().join("setup.exe")).unwrap(),
        bytes
    );
    update.manifest.url = "https://evil.example/setup.exe".into();
    assert!(stage_with_key(update, cache.path(), &bytes[..], |_, _| {}, &public).is_err());
}

#[test]
fn transport_refuses_plain_http() {
    assert!(
        agent(Duration::from_secs(1))
            .get("http://127.0.0.1:1/feed")
            .call()
            .is_err()
    );
}

#[test]
fn stable_versions_compare_numerically_and_never_offer_downgrades() {
    let (key, public, manifest, _) = fixture();
    let envelope = signed(&key, &manifest);
    assert!(
        available_with_key(&envelope, "0.2.0", &public)
            .unwrap()
            .is_some()
    );
    assert!(
        available_with_key(&envelope, "0.2.1", &public)
            .unwrap()
            .is_none()
    );
    assert!(
        available_with_key(&envelope, "0.10.0", &public)
            .unwrap()
            .is_none()
    );
    assert!(available_with_key(&envelope, "invalid", &public).is_err());
}
