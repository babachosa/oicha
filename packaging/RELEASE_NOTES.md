Oicha 0.3.1 — thay ký tự bằng Vim r

- Thêm r{ký tự} trong Normal: rx thay ký tự dưới con trỏ bằng x; 3ra thay 3 ký tự bằng a. Sau khi thay vẫn ở Normal.
- Visual + r thay các ký tự trong vùng chọn, giữ dấu xuống đoạn. Hỗ trợ ký tự Unicode, giữ định dạng và thực hiện trong một bước undo/redo.
- Esc hủy lệnh đang chờ. Phím tắt, đổi focus và nhấp chuột cũng hủy để tránh thay nhầm ký tự khi quay lại viết.
- Bổ sung hướng dẫn tiếng Việt và tiếng Anh trong F1.
- Cập nhật thư viện TLS để vá RUSTSEC-2026-0381 và RUSTSEC-2026-0382.
- Undo: u. Redo: Ctrl+R. Bản này không thêm U hoặc chế độ R.
- Giữ nguyên định dạng kho .oicha của 0.3.0. File 0.2.x vẫn không được mở/chuyển đổi; hãy xuất TXT/DOCX bằng bản cũ trước khi nâng cấp nếu cần.

Oicha 0.3.1 — Vim character replacement

- Add Normal r{char}, including counts such as 3ra and Visual replacement. Stay in Normal after replacing.
- Preserve Unicode graphemes and formatting. Each replacement is one undo/redo transaction.
- Escape, GUI shortcuts, focus changes and mouse repositioning cancel a pending replacement.
- Add Vietnamese and English F1 help. Undo is u; redo is Ctrl+R. U and full R mode are not included.
- Update TLS dependencies to fix RUSTSEC-2026-0381 and RUSTSEC-2026-0382.
- Uses the same .oicha storage format as 0.3.0. Legacy 0.2.x files cannot be opened or converted; export TXT/DOCX with the older app first if needed.

Windows x64. The installer is unsigned with Authenticode; update metadata is signed with Ed25519.
