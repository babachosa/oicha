use crate::{
    motion::Motion,
    pagination::{FIRST_TEXT_TOP, MARGIN, PageGeometry, Paginator},
};
use gpui::prelude::*;
use gpui::*;
use oicha_core::{
    document::RichText,
    editor::{CaretAffinity, DisplayRow, Editor, Effect, InputEditKind, Mode},
};
use std::{
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug)]
pub enum EditorEvent {
    Changed,
    State,
    Command,
    Search,
    Submit,
    Cancel,
    FindNext(bool),
}

#[derive(Clone)]
struct ParagraphLayout {
    line: WrappedLine,
    start: usize,
    y: Pixels,
    rows: Arc<Vec<Range<usize>>>,
    positions: Arc<Vec<(usize, Pixels)>>,
    glyph_rows: Arc<Vec<Vec<RowGlyph>>>,
    row_start: usize,
}

#[derive(Clone)]
struct RowGlyph {
    index: usize,
    font: FontId,
    glyph: GlyphId,
    x: Pixels,
    emoji: bool,
}

#[derive(Clone)]
struct RowLocation {
    paragraph: usize,
    row: usize,
    y: Pixels,
}

fn row_glyphs(line: &WrappedLine) -> Vec<Vec<RowGlyph>> {
    let mut rows = vec![vec![]];
    let mut wraps = line.wrap_boundaries.iter().peekable();
    let mut base = px(0.);
    for (run_ix, run) in line.unwrapped_layout.runs.iter().enumerate() {
        for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
            if wraps
                .peek()
                .is_some_and(|w| w.run_ix == run_ix && w.glyph_ix == glyph_ix)
            {
                wraps.next();
                rows.push(vec![]);
                base = glyph.position.x;
            }
            rows.last_mut().unwrap().push(RowGlyph {
                index: glyph.index,
                font: run.font_id,
                glyph: glyph.id,
                x: glyph.position.x - base,
                emoji: glyph.is_emoji,
            });
        }
    }
    rows
}

impl ParagraphLayout {
    fn x_for_index(&self, index: usize) -> Pixels {
        self.positions
            .get(
                self.positions
                    .partition_point(|(offset, _)| *offset < index),
            )
            .map_or(self.line.unwrapped_layout.width, |(_, x)| *x)
    }
    fn paint_origin(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        baseline: Pixels,
    ) -> Point<Pixels> {
        // DirectWrite may choose a taller fallback font for a Vietnamese tone mark.
        // Keep the baseline fixed instead of recentering every row of the paragraph.
        let shaped_baseline = (line_height + self.line.unwrapped_layout.ascent
            - self.line.unwrapped_layout.descent)
            / 2.;
        origin + point(px(0.), self.y + baseline - shaped_baseline)
    }
}

// Preserve GPUI's first-glyph-with-index>=query semantics, including repeated
// cluster indices and non-monotone glyph order. Prefix record indices are sorted.
fn indexed_positions(
    glyphs: impl Iterator<Item = (usize, Pixels)>,
    len: usize,
    width: Pixels,
) -> Vec<(usize, Pixels)> {
    let mut positions = vec![];
    for (index, x) in glyphs {
        if positions
            .last()
            .is_none_or(|(previous, _)| index > *previous)
        {
            positions.push((index, x));
        }
    }
    positions.push((len, width));
    positions
}

fn wrap_prose(line: &mut WrappedLine, positions: &[(usize, Pixels)], width: Pixels) {
    let stops: Vec<_> = line
        .text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([line.len()])
        .filter_map(|i| {
            positions
                .binary_search_by_key(&i, |(index, _)| *index)
                .ok()
                .map(|index| (i, f32::from(positions[index].1)))
        })
        .collect();
    let breaks = crate::word_wrap::breaks(&line.text, &stops, width.into());
    let mut boundaries = Vec::with_capacity(breaks.len());
    let mut previous_index = None;
    for (run_ix, run) in line.unwrapped_layout.runs.iter().enumerate() {
        for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
            if previous_index != Some(glyph.index) && breaks.binary_search(&glyph.index).is_ok() {
                boundaries.push(WrapBoundary { run_ix, glyph_ix });
            }
            previous_index = Some(glyph.index);
        }
    }
    **line = Arc::new(WrappedLineLayout {
        unwrapped_layout: line.unwrapped_layout.clone(),
        wrap_boundaries: boundaries.into_iter().collect(),
        wrap_width: Some(width),
    });
}

#[derive(Clone)]
struct Layout {
    paragraphs: Vec<ParagraphLayout>,
    height: Pixels,
    width: Pixels,
    generation: u64,
    font_size: f32,
    family: String,
    spacing: f32,
    baseline: Pixels,
    rows: Arc<Vec<RowLocation>>,
    pages: usize,
    page: Option<PageGeometry>,
}

pub struct TextEditor {
    pub language: crate::settings::Language,
    pub engine: Editor,
    pub focus: FocusHandle,
    pub font_size: f32,
    pub family: String,
    pub spacing: f32,
    pub compact: bool,
    pub multiline: bool,
    pub scroll: ScrollHandle,
    layout: Option<Layout>,
    motion_layout_key: Option<(Pixels, u64, u32, u32, String)>,
    bounds: Option<Bounds<Pixels>>,
    marked: Option<Range<usize>>,
    input_clock: Instant,
    selecting: bool,
    blink: bool,
    autoscroll: bool,
    center_cursor: bool,
    advance_page: bool,
    follow_edit: bool,
    last_caret_row: Option<Pixels>,
    viewport_height: Pixels,
    viewport_width: Pixels,
    scroll_motion: Option<Motion>,
    scroll_written: Option<Pixels>,
    caret_motion: Option<(Motion, Motion)>,
    visual_caret: Point<Pixels>,
    visual_layout_key: Option<(Pixels, u32, u32, String)>,
    input_flash: Option<(Range<usize>, u64, Instant)>,
    _blink_task: Task<()>,
    _blur: Subscription,
}

