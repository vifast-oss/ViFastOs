<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R12-viscript — báo cáo vòng 12 (nhất quán hoá: `fmt`, `explain`, `vis test` async, LSP module-aware, `--strict-caps`), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`e392be7c`** (commit nền: `QUEUE@head` đủ **92 hàng**, 91 `DONE` +
ST-82 `READY` — báo cáo này). Số đo chạy trên **mã `8aca7f7b`** (commit cuối vòng 12; sau đó chỉ có
`QUEUE.md`). Bản máy đọc: [`R12-viscript.json`](R12-viscript.json) — **đúng 11 khoá** Khối E
(`project = "viscript"`, `round = 12`). Giống tiền lệ R11: `head` = commit **nền** của báo cáo
(không thể tự chứa hash của chính nó), nên `head` **không** phải `8aca7f7b`.

## 1. Tóm tắt (5 dòng)

1. **Vòng 12 “nhất quán hoá”** đóng 5 lỗ hổng công cụ: `vis fmt` cho cú pháp mới + `.vish` + `--check` thư
   mục (ST-77), `vis explain` đủ 3 ngôn ngữ/32 mã (ST-78), `vis test` **chạy thật `test fn`** async/kênh +
   `assert(bool)` (ST-79), **LSP module-aware** cho chẩn đoán (ST-80), `--strict-caps` (ST-81).
2. **Bug thật tìm được**: `vis fmt` từng cho **chuỗi rỗng** với `.vish` sai lệnh (mất token) — nay chặn **mọi**
   chẩn đoán khác `VS0107`; `vis test` trước đây **không chạy** `test fn` nào (TAP chỉ theo tệp) — nay mỗi test
   một dòng TAP, join cuối, task lỗi `VS0506`, deadlock `VS0505` không treo.
3. **LSP**: cho tệp **có trên đĩa**, chẩn đoán **bằng `vis check`** (nạp `std.*`/`use ./x`); tệp không có trên
   đĩa giữ đường một tệp; completion/hover vẫn một tệp (nợ 42).
4. **`--strict-caps` là opt-in**: không cờ ⇒ hành vi cũ; có cờ ⇒ chạm fs/net ngoài `cap` là **`VS0604`**
   “cần khai `cap[Dir]`/`cap[Net]`” + đường dẫn, exit 2. **Mặc định vẫn chưa siết** (nợ 33/43); Landlock
   **chỉ probe**, **chưa** cưỡng chế bằng kernel ở bất kỳ nền nào.
5. **Số đo**: **914 test**, sem **447/22 nhóm**, parity **398/398**, differential **100.000 / 0 lệch** (226,4 s,
   seed 3523158054), fuzz parser 60 s **2.585.181 ca / 0 vi phạm**, `VBC1` **53**, **7 sổ / 32 mã**, allowlist
   **6**, `fmt --check test/` **497 tệp**. ViScript **chưa hoàn thiện toàn bộ** — nợ đầy đủ ở §5, kết luận §8.

## 2. Bảng vòng 12

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-77 | `920cb957` | **`vis fmt` cú pháp mới + `.vish` + `--check` thư mục**: 12 ca (9 snapshot + lossless 35 fixture + 3 CLI); `fmt --check test/` **497 tệp** (429 `.vis` + 68 `.vish`), 136 input cố ý bỏ; **886 test**; diff 100k/0 lệch (219,3 s) |
| ST-78 | `3953f6bd` | **`vis explain` + sổ mã**: `VS0504/0505/0506` đủ vi/en/ja (câu `0506` rõ nghĩa), `--list` **32 dòng** đúng thứ tự, trợ giúp **7 khối/32 mã** đếm động; 6 ca CLI + snapshot 20→**29**; **892 test**; diff 100k/0 lệch (253,6 s) |
| ST-79 | `2a62cace` | **`vis test` chạy `test fn`** async/await/spawn/chan: TAP mỗi test, join cuối, `VS0506`, deadlock `VS0505` không treo; `assert(bool)` dùng chung interp/VM; 8 ca, **cùng byte** hai đường; **900 test**; diff 100k/0 lệch (442,6 s) |
| ST-80 | `74a48476` | **LSP module-aware**: URI→path (percent-decode), nạp `std.*`/`use ./x` cho tệp trên đĩa, chẩn đoán tệp chính **= `vis check`**; 7 ca; 5 tệp **~8 ms**; **907 test**; diff 100k/0 lệch (96,3 s) |
| ST-81 | `8aca7f7b` | **`--strict-caps`** (`run`/`test`): fs/net phải nằm trong `cap`, không cap ⇒ **`VS0604`** “cần khai `cap[Dir]`/`cap[Net]`” + đường dẫn, exit 2; **mặc định không đổi**; 7 ca; **914 test**; diff 100k/0 lệch (217,6 s) |
| ST-82 | (commit này) | báo cáo **R12**; `report-check --all` PASS; số liệu **chạy lại** trong §3 |

