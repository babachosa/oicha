use crate::settings::{Language, Preferences, ui_format};
use crate::text_editor::{EditorEvent, TextEditor};
use gpui::{prelude::*, *};
use oicha_core::{
    document::{Manuscript, RichText},
    export,
    storage::{Store, atomic_write},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const INK: u32 = 0x303b35;
const MUTED: u32 = 0x8b918b;
const GREEN: u32 = 0x416b55;
const BORDER: u32 = 0xe6e8e0;
const TITLEBAR_HEIGHT: f32 = 42.;

#[derive(Clone)]
enum PromptKind {
    BookTitle,
    Author,
    Chapter(u64),
    Synopsis(u64),
    Goal,
    Search,
    Command,
}
struct Prompt {
    kind: PromptKind,
    title: String,
    input: Entity<TextEditor>,
    _subscription: Subscription,
}
enum ConfirmAction {
    DeleteChapter(u64),
    Export {
        book: Box<Manuscript>,
        path: PathBuf,
        text: bool,
    },
}
struct Confirmation {
    title: String,
    description: String,
    label: &'static str,
    action: Option<ConfirmAction>,
}
enum Pending {
    Close,
    Update,
    New,
    Open(PathBuf),
    Loaded(Box<LoadedBook>),
}

struct LoadedBook {
    store: Store,
    book: Manuscript,
    recovered: bool,
}

pub struct Oicha {
    language: Language,
    settings: bool,
    update_status: UpdateStatus,
    update_available: Option<oicha_update::Available>,
    update_ready: Option<oicha_update::Prepared>,
    update_progress: Arc<std::sync::atomic::AtomicU64>,
    book: Manuscript,
    editors: Vec<Entity<TextEditor>>,
    active: usize,
    store: Option<Arc<Mutex<Store>>>,
    path: Option<PathBuf>,
    saved_revision: u64,
    saving: bool,
    io_busy: bool,
    pending: Option<Pending>,
    status: String,
    error: Option<String>,
    notice: Option<String>,
    recovered: bool,
    focus_mode: bool,
    help: bool,
    prompt: Option<Prompt>,
    confirmation: Option<Confirmation>,
    modal_focus: FocusHandle,
    modal_epoch: u64,
    modal_started: Instant,
    modal_closing: bool,
    modal_exit_from: f32,
    modal_accept_selected: bool,
    query: String,
    font_size: f32,
    family: String,
    spacing: f32,
    vim: bool,
    session_start_words: usize,
    _subscriptions: Vec<Subscription>,
    _autosave: Task<()>,
}

enum UpdateStatus {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Ready,
    Failed(String),
}

fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "Oicha", "Oicha")
        .map(|p| p.data_local_dir().to_owned())
        .unwrap_or_else(|| std::env::temp_dir().join("oicha"))
}
fn remember(path: &Path) {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|s| s == "--smoke-test")
    {
        return;
    }
    if let Some(path) = path.to_str() {
        let _ = atomic_write(&data_dir().join("last-session"), path.as_bytes());
    }
}
fn scratch() -> anyhow::Result<(Store, PathBuf)> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = data_dir()
        .join("manuscripts")
        .join(format!("manuscript-{now}.oicha"));
    Ok((Store::create(path.clone())?, path))
}

async fn settle_editor(
    editor: &Entity<TextEditor>,
    cx: &mut AsyncWindowContext,
) -> anyhow::Result<()> {
    let mut stable = 0;
    for _ in 0..80 {
        cx.background_executor()
            .timer(Duration::from_millis(25))
            .await;
        let settled = cx.update(|_, cx| {
            let settled = editor.read(cx).motion_geometry_settled();
            editor.update(cx, |_, cx| cx.notify());
            settled
        })?;
        stable = if settled { stable + 1 } else { 0 };
        if stable == 2 {
            return Ok(());
        }
    }
    anyhow::bail!(
        "Editor motion/bounds did not settle: {}",
        cx.update(|_, cx| editor.read(cx).scroll_diagnostics())?
    )
}

