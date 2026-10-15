use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/oicha.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = env::var("CARGO_PKG_VERSION").expect("package version");
    let parts: Vec<u16> = version
        .split('.')
        .map(|part| part.parse().expect("numeric release version"))
        .collect();
    assert_eq!(
        parts.len(),
        3,
        "release version must have three numeric components"
    );
    let numeric = format!("{},{},{},0", parts[0], parts[1], parts[2]);
    let icon =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/branding/oicha.ico");
    let icon = icon
        .to_str()
        .expect("UTF-8 icon path")
        .replace('\\', "\\\\");
    let resource = format!(
        r#"
#pragma code_page(65001)
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Oicha contributors\0"
      VALUE "FileDescription", "Oicha — Novel Writing Editor\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "InternalName", "Oicha\0"
      VALUE "OriginalFilename", "Oicha.exe\0"
      VALUE "ProductName", "Oicha\0"
      VALUE "ProductVersion", "{version}\0"
      VALUE "LegalCopyright", "Copyright 2026 Oicha contributors\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("oicha.rc");
    fs::write(&output, resource).expect("write Windows resources");
    embed_resource::compile(output, embed_resource::NONE)
        .manifest_optional()
        .expect("compile app icon/version resources");
}
