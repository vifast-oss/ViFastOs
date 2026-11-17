# Mức bảo đảm host của ViScript (ST-73, VD-107)

Tài liệu **ngắn** về cái gì **thật sự** được bảo đảm trên từng nền. Cập nhật: 2026-11-17
(ST-173: Landlock **chỉ Linux** — xem mục cuối tệp).

## Bảng nền tảng ↔ bảo đảm

| Nền tảng | Cưỡng chế **runtime** (ST-72b) | Landlock (kernel) | Bằng chứng |
|---|---|---|---|
| macOS (máy phát triển) | **Có** — `FsGuard`/`NetGuard`, `VS0604`, `--grant`, đã đo 8 ca interp+VM | **Không có** — nhân Darwin không có Landlock; suite `vis-host-linux` được **cfg-out** (0 ca chạy, ST-173) | `vis run --sandbox-check` in `landlock: không hỗ trợ trên macos`, exit 3; `cargo test -p vis-host-linux` exit 0, **0 ca** |
| Linux | **Có** — cùng mã `vis-cli` | **Có** — crate `landlock =0.4.7` cưỡng chế bằng kernel (ST-132); suite **15/15 ca** trên `ps` (ST-150/ST-166/ST-173) | `cargo test -p vis-host-linux --test landlock` → `15 passed` (log `agent/JOURNAL/st173-ps-truoc.log`, `st173-ps-sau.log`) |
| Windows / wasm | Chưa có host tương ứng | Không áp dụng | Chưa đo |

## Cách đọc `vis run --sandbox-check`

In ra **stdout**, **không** chạy chương trình:

```
landlock: không hỗ trợ trên macos     # hoặc: landlock: có/không (LSM: …) trên Linux
cưỡng chế runtime: bật|tắt            # bật khi `main` khai ≥1 `cap[...]` (ST-72b)
cap điều kiện: fs,net                 # loại quyền đang được cưỡng chế
```

Mã thoát: `0` = Linux có `landlock`; `3` = không có / nền khác; `4` = Linux không đọc được tệp LSM.

## Đa nền tảng — ma trận `cargo check --target` (ST-74)

**Chỉ CHECK biên dịch chéo**; **KHÔNG** chạy trên Windows/wasm (chưa có runner). Lệnh:
`viscript/bin/vis-targets` (in bảng, exit 1 nếu ô **kỳ vọng xanh** bị đỏ; kỳ vọng ghi trong script).

| Nhóm | `x86_64-pc-windows-msvc` | `wasm32-unknown-unknown` | Mức bằng chứng |
|---|---|---|---|
| 7 crate lõi (`vis-diag`, `vis-syntax`, `vis-types`, `vis-bytecode`, `vis-fmt`, `vis-interp`, `vis-vm`) | check xanh | check xanh | **check** |
| `vis-cli` | check xanh | check xanh **nhưng KHÔNG chạy được**: `std::process::Command`, `std::net`, `std::fs`, thread không có OS sau wasm32-unknown-unknown | **check** — không phải hỗ trợ runtime |
| `vis-host-linux` | — | — | `x86_64-unknown-linux-gnu` check (ST-73) |
| `vis-host-wasm` | — | check xanh (khung; `probe()` = “runtime kiểm, không có sandbox OS”) | **check** |
| `vis-host-*` chạy thật | **đo, chưa hỗ trợ** (39/54; 15 ca `not ok` trùng danh sách ST-136 — thiếu chương trình POSIX, VS3) | **chưa có runner** (chỉ check) | Windows: **đo**, Linux: **chạy thật** (`vis-host-linux` 13/13 tại ST-137, **15/15** từ ST-150/ST-166/ST-173 — xem mục cuối; host suite 54/54) |

Bảng đầy đủ (18 ô) + log: `agent/JOURNAL/st74-matrix.log`. **Cấm** viết “chạy trên Windows/wasm”;
đúng là “**check** biên dịch chéo xanh, **chưa chạy**”.

## Bộ hợp chuẩn host — chạy THẬT trên macOS và Linux (ST-75, ST-131/ST-142)

