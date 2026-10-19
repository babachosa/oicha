//! User-facing copy is translated when rendered; backend errors stay in details.
use crate::settings::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MissingLibrary,
    OpenLibrary,
    ProjectList,
    Save,
    SaveAs,
    DeleteProject,
    OpenProject,
    Export,
    Close,
    Settings,
    CheckUpdate,
    DownloadUpdate,
    InstallUpdate,
    UpdateFailed,
    UpdateComplete,
    Clipboard,
    Unsaved,
    UnsupportedCommand,
    LastChapter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Error,
    Warning,
    Success,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub kind: Kind,
    pub details: Option<String>,
}

impl Notification {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            details: None,
        }
    }

    pub fn failure(kind: Kind, error: &anyhow::Error) -> Self {
        log::warn!("{kind:?}: {error:#}");
        Self::with_details(kind, format!("{error:#}"))
    }

    pub fn with_details(kind: Kind, details: String) -> Self {
        // Keep diagnostic panels bounded, even for errors containing remote data.
        let mut details = details;
        if details.len() > 16_384 {
            let mut end = 16_384;
            while !details.is_char_boundary(end) {
                end -= 1;
            }
            details.truncate(end);
            details.push('…');
        }
        Self {
            kind,
            details: (!details.trim().is_empty()).then_some(details),
        }
    }

    pub fn open_library(error: &anyhow::Error) -> Self {
        let missing = error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        });
        Self::failure(
            if missing {
                Kind::MissingLibrary
            } else {
                Kind::OpenLibrary
            },
            error,
        )
    }

    pub fn tone(&self) -> Tone {
        match self.kind {
            Kind::MissingLibrary | Kind::Clipboard | Kind::Unsaved | Kind::LastChapter => {
                Tone::Warning
            }
            Kind::UpdateComplete => Tone::Success,
            Kind::UnsupportedCommand => Tone::Info,
            _ => Tone::Error,
        }
    }

    pub fn blocks_autosave(&self) -> bool {
        // Only a failed write needs explicit retry. A Vim or validation notice
        // must never disable persistence of subsequent edits.
        self.kind == Kind::Save
    }

    pub fn copy(&self, language: Language) -> (&'static str, &'static str) {
        let (vi_title, vi_body, en_title, en_body) = match self.kind {
            Kind::MissingLibrary => (
                "Không tìm thấy kho",
                "File có thể đã bị xóa hoặc di chuyển. Chọn Mở kho hoặc Dự án mới.",
                "Library not found",
                "The file may have been deleted or moved. Choose Open library or New project.",
            ),
            Kind::OpenLibrary => (
                "Chưa mở được kho",
                "Kiểm tra file rồi thử lại, hoặc chọn kho khác.",
                "Couldn't open the library",
                "Check the file and try again, or choose another library.",
            ),
            Kind::ProjectList => (
                "Chưa tải được danh sách dự án",
                "Thử mở lại kho để làm mới danh sách.",
                "Couldn't load the project list",
                "Reopen the library to refresh the list.",
            ),
            Kind::Save => (
                "Chưa lưu được thay đổi",
                "Nội dung vẫn đang mở. Thử lưu lại hoặc chọn Lưu thành… để lưu ở nơi khác.",
                "Changes couldn't be saved",
                "Your text is still open. Try saving again or use Save as… to choose another location.",
            ),
            Kind::SaveAs => (
                "Chưa lưu được kho mới",
                "Kiểm tra vị trí lưu rồi thử lại. Kho hiện tại vẫn được giữ nguyên.",
                "Couldn't save the new library",
                "Check the destination and try again. Your current library is unchanged.",
            ),
            Kind::DeleteProject => (
                "Chưa xóa được dự án",
                "Dự án vẫn được giữ lại. Hãy thử lại.",
                "Couldn't delete the project",
                "The project has been kept. Please try again.",
            ),
            Kind::OpenProject => (
                "Chưa mở được dự án",
                "Thử chọn lại dự án hoặc mở lại kho.",
                "Couldn't open the project",
                "Select the project again or reopen the library.",
            ),
            Kind::Export => (
                "Chưa xuất được file",
                "Kiểm tra vị trí xuất rồi thử lại. Bản thảo vẫn được giữ nguyên.",
                "Couldn't export the file",
                "Check the destination and try again. Your manuscript is unchanged.",
            ),
            Kind::Close => (
                "Chưa thể đóng bản thảo",
                "Thử lại, hoặc chọn Hủy để quay về bản thảo.",
                "Couldn't close the manuscript",
                "Try again, or choose Cancel to return to your manuscript.",
            ),
            Kind::Settings => (
                "Chưa lưu được ngôn ngữ",
                "Cài đặt cũ vẫn được giữ. Hãy thử chọn lại.",
                "Couldn't save the language",
                "Your previous setting is unchanged. Please select the language again.",
            ),
            Kind::CheckUpdate => (
                "Chưa kiểm tra được cập nhật",
                "Kiểm tra kết nối mạng rồi thử lại.",
                "Couldn't check for updates",
                "Check your internet connection and try again.",
            ),
            Kind::DownloadUpdate => (
                "Chưa tải được bản cập nhật",
                "Kiểm tra kết nối và dung lượng trống rồi thử lại.",
                "Couldn't download the update",
                "Check your connection and free disk space, then try again.",
            ),
            Kind::InstallUpdate => (
                "Chưa cài được bản cập nhật",
                "Mở Oicha từ bản đã cài trên máy rồi thử lại.",
                "Couldn't install the update",
                "Open the installed copy of Oicha and try again.",
            ),
            Kind::UpdateFailed => (
                "Cập nhật chưa hoàn tất",
                "Bạn có thể tiếp tục viết và thử cập nhật lại trong Cài đặt.",
                "The update didn't finish",
                "You can keep writing and retry the update in Settings.",
            ),
            Kind::UpdateComplete => (
                "Đã cập nhật Oicha",
                "Bạn có thể tiếp tục viết.",
                "Oicha is up to date",
                "You're ready to keep writing.",
            ),
            Kind::Clipboard => (
                "Chưa truy cập được bảng nhớ tạm",
                "Chưa có nội dung nào bị cắt. Thử sao chép, cắt hoặc dán lại.",
                "Clipboard unavailable",
                "No text was cut. Try copying, cutting or pasting again.",
            ),
            Kind::Unsaved => (
                "Bản thảo chưa được lưu",
                "Dùng :wq để lưu và đóng, hoặc tiếp tục viết.",
                "The manuscript has unsaved changes",
                "Use :wq to save and close, or keep writing.",
            ),
            Kind::UnsupportedCommand => (
                "Lệnh chưa được hỗ trợ",
                "Nhấn F1 để xem các lệnh có sẵn.",
                "Command not supported",
                "Press F1 to see available commands.",
            ),
            Kind::LastChapter => (
                "Cần giữ lại một chương",
                "Bạn có thể xóa nội dung hoặc đổi tên chương này.",
                "Keep at least one chapter",
                "You can clear this chapter's text or rename it.",
            ),
        };
        match language {
            Language::Vi => (vi_title, vi_body),
            Language::En => (en_title, en_body),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Kind] = &[
        Kind::MissingLibrary,
        Kind::OpenLibrary,
        Kind::ProjectList,
        Kind::Save,
        Kind::SaveAs,
        Kind::DeleteProject,
        Kind::OpenProject,
        Kind::Export,
        Kind::Close,
        Kind::Settings,
        Kind::CheckUpdate,
        Kind::DownloadUpdate,
        Kind::InstallUpdate,
        Kind::UpdateFailed,
        Kind::UpdateComplete,
        Kind::Clipboard,
        Kind::Unsaved,
        Kind::UnsupportedCommand,
        Kind::LastChapter,
    ];

    #[test]
    fn backend_details_never_leak_into_copy_in_either_language() {
        let raw = "Cannot access C:\\Users\\writer\\secret.oicha: OS error 2";
        for &kind in ALL {
            let notice = Notification::with_details(kind, raw.into());
            let vi = notice.copy(Language::Vi);
            let en = notice.copy(Language::En);
            for (title, body) in [vi, en] {
                assert!(!title.is_empty() && !body.is_empty());
                assert!(title.chars().count() < 60 && body.chars().count() < 160);
                assert!(!format!("{title} {body}").contains(raw));
                assert!(!format!("{title} {body}").contains("secret.oicha"));
            }
            assert_ne!(vi, en);
            assert!(format!("{} {}", en.0, en.1).replace('…', "...").is_ascii());
            assert_eq!(notice.details.as_deref(), Some(raw));
            // Switching language translates an existing notification, not just future ones.
            assert_eq!(notice.copy(Language::Vi), vi);
        }
    }

    #[test]
    fn missing_library_uses_typed_cause_and_keeps_diagnostics_separate() {
        let error = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
            .context("Cannot access library writer.oicha");
        let notice = Notification::open_library(&error);
        assert_eq!(notice.kind, Kind::MissingLibrary);
        assert_eq!(notice.tone(), Tone::Warning);
        assert!(notice.details.unwrap().contains("writer.oicha"));
        assert_eq!(
            Notification::open_library(&anyhow::anyhow!("invalid database")).kind,
            Kind::OpenLibrary
        );
    }

    #[test]
    fn diagnostics_are_bounded_on_utf8_boundaries_and_empty_details_are_hidden() {
        let notice = Notification::with_details(Kind::Export, "ế".repeat(20_000));
        assert!(notice.details.unwrap().len() <= 16_387);
        assert!(
            Notification::with_details(Kind::Export, "  ".into())
                .details
                .is_none()
        );
        assert_eq!(
            Notification::new(Kind::UpdateComplete).tone(),
            Tone::Success
        );
    }

    #[test]
    fn only_a_failed_write_blocks_automatic_retry() {
        for &kind in ALL {
            assert_eq!(
                Notification::new(kind).blocks_autosave(),
                kind == Kind::Save
            );
        }
    }
}