impl Oicha {
    /// Isolated visual QA, available only under the explicit smoke-test flag.
    pub fn preview_popup(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) {
        match kind {
            "scroll-edges" => {
                self.editors[self.active].update(cx, |e, cx| {
                    e.engine = oicha_core::editor::Editor::new(RichText::plain(
                        "Một hàng tiếng Việt để kiểm tra cuộn trang.\n".repeat(100),
                    ));
                    e.reveal_cursor(cx);
                    cx.emit(EditorEvent::Changed);
                });
                let path = self
                    .path
                    .as_ref()
                    .unwrap()
                    .with_extension("scroll-probe.json");
                let command = path.with_extension("command.json");
                let editor = self.editors[self.active].clone();
                cx.spawn_in(window, async move |_, cx| {
                    let mut last = 0;
                    loop {
                        cx.background_executor().timer(Duration::from_millis(16)).await;
                        if cx.update(|window, cx| {
                            if let Ok(bytes) = std::fs::read(&command)
                                && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
                                && let Some(id) = value["id"].as_u64()
                                && id > last {
                                last = id;
                                editor.update(cx, |e, cx| {
                                    match value["action"].as_str().unwrap_or("") {
                                        "top" => e.scroll.set_offset(point(px(0.), px(0.))),
                                        "bottom" => e.scroll.set_offset(point(px(0.), -e.scroll.max_offset().height)),
                                        "partial" => e.scroll.set_offset(point(px(0.), px(-80.))),
                                        "first" => { e.engine.move_to(3, false); e.reveal_cursor(cx); }
                                        "second" => { e.engine.move_to(e.engine.body.text.find('\n').unwrap() + 1, false); e.reveal_cursor(cx); }
                                        "middle" => { e.engine.move_to(e.engine.body.text.find('\n').unwrap() + 1 + "Một hàng".len(), false); e.reveal_cursor(cx); }
                                        "wide" => { e.engine.move_to(30, false); e.reveal_cursor(cx); }
                                        "dollar" => { e.engine.key("$"); e.reveal_cursor(cx); }
                                        "cursor-fixture" => {
                                            let generation = e.engine.generation + 1;
                                            e.engine = oicha_core::editor::Editor::new(RichText::plain("A long first row with enough room to move vertically.\nA shorter row.\nAnother long row to return to the original horizontal position."));
                                            e.engine.generation = generation;
                                            e.reveal_cursor(cx);
                                            cx.emit(EditorEvent::Changed);
                                        }
                                        "input-fixture" => {
                                            let generation = e.engine.generation + 1;
                                            e.engine = oicha_core::editor::Editor::new(RichText::plain(value["text"].as_str().unwrap_or("")));
                                            e.engine.generation = generation;
                                            if value["vim"] == false { e.engine.toggle_vim(); }
                                            if value["insert"] == true && e.engine.mode != oicha_core::editor::Mode::Insert { e.engine.key("i"); }
                                            e.engine.move_to(value["cursor"].as_u64().unwrap_or(0) as usize, false);
                                            e.reveal_cursor(cx);
                                            cx.emit(EditorEvent::Changed);
                                        }
                                        "novim" => { if e.engine.vim { e.engine.toggle_vim(); } }
                                        "vim" => { if !e.engine.vim { e.engine.toggle_vim(); } e.engine.escape(); }
                                        "pixels-top" | "pixels-bottom" => {
                                            let delta = if value["action"] == "pixels-top" { 10_000. } else { -10_000. };
                                            let event = ScrollWheelEvent { position: e.scroll.bounds().center(), delta: ScrollDelta::Pixels(point(px(0.), px(delta))), ..Default::default() };
                                            e.scroll_wheel(&event, window, cx);
                                        }
                                        _ => {}
                                    }
                                    cx.notify();
                                });
                            }
                            let e = editor.read(cx);
                            if e.scroll.bounds().size.height <= px(0.) { return; }
                            let (middle, height) = e.cursor_viewport_geometry();
                            let (_, _, target, active) = e.motion_probe();
                            let bounds = e.scroll.bounds();
                            let point = e.cursor_screen_point();
                            let value = serde_json::json!({"command": last, "offset": f32::from(e.scroll.offset().y), "target": f32::from(target), "active": active, "max": f32::from(e.scroll.max_offset().height), "caret": f32::from(middle), "height": f32::from(height), "viewport": [f32::from(bounds.left()), f32::from(bounds.top()), f32::from(bounds.size.width), f32::from(bounds.size.height)], "generation": e.engine.generation, "cursor": e.engine.cursor, "anchor": e.engine.anchor, "mode": format!("{:?}", e.engine.mode), "text": e.engine.body.text, "x": f32::from(point.x), "y": f32::from(point.y)});
                            let _ = atomic_write(&path, &serde_json::to_vec(&value).unwrap());
                            let _ = window;
                        }).is_err() { break; }
                    }
                }).detach();
            }
            "settings" | "settings-en" => {
                if kind == "settings-en" {
                    self.set_language(Language::En, window, cx);
                }
                self.begin_modal();
                self.settings = true;
                self.modal_focus.focus(window);
            }
            "prose" => {
                self.editors[self.active].update(cx, |e, cx| {
                    let mut body = RichText::plain("chúng ta không thuộc về nhau, chúng ta không thuộc về nơi này niềm tin. Những niềm vui vẫn ở lại, cùng những câu chuyện chưa kể.\n\n".repeat(10));
                    for run in &mut body.runs { run.style.italic = true; }
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(body);
                    e.engine.generation = generation;
                    e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                });
            }
            "pages" | "pages-top" => {
                let editor = self.editors[self.active].clone();
                editor.update(cx, |e, cx| {
                    let mut body = e.engine.body.clone();
                    let end = body.text.len();
                    body.replace(end..end, &"\n\nMưa rơi ngoài hiên. Những câu chuyện vẫn tiếp tục trên trang giấy, cùng tiếng gió và ánh đèn khuya. 🌿".repeat(36), Default::default());
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(body);
                    e.engine.generation = generation;
                    e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                });
                if kind == "pages-top" {
                    return;
                }
                cx.spawn_in(window, async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let _ = cx.update(|_, cx| {
                        editor.update(cx, |e, cx| {
                            if let Some((_, next, _)) = e.page_boundary_probe() {
                                e.engine.move_to(next, false);
                                e.reveal_cursor(cx);
                            }
                        })
                    });
                    for key in ["z", "z"] {
                        let _ = cx.update(|window, cx| {
                            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx)
                        });
                    }
                })
                .detach();
            }
            "scroll" | "enter-page" => {
                let keys: &[&str] = if kind == "enter-page" {
                    &["G", "A", "enter"]
                } else {
                    &["G", "$", "z", "z"]
                };
                cx.spawn_in(window, async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    for key in keys {
                        let _ = cx.update(|window, cx| {
                            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx)
                        });
                    }
                })
                .detach();
            }
            "help" => self.toggle_help(window, cx),
            "delete" => self.delete_chapter(window, cx),
            "command" => self.show_prompt(PromptKind::Command, window, cx),
            "synopsis" => self.show_prompt(
                PromptKind::Synopsis(self.book.chapters[self.active].id),
                window,
                cx,
            ),
            _ => self.show_prompt(PromptKind::Search, window, cx),
        }
    }
    /// Exercises the real native window, dispatch tree, IME adapter, and save worker.
    /// Only entered with an explicit CLI flag and an isolated manuscript path.
    pub fn start_smoke(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let report = self
            .path
            .as_ref()
            .unwrap()
            .with_extension("smoke-result.txt");
        cx.spawn_in(window, async move |this, cx| {
            let mut close_gate = None;
            let result: anyhow::Result<()> = async {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                this.update_in(cx, |this, window, cx| {
                    let original = this.book.clone();
                    let language = this.language;
                    this.set_language(Language::En, window, cx);
                    assert_eq!(Preferences::load().language, Language::En);
                    assert_eq!(this.editors[this.active].read(cx).language, Language::En);
                    this.show_prompt(PromptKind::Search, window, cx);
                    assert_eq!(this.prompt.as_ref().unwrap().title, "Find in chapter");
                    assert_eq!(this.prompt.as_ref().unwrap().input.read(cx).language, Language::En);
                    this.begin_modal();
                    this.set_language(Language::Vi, window, cx);
                    assert_eq!(Preferences::load().language, Language::Vi);
                    this.set_language(language, window, cx);
                    assert_eq!(serde_json::to_vec(&this.book).unwrap(), serde_json::to_vec(&original).unwrap(), "Changing interface language changed the manuscript");
                    this.editors[this.active].read(cx).focus.focus(window);
                })?;
                cx.update(|window, _| crate::fonts::validate_vietnamese(window))??;
                cx.update(|window, _| TextEditor::validate_prose_wrap(window))??;
                let editor = this.update(cx, |this, _| this.editors[this.active].clone())?;
                let original = cx.update(|_, cx| editor.read(cx).engine.body.clone())?;
                cx.update(|window, cx| editor.update(cx, |e, cx| {
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(RichText::plain("🌿a\u{0301}b"));
                    e.engine.generation = generation; e.engine.toggle_vim();
                    e.replace_and_mark_text_in_range(Some(3..4), "\u{0300}", Some(1..1), window, cx);
                    assert_eq!(e.engine.body.text, "🌿a\u{0300}b");
                    assert_eq!(e.marked_text_range(window, cx), Some(3..4));
                    assert_eq!(e.selected_text_range(false, window, cx).unwrap().range, 4..4);
                    e.replace_text_in_range(None, "\u{0302}", window, cx);
                    assert_eq!(e.engine.body.text, "🌿a\u{0302}b");
                    e.history(false, cx); assert_eq!(e.engine.body.text, "🌿a\u{0301}b");
                }))?;
                let mut latency = vec![];
                for length in [2_000, 10_000, 40_000] {
                    let elapsed = cx.update(|window, cx| editor.update(cx, |e, _| e.measure_layout_edit(length, window)))?;
                    latency.push((length, elapsed));
                }
                atomic_write(&report.with_extension("layout-ms.json"), &serde_json::to_vec_pretty(&latency)?)?;
                let dispatch = |key: &str, window: &mut Window, cx: &mut App| {
                    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                };
                // A find target must bypass search/Ex shortcuts and counts,
                // while still travelling through the real key handler.
                cx.update(|_, cx| editor.update(cx, |e, cx| {
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(RichText::plain("a/b:n 0 a.b.c niềm"));
                    e.engine.generation = generation; e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                }))?;
                settle_editor(&editor, cx).await?;
                for (target, expected) in [("/", 1), (":", 3), ("n", 4), ("0", 6), ("ề", "a/b:n 0 a.b.c ni".len())] {
                    cx.update(|window, cx| { dispatch("f", window, cx); dispatch(target, window, cx); })?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.cursor == expected)?, "Native f target {target:?} was intercepted");
                    anyhow::ensure!(this.update(cx, |this, _| this.prompt.is_none())?, "Native f target {target:?} opened a prompt");
                }
                cx.update(|window, cx| { for key in ["F", ".", ",", ";", "2", "F", "."] { dispatch(key, window, cx); } })?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.cursor == 9)?, "Native counted/repeated find changed direction or partially moved on failure");
                cx.update(|window, cx| { for key in ["c", "f", ".", "X", "escape"] { dispatch(key, window, cx); } })?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "a/b:n 0 aXc niềm")?, "Native cf did not change through its target");
                cx.update(|window, cx| dispatch("u", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "a/b:n 0 a.b.c niềm")?, "Native cf was not one undo transaction");
                cx.update(|_, cx| editor.update(cx, |e, cx| {
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(RichText::plain("abcdef"));
                    e.engine.generation = generation; e.engine.move_to(2, false);
                    e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                }))?;
                cx.update(|window, cx| { dispatch("d", window, cx); dispatch("end", window, cx); })?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "ab")?, "GUI End interrupted a pending Vim operator");
                // Exercise the GUI history/clipboard routes, not only core Vim commands.
                let clipboard = cx.update(|_, cx| cx.read_from_clipboard())?;
                let visual_checks: anyhow::Result<()> = async {
                    for visual in ["v", "V"] {
                        for backwards in [false, true] {
                            for action in ["ctrl-z", "ctrl-x", "ctrl-v", "toolbar-undo"] {
                                cx.update(|_, cx| editor.update(cx, |e, cx| {
                                    let generation = e.engine.generation + 1;
                                    e.engine = oicha_core::editor::Editor::new(RichText::default());
                                    e.engine.generation = generation;
                                    e.engine.key("i"); e.engine.insert("a\u{0301}🌿 đời"); e.engine.escape();
                                    e.engine.move_to(if backwards { e.engine.previous(e.engine.body.text.len()) } else { 0 }, false);
                                    e.engine.key(visual); e.engine.key(if backwards { "h" } else { "l" });
                                    let before = (e.engine.body.clone(), e.engine.painted_selection(), e.engine.generation);
                                    e.cut_if_copied(false);
                                    assert_eq!((e.engine.body.clone(), e.engine.painted_selection(), e.engine.generation), before, "Failed clipboard copy must not cut text");
                                    cx.notify(); cx.emit(EditorEvent::Changed);
                                }))?;
                                let (body, range) = cx.update(|_, cx| { let e = editor.read(cx); (e.engine.body.clone(), e.engine.painted_selection()) })?;
                                let mut expected = body.text.clone();
                                match action {
                                    "ctrl-z" | "toolbar-undo" => expected.clear(),
                                    "ctrl-x" => { expected.replace_range(range, ""); },
                                    _ => { expected.replace_range(range, "ấ🌱"); }
                                }
                                for _ in 0..20 {
                                    if action == "ctrl-v" {
                                        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("ấ🌱".into())))?;
                                        cx.background_executor().timer(Duration::from_millis(25)).await;
                                        if !cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()).as_deref() == Some("ấ🌱"))? { continue; }
                                    }
                                    if action == "toolbar-undo" {
                                        this.update(cx, |this, cx| this.history(false, cx))?;
                                    } else { cx.update(|window, cx| dispatch(action, window, cx))?; }
                                    cx.background_executor().timer(Duration::from_millis(30)).await;
                                    let text = cx.update(|_, cx| editor.read(cx).engine.body.text.clone())?;
                                    if text == expected || text != body.text || !matches!(action, "ctrl-v" | "ctrl-x") { break; }
                                    // Windows clipboard access can be temporarily locked. Retry
                                    // only a no-op; never repeat an edit that changed the body.
                                    cx.background_executor().timer(Duration::from_millis(50)).await;
                                }
                                anyhow::ensure!(cx.update(|_, cx| { let e = editor.read(cx); e.engine.body.text == expected && e.engine.mode == oicha_core::editor::Mode::Normal && e.engine.painted_selection().is_empty() })?, "Visual GUI action failed: {visual}/{backwards}/{action}");
                                if action != "ctrl-z" && action != "toolbar-undo" {
                                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == body)?, "Undo did not restore Visual clipboard edit");
                                    cx.update(|window, cx| dispatch("ctrl-shift-z", window, cx))?;
                                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == expected)?, "Redo did not restore Visual clipboard edit");
                                } else {
                                    cx.update(|window, cx| dispatch("ctrl-shift-z", window, cx))?;
                                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == body)?, "Redo after Visual history did not restore the text");
                                }
                            }
                        }
                    }
                    Ok(())
                }.await;
                cx.update(|_, cx| cx.write_to_clipboard(clipboard.unwrap_or_else(|| ClipboardItem::new_string(String::new()))))?;
                visual_checks?;
                this.update(cx, |this, _| this.notice = None)?;
                for vim in [true, false] {
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let generation = e.engine.generation + 1;
                        e.engine = oicha_core::editor::Editor::new(RichText::plain("0123456789 ".repeat(30)));
                        e.engine.generation = generation;
                        if vim { e.engine.key("i"); } else { e.engine.toggle_vim(); }
                        e.engine.move_to(200, false); e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                    }))?;
                    cx.update(|window, cx| dispatch("home", window, cx))?;
                    let (home, y) = cx.update(|_, cx| { let e = editor.read(cx); (e.engine.cursor, e.cursor_screen_point().y) })?;
                    anyhow::ensure!(home > 0 && home < 200, "Home left the displayed row (vim={vim})");
                    cx.update(|window, cx| dispatch("shift-end", window, cx))?;
                    let end = cx.update(|_, cx| editor.read(cx).engine.cursor)?;
                    anyhow::ensure!(cx.update(|_, cx| { let e = editor.read(cx); e.engine.selection() == (home..end) && e.cursor_screen_point().y == y })?, "Shift+End selected outside its display row");
                    cx.update(|window, cx| dispatch("end", window, cx))?;
                    cx.update(|window, cx| dispatch("end", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.cursor == end)?, "Repeated End crossed a soft wrap");
                    cx.update(|window, cx| dispatch("shift-home", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| { let e = editor.read(cx); e.engine.selection() == (home..end) && e.cursor_screen_point().y == y })?, "Shift+Home selected outside its display row");
                }
                atomic_write(&report.with_extension("visual-gui.json"), &serde_json::to_vec_pretty(&serde_json::json!({"result":"PASS","visualClipboardHistoryCases":16,"homeEndModes":["vim-insert","vim-off"],"shiftSelection":true,"repeatedEnd":true}))?)?;
                for vim in [true, false] {
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let generation = e.engine.generation + 1;
                        e.engine = oicha_core::editor::Editor::new(RichText::plain("Gốc: "));
                        if !vim { e.engine.toggle_vim(); }
                        e.engine.generation = generation; e.engine.move_to(e.engine.body.text.len(), false);
                        if vim { e.engine.key("i"); }
                        e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                    }))?;
                    for c in "Mưa rơi".chars() { cx.update(|window, cx| dispatch(&c.to_string(), window, cx))?; }
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa ")?, "Ctrl+Z erased the whole typing session (vim={vim})");
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: ")?, "Second word undo failed (vim={vim})");
                    cx.update(|window, cx| dispatch("ctrl-shift-z", window, cx))?;
                    cx.update(|window, cx| dispatch("ctrl-shift-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi")?, "Word redo failed (vim={vim})");
                    cx.background_executor().timer(Duration::from_millis(1100)).await;
                    cx.update(|window, cx| dispatch("x", window, cx))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi")?, "Idle typing was merged into the preceding word");
                    cx.update(|window, cx| dispatch("enter", window, cx))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi")?, "Enter undo erased previous typing");
                    cx.update(|window, cx| dispatch("backspace", window, cx))?;
                    cx.update(|window, cx| dispatch("backspace", window, cx))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi")?, "Backspace undo did not restore deleted Vietnamese text");
                    cx.update(|window, cx| dispatch("space", window, cx))?;
                    cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_and_mark_text_in_range(None, "d", Some(1..1), window, cx)))?;
                    cx.background_executor().timer(Duration::from_millis(1100)).await;
                    cx.update(|window, cx| editor.update(cx, |e, cx| {
                        e.replace_and_mark_text_in_range(None, "đắ", Some(2..2), window, cx);
                        e.replace_text_in_range(None, "đắng", window, cx);
                    }))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi ")?, "IME undo left intermediate tone marks or erased previous words");
                    cx.update(|window, cx| dispatch("ctrl-shift-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi đắng")?, "IME redo did not restore the committed word");
                    cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, " — đoạn dán\nhàng mới", window, cx)))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi đắng")?, "Pasted text undo erased the preceding word");
                    cx.update(|window, cx| dispatch("ctrl-a", window, cx))?;
                    cx.update(|window, cx| dispatch("Z", window, cx))?;
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi đắng")?, "Selection replacement undo lost text");
                    cx.update(|_, cx| editor.update(cx, |e, cx| e.history(false, cx)))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi ")?, "Toolbar undo used different history grouping");
                    cx.update(|_, cx| editor.update(cx, |e, cx| e.history(true, cx)))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Gốc: Mưa rơi đắng")?, "Toolbar redo failed");
                }
                // Each update gets its own native frame, as with a Vietnamese input
                // method replacing a vowel while adding/removing its tone mark.
                for font_size in [16., 20., 28.] {
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let generation = e.engine.generation + 1;
                        e.engine = oicha_core::editor::Editor::new(RichText::plain("Mot doan co dinh.\nHang tiep theo."));
                        e.engine.generation = generation; e.engine.key("i"); e.font_size = font_size;
                        e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                    }))?;
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                    let mut initial = None;
                    for text in ["Toi", "Tôi", "Tối", "Tô\u{0301}i", "Toi", "Tối", "a", "ă", "ắ", "a\u{0306}\u{0301}", "a"] {
                        cx.update(|window, cx| editor.update(cx, |e, cx| {
                            let length = text.encode_utf16().count();
                            e.replace_and_mark_text_in_range(None, text, Some(length..length), window, cx);
                        }))?;
                        cx.background_executor().timer(Duration::from_millis(50)).await;
                        let geometry = cx.update(|_, cx| editor.read(cx).input_layout_geometry())?;
                        if let Some(expected) = initial {
                            anyhow::ensure!(geometry == expected, "Vietnamese composition moved rows at {font_size}px for {text:?}: {expected:?} -> {geometry:?}");
                        } else { initial = Some(geometry); }
                    }
                    cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, "ắ", window, cx)))?;
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).input_layout_geometry())? == initial.unwrap(), "Vietnamese commit shifted the baseline");
                    cx.update(|window, cx| dispatch("escape", window, cx))?;
                    cx.update(|window, cx| dispatch("u", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == "Mot doan co dinh.\nHang tiep theo.")?, "Vietnamese composition undo lost the original text");
                }
                // Row advances open a fresh writing area with Vim ON and OFF,
                // both at EOF and while inserting into existing paragraphs.
                let page_text = "Một hàng để viết tiếp câu chuyện.\n".repeat(70) + "Hàng cuối";
                for vim in [true, false] {
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let generation = e.engine.generation + 1;
                        e.engine = oicha_core::editor::Editor::new(RichText::plain(page_text.clone()));
                        e.engine.generation = generation;
                        if vim { e.engine.key("i"); } else { e.engine.toggle_vim(); }
                        e.engine.move_to(e.engine.body.text.len(), false);
                        e.font_size = 20.; e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let first_row = cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)?;
                    let before_pages = cx.update(|_, cx| editor.read(cx).page_metrics())?;
                    let mut wrapped = false;
                    for _ in 0..100 {
                        cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, "mưa ", window, cx)))?;
                        cx.background_executor().timer(Duration::from_millis(35)).await;
                        if cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)? > first_row {
                            wrapped = true; break;
                        }
                    }
                    anyhow::ensure!(wrapped, "Soft wrap fixture did not reach the next display row");
                    settle_editor(&editor, cx).await?;
                    let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                    anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "Soft wrap did not open the same writing area as Enter (vim={vim}): {middle:?}/{height:?}");
                    let after_pages = cx.update(|_, cx| editor.read(cx).page_metrics())?;
                    anyhow::ensure!(after_pages.1 == before_pages.1 && after_pages.0 <= before_pages.0 + 1, "Typing stretched a sheet instead of adding a fixed sheet");
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text.matches('\n').count())? == page_text.matches('\n').count(), "Soft wrapping inserted a real newline");
                    cx.update(|_, cx| editor.read(cx).validate_page_geometry())?;
                    if vim {
                        // Exercise smooth/precision input before the native delta.
                        // Real Windows dispatch is covered by native-scroll-smoke.ps1.
                        let before = cx.update(|_, cx| editor.read(cx).scroll.offset().y)?;
                        cx.update(|window, cx| {
                            let position = editor.read(cx).scroll.bounds().center();
                            let event = ScrollWheelEvent {
                                position, delta: ScrollDelta::Lines(point(0., 3.)), ..Default::default()
                            };
                            editor.update(cx, |e, cx| {
                                e.scroll_wheel(&event, window, cx);
                            });
                        })?;
                        anyhow::ensure!(cx.update(|_, cx| editor.read(cx).scroll.offset().y)? == before, "Wheel jumped before an animation frame");
                        cx.background_executor().timer(Duration::from_millis(45)).await;
                        let (current, target, active) = cx.update(|_, cx| {
                            let e = editor.read(cx); (e.scroll.offset().y, e.motion_probe().2, e.motion_probe().3)
                        })?;
                        anyhow::ensure!(active && current > before && current < target, "Wheel did not produce intermediate animation frames: {before:?}/{current:?}/{target:?}");
                        settle_editor(&editor, cx).await?;
                        anyhow::ensure!(cx.update(|_, cx| { let e = editor.read(cx); e.scroll.offset().y == target && !e.motion_probe().3 })?, "Wheel animation did not settle");
                        cx.update(|window, cx| {
                            let position = editor.read(cx).scroll.bounds().center();
                            let event = ScrollWheelEvent {
                                position, delta: ScrollDelta::Lines(point(0., -3.)), ..Default::default()
                            };
                            editor.update(cx, |e, cx| {
                                e.scroll_wheel(&event, window, cx);
                            });
                        })?;
                        cx.background_executor().timer(Duration::from_millis(30)).await;
                        cx.update(|window, cx| {
                            let position = editor.read(cx).scroll.bounds().center();
                            let event = ScrollWheelEvent {
                                position, delta: ScrollDelta::Pixels(point(px(0.), px(8.))), ..Default::default()
                            };
                            editor.update(cx, |e, cx| {
                                e.scroll_wheel(&event, window, cx);
                            });
                        })?;
                        let interrupted = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                        settle_editor(&editor, cx).await?;
                        anyhow::ensure!(cx.update(|_, cx| editor.read(cx).scroll.offset())? == interrupted, "Touchpad input was overridden by pending wheel motion");
                        cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, "W", window, cx)))?;
                        cx.background_executor().timer(Duration::from_millis(30)).await;
                        let (visual, logical, _, active) = cx.update(|_, cx| editor.read(cx).motion_probe())?;
                        anyhow::ensure!(active && visual.x < logical.x && visual.y == logical.y, "Typing caret did not glide toward the immediate logical caret: {visual:?}/{logical:?}");
                        settle_editor(&editor, cx).await?;
                        let (visual, logical, _, active) = cx.update(|_, cx| editor.read(cx).motion_probe())?;
                        anyhow::ensure!(visual == logical && !active, "Typing motion continued after settling");
                    }
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let generation = e.engine.generation + 1;
                        e.engine = oicha_core::editor::Editor::new(RichText::plain(page_text.clone()));
                        e.engine.generation = generation;
                        if vim { e.engine.key("i"); } else { e.engine.toggle_vim(); }
                        e.engine.move_to(e.engine.body.text.len(), false);
                        e.font_size = 20.; e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                    anyhow::ensure!(middle > height * 0.65, "EOF fixture was not in the lower writing area");
                    cx.update(|window, cx| dispatch("enter", window, cx))?;
                    // Input immediately after Enter must not erase its pending
                    // scroll request before the next native frame.
                    cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_and_mark_text_in_range(None, "đ", Some(1..1), window, cx)))?;
                    cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, "đ", window, cx)))?;
                    settle_editor(&editor, cx).await?;
                    let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                    anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "EOF Enter did not open a fresh writing area (vim={vim}): {middle:?}/{height:?}");
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == format!("{page_text}\nđ"))?, "Auto page scrolling inserted extra document text");
                    let offset = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                    let row_before = cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)?;
                    cx.update(|window, cx| dispatch("enter", window, cx))?;
                    settle_editor(&editor, cx).await?;
                    let after = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                    let row_after = cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)?;
                    if middle + row_after - row_before >= height * 0.65 {
                        // The next sheet's margin can put its first row below
                        // the threshold in a shorter window.
                        let (actual, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                        anyhow::ensure!((f32::from(actual - height * 0.32)).abs() < 2., "Enter across a page margin did not retain a fresh writing area");
                    } else {
                        anyhow::ensure!(after == offset, "Enter jumped again while ample writing space remained (vim={vim}): {offset:?} -> {after:?}; {}", cx.update(|_, cx| editor.read(cx).scroll_diagnostics())?);
                    }
                    cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == format!("{page_text}\nđ"))?, "Paragraph break undo changed more than one newline");
                    // A break near the beginning keeps its existing writing area.
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        e.engine.move_to(e.engine.body.text.find("\n").unwrap(), false);
                        e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let offset = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                    cx.update(|window, cx| dispatch("enter", window, cx))?;
                    settle_editor(&editor, cx).await?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).scroll.offset())? == offset, "Enter in existing text triggered an EOF page jump");
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let middle = e.engine.body.text.match_indices('\n').nth(40).unwrap().0;
                        e.engine.move_to(middle, false);
                        e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    cx.update(|window, cx| dispatch("enter", window, cx))?;
                    settle_editor(&editor, cx).await?;
                    let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                    anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "Enter inside existing text did not open a fresh writing area (vim={vim}): {middle:?}/{height:?}");
                    // Force a soft wrap inside a paragraph, with a suffix after
                    // the caret. Feed a burst before paint to cover fast typing.
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let at = e.engine.body.text.match_indices('\n').nth(45).unwrap().0;
                        e.engine.move_to(at, false);
                        e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let before = cx.update(|_, cx| editor.read(cx).engine.body.text.clone())?;
                    let at = cx.update(|_, cx| editor.read(cx).engine.cursor)?;
                    let input = "mưa ".repeat(18);
                    cx.update(|window, cx| editor.update(cx, |e, cx| {
                        for word in input.split_inclusive(' ') { e.replace_text_in_range(None, word, window, cx); }
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                    anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "Soft wrap inside existing text did not open a fresh writing area (vim={vim}): {middle:?}/{height:?}");
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == format!("{}{}{}", &before[..at], input, &before[at..]))?, "Writing-area scroll changed the paragraph suffix");
                    cx.update(|_, cx| editor.update(cx, |e, cx| {
                        let at = e.engine.body.text.match_indices('\n').nth(50).unwrap().0;
                        e.engine.move_to(at, false);
                        e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx);
                    }))?;
                    settle_editor(&editor, cx).await?;
                    let offset = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                    cx.update(|window, cx| { dispatch("enter", window, cx); dispatch("up", window, cx); })?;
                    settle_editor(&editor, cx).await?;
                    anyhow::ensure!(cx.update(|_, cx| editor.read(cx).scroll.offset())? == offset, "Navigation before paint retained a stale writing-area request (vim={vim}): before={offset:?}; {}", cx.update(|_, cx| editor.read(cx).scroll_diagnostics())?);
                }
                let wrapped = "Mùa gió thổi qua cửa sổ, trên con đường rất dài có những người vẫn đang bước đi. ".repeat(18);
                cx.update(|_, cx| editor.update(cx, |e, cx| {
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(RichText::plain(format!("{wrapped}\nx\n{wrapped}")));
                    e.engine.generation = generation; e.font_size = 20.; e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                }))?;
                cx.background_executor().timer(Duration::from_millis(200)).await;
                cx.update(|window, cx| dispatch("3", window, cx))?;
                cx.update(|window, cx| dispatch("l", window, cx))?;
                let point = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                let height = cx.update(|_, cx| { let e = editor.read(cx); px(e.font_size * e.spacing) })?;
                cx.update(|window, cx| dispatch("j", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).cursor_screen_point().y) ? == point.y + height, "j skipped a shaped display row");
                cx.update(|window, cx| dispatch("k", window, cx))?;
                let returned = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                anyhow::ensure!(returned == point, "k failed to restore desired horizontal position: {point:?} -> {returned:?}");
                let before_append = cx.update(|_, cx| editor.read(cx).engine.body.clone())?;
                cx.update(|window, cx| dispatch("A", window, cx))?;
                let (append_at, append_point, append_mode) = cx.update(|_, cx| {
                    let e = editor.read(cx); (e.engine.cursor, e.cursor_screen_point(), e.engine.mode)
                })?;
                anyhow::ensure!(append_mode == oicha_core::editor::Mode::Insert && append_point.y == point.y
                    && append_point.x > point.x && append_at < wrapped.len(), "A did not enter Insert at the current display-row end");
                cx.update(|window, cx| dispatch("ấ", window, cx))?;
                let mut expected = before_append.text.clone(); expected.insert(append_at, 'ấ');
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text == expected)?, "A inserted Vietnamese text at the wrong offset");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.update(|window, cx| dispatch("u", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == before_append)?, "Undo after A failed to restore wrapped text");
                settle_editor(&editor, cx).await?;
                let (previous, next, gap) = cx.update(|_, cx| editor.read(cx).page_boundary_probe().expect("multi-page fixture"))?;
                cx.update(|_, cx| editor.update(cx, |e, cx| { e.engine.move_to(previous, false); e.reveal_cursor(cx); }))?;
                let before_gap = cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)?;
                cx.update(|window, cx| dispatch("j", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.cursor)? == next, "j counted page margins as document rows");
                let after_gap = cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)?;
                anyhow::ensure!((f32::from(after_gap - before_gap - gap)).abs() < 1., "j did not cross the page gap using actual page coordinates: {before_gap:?} + {gap:?} -> {after_gap:?}");
                cx.update(|_, cx| editor.read(cx).validate_page_geometry())?;
                cx.update(|window, cx| dispatch("g", window, cx))?;
                cx.update(|window, cx| dispatch("g", window, cx))?;
                cx.update(|window, cx| dispatch("3", window, cx))?;
                cx.update(|window, cx| dispatch("l", window, cx))?;
                cx.update(|window, cx| dispatch("3", window, cx))?;
                cx.update(|window, cx| dispatch("j", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)? == point.y + height * 3., "3j did not move three display rows");
                let before_zero = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                cx.update(|window, cx| dispatch("0", window, cx))?;
                let (zero_at, zero_point) = cx.update(|_, cx| {
                    let e = editor.read(cx); (e.engine.cursor, e.cursor_screen_point())
                })?;
                anyhow::ensure!(zero_at > 0 && zero_point.x == px(0.) && zero_point.y == before_zero.y,
                    "0 moved to the paragraph start instead of the displayed row start: {before_zero:?} -> {zero_point:?}");
                cx.update(|window, cx| dispatch("0", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).cursor_screen_point())? == zero_point, "Repeated 0 left the current row");
                cx.update(|window, cx| dispatch("j", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).cursor_screen_point())? == gpui::point(px(0.), zero_point.y + height), "j after 0 retained the old horizontal position");
                // Exercise the same mapping after the font changes and wrapping is recalculated.
                cx.update(|_, cx| editor.update(cx, |e, cx| { e.font_size = 28.; cx.notify(); }))?;
                settle_editor(&editor, cx).await?;
                let point = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                cx.update(|window, cx| dispatch("j", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).cursor_screen_point().y)? == point.y + px(28. * 1.85), "j used stale wrapping after font change");
                let before_zero = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                cx.update(|window, cx| dispatch("0", window, cx))?;
                let zero_point = cx.update(|_, cx| editor.read(cx).cursor_screen_point())?;
                anyhow::ensure!(zero_point.x == px(0.) && zero_point.y == before_zero.y, "0 used stale wrapping after font change");
                cx.update(|window, cx| dispatch("G", window, cx))?;
                cx.update(|window, cx| dispatch("$", window, cx))?;
                let before_center = cx.update(|_, cx| {
                    let e = editor.read(cx); (e.engine.body.clone(), e.engine.cursor, e.engine.generation)
                })?;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height / 2.)).abs() < 2., "zz did not center the final display row: {middle:?}/{height:?}; {}", cx.update(|_, cx| editor.read(cx).scroll_diagnostics())?);
                cx.update(|_, cx| editor.update(cx, |e, cx| { e.scroll.scroll_to_bottom(); cx.notify(); }))?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!(middle < height * 0.4, "No scroll space below the last row: {middle:?}/{height:?}");
                let bottom_offset = cx.update(|_, cx| editor.read(cx).scroll.offset())?;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(80)).await;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).scroll.offset())? == bottom_offset, "First z scrolled before the command completed");
                cx.update(|window, cx| dispatch("z", window, cx))?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height / 2.)).abs() < 2., "zz did not return from the bottom scroll space");
                anyhow::ensure!(cx.update(|_, cx| { let e = editor.read(cx); (e.engine.body.clone(), e.engine.cursor, e.engine.generation) })? == before_center, "zz or end spacing changed manuscript content/cursor/history");
                cx.update(|window, _| window.resize(size(px(1040.), px(660.))))?;
                cx.background_executor().timer(Duration::from_millis(250)).await;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height / 2.)).abs() < 2., "zz did not center after resizing: {middle:?}/{height:?}");
                let tail = cx.update(|_, cx| editor.read(cx).scroll.bounds_for_item(1).unwrap().size.height)?;
                anyhow::ensure!((f32::from(tail - height * 0.65)).abs() < 2., "End scroll space did not follow resized viewport: {tail:?}/{height:?}");
                cx.update(|window, cx| { dispatch("2", window, cx); dispatch("z", window, cx); dispatch("z", window, cx); })?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height / 2.)).abs() < 2., "Counted zz lost its explicit center request when moving the caret");
                // The fresh-writing-area behavior also fits a small viewport
                // with large text, including Vim's o on the final logical line.
                cx.update(|_, cx| editor.update(cx, |e, cx| {
                    e.engine.key("G"); e.engine.key("$"); e.engine.key("A"); e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx);
                }))?;
                settle_editor(&editor, cx).await?;
                cx.update(|window, cx| dispatch("enter", window, cx))?;
                settle_editor(&editor, cx).await?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "EOF Enter failed in a small viewport with large text");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.update(|_, cx| editor.update(cx, |e, cx| { e.scroll.set_offset(gpui::point(px(0.), px(0.))); e.reveal_cursor(cx); }))?;
                settle_editor(&editor, cx).await?;
                cx.update(|window, cx| dispatch("o", window, cx))?;
                // A 120ms animation can start a frame after key dispatch, and
                // Windows may delay the frame applying its final bounds. Keep
                // the exact geometry assertion, with a bounded frame wait.
                let started = Instant::now();
                let mut samples = vec![];
                for _ in 0..40 {
                    cx.background_executor().timer(Duration::from_millis(25)).await;
                    let (middle, height, active) = cx.update(|_, cx| {
                        let e = editor.read(cx); let (middle, height) = e.cursor_viewport_geometry();
                        (f32::from(middle), f32::from(height), e.motion_probe().3)
                    })?;
                    samples.push((started.elapsed().as_millis() as u64, middle, height, active));
                    if (middle - height * 0.32).abs() < 2. && !active { break; }
                }
                atomic_write(&report.with_extension("vim-o-motion.json"), &serde_json::to_vec_pretty(&samples)?)?;
                let (middle, height) = cx.update(|_, cx| editor.read(cx).cursor_viewport_geometry())?;
                anyhow::ensure!((f32::from(middle - height * 0.32)).abs() < 2., "Vim o at EOF did not open the same writing area: {middle:?}/{height:?}; {}", cx.update(|_, cx| editor.read(cx).scroll_diagnostics())?);
                cx.update(|window, _| window.resize(size(px(1440.), px(940.))))?;
                cx.background_executor().timer(Duration::from_millis(200)).await;
                cx.update(|_, cx| editor.update(cx, |e, cx| {
                    let generation = e.engine.generation + 1;
                    e.engine = oicha_core::editor::Editor::new(original.clone());
                    e.engine.generation = generation; e.font_size = 20.; e.reveal_cursor(cx); cx.emit(EditorEvent::Changed);
                }))?;
                settle_editor(&editor, cx).await?;
                cx.update(|window, cx| dispatch("i", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.mode == oicha_core::editor::Mode::Insert)?, "Vim i did not enter Insert through native event dispatch");
                cx.update(|window, cx| dispatch("ấ", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text.starts_with('ấ'))?, "Platform text input did not insert Vietnamese character");
                cx.update(|window, cx| editor.update(cx, |e, cx| {
                    e.replace_and_mark_text_in_range(None, "m", Some(1..1), window, cx);
                    e.replace_and_mark_text_in_range(None, "mư", Some(2..2), window, cx);
                    e.replace_text_in_range(None, "mưa", window, cx);
                }))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text.starts_with("ấmưa"))?, "IME composition/commit lost text");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.update(|window, cx| dispatch("u", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == original)?, "IME composition was not one undo transaction");
                cx.update(|window, cx| dispatch("i", window, cx))?;
                let inserted = "Tiếng Việt 🌿 — bản thảo thử nghiệm.\n";
                cx.update(|window, cx| editor.update(cx, |e, cx| e.replace_text_in_range(None, inserted, window, cx)))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text.starts_with(inserted))?, "IME adapter lost Unicode text");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.update(|window, cx| dispatch("u", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == original)?, "Native Vim undo did not restore document");
                cx.update(|window, cx| dispatch("ctrl-r", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.text.starts_with(inserted))?, "Native Vim redo failed");
                cx.update(|window, cx| dispatch("g", window, cx))?;
                cx.update(|window, cx| dispatch("g", window, cx))?;
                cx.update(|window, cx| dispatch("V", window, cx))?;
                cx.update(|window, cx| dispatch("G", window, cx))?;
                cx.update(|window, cx| editor.update(cx, |e, cx| e.format('b', window, cx)))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body.runs.iter().all(|r| r.style.bold))?, "Formatting did not cover selection");
                cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                cx.update(|window, cx| dispatch("ctrl-z", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == original)?, "Undo failed after formatting");
                cx.update(|window, cx| dispatch("/", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(150)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.prompt.is_some())?, "Search shortcut did not reach root");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(150)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.prompt.is_none())?, "Escape did not dismiss search");
                cx.update(|window, cx| dispatch("f1", window, cx))?;
                let body = cx.update(|_, cx| editor.read(cx).engine.body.clone())?;
                cx.update(|window, cx| dispatch("x", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == body)?, "Help allowed edits to the manuscript");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                // Reopen before the fade-out completes: its stale timer must
                // neither dismiss the new popup nor steal the input's focus.
                this.update_in(cx, |this, window, cx| this.show_prompt(PromptKind::Search, window, cx))?;
                cx.background_executor().timer(Duration::from_millis(160)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.prompt.is_some() && !this.modal_closing && !this.help)?, "Stale help close dismissed a newer popup");
                cx.update(|window, cx| dispatch("z", window, cx))?;
                anyhow::ensure!(this.update(cx, |this, cx| this.prompt.as_ref().unwrap().input.read(cx).engine.body.text == "z")?, "Reopened prompt lost keyboard focus");
                cx.update(|window, cx| dispatch("escape", window, cx))?;
                cx.update(|window, cx| dispatch("x", window, cx))?;
                anyhow::ensure!(cx.update(|_, cx| editor.read(cx).engine.body == body)?, "Fade-out allowed edits behind the popup");
                cx.background_executor().timer(Duration::from_millis(160)).await;
                let count = this.update(cx, |this, _| this.book.chapters.len())?;
                this.update_in(cx, |this, window, cx| this.delete_chapter(window, cx))?;
                cx.update(|window, cx| dispatch("enter", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(160)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.book.chapters.len() == count && this.confirmation.is_none())?, "Confirmation Enter must default to cancel");
                this.update_in(cx, |this, window, cx| {
                    this.add_chapter(window, cx);
                    this.delete_chapter(window, cx);
                })?;
                cx.update(|window, cx| dispatch("tab", window, cx))?;
                cx.update(|window, cx| dispatch("enter", window, cx))?;
                cx.update(|window, cx| dispatch("enter", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(160)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.book.chapters.len() == count && this.confirmation.is_none())?, "Chapter confirmation was not applied exactly once");
                this.update_in(cx, |this, window, cx| this.activate_chapter(0, window, cx))?;
                this.update_in(cx, |this, window, cx| this.show_prompt(PromptKind::Synopsis(this.book.chapters[this.active].id), window, cx))?;
                cx.update(|window, cx| dispatch("z", window, cx))?;
                cx.update(|window, cx| dispatch("ctrl-enter", window, cx))?;
                cx.update(|window, cx| dispatch("ctrl-enter", window, cx))?;
                cx.background_executor().timer(Duration::from_millis(160)).await;
                anyhow::ensure!(this.update(cx, |this, _| this.prompt.is_none() && this.book.chapters[this.active].synopsis == "z")?, "Synopsis shortcut or duplicate submission failed");
                cx.update(|window, cx| dispatch("f11", window, cx))?;
                anyhow::ensure!(this.update(cx, |this, _| this.focus_mode)?, "Focus mode shortcut failed");
                cx.update(|window, cx| dispatch("f11", window, cx))?;
                cx.update(|window, cx| dispatch("ctrl-s", window, cx))?;
                for _ in 0..100 {
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                    if this.update(cx, |this, _| !this.saving)? { break; }
                }
                anyhow::ensure!(this.update(cx, |this, _| this.error.is_none() && this.saved_revision == this.book.revision)?, "Background save did not finish successfully");
                let (path, book) = this.update(cx, |this, _| (this.path.clone().unwrap(), this.book.clone()))?;
                export::export_docx(&book, &path.with_extension("docx"))?;
                book.validate()?;
                let target = path.with_extension("switch.oicha");
                let mut target_store = Store::create(target.clone())?;
                let mut target_book = Manuscript::new(); target_book.title = "Switch regression".into();
                target_store.save(&target_book)?; drop(target_store);
                let (sender, receiver) = std::sync::mpsc::channel();
                this.update_in(cx, |this, window, cx| this.load_with_gate(target.clone(), Some(receiver), window, cx))?;
                cx.update(|window, cx| editor.update(cx, |e, cx| {
                    e.engine.escape(); e.engine.key("i");
                    e.replace_text_in_range(None, "EDIT_DURING_LOAD", window, cx);
                }))?;
                sender.send(())?;
                for _ in 0..200 {
                    cx.background_executor().timer(Duration::from_millis(25)).await;
                    if this.update(cx, |this, _| !this.io_busy && !this.saving && this.pending.is_none())? { break; }
                }
                anyhow::ensure!(this.update(cx, |this, _| this.path.as_ref() == Some(&target) && this.book.title == "Switch regression")?, "Load transition did not finish");
                let stored_old: Manuscript = serde_json::from_slice(&std::fs::read(&path)?)?;
                anyhow::ensure!(stored_old.chapters[0].body.text.contains("EDIT_DURING_LOAD"), "Concurrent input was lost during load");
                anyhow::ensure!(this.update(cx, |this, _| !this.book.chapters[0].body.text.contains("EDIT_DURING_LOAD"))?, "Old input leaked into the destination book");
                let closing_path = path.with_extension("closing.oicha");
                if !std::env::args_os().any(|arg| arg == "--close-load-error") {
                    let mut closing_store = Store::create(closing_path.clone())?;
                    closing_store.save(&Manuscript::new())?;
                }
                let (sender, receiver) = std::sync::mpsc::channel();
                this.update_in(cx, |this, window, cx| {
                    this.load_with_gate(closing_path, Some(receiver), window, cx);
                    this.caption_action(WindowControlArea::Close, window, cx);
                    this.editors[this.active].update(cx, |e, cx| {
                        e.engine.key("i");
                        e.replace_text_in_range(None, "EDIT_DURING_CLOSE_LOAD", window, cx);
                    });
                })?;
                anyhow::ensure!(this.update(cx, |this, _| this.io_busy && matches!(this.pending, Some(Pending::Close)))?, "Close was not queued during load");
                close_gate = Some(sender);
                Ok(())
            }.await;
            let output = match result { Ok(()) => "PASS: whole-word Vietnamese NFC/NFD wrapping across styles, custom-caption close during I/O, embedded Literata Vietnamese shaping in four styles at 16/20/28px without fallback, native UTF16 accent-only replacement/marked range/selection/undo, indexed glyph lookup and layout benchmark, input preserved during deterministic load barrier, queued close during load (completion requires smoke-closed.txt), word/idle undo with Vim ON/OFF, atomic Vietnamese IME, toolbar history, A/0/j/k/wrapping/font reflow, fixed A4 display pages, page-margin hit testing, j across page boundaries, soft-wrap writing area with Vim ON/OFF, intermediate and settled wheel/caret motion, precision-scroll cancellation, zz final-row centering and interrupted prefix, end scroll space and window resizing without document changes, EOF Enter fresh writing area with Vim ON/OFF and immediate IME, middle-document Enter and burst soft-wrap writing area, stable same-row edits, newline undo and Vim o, Vietnamese baseline stability, formatting, search, modal focus/lifecycle/confirmation, background save, DOCX export.".to_owned(), Err(e) => format!("FAIL: {e:#}") };
            let _ = atomic_write(&report, output.as_bytes());
            let _ = this.update(cx, |this, cx| {
                this.editors[this.active].update(cx, |e, cx| { e.scroll.set_offset(gpui::point(px(0.), px(0.))); cx.notify(); });
                this.status = output; cx.notify();
            });
            if let Some(gate) = close_gate { let _ = gate.send(()); }
        }).detach();
    }
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language = crate::settings::Preferences::load().language;
        let remembered = std::fs::read_to_string(data_dir().join("last-session"))
            .ok()
            .map(PathBuf::from);
        let path = path.or(remembered);
        let initial = if let Some(path) = path {
            Store::open(path)
        } else {
            scratch().and_then(|(mut s, _)| {
                let b = Manuscript::demo();
                s.save(&b)?;
                Ok((s, b, false))
            })
        };
        let (book, store, path, recovered, error) = match initial {
            Ok((s, b, recovered)) => {
                let p = s.path.clone();
                remember(&p);
                (b, Some(Arc::new(Mutex::new(s))), Some(p), recovered, None)
            }
            Err(e) => (
                Manuscript::new(),
                None,
                None,
                false,
                Some(ui_format!(
                    language,
                    "Không mở được bản thảo: {e:#}. File gốc được giữ nguyên. Hãy Mở file khác hoặc Lưu thành bản mới.",
                    "Cannot open manuscript: {e:#}. The original file is preserved. Open another file or use Save as."
                )),
            ),
        };
        let start_words = book.words();
        let notice = store
            .as_ref()
            .and_then(|store| store.lock().ok()?.recovery_warning().map(str::to_owned));
        let saved_revision = if recovered {
            book.revision.saturating_sub(1)
        } else {
            book.revision
        };
        let timer = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if this
                    .update_in(cx, |this, window, cx| {
                        if this.book.revision != this.saved_revision
                            && !this.saving
                            && !this.io_busy
                            && this.error.is_none()
                        {
                            this.save(window, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            language,
            settings: false,
            update_status: UpdateStatus::Idle,
            update_available: None,
            update_ready: None,
            update_progress: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            book,
            editors: vec![],
            active: 0,
            store,
            path,
            saved_revision,
            saving: false,
            io_busy: false,
            pending: None,
            status: if recovered {
                language.text("Đã phục hồi bản thảo sau lần thoát trước")
            } else {
                language.text("Đã lưu trên máy")
            }
            .into(),
            error,
            notice,
            recovered,
            focus_mode: false,
            help: false,
            prompt: None,
            confirmation: None,
            modal_focus: cx.focus_handle(),
            modal_epoch: 0,
            modal_started: Instant::now(),
            modal_closing: false,
            modal_exit_from: 1.,
            modal_accept_selected: false,
            query: String::new(),
            font_size: 20.,
            family: crate::fonts::NOVEL_FAMILY.into(),
            spacing: 1.85,
            vim: true,
            session_start_words: start_words,
            _subscriptions: vec![],
            _autosave: timer,
        };
        this.build_editors(window, cx);
        if !std::env::args_os()
            .nth(1)
            .is_some_and(|s| s == "--smoke-test")
        {
            if let Ok(result) =
                std::fs::read_to_string(crate::settings::data_dir().join("update-result.txt"))
            {
                this.notice = Some(if result == "OK" {
                    language
                        .choose(
                            "Đã cập nhật Oicha thành công.",
                            "Oicha was updated successfully.",
                        )
                        .into()
                } else {
                    format!(
                        "{} {result}",
                        language.choose(
                            "Không thể hoàn tất cập nhật.",
                            "The update could not finish."
                        )
                    )
                });
                let _ = std::fs::remove_file(crate::settings::data_dir().join("update-result.txt"));
            }
            this.check_updates(window, cx);
        }
        this
    }

    fn build_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editors.clear();
        self._subscriptions.clear();
        for chapter in &self.book.chapters {
            let id = chapter.id;
            let body = chapter.body.clone();
            let entity = cx.new(|cx| {
                let mut e = TextEditor::new(body, false, window, cx);
                e.language = self.language;
                e.font_size = self.font_size;
                e.family = self.family.clone();
                e.spacing = self.spacing;
                if !self.vim {
                    e.engine.toggle_vim();
                }
                e
            });
            self._subscriptions.push(cx.subscribe_in(
                &entity,
                window,
                move |this, editor, event, window, cx| {
                    match event {
                        EditorEvent::Changed => {
                            if let Some(chapter) =
                                this.book.chapters.iter_mut().find(|c| c.id == id)
                            {
                                chapter.body = editor.read(cx).engine.body.clone();
                                this.book.revision += 1;
                                this.status = this.language.text("Có thay đổi chưa lưu").into();
                            }
                        }
                        EditorEvent::Command => this.show_prompt(PromptKind::Command, window, cx),
                        EditorEvent::Search => this.show_prompt(PromptKind::Search, window, cx),
                        EditorEvent::FindNext(back) => this.find_next(*back, cx),
                        EditorEvent::ClipboardUnavailable => this.clipboard_unavailable(cx),
                        _ => {}
                    }
                    cx.notify();
                },
            ));
            self.editors.push(entity);
        }
        self.active = self.active.min(self.editors.len() - 1);
        self.editors[self.active].read(cx).focus.focus(window);
    }

    fn dirty(&mut self, cx: &mut Context<Self>) {
        let language = self.language;
        self.book.revision += 1;
        self.status = language.text("Có thay đổi chưa lưu").into();
        cx.notify();
    }
    fn clipboard_unavailable(&mut self, cx: &mut Context<Self>) {
        self.notice = Some(self.language.choose(
            "Chưa truy cập được bảng nhớ tạm. Nội dung chưa bị cắt; hãy thử lại thao tác sao chép/cắt/dán.",
            "The clipboard is unavailable. No text was cut; please retry copy/cut/paste.",
        ).into());
        cx.notify();
    }
    fn activate_chapter(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.editors[self.active].update(cx, |e, _| e.engine.finish_group());
        self.active = index;
        self.editors[index].read(cx).focus.focus(window);
        cx.notify();
    }

    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending = Some(Pending::Close);
        self.drive_pending(window, cx);
    }

    fn drive_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.io_busy || self.saving || self.pending.is_none() {
            return;
        }
        // Flush the visible editor state even if its Changed event is still queued.
        for (chapter, editor) in self.book.chapters.iter_mut().zip(&self.editors) {
            let body = &editor.read(cx).engine.body;
            if chapter.body != *body {
                chapter.body = body.clone();
                self.book.revision += 1;
            }
        }
        if self.book.revision == self.saved_revision && self.store.is_some() {
            self.finish_pending(window, cx);
        } else {
            self.save(window, cx);
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        if self.saving || self.io_busy {
            return;
        }
        let Some(store) = self.store.clone() else {
            self.save_as(window, cx);
            return;
        };
        self.saving = true;
        self.status = language.text("Đang lưu…").into();
        let snapshot = self.book.clone();
        let revision = snapshot.revision;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut store = store
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
                    store.checkpoint(&snapshot)?;
                    store.save(&snapshot)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let language = this.language;
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.saved_revision = revision;
                        this.error = None;
                        this.recovered = false;
                        this.status = if this.book.revision == revision {
                            language.text("Đã lưu trên máy")
                        } else {
                            language.text("Có thay đổi chưa lưu")
                        }
                        .into();
                        this.drive_pending(window, cx);
                    }
                    Err(e) => {
                        this.error = Some(ui_format!(
                            language,
                            "Không lưu được: {e:#}",
                            "Could not save: {e:#}"
                        ));
                        this.status = language
                            .text("Lưu thất bại · hãy Lưu thành file khác")
                            .into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        if self.saving || self.io_busy {
            return;
        }
        self.io_busy = true;
        let directory = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .to_owned();
        let receiver = cx.prompt_for_new_path(
            &directory,
            Some(language.choose("Tieu-thuyet.oicha", "Novel.oicha")),
        );
        cx.spawn_in(window, async move |this, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(p))) => p,
                _ => {
                    let _ = this.update_in(cx, |this, _, cx| {
                        this.io_busy = false;
                        this.pending = None;
                        cx.notify();
                    });
                    return;
                }
            };
            let path = if path.extension().is_some_and(|e| e == "oicha") {
                path
            } else {
                path.with_extension("oicha")
            };
            let snapshot = this.update(cx, |this, _| this.book.clone()).ok();
            let Some(snapshot) = snapshot else {
                return;
            };
            let revision = snapshot.revision;
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut store = Store::create(path)?;
                    store.save(&snapshot)?;
                    remember(&store.path);
                    Ok::<_, anyhow::Error>(store)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let language = this.language;
                this.io_busy = false;
                match result {
                    Ok(store) => {
                        this.path = Some(store.path.clone());
                        this.store = Some(Arc::new(Mutex::new(store)));
                        this.saved_revision = revision;
                        this.error = None;
                        this.status = language.text("Đã lưu bản thảo mới").into();
                        this.drive_pending(window, cx);
                    }
                    Err(e) => {
                        this.error = Some(ui_format!(
                            language,
                            "Không lưu được bản mới: {e:#}",
                            "Could not save the new copy: {e:#}"
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        match self.pending.take() {
            Some(Pending::Close) => window.remove_window(),
            Some(Pending::Update) => {
                if let Some(update) = self.update_ready.take() {
                    match update.launch(
                        self.path.clone(),
                        crate::settings::data_dir().join("update-result.txt"),
                    ) {
                        Ok(()) => window.remove_window(),
                        Err(error) => {
                            self.update_status = UpdateStatus::Failed(format!("{error:#}"));
                            cx.notify();
                        }
                    }
                }
            }
            Some(Pending::New) => {
                let result = scratch().and_then(|(mut s, _)| {
                    let mut book = Manuscript::new();
                    if language == Language::En {
                        book.title = "Untitled novel".into();
                        book.chapters[0].title = "Chapter 1".into();
                    }
                    s.save(&book)?;
                    Ok((s, book))
                });
                match result {
                    Ok((s, book)) => {
                        self.path = Some(s.path.clone());
                        remember(&s.path);
                        self.store = Some(Arc::new(Mutex::new(s)));
                        self.book = book;
                        self.saved_revision = 0;
                        self.active = 0;
                        self.session_start_words = 0;
                        self.error = None;
                        self.notice = None;
                        self.build_editors(window, cx);
                    }
                    Err(e) => {
                        self.error = Some(ui_format!(
                            language,
                            "Không tạo được bản thảo: {e:#}",
                            "Could not create manuscript: {e:#}"
                        ))
                    }
                }
            }
            Some(Pending::Open(path)) => self.load(path, window, cx),
            Some(Pending::Loaded(loaded)) => self.install_loaded(*loaded, window, cx),
            None => {}
        }
    }

    fn new_book(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.io_busy {
            return;
        }
        self.pending = Some(Pending::New);
        self.save(window, cx);
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        if self.io_busy || self.saving {
            return;
        }
        self.io_busy = true;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                language
                    .text("Mở bản thảo .oicha, .txt hoặc .recovery")
                    .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.io_busy = false;
                if matches!(this.pending, Some(Pending::Close | Pending::Update)) {
                    this.drive_pending(window, cx);
                    cx.notify();
                    return;
                }
                if let Ok(Ok(Some(paths))) = result
                    && let Some(path) = paths.into_iter().next()
                {
                    if this.path.as_ref() == Some(&path) {
                        return;
                    }
                    this.pending = Some(Pending::Open(path));
                    this.drive_pending(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.load_with_gate(path, None, window, cx);
    }

    // Native smoke uses a deterministic barrier to exercise edits/close during I/O.
    // The production path always passes None, without a timer or artificial delay.
    fn load_with_gate(
        &mut self,
        path: PathBuf,
        gate: Option<std::sync::mpsc::Receiver<()>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self.language;
        self.io_busy = true;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if let Some(gate) = gate {
                        gate.recv()
                            .map_err(|error| anyhow::anyhow!("Load test barrier: {error}"))?;
                    }
                    match path.extension().and_then(|e| e.to_str()) {
                        Some("txt") => {
                            anyhow::ensure!(
                                std::fs::metadata(&path)?.len() <= 64 * 1024 * 1024,
                                "Text file exceeds 64 MiB"
                            );
                            let mut book = Manuscript::new();
                            if language == Language::En {
                                book.chapters[0].title = "Chapter 1".into();
                            }
                            book.title = path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into();
                            book.chapters[0].body = RichText::plain(
                                std::fs::read_to_string(&path)?
                                    .replace("\r\n", "\n")
                                    .replace('\r', "\n"),
                            );
                            let (mut store, _) = scratch()?;
                            store.save(&book)?;
                            Ok((store, book, false))
                        }
                        Some("recovery") => {
                            let book = Store::recover_copy(&path)?;
                            let (mut store, _) = scratch()?;
                            store.save(&book)?;
                            Ok((store, book, true))
                        }
                        _ => Store::open(path),
                    }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let language = this.language;
                this.io_busy = false;
                match result {
                    Ok((store, book, recovered)) => {
                        if !matches!(this.pending, Some(Pending::Close | Pending::Update)) {
                            this.pending = Some(Pending::Loaded(Box::new(LoadedBook {
                                store,
                                book,
                                recovered,
                            })));
                        }
                        this.drive_pending(window, cx);
                    }
                    Err(e) => {
                        this.error = Some(ui_format!(
                            language,
                            "Không mở được: {e:#}",
                            "Could not open: {e:#}"
                        ));
                        this.drive_pending(window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn install_loaded(&mut self, loaded: LoadedBook, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        let LoadedBook {
            store,
            book,
            recovered,
        } = loaded;
        self.notice = store.recovery_warning().map(str::to_owned);
        self.path = Some(store.path.clone());
        remember(&store.path);
        self.store = Some(Arc::new(Mutex::new(store)));
        self.saved_revision = if recovered {
            book.revision.saturating_sub(1)
        } else {
            book.revision
        };
        self.session_start_words = book.words();
        self.book = book;
        self.active = 0;
        self.recovered = recovered;
        self.error = None;
        self.prompt = None;
        self.query.clear();
        self.status = if recovered {
            language.text("Đã phục hồi bản thảo")
        } else {
            language.text("Đã mở bản thảo")
        }
        .into();
        self.build_editors(window, cx);
        cx.notify();
    }

    fn export(&mut self, text: bool, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        let snapshot = self.book.clone();
        let directory = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .to_owned();
        let extension = if text { "txt" } else { "docx" };
        let receiver = cx.prompt_for_new_path(
            &directory,
            Some(if text {
                language.choose("Tieu-thuyet.txt", "Novel.txt")
            } else {
                language.choose("Tieu-thuyet.docx", "Novel.docx")
            }),
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let converted = path.with_extension(extension);
            let _ = this.update_in(cx, |this, window, cx| {
                let language = this.language;
                // The native picker already confirms an unchanged path. A
                // changed extension can point at a different existing file.
                if converted != path && converted.exists() {
                    this.begin_modal();
                    this.confirmation = Some(Confirmation {
                        title: language.text("Thay thế file đã có?").into(),
                        description: converted.display().to_string(),
                        label: language.text("Ghi đè file"),
                        action: Some(ConfirmAction::Export {
                            book: Box::new(snapshot),
                            path: converted,
                            text,
                        }),
                    });
                    this.modal_focus.focus(window);
                    cx.notify();
                } else {
                    this.write_export(snapshot, converted, text, window, cx);
                }
            });
        })
        .detach();
    }

    fn write_export(
        &mut self,
        snapshot: Manuscript,
        path: PathBuf,
        text: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let display = path.display().to_string();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if text {
                        export::export_text(&snapshot, &path)
                    } else {
                        export::export_docx(&snapshot, &path)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let language = this.language;
                match result {
                    Ok(()) => {
                        this.status =
                            ui_format!(language, "Đã xuất: {display}", "Exported: {display}")
                    }
                    Err(e) => {
                        this.error = Some(ui_format!(
                            language,
                            "Xuất file thất bại: {e:#}",
                            "Export failed: {e:#}"
                        ))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // Each transition gets a new element ID. A delayed close can never clear a
    // newer popup, including one opened by a Vim command during submission.
    fn begin_modal(&mut self) {
        self.modal_epoch += 1;
        self.modal_started = Instant::now();
        self.modal_closing = false;
        self.modal_accept_selected = false;
        self.prompt = None;
        self.help = false;
        self.confirmation = None;
        self.settings = false;
    }

    fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_closing {
            return;
        }
        self.modal_exit_from =
            ease_out_quint()((self.modal_started.elapsed().as_secs_f32() / 0.2).min(1.));
        self.modal_epoch += 1;
        self.modal_closing = true;
        self.modal_focus.focus(window);
        let epoch = self.modal_epoch;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.modal_epoch != epoch {
                    return;
                }
                this.prompt = None;
                this.help = false;
                this.confirmation = None;
                this.settings = false;
                this.modal_closing = false;
                this.editors[this.active].read(cx).focus.focus(window);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn toggle_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.help {
            self.close_modal(window, cx);
        } else {
            self.begin_modal();
            self.help = true;
            self.editors[self.active].update(cx, |e, _| e.engine.finish_group());
            self.modal_focus.focus(window);
            cx.notify();
        }
    }

    fn show_prompt(&mut self, kind: PromptKind, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        self.begin_modal();
        let (title, value) = match &kind {
            PromptKind::BookTitle => (language.text("Tên tiểu thuyết"), self.book.title.clone()),
            PromptKind::Author => (language.text("Tên tác giả"), self.book.author.clone()),
            PromptKind::Chapter(id) => (
                language.text("Đổi tên chương"),
                self.book
                    .chapters
                    .iter()
                    .find(|c| c.id == *id)
                    .map(|c| c.title.clone())
                    .unwrap_or_default(),
            ),
            PromptKind::Synopsis(id) => (
                language.text("Ghi chú & tóm tắt chương"),
                self.book
                    .chapters
                    .iter()
                    .find(|c| c.id == *id)
                    .map(|c| c.synopsis.clone())
                    .unwrap_or_default(),
            ),
            PromptKind::Goal => (
                language.text("Mục tiêu từ mỗi phiên viết"),
                self.book.daily_goal.to_string(),
            ),
            PromptKind::Search => (language.text("Tìm trong chương"), self.query.clone()),
            PromptKind::Command => (language.text("Lệnh Vim"), String::new()),
        };
        self.editors[self.active].update(cx, |e, _| e.engine.finish_group());
        let input = cx.new(|cx| {
            let mut e = TextEditor::new(RichText::plain(value), true, window, cx);
            e.language = language;
            e.engine.select_all();
            e.multiline = matches!(kind, PromptKind::Synopsis(_));
            e
        });
        input.read(cx).focus.focus(window);
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                EditorEvent::Submit => this.submit_prompt(window, cx),
                EditorEvent::Cancel => this.cancel_prompt(window, cx),
                EditorEvent::ClipboardUnavailable => this.clipboard_unavailable(cx),
                _ => {}
            });
        self.prompt = Some(Prompt {
            kind,
            title: title.into(),
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn cancel_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_modal(window, cx);
    }
    fn submit_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        if self.modal_closing {
            return;
        }
        let Some(prompt) = self.prompt.as_ref() else {
            return;
        };
        let value = prompt.input.read(cx).engine.body.text.clone();
        let kind = prompt.kind.clone();
        self.close_modal(window, cx);
        match kind {
            PromptKind::BookTitle => {
                if !value.trim().is_empty() {
                    self.book.title = value.trim().into();
                    self.dirty(cx);
                }
            }
            PromptKind::Author => {
                self.book.author = value.trim().into();
                self.dirty(cx);
            }
            PromptKind::Chapter(id) => {
                if !value.trim().is_empty()
                    && let Some(index) = self.book.chapters.iter().position(|c| c.id == id)
                {
                    self.book.chapters[index].title = value.trim().into();
                    self.dirty(cx);
                }
            }
            PromptKind::Synopsis(id) => {
                if let Some(chapter) = self.book.chapters.iter_mut().find(|c| c.id == id) {
                    chapter.synopsis = value;
                    self.dirty(cx);
                }
            }
            PromptKind::Goal => match value.trim().parse::<usize>() {
                Ok(goal) if (1..=100_000).contains(&goal) => {
                    self.book.daily_goal = goal;
                    self.dirty(cx);
                }
                _ => {
                    self.error = Some(
                        language
                            .text("Mục tiêu phải là số từ 1 đến 100.000.")
                            .into(),
                    )
                }
            },
            PromptKind::Search => {
                self.query = value;
                self.find_next(false, cx);
            }
            PromptKind::Command => self.command(value.trim().trim_start_matches(':'), window, cx),
        }
        if self.modal_closing {
            self.modal_focus.focus(window);
        }
        cx.notify();
    }
    fn command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        match command {
            "w" => self.save(window, cx),
            "wa" | "saveas" => self.save_as(window, cx),
            "wq" | "x" => self.request_close(window, cx),
            "q" => {
                if self.book.revision == self.saved_revision {
                    self.request_close(window, cx);
                } else {
                    self.error = Some(
                        language
                            .text("Bản thảo chưa lưu. Dùng :wq để lưu và đóng.")
                            .into(),
                    );
                }
            }
            "new" => self.new_book(window, cx),
            "e" => self.open(window, cx),
            "export" => self.export(false, window, cx),
            "set novim" => {
                if self.vim {
                    self.toggle_vim(window, cx);
                }
            }
            "set vim" => {
                if !self.vim {
                    self.toggle_vim(window, cx);
                }
            }
            "help" => self.toggle_help(window, cx),
            _ => {
                if let Ok(line) = command.parse::<usize>() {
                    self.editors[self.active].update(cx, |e, cx| {
                        e.engine.move_to(0, false);
                        e.engine
                            .vertical(line.saturating_sub(1).min(1_000_000) as isize, false);
                        e.reveal_cursor(cx);
                    });
                } else {
                    self.error = Some(ui_format!(
                        language,
                        "Chưa hỗ trợ lệnh :{command}. Nhấn F1 để xem các lệnh có sẵn.",
                        "Command :{command} is not supported. Press F1 to see available commands."
                    ));
                }
            }
        }
    }
    fn find_next(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let language = self.language;
        let query = self.query.clone();
        let found = self.editors[self.active].update(cx, |e, cx| {
            let found = e.engine.find(&query, backwards);
            e.reveal_cursor(cx);
            found
        });
        self.status = if found {
            ui_format!(
                language,
                "Tìm thấy “{query}” · n / N để di chuyển",
                "Found “{query}” · n / N to navigate"
            )
        } else {
            ui_format!(
                language,
                "Không tìm thấy “{query}” trong chương",
                "“{query}” was not found in this chapter"
            )
        };
    }
    fn toggle_vim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.vim = !self.vim;
        for editor in &self.editors {
            editor.update(cx, |e, cx| {
                e.engine.toggle_vim();
                cx.notify();
            });
        }
        self.editors[self.active].read(cx).focus.focus(window);
        cx.notify();
    }
    fn add_chapter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        self.active = self.book.add_chapter();
        if language == Language::En {
            self.book.chapters[self.active].title = format!("Chapter {}", self.active + 1);
        }
        // Existing editor entities keep their undo histories when chapters are added.
        let chapter = &self.book.chapters[self.active];
        let id = chapter.id;
        let entity = cx.new(|cx| {
            let mut e = TextEditor::new(chapter.body.clone(), false, window, cx);
            e.language = language;
            e.font_size = self.font_size;
            e.family = self.family.clone();
            e.spacing = self.spacing;
            if !self.vim {
                e.engine.toggle_vim();
            }
            e
        });
        self._subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            move |this, editor, event, window, cx| {
                match event {
                    EditorEvent::Changed => {
                        if let Some(c) = this.book.chapters.iter_mut().find(|c| c.id == id) {
                            c.body = editor.read(cx).engine.body.clone();
                            this.dirty(cx);
                        }
                    }
                    EditorEvent::Command => this.show_prompt(PromptKind::Command, window, cx),
                    EditorEvent::Search => this.show_prompt(PromptKind::Search, window, cx),
                    EditorEvent::FindNext(back) => this.find_next(*back, cx),
                    EditorEvent::ClipboardUnavailable => this.clipboard_unavailable(cx),
                    _ => {}
                }
                cx.notify();
            },
        ));
        entity.read(cx).focus.focus(window);
        self.editors.push(entity);
        self.status = language.text("Đã thêm chương").into();
        cx.notify();
    }
    fn reorder(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.active as isize + delta;
        if next < 0 || next >= self.book.chapters.len() as isize {
            return;
        }
        self.book.chapters.swap(self.active, next as usize);
        self.editors.swap(self.active, next as usize);
        self.active = next as usize;
        self.dirty(cx);
    }
    fn delete_chapter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let language = self.language;
        if self.book.chapters.len() == 1 {
            self.error = Some(language.text("Bản thảo cần ít nhất một chương.").into());
            return;
        }
        let chapter = &self.book.chapters[self.active];
        let id = chapter.id;
        let description = ui_format!(
            language,
            "“{}” sẽ bị xóa khỏi bản thảo. File .bak giữ phiên bản lưu trước đó.",
            "“{}” will be deleted from the manuscript. The .bak file keeps the previous saved version.",
            chapter.title
        );
        self.begin_modal();
        self.confirmation = Some(Confirmation {
            title: language.text("Xóa chương này?").into(),
            description,
            label: language.text("Xóa chương"),
            action: Some(ConfirmAction::DeleteChapter(id)),
        });
        self.modal_focus.focus(window);
        cx.notify();
    }

    fn accept_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal_closing {
            return;
        }
        let action = self.confirmation.as_mut().and_then(|c| c.action.take());
        self.close_modal(window, cx);
        match action {
            Some(ConfirmAction::DeleteChapter(id)) => {
                if self.book.chapters.len() > 1
                    && let Some(index) = self.book.chapters.iter().position(|c| c.id == id)
                {
                    self.book.chapters.remove(index);
                    self.editors.remove(index);
                    self.active = self.active.min(self.editors.len() - 1);
                    self.dirty(cx);
                }
            }
            Some(ConfirmAction::Export { book, path, text }) => {
                self.write_export(*book, path, text, window, cx)
            }
            None => {}
        }
    }

    fn keyboard(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        let ctrl = k.modifiers.control || k.modifiers.platform;
        if self.modal_closing || self.help || self.settings || self.confirmation.is_some() {
            if !self.modal_closing {
                match k.key.as_str() {
                    "escape" => self.close_modal(window, cx),
                    "f1" if self.help => self.close_modal(window, cx),
                    "tab" if self.confirmation.is_some() => {
                        self.modal_accept_selected = !self.modal_accept_selected;
                        cx.notify();
                    }
                    "enter" if self.confirmation.is_some() => {
                        if self.modal_accept_selected {
                            self.accept_confirmation(window, cx);
                        } else {
                            self.close_modal(window, cx);
                        }
                    }
                    _ => {}
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if let Some(prompt) = &self.prompt {
            if k.key == "enter" && ctrl {
                self.submit_prompt(window, cx);
                window.prevent_default();
                cx.stop_propagation();
            } else if k.key == "tab" {
                prompt.input.read(cx).focus.focus(window);
                window.prevent_default();
                cx.stop_propagation();
            }
            // Input shortcuts belong to the input; document shortcuts cannot
            // replace a manuscript underneath a modal.
            return;
        }
        match k.key.as_str() {
            "s" if ctrl && k.modifiers.shift => self.save_as(window, cx),
            "s" if ctrl => self.save(window, cx),
            "o" if ctrl => self.open(window, cx),
            "n" if ctrl => self.new_book(window, cx),
            "f" if ctrl => {
                let engine = &self.editors[self.active].read(cx).engine;
                if engine.vim && engine.mode != oicha_core::editor::Mode::Insert {
                    return;
                }
                self.show_prompt(PromptKind::Search, window, cx);
            }
            "e" if ctrl && k.modifiers.shift => self.export(false, window, cx),
            "f1" => self.toggle_help(window, cx),
            "f11" => {
                self.focus_mode = !self.focus_mode;
                cx.notify();
            }
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(id)
            .px(px(12.))
            .py(px(7.))
            .rounded(px(6.))
            .text_size(px(12.))
            .cursor_pointer()
            .text_color(rgb(INK))
            .hover(|s| s.bg(rgb(0xeceee7)).text_color(rgb(INK)))
            .child(label.into())
    }

    fn caption_button(id: &'static str, area: WindowControlArea, maximized: bool) -> Stateful<Div> {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(area)
            .text_color(rgb(INK))
            .hover(move |s| {
                if area == WindowControlArea::Close {
                    s.bg(rgb(0xc94949)).text_color(rgb(0xffffff))
                } else {
                    s.bg(rgb(0xe8ece4))
                }
            })
            // Windows owns Min/Max, including restore and snap-layout hover.
            // Close is handled once by the application's save/queued-I/O guard.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if area == WindowControlArea::Close {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let color = window.text_style().color;
                        let origin = bounds.origin;
                        let mut path = PathBuilder::stroke(px(1.));
                        match area {
                            WindowControlArea::Min => {
                                path.move_to(origin + point(px(1.), px(6.)));
                                path.line_to(origin + point(px(11.), px(6.)));
                            }
                            WindowControlArea::Max => {
                                let rect = Bounds::new(
                                    origin + point(px(1.), px(1.)),
                                    size(px(10.), px(10.)),
                                );
                                if maximized {
                                    window.paint_quad(quad(
                                        Bounds::new(
                                            origin + point(px(3.), px(0.)),
                                            size(px(9.), px(9.)),
                                        ),
                                        px(0.),
                                        rgba(0),
                                        px(1.),
                                        color,
                                        BorderStyle::Solid,
                                    ));
                                }
                                window.paint_quad(quad(
                                    rect,
                                    px(0.),
                                    rgb(0xf6f8f2),
                                    px(1.),
                                    color,
                                    BorderStyle::Solid,
                                ));
                            }
                            WindowControlArea::Close => {
                                path.move_to(origin + point(px(1.), px(1.)));
                                path.line_to(origin + point(px(11.), px(11.)));
                                path.move_to(origin + point(px(11.), px(1.)));
                                path.line_to(origin + point(px(1.), px(11.)));
                            }
                            WindowControlArea::Drag => {}
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    },
                )
                .size(px(12.)),
            )
    }

    fn caption_action(
        &mut self,
        area: WindowControlArea,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match area {
            WindowControlArea::Min => window.minimize_window(),
            WindowControlArea::Max => window.zoom_window(),
            WindowControlArea::Close => self.request_close(window, cx),
            WindowControlArea::Drag => {}
        }
    }

    fn titlebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("app-titlebar")
            .h(px(TITLEBAR_HEIGHT))
            .pt(px(4.))
            .px(px(4.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(rgb(0xf6f8f2))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .id("window-drag-region")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .px(px(18.))
                    .gap(px(12.))
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .font_family(crate::fonts::NOVEL_FAMILY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(16.))
                            .text_color(rgb(GREEN))
                            .child("oicha."),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(rgb(BORDER)))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .child(self.book.title.clone()),
                    )
                    .when(self.book.revision != self.saved_revision, |d| {
                        d.child(div().text_size(px(10.)).text_color(rgb(GREEN)).child("●"))
                    }),
            )
            .child(
                Self::button(
                    "settings",
                    if self.update_available.is_some() {
                        self.language
                            .choose("Cài đặt · Có cập nhật", "Settings · Update available")
                    } else {
                        self.language.choose("Cài đặt", "Settings")
                    },
                )
                .mr(px(12.))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.begin_modal();
                    this.settings = true;
                    this.editors[this.active].update(cx, |e, _| e.engine.finish_group());
                    this.modal_focus.focus(window);
                    cx.notify();
                })),
            )
            .child(
                Self::caption_button("window-minimize", WindowControlArea::Min, false).when(
                    !cfg!(target_os = "windows"),
                    |button| {
                        button.on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.caption_action(WindowControlArea::Min, window, cx);
                        }))
                    },
                ),
            )
            .child(
                Self::caption_button(
                    "window-maximize",
                    WindowControlArea::Max,
                    window.is_maximized(),
                )
                .when(!cfg!(target_os = "windows"), |button| {
                    button.on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.caption_action(WindowControlArea::Max, window, cx);
                    }))
                }),
            )
            .child(
                Self::caption_button("window-close", WindowControlArea::Close, false).on_click(
                    cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.caption_action(WindowControlArea::Close, window, cx);
                    }),
                ),
            )
    }

    /// Explicit QA entry point: uses a real signed installer, isolated document,
    /// and the production helper/save/installation path. No feed overrides exist
    /// in normal app sessions.
    pub fn start_update_smoke(
        &mut self,
        envelope: PathBuf,
        installer: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let result: anyhow::Result<()> = async {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let update =
                    oicha_update::available(&std::fs::read(envelope)?, env!("CARGO_PKG_VERSION"))?
                        .ok_or_else(|| {
                            anyhow::anyhow!("QA target must be newer than the installed baseline")
                        })?;
                let ready = cx
                    .background_executor()
                    .spawn(async move {
                        oicha_update::stage(
                            update,
                            &crate::settings::data_dir().join("updates"),
                            std::fs::File::open(installer)?,
                            |_, _| {},
                        )
                    })
                    .await?;
                let (path, original) = this.update_in(cx, |this, window, cx| {
                    this.set_language(Language::En, window, cx);
                    this.update_available = Some(ready.available.clone());
                    this.update_ready = Some(ready);
                    this.update_status = UpdateStatus::Ready;
                    let path = this.path.clone().unwrap();
                    let original = std::fs::read(&path).unwrap();
                    this.editors[this.active].update(cx, |e, cx| {
                        e.engine.move_to(e.engine.body.text.len(), false);
                        e.engine
                            .insert("\nUPDATE_PRESERVE: Tiếng Việt 🌿 — unsaved text");
                        cx.emit(EditorEvent::Changed);
                    });
                    (path, original)
                })?;
                // A concurrent external edit must block the updater at the save
                // gate. Restore the fixture, then retry the exact same update.
                atomic_write(&path, b"external edit must block update")?;
                this.update_in(cx, |this, window, cx| this.install_update(window, cx))?;
                for _ in 0..100 {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    if this.update(cx, |this, _| !this.saving)? {
                        break;
                    }
                }
                anyhow::ensure!(
                    this.update(cx, |this, _| this.error.is_some()
                        && this.update_ready.is_some()
                        && matches!(this.pending, Some(Pending::Update)))?,
                    "Save failure did not block installation"
                );
                atomic_write(
                    &path.with_extension("update-save-gate.txt"),
                    b"PASS: failed save kept window and pending update open",
                )?;
                atomic_write(&path, &original)?;
                let loading = path.with_extension("update-target.oicha");
                let mut target_store = Store::create(loading.clone())?;
                target_store.save(&Manuscript::new())?;
                drop(target_store);
                this.update_in(cx, |this, window, cx| {
                    this.error = None;
                    this.load(loading, window, cx);
                    this.install_update(window, cx);
                })?;
                Ok(())
            }
            .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    if let Some(path) = &this.path {
                        let _ = atomic_write(
                            &path.with_extension("update-failure.txt"),
                            format!("{error:#}").as_bytes(),
                        );
                    }
                    this.error = Some(format!("Update QA: {error:#}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == language {
            return;
        }
        if let Err(error) = (Preferences { language }).save() {
            self.error = Some(format!(
                "{} {error:#}",
                self.language
                    .choose("Không lưu được cài đặt:", "Could not save settings:")
            ));
            cx.notify();
            return;
        }
        self.language = language;
        self.status = language
            .text(if self.saving {
                "Đang lưu…"
            } else if self.book.revision != self.saved_revision {
                "Có thay đổi chưa lưu"
            } else {
                "Đã lưu trên máy"
            })
            .into();
        for editor in &self.editors {
            editor.update(cx, |e, cx| {
                e.language = language;
                cx.notify();
            });
        }
        window.set_window_title(language.choose(
            "Oicha · Không gian viết tiểu thuyết",
            "Oicha · A space for your novel",
        ));
        cx.notify();
    }

    fn check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.update_status,
            UpdateStatus::Checking | UpdateStatus::Downloading | UpdateStatus::Ready
        ) {
            return;
        }
        self.update_status = UpdateStatus::Checking;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { oicha_update::check(env!("CARGO_PKG_VERSION")) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Some(update)) => {
                        this.update_available = Some(update);
                        this.update_status = UpdateStatus::Available;
                    }
                    Ok(None) => {
                        this.update_available = None;
                        this.update_status = UpdateStatus::Current;
                    }
                    Err(error) => this.update_status = UpdateStatus::Failed(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn download_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.update_status,
            UpdateStatus::Downloading | UpdateStatus::Ready
        ) {
            return;
        }
        let Some(available) = self.update_available.clone() else {
            return;
        };
        self.update_status = UpdateStatus::Downloading;
        self.update_progress
            .store(0, std::sync::atomic::Ordering::Relaxed);
        let progress = self.update_progress.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    oicha_update::download(
                        available,
                        &crate::settings::data_dir().join("updates"),
                        |done, total| {
                            progress.store(
                                done.saturating_mul(100) / total.max(1),
                                std::sync::atomic::Ordering::Relaxed,
                            );
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(update) => {
                        this.update_ready = Some(update);
                        this.update_status = UpdateStatus::Ready;
                    }
                    Err(error) => this.update_status = UpdateStatus::Failed(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let downloading = this.update(cx, |this, cx| {
                    if this.settings {
                        cx.notify();
                    }
                    matches!(this.update_status, UpdateStatus::Downloading)
                });
                if !matches!(downloading, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn install_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_ready.is_none() {
            return;
        }
        if let Err(error) = oicha_update::managed_installation().and_then(|target| {
            anyhow::ensure!(
                std::env::current_exe()?.canonicalize()?
                    == target.join("Oicha.exe").canonicalize()?,
                "Use the installed Oicha app to update"
            );
            Ok(())
        }) {
            self.update_status = UpdateStatus::Failed(format!("{error:#}"));
            cx.notify();
            return;
        }
        self.pending = Some(Pending::Update);
        self.drive_pending(window, cx);
    }

    fn settings_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let l = self.language;
        let version = self
            .update_available
            .as_ref()
            .map(|u| u.manifest.version.as_str())
            .unwrap_or("");
        let message = match &self.update_status {
            UpdateStatus::Idle => l
                .choose(
                    "Kiểm tra phiên bản mới trên GitHub Releases.",
                    "Check GitHub Releases for a new version.",
                )
                .into(),
            UpdateStatus::Checking => l
                .choose("Đang kiểm tra cập nhật…", "Checking for updates…")
                .into(),
            UpdateStatus::Current => l
                .choose(
                    "Bạn đang dùng phiên bản mới nhất đã phát hành.",
                    "You have the latest published version.",
                )
                .into(),
            UpdateStatus::Available => format!(
                "{} {version}",
                l.choose("Có phiên bản mới:", "New version available:")
            ),
            UpdateStatus::Downloading => format!(
                "{} {}%",
                l.choose("Đang tải…", "Downloading…"),
                self.update_progress
                    .load(std::sync::atomic::Ordering::Relaxed)
            ),
            UpdateStatus::Ready => l
                .choose(
                    "Đã xác minh gói cập nhật. Bản thảo sẽ được lưu trước khi khởi động lại.",
                    "Update verified. Your manuscript will be saved before restarting.",
                )
                .into(),
            UpdateStatus::Failed(error) => format!(
                "{}\n{error}",
                l.choose(
                    "Không thể cập nhật. Bạn có thể thử lại.",
                    "Could not update. You can try again."
                )
            ),
        };
        let busy = matches!(
            self.update_status,
            UpdateStatus::Checking | UpdateStatus::Downloading
        );
        let card = div()
            .w(px(570.))
            .p(px(30.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(Self::modal_heading(
                l.choose("Cài đặt", "Settings").into(),
                l.choose(
                    "Tùy chỉnh không gian viết của bạn.",
                    "Make your writing space your own.",
                ),
                "OICHA",
            ))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(l.choose("Ngôn ngữ giao diện", "Interface language")),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        Self::modal_button("language-vi", "Tiếng Việt", l == Language::Vi, false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language(Language::Vi, window, cx)
                            })),
                    )
                    .child(
                        Self::modal_button("language-en", "English", l == Language::En, false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_language(Language::En, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(l.choose(
                        "Áp dụng ngay. Nội dung bản thảo giữ nguyên.",
                        "Applies immediately. Your manuscript content stays as written.",
                    )),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .pt(px(18.))
                    .text_size(px(14.))
                    .child(format!("Oicha {}", env!("CARGO_PKG_VERSION"))),
            )
            .child(div().text_size(px(12.)).line_height(px(20.)).child(message))
            .when(self.update_available.is_some(), |d| {
                d.child(
                    div()
                        .id("release-notes")
                        .max_h(px(100.))
                        .overflow_y_scroll()
                        .text_size(px(12.))
                        .line_height(px(20.))
                        .text_color(rgb(MUTED))
                        .child(
                            self.update_available
                                .as_ref()
                                .unwrap()
                                .manifest
                                .notes
                                .clone(),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Self::modal_button("close-settings", l.text("Đóng"), false, false)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_modal(window, cx)),
                            ),
                    )
                    .child(
                        Self::modal_button(
                            "update-action",
                            if self.update_ready.is_some() {
                                l.choose("Khởi động lại để cập nhật", "Restart to update")
                            } else if self.update_available.is_some() {
                                l.choose("Tải cập nhật", "Download update")
                            } else {
                                l.choose("Kiểm tra cập nhật", "Check for updates")
                            },
                            true,
                            false,
                        )
                        .when(busy, |d| d.opacity(0.5))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if matches!(
                                this.update_status,
                                UpdateStatus::Checking | UpdateStatus::Downloading
                            ) {
                                return;
                            }
                            if this.update_ready.is_some() {
                                this.install_update(window, cx);
                            } else if this.update_available.is_some() {
                                this.download_update(window, cx);
                            } else {
                                this.check_updates(window, cx);
                            }
                        })),
                    ),
            );
        self.modal_shell(card, cx)
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let session_words = self.book.words().saturating_sub(self.session_start_words);
        let goal = self.book.daily_goal.max(1);
        div()
            .w(px(248.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf9faf5))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .px(px(22.))
                    .py(px(24.))
                    .child(
                        div()
                            .font_family(crate::fonts::NOVEL_FAMILY)
                            .text_size(px(23.))
                            .text_color(rgb(GREEN))
                            .child("oicha."),
                    )
                    .child(
                        div()
                            .mt(px(3.))
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(language.text("MỖI CÂU CHỮ, MỘT THẾ GIỚI")),
                    ),
            )
            .child(
                div()
                    .px(px(18.))
                    .py(px(18.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .mb(px(8.))
                            .child(language.text("BẢN THẢO")),
                    )
                    .child(
                        div()
                            .id("book-name")
                            .text_size(px(17.))
                            .text_color(rgb(INK))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .child(self.book.title.clone())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_prompt(PromptKind::BookTitle, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .id("author")
                            .mt(px(6.))
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .cursor_pointer()
                            .child(if self.book.author.is_empty() {
                                language.text("Thêm tên tác giả").into()
                            } else {
                                self.book.author.clone()
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_prompt(PromptKind::Author, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .mt(px(10.))
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(ui_format!(
                                language,
                                "{} chương  ·  {} từ",
                                "{} chapters  ·  {} words",
                                self.book.chapters.len(),
                                self.book.words()
                            )),
                    ),
            )
            .child(
                div()
                    .px(px(18.))
                    .pt(px(8.))
                    .pb(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(language.text("MỤC LỤC")),
                    )
                    .child(
                        Self::button("add-chapter", "+").on_click(
                            cx.listener(|this, _, window, cx| this.add_chapter(window, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .id("chapter-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(10.))
                    .children(self.book.chapters.iter().enumerate().map(|(i, c)| {
                        div()
                            .id(("chapter", c.id))
                            .px(px(12.))
                            .py(px(13.))
                            .mb(px(5.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .bg(rgb(if i == self.active { 0xe8eee3 } else { 0xf9faf5 }))
                            .hover(|s| s.bg(rgb(0xeef1e8)))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(9.))
                                    .items_start()
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(if i == self.active {
                                                GREEN
                                            } else {
                                                MUTED
                                            }))
                                            .pt(px(3.))
                                            .child(format!("{:02}", i + 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .text_color(rgb(INK))
                                                    .child(c.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .mt(px(4.))
                                                    .text_size(px(10.))
                                                    .text_color(rgb(MUTED))
                                                    .child(ui_format!(
                                                        language,
                                                        "{} từ",
                                                        "{} words",
                                                        c.body.words()
                                                    )),
                                            ),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate_chapter(i, window, cx)
                            }))
                    })),
            )
            .child(
                div()
                    .p(px(18.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(language.text("PHIÊN VIẾT NÀY"))
                            .child(ui_format!(
                                language,
                                "{} / {} từ",
                                "{} / {} words",
                                session_words,
                                goal
                            )),
                    )
                    .child(
                        div()
                            .h(px(4.))
                            .mt(px(12.))
                            .rounded(px(2.))
                            .bg(rgb(0xe4e8dc))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative((session_words as f32 / goal as f32).min(1.)))
                                    .bg(rgb(0x79916c))
                                    .rounded(px(2.)),
                            ),
                    )
                    .child(
                        Self::button("goal", language.text("Đặt mục tiêu"))
                            .mt(px(7.))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_prompt(PromptKind::Goal, window, cx)
                            })),
                    ),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let e = self.editors[self.active].read(cx);
        let style = e.engine.typing_style;
        div()
            .h(px(54.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px(px(22.))
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0xfbfdf8))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .child(
                        Self::button("font-family", self.family.clone()).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.family = if this.family == crate::fonts::NOVEL_FAMILY {
                                    "Times New Roman"
                                } else if this.family == "Times New Roman" {
                                    "Segoe UI"
                                } else {
                                    crate::fonts::NOVEL_FAMILY
                                }
                                .into();
                                for e in &this.editors {
                                    e.update(cx, |e, cx| {
                                        e.family = this.family.clone();
                                        cx.notify();
                                    });
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Self::button("smaller", "−")
                            .on_click(cx.listener(|this, _, _, cx| this.font_change(-1., cx))),
                    )
                    .child(
                        div()
                            .w(px(30.))
                            .text_center()
                            .text_size(px(12.))
                            .child(format!("{}", self.font_size as u32)),
                    )
                    .child(
                        Self::button("larger", "+")
                            .on_click(cx.listener(|this, _, _, cx| this.font_change(1., cx))),
                    )
                    .child(div().w(px(1.)).h(px(20.)).mx(px(8.)).bg(rgb(BORDER)))
                    .children(
                        [
                            ('b', "B", style.bold),
                            ('i', "I", style.italic),
                            ('u', "U", style.underline),
                        ]
                        .into_iter()
                        .map(|(flag, label, active)| {
                            Self::button(("format", flag as u32), label)
                                .font_family(crate::fonts::NOVEL_FAMILY)
                                .font_weight(FontWeight::BOLD)
                                .when(active, |b| b.bg(rgb(0xe1eadb)).text_color(rgb(GREEN)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.editors[this.active]
                                        .update(cx, |e, cx| e.format(flag, window, cx))
                                }))
                        }),
                    )
                    .child(div().w(px(1.)).h(px(20.)).mx(px(8.)).bg(rgb(BORDER)))
                    .child(
                        Self::button("line-spacing", format!("↕ {:.2}", self.spacing)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.spacing = if this.spacing > 1.9 {
                                    1.5
                                } else if this.spacing > 1.6 {
                                    2.0
                                } else {
                                    1.85
                                };
                                for e in &this.editors {
                                    e.update(cx, |e, cx| {
                                        e.spacing = this.spacing;
                                        cx.notify();
                                    });
                                }
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        Self::button("undo", "↶")
                            .on_click(cx.listener(|this, _, _, cx| this.history(false, cx))),
                    )
                    .child(
                        Self::button("redo", "↷")
                            .on_click(cx.listener(|this, _, _, cx| this.history(true, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(
                        Self::button("vim", if self.vim { "VIM  ON" } else { "VIM  OFF" })
                            .text_size(px(10.))
                            .bg(rgb(0xe8eee3))
                            .text_color(rgb(GREEN))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_vim(window, cx)),
                            ),
                    )
                    .child(
                        Self::button("focus", language.text("Tập trung  ↗")).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.focus_mode = !this.focus_mode;
                                cx.notify();
                            },
                        )),
                    ),
            )
    }
    fn font_change(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + delta).clamp(14., 32.);
        for e in &self.editors {
            e.update(cx, |e, cx| {
                e.font_size = self.font_size;
                cx.notify();
            });
        }
        cx.notify();
    }
    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        self.editors[self.active].update(cx, |e, cx| {
            e.history(redo, cx);
        });
    }

    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let chapter = &self.book.chapters[self.active];
        let id = chapter.id;
        let words = chapter.body.words();
        let chars = chapter.body.text.chars().count();
        div()
            .w(px(236.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0xf9faf5))
            .border_l_1()
            .border_color(rgb(BORDER))
            .p(px(22.))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .mb(px(22.))
                    .child(language.text("GÓC BẢN THẢO")),
            )
            .child(
                div()
                    .text_size(px(15.))
                    .text_color(rgb(INK))
                    .child(chapter.title.clone()),
            )
            .child(
                Self::button("rename-chapter", language.text("Đổi tên chương"))
                    .mt(px(7.))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_prompt(PromptKind::Chapter(id), window, cx)
                    })),
            )
            .child(
                div()
                    .mt(px(28.))
                    .mb(px(12.))
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(language.text("GHI CHÚ & TÓM TẮT")),
            )
            .child(
                div()
                    .id("synopsis")
                    .min_h(px(110.))
                    .p(px(12.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0xfdfefb))
                    .text_size(px(12.))
                    .line_height(px(22.))
                    .text_color(rgb(if chapter.synopsis.is_empty() {
                        MUTED
                    } else {
                        INK
                    }))
                    .cursor_pointer()
                    .child(if chapter.synopsis.is_empty() {
                        language
                            .text("Một chi tiết, một ý tưởng…\nNhấn để ghi lại.")
                            .into()
                    } else {
                        chapter.synopsis.clone()
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_prompt(PromptKind::Synopsis(id), window, cx)
                    })),
            )
            .child(
                div()
                    .mt(px(30.))
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(language.text("THỐNG KÊ CHƯƠNG")),
            )
            .children(
                [
                    (format!("{words}"), language.text("Từ")),
                    (format!("{chars}"), language.text("Ký tự")),
                    (
                        ui_format!(language, "{} phút", "{} min", words.div_ceil(200)),
                        language.text("Thời gian đọc"),
                    ),
                ]
                .into_iter()
                .map(|(value, label)| {
                    div()
                        .mt(px(14.))
                        .pb(px(12.))
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .flex()
                        .justify_between()
                        .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(label))
                        .child(div().text_size(px(13.)).text_color(rgb(INK)).child(value))
                }),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .gap(px(3.))
                    .child(
                        Self::button("chapter-up", "↑")
                            .on_click(cx.listener(|this, _, _, cx| this.reorder(-1, cx))),
                    )
                    .child(
                        Self::button("chapter-down", "↓")
                            .on_click(cx.listener(|this, _, _, cx| this.reorder(1, cx))),
                    )
                    .child(
                        Self::button("delete-chapter", language.text("Xóa chương"))
                            .text_color(rgb(0x976552))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.delete_chapter(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(42.))
                    .p(px(14.))
                    .rounded(px(6.))
                    .bg(rgb(0xeef1e7))
                    .child(
                        div()
                            .font_family(crate::fonts::NOVEL_FAMILY)
                            .text_size(px(13.))
                            .line_height(px(23.))
                            .text_color(rgb(0x737d67))
                            .child(language.text(
                                "“Cứ viết. Một trang chưa hoàn hảo vẫn hơn một trang trắng.”",
                            )),
                    ),
            )
    }

    fn modal_button(
        id: &'static str,
        label: &'static str,
        primary: bool,
        danger: bool,
    ) -> Stateful<Div> {
        let color = if danger { 0x9c4c40 } else { GREEN };
        div()
            .id(id)
            .px(px(18.))
            .py(px(11.))
            .rounded(px(10.))
            .text_size(px(13.))
            .cursor_pointer()
            .border_1()
            .border_color(rgb(if primary { color } else { BORDER }))
            .bg(rgb(if primary { color } else { 0xffffff }))
            .text_color(rgb(if primary { 0xffffff } else { INK }))
            .hover(move |s| {
                s.bg(rgb(if primary {
                    if danger { 0x873e34 } else { 0x345842 }
                } else {
                    0xf0f3ed
                }))
            })
            .active(|s| s.opacity(0.85))
            .child(label)
    }

    fn modal_heading(title: String, description: &'static str, badge: &'static str) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div().flex().child(
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(rgb(0xeaf1eb))
                        .text_color(rgb(GREEN))
                        .text_size(px(10.))
                        .child(badge),
                ),
            )
            .child(
                div()
                    .font_family(crate::fonts::NOVEL_FAMILY)
                    .text_size(px(26.))
                    .text_color(rgb(INK))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(21.))
                    .text_color(rgb(0x738077))
                    .child(description),
            )
    }

    fn modal_shell(&self, card: Div, cx: &mut Context<Self>) -> impl IntoElement {
        let closing = self.modal_closing;
        let from = self.modal_exit_from;
        let animation = Animation::new(Duration::from_millis(if closing { 120 } else { 200 }))
            .with_easing(ease_out_quint());
        let progress = move |delta: f32| if closing { from * (1. - delta) } else { delta };
        div()
            .id("modal-layer")
            .absolute()
            .occlude()
            .inset_0()
            .top(px(TITLEBAR_HEIGHT))
            .track_focus(&self.modal_focus)
            .flex()
            .items_center()
            .justify_center()
            .p(px(32.))
            .child(
                div()
                    .id("modal-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x182c235c))
                    .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx)))
                    .with_animation(
                        ElementId::NamedInteger("modal-backdrop-motion".into(), self.modal_epoch),
                        animation.clone(),
                        move |this, delta| this.opacity(progress(delta)),
                    ),
            )
            .child(
                card.relative()
                    .occlude()
                    .rounded(px(20.))
                    .bg(rgb(0xfdfefa))
                    .border_1()
                    .border_color(rgba(0xffffffd9))
                    .shadow(vec![
                        BoxShadow {
                            color: rgba(0x122b2224).into(),
                            offset: point(px(0.), px(18.)),
                            blur_radius: px(60.),
                            spread_radius: px(0.),
                        },
                        BoxShadow {
                            color: rgba(0x122b2214).into(),
                            offset: point(px(0.), px(3.)),
                            blur_radius: px(12.),
                            spread_radius: px(0.),
                        },
                    ])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .with_animation(
                        ElementId::NamedInteger("modal-card-motion".into(), self.modal_epoch),
                        animation,
                        move |this, delta| {
                            let p = progress(delta);
                            this.opacity(p).top(px(14. * (1. - p)))
                        },
                    ),
            )
    }

    fn prompt_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let prompt = self.prompt.as_ref().unwrap();
        let (badge, description, label) = match prompt.kind {
            PromptKind::BookTitle => (
                language.text("BẢN THẢO"),
                language.text("Đặt tên cho thế giới bạn đang viết."),
                language.text("Lưu tên"),
            ),
            PromptKind::Author => (
                language.text("BẢN THẢO"),
                language.text("Tên tác giả hiển thị cùng bản thảo khi xuất file."),
                language.text("Lưu thay đổi"),
            ),
            PromptKind::Chapter(_) => (
                language.text("CHƯƠNG"),
                language.text("Một cái tên để gợi mở câu chuyện phía trước."),
                language.text("Lưu tên"),
            ),
            PromptKind::Synopsis(_) => (
                language.text("GHI CHÚ"),
                language.text("Giữ ý tưởng, tình tiết và những điều cần nhớ ở đây."),
                language.text("Lưu ghi chú"),
            ),
            PromptKind::Goal => (
                language.text("NHỊP VIẾT"),
                language.text("Chọn mục tiêu từ 1 đến 100.000 từ cho mỗi phiên viết."),
                language.text("Lưu mục tiêu"),
            ),
            PromptKind::Search => (
                language.text("TÌM KIẾM"),
                language.text("Tìm từ hoặc cụm từ trong chương đang mở."),
                language.text("Tìm kiếm"),
            ),
            PromptKind::Command => (
                "VIM",
                language.text("Nhập lệnh rồi nhấn Enter để thực hiện."),
                language.text("Thực hiện"),
            ),
        };
        let multiline = matches!(prompt.kind, PromptKind::Synopsis(_));
        let card =
            div()
                .w(px(560.))
                .p(px(30.))
                .flex()
                .flex_col()
                .gap(px(24.))
                .child(Self::modal_heading(
                    prompt.title.clone(),
                    description,
                    badge,
                ))
                .child(
                    div()
                        .h(px(if multiline { 216. } else { 58. }))
                        .p(px(12.))
                        .border_1()
                        .rounded(px(10.))
                        .border_color(rgb(0x96b39f))
                        .bg(rgb(0xffffff))
                        .shadow(vec![BoxShadow {
                            color: rgba(0x416b5510).into(),
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: px(3.),
                        }])
                        .child(prompt.input.clone()),
                )
                .when(matches!(prompt.kind, PromptKind::Command), |d| {
                    d.child(
                        div().flex().flex_wrap().gap(px(6.)).children(
                            [":w", ":wq", ":e", ":new", ":export", ":help", ":42"]
                                .into_iter()
                                .map(|key| {
                                    div()
                                        .px(px(8.))
                                        .py(px(5.))
                                        .rounded(px(6.))
                                        .bg(rgb(0xf0f3ed))
                                        .text_color(rgb(GREEN))
                                        .text_size(px(11.))
                                        .child(key)
                                }),
                        ),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(div().text_size(px(11.)).text_color(rgb(0x738077)).child(
                            if multiline {
                                language.text("Ctrl+Enter để lưu · Esc để hủy")
                            } else {
                                language.text("Enter để xác nhận · Esc để hủy")
                            },
                        ))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(
                                    Self::modal_button(
                                        "cancel-prompt",
                                        language.text("Hủy"),
                                        false,
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.cancel_prompt(window, cx),
                                    )),
                                )
                                .child(
                                    Self::modal_button("submit-prompt", label, true, false)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.submit_prompt(window, cx)
                                        })),
                                ),
                        ),
                );
        self.modal_shell(card, cx)
    }

    fn confirmation_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let confirmation = self.confirmation.as_ref().unwrap();
        let card = div()
            .w(px(500.))
            .p(px(30.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(Self::modal_heading(
                confirmation.title.clone(),
                language.text("Thao tác này sẽ thay đổi dữ liệu đã có."),
                language.text("XÁC NHẬN"),
            ))
            .child(
                div()
                    .p(px(14.))
                    .rounded(px(10.))
                    .bg(rgb(0xf7f0e9))
                    .text_color(rgb(0x825c47))
                    .text_size(px(13.))
                    .line_height(px(22.))
                    .child(confirmation.description.clone()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0x738077))
                    .child(language.text("Tab để chọn · Enter để xác nhận · Esc để hủy")),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Self::modal_button(
                            "cancel-confirmation",
                            language.text("Hủy"),
                            false,
                            false,
                        )
                        .when(!self.modal_accept_selected, |d| d.border_color(rgb(GREEN)))
                        .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx))),
                    )
                    .child(
                        Self::modal_button("accept-confirmation", confirmation.label, true, true)
                            .when(self.modal_accept_selected, |d| {
                                d.border_color(rgb(INK)).border_2()
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.accept_confirmation(window, cx)
                            })),
                    ),
            );
        self.modal_shell(card, cx)
    }

    fn help_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let rows = [
            (
                "i / a / I / A",
                language.text("Insert tại con trỏ / sau ký tự / đầu / cuối dòng"),
            ),
            ("Esc", language.text("Trở về Normal, kết thúc nhóm undo")),
            (
                "h/l · j/k · gj/gk",
                language.text("Trái/phải · lên/xuống từng hàng hiển thị"),
            ),
            (
                "zz · 42zz",
                language.text("Đưa hàng có con trỏ vào giữa · đến dòng 42 rồi căn giữa"),
            ),
            (
                "f{char} / F{char} · ; / ,",
                language.text("Tìm ký tự tới/lùi trong dòng · lặp lại / đảo chiều"),
            ),
            (
                "2fa · dfa / cfa / yfa",
                language.text("Đến ký tự a thứ hai · xóa / thay / sao chép tới a"),
            ),
            (
                "gg / G · o / O",
                language.text("Đầu/cuối chương · tạo dòng dưới/trên"),
            ),
            (
                "v / V · d / y / c",
                language.text("Visual / Visual Line · xóa / sao chép / thay"),
            ),
            (
                "dd · yy · cc · 3dw",
                language.text("Thao tác dòng và lệnh có số lần lặp"),
            ),
            (
                "diw / daw / ciw",
                language.text("Thao tác với từ trong / cả khoảng trắng"),
            ),
            (
                "x X · D C · J · p P",
                language.text("Xóa ký tự / cuối dòng · nối dòng · dán"),
            ),
            (
                "u / Ctrl+R · / · n / N",
                language.text("Undo / redo · tìm · kết quả tiếp/trước"),
            ),
            (
                "Ctrl+S / Ctrl+Shift+S",
                language.text("Lưu / lưu thành file mới"),
            ),
            (
                "Ctrl+O / Ctrl+N · /",
                language.text("Mở / bản thảo mới · tìm trong chương"),
            ),
            (
                "Ctrl+B/F · Ctrl+U/D",
                language.text("Vim Normal: lên/xuống trang · nửa trang"),
            ),
            (
                "Ctrl+B / I / U",
                language.text("Định dạng trong Insert hoặc khi tắt Vim"),
            ),
            (
                "Ctrl+Shift+E · F11 · F1",
                language.text("Xuất DOCX · tập trung · hướng dẫn"),
            ),
        ];
        let card = div().w(px(740.)).p(px(30.)).flex().flex_col().gap(px(20.))
            .child(Self::modal_heading(language.text("Viết theo nhịp của bạn").into(), language.text("Các phím tắt để đôi tay theo kịp dòng suy nghĩ."), language.text("PHÍM TẮT")))
            .child(div().id("help-scroll").max_h(px(340.)).overflow_y_scroll()
                .children(rows.into_iter().enumerate().map(|(index, (key, explanation))|
                    div().flex().items_center().gap(px(16.)).px(px(12.)).py(px(10.)).rounded(px(8.))
                        .when(index % 2 == 0, |d| d.bg(rgb(0xf2f5ee)))
                        .child(div().w(px(236.)).flex_shrink_0().text_size(px(12.)).text_color(rgb(GREEN)).child(key))
                        .child(div().text_size(px(12.)).line_height(px(20.)).text_color(rgb(INK)).child(explanation)))))
            .child(div().flex().items_center().justify_between().gap(px(24.))
                .child(div().flex_1().text_size(px(11.)).line_height(px(18.)).text_color(rgb(0x738077))
                    .child(language.text("Cuộn để xem thêm. Bật/tắt bằng nút VIM. Chưa hỗ trợ macro, plugin và toàn bộ lệnh Ex.")))
                .child(Self::modal_button("close-help", language.text("Đóng · Esc"), true, false)
                    .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx)))));
        self.modal_shell(card, cx)
    }
}

