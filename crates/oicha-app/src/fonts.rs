//! Ship the reading font with the app; never depend on an OS font installation.
use std::borrow::Cow;

pub const NOVEL_FAMILY: &str = "Literata";

pub fn register(cx: &gpui::App) -> anyhow::Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/literata/Literata-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/literata/Literata-Italic.ttf"
        )),
        Cow::Borrowed(include_bytes!("../assets/fonts/literata/Literata-Bold.ttf")),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/literata/Literata-BoldItalic.ttf"
        )),
    ])
}

/// Run only in the isolated native smoke scenario, on the real text backend.
pub fn validate_vietnamese(window: &mut gpui::Window) -> anyhow::Result<()> {
    use gpui::*;
    anyhow::ensure!(
        window
            .text_system()
            .all_font_names()
            .iter()
            .any(|name| name == NOVEL_FAMILY),
        "Bundled Literata is unavailable"
    );
    let mut sample = String::from(
        "ĂăÂâĐđÊêÔôƠơƯư áàảãạ éèẻẽẹ íìỉĩị óòỏõọ úùủũụ ýỳỷỹỵ a\u{0306}\u{0301} o\u{031b}\u{0323} “Mưa ngoài hiên…” — ",
    );
    sample.extend((0x1EA0..=0x1EF9).filter_map(char::from_u32));
    let sample: SharedString = sample.into();
    let mut faces = std::collections::HashSet::new();
    for face in [
        font(NOVEL_FAMILY),
        font(NOVEL_FAMILY).bold(),
        font(NOVEL_FAMILY).italic(),
        font(NOVEL_FAMILY).bold().italic(),
    ] {
        let expected = window.text_system().resolve_font(&face);
        faces.insert(expected);
        for size in [16., 20., 28.] {
            let line = window.text_system().shape_line(
                sample.clone(),
                px(size),
                &[TextRun {
                    len: sample.len(),
                    font: face.clone(),
                    color: rgb(0x302f2c).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            anyhow::ensure!(
                !line.runs.is_empty() && line.runs.iter().all(|run| run.font_id == expected),
                "Vietnamese fell back outside Literata at {size}px for {face:?}"
            );
        }
    }
    anyhow::ensure!(
        faces.len() == 4,
        "Literata styles did not resolve to four distinct faces"
    );
    Ok(())
}