## 3. Số liệu chạy lại (mã `8aca7f7b`, đo 2026-11-14, macOS aarch64, `debug` trừ khi ghi khác)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **914** = 21 diag + 96 syntax + **27 fmt** + 154 types + 26 interp + 62 bytecode + 9 vm + **519 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) — không đổi | `ls test/sem/**` + `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 7 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447) | `cargo test -p vis-cli --test diag_cmp` |
| Differential | **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=226.4 toc_do=442/s seed=3523158054`** | `./bin/vis-diff` |
| Fuzz parser 60 s | **2.585.181 ca, 58.775.039 token, 43.086 ca/s, 979.584 token/s, 60,0 s, vi phạm 0** (PASS) | `target/release/vis-parse-fuzz --seconds 60 --seed 20261114` |
| `VBC1` | **53 tag** (không bump ở vòng 12) | đếm biến thể `Instr` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới ở vòng 12) | `vis explain --list` |
| Allowlist doc-examples | **6** mục | đếm dòng không `#` trong `docs/viscript/doc-examples-allow.txt` |
| 10k task async | **interp 228 ms, vm 74 ms**, hai đường cùng byte, không tràn | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| Bộ hợp chuẩn host | **54/54 ca xanh, 120 lần chạy (interp+VM), 0 lệch**, harness **4,83 s** | `./bin/vis-host-suite` / `cargo test -p vis-cli --test host_conformance -- --nocapture` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `fmt --check test/` | **exit 0**, **497 tệp** kiểm (429 `.vis` + 68 `.vish`), 136 input cố ý bỏ (3 thư mục fixture + `.err`) | `vis fmt --check test` |
| `--sandbox-check` (macOS) | `landlock: không hỗ trợ trên macos` / `cưỡng chế runtime: tắt` (fixture không cap) / `cap điều kiện: không` — **exit 3** | `vis run --sandbox-check f.vis` |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK, report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số trên do cột 3 sinh; không có số viết tay. Thứ tự đo: `cargo test --workspace` →
`vis test`/`--vm` → parity/`diag_cmp` → `async_10k` → host suite → targets → `fmt --check` → differential →
fuzz 60 s → gate.

## 4. §19 — đối chiếu **VS2** sau 12 vòng: mức thật

| # | Hạng mục VS2 | Trạng thái | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối theo thứ tự tạo, lỗi thân `VS0506`; **`vis test` chạy test async** (ST-79); 10k task **228/74 ms** | **Chưa song song thật** (run-to-completion, không nhường), **không GC**; `async` trên method/`cmd` còn `VS0201`; `async fn` trong thân `test` không hỗ trợ |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** | `chan[T](n)`/`send`/`recv`/`close`; deadlock `VS0505` (không treo); `VS0504`; 15 ca sem + 5 Rust + **test async/kênh** (ST-79); hai đường cùng byte | **Chưa `select`/timeout**; drain chỉ task `ready`; `cap=0` không dùng được; `Chan` không qua biên host |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | `NetHost` + TCP client/server loopback thật (e2e 2 tiến trình), `http.get` trong `vis-interp`; **24 ca** net/http; chỉ **loopback** | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự**, handle per-process, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** (runtime) — Landlock **CHỈ PROBE** | `cap[...]` là kiểu thật; cưỡng chế **ở biên host** (`FsGuard`/`NetGuard`), vi phạm ⇒ **trap `VS0604`** exit 2, `--grant`; **`--strict-caps`** (ST-81) bắt buộc `cap` khi bật cờ; 8 + 7 ca interp+VM | **Chưa siết mặc định**; **chưa cưỡng chế bằng kernel** ở **bất kỳ** nền nào; Linux **chưa chạy thật**; `--sandbox-check` **chỉ đọc** `/sys/.../lsm`; `fs.exists` ngoài quyền trả `false` |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | `vis lsp --stdio` (framing + JSON-RPC 2.0), diagnostics UTF-16, completion + hover; **chẩn đoán module-aware** cho tệp trên đĩa, **= `vis check`** (ST-80); **29 ca** | **Completion/hover vẫn một tệp**; chẩn đoán tệp phụ không publish; buffer chưa lưu chưa có module; full sync; hover chưa field |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local** (`./registry`/`--registry`/`VIS_REGISTRY`), manifest `files[]`, lock SHA-256 tất định, `.vis/store` content-addressed, `use pkg.*` + `VS0701`/`VS0702` | **Chưa ký số**, **chưa registry HTTP**, **chưa resolver dải** (ghim chính xác), chưa `pkg remove/update`, `.vis/store` không GC |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; **`vis-cli`×wasm check xanh nhưng không chạy được** | **Chưa có runner** nền nào ngoài macOS; chưa đo `cargo build`/link Windows; `vis-host-linux` chưa nối vào `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS**, **120 lần chạy, 0 lệch**, 4,83 s; 22 FsHost · 13 Ngoai · 11 shell · 5 quyền · 3 determinism | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng); 13 ca Ngoai kỳ vọng `VS0604` ở VM |