**54 ca**, mỗi ca chạy **cả** `vis run` và `vis run --vm --no-cache` (**120 lần chạy**), so **byte**
stdout + mã thoát (+ stderr chứa khi khai): **0 lệch**.

| Nhóm | Số ca | Ghi chú |
|---|---|---|
| FsHost (`std.fs`, `--fs-mock`) | 22 | đọc/ghi/exists/list, UTF-8/emoji/CRLF, tên có dấu cách/Unicode, 64 KiB, ca âm |
| Ngoai (chương trình ngoài) | 13 | echo/cat/grep/`complete`/mã thoát/`--`/nhiều stage; VM **không có host Ngoai** ⇒ 13 ca khai `vm:vs0604` (kỳ vọng `VS0604`, exit 1 — đúng thiết kế) |
| Shell/`ls`/`open` | 11 | bảng `ls`, `--all`, `--long`, `where`, `sort_by`, `to json`, `take/first/count/select` |
| Quyền `cap` | 5 | trong root, ngoài root `VS0604`, thiếu `(write)`, `--grant`, không khai cap |
| Determinism | 3 | mỗi ca chạy **3 lần** mỗi đường, cùng stdout |

Lệnh: `viscript/bin/vis-host-suite` (in bảng `ca → PASS/FAIL`, exit 1 nếu thiếu/đỏ). Log:
Log: `agent/JOURNAL/st75-suite.log` (macOS, ST-75); `st137-host-suite-macos.log`, `st137-host-suite-ps.log`,
`st142-*`, `st144-host-suite.log` (Windows). **Linux: đã đo** (54/54, ST-142). **Windows: đã đo, chỉ đo**
(39/54, VD-146/VD-121 — chưa hỗ trợ). **wasm: chưa đo** (chỉ check, VD-121).

## Cấm ngôn từ

- **Sai**: “đã cưỡng chế bằng Landlock” khi nói về **macOS/Windows**; “Landlock đã bảo vệ chương trình” trên nền không phải Linux; “cách ly kernel đã bật” chung chung.
- **Đúng**: “**cưỡng chế ở tầng runtime** (ST-72b)” trên **mọi** nền; “**Landlock cưỡng chế chỉ trên Linux** — đo **15/15** ca (ST-132/ST-150/ST-166/ST-173)”.
- `vis-host-linux` **chỉ biên dịch trên Linux**: `src/lib.rs` và `tests/landlock.rs` có `#![cfg(target_os = "linux")]`, `landlock` nằm trong `[target.'cfg(target_os = "linux")'.dependencies]` (ST-173). Trên mac/Windows đây là crate **rỗng, 0 ca** — **giới hạn nền**, không phải thiếu sót; xem mục cuối tệp.

- Cấm nói "async song song", "chạy đồng thời", "rendezvous", "HTTPS", "sandbox kernel" khi mô tả VS2 (VD-122, VD-123, VD-120).
- Cấm nói "hỗ trợ Windows/wasm/Linux" khi chỉ có cargo check (VD-104, VD-121).

## Điều kiện để có Landlock thật (VD-107 (4))

Cần **một** trong hai, kèm runner Linux để **đo**:

1. Agent 0 duyệt crate `landlock` (MIT/Apache) vào allowlist; hoặc
2. Agent 0 duyệt `unsafe` **đóng** trong `vis-host-linux` + ca âm.

Hiện cả hai đều **chưa** được duyệt (R2 cấm `unsafe`, allowlist crate đóng).

**Cập nhật 2026-11-17 (ST-132/ST-166/ST-173):** điều kiện **1 đã đạt** — VD-120 duyệt crate
`landlock =0.4.7` (MIT OR Apache-2.0) **chỉ** trong crate `vis-host-linux`; suite kernel **15/15**
trên `ps`, nợ **#132 đóng** (ST-166). Điều kiện 2 (`unsafe`) **không** dùng.

## Thụt lề `.vish` — **có ngữ nghĩa ở cấp tệp** (ST-85, VD-109)

