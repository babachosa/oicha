# Oicha

Ứng dụng desktop viết tiểu thuyết bằng Rust, render bằng **GPUI 0.2.2 của Zed**. Không egui, Electron, JavaScript hay webview. Giao diện native, trang viết nền giấy, Unicode tiếng Việt, thao tác bàn phím Vim và bản thảo lưu trên máy.

## Chạy trên Windows

Bộ cài: [Oicha 0.3.0 cho Windows x64](https://github.com/babachosa/oicha/releases/tag/v0.3.0), hoặc `dist/Oicha-0.3.0-Windows-x64-Setup.exe` khi tự build. Dùng Windows 10 phiên bản 2004 trở lên hoặc Windows 11 x64, GPU hỗ trợ DirectX 11. Cài cho tài khoản hiện tại, không cần quyền admin, Rust, Node hay cài font riêng. Start Menu có shortcut Oicha; có thể chọn thêm shortcut Desktop và mục Open with cho file `.oicha`. Gỡ trong Windows Settings → Apps sẽ giữ lại bản thảo. Đóng app để lưu trước khi nâng cấp/gỡ cài đặt. Bản hiện tại chưa ký số theo lựa chọn của người dùng.

Bản 0.3.0 đổi định dạng kho: file `.oicha` cũ từ 0.2.x được giữ nguyên nhưng không mở hoặc chuyển đổi. Nếu cần dùng nội dung bên ngoài app, hãy xuất TXT/DOCX bằng bản cũ trước khi nâng cấp.

Để tự build từ mã nguồn:

Yêu cầu Windows 10/11 x64, GPU/driver hỗ trợ DirectX 11, Rust 1.98.1 MSVC và Visual Studio Build Tools (Desktop development with C++ / Windows SDK). Toolchain và GPUI được cố định; `Cargo.lock` được đưa vào source.

```powershell
cargo run -p oicha --release --locked
# Hoặc mở một bản thảo:
cargo run -p oicha --release --locked -- "D:\Ban-thao\Tieu-thuyet.oicha"
```

Build xong, chạy trực tiếp `target/release/oicha.exe`. Không cần Node, trình duyệt hay dịch vụ đám mây. Lần đầu mở là **Trang chủ** của kho rỗng; chọn **Dự án mới** để tạo tiểu thuyết. Nhiều dự án cùng nằm trong một file `.oicha`.

Build bộ cài bằng PowerShell 7.2+: `./tools/build-windows-installer.ps1`. Xem `packaging/windows/README.md` và `WINDOWS_INSTALLER_VALIDATION.md` để biết pipeline, checksum, runtime đóng gói và kết quả kiểm thử cài/gỡ.


## Ngôn ngữ và cập nhật

Mở **Cài đặt** ở thanh tiêu đề, chọn **Tiếng Việt** hoặc **English**. Giao diện đổi ngay và nhớ lựa chọn cho lần mở sau; nội dung bản thảo giữ nguyên. Trong cùng mục này có kiểm tra, tải và khởi động lại để cập nhật. App cũng kiểm tra GitHub Releases khi khởi động và hiện thông báo trên nút Cài đặt nếu có bản mới.

Gói cập nhật được xác minh chữ ký Ed25519 và SHA-256 trước khi chạy; phải lưu xong bản thảo mới đóng app để nâng cấp. Khóa ký cập nhật độc lập với chứng thư Windows. Bản 0.1.0 cần cài thủ công bản mới lần đầu. Xem [UPDATES.md](UPDATES.md) cho quy trình phát hành và kiểm thử.

## Chức năng

- Home/End và Shift+Home/End đi theo hàng đang hiển thị, kể cả khi đoạn tự xuống hàng và khi tắt Vim. Undo/Redo, Cut/Paste trong Vim Visual giữ vùng chọn hợp lệ, kể cả khi chương trở về trống.

- Khi về hàng hiển thị đầu tiên, trang tự hiện trọn lề trên và khoảng thoáng phía trên; áp dụng với bàn phím và nhấp chuột. Cuộn tiếp ở đầu/cuối trang không đẩy trang vượt biên rồi bật lại; đổi hướng cuộn có hiệu lực ngay.


- Tạo, đổi tên, sắp xếp và xóa chương; ghi chú/tóm tắt riêng từng chương; đổi tên sách và tác giả.
- Popup nhập liệu, phím tắt và xác nhận cùng một thiết kế: mở bằng fade/slide 200 ms, đóng 120 ms, nền dịu, bo góc và bóng mềm. Esc hoặc nhấn nền ngoài để hủy; Ctrl+Enter lưu ghi chú nhiều dòng. Xác nhận xóa/ghi đè chọn Hủy mặc định, Tab đổi lựa chọn rồi Enter xác nhận.
- Chọn văn bản bằng chuột, Shift + phím mũi tên; double-click chọn từ; clipboard, undo/redo và đậm/nghiêng/gạch chân trên từng vùng chữ.
- Undo khi gõ được chia theo từ/đợt gõ: tách sau khoảng trắng hoặc dấu câu, nghỉ gõ từ 1 giây, tối đa 32 ký tự ghép hoặc 5 giây gõ liên tục. Enter, Tab, dán và thay vùng chọn có lượt undo riêng; Backspace/Delete liên tiếp được gom riêng. Các bước ghép dấu của một phiên IME và commit giữ chung một lượt, kể cả nghỉ gõ giữa lúc ghép dấu. Áp dụng khi VIM bật hoặc tắt và cho nút undo/redo trên toolbar.
- Khi gõ, cả tự xuống hàng và Enter đều đưa hàng mới lên khoảng 32% vùng nhìn nếu đã vào phần dưới màn hình, kể cả khi chèn giữa nội dung có sẵn. Khi còn thoáng hoặc chỉ sửa dấu trong cùng hàng, vùng nhìn giữ ổn định. Áp dụng với VIM bật/tắt và lệnh `o` tại dòng cuối.
- Cảm giác gõ mượt: con trỏ lướt 80 ms, cuộn bằng bánh xe/`zz`/theo hàng mới chuyển động 120 ms với tốc độ giảm dần. Gõ nhanh tiếp tục từ vị trí đang hiển thị; chữ mới hiện nhẹ 70 ms nhưng được nhập ngay. Ghép dấu IME không chạy hiệu ứng chữ; touchpad giữ chuyển động và quán tính native. Hiệu ứng dừng khi đến đích, không chạy vòng render liên tục lúc nghỉ.
- Giấy hiển thị theo tỷ lệ A4: mỗi tờ giữ nguyên chiều cao tại cùng độ rộng, có lề và khoảng cách 12 px giữa các tờ. Viết đầy sẽ sang tờ kế tiếp thay vì kéo dài một tờ; đổi độ rộng cửa sổ sẽ dàn lại chữ theo tỷ lệ trang. Phân trang chỉ thuộc giao diện, không chèn ký tự hoặc ngắt trang thủ công vào bản thảo.
- Trên Windows, phím lệnh Vim Normal/Visual được nhận trước bộ gõ bên ngoài; vào Insert sẽ bắt đầu ngữ cảnh nhập mới. UniKey không giữ hoặc đổi phím `w`, `dw`, `cw`, `i` thành chữ tiếng Việt. Insert và các ô nhập vẫn dùng bộ gõ của hệ điều hành; không tự đổi lựa chọn Việt/Anh của người dùng.
- Font đọc mặc định **Literata**, đóng gói đủ thường/nghiêng/đậm/đậm nghiêng, có ký tự tiếng Việt và dấu ghép, không cần cài thêm font. Mặt giấy chỉ có nội dung viết; không tự vẽ tiêu đề chương hay số trang. Nội dung bắt đầu ở lề trên như các trang tiếp theo; tên chương vẫn quản lý ở thanh bên.
- Xuống dòng theo từ: “niềm” và các từ có dấu được chuyển nguyên sang hàng sau khi không đủ chỗ, kể cả dấu ghép và đổi kiểu chữ giữa từ. Chỉ ngắt trong từ dài hơn cả hàng, tại ranh giới ký tự ghép. Không chèn Enter vào nội dung.
- Thanh tiêu đề tích hợp trong app, hiển thị tên bản thảo và dấu thay đổi chưa lưu; kéo/nhấp đúp để di chuyển hoặc phóng to/khôi phục, các nút thu nhỏ, phóng to/khôi phục và đóng. Windows giữ xử lý vùng kéo, mép resize và nút Min/Max; nút đóng đi qua cơ chế lưu và chờ I/O của ứng dụng.
- Cuộn được qua hàng cuối nhờ khoảng trống bằng khoảng 65% chiều cao vùng viết, tự điều chỉnh khi đổi kích thước cửa sổ. Khoảng trống chỉ thuộc giao diện, không thêm Enter, không ảnh hưởng undo, số từ hay file xuất.
- Trang viết tự xuống dòng; đổi font, cỡ chữ và giãn dòng khi đọc/viết. Thống kê từ, ký tự và thời gian đọc. F11 ẩn hai thanh bên.
- Native text input/IME dùng chỉ số UTF-16 của hệ điều hành, lõi UTF-8 và di chuyển/xóa theo grapheme; hỗ trợ emoji ghép và dấu tổ hợp.
- Range sửa của IME giữ đúng biên Unicode scalar, kể cả cập nhật riêng dấu trong một ký tự ghép; chuyển động/xóa Vim vẫn theo grapheme. Glyph được lập bảng tra và cache theo paragraph để tránh dò lại từ đầu cho mỗi ký tự.
- Đường chân chữ cố định theo font/cỡ chữ: thêm hoặc bỏ dấu tiếng Việt không làm cả đoạn nhấp nhô khi hệ điều hành chọn font dự phòng.
- Tìm trong chương hiện tại, tìm tiếp/quay lại có vòng lặp. **Mở kho** chọn file `.oicha`; **Trang chủ** liệt kê và chuyển giữa các dự án trong kho. Xuất TXT/DOCX chỉ xuất dự án đang mở.
- Mỗi dự án ở Home có menu **⋯ → Xóa dự án**, xác nhận theo tên dự án. Enter mặc định hủy; Tab chọn rồi Enter xác nhận. Xóa dự án cuối cùng giữ lại kho rỗng, không tạo lại dự án qua autosave.
- Xuất DOCX OOXML (chương, trang A4, định dạng đậm/nghiêng/gạch chân), và TXT. DOCX xuất font Times New Roman 12pt, giãn dòng 1.5; các chỉnh font trong giao diện là tùy chọn đọc/viết.
- Tự lưu mỗi 2 giây, lưu thủ công, Lưu thành file mới. Đóng ứng dụng hoặc chuyển sách đợi lưu xong; lỗi ghi file giữ cửa sổ và bản thảo hiện tại.
- Phần gõ phát sinh khi đang tải sách khác được lưu vào sách cũ trước khi chuyển. Yêu cầu đóng trong lúc I/O được xử lý sau khi I/O kết thúc, kể cả tải thất bại.

## Vim

Vim bật mặc định. Nhấn **i** để viết, **Esc** trở lại Normal. Có thể tắt bằng nút VIM. F1 mở bảng phím tắt trong ứng dụng.

| Thao tác | Phím |
| --- | --- |
| Di chuyển | `h/l` trái/phải; `j/k`, `gj/gk` xuống/lên theo **hàng hiển thị**, giữ vị trí ngang qua hàng ngắn |
| Căn giữa vùng nhìn | `zz` đưa hàng hiển thị có con trỏ vào giữa, giữ cột; `42zz` đến dòng logic 42 rồi căn giữa. Dùng trong Normal/Visual |
| Từ, đầu/cuối | `w b e`; **`0` về đầu hàng đang hiển thị**, tương đương `g0`; `^ $`, `g^ g$`, `gg G`, `3j`, `2gg`, `2G` |
| Insert | `i a I A`, `o O`; **`A` vào Insert tại cuối hàng đang hiển thị**, giữ caret ở hàng đó tại điểm xuống dòng mềm |
| Chọn | `v`, `V` |
| Operator + motion | `d y c`, ví dụ `dw`, `3dw`, `d2w`, `dd`, `yy`, `cc` |
| Xóa hàng hiển thị | `dd` chỉ xóa hàng đang nhìn thấy; `3dd`, `d3d`, `2d2d` đếm hàng hiển thị. Xóa hàng cuối giữ một hàng trống để `i` viết tiếp ngay. `u` khôi phục nguyên nội dung và định dạng; phần đã xóa vẫn vào register để `p/P` dán theo dòng |
| Text object | `diw`, `daw`, `ciw`, `yiw` |
| Xóa, nối dòng, dán | `x X`, `D C`, `J`, `p P` |
| Lịch sử | `u`, `Ctrl+R`, `Ctrl+Z`; `Ctrl+Y` redo khi Insert hoặc tắt Vim |
| Tìm | `/`, `n`, `N`; `Ctrl+F` khi Insert hoặc tắt Vim |
| Tìm ký tự trong dòng | `f{ký tự}` tới / `F{ký tự}` lùi; `2fa` tới chữ `a` thứ hai; `;` lặp cùng chiều, `,` lặp ngược chiều. Dùng được với Visual và `d/c/y`, ví dụ `dfa`, `cf.`, `yf,` |
| Trang trong Vim Normal/Visual | `Ctrl+B/F` lên/xuống trang, `Ctrl+U/D` nửa trang, `Ctrl+Y/E` cuộn một hàng |
| Lệnh | `:w`, `:wq`, `:q`, `:e`, `:new`, `:export`, `:saveas`, `:help`, `:42` |
| Kho / dự án | `Ctrl+S` lưu dự án, `Ctrl+Shift+S` lưu toàn bộ kho thành file mới, `Ctrl+O` mở kho, `Ctrl+N` tạo dự án |
| Định dạng | Nút toolbar; `Ctrl+B`, `Ctrl+I`, `Ctrl+U` khi Insert hoặc tắt Vim |
| Xuất DOCX | `Ctrl+Shift+E` |

`f/F` tìm trong dòng logic: có thể đi qua các hàng tự xuống dòng, nhưng dừng ở dấu Enter. Nhấn `Esc` để hủy khi đang chờ ký tự. Nếu không đủ số lần khớp, con trỏ và văn bản giữ nguyên, kể cả với `df/cf/yf`. Ký tự đích được hiểu nguyên văn, bao gồm chữ số, khoảng trắng, `/`, `:` và dấu tiếng Việt. `,` tìm ngược tạm thời; `;` vẫn dùng chiều của lệnh `f/F` ban đầu.

Theo yêu cầu soạn tiểu thuyết, `j/k` được ánh xạ như `gj/gk` của Vim để đi từng hàng được renderer xuống dòng. Vim mặc định dùng `j/k` cho dòng logic; đây là ánh xạ có chủ đích, tương đương `nnoremap j gj`, `nnoremap k gk`, `xnoremap j gj`, `xnoremap k gk`. `0` (và Home trong Normal/Visual) dùng đầu hàng hiển thị; `v0`, `d0`, `c0`, `y0` cũng dùng biên hàng này. Chữ số `0` trong count như `10l` vẫn giữ ý nghĩa đếm. Operator `dj/dk`, `yy/cc` và Visual Line vẫn theo dòng logic; `dd` dùng hàng hiển thị và giữ hàng trống ở cuối bản thảo; `dgj/dgk` là thao tác theo hàng hiển thị. `h/l` dừng tại biên dòng, Normal đứng trên ký tự cuối, `$`/`G`/`gg`, `cw`, xóa/dán dòng cuối và số lần lặp đã có kiểm thử đối chiếu Vim thật.

Đây là engine Vim nội bộ với tập lệnh trên. Chưa hỗ trợ toàn bộ Vim/Ex, macro, dot-repeat, registers có tên, Vim plugins, LSP hoặc config vimrc. Không tuyên bố tương thích 100% toàn bộ Vim. Vim chỉ dùng làm test oracle, ứng dụng vẫn hoàn toàn bằng Rust.

## Lưu trữ và phục hồi

Nút X hỏi **Lưu / Không lưu / Hủy** khi còn thay đổi chưa lưu. Chỉ chọn Lưu mới mở hộp chọn đường dẫn nếu bản thảo chưa có nơi lưu; hủy hộp chọn sẽ giữ app mở. Không lưu giữ nguyên bản đã lưu trước đó, bỏ phần chưa lưu và checkpoint tương ứng. Nội dung đã được autosave từ trước vẫn được giữ; nếu không còn thay đổi chưa lưu, app đóng ngay. `:wq` vẫn chủ động lưu rồi đóng.

`.oicha` là một cơ sở dữ liệu redb, viết bằng Rust, chứa nhiều dự án với ID riêng, chương, định dạng và checkpoint nội bộ. Định dạng kho phiên bản 2 là breaking change: không mở/chuyển đổi file JSON của các bản cũ; file cũ được giữ nguyên. Giới hạn 64 MiB mỗi dự án, tối đa 10.000 dự án mỗi kho.

- Một file duy nhất: không tạo `.bak`, `.lock` hay `.recovery` bên cạnh kho. Giao dịch bền vững và khóa trên chính file do redb quản lý. Bản lưu đã commit vẫn đọc được sau khi tiến trình dừng bất thường; giao dịch chưa commit bị bỏ.
- Tự lưu mỗi 2 giây; chuyển về Home, sang dự án khác hoặc mở kho khác sẽ chờ lưu xong. Nếu lưu lỗi, dự án đang viết được giữ lại cùng thông báo. Có thể mất phần chữ chưa đến lần lưu tiếp theo nếu mất điện hoặc kill process.
- **Dự án mới** tạo tiểu thuyết trong kho hiện tại. **Lưu kho thành…** sao chép toàn bộ dự án cùng thay đổi đang viết sang một file mới và chuyển sang bản sao; từ chối ghi đè file đã tồn tại.
- Kho mặc định là `Library.oicha` trong thư mục dữ liệu Oicha (`directories::ProjectDirs`). Con trỏ `last-library` ghi nhớ kho đã chọn; tùy chọn ngôn ngữ vẫn thuộc ứng dụng. Không sử dụng con trỏ `last-session` của định dạng cũ.
- Khi kho đã ghi nhớ bị xóa hoặc di chuyển, mở lại EXE sẽ báo không tìm thấy kho và ở Home; không tự tạo lại file, không tự mở hộp lưu. Chọn **Mở kho** để tìm kho khác hoặc **Dự án mới** để chủ động tạo kho mới. Đường dẫn truyền qua dòng lệnh cũng phải là file đã tồn tại. Chỉ hồ sơ lần đầu chưa có `last-library` mới tự khởi tạo kho mặc định.
- Undo nằm trong RAM của dự án đang mở, tối đa khoảng 32 MiB mỗi chương. Về Home rồi mở lại chính dự án đó giữ vị trí và undo; chuyển sang dự án khác sẽ nạp lại các editor.
- Không sao lưu ngoài file tự động. Có thể dùng **Lưu kho thành…** để tạo bản sao độc lập.

## Kiến trúc và kiểm tra

```
crates/oicha-core/
  document.rs   Model/version/định dạng
  editor.rs     Unicode, history, Vim, tìm kiếm
  storage.rs    Single-file library, transactions, projects, checkpoints
  export.rs     DOCX/TXT bằng Rust
crates/oicha-app/
  text_editor.rs  GPUI custom Element, GPU text shaping, IME, selection/caret
  app.rs          Chương, modal, workflow, I/O worker
  main.rs         Native application lifecycle
```

Lõi không phụ thuộc UI. GPUI định hình chữ qua font hệ điều hành, cache layout theo generation/width/font; chỉ paint đoạn trong viewport. I/O lưu bản thảo chạy trên background executor. Mã soạn thảo không dùng unsafe. Helper cập nhật có một đoạn Win32 nhỏ để chờ tiến trình cũ thoát, với hợp đồng an toàn được ghi rõ tại chỗ. Các thư viện nền và API hệ điều hành vẫn có native dependencies; “Rust” ở đây là toàn bộ mã ứng dụng/lõi, không phải viết lại hệ điều hành hoặc driver.

```powershell
cargo fmt --all --check
cargo test -p oicha-core --locked
cargo test --workspace --locked
cargo test -p oicha-core --test security_regressions --release --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
./tools/check-dependencies.ps1
# Đối chiếu các lệnh được hỗ trợ với Vim thật (cần cài Vim):
cargo test -p oicha-core --test vim_reference --locked -- --ignored
cargo build -p oicha --release --locked
# Kiểm thử tích hợp với cửa sổ GPUI thật; path phải chưa tồn tại:
./tools/native-smoke.ps1
```

Smoke test dùng bản thảo riêng, không thay last-library, kiểm tra dispatch bàn phím thật, adapter IME, Unicode, undo/redo, định dạng, modal tìm, focus mode, background save và xuất DOCX. Driver còn kiểm tra giữ phần gõ khi tải/đóng và yêu cầu cả kết quả `.smoke-result.txt`, marker `.smoke-closed.txt`, process exit 0 và nội dung thực tế đã lưu. Cửa sổ test cần hiển thị để Windows vẽ các frame reflow. CI Windows chạy format, workspace tests, security regression ở release, dependency scan, Clippy và release build; smoke native cần desktop/GPU và chạy trên máy phát triển.

GPUI/Taffy dùng bản `grid 0.18.0` được giữ trong `vendor/grid` với backport kiểm tra tràn số và giấy phép MIT gốc. Xem `vendor/grid/OICHA_PATCH.md` và `DEPENDENCY_POLICY.md` để đối chiếu nguồn vá và các advisory INFO còn theo dõi.

## Phạm vi tính năng

Đây là nền tảng desktop có quy trình lưu/kiểm thử rõ ràng, chưa phải tính năng ngang Microsoft Word hay bản đã được chứng nhận production. Đã phân trang A4 trên giao diện; chưa in/PDF, nhập DOCX, bảng, ảnh, footnote, track changes, spellcheck hay accessibility đầy đủ. History dùng snapshot và model String; bản thảo cực lớn cần rope/piece tree, cache theo đoạn và benchmark thêm trước khi cam kết latency. Hiện kiểm chứng trên Windows x64; chưa kiểm chứng macOS/Linux. GPUI vẫn pre-1.0, cập nhật phiên bản cần build và smoke test lại.

Kiểm tra kho nhiều dự án trên desktop: `tools/native-library-smoke.ps1`. Các driver native dùng `--inspect-store` để kiểm tra nội dung nhị phân sau khi đóng kho.

Thông báo dùng ngôn ngữ giao diện đang chọn, có nội dung ngắn và hướng xử lý. Lỗi kỹ thuật và đường dẫn chỉ xuất hiện khi mở **Chi tiết**; có nút sao chép để chẩn đoán. Cảnh báo, lỗi và thông báo thành công có cách hiển thị riêng. Xem `NOTIFICATIONS_VALIDATION_2026-10-19.md` để đối chiếu kiểm thử.