## 5. Lỗ hổng còn mở (nợ có chủ đích — **gộp đủ** R11 §5 (37 mục) + ST-77..ST-81)

**A. Nợ lõi R10 còn lại (ST-56..ST-61)**

1. **Shell-let chưa hạ**: `let out = ls -l` qua `check` nhưng `run` báo `out` chưa bind.
2. **`match` không khớp ở ngữ cảnh giá trị** vẫn đẩy `false` thay vì `VS0501` — nghi lệch interp ↔ VM,
   chưa dựng được ca tối thiểu.
3. Gán **lồng sâu hơn một tầng trường** (`x.f[i][j]`) là `VS0412` **có chủ đích**.
4. **Thân `impl` chưa tyck đầy đủ** — mới kiểm **đích gán**.
5. **Siêu trait bắc cầu** mới kiểm **một tầng**.
6. **`impl_pairs` per-tệp**: `impl` sinh từ `derives` ở tệp khác không thấy; dispatch xuyên tệp per-tệp.
7. **`co_float_sau` không soi bản ghi lồng** (`Eq` derives).
8. **`derives Json` chưa sinh mã** (chỉ kiểm tĩnh).
9. **`Ty::Fn` thật** cho `sort_by` (đang `Ty::FnKhoa`), **map khoá không-`str`**, **thứ tự khoá map**.
10. **Nhóm nhiều pipeline** + kiểu tĩnh **lấy stage đầu**; **lambda `where` không chú thích** ⇒ `{error}`.
11. **`lower/cmd.rs` 1.491 dòng** — sát ngưỡng R1, cần tách trước khi thêm tính năng.
12. **Async chưa song song thật**: scheduler run-to-completion, không nhường, **không GC** (chu trình `Rc`);
    `await` lồng sát ngưỡng khung 256.
13. **`async` trên method/`cmd` ⇒ `VS0201`**; **`async fn` không nhận `--tên`**.
14. **`p.x += 1` là lỗi cú pháp `VS0201`** (front end chặn trước tyck).
15. **Drain chỉ task `ready` (`spawn`)**: task `Moi` do gọi `async fn` chưa chạy ⇒ `recv` có thể `VS0505`.
16. **`close` idempotent nhưng không đánh thức ai** (không wait-set).
17. **Không `select`/timeout** trên kênh.
18. **`Chan` không truyền qua biên host**.
19. **`cap = 0` thực tế không dùng được** (hai phía chặn đều drain rồi `VS0505`).
20. **Dung lượng âm** là lỗi **lúc chạy** `VS0501`; `Chan` in `<kênh>`.

**B. Net/HTTP (ST-63..ST-66)**

21. **Chỉ loopback**; host khác ⇒ `Err`; **không TLS**, không redirect, không keep-alive, không chunked.
22. **Một luồng**: server phục vụ **tuần tự** một client, không `select`/epoll; handle **per-process**.
23. `net.read` **3 đối số** `(tay, tối_đa, timeout_ms)`; `std.net.connect` dạng **đầy đủ** chưa hạ (dùng dạng
    ngắn sau `use`); **bỏ `is_open`**; `read` trả `str` **lossy**; connect mặc định **1 s**.
24. **`vis check` chẩn đoán = exit 1** (exit 2 = dùng sai CLI); thiếu `use std.http` ⇒ **`VS0501` lúc chạy**
    (không `VS0301`) — `vis check` permissive với tên cấp tệp là **nợ có sẵn**.
25. Cap body **8 MiB**, header **64 KiB**; HTTP chỉ `http://`; ca net/http nằm ở `net*.rs`/`http.rs`, **không**
    trong sem (sem không có host net).

