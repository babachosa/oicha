# Audit review — 2026-10-17

**Theo dõi sửa:** bản 0.2.4 xử lý các finding của review này; xem [AUDIT_FIX_VALIDATION_2026-10-17.md](AUDIT_FIX_VALIDATION_2026-10-17.md) để biết thay đổi và bằng chứng kiểm tra. Các kết luận và vị trí code bên dưới mô tả commit 0.2.3 được audit, không phải source sau sửa.

**Kết luận: chưa duyệt bản 0.2.3 để phát hành rộng.** Có một lỗi P1 gây crash đã tái hiện trên executable phát hành, một lỗi P2 về Home/End và một dependency gate đang fail. Đây là review; không sửa source sản phẩm, không thay executable hoặc bản cài đang chạy của người dùng.

Phạm vi: ba crate Rust, luồng lưu/mở/phục hồi/đóng, editor/Vim/IME/undo, layout/cuộn trang, xuất DOCX/TXT, cập nhật ký số, NSIS và GitHub workflows. Commit được review: `6dd354cf69688a76f86c41e53f8daecc4d76936e`.

Executable native đã kiểm tra: `dist/Oicha.exe`, SHA-256 `9B56EC7A280F917264F2FCA5A2703F7AC2EE0B63565A82F94BCEE7D2D6F62A44`, trùng `target/release/oicha.exe`.

## 1. [P1] Undo/Cut trong Vim Visual giữ anchor cũ, gây panic khi vẽ lại