Đo ngày 2026-11-14 (log `agent/JOURNAL/st85-do.log`): trong **thân `cmd`**, 2 vs 4 dấu cách cho **cùng**
stdout/exit (interp = VM); TAB bị lexer từ chối (`VS0107`). Nhưng ở **cấp tệp**, thụt lề **đổi hành vi**:
tệp mà **mọi** dòng thụt lề ⇒ chương trình **không in gì**, exit 0; dòng cấp tệp thụt 1 dấu cách đứng
**đầu** bị bỏ (`topmix` chỉ in `y`); đứng **sau** lại được chạy (`topmix2` in `y` rồi `x`).

⇒ **Không** chuẩn hoá thụt lề trong `vis fmt` (giữ nguyên văn). `vis fmt --check` chỉ **cảnh báo**
(không đỏ) và `vis fmt --indent-report` **đếm** dòng lệch chuẩn `4 × độ sâu khối` (ước lượng theo dòng
khi thân `cmd` chỉ là lệnh). Tại thời điểm ST-85: **8 tệp / 10 dòng** trong `test/` lệch (không sửa).

## HTTP client (ST-104, ST-109) — bảo đảm và giới hạn

- **Chỉ** `http://`, **chỉ loopback** (`127.0.0.1`/`localhost`); **không** TLS, **không** HTTP/2.
- **Chunked**: ghép chunk (bỏ `;ext`), dừng ở chunk `0`; **chunk trailer bị bỏ qua**.
- **Redirect**: **chỉ GET**, tối đa **5** lần; theo `Location` tuyệt đối/tương đối, có `.`/`..`; query-only
  giữ path; fragment bị bỏ. `HEAD` **không** theo redirect (trả mã gốc + `Location`).
- **Không keep-alive**: mỗi request một kết nối (`Connection: close`); không cookie/auth tự động.
- **`Content-Length`** đọc đúng N byte; **timeout đọc 2 s/lần** (server im lặng ⇒ `Err` rõ).
- **`http.head`**: gửi `HEAD`, **không đọc thân** dù server gửi `Content-Length`/chunked; trả
  `{status, headers}` (headers khoá chữ thường).

### FsGuard: mức bảo đảm chống đổi tệp giữa kiểm và dùng (sau ST-124)

- **Đường đọc**: mở handle trước, quyết định quyền trên đường dẫn thật của chính handle, rồi đọc nội dung từ chính handle đó. Bất biến: nội dung trả về là nội dung của chính tệp mà quyết định quyền đã áp.
  - **Linux**: đường dẫn thật lấy qua /proc/self/fd/<fd>. /proc không đọc được thì từ chối, không quay về kiểm theo tên. Bảo đảm theo cơ chế (ST-137: 0/50 000).
  - **macOS: cửa sổ symlink của đường đọc ĐÃ ĐÓNG (ST-174, VD-176 §1).** Std an toàn không có cách lấy đường dẫn của handle, nên vẫn giữ bước xác nhận `(dev, ino)`; từ ST-174 bước kiểm dùng `std::fs::symlink_metadata` (**không đi theo symlink**) thay cho `std::fs::metadata`. Vì `canonicalize` chỉ trả **tên** và bước kiểm chạy sau đó, mục tên bị đổi thành symlink trong khe giữa hai syscall nay bị **từ chối** (`VS0604`, "bị thay đổi trong khi mở"): `(dev, ino)` của **symlink** không thể trùng `(dev, ino)` của handle đã mở. Bằng chứng **tất định** (không dựa số vòng): ca đỏ-trước 100/100 trên mã cũ, sau vá 100/100 chặn trên mac **và** `ps`, 6 triệu vòng ngẫu nhiên trên mac 0 đỏ — `agent/JOURNAL/st174-*.log`. Giới hạn **còn lại** của đường đọc: **hardlink** (mục dưới). Cách triệt để (mở từng thành phần bằng `openat` + `O_NOFOLLOW`, bỏ hẳn kiểm theo đường dẫn) là việc VS3, cùng #140/#109b.