**C. LSP (ST-67..ST-69)**

26. **LSP chỉ module-aware ở chẩn đoán** (ST-80): tệp **có trên đĩa** nạp `std.*`/`use ./x`, chẩn đoán tệp
    chính **= `vis check`**; **completion/hover vẫn `phan_tich` một tệp**; chẩn đoán tệp phụ **không** publish;
    buffer **chưa lưu** chưa có chẩn đoán module; không cache giữa các `didChange`.
27. **Chỉ full sync** (`didChange` toàn văn), một luồng; **EOF ⇒ exit 0** (khác spec LSP — cần cho test/CI).
28. Completion **chưa theo ngữ cảnh** sau `.`/kiểu; hover **chưa field**; biến gộp **mọi phạm vi**.
29. Lower/tyck vẫn chạy khi lex/parse lỗi ⇒ có thể thêm nhiễu chẩn đoán.

**D. Gói + registry (ST-70/ST-71)**

30. **Chưa resolver dải**: phụ thuộc ghim **phiên bản chính xác**; **chưa ký số**, **chưa registry HTTP**.
31. **Lock chưa lưu `entry`** (dùng quy ước); lock chỉ tìm ở `main_dir`; `.vis/store` **không GC**.
32. Chưa `pkg remove/update`; `use pkg.*` chỉ nạp tệp **có trong lock** (đúng chủ đích, nhưng chưa có kiểm
    toàn vẹn cây ngoài `file_sha`).

**E. Quyền + Landlock (ST-72/72b/73)**

33. **Siết mặc định vẫn là nợ**: `--strict-caps` **đã có** (ST-81) nhưng **mặc định không cờ ⇒ không hạn chế**;
    muốn siết mặc định phải chuyển **toàn bộ** fixture/host suite sang khai `cap`. `cap[Net]` vẫn bị luật
    **loopback** ST-63; `fs.exists` ngoài quyền trả **`false`** im lặng (không trap — nợ mới 44);
    `main` trộn cap + tham số thường: tham số thường nhận argv; `--grant` một tên/lần.
34. **Chưa cưỡng chế bằng Landlock** ở **bất kỳ** nền nào — chỉ **probe**; Linux **chưa chạy thật** (chỉ check
    chéo); `vis-cli` **chưa dùng** crate `vis-host-linux` (logic probe lặp). Điều kiện Landlock thật: **một**
    trong hai — (a) crate `landlock` vào allowlist, hoặc (b) `unsafe` **đóng** trong `vis-host-linux` + case âm;
    cả hai phải kèm **runner Linux** để đo.
35. Ca sem `cap/` mới kiểm **tyck**; giá trị runtime chỉ đến từ tham số `main`.

**F. Đa nền tảng + hợp chuẩn (ST-74/ST-75)**

36. **Chưa có runner** Windows/wasm/Linux ⇒ mọi ô chỉ là **check biên dịch chéo**; `vis-cli` trên wasm **không
    chạy được**; chưa đo `cargo build`/link Windows (msvc cần linker); `vis-host-wasm` mới là khung `probe()`.
37. **Net không nằm trong suite host** (24 ca `net*.rs` riêng); 13 ca Ngoai khai `vm:vs0604` (VM **không** có host
    `Ngoai` — đúng thiết kế); suite phụ thuộc lệnh macOS (`/bin/echo`, `/bin/cat`, `/usr/bin/grep`, `/bin/sh`,
    `/usr/bin/false`, `/usr/bin/printf`); **Windows/Linux/wasm chưa đo**.


**G. Nợ mới ST-77..ST-81 (vòng 12)**

38. **`.vish` không chuẩn hoá thụt lề** (ST-77): `vis fmt` giữ **thụt lề gốc** của khối `cmd` (tab→4 dấu cách,
    bỏ khoảng trắng cuối, đúng 1 newline) nhưng **không** tự chuẩn hoá độ sâu; muốn chuẩn hoá phải sửa **68**
    fixture `.vish`.
39. **`usage_explain` chỉ tiếng Việt** (ST-78): trợ giúp in “7 khối, 32 mã” **đếm động** nhưng **không** theo
    `--lang`; `--list` vẫn in kèm cột severity (không thuần “mã + câu ngắn”).
40. **`assert` chỉ dạng gọi hàm** (ST-79): `assert(đk)`; **chưa** hỗ trợ `assert đk` như tài liệu; mã lỗi dùng
    `VS0501` “assert thất bại” (**không** thêm mã mới).