impl EventEmitter<EditorEvent> for TextEditor {}
impl Focusable for TextEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextEditor {
    pub fn validate_prose_wrap(window: &mut Window) -> anyhow::Result<()> {
        for word in ["niềm", "nie\u{0302}\u{0300}m"] {
            let text: SharedString =
                format!("chúng ta không thuộc về nhau, chúng ta không thuộc về nơi này {word} tin")
                    .into();
            let start = text.find(word).unwrap();
            let end = start + word.len();
            for face in [
                font(crate::fonts::NOVEL_FAMILY),
                font(crate::fonts::NOVEL_FAMILY).italic(),
                font(crate::fonts::NOVEL_FAMILY).bold(),
            ] {
                let mut lines = window.text_system().shape_text(
                    text.clone(),
                    px(20.),
                    &[
                        TextRun {
                            len: start + 2,
                            font: face.clone(),
                            color: rgb(0x302f2c).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        },
                        TextRun {
                            len: text.len() - start - 2,
                            font: face.bold().italic(),
                            color: rgb(0x302f2c).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        },
                    ],
                    None,
                    None,
                )?;
                let line = &mut lines[0];
                let positions = indexed_positions(
                    line.unwrapped_layout
                        .runs
                        .iter()
                        .flat_map(|run| run.glyphs.iter())
                        .map(|g| (g.index, g.position.x)),
                    line.len(),
                    line.unwrapped_layout.width,
                );
                let at = |index| positions[positions.partition_point(|(i, _)| *i < index)].1;
                let width = at(start) + (at(end) - at(start)) / 2.;
                wrap_prose(line, &positions, width);
                let rows = Self::wrap_ranges(line);
                anyhow::ensure!(
                    rows.iter().any(|row| row.start == start),
                    "Vietnamese word did not move intact to the next display row: {word}"
                );
                anyhow::ensure!(
                    rows.iter()
                        .all(|row| row.start <= start || row.start >= end),
                    "Wrap split a Vietnamese word across font/style runs: {word}"
                );
                anyhow::ensure!(
                    rows.iter()
                        .all(|row| text.grapheme_indices(true).any(|(i, _)| i == row.start)),
                    "Wrap split a composed grapheme"
                );
            }
        }
        Ok(())
    }

    pub fn new(body: RichText, compact: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let blur = cx.on_blur(&focus, window, |this, _, cx| {
            this.engine.finish_group();
            cx.notify();
        });
        let blink_task = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(550))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        if this.focus.is_focused(window)
                            && (!this.engine.vim || this.engine.mode == Mode::Insert)
                        {
                            this.blink = !this.blink;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut engine = Editor::new(body);
        if compact {
            engine.toggle_vim();
            engine.begin_group();
        }
        Self {
            engine,
            focus,
            font_size: if compact { 15. } else { 20. },
            family: if compact {
                "Segoe UI"
            } else {
                crate::fonts::NOVEL_FAMILY
            }
            .into(),
            spacing: if compact { 1.6 } else { 1.85 },
            compact,
            multiline: !compact,
            language: crate::settings::Language::Vi,
            scroll: ScrollHandle::new(),
            layout: None,
            motion_layout_key: None,
            bounds: None,
            marked: None,
            input_clock: Instant::now(),
            selecting: false,
            blink: true,
            autoscroll: false,
            center_cursor: false,
            advance_page: false,
            follow_edit: false,
            last_caret_row: None,
            viewport_height: px(0.),
            viewport_width: px(0.),
            scroll_motion: None,
            scroll_written: None,
            caret_motion: None,
            visual_caret: point(px(0.), px(0.)),
            visual_layout_key: None,
            input_flash: None,
            _blink_task: blink_task,
            _blur: blur,
        }
    }