- **Đường ghi: giới hạn đã biết, còn nguyên trong VS2.** FsGuard kiểm đường dẫn rồi mới ghi, vì mở trước để lấy handle là đã tạo hoặc cắt tệp. Trong khoảng giữa hai bước, nếu bên khác ghi được vào thư mục cha, đường dẫn có thể bị thay bằng symlink trỏ ra ngoài vùng được cấp. Sửa triệt để cần openat và O_NOFOLLOW (cần unsafe hoặc crate ngoài), việc VS3. Trên Linux khi chạy với --sandbox, Landlock chặn ghi ra ngoài vùng được cấp ở mức kernel; không có --sandbox thì giới hạn này còn nguyên.
- **Host không báo danh tính tệp** (--fs-mock, Windows): chỉ so danh tính trước và sau có khác nhau không. Chặn được "đổi rồi để nguyên", không chặn được "đổi rồi đổi lại".
- **Hardlink: giới hạn đã biết.** Hardlink nằm trong vùng được cấp nhưng trỏ tới cùng inode với tệp ngoài vùng thì đọc được, vì về mặt hệ tệp đó là cùng một tệp. Landlock cũng không chặn trường hợp này. Muốn tạo hardlink như vậy cần quyền ghi vào thư mục trong vùng và cùng hệ tệp với tệp đích.
- exists và list không trả nội dung tệp.

### Hardlink của tệp ngoài vào trong root — **giới hạn chính sách**, KHÔNG phải lỗi đua (ST-174, VD-176 §3)

**Kết luận một dòng:** một **hardlink** trỏ tới tệp **ngoài** root nhưng **nằm trong** root được **coi
là nằm trong root** — đọc được; bản vá ST-174 **không** (và không thể) chặn bằng `symlink_metadata`,
vì đây là **giới hạn chính sách**, **không** phải lỗi đua (TOCTOU).

- **Vì sao không phải lỗi đua:** kể cả khi **không** có khe thời gian nào, hardlink trong root vẫn là
  một **tên hợp lệ** trong vùng được cấp và trỏ tới **cùng inode** với tệp ngoài — hệ tệp coi đó là
  **cùng một vật** (hardlink không có "đích" để phân giải như symlink). `canonicalize` không đổi gì,
  `symlink_metadata` và `metadata` trả **cùng** `(dev, ino)` ⇒ quyết định "trong root" là **đúng chính
  sách hiện tại** ("vùng = cây tên"), **không** phải "lọt do đổi tên trong khe".
- **Điều kiện tạo được:** cần **quyền ghi** vào một thư mục **trong root** và tệp đích **cùng một hệ
  tệp** (hardlink không qua hệ tệp). Người chỉ có `cap[Dir]` **đọc** không tạo được hardlink.
- **Landlock cũng không chặn:** Landlock áp trên **đường dẫn lúc mở**, không trên nguồn gốc inode ⇒ ca
  hardlink lọt cả khi có `--sandbox`.
- **Đường ghi vẫn còn giới hạn riêng** (kiểm-rồi-ghi; xem gạch đầu dòng "Đường ghi" ở trên) — ST-174
  **không** đổi đường ghi.
- **Muốn chặn theo chính sách (không làm trong VS2):** đổi định nghĩa vùng từ "cây tên" sang "vật" (so
  `(dev, ino)` của mọi tên trong root với tệp đích), hoặc chuyển sang `openat`/`openat2` +
  `RESOLVE_BENEATH` + kiểm quyền trên **handle** ⇒ thuộc **VS3**, cùng #140/#109b. **Không** tự đổi
  chính sách trong VS2.

## Landlock chỉ Linux — **giới hạn nền**, không phải thiếu sót (ST-173, VD-175 §2)

**Kết luận một dòng:** Landlock là cơ chế **chỉ có trong nhân Linux**; `vis-host-linux` vì vậy **chỉ
biên dịch và chỉ có ca kiểm trên Linux**. Trên macOS/Windows crate này **rỗng (0 ca)**, và **mọi** tuyên
bố “kernel chặn” chỉ có bằng chứng từ Linux — đây là **giới hạn nền**, **không** phải lỗi, không phải
test bị bỏ.

### Cách cfg-hoá (ST-173)

