//! UI-independent, UTF-8/grapheme-safe editing and a deliberately scoped Vim engine.
use crate::document::{RichText, TextStyle};
use std::{ops::Range, time::Duration};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

const HISTORY_BUDGET: usize = 32 * 1024 * 1024;

/// Native text input uses short undo transactions independently of Vim commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEditKind {
    Typing,
    Backspace,
    Delete,
    Composition,
    Commit,
    Atomic,
}

struct InputGroup {
    kind: InputEditKind,
    last_at: Duration,
    started_at: Duration,
    cursor: usize,
    units: usize,
    boundary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    body: RichText,
    cursor: usize,
    anchor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    CenterCursor,
    Command,
    Search,
    Clipboard,
}

/// One shaped display row. Positions are UTF-8 grapheme boundaries and horizontal pixels.
#[derive(Clone, Debug)]
pub struct DisplayRow {
    pub range: Range<usize>,
    pub stops: Vec<(usize, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretAffinity {
    Leading,
    Trailing,
}

#[derive(Clone)]
struct CharacterFind {
    target: String,
    backwards: bool,
}

pub struct Editor {
    pub body: RichText,
    pub cursor: usize,
    pub caret_affinity: CaretAffinity,
    pub anchor: usize,
    pub mode: Mode,
    pub vim: bool,
    pub typing_style: TextStyle,
    pub generation: u64,
    pub register: RichText,
    linewise_register: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    group: Option<Snapshot>,
    input_group: Option<InputGroup>,
    count: String,
    operator: Option<char>,
    operator_count: usize,
    pending_g: bool,
    pending_z: Option<usize>,
    pending_find: Option<(bool, usize)>,
    last_find: Option<CharacterFind>,
    object: Option<char>,
    visual_anchor: usize,
    preferred_column: Option<usize>,
    display_rows: Vec<DisplayRow>,
    preferred_x: Option<f32>,
    prefer_end: bool,
    g_count: usize,
    object_count: usize,
}

impl Editor {
    pub fn new(body: RichText) -> Self {
        Self {
            body,
            cursor: 0,
            caret_affinity: CaretAffinity::Leading,
            anchor: 0,
            mode: Mode::Normal,
            vim: true,
            typing_style: TextStyle::default(),
            generation: 0,
            register: RichText::default(),
            linewise_register: false,
            undo: vec![],
            redo: vec![],
            group: None,
            input_group: None,
            count: String::new(),
            operator: None,
            operator_count: 1,
            pending_g: false,
            pending_z: None,
            pending_find: None,
            last_find: None,
            object: None,
            visual_anchor: 0,
            preferred_column: None,
            display_rows: vec![],
            preferred_x: None,
            prefer_end: false,
            g_count: 1,
            object_count: 1,
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            body: self.body.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.body = s.body;
        self.cursor = self.char_boundary(s.cursor);
        self.caret_affinity = CaretAffinity::Leading;
        self.anchor = self.char_boundary(s.anchor);
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
        }
        self.typing_style = self.body.style_at(self.cursor);
        self.display_rows.clear();
        self.clamp_normal();
        self.visual_anchor = self.snap(self.cursor);
        self.preferred_column = None;
        self.preferred_x = None;
        self.prefer_end = false;
        self.generation += 1;
        self.reset_pending();
    }
    fn push_undo(&mut self, snapshot: Snapshot) {
        self.undo.push(snapshot);
        let mut bytes = 0;
        let keep_from = self.undo.iter().enumerate().rev().find_map(|(i, s)| {
            bytes += s.body.text.len()
                + s.body.runs.len() * std::mem::size_of::<crate::document::StyledRange>();
            (bytes > HISTORY_BUDGET).then_some(i + 1)
        });
        if let Some(n) = keep_from {
            // Keep the most recent transaction even if a single chapter exceeds the budget.
            let n = n.min(self.undo.len().saturating_sub(1));
            self.undo.drain(..n);
        }
        self.redo.clear();
    }
    pub fn begin_group(&mut self) {
        if self.group.is_none() {
            self.group = Some(self.snapshot());
        }
    }
    pub fn finish_group(&mut self) {
        self.input_group = None;
        if let Some(snapshot) = self.group.take()
            && snapshot.body != self.body
        {
            self.push_undo(snapshot);
        }
    }
    pub fn undo(&mut self) {
        self.finish_group();
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(s);
        }
    }
    pub fn redo(&mut self) {
        self.finish_group();
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(s);
        }
    }

