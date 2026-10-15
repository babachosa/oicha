#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod fonts;
mod motion;
mod pagination;
mod settings;
mod text_editor;
mod translations;
mod word_wrap;

use app::Oicha;
use gpui::*;

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|s| s == "--update-verify")
    {
        if let Some(path) = std::env::args_os().nth(2) {
            let path = std::path::PathBuf::from(path);
            let result = (|| -> anyhow::Result<()> {
                let (_, book, _) = oicha_core::storage::Store::open(path.clone())?;
                anyhow::ensure!(
                    book.chapters[0]
                        .body
                        .text
                        .contains("UPDATE_PRESERVE: Tiếng Việt 🌿 — unsaved text"),
                    "Unsaved manuscript text was lost"
                );
                let preferences: serde_json::Value = serde_json::from_slice(&std::fs::read(
                    path.with_extension("settings").join("preferences.json"),
                )?)?;
                anyhow::ensure!(
                    preferences["language"] == "en",
                    "Language preference was lost"
                );
                let installed: serde_json::Value = serde_json::from_slice(&std::fs::read(
                    std::env::current_exe()?
                        .parent()
                        .unwrap()
                        .join("package-manifest.json"),
                )?)?;
                anyhow::ensure!(
                    installed["version"] == env!("CARGO_PKG_VERSION"),
                    "Updated executable version mismatch"
                );
                Ok(())
            })();
            let report = match result {
                Ok(()) => format!(
                    "PASS: restarted Oicha {}; manuscript and English preference preserved",
                    env!("CARGO_PKG_VERSION")
                ),
                Err(error) => format!("FAIL: {error:#}"),
            };
            let _ = oicha_core::storage::atomic_write(
                &path.with_extension("update-verified.txt"),
                report.as_bytes(),
            );
        }
        return;
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|s| s == "--apply-update")
    {
        if let Some(plan) = std::env::args_os().nth(2)
            && let Err(error) = oicha_update::apply(&std::path::PathBuf::from(plan))
        {
            log::error!("Update helper failed: {error:#}");
        }
        return;
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let smoke = args.first().is_some_and(|s| s == "--smoke-test");
    let path = if smoke {
        let path = args
            .get(1)
            .map(std::path::PathBuf::from)
            .expect("--smoke-test requires an isolated .oicha path");
        let mut store =
            oicha_core::storage::Store::create(path.clone()).expect("create smoke manuscript");
        store
            .save(&oicha_core::document::Manuscript::demo())
            .expect("save smoke manuscript");
        Some(path)
    } else {
        args.first().map(std::path::PathBuf::from)
    };
    Application::new().run(move |cx: &mut App| {
        if let Err(error) = fonts::register(cx) {
            log::error!("Cannot load bundled reading fonts: {error:#}");
            cx.quit();
            return;
        }
        let smoke_closed =
            smoke.then(|| std::path::PathBuf::from(&args[1]).with_extension("smoke-closed.txt"));
        cx.on_window_closed(move |cx| {
            if cx.windows().is_empty() {
                if let Some(path) = &smoke_closed {
                    let _ = oicha_core::storage::atomic_write(
                        path,
                        b"PASS: queued close completed after I/O",
                    );
                }
                cx.quit();
            }
        })
        .detach();
        let window_size = if smoke && args.iter().any(|arg| arg == "--preview-small") {
            size(px(1040.), px(660.))
        } else {
            size(px(1440.), px(940.))
        };
        let bounds = Bounds::centered(None, window_size, cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1040.), px(660.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        settings::Preferences::load()
                            .language
                            .choose(
                                "Oicha · Không gian viết tiểu thuyết",
                                "Oicha · A space for your novel",
                            )
                            .into(),
                    ),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("dev.oicha.writer".into()),
                ..Default::default()
            },
            move |window, cx| {
                let entity = cx.new(|cx| Oicha::new(path, window, cx));
                if smoke {
                    if let Some(index) = args.iter().position(|arg| arg == "--update-fixture") {
                        let envelope = args
                            .get(index + 1)
                            .map(std::path::PathBuf::from)
                            .expect("--update-fixture requires signed metadata");
                        let installer_index = args
                            .iter()
                            .position(|arg| arg == "--update-installer")
                            .expect("--update-installer is required");
                        let installer = args
                            .get(installer_index + 1)
                            .map(std::path::PathBuf::from)
                            .expect("--update-installer requires a real setup file");
                        entity.update(cx, |app, cx| {
                            app.start_update_smoke(envelope, installer, window, cx)
                        });
                    } else if let Some(index) = args.iter().position(|arg| arg == "--preview-popup")
                    {
                        let kind = args
                            .get(index + 1)
                            .and_then(|s| s.to_str())
                            .unwrap_or("search");
                        entity.update(cx, |app, cx| app.preview_popup(kind, window, cx));
                    } else {
                        entity.update(cx, |app, cx| app.start_smoke(window, cx));
                    }
                }
                let weak = entity.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    weak.update(cx, |app, cx| {
                        app.request_close(window, cx);
                        false
                    })
                    .unwrap_or(true)
                });
                entity
            },
        );
        if let Err(error) = result {
            eprintln!("Cannot open Oicha window: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