- `vis-host-linux/Cargo.toml`: `landlock = "=0.4.7"` nằm trong
  `[target.'cfg(target_os = "linux")'.dependencies]` — trên nền khác crate **không** được kéo vào.
- `vis-host-linux/src/lib.rs` và `vis-host-linux/tests/landlock.rs` có `#![cfg(target_os = "linux")]`
  (có từ ST-132; ST-173 chỉ chuyển dependency, **không** sửa thân test).
- **Không** `#[ignore]`, **không** bỏ ca, **không** đổi kỳ vọng/ngưỡng. Trên Linux dependency **vẫn bắt
  buộc**: thiếu crate `landlock` ⇒ build Linux **đỏ** — không có đường “âm thầm bỏ bảo vệ”.

### Hệ quả (đọc kỹ trước khi trích dẫn)

1. **macOS/Windows không chạy ca Landlock.** `cargo test -p vis-host-linux` trên mac: **0 ca**
   (trước ST-173: **đỏ biên dịch** crate `landlock`, 6 lỗi, `[exit=101]`; sau ST-173: `[exit=0]`, 0 ca).
   Crate rỗng **không** phải “đã kiểm” — chỉ là “không áp dụng”.
2. **Hành vi bảo vệ chỉ đo trên Linux.** Số ca trên `ps` (Linux `7.0.0-43-generic`, LSM có `landlock`):
   **trước 15 passed / sau 15 passed** (`0 failed`, `[exit=0]`) — **KHÔNG đổi**. Log:
   `agent/JOURNAL/st173-ps-truoc.log`, `st173-ps-sau.log`.
3. **Cổng trên mac không phát hiện hồi quy Landlock.** Muốn biết Landlock còn chặn thật phải chạy
   `cargo test -p vis-host-linux --test landlock` trên **Linux** (runner `ps`).
4. Trên mac/Windows, bảo đảm duy nhất là **cưỡng chế runtime** (`FsGuard`/`NetGuard`, ST-72b) —
   **không** được suy rộng thành “có sandbox kernel”.

| Nền | Biên dịch `vis-host-linux` | Số ca Landlock chạy | Bằng chứng |
|---|---|---|---|
| Linux (`ps`, kernel `7.0.0-43-generic`) | Có — dependency **bắt buộc** | **15/15** | `st173-ps-truoc.log`, `st173-ps-sau.log` |
| macOS 27.0.1 arm64 | Có, nhưng crate **rỗng** | **0** (không áp dụng) | `st173-mac-truoc.log`, `st173-mac-sau.log` |
| Windows | — | **0** (không áp dụng) | chưa có runner |

**Không** dùng câu: “Landlock bảo vệ trên mac/Windows”; “không biên dịch được nên bỏ qua”; “mac thiếu
sandbox kernel”. Đúng: “**Landlock chỉ Linux** — mac/Windows chỉ có `FsGuard`/`NetGuard` ở tầng runtime”.

## Cây nguồn để đo phải SẠCH (ST-175, VD-177 §4)

- `bin/vis-remote` kiểm `git -C <repo> status --porcelain` trên `ps`/`psvn` **trước khi chạy**; cây bẩn ⇒
  **từ chối**, in số mục bẩn + 5 mục đầu + gợi ý, mã thoát **6**. `mac` **không** kiểm mặc định (cây làm việc
  của agent; chính công cụ ghi log vào repo) — bật bằng `--require-clean`.
- `--allow-dirty` **chỉ để chẩn đoán**, KHÔNG dùng cho phép đo. Bằng chứng: `JOURNAL/st175-chan-cay-ban.log`
  (bẩn ⇒ `[exit=6]`), `st175-cho-phep-cay-ban.log` (`--allow-dirty` ⇒ `[exit=0]`), `st175-kiem-che-do.log`.
- `git rev-parse HEAD:viscript` là hash **của commit**, KHÔNG phải cây làm việc ⇒ không trích dẫn nó như
  bằng chứng “cây sạch”; bằng chứng đúng là `git status --porcelain` rỗng + dòng `# ST-175: cay SACH` trong log.
