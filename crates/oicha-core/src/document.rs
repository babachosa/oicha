use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::ops::Range;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledRange {
    pub range: Range<usize>,
    pub style: TextStyle,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichText {
    pub text: String,
    pub runs: Vec<StyledRange>,
}

impl RichText {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        let runs = if text.is_empty() {
            vec![]
        } else {
            vec![StyledRange {
                range: 0..text.len(),
                style: TextStyle::default(),
            }]
        };
        Self { text, runs }
    }

    pub fn validate(&self) -> Result<()> {
        let mut end = 0;
        for run in &self.runs {
            ensure!(
                run.range.start == end && run.range.end > end,
                "Invalid style range"
            );
            ensure!(
                run.range.end <= self.text.len()
                    && self.text.is_char_boundary(run.range.start)
                    && self.text.is_char_boundary(run.range.end),
                "Style splits UTF-8 text"
            );
            end = run.range.end;
        }
        ensure!(end == self.text.len(), "Styles do not cover the text");
        Ok(())
    }

    pub fn style_at(&self, index: usize) -> TextStyle {
        self.runs
            .iter()
            .find(|r| r.range.contains(&index))
            .map(|r| r.style)
            .or_else(|| self.runs.last().map(|r| r.style))
            .unwrap_or_default()
    }

    fn push(runs: &mut Vec<StyledRange>, range: Range<usize>, style: TextStyle) {
        if range.is_empty() {
            return;
        }
        if let Some(last) = runs.last_mut()
            && last.style == style
            && last.range.end == range.start
        {
            last.range.end = range.end;
            return;
        }
        runs.push(StyledRange { range, style });
    }

    pub fn replace(&mut self, range: Range<usize>, value: &str, style: TextStyle) {
        assert!(range.start <= range.end && range.end <= self.text.len());
        assert!(self.text.is_char_boundary(range.start) && self.text.is_char_boundary(range.end));
        let mut runs = vec![];
        for run in &self.runs {
            Self::push(
                &mut runs,
                run.range.start.min(range.start)..run.range.end.min(range.start),
                run.style,
            );
        }
        Self::push(&mut runs, range.start..range.start + value.len(), style);
        for run in &self.runs {
            let start = run.range.start.max(range.end);
            let end = run.range.end.max(range.end);
            Self::push(
                &mut runs,
                start - range.end + range.start + value.len()
                    ..end - range.end + range.start + value.len(),
                run.style,
            );
        }
        self.text.replace_range(range, value);
        self.runs = runs;
    }

    pub fn format(&mut self, range: Range<usize>, mut change: impl FnMut(&mut TextStyle)) {
        let mut runs = vec![];
        for run in &self.runs {
            let start = run.range.start.max(range.start);
            let end = run.range.end.min(range.end);
            if start >= end {
                Self::push(&mut runs, run.range.clone(), run.style);
                continue;
            }
            Self::push(&mut runs, run.range.start..start, run.style);
            let mut style = run.style;
            change(&mut style);
            Self::push(&mut runs, start..end, style);
            Self::push(&mut runs, end..run.range.end, run.style);
        }
        self.runs = runs;
    }

    /// Replace a span while retaining the replacement's individual styles.
    pub fn replace_rich(&mut self, range: Range<usize>, value: &Self) {
        assert!(range.start <= range.end && range.end <= self.text.len());
        assert!(self.text.is_char_boundary(range.start) && self.text.is_char_boundary(range.end));
        let mut runs = Vec::new();
        for run in &self.runs {
            Self::push(
                &mut runs,
                run.range.start.min(range.start)..run.range.end.min(range.start),
                run.style,
            );
        }
        for run in &value.runs {
            Self::push(
                &mut runs,
                range.start + run.range.start..range.start + run.range.end,
                run.style,
            );
        }
        for run in &self.runs {
            let start = run.range.start.max(range.end);
            let end = run.range.end.max(range.end);
            Self::push(
                &mut runs,
                start - range.end + range.start + value.text.len()
                    ..end - range.end + range.start + value.text.len(),
                run.style,
            );
        }
        if self.text[range.clone()] != value.text {
            self.text.replace_range(range, &value.text);
        }
        self.runs = runs;
    }

    pub fn slice(&self, range: Range<usize>) -> Self {
        let mut result = Self {
            text: self.text[range.clone()].into(),
            ..Default::default()
        };
        for run in &self.runs {
            let start = run.range.start.max(range.start);
            let end = run.range.end.min(range.end);
            if start < end {
                Self::push(
                    &mut result.runs,
                    start - range.start..end - range.start,
                    run.style,
                );
            }
        }
        result
    }

    pub fn words(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub id: u64,
    pub title: String,
    pub body: RichText,
    pub synopsis: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manuscript {
    pub version: u32,
    pub title: String,
    pub author: String,
    pub chapters: Vec<Chapter>,
    pub daily_goal: usize,
    pub revision: u64,
    pub next_id: u64,
}

impl Manuscript {
    pub fn new() -> Self {
        Self {
            version: FORMAT_VERSION,
            title: "Tiểu thuyết chưa đặt tên".into(),
            author: String::new(),
            chapters: vec![Chapter {
                id: 1,
                title: "Chương 1".into(),
                body: RichText::default(),
                synopsis: String::new(),
            }],
            daily_goal: 1000,
            revision: 0,
            next_id: 2,
        }
    }

    pub fn demo() -> Self {
        let mut book = Self::new();
        book.title = "Mùa gió đi qua".into();
        book.chapters[0].title = "Người trở về".into();
        book.chapters[0].synopsis = "An trở về thị trấn sau mười năm. Một lá thư cũ khiến cô phải đối mặt với điều mình từng bỏ lại.".into();
        book.chapters[0].body = RichText::plain(
            "Chiều hôm ấy, An trở về khi cơn mưa vừa dứt.\n\nCon đường dẫn vào thị trấn vẫn hẹp như trong trí nhớ. Những mái nhà màu ngói sẫm nằm im sau hàng bằng lăng, còn gió thì mang theo mùi đất ẩm và một điều gì đó rất cũ — mùi của những mùa hè cô đã đánh mất.\n\nCô dừng lại trước cánh cổng. Trên bậc thềm, một chiếc lá khô xoay tròn, chậm rãi, như đang chờ ai đó gọi tên.\n\n“Cuối cùng con cũng về.”\n\nGiọng mẹ vang lên từ phía sau. An quay lại. Trong khoảnh khắc ấy, mười năm bỗng co lại thành khoảng cách giữa hai người, ngắn đến mức chỉ cần đưa tay ra là chạm được.\n\nNhưng cô vẫn đứng yên.\n\nTrong túi áo khoác là lá thư chưa từng được gửi. Mép giấy đã mềm đi vì những lần cô mở ra rồi lại gấp vào. Một dòng chữ ở cuối thư vẫn còn rõ:\n\nCó những nơi ta rời đi, nhưng chưa bao giờ thật sự rời khỏi.",
        );
        let start = book.chapters[0].body.text.find("Có những nơi").unwrap_or(0);
        let end = book.chapters[0].body.text.len();
        book.chapters[0]
            .body
            .format(start..end, |s| s.italic = true);
        for (id, title) in [(2, "Lá thư chưa gửi"), (3, "Những ngày không tên")] {
            book.chapters.push(Chapter {
                id,
                title: title.into(),
                body: RichText::default(),
                synopsis: String::new(),
            });
        }
        book.next_id = 4;
        book
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == FORMAT_VERSION,
            "Unsupported manuscript version {}",
            self.version
        );
        ensure!(
            !self.chapters.is_empty(),
            "A manuscript must have at least one chapter"
        );
        let mut ids = std::collections::HashSet::new();
        for chapter in &self.chapters {
            ensure!(
                ids.insert(chapter.id) && chapter.id < self.next_id,
                "Invalid chapter ID"
            );
            chapter.body.validate()?;
        }
        Ok(())
    }

    pub fn words(&self) -> usize {
        self.chapters.iter().map(|c| c.body.words()).sum()
    }
    pub fn add_chapter(&mut self) -> usize {
        self.chapters.push(Chapter {
            id: self.next_id,
            title: format!("Chương {}", self.chapters.len() + 1),
            body: RichText::default(),
            synopsis: String::new(),
        });
        self.next_id += 1;
        self.revision += 1;
        self.chapters.len() - 1
    }
}

impl Default for Manuscript {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn styles_survive_unicode_edits() {
        let mut text = RichText::plain("Xin chào 🌿!");
        text.format(4..9, |s| s.bold = true);
        text.replace(
            4..9,
            "đời",
            TextStyle {
                italic: true,
                ..Default::default()
            },
        );
        assert_eq!(text.text, "Xin đời 🌿!");
        text.validate().unwrap();
        assert!(text.style_at(4).italic);
        assert!(!text.style_at(0).bold);
        text.replace(0..text.text.len(), "", TextStyle::default());
        text.validate().unwrap();
    }
    #[test]
    fn format_and_slice_preserve_mixed_runs() {
        let mut text = RichText::plain("abcdef");
        text.format(1..5, |s| s.bold = true);
        text.format(2..4, |s| s.italic = true);
        let part = text.slice(1..5);
        part.validate().unwrap();
        assert_eq!(part.runs.len(), 3);
        text.replace(3..3, "XYZ", TextStyle::default());
        text.validate().unwrap();
        assert!(text.style_at(6).italic);
    }
    #[test]
    fn mixed_unicode_edit_sequences_keep_styles_valid() {
        let mut text = RichText::plain("Mùa gió 👨‍👩‍👧‍👦 đi qua.\n");
        let mut state = 0x0c1ca_u64;
        let values = ["đời", "🌿", "a\u{0301}", "\n", "", " "];
        for i in 0..500 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let boundaries: Vec<_> = text
                .text
                .char_indices()
                .map(|(p, _)| p)
                .chain([text.text.len()])
                .collect();
            let a = boundaries[state as usize % boundaries.len()];
            let b = boundaries[(state >> 32) as usize % boundaries.len()];
            let range = a.min(b)..a.max(b);
            if i % 3 == 0 {
                text.format(range, |s| s.bold = !s.bold);
            } else {
                text.replace(
                    range,
                    values[i % values.len()],
                    TextStyle {
                        italic: i % 2 == 0,
                        ..Default::default()
                    },
                );
            }
            text.validate().unwrap();
            for run in &text.runs {
                assert!(
                    text.text.is_char_boundary(run.range.start)
                        && text.text.is_char_boundary(run.range.end)
                );
            }
        }
    }
}
