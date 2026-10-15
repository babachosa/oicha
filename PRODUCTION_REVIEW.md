# Production review — 2026-10-14

**Cập nhật sau sửa: sáu finding bên dưới đã được xử lý và kiểm tra release. Xem [FIX_VALIDATION.md](FIX_VALIDATION.md) và VALIDATION.md cho candidate hiện tại.**

**Kết luận tại thời điểm review ban đầu: chưa nên phát hành production.** Có hai lỗi P1 đã tái hiện liên quan mất phần gõ khi chuyển bản thảo và sai vị trí IME dẫn tới panic. Các kiểm thử hiện tại đạt nhưng chưa bao phủ những trường hợp này.

Review này không sửa mã sản phẩm hoặc thay executable. Mã thử nghiệm nằm trong `artifacts/`; bản sao giao diện dùng tên `oicha-review` và bản thảo riêng. Candidate Windows x64 là `dist/Oicha.exe`, SHA-256 `2A44A4691096A4691A4E0D0F0D33233FE9AD5B63EC476488348A6E94A500931E`; build release từ mã hiện tại có cùng hash.

## Findings

### 1. [P1] Phần gõ trong lúc tải file bị bỏ mà không lưu

Vị trí: [app.rs:715](crates/oicha-app/src/app.rs#L715), [app.rs:666](crates/oicha-app/src/app.rs#L666).

`load()` đặt `io_busy` và đọc trên background executor. Editor hiện tại vẫn nhận input; trong lúc đó autosave bị chặn bởi `io_busy`. Khi đọc xong, callback gán `this.book = book` và xây lại editor, không kiểm tra revision của bản thảo cũ có thay đổi sau lúc bắt đầu tải hay không.

Tái hiện trên bản sao native release: mô phỏng đọc file chậm bằng cách thêm 500 ms trong worker; nhập `TYPED_WHILE_LOADING` vào editor cũ trong lúc tải. Trước callback, marker tồn tại trong book. Sau callback, marker không có trong book mới và không được lưu trong file cũ. Điều kiện mất dữ liệu được ghi là `load_loses_concurrent_input: true`.

Cần khóa các thao tác sửa trong giai đoạn chuyển file hoặc giữ editor cũ, kiểm tra lại revision và lưu phần phát sinh trước khi đổi book. Không được chỉ bỏ input âm thầm. Kiểm thử cần có barrier xác định thay cho dựa vào tốc độ ổ đĩa.

### 2. [P1] Range UTF-16 hợp lệ của IME có thể làm mất ký tự nền và tạo marked range sai UTF-8

Vị trí: [text_editor.rs:727](crates/oicha-app/src/text_editor.rs#L727), [editor.rs:424](crates/oicha-core/src/editor.rs#L424), [editor.rs:1255](crates/oicha-core/src/editor.rs#L1255).

Adapter lấy `start` từ range UTF-16 đã đổi sang byte offset. `Editor::replace()` sau đó kéo cả hai đầu range về biên grapheme, nhưng adapter vẫn tính marked range và selection từ `start` cũ. Hai lớp đang dùng hai định nghĩa range khác nhau.

Probe với dependency được giữ từ Cargo.lock: body `a\u{0301}b`, native replacement range UTF-16 `1..2` (chỉ dấu sắc), preedit mới `\u{0300}` (dấu huyền). Kết quả phải là `a\u{0300}b`; hiện tại thành `\u{0300}b`, mất chữ `a`. Marked range mà adapter tính là byte `1..3`; chuyển range đó về UTF-16 panic vì byte 1 nằm giữa ký tự UTF-8. Đây là phép gọi hợp lệ theo chỉ số scalar UTF-16; không phải thử nghiệm con trỏ mouse ngoài biên.

Probe gọi cùng các thao tác core và phép tính range của adapter, bắt panic để thu bằng chứng. Nó chưa chứng minh mọi bộ gõ Windows sẽ gửi range này. Tuy nhiên hợp đồng native input hiện không xử lý an toàn range hợp lệ này, nên chưa đủ để phát hành.

Cần tách thay văn bản bằng native range khỏi chuyển động/xóa theo grapheme của Vim; giữ scalar range chính xác và trả range thực tế sau sửa cho IME metadata. Thêm kiểm thử NFD/NFC, cập nhật riêng dấu, range có emoji/surrogate, cursor trong preedit và commit/cancel.

### 3. [P2] Tính vị trí hàng/con trỏ có độ phức tạp bậc hai với đoạn dài

Vị trí: [text_editor.rs:484](crates/oicha-app/src/text_editor.rs#L484), [text_editor.rs:498](crates/oicha-app/src/text_editor.rs#L498).

`sync_display_rows()` gọi GPUI `x_for_index()` cho từng grapheme. Trong GPUI 0.2.2, `x_for_index()` duyệt glyph từ đầu mỗi lần. Cache bị làm mới theo generation của toàn chương, nên một lần nhập lại dựng toàn bộ mapping. Paint và tính vị trí con trỏ cũng dùng cách tra này.

Đo trong native release, font Georgia 20px, width 580px, một paragraph ASCII có khoảng trắng; sau một lần chèn `ấ`, đo riêng `ensure_layout` + `sync_display_rows`:

| Ký tự trong paragraph | Thời gian một lần sửa |
| --- | ---: |
| 2.000 | 1,04 ms |
| 10.000 | 15,18 ms |
| 40.000 | 264,65 ms |

Đây là một lần đo trên máy hiện tại, không phải percentile benchmark và chưa tính toàn bộ paint hay thời gian gõ đầu-cuối. Số liệu debug tương ứng là 3,30 / 95,98 / 1.723,38 ms. Không dùng số debug để mô tả bản production.

Cần dựng bảng byte offset → x một lần từ glyph, tra bằng binary search hoặc iterator, và cache theo paragraph. Thêm benchmark đoạn dài và nhiều chương, đo cả input-to-paint trên release trước khi chốt giới hạn dung lượng hỗ trợ.

### 4. [P2] Recovery hỏng khiến file chính còn hợp lệ không mở được

Vị trí: [storage.rs:110](crates/oicha-core/src/storage.rs#L110).

Sau khi đã parse và validate file chính, `Store::open()` dùng `?` khi đọc/parse/validate sidecar `.recovery`. Probe tạo file chính hợp lệ rồi thêm checkpoint bị cắt cụt: `Store::open()` trả `Invalid recovery file; original manuscript has been preserved`, thay vì cho mở file chính. Dữ liệu chưa bị ghi đè, nhưng người dùng phải can thiệp file thủ công để mở được.

Cần giữ sidecar để điều tra và cung cấp lựa chọn mở file chính hoặc khôi phục một bản riêng; lỗi sidecar không nên làm mất đường truy cập file chính hợp lệ. Kiểm thử recovery bị cắt cụt, vượt giới hạn và model không hợp lệ.

### 5. [P2] Yêu cầu đóng khi đang tải bị treo trong pending state

Vị trí: [app.rs:460](crates/oicha-app/src/app.rs#L460), [app.rs:709](crates/oicha-app/src/app.rs#L709).

`request_close()` đặt `Pending::Close` rồi trả khi `io_busy`. Callback của `load()` không xử lý lại pending close. Native release probe gửi close ngay sau khi bắt đầu tải, chờ hoàn thành và ghi nhận `close_request_stranded_after_load: true`; cửa sổ vẫn còn mở. Callback tải thất bại cũng không nối tiếp xử lý đóng.

Cần một state machine chung cho save/open/new/close và drain pending khi I/O kết thúc, với kiểm tra revision trước các hành động bỏ editor.

### 6. [P2] Dependency đang chứa một advisory về lỗi an toàn bộ nhớ

Vị trí: [Cargo.lock:2505](Cargo.lock#L2505).

Quét 736 package/version registry trong Cargo.lock bằng [OSV querybatch](https://google.github.io/osv.dev/post-v1-querybatch/) và kiểm tra các nguồn gốc advisory. Có 11 package/version được đánh dấu, tương ứng 8 advisory khác nhau. Không phải tất cả đều là lỗ hổng: 7 advisory là thông tin ngừng bảo trì.

`cargo tree --target x86_64-pc-windows-msvc -i grid@0.18.0` xác nhận đường dependency `oicha → gpui 0.2.2 → taffy 0.9.0 → grid 0.18.0`. Phiên bản này nằm trong dải ảnh hưởng của [GHSA-38c5-483c-4qqp / CVE-2026-42199](https://github.com/becheran/grid/security/advisories/GHSA-38c5-483c-4qqp): overflow trong `Grid::expand_rows()` có thể dẫn tới undefined behavior qua API safe; advisory công bố bản sửa 1.0.1.

Chưa tìm thấy lời gọi `expand_rows` trong source Taffy 0.9.0 được cài và chưa tái hiện đường kích hoạt qua UI Oicha. Vì vậy đây là dependency bị ảnh hưởng cần xử lý hoặc phân tích reachability, không phải tuyên bố ứng dụng có exploit đã chứng minh. Nâng 0.18 lên 1.0.1 có thể cần thay đổi upstream, không chỉ đổi số version rồi bỏ qua API compatibility.

Các cảnh báo INFO Unmaintained được xác minh:

| Crate | Advisory |
| --- | --- |
| async-std 1.13.2 | [RUSTSEC-2025-0052](https://rustsec.org/advisories/RUSTSEC-2025-0052.html) |
| instant 0.1.13 | [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384.html) |
| paste 1.0.15 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) |
| proc-macro-error2 2.0.1 | [RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173.html) |
| rustls-pemfile 2.2.0 | [RUSTSEC-2025-0134](https://rustsec.org/advisories/RUSTSEC-2025-0134.html) |
| rustybuzz 0.14.1 và 0.20.1 | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html) |
| ttf-parser 0.20.0, 0.21.1 và 0.25.1 | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) |

Scan theo lockfile gồm cả dependency cho nền tảng khác; danh sách INFO không đồng nghĩa tất cả đều nằm trong executable Windows. `cargo-audit` chưa được cài; kết quả trên là truy vấn OSV exact package/version có kiểm tra đủ số response và pagination, không giả là đã chạy cargo-audit. Cần thêm kiểm tra advisory vào CI và chính sách xử lý exception có tài liệu.

## Kiểm tra đạt

- `cargo fmt --all --check`.
- `cargo test --workspace --locked`: 33 core tests; app có 0 unit tests.
- Differential test riêng: 60 tình huống Vim đối chiếu Vim 9.2 đạt.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo build -p oicha --release --locked`.
- Native smoke trên đúng `dist/Oicha.exe`: PASS cho word/idle undo, IME adapter, Vim ON/OFF, toolbar, wrapping, A/0/j/k, font reflow, formatting, search, save và DOCX export.
- Hash executable build và dist trùng nhau. Các bản thảo review tách riêng, không thay normal last-session.

## Bằng chứng và tái chạy

- [Core probe JSON](artifacts/production-review-core-exact.json).
- [Native release probe JSON](artifacts/review-native-release/candidate.review-result.json).
- [Dependency scan JSON](artifacts/production-review-dependencies.json).
- [Candidate smoke result](artifacts/production-candidate.smoke-result.txt).
- Probe core: `cargo run --manifest-path artifacts/production-review-probe/Cargo.toml --release --offline -- artifacts/<unique-output>.json`; output filename phải mới vì probe dùng Store::create để bảo vệ file đã có.
- Bản sao native: `artifacts/production-review-ui`; chỉ worker load có delay 500ms và smoke được thay bằng review probes; thuật toán editor/layout không sửa. Compile với target-dir chung để dùng cache nhưng executable riêng tên `oicha-review`. Khi tái chạy phải dùng một thư mục bản thảo mới vì probe tạo các destination cố định trong đó.

## Điều chưa được xác nhận

Chưa thực hiện kiểm thử mất điện/kill tại từng bước checkpoint/backup/persist, disk-full/permission failure của toàn workflow GUI, kiểm thử từng bộ gõ thật như UniKey/EVKey/Windows Telex, soak test bản thảo lớn và mở DOCX bằng Microsoft Word. Không có chứng nhận accessibility hay xác nhận macOS/Linux. Workspace hiện không phải Git repository, nên chưa có bằng chứng CI từ remote/tag phát hành. Executable được kiểm tra có Authenticode status `NotSigned`; chưa có installer/release-signing workflow được kiểm chứng.

Cần sửa hai P1, bổ sung các regression vào bộ kiểm thử chính, xử lý vấn đề hiệu năng/recovery/dependency và chạy lại trên artifact phát hành mới trước khi duyệt production. Review đạt các test hiện có không phải bằng chứng mọi đường sử dụng đều an toàn.
