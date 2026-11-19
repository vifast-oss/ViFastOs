# KT-181 — quét thân `macro_rules!` ngoài `v2/lib` và `v2/kernel/*/src` (D2-164)

**Ngày**: 2026-11-19/20 · **Agent 1 (lead) → sub-agent L37** · nhánh `night3/L37-kt181` ·
worktree `/private/tmp/night3-L37-kt181` · HEAD `8f5a1189` (từ `main`).
**Nguồn đọc trước**: `AGENTS.md`, `v2/AGENTS.md`, `v2/tools/check-wake-paths.py`,
`v2/agent/reports/CHECKER-WAKE-CONTRACT.md`, `v2/agent/JOURNAL/kt171-wakefix3.md` §4.4,
`v2/agent/JOURNAL/kt177-wakefix4.md`.
**Ngân sách**: 2,5 h. Không merge, không push, không `gate --fast`, không QEMU.

## 0. Kết luận trước

| Câu hỏi | Trả lời | Căn cứ |
|---|---|---|
| Có **lỗ hổng thật** (macro sinh `wake()` trong mã sản phẩm mà checker bỏ lọt) không? | **KHÔNG** — không dựng được đường lọt | §1.2 (M1b/M1c) |
| Có **khoảng trống phạm vi** (thân macro không được kiểm token `wake`) không? | **CÓ** — `init/`, `vifast-plat-user/`, `tests/` của kernel/sim | §1.1 (M1/M3/M4/M5) |
| Đã vá? | **CÓ** — một hàm `scan_macro_bodies`, ba gốc, hai luật | §3 |
| Selftest | **118/118 → 128/128** (10 ca mới; 118 ca cũ **không mất ca nào**) | §4 |
| `./v2/bin/v2 check` | **PASS** trên cây thật sau vá | `kt181-logs/after-v2-check.log` |

**Tóm tắt một dòng**: đây là **phòng ngừa** (đóng khoảng trống phạm vi), **không** phải vá một
lỗ hổng đã bị khai thác. Không thổi phồng: mọi đường lọt khả dĩ đều đã bị chặn bởi cơ chế khác
(`_check_wake_text` cho `vifast-plat-*/src/`, `extra` cho tệp bị `#[path]`/`include!` kéo vào).

## 1. Xác định lỗ hổng bằng bằng chứng (không suy đoán)

### 1.1 Khoảng trống phạm vi — có thật

Hai hàm quét thân macro **trước** KT-181 có phạm vi **khác nhau**:

| Hàm | Gốc quét | Luật token `wake` | Luật `include!`/`#[path]`/`mod`/`:tt` |
|---|---|---|---|
| `scan_lib_macros` | `v2/lib/**` | **có** (`E_STRICT_MACRO_WAKE`) | có (`_check_macro_body`) |
| `scan_kernel_macro_includes` | `v2/kernel/**` + `v2/sim/**` | **không** | có (`_check_macro_body`) |

Luật `wake` toàn văn bản (`_check_wake_text`) chỉ phủ `v2/kernel/vifast-core/src/`,
`v2/sim/vifast-sim/src/`, `v2/kernel/vifast-kbin-riscv64/src/`, `v2/kernel/vifast-plat-*/src/`
(`in_test_only_wake_scope`). Vì vậy thân macro ở các thư mục **không** thuộc phạm vi đó **không**
bị luật token bắt. Đột biến thật (mỗi đột biến: thêm tệp, chạy checker, `git checkout --`/xoá):

| Mã | Tệp đột biến | Trước vá | Log |
|---|---|---|---|
| M1 | `v2/kernel/vifast-kbin-riscv64/init/evil_wake.rs` (`#[macro_export] macro_rules! wake_call { ($m,$t) => { $m.wake($t) } }`) | **XANH** (RC=0) | `kt181-logs/m1-init-before.log` |
| M3 | `v2/kernel/vifast-plat-user/lib.rs` (cùng macro) | **XANH** (RC=0) | `kt181-logs/m3-plat-user-before.log` |
| M4 | `v2/kernel/vifast-plat-common/tests/evil_wake.rs` (cùng macro) | **XANH** (RC=0) | `kt181-logs/m4-plat-common-tests-before.log` |
| M5 | `v2/sim/vifast-sim/tests/evil_wake.rs` (cùng macro) | **XANH** (RC=0) | `kt181-logs/m5-sim-tests-before.log` |

**Đối chứng ngược — `vifast-plat-common/src/` KHÔNG hở** (đề bài nghi ngờ thư mục này):
M2 đặt cùng macro vào `v2/kernel/vifast-plat-common/src/lib.rs` ⇒ checker **ĐỎ**
`E_STRICT_TEST_ONLY_WAKE` (do `_check_wake_text` phủ `vifast-plat-*/src/`), log
`kt181-logs/m2-plat-common-src.log`. Vậy `vifast-plat-common` chỉ hở phần `tests/`, không hở `src/`.

### 1.2 Không có đường lọt thật — bằng chứng