41. **Runner `test` biến đổi văn bản** (ST-79): mỗi `test fn` chạy bằng **bản sao tạm** đổi thành `fn main`;
    **câu lệnh cấp tệp** (nếu có) chạy lại cho **mỗi** test; `async fn` **trong** thân `test` **không** hỗ trợ
    (phải khai cấp tệp); `--vm` dùng `--no-cache` cho tệp tạm.
42. **LSP completion/hover chưa module-aware** (ST-80): chỉ **chẩn đoán** đi đường module; gợi ý/hover vẫn
    một tệp; xem thêm nợ 26.
43. **Siết mặc định chưa làm** (ST-81): cờ `--strict-caps` là **opt-in**; không cờ ⇒ hành vi cũ (không khai
    `cap` không hạn chế) — xem nợ 33; `--sandbox-check`/`--dry` **không** áp cờ.
44. **`fs.exists` ngoài quyền trả `false` im lặng** (ST-81): `FsGuard::exists` trả `false` thay vì trap
    `VS0604` như `read`/`write`/`list`; chương trình không phân biệt “không có” với “không có quyền”.

## 6. VD-100..VD-108

- **VD-100..VD-107** — đã ghi đầy đủ ở [`R11-viscript.md`](R11-viscript.md) §6 (net/HTTP, đa nền tảng, `Host` mở
  rộng, `cap` là kiểu thật, Landlock chỉ probe). Không lặp lại ở đây.
- **VD-108** — **Nhất quán hoá vòng 12**: (1) `vis fmt` phủ cú pháp mới + `.vish` chế độ **lệnh** (giữ thụt lề
  gốc), `--check` nhận **thư mục**, chỉ chặn khi có chẩn đoán **khác** `VS0107`; (2) `vis explain` là **một**
  nguồn câu cho 3 ngôn ngữ, trợ giúp đếm **động** từ sổ; (3) `vis test` **tự chạy `test fn`** bằng bản sao
  `fn main`, join cuối như `run`, thêm builtin `assert(bool)` (không mã mới); (4) LSP **chỉ** module-aware cho
  **chẩn đoán** tệp có trên đĩa — dùng `modules::load_full_voi_text` (buffer tệp chính + đĩa cho tệp phụ), chỉ
  publish chẩn đoán tệp chính; (5) `--strict-caps` **opt-in**, `Quyen.siet`, câu “cần khai `cap[...]`” cho ca
  **không** khai cap, **không** đổi câu ST-72b; mặc định vẫn không siết (nợ).

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **12 báo cáo**, **92 hàng QUEUE** phủ đủ;
  `R12-viscript.json` đúng **11 khoá**, `round=12`, `head=e392be7c` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check trong
  cổng PASS; differential **100k 0 lệch**; fuzz parser 60 s **0 vi phạm**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã **32**
  (vòng 12 **không** thêm mã); không sửa `R1..R11`.
- **Đề xuất Agent 0**: (1) lật **ST-82 = DONE**; (2) ưu tiên **siết mặc định `cap`** (chuyển toàn fixture sang
  `cap` + `--strict-caps` bật mặc định) và **`fs.exists` trap** thay vì `false`; (3) **Landlock thật** chỉ khi
  có runner Linux + một trong hai điều kiện VD-107; (4) LSP completion/hover module-aware + đa tài liệu;
  (5) giữ **54 ca host** và **497 tệp `fmt --check`** làm chuẩn R13; (6) chưa có runner Windows/wasm — mọi
  báo cáo tiếp tục ghi **mức `check`**, **không** nói “chạy”.

## 8. Kết luận trung thực — còn gì để gọi là “hoàn thiện”

Sau 12 vòng, **công cụ** đã nhất quán (fmt/explain/test/LSP/strict-caps) và **914 test** xanh, nhưng ViScript
**chưa hoàn thiện**: (1) **Landlock thật** chưa có — chỉ probe, cần **runner Linux** + duyệt crate/`unsafe`;
(2) **runner Windows/wasm/Linux** chưa có, mọi ô chỉ là **check biên dịch** (18/18), không phải “chạy”;
(3) net **chưa TLS/redirect**, một luồng, tuần tự; (4) async **chưa song song thật** (run-to-completion);
(5) kênh **chưa `select`/timeout**; (6) **quyền chưa siết mặc định**; (7) gói **chưa resolver dải/ký số**;
(8) LSP **chưa đa tài liệu**, completion/hover chưa module-aware. Đây là **nợ có chủ đích** đã đánh số ở §5,
không phải tuyên bố “đã xong”.