impl Render for Oicha {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.language;
        let editor = &self.editors[self.active];
        let mode = editor.read(cx).engine.mode.label();
        let position = editor.read(cx).engine.position();
        let pending = editor.read(cx).engine.pending();
        window.set_window_title(&format!(
            "{}{} · Oicha",
            if self.saved_revision != self.book.revision {
                "● "
            } else {
                ""
            },
            self.book.title
        ));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family("Segoe UI")
            .text_color(rgb(INK))
            .text_size(px(13.))
            .bg(rgb(0xf9faf5))
            .capture_key_down(cx.listener(Self::keyboard))
            .child(self.titlebar(window, cx))
            .child(
                div()
                    .h(px(50.))
                    .flex_shrink_0()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px(px(20.))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.))
                            .child(
                                Self::button("new-book", language.text("Bản thảo mới")).on_click(
                                    cx.listener(|this, _, window, cx| this.new_book(window, cx)),
                                ),
                            )
                            .child(
                                Self::button("open", language.text("Mở")).on_click(
                                    cx.listener(|this, _, window, cx| this.open(window, cx)),
                                ),
                            )
                            .child(
                                Self::button("save", language.text("Lưu")).on_click(
                                    cx.listener(|this, _, window, cx| this.save(window, cx)),
                                ),
                            )
                            .child(
                                Self::button("save-as", language.text("Lưu thành…")).on_click(
                                    cx.listener(|this, _, window, cx| this.save_as(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(Self::button("find", language.text("Tìm kiếm")).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.show_prompt(PromptKind::Search, window, cx)
                                }),
                            ))
                            .child(Self::button("help", language.text("Phím tắt")).on_click(
                                cx.listener(|this, _, window, cx| this.toggle_help(window, cx)),
                            ))
                            .child(Self::button("export-txt", "TXT").on_click(
                                cx.listener(|this, _, window, cx| this.export(true, window, cx)),
                            ))
                            .child(
                                Self::button("export", language.text("Xuất DOCX  ↗"))
                                    .bg(rgb(GREEN))
                                    .text_color(rgb(0xffffff))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.export(false, window, cx)
                                    })),
                            ),
                    ),
            )
            .when(self.error.is_some() || self.notice.is_some(), |d| {
                d.child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .px(px(20.))
                        .py(px(10.))
                        .bg(rgb(0xfbece5))
                        .text_size(px(12.))
                        .text_color(rgb(0x97503c))
                        .child(
                            self.error
                                .clone()
                                .or_else(|| self.notice.clone())
                                .unwrap_or_default(),
                        )
                        .child(
                            Self::button("dismiss-error", language.text("Đóng")).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    this.notice = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(!self.focus_mode, |d| d.child(self.sidebar(cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(self.toolbar(cx))
                            .child(self.editors[self.active].clone()),
                    )
                    .when(!self.focus_mode, |d| d.child(self.inspector(cx))),
            )
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px(px(16.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(0xf8faf4))
                    .text_size(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .px(px(7.))
                                    .py(px(3.))
                                    .rounded(px(3.))
                                    .bg(rgb(GREEN))
                                    .text_color(rgb(0xffffff))
                                    .child(if self.vim {
                                        mode
                                    } else {
                                        language.text("SOẠN THẢO")
                                    }),
                            )
                            .child(div().text_color(rgb(MUTED)).child(ui_format!(
                                language,
                                "Dòng {} · Cột {}  {}",
                                "Line {} · Column {}  {}",
                                position.0,
                                position.1,
                                pending
                            )))
                            .child(div().text_color(rgb(MUTED)).child(self.status.clone())),
                    )
                    .child(div().text_color(rgb(MUTED)).child(ui_format!(
                        language,
                        "{} từ  ·  UTF-8  ·  GPUI",
                        "{} words  ·  UTF-8  ·  GPUI",
                        self.book.words()
                    ))),
            )
            .when(self.help, |d| d.child(self.help_overlay(cx)))
            .when(self.settings, |d| d.child(self.settings_overlay(cx)))
            .when(self.prompt.is_some(), |d| d.child(self.prompt_overlay(cx)))
            .when(self.confirmation.is_some(), |d| {
                d.child(self.confirmation_overlay(cx))
            })
    }
}
