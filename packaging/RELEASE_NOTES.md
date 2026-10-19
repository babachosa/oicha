Oicha 0.3.0 — kho tiểu thuyết và thông báo mới

- Một file .oicha chứa nhiều dự án. Trang chủ giúp tạo, chọn và xóa dự án qua menu ba chấm có xác nhận.
- Định dạng kho mới là breaking change: không mở hoặc chuyển đổi file .oicha của bản 0.2.x. File cũ được giữ nguyên. Hãy xuất TXT/DOCX bằng bản cũ trước khi nâng cấp nếu cần giữ nội dung để sử dụng bên ngoài app.
- dd xóa hàng đang hiển thị, kể cả đoạn tự xuống hàng; giữ chỗ viết trống khi xóa hàng cuối.
- Nút đóng hỏi Lưu / Không lưu / Hủy khi còn thay đổi chưa lưu. Các nút cửa sổ sát mép trên; giao diện gọn hơn.
- Không tự tạo lại kho đã bị xóa hoặc di chuyển. Lần đầu dùng app vẫn mở Trang chủ bình thường.
- Thông báo ngắn gọn, thống nhất Việt/Anh; chi tiết kỹ thuật chỉ hiện khi mở Chi tiết. Hộp thoại dài cuộn được trong cửa sổ nhỏ.
- Cảnh báo lệnh Vim không chặn tự lưu. Các dự án được lưu trong giao dịch trên cùng một file, không sinh file phụ.

Oicha 0.3.0 — project libraries and clearer notifications

- Keep multiple novels in one .oicha file. Use Home to create, switch or delete projects with confirmation.
- Breaking storage change: 0.2.x .oicha files are preserved but cannot be opened or converted. Export TXT/DOCX with the older app before upgrading if you need the text outside Oicha.
- dd deletes the displayed row and retains writing space at the end. Closing offers Save / Don't Save / Cancel for unsaved edits.
- Deleted libraries stay deleted. New users start on Home without a missing-file warning.
- Short, fully localized notices separate technical details from guidance; long dialogs scroll. Vim notices no longer pause autosave.

Windows x64. The installer is unsigned with Authenticode; update metadata is signed with Ed25519.