Đột biến M1 (macro trong `init/`) **không thể** được gọi từ mã sản phẩm kernel:

- **M1b** — thêm lời gọi `wake_call!(m, t)` vào `v2/kernel/vifast-kbin-riscv64/src/main.rs`
  (không nạp tệp `init/`): `cargo check --manifest-path v2/kernel/Cargo.toml --target
  riscv64gc-unknown-none-elf -p vifast-kbin-riscv64` ⇒ **RC=101**,
  `error: cannot find macro wake_call in this scope` (`kt181-logs/m1b-call-without-include.log`).
  `init/` không phải module của crate nào — `build.rs` biên dịch từng chương trình U-mode bằng
  `rustc` trực tiếp với `--extern` **không** có `vifast_core` (`v2/kernel/vifast-kbin-riscv64/build.rs`
  `build_user_elf_with`).
- **M1c** — nạp `init/evil_wake.rs` bằng `#[path = "../init/evil_wake.rs"] mod evil_wake;` rồi gọi:
  `cargo check` **RC=0** (mã biên dịch được — đúng kịch bản "đột biến thật"), nhưng checker
  **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` tại `init/evil_wake.rs:9` (`kt181-logs/m1c-call-with-path.log`).
  Lý do: `resolve_include_targets` phân giải `#[path]` trong `v2/kernel`, đưa tệp đích vào `extra`,
  và `_check_wake_text` quét `extra` **bất kể** phạm vi. Vậy **mọi** đường đưa macro từ `init/`
  vào kernel đều bị bắt.

Tương tự cho `vifast-plat-user/` (shim U-mode, biên dịch thành rlib cho chương trình U-mode, không
link vào kernel) và `tests/` (integration test, không phải sản phẩm). **Kết luận trung thực**:
không có đường lọt hiện tại; việc mở rộng phạm vi là **phòng ngừa** để một thay đổi cấu trúc
sau này (ví dụ `init/` được `include!` vào kernel mà không qua `#[path]`, hoặc crate mới) không
âm thầm mở lỗ.

## 2. Đã sửa gì

Chỉ **siết**: không nới ngưỡng, không thêm allowlist, không `#[ignore]`/`#[should_panic]`,
không đổi kỳ vọng của ca cũ. Toàn bộ thay đổi logic nằm trong `v2/tools/check-wake-paths.py`.

### 2.1 Hợp nhất hai hàm quét macro

- Thêm `MACRO_SCAN_ROOTS = ("v2/lib", "v2/kernel", "v2/sim")`.
- Thay `scan_lib_macros` + `scan_kernel_macro_includes` bằng **một** `scan_macro_bodies`:
  mỗi thân `macro_rules!` ở **mọi** tệp thuộc ba gốc được kiểm **cả hai** luật:
  1. token `wake` ⇒ `E_STRICT_MACRO_WAKE` (trừ `MACRO_WAKE_ALLOW` có lý do — hiện **rỗng**);
  2. `include!`/`#[path]`/`cfg_attr(path)`/`mod`/`$…:tt` ⇒ `_check_macro_body` (fail-closed).
- Cập nhật call site trong `scan_v2` (mục 7) và các chú thích liên quan.
- **Ngoài** phạm vi (lý do ghi ở hợp đồng §1/§4): `v2/test/fixtures/**` (fixture cố tình sai),
  `v2/tools/**` (công cụ phát triển, không link vào ảnh sản phẩm).

### 2.2 Hợp đồng

`v2/agent/reports/CHECKER-WAKE-CONTRACT.md`: §1 (bảng cây + đoạn KT-181), §2 (mục KT-181),
§3 (dòng `E_STRICT_MACRO_WAKE` cho mọi gốc), §4 (hai dòng: macro ngoài `MACRO_SCAN_ROOTS`;
**không có đường lọt thật** — ghi rõ để không thổi phồng), §5 (ba đường ngoài `v2/kernel`/`v2/sim`).

## 3. Đột biến đỏ/xanh (bảng đầy đủ)

Mỗi đột biến: thêm tệp/nối mã, chạy checker, rồi `git checkout --` + xoá tệp mới. Cây sạch
trước commit (chỉ `v2/tools/check-wake-paths.py` + hợp đồng + journal/log).