Vị trí chính: [editor.rs:145](crates/oicha-core/src/editor.rs#L145), [editor.rs:664](crates/oicha-core/src/editor.rs#L664). Các đường GUI: [text_editor.rs:458](crates/oicha-app/src/text_editor.rs#L458), [text_editor.rs:546](crates/oicha-app/src/text_editor.rs#L546), [text_editor.rs:1529](crates/oicha-app/src/text_editor.rs#L1529).

`restore()` thay body/cursor/anchor nhưng không reset `mode` hay `visual_anchor`. Ctrl+Z và nút Undo gọi `engine.undo()` trực tiếp. Tương tự, Ctrl+X gọi `replace(painted_selection(), "")` mà không kết thúc Visual. Nếu văn bản sau sửa ngắn hơn `visual_anchor`, `painted_selection()` → `visual_range()` → `next(visual_anchor)` cắt chuỗi ngoài giới hạn.

Probe core release dùng nguyên source hiện tại:

- Gõ `abcdef` vào editor trống, Esc, chọn Visual tại cuối, gọi Undo: body trở về trống nhưng mode vẫn Visual; tính selection panic ở byte 5 trên chuỗi dài 0.
- Body `abcdef`, đặt cursor 5, `v h`, Cut theo đúng nhánh Ctrl+X: body thành `abcd`, nhưng tính selection panic ở byte 5 trên chuỗi dài 4.

Đã xác nhận trên **bản EXE 0.2.3 thật** bằng cửa sổ/bản thảo QA riêng: đến dòng cuối, `o`, gõ `abcdef`, Esc, `v`, `u`. Nhánh `u` hiện gọi cùng `undo()`; cửa sổ thoát bất thường, exit code **-1073740791**, stderr: `start byte index 138 is out of bounds for string of length 132`. Đây là crash thực tế, không chỉ cảnh báo suy luận. Phần gõ sau lần lưu tự động cuối có nguy cơ chưa được lưu; không thử nghiệm trên bản thảo của người dùng.

Cần chuyển mode/anchor về trạng thái hợp lệ sau history và GUI Cut, đồng thời chặn range ngoài biên khi tính vùng chọn. Không chỉ clamp byte offset vì phải giữ ranh giới grapheme và semantics vùng chọn. Thêm regression cho Ctrl+Z/Redo/nút Undo/Cut trong Visual và VisualLine, chọn xuôi/ngược, Unicode và undo về body trống.

Bằng chứng:

- `artifacts/audit-2026-10-17-core-probe.json`
- `artifacts/audit-2026-10-17-core-probe.stderr.txt`
- `artifacts/audit-2026-10-17-visual-native/results.json`
- Probe source: `artifacts/audit-2026-10-17-probe/`, `artifacts/audit-2026-10-17-visual-native.ps1`.

## 2. [P2] Home/End trong Insert hoặc khi tắt Vim dùng dòng logic thay vì hàng hiển thị

Vị trí: [text_editor.rs:665](crates/oicha-app/src/text_editor.rs#L665).

Nhánh nhập thông thường xử lý Home bằng `line_start()` và End bằng `line_end()`, đều tìm dấu xuống dòng cứng. Một đoạn tự ngắt thành nhiều hàng vì vậy bị nhảy về đầu/cuối cả đoạn. Khác biệt dễ gặp khi viết tiểu thuyết dài, và không nhất quán với các lệnh `0`/`A` đã được chuyển sang hàng hiển thị theo yêu cầu trước đó. Shift+Home/End cũng đi cùng nhánh và có thể chọn nhiều hàng ngoài ý định.

Tái hiện trên EXE release, **Vim tắt**, chèn 330 ký tự vào một đoạn dài đủ tự xuống hàng:

- Home: cursor 330, y=185 → cursor 0, y=0; đã nhảy 5 hàng lên đầu đoạn.
- Từ hàng tiếp theo, End: cursor 55, y=37 → cursor 383, y=222; đã nhảy xuống cuối đoạn.
- Generation không đổi: đây là sai chuyển động, chưa phải mất nội dung. Vim Insert dùng cùng nhánh; phép tái hiện native này chạy với Vim tắt.

Cần tách chuyển động đầu/cuối hàng hiển thị cho các phím GUI, xử lý affinity ở soft wrap, và thêm kiểm tra selection Shift+Home/End. Giữ semantics các lệnh Vim logic đã được định nghĩa riêng.

Bằng chứng: `artifacts/audit-2026-10-17-home-end-native/results.json`. Tái chạy probe native bằng `-HomeEnd` với output mới.

## 3. [P2] Dependency gate hiện fail với ba advisory của zip 4.6.1

Vị trí: [Cargo.toml:16](crates/oicha-core/Cargo.toml#L16), [Cargo.lock:7494](Cargo.lock#L7494).

Quét OSV ngày 2026-10-17: đủ **741** registry package/version; hash backport grid đúng; 10 kết quả INFO thuộc 7 advisory đã được duyệt; **3 advisory chưa được duyệt** cho `zip 4.6.1` làm `tools/check-dependencies.ps1` thoát lỗi:

| Advisory | Phạm vi theo nguồn upstream | Bản sửa upstream |
| --- | --- | --- |
| [RUSTSEC-2026-0367 / GHSA-vv5m-jxxh-4627](https://github.com/zip-rs/zip2/security/advisories/GHSA-vv5m-jxxh-4627) | Giữ SUID/SGID khi giải nén ZIP trên Unix | 9.0.0 |
| [RUSTSEC-2026-0368 / GHSA-4v9h-p7m9-rh26](https://github.com/zip-rs/zip2/security/advisories/GHSA-4v9h-p7m9-rh26) | Giải nén symlink có target ngoài thư mục đích | 9.0.0 |
| [RUSTSEC-2026-0369 / GHSA-4684-h2q9-2vcp](https://github.com/zip-rs/zip2/security/advisories/GHSA-4684-h2q9-2vcp) | Áp quyền Unix qua symlink khi giải nén | 9.0.0 |

`cargo tree --target x86_64-pc-windows-msvc -i zip@4.6.1` xác nhận dependency trực tiếp qua oicha-core. Phân tích reachability tại commit này: runtime chỉ dùng **ZipWriter để xuất DOCX**; ZipArchive chỉ có trong test để đọc file tự tạo; không có lời gọi extract trong runtime. Do đó **chưa tìm thấy đường kích hoạt các lỗi giải nén này trong Oicha Windows**. Không trình bày đây là exploit Oicha đã chứng minh, cũng không coi các advisory đã được sửa. Theo policy hiện hành, release/CI vẫn bị chặn khi chạy scan mới.

Cần nâng dependency với kiểm tra API/DOCX, hoặc ghi nhận exception có phân tích reachability và phạm vi rõ ràng. Không đưa ba ID này vào danh sách INFO Unmaintained để làm scan xanh.

Bằng chứng: `artifacts/audit-2026-10-17-dependencies.json` và ba snapshot `artifacts/audit-2026-10-17-RUSTSEC-2026-036{7,8,9}.json` từ OSV; đã đối chiếu các advisory upstream trực tiếp.

## Kiểm tra đạt và giới hạn

| Kiểm tra hiện tại | Kết quả |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo test --workspace --locked` | PASS: 58 tests; Vim oracle chạy riêng |
| Vim reference, `--ignored` | PASS |
| Clippy workspace/all targets, `-D warnings` | PASS |
| Security regressions, release | PASS: 2 tests |
| Native release: load success/failure, lưu/đóng sau concurrent edits | PASS cả hai |
| Native release: cursor matrix | PASS 7 trường hợp |
| Native release: first-row margin + scroll boundaries | PASS |
| Native caption: drag/resize, maximize/restore, minimize, close/lưu; có và không popup | PASS cả hai |
| Public stable feed từ 0.2.2 | PASS: nhận 0.2.3, xác minh chữ ký, tải installer và checksum |
| Dependency audit mới | **FAIL**: 3 advisory zip chưa được xử lý |
| Native Visual undo probe | **CRASH tái hiện** |
| Native Home/End soft wrap | **Sai hàng tái hiện** |

Bộ native smoke còn kiểm tra UTF-16/IME, Vietnamese NFC/NFD, undo nhóm từ/idle, font và whole-word wrap, pagination/motion, popup/language, tìm kiếm, lưu và xuất DOCX. Bằng chứng mới trong `artifacts/audit-2026-10-17-native/`, `...-cursor/`, `...-scroll/`, `...-caption/`, `...-caption-modal/`.

Remote CI của commit review đã hoàn tất thành công ngày 2026-10-16 ([Rust checks](https://github.com/babachosa/oicha/actions/runs/38831782175), [Windows release](https://github.com/babachosa/oicha/actions/runs/38831782203)). Đây là kết quả lịch sử; không thay thế scan advisory mới đang fail.

Không chạy cài/gỡ hoặc restart-update vào installation đang dùng; app của người dùng đang mở được giữ nguyên. Installer/updater có review source và probe tải/xác minh, nhưng lần audit này chưa có fault injection disk-full/mất điện/permission ở mọi bước, chưa có kiểm thử từng bộ gõ UniKey/EVKey/Telex thật, soak test dài hạn, schema/round-trip DOCX bằng Microsoft Word, accessibility hoặc macOS/Linux. Chưa rebuild bộ cài vì không có thay đổi source sản phẩm. Các kiểm tra đạt không đủ để bỏ qua crash P1 ở trên.