    /// `at` is monotonic time supplied by the UI; tests can advance it without sleeping.
    pub fn edit_input(
        &mut self,
        range: Range<usize>,
        text: &str,
        kind: InputEditKind,
        at: Duration,
    ) -> Range<usize> {
        if range.is_empty() && text.is_empty() {
            return range;
        }
        let composing = self
            .input_group
            .as_ref()
            .is_some_and(|g| g.kind == InputEditKind::Composition)
            && matches!(kind, InputEditKind::Composition | InputEditKind::Commit);
        let next_kind = if kind == InputEditKind::Commit {
            InputEditKind::Typing
        } else {
            kind
        };
        let split = kind == InputEditKind::Atomic
            || (!composing
                && self.input_group.as_ref().is_some_and(|g| {
                    at.saturating_sub(g.last_at) >= Duration::from_secs(1)
                        || at.saturating_sub(g.started_at) >= Duration::from_secs(5)
                        || g.units >= 32
                        || g.cursor != self.cursor
                        || (g.kind != next_kind
                            && !(g.kind == InputEditKind::Typing
                                && next_kind == InputEditKind::Composition))
                        || (g.boundary
                            && matches!(
                                next_kind,
                                InputEditKind::Typing | InputEditKind::Composition
                            ))
                }));
        if split {
            self.finish_group();
        }
        self.begin_group();
        let inserted = self.replace_native(range, text);
        if kind == InputEditKind::Atomic {
            self.finish_group();
            return inserted;
        }
        let previous = self.input_group.take();
        let boundary = text.chars().next_back().is_some_and(|c| {
            c.is_whitespace()
                || (c.is_ascii_punctuation() && c != '_')
                || matches!(
                    c,
                    '…' | '—'
                        | '–'
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | '«'
                        | '»'
                        | '。'
                        | '，'
                        | '！'
                        | '？'
                )
        });
        self.input_group = Some(InputGroup {
            kind: next_kind,
            last_at: at,
            started_at: previous.as_ref().map_or(at, |g| g.started_at),
            cursor: self.cursor,
            units: previous
                .as_ref()
                .map_or(0, |g| g.units)
                .saturating_add(text.graphemes(true).count().max(1)),
            boundary,
        });
        inserted
    }

    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
    pub fn previous(&self, p: usize) -> usize {
        self.body.text[..p]
            .grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
    pub fn next(&self, p: usize) -> usize {
        self.body.text[p..]
            .graphemes(true)
            .next()
            .map(|s| p + s.len())
            .unwrap_or(p)
    }
    pub fn snap(&self, p: usize) -> usize {
        let text = &self.body.text;
        let mut p = p.min(text.len());
        while !text.is_char_boundary(p) {
            p -= 1;
        }
        let mut cursor = GraphemeCursor::new(p, text.len(), true);
        if cursor.is_boundary(text, 0).unwrap_or(false) {
            p
        } else {
            cursor.prev_boundary(text, 0).ok().flatten().unwrap_or(0)
        }
    }
    pub fn line_start(&self, p: usize) -> usize {
        self.body.text[..p].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }
    pub fn line_end(&self, p: usize) -> usize {
        self.body.text[p..]
            .find('\n')
            .map(|i| p + i)
            .unwrap_or(self.body.text.len())
    }
    fn first_nonblank(&self, p: usize) -> usize {
        let start = self.line_start(p);
        start
            + self.body.text[start..self.line_end(p)]
                .char_indices()
                .find(|(_, c)| !c.is_whitespace())
                .map(|(i, _)| i)
                .unwrap_or(0)
    }
    pub fn move_to(&mut self, p: usize, select: bool) {
        self.cursor = self.snap(p);
        self.caret_affinity = CaretAffinity::Leading;
        if !select {
            self.anchor = self.cursor;
        }
        self.typing_style = self.body.style_at(self.cursor);
        self.preferred_column = None;
        self.preferred_x = None;
        self.prefer_end = false;
    }
    fn last_character(&self, p: usize) -> usize {
        let start = self.line_start(p);
        let end = self.line_end(p);
        if end > start {
            self.previous(end)
        } else {
            start
        }
    }
    fn clamp_normal(&mut self) {
        if self.vim && self.mode == Mode::Normal {
            self.cursor = self.cursor.min(self.last_character(self.cursor));
            self.anchor = self.cursor;
        }
    }
    pub fn set_display_rows(&mut self, rows: Vec<DisplayRow>) {
        self.display_rows = rows;
        self.preferred_x = None;
    }
    fn display_row_index(&self, p: usize) -> Option<usize> {
        if self.caret_affinity == CaretAffinity::Trailing
            && let Some(index) = self.display_rows.iter().position(|r| r.range.end == p)
        {
            return Some(index);
        }
        self.display_rows
            .iter()
            .position(|r| r.range.contains(&p) || r.range.is_empty() && r.range.start == p)
            .or_else(|| self.display_rows.iter().rposition(|r| r.range.end == p))
    }
    /// GUI arrows use the current horizontal position rather than Vim's
    /// persistent end-of-line goal. Keep a finite preferred x across short rows.
    pub fn use_finite_vertical_column(&mut self) {
        if self.prefer_end {
            self.prefer_end = false;
            self.preferred_x = None;
            self.preferred_column = None;
        }
    }

    /// Home/End use shaped rows; a soft-wrap endpoint stays on its original row.
    pub fn display_boundary(&mut self, end: bool, select: bool) {
        let (range, soft_end) = if let Some(index) = self.display_row_index(self.cursor) {
            let range = self.display_rows[index].range.clone();
            let soft_end = self
                .display_rows
                .get(index + 1)
                .is_some_and(|next| next.range.start == range.end);
            (range, soft_end)
        } else {
            (
                self.line_start(self.cursor)..self.line_end(self.cursor),
                false,
            )
        };
        let visual = matches!(self.mode, Mode::Visual | Mode::VisualLine);
        let target = if !end {
            range.start
        } else if self.vim && self.mode != Mode::Insert && (visual || !select) && !range.is_empty()
        {
            self.previous(range.end)
        } else {
            range.end
        };
        self.move_to(target, select || visual);
        if end && soft_end && target == range.end {
            self.caret_affinity = CaretAffinity::Trailing;
        }
        if visual {
            self.update_visual();
        } else if !select {
            self.clamp_normal();
        }
    }

    pub fn display_vertical(&mut self, delta: isize, select: bool) {
        let Some(index) = self.display_row_index(self.cursor) else {
            self.vertical(delta, select);
            return;
        };
        let row = &self.display_rows[index];
        let x = self.preferred_x.unwrap_or_else(|| {
            row.stops
                .iter()
                .find(|(p, _)| *p == self.cursor)
                .map(|(_, x)| *x)
                .unwrap_or(0.)
        });
        let target =
            (index as isize + delta).clamp(0, self.display_rows.len() as isize - 1) as usize;
        let row = &self.display_rows[target];
        if target == index {
            return;
        }
        let allow_end = (!self.vim || self.mode != Mode::Normal)
            && self.line_end(row.range.start) == row.range.end;
        let candidate = if self.prefer_end {
            if row.range.is_empty() || allow_end {
                row.range.end
            } else {
                self.previous(row.range.end)
            }
        } else {
            row.stops
                .iter()
                .filter(|(p, _)| allow_end || *p < row.range.end || row.range.is_empty())
                .min_by(|(_, a), (_, b)| (a - x).abs().total_cmp(&(b - x).abs()))
                .map(|(p, _)| *p)
                .unwrap_or(row.range.start)
        };
        let prefer_end = self.prefer_end;
        self.move_to(candidate, select);
        self.preferred_x = Some(x);
        self.prefer_end = prefer_end;
        if self.vim && self.mode == Mode::Normal {
            self.anchor = self.cursor;
        }
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.update_visual();
        }
    }
    pub fn select_all(&mut self) {
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
        }
        self.caret_affinity = CaretAffinity::Leading;
        self.anchor = 0;
        self.cursor = self.body.text.len();
    }
    pub fn move_word(&mut self, backwards: bool, select: bool) {
        let p = if backwards {
            self.word_back(self.cursor)
        } else {
            self.word_forward(self.cursor)
        };
        self.move_to(p, select);
    }
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let range = self.snap(range.start)..self.snap(range.end);
        self.replace_native(range, text);
    }

    fn char_boundary(&self, p: usize) -> usize {
        let mut p = p.min(self.body.text.len());
        while !self.body.text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    /// OS input ranges are Unicode scalar boundaries, including positions inside
    /// graphemes. Vim navigation/deletion continues to use `replace` and `snap`.
    pub fn replace_native(&mut self, range: Range<usize>, text: &str) -> Range<usize> {
        let start = self.char_boundary(range.start);
        let end = self.char_boundary(range.end.max(start)).max(start);
        let range = start..end;
        if self.body.text[range.clone()] == *text && text.is_empty() {
            return start..start;
        }
        if self.group.is_none() {
            self.push_undo(self.snapshot());
        }
        let start = range.start;
        self.body.replace(range, text, self.typing_style);
        self.cursor = start + text.len();
        self.caret_affinity = CaretAffinity::Leading;
        self.anchor = self.cursor;
        self.generation += 1;
        self.preferred_column = None;
        self.preferred_x = None;
        self.prefer_end = false;
        self.display_rows.clear();
        let inserted = start..self.cursor;
        // GUI Cut/Paste can edit directly while Vim Visual is active. The
        // previous Visual anchor belongs to the old body, not this revision.
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
            self.cursor = self.snap(self.cursor);
            self.anchor = self.cursor;
            self.clamp_normal();
        }
        self.visual_anchor = self.snap(self.cursor);
        inserted
    }
    pub fn insert(&mut self, text: &str) {
        self.replace(
            self.painted_selection(),
            &text.replace("\r\n", "\n").replace('\r', "\n"),
        );
    }
    pub fn backspace(&mut self) {
        let range = if self.selection().is_empty() {
            self.previous(self.cursor)..self.cursor
        } else {
            self.selection()
        };
        self.replace(range, "");
    }
    pub fn delete(&mut self) {
        let range = if self.selection().is_empty() {
            self.cursor..self.next(self.cursor)
        } else {
            self.selection()
        };
        self.replace(range, "");
    }
    pub fn format(&mut self, flag: char) {
        let range = self.painted_selection();
        let get = |s: TextStyle| match flag {
            'b' => s.bold,
            'i' => s.italic,
            _ => s.underline,
        };
        let value = !get(self.typing_style);
        let change = |s: &mut TextStyle| match flag {
            'b' => s.bold = value,
            'i' => s.italic = value,
            _ => s.underline = value,
        };
        if !range.is_empty() {
            self.finish_group();
            self.push_undo(self.snapshot());
            self.body.format(range, change);
            self.generation += 1;
        }
        change(&mut self.typing_style);
    }
    pub fn toggle_vim(&mut self) {
        self.finish_group();
        self.vim = !self.vim;
        self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
        self.anchor = self.cursor;
        self.reset_pending();
        self.clamp_normal();
    }
    pub fn escape(&mut self) {
        self.caret_affinity = CaretAffinity::Leading;
        self.finish_group();
        if self.mode == Mode::Insert && self.cursor > self.line_start(self.cursor) {
            self.cursor = self.previous(self.cursor);
        }
        self.mode = if self.vim { Mode::Normal } else { Mode::Insert };
        self.anchor = self.cursor;
        self.reset_pending();
        self.clamp_normal();
        self.preferred_x = None;
        self.prefer_end = false;
    }
    fn reset_pending(&mut self) {
        self.operator = None;
        self.count.clear();
        self.object = None;
        self.pending_g = false;
        self.pending_z = None;
        self.pending_find = None;
        self.operator_count = 1;
        self.object_count = 1;
    }
    pub fn pending(&self) -> String {
        format!(
            "{}{}{}{}{}",
            if self.pending_g {
                "g"
            } else if self.pending_z.is_some() {
                "z"
            } else {
                ""
            },
            self.operator.map(|c| c.to_string()).unwrap_or_default(),
            self.object.map(|c| c.to_string()).unwrap_or_default(),
            self.count,
            self.pending_find
                .map(|(backwards, _)| if backwards { "F" } else { "f" })
                .unwrap_or_default()
        )
    }

    fn find_character(&mut self, find: &CharacterFind, count: usize) -> Effect {
        let old = self.cursor;
        let start = if find.backwards {
            self.line_start(old)
        } else {
            self.next(old).min(self.line_end(old))
        };
        let end = if find.backwards {
            old
        } else {
            self.line_end(old)
        };
        let mut matches =
            self.body.text[start..end]
                .grapheme_indices(true)
                .filter(|(_, grapheme)| {
                    *grapheme == find.target
                        || (find.target.chars().count() == 1 && grapheme.starts_with(&find.target))
                });
        // Determine the complete counted motion before changing any editor state.
        // A failed find must not delete, enter Insert, or leave an operator armed.
        let found = if find.backwards {
            matches.rev().nth(count - 1)
        } else {
            matches.nth(count - 1)
        };
        let Some((offset, _)) = found else {
            self.reset_pending();
            return Effect::None;
        };
        let target = start + offset;
        if let Some(op) = self.operator {
            let end = if target > old { self.next(target) } else { old };
            return self.operate(op, old.min(target)..end, false);
        }
        self.move_to(target, true);
        self.update_visual();
        self.reset_pending();
        Effect::None
    }
    fn enter_insert(&mut self) {
        self.mode = Mode::Insert;
        self.anchor = self.cursor;
        self.begin_group();
    }
    fn class(&self, p: usize) -> u8 {
        match self.body.text[p..].chars().next() {
            None => 0,
            Some(c) if c.is_whitespace() => 0,
            Some(c) if c.is_alphanumeric() || c == '_' => 1,
            Some(_) => 2,
        }
    }
    fn word_forward(&self, mut p: usize) -> usize {
        let original = p;
        let class = self.class(p);
        while p < self.body.text.len() && self.class(p) == class {
            if p > original && p == self.line_start(p) && self.body.text[p..].starts_with('\n') {
                return p;
            }
            p = self.next(p);
        }
        while p < self.body.text.len() && self.class(p) == 0 {
            if p > original && p == self.line_start(p) && self.body.text[p..].starts_with('\n') {
                return p;
            }
            p = self.next(p);
        }
        p
    }
    fn word_back(&self, mut p: usize) -> usize {
        p = self.previous(p);
        while p > 0 && self.class(p) == 0 {
            if p == self.line_start(p) && self.body.text[p..].starts_with('\n') {
                return p;
            }
            p = self.previous(p);
        }
        let class = self.class(p);
        while p > 0 && self.class(self.previous(p)) == class {
            p = self.previous(p);
        }
        p
    }
    fn word_end(&self, mut p: usize) -> usize {
        p = self.next(p);
        self.word_end_from(p)
    }
    fn word_end_from(&self, mut p: usize) -> usize {
        while p < self.body.text.len() && self.class(p) == 0 {
            p = self.next(p);
        }
        let class = self.class(p);
        while p < self.body.text.len() && self.class(p) == class {
            p = self.next(p);
        }
        self.previous(p)
    }
    pub fn vertical(&mut self, delta: isize, select: bool) {
        let column = self.preferred_column.unwrap_or_else(|| {
            self.body.text[self.line_start(self.cursor)..self.cursor]
                .graphemes(true)
                .count()
        });
        let mut start = self.line_start(self.cursor);
        for _ in 0..delta.unsigned_abs() {
            start = if delta > 0 {
                let end = self.line_end(start);
                if end == self.body.text.len() {
                    break;
                }
                end + 1
            } else {
                if start == 0 {
                    break;
                }
                self.line_start(start - 1)
            };
        }
        let end = self.line_end(start);
        let offset = self.body.text[start..end]
            .grapheme_indices(true)
            .nth(column)
            .map(|(i, _)| i)
            .unwrap_or(end - start);
        self.move_to(start + offset, select);
        if self.vim && self.mode == Mode::Normal {
            self.cursor = self.cursor.min(self.last_character(self.cursor));
            if !select {
                self.anchor = self.cursor;
            }
        }
        self.preferred_column = Some(column);
    }
    fn line_range(&self, p: usize, count: usize) -> Range<usize> {
        let start = self.line_start(p);
        let mut end = start;
        for _ in 0..count {
            end = (self.line_end(end) + 1).min(self.body.text.len());
        }
        start..end
    }
    fn visual_range(&self) -> Range<usize> {
        // Defend rendering against stale byte offsets, including offsets that
        // would split a Unicode grapheme after a body replacement.
        let anchor = self.snap(self.visual_anchor);
        let cursor = self.snap(self.cursor);
        if self.mode == Mode::VisualLine {
            self.line_start(anchor.min(cursor))
                ..(self.line_end(anchor.max(cursor)) + 1).min(self.body.text.len())
        } else {
            anchor.min(cursor)..self.next(anchor.max(cursor))
        }
    }
    fn update_visual(&mut self) {
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            // Visual selection is inclusive; cursor remains the active endpoint.
            let range = self.visual_range();
            self.anchor = if self.cursor < self.visual_anchor {
                range.end
            } else {
                range.start
            };
        } else {
            self.anchor = self.cursor;
        }
    }
    pub fn painted_selection(&self) -> Range<usize> {
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.visual_range()
        } else {
            self.selection()
        }
    }
    fn operate(&mut self, op: char, range: Range<usize>, linewise: bool) -> Effect {
        let old_cursor = self.cursor;
        if range.is_empty() && !linewise && op != 'c' {
            self.reset_pending();
            return Effect::None;
        }
        self.register = self.body.slice(range.clone());
        self.linewise_register = linewise;
        let mut edit_range = range.clone();
        if linewise && op == 'd' && range.end == self.body.text.len() && range.start > 0 {
            edit_range.start -= 1; // Removing the last logical line also removes its preceding separator.
        }
        if linewise
            && op == 'c'
            && edit_range.end > edit_range.start
            && self.body.text[..edit_range.end].ends_with('\n')
        {
            edit_range.end -= 1; // cc leaves a blank line ready for Insert.
        }
        if op != 'y' {
            if op == 'c' {
                self.begin_group();
            }
            self.replace(edit_range.clone(), "");
        }
        self.mode = if op == 'c' {
            Mode::Insert
        } else {
            Mode::Normal
        };
        let at = if op == 'y' {
            old_cursor
        } else {
            edit_range.start.min(self.body.text.len())
        };
        self.move_to(at, false);
        if linewise && op != 'y' {
            self.move_to(self.first_nonblank(self.cursor), false);
        }
        self.clamp_normal();
        self.reset_pending();
        if op == 'c' {
            self.enter_insert();
        }
        Effect::Clipboard
    }

    /// `key` is a GPUI-independent key name, preserving uppercase commands.
    pub fn key(&mut self, key: &str) -> Effect {
        if key == "escape" {
            self.escape();
            return Effect::None;
        }
        if !self.vim || self.mode == Mode::Insert {
            return Effect::None;
        }
        if let Some((backwards, count)) = self.pending_find.take() {
            let target = match key {
                "space" => " ",
                "tab" => "\t",
                _ => key,
            };
            if target.graphemes(true).count() != 1
                || target.chars().any(|c| c.is_control() && c != '\t')
            {
                self.reset_pending();
                return Effect::None;
            }
            let find = CharacterFind {
                target: target.into(),
                backwards,
            };
            self.last_find = Some(find.clone());
            return self.find_character(&find, count);
        }
        if let Some(line) = self.pending_z.take() {
            self.reset_pending();
            if key != "z" {
                return Effect::None;
            }
            if line > 0 {
                let column = self.body.text[self.line_start(self.cursor)..self.cursor]
                    .graphemes(true)
                    .count();
                self.cursor = 0;
                self.preferred_column = Some(column);
                self.vertical(line.saturating_sub(1) as isize, self.mode != Mode::Normal);
                self.update_visual();
            }
            return Effect::CenterCursor;
        }
        if key == ":" {
            self.reset_pending();
            return Effect::Command;
        }
        if key == "/" {
            self.reset_pending();
            return Effect::Search;
        }
        if key == "ctrl-r" {
            self.redo();
            return Effect::None;
        }
        if key.len() == 1
            && key.as_bytes()[0].is_ascii_digit()
            && (key != "0" || !self.count.is_empty())
        {
            if self.count.len() < 4 {
                self.count.push_str(key);
            }
            return Effect::None;
        }
        let counted = !self.count.is_empty();
        let count = self.count.parse::<usize>().unwrap_or(1).clamp(1, 9999);
        self.count.clear();
        let total = count.saturating_mul(self.operator_count).min(9999);
        if let Some(object) = self.object.take() {
            let total = self.object_count.saturating_mul(count).min(9999);
            if key == "w" {
                let mut start = self.cursor;
                let class = self.class(start);
                while start > 0 && self.class(self.previous(start)) == class {
                    start = self.previous(start);
                }
                let mut end = self.cursor;
                while end < self.body.text.len() && self.class(end) == class {
                    end = self.next(end);
                }
                if object == 'i' {
                    for _ in 1..total {
                        let class = self.class(end);
                        while end < self.body.text.len() && self.class(end) == class {
                            end = self.next(end);
                        }
                    }
                } else {
                    let extra = if class == 0 {
                        total
                    } else {
                        total.saturating_sub(1)
                    };
                    for _ in 0..extra {
                        while end < self.body.text.len() && self.class(end) == 0 {
                            end = self.next(end);
                        }
                        let class = self.class(end);
                        while end < self.body.text.len() && self.class(end) == class {
                            end = self.next(end);
                        }
                    }
                }
                if object == 'a' {
                    let before = end;
                    while end < self.body.text.len() && self.class(end) == 0 {
                        if self.body.text[end..].starts_with('\n') {
                            break;
                        }
                        end = self.next(end);
                    }
                    if end == before {
                        while start > self.line_start(start)
                            && self.class(self.previous(start)) == 0
                        {
                            start = self.previous(start);
                        }
                    }
                }
                if let Some(op) = self.operator {
                    if op == 'y' {
                        self.cursor = start;
                    }
                    return self.operate(op, start..end, false);
                }
                if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                    self.mode = Mode::Visual;
                    self.visual_anchor = start;
                    self.cursor = if end > start {
                        self.previous(end)
                    } else {
                        start
                    };
                    self.update_visual();
                    self.reset_pending();
                    return Effect::None;
                }
            }
            self.reset_pending();
            return Effect::None;
        }
        if self.pending_g {
            self.pending_g = false;
            let count = self.g_count;
            if matches!(key, "j" | "k" | "down" | "up") {
                let old = self.cursor;
                self.display_vertical(
                    if matches!(key, "j" | "down") {
                        count as isize
                    } else {
                        -(count as isize)
                    },
                    true,
                );
                if let Some(op) = self.operator {
                    return self.operate(op, old.min(self.cursor)..old.max(self.cursor), false);
                }
                self.update_visual();
                return Effect::None;
            }
            if matches!(key, "0" | "^" | "$") {
                let old = self.cursor;
                if key == "$" && count > 1 {
                    self.display_vertical(count as isize - 1, true);
                }
                if let Some(index) = self.display_row_index(self.cursor) {
                    let row = &self.display_rows[index];
                    let start = row.range.start;
                    let end = row.range.end;
                    self.cursor = if key == "$" {
                        if start == end {
                            start
                        } else {
                            self.previous(end)
                        }
                    } else if key == "^" {
                        start
                            + self.body.text[start..end]
                                .find(|c: char| !c.is_whitespace())
                                .unwrap_or(0)
                    } else {
                        start
                    };
                    if let Some(op) = self.operator {
                        return self.operate(
                            op,
                            old.min(self.cursor)..if key == "$" {
                                self.next(old.max(self.cursor))
                            } else {
                                old.max(self.cursor)
                            },
                            false,
                        );
                    }
                    self.preferred_x = None;
                    self.prefer_end = key == "$";
                    self.update_visual();
                }
                return Effect::None;
            }
            if key == "g" {
                let old = self.cursor;
                self.preferred_x = None;
                self.prefer_end = false;
                self.preferred_column = None;
                self.cursor = 0;
                if count > 1 {
                    self.vertical(count as isize - 1, false);
                }
                self.cursor = self.first_nonblank(self.cursor);
                if let Some(op) = self.operator {
                    return self.operate(
                        op,
                        self.line_start(self.cursor.min(old))
                            ..(self.line_end(self.cursor.max(old)) + 1).min(self.body.text.len()),
                        true,
                    );
                }
                self.clamp_normal();
                self.update_visual();
            } else {
                self.reset_pending();
            }
            return Effect::None;
        }
        if key == "z" {
            if self.operator.is_none() {
                self.pending_z = Some(if counted { count } else { 0 });
            } else {
                self.reset_pending();
            }
            return Effect::None;
        }
        if matches!(key, "f" | "F") {
            self.pending_find = Some((key == "F", total));
            return Effect::None;
        }
        if matches!(key, ";" | ",") {
            if let Some(mut find) = self.last_find.clone() {
                // Reversing with ',' does not change the remembered direction.
                find.backwards ^= key == ",";
                return self.find_character(&find, total);
            }
            self.reset_pending();
            return Effect::None;
        }
        if let Some(op) = self.operator {
            if key == op.to_string() {
                return self.operate(op, self.line_range(self.cursor, total), true);
            }
            if key == "i" || key == "a" {
                self.object = key.chars().next();
                self.object_count = total;
                return Effect::None;
            }
        }
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) && matches!(key, "i" | "a") {
            self.object = key.chars().next();
            self.object_count = total;
            return Effect::None;
        }
        if matches!(self.mode, Mode::Visual | Mode::VisualLine)
            && matches!(key, "d" | "c" | "y" | "x")
        {
            let range = self.visual_range();
            if key == "y" {
                self.cursor = range.start;
            }
            return self.operate(
                if key == "x" {
                    'd'
                } else {
                    key.chars().next().unwrap_or('d')
                },
                range,
                self.mode == Mode::VisualLine,
            );
        }
        let old = self.cursor;
        if !matches!(key, "j" | "k" | "down" | "up" | "g") {
            self.preferred_column = None;
            self.preferred_x = None;
            self.prefer_end = false;
        }
        let mut motion = true;
        let mut inclusive = false;
        match key {
            "h" | "left" => {
                for _ in 0..total {
                    self.cursor = self.previous(self.cursor).max(self.line_start(self.cursor));
                }
            }
            "l" | "right" => {
                for _ in 0..total {
                    let limit = if self.operator.is_some() || self.mode != Mode::Normal {
                        self.line_end(self.cursor)
                    } else {
                        self.last_character(self.cursor)
                    };
                    self.cursor = self.next(self.cursor).min(limit);
                }
            }
            "j" | "down" => {
                if self.operator.is_some() {
                    self.vertical(total as isize, true);
                } else {
                    self.display_vertical(total as isize, true);
                }
            }
            "k" | "up" => {
                if self.operator.is_some() {
                    self.vertical(-(total as isize), true);
                } else {
                    self.display_vertical(-(total as isize), true);
                }
            }
            "w" => {
                if self.operator == Some('c') && self.class(self.cursor) != 0 {
                    // Vim's cw changes through word-end, preserving the following whitespace.
                    for n in 0..total {
                        if n > 0 {
                            self.cursor = self.next(self.cursor);
                        }
                        self.cursor = self.word_end_from(self.cursor);
                    }
                    inclusive = true;
                } else {
                    for n in 0..total {
                        let before = self.cursor;
                        self.cursor = self.word_forward(self.cursor);
                        if n + 1 == total
                            && self.operator.is_some()
                            && self.cursor > self.line_end(before)
                        {
                            self.cursor = self.line_end(before);
                        }
                    }
                }
            }
            "b" => {
                for _ in 0..total {
                    self.cursor = self.word_back(self.cursor);
                }
            }
            "e" => {
                for _ in 0..total {
                    self.cursor = self.word_end(self.cursor);
                }
                inclusive = true;
            }
            "0" | "home" => {
                let start = self
                    .display_row_index(self.cursor)
                    .map(|index| self.display_rows[index].range.start)
                    .unwrap_or_else(|| self.line_start(self.cursor));
                self.move_to(start, true);
            }
            "^" => self.cursor = self.first_nonblank(self.cursor),
            "$" | "end" => {
                if total > 1 {
                    self.vertical(total as isize - 1, true);
                }
                self.cursor = self.last_character(self.cursor);
                inclusive = true;
                self.prefer_end = true;
            }
            "g" => {
                self.pending_g = true;
                self.g_count = total;
                return Effect::None;
            }
            "G" => {
                self.cursor = if counted {
                    0
                } else {
                    self.line_start(self.body.text.len())
                };
                if counted && total > 1 {
                    self.vertical(total as isize - 1, true);
                }
                self.cursor = self.first_nonblank(self.cursor);
            }
            _ => motion = false,
        }
        if motion {
            if let Some(op) = self.operator {
                if self.cursor == old && matches!(key, "j" | "k" | "up" | "down") {
                    self.reset_pending();
                    return Effect::None;
                }
                let linewise = matches!(key, "j" | "k" | "up" | "down" | "G");
                let range = if linewise {
                    self.line_start(old.min(self.cursor))
                        ..(self.line_end(old.max(self.cursor)) + 1).min(self.body.text.len())
                } else {
                    old.min(self.cursor)..if inclusive {
                        self.next(old.max(self.cursor))
                    } else {
                        old.max(self.cursor)
                    }
                };
                self.cursor = old;
                return self.operate(op, range, linewise);
            }
            self.update_visual();
            self.clamp_normal();
            return Effect::None;
        }
        if self.operator.is_some() {
            self.reset_pending();
            return Effect::None;
        }
        match key {
            "i" => self.enter_insert(),
            "a" => {
                self.cursor = self.next(self.cursor).min(self.line_end(self.cursor));
                self.enter_insert();
            }
            "I" => {
                self.cursor = self.first_nonblank(self.cursor);
                self.enter_insert();
            }
            "A" => {
                let end = self
                    .display_row_index(self.cursor)
                    .map(|index| self.display_rows[index].range.end)
                    .unwrap_or_else(|| self.line_end(self.cursor));
                self.cursor = end;
                self.typing_style =
                    self.body
                        .style_at(if end > 0 { self.previous(end) } else { end });
                self.enter_insert();
                // A soft-wrap boundary is also the next row's start. Keep the insertion
                // caret on the preceding row until text is inserted or it is moved.
                self.caret_affinity = CaretAffinity::Trailing;
            }
            "o" => {
                self.begin_group();
                self.move_to(self.line_end(self.cursor), false);
                self.insert("\n");
                self.enter_insert();
            }
            "O" => {
                self.begin_group();
                self.move_to(self.line_start(self.cursor), false);
                let p = self.cursor;
                self.insert("\n");
                self.move_to(p, false);
                self.enter_insert();
            }
            "v" | "V" => {
                let mode = if key == "v" {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                if self.mode == mode {
                    self.escape();
                } else {
                    if !matches!(self.mode, Mode::Visual | Mode::VisualLine) {
                        self.visual_anchor = self.cursor;
                    }
                    self.mode = mode;
                    self.update_visual();
                }
            }
            "d" | "y" | "c" => {
                self.operator = key.chars().next();
                self.operator_count = count;
            }
            "x" => {
                let mut end = self.cursor;
                for _ in 0..count {
                    end = self.next(end).min(self.line_end(self.cursor));
                }
                return self.operate('d', self.cursor..end, false);
            }
            "X" => {
                let mut start = self.cursor;
                for _ in 0..count {
                    start = self.previous(start).max(self.line_start(self.cursor));
                }
                return self.operate('d', start..self.cursor, false);
            }
            "D" | "C" => {
                let start = self.cursor;
                if count > 1 {
                    self.vertical(count as isize - 1, true);
                }
                let end = self.line_end(self.cursor);
                self.cursor = start;
                return self.operate(if key == "D" { 'd' } else { 'c' }, start..end, false);
            }
            "u" => self.undo(),
            "p" | "P" => {
                let mut register = self.register.clone();
                if !register.text.is_empty() || self.linewise_register {
                    self.begin_group();
                    let linewise = self.linewise_register;
                    if linewise && register.text.ends_with('\n') {
                        register = register.slice(0..register.text.len() - 1);
                    }
                    let at_end = self.line_end(self.cursor) == self.body.text.len();
                    let at = if self.linewise_register {
                        if key == "P" {
                            self.line_start(self.cursor)
                        } else {
                            (self.line_end(self.cursor) + 1).min(self.body.text.len())
                        }
                    } else if key == "p" {
                        self.next(self.cursor).min(self.line_end(self.cursor))
                    } else {
                        self.cursor
                    };
                    self.move_to(at, false);
                    if linewise && key == "p" && at_end {
                        self.insert("\n");
                    }
                    let at = self.cursor;
                    for n in 0..count {
                        if linewise && n > 0 {
                            self.insert("\n");
                        }
                        let start = self.cursor;
                        self.insert(&register.text);
                        for run in &register.runs {
                            self.body
                                .format(start + run.range.start..start + run.range.end, |s| {
                                    *s = run.style
                                });
                        }
                    }
                    let end = self.cursor;
                    if linewise && (key == "P" || !at_end) {
                        self.insert("\n");
                    }
                    self.finish_group();
                    self.move_to(
                        if linewise {
                            self.first_nonblank(at)
                        } else {
                            self.previous(end)
                        },
                        false,
                    );
                    self.clamp_normal();
                }
            }
            "J" => {
                self.begin_group();
                let mut cursor = self.line_end(self.cursor);
                for _ in 0..count.saturating_sub(1).max(1) {
                    let end = self.line_end(self.cursor);
                    if end == self.body.text.len() {
                        break;
                    }
                    let mut next = end + 1;
                    while next < self.body.text.len()
                        && matches!(self.body.text[next..].chars().next(), Some(' ' | '\t'))
                    {
                        next = self.next(next);
                    }
                    let separator = if end == self.line_start(end)
                        || self.body.text[..end].ends_with(char::is_whitespace)
                    {
                        ""
                    } else {
                        " "
                    };
                    self.replace(end..next, separator);
                    cursor = end;
                }
                self.finish_group();
                self.move_to(cursor, false);
                self.clamp_normal();
            }
            _ => self.reset_pending(),
        }
        Effect::None
    }

    pub fn find(&mut self, query: &str, backwards: bool) -> bool {
        if query.is_empty() {
            return false;
        }
        let text = &self.body.text;
        let result = if backwards {
            text[..self.cursor]
                .rfind(query)
                .or_else(|| text.rfind(query))
        } else {
            let start = self.next(self.cursor);
            text[start..]
                .find(query)
                .map(|p| p + start)
                .or_else(|| text[..start].find(query))
        };
        if let Some(start) = result {
            self.move_to(start, false);
            if self.mode == Mode::Insert {
                self.anchor = start;
                self.cursor = start + query.len();
            }
            true
        } else {
            false
        }
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.body.text[..self.char_boundary(offset)]
            .encode_utf16()
            .count()
    }
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (i, c) in self.body.text.char_indices() {
            if units + c.len_utf16() > offset {
                return i;
            }
            units += c.len_utf16();
        }
        self.body.text.len()
    }
    pub fn from_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
    pub fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
    pub fn position(&self) -> (usize, usize) {
        (
            self.body.text[..self.cursor]
                .bytes()
                .filter(|c| *c == b'\n')
                .count()
                + 1,
            self.body.text[self.line_start(self.cursor)..self.cursor]
                .graphemes(true)
                .count()
                + 1,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn character_find_is_inclusive_counted_and_grapheme_safe() {
        let mut e = editor("a niềm 🌿 a\u{0301} niềm 🌿\nniềm");
        let original = e.body.clone();
        keys(&mut e, &["2", "f", "ề"]);
        assert_eq!(e.cursor, e.body.text.rfind("ềm 🌿").unwrap());
        keys(&mut e, &["F", "🌿"]);
        assert_eq!(e.cursor, "a niềm ".len());
        keys(&mut e, &["f", "a"]);
        assert_eq!(e.cursor, "a niềm 🌿 ".len());
        assert_eq!(e.body, original);
        assert_eq!(e.generation, 0);
        keys(&mut e, &["v", "f", "🌿"]);
        assert_eq!(&e.body.text[e.painted_selection()], "a\u{0301} niềm 🌿");
        keys(&mut e, &["d"]);
        assert_eq!(e.register.text, "a\u{0301} niềm 🌿");
        e.undo();
        assert_eq!(e.body, original);

        let mut e = wrapped_editor();
        keys(&mut e, &["f", "f"]);
        assert_eq!(
            e.cursor, 5,
            "find follows the logical line across soft wraps"
        );
    }

    #[test]
    fn failed_or_cancelled_find_never_edits_or_leaks_an_operator() {
        for prefix in [vec!["d"], vec!["c"], vec!["y"], vec!["v"], vec![]] {
            for suffix in [
                vec!["2", "f", "b"],
                vec!["f", "z"],
                vec!["F", "a"],
                vec!["f", "escape"],
                vec!["f", "left"],
            ] {
                let mut e = editor("abc\nz");
                e.register = RichText::plain("kept");
                keys(&mut e, &prefix);
                let mode = e.mode;
                keys(&mut e, &suffix);
                assert_eq!(e.cursor, 0, "{prefix:?} {suffix:?}");
                assert_eq!(e.body.text, "abc\nz");
                assert_eq!(e.register.text, "kept");
                assert_eq!(e.generation, 0);
                assert!(e.pending().is_empty());
                assert_eq!(
                    e.mode,
                    if suffix.contains(&"escape") {
                        Mode::Normal
                    } else {
                        mode
                    }
                );
                e.key("l");
                assert_eq!(e.body.text, "abc\nz", "failed find leaked its operator");
                assert_eq!(e.cursor, 1);
            }
        }
        let mut e = editor("");
        keys(&mut e, &["d", "f", "a", ";", ","]);
        assert_eq!(e.body.text, "");
        assert_eq!(e.mode, Mode::Normal);
    }

    #[test]
    fn character_find_treats_command_keys_as_literal_targets() {
        for target in ["/", ":", "0", "2", "f", "F", ";", ",", "n", " ", "\t"] {
            let mut e = editor(&format!("a{target}b"));
            e.key("f");
            assert_eq!(e.pending(), "f");
            assert_eq!(e.key(target), super::Effect::None);
            assert_eq!(e.cursor, 1, "{target:?}");
            assert_eq!(e.mode, Mode::Normal);
            assert!(e.pending().is_empty());
        }
        let mut e = editor("a b\tc");
        keys(&mut e, &["f", "space", "f", "tab"]);
        assert_eq!(e.cursor, 3);
    }

    #[test]
    fn character_find_repeat_preserves_direction_and_operator_counts() {
        let mut e = editor("a.b.c.d.e.f");
        keys(&mut e, &["f", ".", "2", ";"]);
        assert_eq!(e.cursor, 5);
        keys(&mut e, &[",", ","]);
        assert_eq!(e.cursor, 1);
        keys(&mut e, &[";"]);
        assert_eq!(e.cursor, 3);
        keys(&mut e, &["2", "d", "2", ";"]);
        assert_eq!(
            e.cursor, 3,
            "insufficient matches cancel the entire operator"
        );
        assert_eq!(e.body.text, "a.b.c.d.e.f");
        keys(&mut e, &["d", "2", ";"]);
        assert_eq!(e.body.text, "a.be.f");
        assert_eq!(e.register.text, ".c.d.");
    }

    #[test]
    fn visual_history_restores_valid_state_through_empty_unicode_bodies() {
        for visual in ["v", "V"] {
            for backwards in [false, true] {
                let mut e = editor("");
                keys(&mut e, &["i"]);
                e.insert("a\u{0301}🌿 đời");
                e.escape();
                let original = e.body.clone();
                e.move_to(e.previous(e.body.text.len()), false);
                keys(&mut e, &[visual]);
                if backwards {
                    keys(&mut e, &["h"]);
                }
                e.undo();
                assert_eq!(e.body.text, "");
                assert_eq!(e.mode, Mode::Normal);
                assert_eq!(e.painted_selection(), 0..0);
                e.redo();
                assert_eq!(e.body, original);
                assert_eq!(e.mode, Mode::Normal);
                assert!(e.painted_selection().is_empty());
                keys(&mut e, &[visual]);
                e.undo();
                // Redo itself is also safe when entered from a Visual selection.
                keys(&mut e, &[visual]);
                e.redo();
                assert_eq!(e.body, original);
                assert_eq!(e.mode, Mode::Normal);
                assert!(e.painted_selection().is_empty());
            }
        }
    }

    #[test]
    fn gui_cut_paste_and_select_all_finish_visual_and_roundtrip_history() {
        for visual in ["v", "V"] {
            for backwards in [false, true] {
                for paste in [false, true] {
                    let mut e = editor("a\u{0301}🌿 đời");
                    let original = e.body.clone();
                    e.move_to(
                        if backwards {
                            e.previous(e.body.text.len())
                        } else {
                            0
                        },
                        false,
                    );
                    keys(&mut e, &[visual, if backwards { "h" } else { "l" }]);
                    let selected = e.painted_selection();
                    let value = if paste { "ấ🌱" } else { "" };
                    let mut expected = original.text.clone();
                    expected.replace_range(selected.clone(), value);
                    e.finish_group();
                    if paste {
                        e.begin_group();
                        e.insert(value);
                        e.finish_group();
                    } else {
                        let inserted = e.replace_native(selected.clone(), value);
                        assert_eq!(inserted, selected.start..selected.start);
                    }
                    assert_eq!(e.body.text, expected);
                    assert_eq!(e.mode, Mode::Normal);
                    assert!(e.painted_selection().is_empty());
                    e.undo();
                    assert_eq!(e.body, original);
                    e.redo();
                    assert_eq!(e.body.text, expected);
                    e.body.validate().unwrap();
                }
                let mut e = editor("a\u{0301}🌿 đời");
                keys(&mut e, &[visual]);
                e.select_all();
                assert_eq!(e.painted_selection(), 0..e.body.text.len());
                e.replace(e.painted_selection(), "");
                assert_eq!(e.painted_selection(), 0..0);
            }
        }
    }

    #[test]
    fn stale_visual_offsets_never_escape_utf8_or_grapheme_boundaries() {
        let mut e = editor("a\u{0301}🌿");
        for mode in [Mode::Visual, Mode::VisualLine] {
            e.mode = mode;
            for (anchor, cursor) in [(usize::MAX, usize::MAX), (2, 5), (5, 2)] {
                e.visual_anchor = anchor;
                e.cursor = cursor;
                let range = e.painted_selection();
                assert!(range.start <= range.end && range.end <= e.body.text.len());
                assert_eq!(e.snap(range.start), range.start);
                assert_eq!(e.snap(range.end), range.end);
            }
        }
    }

    #[test]
    fn gui_home_end_use_display_rows_with_selection_and_soft_wrap_affinity() {
        for mode in [Mode::Normal, Mode::Insert] {
            for vim in [true, false] {
                let mut e = wrapped_editor();
                e.vim = vim;
                e.mode = if vim { mode } else { Mode::Insert };
                let original = e.body.clone();
                e.move_to(4, false);
                e.display_boundary(false, false);
                assert_eq!(e.cursor, 3);
                e.display_boundary(true, false);
                assert_eq!(e.cursor, if vim && mode == Mode::Normal { 5 } else { 6 });
                e.display_boundary(true, false);
                e.display_boundary(false, false);
                assert_eq!(e.cursor, 3, "End lost its row affinity");
                e.move_to(1, false);
                e.display_boundary(true, true);
                assert_eq!(e.selection(), 1..3);
                e.display_boundary(false, true);
                assert_eq!(e.selection(), 0..1);
                e.move_to(7, false);
                e.display_boundary(true, false);
                assert_eq!(e.cursor, if vim && mode == Mode::Normal { 7 } else { 8 });
                assert_eq!(e.body, original);
                assert_eq!(e.generation, 0);
            }
        }
        let mut e = wrapped_editor();
        e.move_to(1, false);
        keys(&mut e, &["v"]);
        e.display_boundary(true, false);
        assert_eq!(e.painted_selection(), 1..3);
        let mut e = editor("a\u{0301}🌿\n");
        e.toggle_vim();
        e.move_to(4, false);
        e.display_boundary(false, false);
        assert_eq!(e.cursor, 0);
        e.display_boundary(true, true);
        assert_eq!(e.selection(), 0.."a\u{0301}🌿".len());
        e.move_to(e.body.text.len(), false);
        e.display_boundary(true, false);
        assert_eq!(e.cursor, e.body.text.len());
    }

    #[test]
    fn zz_preserves_cursor_selection_history_and_cancels_invalid_prefixes() {
        use super::Effect;
        let mut e = Editor::new(RichText::plain("Mùa gió\n🌿a\u{0301} dài"));
        e.move_to("Mùa ".len(), false);
        let cursor = e.cursor;
        let original = e.body.clone();
        assert_eq!(e.key("z"), Effect::None);
        assert_eq!(e.pending(), "z");
        assert_eq!(e.key("z"), Effect::CenterCursor);
        assert_eq!(e.cursor, cursor);
        assert!(e.pending().is_empty());
        e.key("v");
        e.key("l");
        let selection = e.painted_selection();
        e.key("z");
        assert_eq!(e.key("z"), Effect::CenterCursor);
        assert_eq!(e.painted_selection(), selection);
        e.key("escape");
        e.key("z");
        e.key("escape");
        assert_eq!(e.key("z"), Effect::None);
        assert_eq!(e.key("h"), Effect::None);
        assert!(e.pending().is_empty());
        e.key("d");
        e.key("z");
        e.key("escape");
        e.undo();
        assert_eq!(e.body, original);
        assert_eq!(e.generation, 0);
        e.key("i");
        assert_eq!(e.key("z"), Effect::None);
        assert!(e.pending().is_empty());
    }

    #[test]
    fn counted_zz_uses_logical_lines_and_keeps_unicode_column() {
        use super::Effect;
        let mut e = Editor::new(RichText::plain("a🌿ếd\nx\na🌿ếd"));
        e.move_to("a🌿".len(), false);
        e.key("2");
        e.key("z");
        assert_eq!(e.key("z"), Effect::CenterCursor);
        assert_eq!(e.cursor, "a🌿ếd\n".len());
        e.key("3");
        e.key("z");
        assert_eq!(e.key("z"), Effect::CenterCursor);
        // zz preserves the actual column on the short line, as Vim does.
        assert_eq!(e.cursor, "a🌿ếd\nx\n".len());
        e.key("9");
        e.key("9");
        e.key("z");
        e.key("z");
        assert_eq!(e.cursor, "a🌿ếd\nx\n".len());
        assert_eq!(e.generation, 0);
    }
    use super::*;
    fn editor(text: &str) -> Editor {
        Editor::new(RichText::plain(text))
    }
    fn keys(e: &mut Editor, keys: &[&str]) {
        for key in keys {
            e.key(key);
        }
        e.body.validate().unwrap();
    }
    #[test]
    fn vietnamese_and_emoji_are_atomic() {
        let mut e = editor("a\u{0301}👨‍👩‍👧‍👦đời");
        keys(&mut e, &["x"]);
        assert_eq!(e.body.text, "👨‍👩‍👧‍👦đời");
        keys(&mut e, &["x"]);
        assert_eq!(e.body.text, "đời");
        e.undo();
        assert_eq!(e.body.text, "👨‍👩‍👧‍👦đời");
        e.redo();
        assert_eq!(e.body.text, "đời");
    }
    #[test]
    fn counted_operators_and_linewise_paste() {
        let mut e = editor("one two three four\nlast");
        keys(&mut e, &["d", "2", "w"]);
        assert_eq!(e.body.text, "three four\nlast");
        e.undo();
        keys(&mut e, &["2", "d", "w"]);
        assert_eq!(e.body.text, "three four\nlast");
        keys(&mut e, &["d", "d", "p"]);
        assert_eq!(e.body.text, "last\nthree four");
    }
    #[test]
    fn text_object_change_is_one_undo_transaction() {
        let mut e = editor("hello world");
        keys(&mut e, &["w", "c", "i", "w"]);
        assert_eq!(e.mode, Mode::Insert);
        e.insert("đất");
        e.insert(" trời");
        e.escape();
        assert_eq!(e.body.text, "hello đất trời");
        e.undo();
        assert_eq!(e.body.text, "hello world");
    }
    #[test]
    fn visual_includes_active_grapheme() {
        let mut e = editor("mùa gió");
        keys(&mut e, &["v", "l", "l", "d"]);
        assert_eq!(e.body.text, " gió");
        e.undo();
        keys(&mut e, &["V", "y"]);
        assert_eq!(e.register.text, "mùa gió");
    }
    #[test]
    fn utf16_roundtrip_and_surrogate_boundary() {
        let e = editor("A🌿đ");
        assert_eq!(e.offset_from_utf16(2), 1);
        for p in [0, 1, 5, 7] {
            assert_eq!(e.offset_from_utf16(e.offset_to_utf16(p)), p);
        }
    }
    #[test]
    fn vertical_keeps_column_across_short_lines() {
        let mut e = editor("abcd\nx\nabcdef");
        e.move_to(3, false);
        e.vertical(1, false);
        assert_eq!(e.cursor, 5);
        e.vertical(1, false);
        assert_eq!(e.cursor, 10);
    }
    #[test]
    fn invalid_sequences_do_not_edit_and_search_wraps() {
        let mut e = editor("mưa và mưa");
        keys(&mut e, &["d", "q", "i"]);
        e.escape();
        assert_eq!(e.body.text, "mưa và mưa");
        assert!(e.find("mưa", false));
        assert_eq!(e.cursor, 9);
        assert!(e.find("mưa", false));
        assert_eq!(e.cursor, 0);
    }
    #[test]
    fn word_motion_works_in_insert_and_undo_resets_typing_style() {
        let mut e = editor("mùa gió");
        e.toggle_vim();
        e.move_word(false, false);
        assert_eq!(e.cursor, 5);
        e.move_word(true, true);
        assert_eq!(e.selection(), 0..5);
        e.format('b');
        assert!(e.typing_style.bold);
        e.undo();
        assert!(!e.typing_style.bold);
        assert!(!e.body.style_at(0).bold);
    }
    #[test]
    fn arbitrary_byte_positions_snap_to_whole_graphemes() {
        let e = editor("a\u{0301}👨‍👩‍👧‍👦đ");
        for p in 0..3 {
            assert_eq!(e.snap(p), 0);
        }
        for p in 3..28 {
            assert_eq!(e.snap(p), 3);
        }
        assert_eq!(e.snap(28), 28);
        assert_eq!(e.snap(29), 28);
        assert_eq!(e.snap(999), 30);
    }
    fn wrapped_editor() -> Editor {
        let mut e = editor("abcdef\nx\nabcdef");
        e.set_display_rows(
            [0..3, 3..6, 7..8, 9..12, 12..15]
                .into_iter()
                .map(|range| {
                    let stops = (range.start..=range.end)
                        .map(|i| (i, (i - range.start) as f32 * 10.))
                        .collect();
                    DisplayRow { range, stops }
                })
                .collect(),
        );
        e
    }
    #[test]
    fn jk_follow_wrapped_rows_and_preserve_x_across_short_rows() {
        let mut e = wrapped_editor();
        e.move_to(2, false);
        keys(&mut e, &["j"]);
        assert_eq!(e.cursor, 5);
        keys(&mut e, &["j"]);
        assert_eq!(e.cursor, 7);
        keys(&mut e, &["j"]);
        assert_eq!(e.cursor, 11);
        keys(&mut e, &["k", "k"]);
        assert_eq!(e.cursor, 5);
        keys(&mut e, &["h", "j", "j"]);
        assert_eq!(e.cursor, 10);
    }
    #[test]
    fn counted_screen_motions_boundaries_and_g_prefix() {
        let mut e = wrapped_editor();
        e.move_to(2, false);
        keys(&mut e, &["3", "j"]);
        assert_eq!(e.cursor, 11);
        keys(&mut e, &["3", "k"]);
        assert_eq!(e.cursor, 2);
        keys(&mut e, &["g", "j", "g", "j", "g", "j"]);
        assert_eq!(e.cursor, 11);
        keys(&mut e, &["9", "j"]);
        assert_eq!(e.cursor, 14);
        keys(&mut e, &["9", "k"]);
        assert_eq!(e.cursor, 2);
    }
    #[test]
    fn zero_targets_display_row_start_and_resets_desired_x() {
        let mut e = wrapped_editor();
        for (from, expected) in [(2, 0), (3, 3), (5, 3), (7, 7), (14, 12)] {
            e.move_to(from, false);
            keys(&mut e, &["0", "0"]);
            assert_eq!(e.cursor, expected);
            assert!(e.selection().is_empty());
            assert_eq!(e.mode, Mode::Normal);
        }
        e.move_to(2, false);
        keys(&mut e, &["j", "0", "j", "j"]);
        assert_eq!(e.cursor, 9); // Vertical navigation now keeps the row-start x.
        e.move_to(5, false);
        keys(&mut e, &["home"]);
        assert_eq!(e.cursor, 3);
    }
    #[test]
    fn zero_visual_and_operators_use_current_display_row() {
        let mut e = wrapped_editor();
        e.move_to(5, false);
        keys(&mut e, &["v", "0"]);
        assert_eq!(e.cursor, 3);
        assert_eq!(e.painted_selection(), 3..6);
        assert_eq!(e.mode, Mode::Visual);
        let mut e = wrapped_editor();
        e.move_to(5, false);
        keys(&mut e, &["d", "0"]);
        assert_eq!(e.body.text, "abcf\nx\nabcdef");
        assert_eq!(e.register.text, "de");
        e.undo();
        assert_eq!(e.body.text, "abcdef\nx\nabcdef");
        let mut e = wrapped_editor();
        e.move_to(5, false);
        keys(&mut e, &["y", "0"]);
        assert_eq!(e.cursor, 5);
        assert_eq!(e.register.text, "de");
        assert_eq!(e.body.text, "abcdef\nx\nabcdef");
        let mut e = wrapped_editor();
        e.move_to(5, false);
        keys(&mut e, &["c", "0"]);
        e.insert("ấ");
        e.escape();
        assert_eq!(e.body.text, "abcấf\nx\nabcdef");
        e.undo();
        assert_eq!(e.body.text, "abcdef\nx\nabcdef");
    }
    #[test]
    fn zero_handles_unicode_empty_rows_fallback_and_counts() {
        let mut e = editor("mùa gió 🌿\n\nx");
        let start = "mùa ".len();
        let end = "mùa gió 🌿".len();
        e.set_display_rows(vec![
            DisplayRow {
                range: 0..start,
                stops: vec![(0, 0.), (start, 30.)],
            },
            DisplayRow {
                range: start..end,
                stops: vec![(start, 0.), (end, 30.)],
            },
            DisplayRow {
                range: end + 1..end + 1,
                stops: vec![(end + 1, 0.)],
            },
            DisplayRow {
                range: end + 2..end + 3,
                stops: vec![(end + 2, 0.), (end + 3, 10.)],
            },
        ]);
        e.move_to("mùa gió ".len(), false);
        keys(&mut e, &["0"]);
        assert_eq!(e.cursor, start);
        e.move_to(end + 1, false);
        keys(&mut e, &["0"]);
        assert_eq!(e.cursor, end + 1);
        let mut e = editor("abc\ndef");
        e.move_to(6, false);
        keys(&mut e, &["0"]);
        assert_eq!(e.cursor, 4);
        let mut e = editor("");
        keys(&mut e, &["0"]);
        assert_eq!(e.cursor, 0);
        let mut e = wrapped_editor();
        keys(&mut e, &["1", "0", "l"]);
        assert_eq!(e.cursor, 5); // The zero in 10l remains part of the count.
    }
    fn type_native(e: &mut Editor, text: &str, start_ms: u64) {
        for (i, c) in text.chars().enumerate() {
            e.edit_input(
                e.selection(),
                &c.to_string(),
                InputEditKind::Typing,
                Duration::from_millis(start_ms + i as u64 * 20),
            );
        }
    }
    #[test]
    fn native_typing_undo_steps_through_words_and_redoes_exactly() {
        let mut e = editor("");
        e.key("i");
        type_native(&mut e, "Mưa rơi.", 0);
        e.undo();
        assert_eq!(e.body.text, "Mưa ");
        assert_eq!(e.mode, Mode::Insert);
        e.undo();
        assert_eq!(e.body.text, "");
        e.redo();
        assert_eq!(e.body.text, "Mưa ");
        e.redo();
        assert_eq!(e.body.text, "Mưa rơi.");
        e.undo();
        type_native(&mut e, "nắng", 500);
        e.redo();
        assert_eq!(e.body.text, "Mưa nắng"); // Editing after undo discards the old redo branch.
    }
    #[test]
    fn native_typing_breaks_on_idle_and_bounds_uninterrupted_input() {
        let mut e = editor("");
        e.key("i");
        type_native(&mut e, "abc", 0);
        type_native(&mut e, "def", 1500);
        e.undo();
        assert_eq!(e.body.text, "abc");
        e.undo();
        assert_eq!(e.body.text, "");
        let mut e = editor("");
        e.toggle_vim();
        type_native(&mut e, &"a".repeat(70), 0);
        for expected in [64, 32, 0] {
            e.undo();
            assert_eq!(e.body.text.len(), expected);
        }
        let mut e = editor("");
        e.key("i");
        for i in 0..8 {
            e.edit_input(
                e.selection(),
                "a",
                InputEditKind::Typing,
                Duration::from_millis(i * 800),
            );
        }
        e.undo();
        assert_eq!(e.body.text, "a".repeat(7)); // 5s limit without any 1s idle gap.
    }
    #[test]
    fn native_combining_marks_stay_with_the_word() {
        let mut e = editor("");
        e.key("i");
        type_native(&mut e, "a\u{0306}\u{0301}bc def", 0);
        e.undo();
        assert_eq!(e.body.text, "a\u{0306}\u{0301}bc ");
        e.undo();
        assert_eq!(e.body.text, "");
        e.redo();
        e.redo();
        assert_eq!(e.body.text, "a\u{0306}\u{0301}bc def");
    }

    #[test]
    fn native_scalar_ranges_preserve_the_base_and_return_actual_marked_range() {
        let mut e = editor("a\u{0301}b");
        e.toggle_vim();
        let range = e.from_utf16(1..2);
        let marked = e.edit_input(
            range,
            "\u{0300}",
            InputEditKind::Composition,
            Duration::ZERO,
        );
        assert_eq!(e.body.text, "a\u{0300}b");
        assert_eq!(marked, 1..3);
        assert_eq!(e.to_utf16(marked.clone()), 1..2);
        e.edit_input(
            marked,
            "\u{0302}",
            InputEditKind::Commit,
            Duration::from_secs(2),
        );
        assert_eq!(e.body.text, "a\u{0302}b");
        e.body.validate().unwrap();
        e.undo();
        assert_eq!(e.body.text, "a\u{0301}b");
        e.redo();
        assert_eq!(e.body.text, "a\u{0302}b");
        let mut e = editor("🌿a\u{0301}b");
        e.toggle_vim();
        let marked = e.edit_input(
            e.from_utf16(3..4),
            "\u{0300}",
            InputEditKind::Composition,
            Duration::ZERO,
        );
        assert_eq!(e.to_utf16(marked), 3..4);
        assert_eq!(e.body.text, "🌿a\u{0300}b");
    }
    #[test]
    fn native_ime_updates_and_commit_are_atomic_even_across_idle() {
        let mut e = editor("");
        e.key("i");
        type_native(&mut e, "Mưa ", 0);
        let start = e.cursor;
        for (text, seconds) in [("d", 1), ("đ", 3), ("đă", 6), ("đắ", 9)] {
            e.edit_input(
                start..e.body.text.len(),
                text,
                InputEditKind::Composition,
                Duration::from_secs(seconds),
            );
        }
        e.edit_input(
            start..e.body.text.len(),
            "đắng",
            InputEditKind::Commit,
            Duration::from_secs(12),
        );
        e.undo();
        assert_eq!(e.body.text, "Mưa ");
        e.undo();
        assert_eq!(e.body.text, "");
        e.redo();
        e.redo();
        assert_eq!(e.body.text, "Mưa đắng");
        let mut e = editor("Giữ nguyên ");
        e.toggle_vim();
        e.move_to(e.body.text.len(), false);
        let start = e.cursor;
        e.edit_input(
            start..start,
            "a\u{0306}\u{0301}",
            InputEditKind::Composition,
            Duration::ZERO,
        );
        e.edit_input(
            start..e.cursor,
            "",
            InputEditKind::Commit,
            Duration::from_secs(3),
        );
        e.undo();
        assert_eq!(e.body.text, "Giữ nguyên "); // Cancelling preedit creates no document edit.
    }
    #[test]
    fn native_paste_newline_deletion_and_navigation_have_separate_history() {
        let mut e = editor("");
        e.key("i");
        type_native(&mut e, "abc", 0);
        e.edit_input(
            e.selection(),
            "\n",
            InputEditKind::Atomic,
            Duration::from_millis(100),
        );
        e.edit_input(
            e.selection(),
            "Nội dung dán 🌿",
            InputEditKind::Atomic,
            Duration::from_millis(200),
        );
        e.undo();
        assert_eq!(e.body.text, "abc\n");
        e.undo();
        assert_eq!(e.body.text, "abc");
        e.edit_input(
            2..3,
            "",
            InputEditKind::Backspace,
            Duration::from_millis(300),
        );
        e.edit_input(
            1..2,
            "",
            InputEditKind::Backspace,
            Duration::from_millis(320),
        );
        e.undo();
        assert_eq!(e.body.text, "abc");
        e.finish_group();
        e.move_to(0, false);
        type_native(&mut e, "x", 500);
        e.undo();
        assert_eq!(e.body.text, "abc");
        assert_eq!(e.cursor, 0);
    }
    #[test]
    fn visual_uses_display_rows_but_linewise_operators_keep_logical_lines() {
        let mut e = wrapped_editor();
        e.move_to(1, false);
        keys(&mut e, &["v", "j"]);
        assert_eq!(e.painted_selection(), 1..5);
        keys(&mut e, &["escape", "0", "d", "j"]);
        assert_eq!(e.body.text, "abcdef");
        let mut e = wrapped_editor();
        keys(&mut e, &["d", "g", "j"]);
        assert_eq!(e.body.text, "def\nx\nabcdef");
    }
    #[test]
    fn display_end_and_insert_selection_are_consistent() {
        let mut e = wrapped_editor();
        keys(&mut e, &["g", "$"]);
        assert_eq!(e.cursor, 2);
        keys(&mut e, &["j", "j", "j"]);
        assert_eq!(e.cursor, 11);
        e.toggle_vim();
        e.move_to(1, false);
        e.display_vertical(1, true);
        assert_eq!(e.selection(), 1..4);
    }
    #[test]
    fn gg_and_reflow_reset_the_desired_horizontal_position() {
        let mut e = wrapped_editor();
        e.move_to(2, false);
        keys(&mut e, &["j", "g", "g", "j"]);
        assert_eq!(e.cursor, 3);
        e.move_to(2, false);
        keys(&mut e, &["j", "j"]);
        assert_eq!(e.cursor, 7);
        let rows = e.display_rows.clone();
        e.set_display_rows(rows);
        keys(&mut e, &["j"]);
        assert_eq!(e.cursor, 9);
    }
    #[test]
    fn counted_display_end_and_normal_pages_do_not_select_text() {
        let mut e = wrapped_editor();
        keys(&mut e, &["3", "g", "$"]);
        assert_eq!(e.cursor, 7);
        e.display_vertical(1, true);
        assert!(e.selection().is_empty());
        assert_eq!(e.cursor, 11);
    }
    #[test]
    fn gui_arrows_after_end_use_a_finite_column_and_restore_it_after_short_rows() {
        let mut e = editor("abcdef\nabc\nabcdef");
        e.set_display_rows(
            [0..6, 7..10, 11..17]
                .into_iter()
                .map(|range| {
                    let stops = (range.start..=range.end)
                        .map(|i| (i, (i - range.start) as f32 * 10.))
                        .collect();
                    DisplayRow { range, stops }
                })
                .collect(),
        );
        e.move_to(8, false);
        keys(&mut e, &["end"]);
        assert_eq!(e.cursor, 9);
        e.use_finite_vertical_column();
        keys(&mut e, &["up"]);
        assert_eq!(e.cursor, 2, "Arrow Up stuck to the longer row's end");
        e.use_finite_vertical_column();
        keys(&mut e, &["down"]);
        assert_eq!(e.cursor, 9);
        e.move_to(4, false);
        for expected in [9, 15] {
            e.use_finite_vertical_column();
            keys(&mut e, &["down"]);
            assert_eq!(e.cursor, expected);
        }
        // Explicit Vim motions still honor their end-of-line goal.
        e.move_to(8, false);
        keys(&mut e, &["$", "k"]);
        assert_eq!(e.cursor, 5);
        // Switching from a Vim end-goal to arrows uses the position now shown.
        e.use_finite_vertical_column();
        keys(&mut e, &["down", "down"]);
        assert_eq!(e.cursor, 16);
    }
    #[test]
    fn append_targets_current_display_row_and_is_one_undo_group() {
        let mut e = wrapped_editor();
        e.move_to(1, false);
        keys(&mut e, &["A"]);
        assert_eq!(e.cursor, 3);
        assert_eq!(e.mode, Mode::Insert);
        assert_eq!(e.caret_affinity, CaretAffinity::Trailing);
        e.display_vertical(-1, false);
        assert_eq!(e.cursor, 3); // Already on the first displayed row.
        e.insert("ấ");
        assert_eq!(e.body.text, "abcấdef\nx\nabcdef");
        assert_eq!(e.caret_affinity, CaretAffinity::Leading);
        e.escape();
        e.undo();
        assert_eq!(e.body.text, "abcdef\nx\nabcdef");
        let mut e = wrapped_editor();
        e.move_to(3, false);
        keys(&mut e, &["A"]);
        assert_eq!(e.cursor, 6);
        let mut e = wrapped_editor();
        e.move_to(14, false);
        keys(&mut e, &["A"]);
        assert_eq!(e.cursor, 15);
    }
    #[test]
    fn append_handles_empty_rows_and_logical_fallback() {
        let mut e = editor("\nabc");
        e.set_display_rows(vec![
            DisplayRow {
                range: 0..0,
                stops: vec![(0, 0.)],
            },
            DisplayRow {
                range: 1..4,
                stops: vec![(1, 0.), (4, 30.)],
            },
        ]);
        keys(&mut e, &["A"]);
        assert_eq!(e.cursor, 0);
        e.insert("x");
        assert_eq!(e.body.text, "x\nabc");
        let mut e = editor("abc\ndef");
        keys(&mut e, &["A"]);
        assert_eq!(e.cursor, 3);
        let mut e = editor("");
        keys(&mut e, &["A"]);
        e.insert("ấ");
        assert_eq!(e.body.text, "ấ");
    }
}
