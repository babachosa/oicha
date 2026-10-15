# Kết quả sửa review — 2026-10-14

**Đã xử lý cả sáu finding của review trước và kiểm tra trên release Windows x64.** Hai lỗi P1 có kiểm thử hồi quy đi qua adapter/lifecycle thật; các phần gõ phát sinh được kiểm tra trong file đã lưu.

| Finding ban đầu | Cách xử lý | Bằng chứng |
| --- | --- | --- |
| P1: mất phần gõ khi tải sách khác | Giữ book đã tải trong trạng thái chờ, đồng bộ editor cũ và lưu mọi revision mới trước khi thay book; yêu cầu đóng được ưu tiên | Native barrier; marker có trong file cũ, không rò vào book mới |
| P1: range IME làm mất chữ nền/panic | Tách native scalar replacement khỏi grapheme motion của Vim; dùng inserted range thực tế để tính marked range/cursor | NFD đổi riêng dấu sau emoji, UTF-16 marked range/selection, commit và undo đều đạt |
| P2: tính vị trí bậc hai | Bảng glyph → x với binary lookup; rows và position metadata cache/chia sẻ theo paragraph | So sánh semantic với GPUI; 40.000 ký tự giảm từ 264,65 ms xuống 8,34–8,74 ms trên release |
| P2: recovery hỏng chặn mở file tốt | Giữ recovery hỏng tại tên riêng và mở primary với thông báo; nếu không thể giữ riêng thì chặn ghi đường dẫn đó, yêu cầu Save As | Recovery bị cắt cụt/model sai/vượt 64 MiB, lỗi sharing Windows; primary và sidecar được bảo vệ |
| P2: đóng khi tải bị treo | Luồng drain pending thống nhất cho save/load/close, cả nhánh tải thành công và lỗi | Hai process release tự đóng, exit 0, marker close và nội dung sửa trước close được kiểm tra |
| P2: grid dependency có advisory | Vendor API 0.18 với kiểm tra checked-add/checked-mul theo bản sửa upstream; giữ giấy phép và provenance; CI kiểm tra hash và chạy test release | Overflow row/column không làm đổi state, cả hai memory orders; valid growth đạt; cargo tree xác nhận dùng bản local |

Đã thêm check dependency vào CI. 735 package/version registry đã được kiểm tra. Các advisory INFO Unmaintained của GPUI graph vẫn được ghi rõ trong DEPENDENCY_POLICY.md; chúng là rủi ro bảo trì còn theo dõi, không bị đổi tên thành “đã sửa”. Advisory mới, scan không đầy đủ hoặc source backport đổi ngoài hồ sơ đều làm checker thất bại.

Kết quả kiểm tra: format PASS, 39 test tự động PASS, 60 tình huống Vim PASS, Clippy PASS, build release PASS, hai native scenario PASS và 5 XML part của mỗi DOCX đều parse được.

Bản chạy: [Oicha.exe](dist/Oicha.exe).
SHA-256 của bản sửa review trước khi cập nhật popup (hash bản hiện tại xem VALIDATION.md): `1A97E1D8E4AB7A804FB7CE8DD4EB8690AF53FC3458BFFC7497FAB583985EA923`.

Chi tiết: [VALIDATION.md](VALIDATION.md), [native evidence](artifacts/fixes-native-release/results.json), [dependency evidence](artifacts/fixes-dependency-audit-final.json).

Bản này đạt các lỗi hồi quy và phạm vi kiểm tra nêu trên. Chưa xác nhận mọi bộ gõ thực tế, mọi tình huống mất điện/disk-full, accessibility, macOS/Linux hoặc full compatibility Word/Vim; executable chưa ký số. Các giới hạn đó không bị che bằng kết quả PASS tự động.