    fn changed(&mut self, old_generation: u64, cx: &mut Context<Self>) {
        if self.engine.generation != old_generation {
            cx.emit(EditorEvent::Changed);
        }
        self.blink = true;
        self.autoscroll = true;
        cx.emit(EditorEvent::State);
        cx.notify();
    }
    pub fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        self.blink = true;
        cx.emit(EditorEvent::State);
        cx.notify();
    }

    fn scroll_target(&self) -> Pixels {
        self.scroll_motion
            .as_ref()
            .map_or(self.scroll.offset().y, |m| px(m.target))
    }

    fn scroll_to(&mut self, target: Pixels, now: Instant) {
        let target = target.max(-self.scroll.max_offset().height).min(px(0.));
        let motion = self.scroll_motion.get_or_insert_with(|| {
            Motion::new(
                self.scroll.offset().y.into(),
                now,
                Duration::from_millis(120),
            )
        });
        motion.retarget(target.into(), now);
        self.scroll_written = Some(self.scroll.offset().y);
    }

    pub fn scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.autoscroll = false;
        self.center_cursor = false;
        self.advance_page = false;
        if self.compact {
            // Popup inputs retain GPUI's native scrolling.
            self.scroll_motion = None;
            self.scroll_written = None;
            return;
        }
        // This listener runs before GPUI's built-in scroll handler. Never undo
        // a native delta: its line height belongs to the painted element's style,
        // which need not match the event-dispatch context's line height.
        window.prevent_default();
        cx.stop_propagation();
        if self
            .scroll_written
            .is_some_and(|last| (last - self.scroll.offset().y).abs() > px(0.5))
        {
            self.scroll_motion = None;
            self.scroll_written = None;
        }
        let delta = event.delta.pixel_delta(window.line_height());
        let delta = if delta.y == px(0.) { delta.x } else { delta.y };
        if event.delta.precise() {
            // Keep touchpad deltas/inertia immediate, but within the same bounds.
            let mut offset = self.scroll.offset();
            offset.y = (offset.y + delta)
                .max(-self.scroll.max_offset().height)
                .min(px(0.));
            self.scroll_motion = None;
            self.scroll_written = None;
            self.scroll.set_offset(offset);
        } else {
            self.scroll_to(self.scroll_target() + delta, Instant::now());
        }
        cx.notify();
    }

    /// Native renderer regression probe: visual state versus immediate model state.
    pub fn motion_probe(&self) -> (Point<Pixels>, Point<Pixels>, Pixels, bool) {
        (
            self.visual_caret,
            self.cursor_position(self.engine.cursor),
            self.scroll_target(),
            self.scroll_motion
                .as_ref()
                .is_some_and(|m| m.sample(Instant::now()).1)
                || self
                    .caret_motion
                    .as_ref()
                    .is_some_and(|(x, y)| x.sample(Instant::now()).1 || y.sample(Instant::now()).1)
                || self.input_flash.is_some(),
        )
    }

    pub fn format(&mut self, flag: char, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.engine.generation;
        self.engine.finish_group();
        self.engine.format(flag);
        self.focus.focus(window);
        self.changed(old, cx);
    }

    fn replay_history(&mut self, redo: bool) {
        self.advance_page = false;
        self.marked = None;
        if redo {
            self.engine.redo();
        } else {
            self.engine.undo();
        }
    }

    pub fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        let old = self.engine.generation;
        self.replay_history(redo);
        self.changed(old, cx);
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let selection = self.engine.painted_selection();
        if !selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.engine.body.text[selection].into(),
            ));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Key events may arrive before the next paint after an edit; never use stale wrap positions.
        if let Some(bounds) = self.bounds {
            let layout = self.ensure_layout(self.text_width(bounds.size.width), window);
            self.sync_display_rows(&layout);
        }
        let stroke = &event.keystroke;
        let old = self.engine.generation;
        let key = stroke.key.as_str();
        if matches!(key, "f1" | "f11") {
            return;
        }
        if stroke.modifiers.control || stroke.modifiers.platform {
            if self.engine.vim && self.engine.mode != Mode::Insert && !stroke.modifiers.shift {
                let rows = (f32::from(self.scroll.bounds().size.height)
                    / f32::from(self.line_height()))
                .floor()
                .max(2.) as isize;
                match key {
                    "b" | "f" | "u" | "d" => {
                        let rows = if matches!(key, "u" | "d") {
                            (rows / 2).max(1)
                        } else {
                            (rows - 2).max(1)
                        };
                        self.engine.display_vertical(
                            if matches!(key, "b" | "u") {
                                -rows
                            } else {
                                rows
                            },
                            true,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        self.changed(old, cx);
                        return;
                    }
                    "e" | "y" => {
                        self.scroll_to(
                            self.scroll_target()
                                + self.line_height() * if key == "y" { 1. } else { -1. },
                            Instant::now(),
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    "i" => {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    _ => {}
                }
            }
            match key {
                "a" => self.engine.select_all(),
                "c" => self.copy(cx),
                "x" => {
                    self.copy(cx);
                    self.engine.finish_group();
                    self.engine.replace(self.engine.painted_selection(), "");
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        self.engine.finish_group();
                        self.engine.begin_group();
                        self.engine.insert(&if self.multiline {
                            text
                        } else {
                            text.replace(['\r', '\n'], " ")
                        });
                        self.engine.finish_group();
                    }
                }
                "z" if stroke.modifiers.shift => self.replay_history(true),
                "z" => self.replay_history(false),
                "y" => self.replay_history(true),
                "r" if self.engine.vim && self.engine.mode == Mode::Normal => {
                    self.replay_history(true)
                }
                "b" | "i" | "u" if !self.compact => {
                    self.engine.finish_group();
                    self.engine.format(key.chars().next().unwrap());
                }
                "left" => {
                    self.engine.finish_group();
                    self.engine.move_word(true, stroke.modifiers.shift);
                }
                "right" => {
                    self.engine.finish_group();
                    self.engine.move_word(false, stroke.modifiers.shift);
                }
                _ => return,
            }
        } else if key == "escape" {
            if self.compact {
                cx.emit(EditorEvent::Cancel);
            } else {
                self.engine.escape();
                self.marked = None;
            }
        } else if self.engine.vim && self.engine.mode != Mode::Insert {
            let command = stroke.key_char.as_deref().unwrap_or(key);
            if matches!(command, "n" | "N") && self.engine.pending().is_empty() {
                cx.emit(EditorEvent::FindNext(command == "N"));
            } else {
                let append_row = command == "o"
                    && self.engine.mode == Mode::Normal
                    && self.engine.line_end(self.engine.cursor) == self.engine.body.text.len();
                match self.engine.key(command) {
                    Effect::Command => cx.emit(EditorEvent::Command),
                    Effect::Search => cx.emit(EditorEvent::Search),
                    Effect::Clipboard => cx.write_to_clipboard(ClipboardItem::new_string(
                        self.engine.register.text.clone(),
                    )),
                    Effect::None => {}
                    Effect::CenterCursor => self.center_cursor = true,
                }
                self.advance_page |= append_row && self.engine.generation != old;
            }
        } else {
            match key {
                "enter" if !self.multiline => {
                    self.engine.finish_group();
                    cx.emit(EditorEvent::Submit);
                }
                "enter" | "tab" if key == "enter" || !self.compact => {
                    self.advance_page |= key == "enter"
                        && !self.compact
                        && self.engine.selection().end == self.engine.body.text.len();
                    self.engine.edit_input(
                        self.engine.selection(),
                        if key == "enter" { "\n" } else { "    " },
                        InputEditKind::Atomic,
                        self.input_clock.elapsed(),
                    );
                }
                "backspace" | "delete" => {
                    let selection = !self.engine.selection().is_empty();
                    let range = if !self.engine.selection().is_empty() {
                        self.engine.selection()
                    } else if key == "backspace" {
                        self.engine.previous(self.engine.cursor)..self.engine.cursor
                    } else {
                        self.engine.cursor..self.engine.next(self.engine.cursor)
                    };
                    self.engine.edit_input(
                        range,
                        "",
                        if selection {
                            InputEditKind::Atomic
                        } else if key == "backspace" {
                            InputEditKind::Backspace
                        } else {
                            InputEditKind::Delete
                        },
                        self.input_clock.elapsed(),
                    );
                }
                "left" | "right" | "home" | "end" => {
                    self.engine.finish_group();
                    let p = match key {
                        "left" => {
                            if !stroke.modifiers.shift && !self.engine.selection().is_empty() {
                                self.engine.selection().start
                            } else {
                                self.engine.previous(self.engine.cursor)
                            }
                        }
                        "right" => {
                            if !stroke.modifiers.shift && !self.engine.selection().is_empty() {
                                self.engine.selection().end
                            } else {
                                self.engine.next(self.engine.cursor)
                            }
                        }
                        "home" => self.engine.line_start(self.engine.cursor),
                        _ => self.engine.line_end(self.engine.cursor),
                    };
                    self.engine.move_to(p, stroke.modifiers.shift);
                }
                "up" | "down" => {
                    self.engine.finish_group();
                    self.move_visual(if key == "up" { -1 } else { 1 }, stroke.modifiers.shift);
                }
                _ => return, // Printable text goes through the platform input handler / IME.
            }
        }
        window.prevent_default();
        cx.stop_propagation();
        self.changed(old, cx);
        if self.engine.pending().starts_with('z') {
            // A prefix is not a completed scrolling command. Preserve a
            // manually scrolled viewport until the second z arrives.
            self.autoscroll = false;
        }
    }

    fn line_height(&self) -> Pixels {
        px(self.font_size * self.spacing)
    }

    fn paint_pages(layout: &Layout, bounds: Bounds<Pixels>, window: &mut Window) {
        let Some(page) = layout.page else {
            return;
        };
        let mask = window.content_mask().bounds;
        let first = ((f32::from(mask.top() - bounds.top()) - 20.) / page.stride())
            .floor()
            .max(0.) as usize;
        let end = (((f32::from(mask.bottom() - bounds.top()) + 20.) / page.stride())
            .floor()
            .max(0.) as usize
            + 1)
        .min(layout.pages);
        for index in first..end {
            let rect = Bounds::new(
                bounds.origin + point(px(0.), px(page.top(index))),
                size(px(page.width), px(page.height)),
            );
            window.paint_shadows(
                rect,
                Corners::default(),
                &[BoxShadow {
                    color: rgba(0x28372b22).into(),
                    offset: point(px(0.), px(3.)),
                    blur_radius: px(12.),
                    spread_radius: px(0.),
                }],
            );
            window.paint_quad(quad(
                rect,
                px(0.),
                rgb(0xfffefa),
                px(1.),
                rgb(0xe4e0d8),
                BorderStyle::Solid,
            ));
        }
    }

    fn text_width(&self, width: Pixels) -> Pixels {
        if self.compact {
            width
        } else {
            (width - px(MARGIN * 2.)).max(px(1.))
        }
    }

    fn ensure_layout(&mut self, width: Pixels, window: &mut Window) -> Layout {
        if let Some(layout) = &self.layout
            && layout.generation == self.engine.generation
            && layout.width == width
            && layout.font_size == self.font_size
            && layout.family == self.family
            && layout.spacing == self.spacing
        {
            return layout.clone();
        }
        let mut runs = Vec::new();
        for run in &self.engine.body.runs {
            let mut font = font(self.family.clone());
            if run.style.bold {
                font = font.bold();
            }
            if run.style.italic {
                font = font.italic();
            }
            runs.push(TextRun {
                len: run.range.len(),
                font,
                color: rgb(0x302f2c).into(),
                background_color: None,
                underline: run.style.underline.then_some(UnderlineStyle {
                    color: None,
                    thickness: px(1.),
                    wavy: false,
                }),
                strikethrough: None,
            });
        }
        let lines = window
            .text_system()
            .shape_text(
                self.engine.body.text.clone().into(),
                px(self.font_size),
                &runs,
                None,
                None,
            )
            .unwrap_or_else(|error| {
                log::error!("Text layout: {error}");
                Default::default()
            });
        // Measure the base font independently of document contents. Vietnamese
        // fallback glyphs, emoji, or IME preedit text must not move existing rows.
        let reference = window.text_system().shape_line(
            "Hg".into(),
            px(self.font_size),
            &[TextRun {
                len: 2,
                font: font(self.family.clone()),
                color: rgb(0x302f2c).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let baseline = (self.line_height() + reference.ascent - reference.descent) / 2.;
        let mut y = px(0.);
        let page = (!self.compact).then(|| PageGeometry::new(f32::from(width) + MARGIN * 2.));
        let mut pager =
            page.map(|geometry| Paginator::new(geometry, f32::from(self.line_height())));
        let mut locations = vec![];
        let mut start = 0;
        let mut paragraphs = Vec::with_capacity(lines.len());
        for (index, mut line) in lines.into_iter().enumerate() {
            let len = line.len();
            let previous = self
                .layout
                .as_ref()
                .filter(|previous| previous.width == width)
                .and_then(|previous| previous.paragraphs.get(index))
                .filter(|previous| {
                    Arc::ptr_eq(&previous.line.unwrapped_layout, &line.unwrapped_layout)
                });
            let (rows, positions, glyph_rows) = if let Some(previous) = previous {
                line = previous.line.clone();
                (
                    previous.rows.clone(),
                    previous.positions.clone(),
                    previous.glyph_rows.clone(),
                )
            } else {
                let positions = indexed_positions(
                    line.unwrapped_layout
                        .runs
                        .iter()
                        .flat_map(|run| run.glyphs.iter())
                        .map(|glyph| (glyph.index, glyph.position.x)),
                    len,
                    line.unwrapped_layout.width,
                );
                wrap_prose(&mut line, &positions, width);
                let glyph_rows = Arc::new(row_glyphs(&line));
                (
                    Arc::new(Self::wrap_ranges(&line)),
                    Arc::new(positions),
                    glyph_rows,
                )
            };
            let row_start = locations.len();
            for row in 0..rows.len() {
                let row_y = if let Some(pager) = &mut pager {
                    px(pager.place_row())
                } else {
                    let row_y = y;
                    y += self.line_height();
                    row_y
                };
                locations.push(RowLocation {
                    paragraph: index,
                    row,
                    y: row_y,
                });
            }
            paragraphs.push(ParagraphLayout {
                line,
                start,
                y: locations[row_start].y,
                rows,
                positions,
                glyph_rows,
                row_start,
            });
            start += len + 1;
        }
        let pages = pager.as_ref().map_or(1, Paginator::pages);
        let layout = Layout {
            paragraphs,
            height: page.map_or(y.max(self.line_height()), |geometry| {
                px(geometry.total_height(pages))
            }),
            rows: Arc::new(locations),
            pages,
            page,
            width,
            generation: self.engine.generation,
            font_size: self.font_size,
            family: self.family.clone(),
            spacing: self.spacing,
            baseline,
        };
        self.layout = Some(layout.clone());
        layout
    }

    fn sync_display_rows(&mut self, layout: &Layout) {
        // GPUI may measure at several provisional widths in one frame. Only committed
        // bounds may update navigation geometry or reset the desired horizontal column.
        let key = (
            layout.width,
            layout.generation,
            layout.font_size.to_bits(),
            layout.spacing.to_bits(),
            layout.family.clone(),
        );
        if self.motion_layout_key.as_ref() == Some(&key) {
            return;
        }
        let mut display_rows = vec![];
        for paragraph in &layout.paragraphs {
            for range in Self::rows(paragraph) {
                let base = paragraph.x_for_index(range.start);
                let start = paragraph.start + range.start;
                let end = paragraph.start + range.end;
                let stops = self.engine.body.text[start..end]
                    .grapheme_indices(true)
                    .map(|(i, _)| start + i)
                    .chain([end])
                    .map(|p| {
                        (
                            p,
                            f32::from(paragraph.x_for_index(p - paragraph.start) - base),
                        )
                    })
                    .collect();
                display_rows.push(DisplayRow {
                    range: start..end,
                    stops,
                });
            }
        }
        self.engine.set_display_rows(display_rows);
        self.motion_layout_key = Some(key);
    }

    fn rows(paragraph: &ParagraphLayout) -> &[Range<usize>] {
        &paragraph.rows
    }

    fn wrap_ranges(line: &WrappedLine) -> Vec<Range<usize>> {
        let boundaries = line
            .wrap_boundaries
            .iter()
            .map(|b| line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix].index);
        let mut start = 0;
        boundaries
            .chain([line.len()])
            .map(|end| {
                let range = start..end;
                start = end;
                range
            })
            .collect()
    }

    fn cursor_position(&self, index: usize) -> Point<Pixels> {
        let Some(layout) = &self.layout else {
            return point(px(0.), px(0.));
        };
        for paragraph in &layout.paragraphs {
            if index >= paragraph.start && index <= paragraph.start + paragraph.line.len() {
                let local = index - paragraph.start;
                let rows = Self::rows(paragraph);
                for (i, row) in rows.iter().enumerate() {
                    if local < row.end
                        || i == rows.len() - 1
                        || (local == row.end
                            && index == self.engine.cursor
                            && self.engine.caret_affinity == CaretAffinity::Trailing)
                    {
                        return point(
                            paragraph.x_for_index(local) - paragraph.x_for_index(row.start)
                                + px(if self.compact { 0. } else { MARGIN }),
                            layout.rows[paragraph.row_start + i].y,
                        );
                    }
                }
            }
        }
        point(
            px(if self.compact { 0. } else { MARGIN }),
            layout.rows.last().map_or(px(0.), |row| row.y),
        )
    }
    pub fn cursor_screen_point(&self) -> Point<Pixels> {
        let inset = if self.compact {
            point(px(0.), px(0.))
        } else {
            point(px(MARGIN), px(FIRST_TEXT_TOP))
        };
        self.cursor_position(self.engine.cursor) - inset
    }
    pub fn cursor_viewport_geometry(&self) -> (Pixels, Pixels) {
        let viewport = self.scroll.bounds();
        let middle = self.bounds.unwrap().top()
            + self.cursor_position(self.engine.cursor).y
            + self.line_height() / 2.;
        (middle - viewport.top(), viewport.size.height)
    }
    pub fn page_metrics(&self) -> (usize, Pixels, Pixels) {
        let layout = self.layout.as_ref().expect("painted pagination");
        (
            layout.pages,
            px(layout.page.expect("manuscript pages").height),
            layout.height,
        )
    }
    pub fn page_boundary_probe(&self) -> Option<(usize, usize, Pixels)> {
        let layout = self.layout.as_ref()?;
        let pair = layout
            .rows
            .windows(2)
            .find(|rows| rows[1].y - rows[0].y > self.line_height() + px(1.))?;
        let offset = |row: &RowLocation| {
            let paragraph = &layout.paragraphs[row.paragraph];
            paragraph.start + paragraph.rows[row.row].start
        };
        Some((offset(&pair[0]), offset(&pair[1]), pair[1].y - pair[0].y))
    }
    pub fn validate_page_geometry(&self) {
        let layout = self.layout.as_ref().expect("painted pagination");
        let geometry = layout.page.expect("manuscript pages");
        assert_eq!(layout.height, px(geometry.total_height(layout.pages)));
        for row in layout.rows.iter() {
            let page = (f32::from(row.y) / geometry.stride()).floor() as usize;
            assert!(row.y >= px(geometry.top(page) + MARGIN));
            assert!(row.y + self.line_height() <= px(geometry.text_bottom(page) + 0.02));
            let paragraph = &layout.paragraphs[row.paragraph];
            let start = paragraph.start + paragraph.rows[row.row].start;
            let (hit, _) = self.hit_test(point(px(0.), row.y + self.line_height() / 2.));
            assert_eq!(
                hit, start,
                "page hit testing did not preserve the row's byte offset"
            );
        }
    }
    pub fn scroll_diagnostics(&self) -> String {
        format!(
            "offset={:?}; max={:?}; text={:?}; layout={:?}; tail={:?}; paper={:?}; spacer={:?}; pending={:?}; font={}/{}; rows={:?}; newlines={}",
            self.scroll.offset(),
            self.scroll.max_offset(),
            self.bounds,
            self.layout.as_ref().map(|l| (l.width, l.height)),
            self.viewport_height,
            self.scroll.bounds_for_item(0),
            self.scroll.bounds_for_item(1),
            self.engine.pending(),
            self.font_size,
            self.spacing,
            self.layout.as_ref().map(|l| (
                l.rows.len(),
                l.rows.iter().rev().take(4).map(|r| r.y).collect::<Vec<_>>()
            )),
            self.engine.body.text.matches('\n').count()
        )
    }

    pub fn measure_layout_edit(&mut self, length: usize, window: &mut Window) -> f64 {
        let generation = self.engine.generation + 1;
        self.engine = Editor::new(RichText::plain("mua gio ".repeat(length / 8)));
        self.engine.generation = generation;
        self.layout = None;
        self.motion_layout_key = None;
        let layout = self.ensure_layout(px(580.), window);
        self.sync_display_rows(&layout);
        self.engine.insert("ấ");
        let started = Instant::now();
        let layout = self.ensure_layout(px(580.), window);
        self.sync_display_rows(&layout);
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        let paragraph = &layout.paragraphs[0];
        for index in [0, 3, length / 2, length] {
            assert_eq!(
                paragraph.x_for_index(index),
                paragraph.line.unwrapped_layout.x_for_index(index)
            );
        }
        elapsed
    }

    pub fn input_layout_geometry(&self) -> (Pixels, Pixels, Pixels, Pixels) {
        let layout = self
            .layout
            .as_ref()
            .expect("input geometry needs a painted layout");
        let paragraph = &layout.paragraphs[0];
        let shaped_baseline = (self.line_height() + paragraph.line.unwrapped_layout.ascent
            - paragraph.line.unwrapped_layout.descent)
            / 2.;
        let baseline = paragraph
            .paint_origin(point(px(0.), px(0.)), self.line_height(), layout.baseline)
            .y
            + shaped_baseline;
        (
            layout.height,
            baseline,
            self.bounds.unwrap().top(),
            self.scroll.offset().y,
        )
    }

    fn hit_test(&self, pos: Point<Pixels>) -> (usize, CaretAffinity) {
        let Some(layout) = &self.layout else {
            return (0, CaretAffinity::Leading);
        };
        if layout.rows.is_empty() {
            return (0, CaretAffinity::Leading);
        }
        let height = self.line_height();
        let next = layout
            .rows
            .partition_point(|row| row.y + height / 2. < pos.y);
        let mut index = next.min(layout.rows.len() - 1);
        if next > 0
            && (f32::from(layout.rows[next - 1].y + height / 2. - pos.y)).abs()
                <= (f32::from(layout.rows[index].y + height / 2. - pos.y)).abs()
        {
            index = next - 1;
        }
        let row = &layout.rows[index];
        let paragraph = &layout.paragraphs[row.paragraph];
        let x = pos.x - px(if self.compact { 0. } else { MARGIN });
        let natural = point(x, height * row.row + height / 2.);
        let local = paragraph
            .line
            .closest_index_for_position(natural, height)
            .unwrap_or_else(|i| i);
        let offset = self.engine.snap(paragraph.start + local);
        let affinity = if local == paragraph.rows[row.row].end && row.row + 1 < paragraph.rows.len()
        {
            CaretAffinity::Trailing
        } else {
            CaretAffinity::Leading
        };
        (offset, affinity)
    }
    fn index_at_local(&self, pos: Point<Pixels>) -> usize {
        self.hit_test(pos).0
    }

    fn move_visual(&mut self, delta: i32, select: bool) {
        self.engine.display_vertical(delta as isize, select);
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.caret_motion = None;
        self.scroll_motion = None;
        self.scroll_written = None;
        self.focus.focus(window);
        self.engine.finish_group();
        self.selecting = true;
        let Some(bounds) = self.bounds else {
            return;
        };
        let (index, affinity) = self.hit_test(event.position - bounds.origin);
        self.engine.move_to(index, event.modifiers.shift);
        self.engine.caret_affinity = affinity;
        if event.click_count == 2 {
            let text = &self.engine.body.text;
            let start = text[..index]
                .rfind(char::is_whitespace)
                .map(|i| i + text[i..].chars().next().map(char::len_utf8).unwrap_or(1))
                .unwrap_or(0);
            let end = text[index..]
                .find(char::is_whitespace)
                .map(|i| i + index)
                .unwrap_or(text.len());
            self.engine.anchor = self.engine.snap(start);
            self.engine.cursor = self.engine.snap(end);
        }
        if !self.compact
            && self
                .layout
                .as_ref()
                .and_then(|l| l.rows.first())
                .is_some_and(|row| row.y == self.cursor_position(self.engine.cursor).y)
        {
            self.autoscroll = true;
        }
        self.blink = true;
        cx.emit(EditorEvent::State);
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && let Some(bounds) = self.bounds
        {
            let (index, affinity) = self.hit_test(event.position - bounds.origin);
            self.engine.move_to(index, true);
            self.engine.caret_affinity = affinity;
            cx.emit(EditorEvent::State);
            cx.notify();
        }
    }
}

impl EntityInputHandler for TextEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.engine.from_utf16(range);
        *actual = Some(self.engine.to_utf16(range.clone()));
        Some(self.engine.body.text[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.engine.vim && self.engine.mode != Mode::Insert {
            return None;
        }
        Some(UTF16Selection {
            range: self.engine.to_utf16(self.engine.selection()),
            reversed: self.engine.cursor < self.engine.anchor,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|r| self.engine.to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.engine.vim && self.engine.mode != Mode::Insert {
            return;
        }
        let old = self.engine.generation;
        let committing = self.marked.is_some();
        let range = range
            .map(|r| self.engine.from_utf16(r))
            .or(self.marked.take())
            .unwrap_or_else(|| self.engine.selection());
        let text = if self.multiline {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.replace(['\n', '\r'], " ")
        };
        let kind = if committing {
            InputEditKind::Commit
        } else if !range.is_empty() || text.contains('\n') || text.graphemes(true).count() > 1 {
            InputEditKind::Atomic
        } else {
            InputEditKind::Typing
        };
        self.advance_page |=
            !self.compact && text == "\n" && range.end == self.engine.body.text.len();
        let flash = !self.compact
            && !committing
            && range.is_empty()
            && text.graphemes(true).count() == 1
            && !text.chars().any(char::is_whitespace);
        let inserted = self
            .engine
            .edit_input(range, &text, kind, self.input_clock.elapsed());
        self.input_flash = flash.then(|| (inserted, self.engine.generation, Instant::now()));
        self.marked = None;
        self.follow_edit |= !self.compact
            && self.engine.generation != old
            && self.engine.cursor == self.engine.body.text.len();
        self.changed(old, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.engine.vim && self.engine.mode != Mode::Insert {
            return;
        }
        self.input_flash = None;
        let old = self.engine.generation;
        let range = range
            .map(|r| self.engine.from_utf16(r))
            .or(self.marked.take())
            .unwrap_or_else(|| self.engine.selection());
        let inserted = self.engine.edit_input(
            range,
            text,
            InputEditKind::Composition,
            self.input_clock.elapsed(),
        );
        let start = inserted.start;
        self.marked = (!inserted.is_empty()).then_some(inserted);
        if let Some(selection) = selection {
            let temporary = Editor::new(RichText::plain(text));
            self.engine.anchor = start + temporary.offset_from_utf16(selection.start);
            self.engine.cursor = start + temporary.offset_from_utf16(selection.end);
        }
        self.follow_edit |= !self.compact
            && self.engine.generation != old
            && self.engine.cursor == self.engine.body.text.len();
        self.changed(old, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        let range = self.engine.from_utf16(range);
        let pos = self.cursor_position(range.start);
        Some(Bounds::new(
            bounds.origin + pos,
            size(px(2.), self.line_height()),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.index_at_local(point - self.bounds?.origin);
        Some(self.engine.offset_to_utf16(index))
    }
}

struct ManuscriptElement {
    editor: Entity<TextEditor>,
}
impl IntoElement for ManuscriptElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ManuscriptElement {
    type RequestLayoutState = ();
    type PrepaintState = Layout;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, ()) {
        let entity = self.editor.clone();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.flex_shrink = 0.;
        (
            window.request_measured_layout(style, move |known, available, window, cx| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(p) => p,
                    _ => px(580.),
                });
                let layout = entity.update(cx, |e, _| e.ensure_layout(e.text_width(width), window));
                size(width, layout.height)
            }),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Layout {
        self.editor.update(cx, |e, cx| {
            let now = Instant::now();
            let visual_key = (
                e.text_width(bounds.size.width),
                e.font_size.to_bits(),
                e.spacing.to_bits(),
                e.family.clone(),
            );
            let reflow = e
                .visual_layout_key
                .as_ref()
                .is_some_and(|key| key != &visual_key);
            e.visual_layout_key = Some(visual_key);
            let layout = e.ensure_layout(e.text_width(bounds.size.width), window);
            e.sync_display_rows(&layout);
            e.bounds = Some(bounds);
            let viewport = e.scroll.bounds();
            if !e.compact
                && (e.viewport_height != viewport.size.height
                    || e.viewport_width != viewport.size.width)
            {
                e.scroll_motion = None;
                e.scroll_written = None;
                e.caret_motion = None;
                e.viewport_height = viewport.size.height;
                e.viewport_width = viewport.size.width;
                cx.on_next_frame(window, |_, _, cx| cx.notify());
            }
            // External scroll handles (including document shrink/clamping) win.
            if reflow
                || e.scroll_written
                    .is_some_and(|last| (last - e.scroll.offset().y).abs() > px(0.5))
            {
                e.scroll_motion = None;
                e.scroll_written = None;
            }
            if reflow {
                e.caret_motion = None;
            }
            if (e.autoscroll || e.center_cursor || e.advance_page) && !e.compact {
                e.autoscroll = false;
                let row_y = e.cursor_position(e.engine.cursor).y;
                let wrapped_forward = std::mem::take(&mut e.follow_edit)
                    && e.last_caret_row
                        .is_some_and(|previous| row_y > previous + e.line_height() / 2.);
                let advance_page = std::mem::take(&mut e.advance_page) || wrapped_forward;
                // Evaluate against the destination, so rapid input cannot undo
                // a page advance while its previous frames are still moving.
                let caret = bounds.top() + row_y + e.scroll_target() - e.scroll.offset().y;
                let mut offset = e.scroll.offset();
                offset.y = e.scroll_target();
                let old = offset;
                if e.center_cursor {
                    e.center_cursor = false;
                    offset.y +=
                        viewport.top() + viewport.size.height / 2. - caret - e.line_height() / 2.;
                    offset.y = offset.y.max(-e.scroll.max_offset().height).min(px(0.));
                } else if layout.rows.first().is_some_and(|row| row.y == row_y) {
                    // Reveal the page's full top margin and surrounding space,
                    // even if the first row's glyphs already fit in the viewport.
                    offset.y = px(0.);
                } else if advance_page
                    && e.engine.cursor == e.engine.body.text.len()
                    && caret + e.line_height() / 2. >= viewport.top() + viewport.size.height * 0.65
                {
                    // Hard breaks and soft wraps share the writing-area rule.
                    // Edits within the same display row do not move the viewport.
                    offset.y +=
                        viewport.top() + viewport.size.height * 0.32 - caret - e.line_height() / 2.;
                    offset.y = offset.y.max(-e.scroll.max_offset().height).min(px(0.));
                } else if caret < viewport.top() + px(12.) {
                    offset.y += viewport.top() + px(12.) - caret;
                } else if caret + e.line_height() > viewport.bottom() - px(12.) {
                    offset.y -= caret + e.line_height() - viewport.bottom() + px(12.);
                }
                if offset != old && viewport.size.height > px(0.) {
                    e.scroll_to(offset.y, now);
                }
            }
            if let Some(motion) = &e.scroll_motion {
                let (value, active) = motion.sample(now);
                let mut offset = e.scroll.offset();
                offset.y = px(value).max(-e.scroll.max_offset().height).min(px(0.));
                if offset != e.scroll.offset() {
                    e.scroll.set_offset(offset);
                    // This offset changes the element bounds on the next frame.
                    cx.on_next_frame(window, |_, _, cx| cx.notify());
                }
                e.scroll_written = Some(offset.y);
                if active {
                    window.request_animation_frame();
                }
            }
            let caret = e.cursor_position(e.engine.cursor);
            let x: f32 = caret.x.into();
            let y: f32 = caret.y.into();
            let duration = Duration::from_millis(80);
            if e.compact || e.selecting || !e.focus.is_focused(window) {
                e.caret_motion = None;
                e.visual_caret = caret;
            } else {
                let (mx, my) = e.caret_motion.get_or_insert_with(|| {
                    (Motion::new(x, now, duration), Motion::new(y, now, duration))
                });
                if my.target != y {
                    // A new row starts at its own column; never sweep diagonally
                    // through the preceding paragraph or across a page margin.
                    *mx = Motion::new(x, now, duration);
                    if (my.target - y).abs() > e.font_size * e.spacing * 2. {
                        *my = Motion::new(y, now, duration);
                    } else {
                        my.retarget(y, now);
                    }
                } else {
                    mx.retarget(x, now);
                }
                let (x, moving_x) = mx.sample(now);
                let (y, moving_y) = my.sample(now);
                e.visual_caret = point(px(x), px(y));
                if moving_x || moving_y {
                    window.request_animation_frame();
                }
            }
            if let Some((_, generation, started)) = &e.input_flash {
                if *generation != e.engine.generation
                    || now.duration_since(*started) >= Duration::from_millis(70)
                {
                    e.input_flash = None;
                } else {
                    window.request_animation_frame();
                }
            }
            e.last_caret_row = Some(e.cursor_position(e.engine.cursor).y);
            layout
        })
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        layout: &mut Layout,
        window: &mut Window,
        cx: &mut App,
    ) {
        if layout.page.is_some() {
            TextEditor::paint_pages(layout, bounds, window);
        }
        let e = self.editor.read(cx);
        let focus = e.focus.clone();
        let selection = e.engine.painted_selection();
        let line_height = e.line_height();
        let caret = e.visual_caret;
        let block = e.engine.vim && e.engine.mode != Mode::Insert;
        let blink = e.blink;
        let compact = e.compact;
        let language = e.language;
        let empty = e.engine.body.text.is_empty();
        let marked = e.marked.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let mask = window.content_mask().bounds;
        let left = px(if compact { 0. } else { MARGIN });
        let first = layout
            .rows
            .partition_point(|row| bounds.top() + row.y + line_height < mask.top());
        for location in layout.rows.iter().skip(first) {
            let y = bounds.top() + location.y;
            if y > mask.bottom() {
                break;
            }
            let paragraph = &layout.paragraphs[location.paragraph];
            let i = location.row;
            let row = &paragraph.rows[i];
            let origin = point(bounds.left() + left, y);
            let a = selection.start.max(paragraph.start + row.start);
            let b = selection.end.min(paragraph.start + row.end);
            let xbase = paragraph.x_for_index(row.start);
            if a < b
                || (selection.start <= paragraph.start + row.end
                    && selection.end > paragraph.start + row.end
                    && i == paragraph.rows.len() - 1)
            {
                let x1 = if a <= paragraph.start + row.end {
                    paragraph.x_for_index(a - paragraph.start) - xbase
                } else {
                    px(0.)
                };
                let x2 = if b >= a {
                    paragraph.x_for_index(b - paragraph.start) - xbase
                } else {
                    x1
                };
                window.paint_quad(fill(
                    Bounds::new(
                        origin + point(x1, px(0.)),
                        size((x2 - x1).max(px(5.)), line_height),
                    ),
                    rgba(0x74998c45),
                ));
            }
            if let Some(marked) = &marked {
                let a = marked.start.max(paragraph.start + row.start);
                let b = marked.end.min(paragraph.start + row.end);
                if a < b {
                    let x1 = paragraph.x_for_index(a - paragraph.start) - xbase;
                    let x2 = paragraph.x_for_index(b - paragraph.start) - xbase;
                    window.paint_quad(fill(
                        Bounds::new(
                            origin + point(x1, line_height - px(3.)),
                            size(x2 - x1, px(1.)),
                        ),
                        rgb(0x507c69),
                    ));
                }
            }
            // Paint only visible, pre-indexed glyph rows. A paragraph spanning
            // many sheets is shaped once and never rescanned per page/frame.
            for glyph in &paragraph.glyph_rows[i] {
                let position = origin + point(glyph.x, layout.baseline);
                let alpha = e
                    .input_flash
                    .as_ref()
                    .filter(|(range, _, _)| range.contains(&(paragraph.start + glyph.index)))
                    .map_or(255, |(_, _, started)| {
                        let t = (started.elapsed().as_secs_f32() / 0.070).min(1.);
                        (255. * (0.75 + 0.25 * (1. - (1. - t).powi(3)))) as u32
                    });
                let result = if glyph.emoji {
                    window.paint_emoji(position, glyph.font, glyph.glyph, px(layout.font_size))
                } else {
                    window.paint_glyph(
                        position,
                        glyph.font,
                        glyph.glyph,
                        px(layout.font_size),
                        rgba(0x302f2c00 | alpha).into(),
                    )
                };
                if let Err(error) = result {
                    log::error!("Text paint: {error}");
                }
            }
            let start = paragraph.start + row.start;
            let end = paragraph.start + row.end;
            let first_run = e
                .engine
                .body
                .runs
                .partition_point(|run| run.range.end <= start);
            for run in e
                .engine
                .body
                .runs
                .iter()
                .skip(first_run)
                .take_while(|run| run.range.start < end)
            {
                if !run.style.underline {
                    continue;
                }
                let a = run.range.start.max(start) - paragraph.start;
                let b = run.range.end.min(end) - paragraph.start;
                let x1 = paragraph.x_for_index(a) - xbase;
                let x2 = paragraph.x_for_index(b) - xbase;
                window.paint_underline(
                    origin
                        + point(
                            x1,
                            layout.baseline + paragraph.line.unwrapped_layout.descent * 0.618,
                        ),
                    (x2 - x1).max(px(1.)),
                    &UnderlineStyle {
                        color: Some(rgb(0x302f2c).into()),
                        thickness: px(1.),
                        wavy: false,
                    },
                );
            }
        }
        if empty {
            let placeholder: SharedString = if compact {
                language.text("Nhập tại đây…")
            } else {
                language.text("Nhấn i để bắt đầu câu chuyện của bạn…")
            }
            .into();
            let line = window.text_system().shape_line(
                placeholder.clone(),
                px(layout.font_size),
                &[TextRun {
                    len: placeholder.len(),
                    font: font(layout.family.clone()),
                    color: rgb(0xaaa69c).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            let top = layout.rows.first().map_or(px(0.), |row| row.y);
            let _ = line.paint(bounds.origin + point(left, top), line_height, window, cx);
        }
        if focus.is_focused(window) && (blink || block) {
            window.paint_quad(fill(
                Bounds::new(
                    bounds.origin + caret,
                    size(px(if block { 10. } else { 2. }), line_height),
                ),
                if block {
                    rgba(0x3f715866)
                } else {
                    rgba(0x3f7158ff)
                },
            ));
        }
    }
}

impl Render for TextEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paper = div()
            .when(self.compact, |d| {
                d.w_full().min_h(px(30.)).bg(rgb(0xffffff))
            })
            .when(!self.compact, |d| {
                d.w(if self.viewport_width > px(0.) {
                    (self.viewport_width - px(64.)).max(px(1.)).min(px(760.))
                } else {
                    px(760.)
                })
            })
            .flex_shrink_0()
            .child(ManuscriptElement {
                editor: cx.entity(),
            });
        let scroller = div()
            .id("editor-scroll")
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .track_focus(&self.focus)
            .key_context("Manuscript")
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .when(!self.compact, |d| {
                d.px(px(32.)).py(px(30.)).bg(rgb(0xf0efea))
            })
            // Resolve paper width before intrinsic height measurement, so the
            // flex basis and painted text use the same wrapping width.
            .child(paper)
            .when(!self.compact, |d| {
                // Pure layout space: never part of the document, undo or export.
                // Use the measured viewport; percent heights are indefinite
                // inside this scrolling flex layout.
                d.child(
                    div()
                        .h(self.viewport_height * 0.65)
                        .w_full()
                        .flex_shrink_0(),
                )
            });
        // A non-occluding sibling handles wheel input before the scroller's
        // built-in bubble listener. Cover the whole viewport, including the
        // margins and tail space, without intercepting clicks or selection.
        div()
            .relative()
            .size_full()
            .min_h_0()
            .child(scroller)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .on_scroll_wheel(cx.listener(Self::scroll_wheel)),
            )
    }
}

#[cfg(test)]
mod layout_tests {
    use super::indexed_positions;
    use gpui::px;
    #[test]
    fn indexed_glyph_positions_match_gpui_linear_lookup() {
        for glyphs in [
            vec![(0, px(0.)), (2, px(10.)), (2, px(12.)), (4, px(20.))],
            vec![(3, px(20.)), (1, px(10.)), (5, px(0.))],
            vec![],
        ] {
            let positions = indexed_positions(glyphs.iter().copied(), 8, px(30.));
            for index in 0..=8 {
                let expected = glyphs
                    .iter()
                    .find(|(offset, _)| *offset >= index)
                    .map_or(px(30.), |(_, x)| *x);
                let actual = positions
                    .get(positions.partition_point(|(offset, _)| *offset < index))
                    .map_or(px(30.), |(_, x)| *x);
                assert_eq!(actual, expected);
            }
        }
    }
}