| Tên | Lệnh | Trước vá | Sau vá | Log |
|---|---|---|---|---|
| M1 `init/` macro `wake` | `python3 v2/tools/check-wake-paths.py --strict` | XANH (RC=0) | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `init/evil_wake.rs:5` | `m1-init-before.log` / `m1-init-after.log` |
| M1b gọi từ kernel **không** nạp | `cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf -p vifast-kbin-riscv64` | RC=101 `cannot find macro` | (không đổi) | `m1b-call-without-include.log` |
| M1c nạp bằng `#[path]` + gọi | `cargo check …` rồi checker `--strict` | cargo RC=0, checker **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` | cargo RC=0, checker **ĐỎ** (thêm `E_STRICT_MACRO_WAKE`) | `m1c-call-with-path.log` / `m1c-call-with-path-after.log` |
| M2 `vifast-plat-common/src/` macro `wake` | checker `--strict` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` | **ĐỎ** (không đổi) | `m2-plat-common-src.log` |
| M3 `vifast-plat-user/lib.rs` macro `wake` | checker `--strict` | XANH (RC=0) | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `lib.rs:392` | `m3-plat-user-before.log` / `m3-plat-user-after.log` |
| M4 `vifast-plat-common/tests/` macro `wake` | checker `--strict` | XANH (RC=0) | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `evil_wake.rs:5` | `m4-plat-common-tests-before.log` / `m4-plat-common-tests-after.log` |
| M5 `vifast-sim/tests/` macro `wake` | checker `--strict` | XANH (RC=0) | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `evil_wake.rs:5` | `m5-sim-tests-before.log` / `m5-sim-tests-after.log` |
| M6 `v2/tools/` macro `wake` (ngoài phạm vi) | checker `--strict` | XANH (RC=0) | XANH (RC=0) — **có chủ đích**, lý do ở hợp đồng §4 | `m6-tools-before.log` / `m6-tools-after.log` |
| C1–C4 đối chứng `wake_from` **cùng thư mục** | checker `--strict` | XANH | XANH (không báo oan) | `c1-init-after.log` … `c4-sim-tests-after.log` |

## 4. Selftest — trước/sau

| | Số ca | Kết quả |
|---|---|---|
| Trước vá (HEAD `8f5a1189`) | **118** | 118/118 ok (`kt181-logs/baseline-selftest.log`) |
| Sau vá | **128** | 128/128 ok (`kt181-logs/after-selftest.log`) |

**10 ca mới** (đều nằm trong `--selftest`):

| # | Ca | Trước vá | Sau vá |
|---|---|---|---|
| 1 | `KT-181 init/ macro wake ⇒ đỏ` | **FAIL** (checker xanh) | PASS |
| 2 | `KT-181 init/ macro wake_from (đối chứng) ⇒ xanh` | PASS | PASS |
| 3 | `KT-181 init/ macro chuỗi "wake" (đối chứng nhiễu) ⇒ xanh` | PASS | PASS |
| 4 | `KT-181 vifast-plat-user macro wake ⇒ đỏ` | **FAIL** | PASS |
| 5 | `KT-181 vifast-plat-user macro wake_from (đối chứng) ⇒ xanh` | PASS | PASS |
| 6 | `KT-181 vifast-plat-common/tests macro wake ⇒ đỏ` | **FAIL** | PASS |
| 7 | `KT-181 vifast-plat-common/tests macro wake_from (đối chứng) ⇒ xanh` | PASS | PASS |
| 8 | `KT-181 sim/tests macro wake ⇒ đỏ` | **FAIL** | PASS |
| 9 | `KT-181 sim/tests macro wake_from (đối chứng) ⇒ xanh` | PASS | PASS |
| 10 | `KT-181 v2/tools macro wake (ngoài phạm vi) ⇒ xanh` | PASS | PASS |

**Bằng chứng "có răng"**: chạy **bộ ca mới** trên **checker cũ** (HEAD `8f5a1189`, chèn nguyên
khối ca KT-181) ⇒ `# selftest: 124/128 ok`, đúng **4 ca ĐỎ** fail (`kt181-logs/selftest-new-cases-on-old-checker.log`).
Trên checker mới ⇒ 128/128. **118 ca cũ không mất ca nào** (so khớp tên ca: `comm` rỗng).

## 5. Giới hạn còn lại (ghi rõ, không tuyên bố đã đóng)

1. **`v2/tools/**` và `v2/test/fixtures/**`** không được quét thân macro — **có chủ đích**
   (công cụ phát triển / fixture cố tình sai). Nếu tương lai có mã sản phẩm ở đó, phải thêm gốc
   vào `MACRO_SCAN_ROOTS`; hiện **không** tuyên bố bảo vệ.
2. **`#[path]`/`include!` ngoài thân macro trong `v2/lib`** vẫn không được phân giải
   (`resolve_include_targets` chỉ lặp `v2/kernel`+`v2/sim`) — sai lệch **có trước**, ghi ở hợp
   đồng §4; ngoài phạm vi KT-181.
3. **Lời gọi macro** (`_walk_items` bỏ qua nhóm sau `!`) — sai lệch **có trước**, ghi ở hợp đồng §4.
4. **Không có đường lọt thật** cho `wake` ngoài phạm vi cũ (M1b/M1c) — đây là phòng ngừa, không
   phải lỗ hổng đã bị khai thác. Không được báo cáo như "đã đóng lỗ hổng".
5. Không chạy `gate --fast` (luật task); chỉ `./v2/bin/v2 check`.

## 6. Luật đã giữ

- Không sửa `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/QUYET-DINH-sau-review.md`,
  `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`.
- Không `#[ignore]`/`#[should_panic]`, không nới ngưỡng, không retry để lấy xanh.
- Không `pkill -f qemu-system`; task không dùng QEMU.
- Không push, không merge, không tạo worktree mới.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit.
