<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R11-viscript — báo cáo vòng 11 (VS2 #3..#8: net/HTTP, LSP, gói/registry, quyền runtime, đa nền tảng, hợp chuẩn host), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`3b96011e`** (commit nền: `QUEUE@head` đủ **86 hàng**, 85 `DONE` +
ST-76 `READY` — báo cáo này). Số đo chạy trên **mã `2602333d`** (commit cuối vòng 11; sau đó chỉ có
`QUEUE.md`/`DECISIONS.md`). Bản máy đọc: [`R11-viscript.json`](R11-viscript.json) — **đúng 11 khoá**
Khối E (`project = "viscript"`, `round = 11`). Giống tiền lệ R10: `head` = commit **nền** của báo cáo
(không thể tự chứa hash của chính nó).

## 1. Tóm tắt (5 dòng)

1. **Vòng 11 mở 6 hạng mục VS2**: **net/HTTP** (ST-63..66), **LSP** (ST-67..69), **gói/registry**
   (ST-70/71), **quyền runtime** (ST-72/72b/73), **đa nền tảng theo mức** (ST-74), **hợp chuẩn host**
   (ST-75) — tất cả **có mặt một phần**, đúng mức đã đo, không tuyên bố quá.
2. **Quyền `cap[...]` được cưỡng chế thật ở biên host**: `Value::Cap`, `FsGuard`/`NetGuard`, vi phạm ⇒
   **trap `VS0604`** (exit 2) nêu quyền thiếu; `--grant tên=path`; 8 ca interp+VM. **Chưa** siết mặc định
   (không khai `cap` ⇒ không hạn chế), **chưa** Landlock thật.
3. **Net/HTTP chỉ loopback**, một luồng, phục vụ tuần tự, **không** TLS/redirect/keep-alive/chunked;
   **gói/registry local**, lock SHA-256, **chưa** ký số/registry HTTP; **LSP** 22 phản hồi, **chưa**
   module-aware.
4. **Đa nền tảng: chỉ `cargo check` chéo 18/18 ô** — **chưa chạy** trên Windows/wasm/Linux; macOS có
   **54 ca host chạy thật / 120 lần chạy / 0 lệch**. **Landlock: chỉ probe** (`--sandbox-check`), **chưa
   cưỡng chế bằng kernel** ở bất kỳ nền nào.
5. **Số đo**: **874 test**, sem **447/22 nhóm**, parity **398/398**, differential **100.000 / 0 lệch**
   (352,5 s, seed 3523158054), fuzz parser 60 s **4.776.316 ca / 0 vi phạm**, `VBC1` **53**, **7 sổ / 32
   mã**, allowlist **6**. ViScript **chưa hoàn thiện toàn bộ** — nợ đầy đủ ở §5.

## 2. Bảng vòng 11

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-63 | `29f236aa` | **VS2 #3a — `NetHost` + TCP client**: 7 method, handle `str` mờ, `VS0604` không host; chỉ **loopback**; tách `interp/goi.rs` giữ R1; 6 ca Rust loopback thật; **793 test**, parity 395/395, diff 100k/0 lệch (215,3 s) |
| ST-64 | `609d313d` | **VS2 #3b — `http.get`**: giao thức HTTP trong `vis-interp` (hai đường cùng byte), `http://`+loopback, cap body 8 MiB, non-2xx ⇒ `Err`; 7 ca Rust; **802 test**, diff 100k/0 lệch (233,0 s) |
| ST-65 | `6514fce1` | **VS2 #3c — server TCP**: `listen/local_addr/accept(_timeout)/close_listen`, handle `L`, accept 0 = chặn; 5 ca gồm **e2e 2 tiến trình** qua tệp; **807 test**, diff 100k/0 lệch (257,7 s) |
| ST-66 | `f1654ef5` | **VS2 #3d — biên net/http**: **24 ca** (read/write sau close, cap 64, accept 2 lần, path+query, body rỗng, `--explain-perms`, cache hit) — **813 test**, không mã mới, diff 100k/0 lệch (217,0 s) |
| ST-67 | `c35a0c00` | **VS2 #5a — khung LSP**: `vis lsp --stdio`, framing `Content-Length`, JSON-RPC 2.0 (initialize/shutdown/exit, mã lỗi), stdout sạch; 8 snapshot; **821 test**, diff 100k/0 lệch (282,6 s) |
| ST-68 | `ab1ed6c8` | **VS2 #5b — diagnostics + UTF-16**: `didOpen/didChange/didClose` ⇒ `publishDiagnostics` đúng đường `vis check`; vị trí 0-based UTF-16 tab=1; 7 snapshot + 6 unit (emoji `0:13–0:14`); **834 test**, diff 100k/0 lệch (313,5 s) |
| ST-69 | `40f32a40` | **VS2 #5c — completion + hover**: tổng **22 phản hồi**; item/biến/builtin/từ khoá, hover markdown + range UTF-16; tách tệp LSP ≤300 dòng; **841 test**, diff 100k/0 lệch (214,1 s) |
| ST-70 | `7743e0e2` | **VS2 #6a — `vis pkg`**: registry **local**, manifest `files[]` tường minh, chuẩn hoá BOM/CRLF, `tree_sha`, lock tất định, `.vis/store` content-addressed; 8 ca; **849 test**, diff 100k/0 lệch (274,1 s) |
| ST-71 | `d85e9665` | **VS2 #6b — `use pkg.*`**: nạp theo `vis.lock`, `rel = pkg/<tên>/<ver>/<path>`, so `file_sha` ⇒ `VS0701`, vòng gói ⇒ `VS0702`; 5 ca; **854 test**, diff 100k/0 lệch (246,6 s) |
| ST-72 | `4094a8d0` | **VS2 #4a — `cap[...]` là kiểu thật**: `TypeRef{"cap",[…]}` (Dir/File/Net/Proc/Env/Term), `@`/`(write)` kiểm ở lower ⇒ `VS0412`, `--dry` in quyền từ CST; 6 ca sem nhóm `cap/` + 4 Rust; **858 test**, sem 447/22, parity 398/398, diff 100k/0 lệch (286,7 s) |
| ST-72b | `0d7e9dba` | **VS2 #4b — cưỡng chế ở biên host**: `Value::Cap`, bind tham số `cap` của `main` từ CST, `FsGuard` canonicalize + `starts_with(root)`, `NetGuard`, vi phạm ⇒ **trap `VS0604`** (exit 2), `--grant`; 8 ca interp+VM; **866 test**, diff 100k/0 lệch (196,0 s) |
| ST-73 | `f9c201ba` | **VS2 #4c — probe Landlock + khung Linux**: `vis run --sandbox-check` (**chỉ đọc**, exit 0/3/4), khung `vis-host-linux` ngoài `viscript/` (check chéo **xanh**), `HOST-BAO-DAM.md`; 6 ca + 1 cfg(linux); **872 test**, diff 100k/0 lệch (101,5 s) |
| ST-74 | `0af15902` | **VS2 #7 — đa nền tảng theo mức**: ma trận **18/18 ô PASS** (8 crate × windows/wasm + 2 khung), `bin/vis-targets`, khung `vis-host-wasm`, 1 smoke test; **873 test**, diff 100k/0 lệch (194,7 s); **`vis-cli`×wasm check xanh nhưng không chạy được** |
| ST-75 | `2602333d` | **VS2 #8 — hợp chuẩn host**: `test/host/suite.txt` **54 ca / 120 lần chạy / 0 lệch** trên macOS (FS 22 · Ngoai 13 · shell 11 · quyền 5 · determinism 3), harness so byte interp+VM, `bin/vis-host-suite`; **874 test**, diff 100k/0 lệch (196,3 s) |
| ST-76 | (commit này) | báo cáo **R11**; `report-check --all` PASS; số liệu **chạy lại** trong §3 |

## 3. Số liệu chạy lại (mã `2602333d`, đo 2026-11-14, macOS aarch64, `debug` trừ khi ghi khác)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **874** = 21 diag + 96 syntax + 18 fmt + 154 types + 26 interp + 62 bytecode + 9 vm + **488 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) | `ls test/sem/**` + `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 7 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447) | `cargo test -p vis-cli --test diag_cmp` |
| Differential | **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=352.5 toc_do=284/s seed=3523158054`** | `./bin/vis-diff` |
| Fuzz parser 60 s | **4.776.316 ca, 108.586.410 token, 79.605 ca/s, 1.809.773 token/s, 60,0 s, vi phạm 0** (PASS) | `cargo build --release -p vis-syntax --bin vis-parse-fuzz` + `target/release/vis-parse-fuzz --seconds 60 --seed 20261114` |
| `VBC1` | **53 tag** (không bump ở ST-63..ST-75) | đếm biến thể `Instr` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới ở vòng 11) | `vis explain --list` |
| Allowlist doc-examples | **6** mục (10 dòng gồm chú thích) | `wc -l ../docs/viscript/doc-examples-allow.txt` + đếm dòng không `#` |
| 10k task async | **interp 328 ms, vm 182 ms**, hai đường cùng byte, không tràn | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| Bộ hợp chuẩn host | **54/54 ca xanh, 120 lần chạy (interp+VM+determinism), 0 lệch**, harness **3,9 s** | `./bin/vis-host-suite` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `--sandbox-check` (macOS) | `landlock: không hỗ trợ trên macos` / `cưỡng chế runtime: bật` / `cap điều kiện: fs` — **exit 3** | `vis run --sandbox-check f.vis` (với `main(d: cap[Dir] @ p".")`) |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK, report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số trên do cột 3 sinh; không có số viết tay. Thứ tự đo: `cargo test --workspace` →
`vis test`/`--vm` → parity/`diag_cmp` → `async_10k` → host suite → targets → differential → fuzz 60 s → gate.

## 4. §19 — đối chiếu **VS2**: đã có mặt / chưa có + mức thật

| # | Hạng mục VS2 | Trạng thái sau vòng 11 | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối theo thứ tự tạo, lỗi thân `VS0506`; 10k task **328/182 ms** | **Chưa song song thật** (run-to-completion, không nhường), **không GC**; `async` trên method/`cmd` còn `VS0201` |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** | `chan[T](n)`/`send`/`recv`/`close`; deadlock `VS0505` ~0,09 s (không treo); `VS0504`; 15 ca sem + 5 Rust; hai đường cùng byte | **Chưa `select`/timeout**; drain chỉ task `ready`; `cap=0` không dùng được; `Chan` không qua biên host |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | `NetHost` + TCP client/server loopback thật (e2e 2 tiến trình), `http.get` trong `vis-interp`; **24 ca** net/http; chỉ **loopback** | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự** một client, handle per-process, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** (runtime) — Landlock **CHỈ PROBE** | `cap[...]` là kiểu thật; cưỡng chế **ở biên host** (`FsGuard`/`NetGuard`), vi phạm ⇒ **trap `VS0604`** exit 2, `--grant`; 8 ca interp+VM | **Chưa siết mặc định** (không khai `cap` ⇒ không hạn chế); **chưa cưỡng chế bằng kernel** ở **bất kỳ** nền nào; Linux **chưa chạy thật**; `--sandbox-check` **chỉ đọc** `/sys/.../lsm` |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | `vis lsp --stdio` (framing + JSON-RPC 2.0), diagnostics UTF-16, completion + hover; **22 phản hồi** | **Chưa module-aware** (`use`/`std.*` có thể khác `vis check`); full sync; completion chưa theo `.`/kiểu; hover chưa field; lower/tyck chạy cả khi lex/parse lỗi |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local** (`./registry`/`--registry`/`VIS_REGISTRY`), manifest `files[]`, lock SHA-256 tất định, `.vis/store` content-addressed, `use pkg.*` + `VS0701`/`VS0702` | **Chưa ký số**, **chưa registry HTTP**, **chưa resolver dải** (ghim phiên bản chính xác), chưa `pkg remove/update`, `.vis/store` không GC, lock chỉ ở `main_dir` |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; **`vis-cli`×wasm check xanh nhưng không chạy được** (process/net/fs/thread) | **Chưa có runner** nền nào ngoài macOS; chưa đo `cargo build`/link Windows; `vis-host-linux` chưa nối vào `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS**, **120 lần chạy, 0 lệch**; 22 FsHost · 13 Ngoai · 11 shell · 5 quyền · 3 determinism | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng); 13 ca Ngoai có kỳ vọng `VS0604` ở VM (VM không có host `Ngoai`) |

## 5. Lỗ hổng còn mở (nợ có chủ đích — **gộp đủ** R10 §5 + ST-63..ST-75)

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

26. **Chưa module-aware**: `Use`/`std.*` có thể khác `vis check`; chưa đọc `params` đầy đủ.
27. **Chỉ full sync** (`didChange` toàn văn), một luồng; **EOF ⇒ exit 0** (khác spec LSP — cần cho test/CI).
28. Completion **chưa theo ngữ cảnh** sau `.`/kiểu; hover **chưa field**; biến gộp **mọi phạm vi**.
29. Lower/tyck vẫn chạy khi lex/parse lỗi ⇒ có thể thêm nhiễu chẩn đoán.

**D. Gói + registry (ST-70/ST-71)**

30. **Chưa resolver dải**: phụ thuộc ghim **phiên bản chính xác**; **chưa ký số**, **chưa registry HTTP**.
31. **Lock chưa lưu `entry`** (dùng quy ước); lock chỉ tìm ở `main_dir`; `.vis/store` **không GC**.
32. Chưa `pkg remove/update`; `use pkg.*` chỉ nạp tệp **có trong lock** (đúng chủ đích, nhưng chưa có kiểm
    toàn vẹn cây ngoài `file_sha`).

**E. Quyền + Landlock (ST-72/72b/73)**

33. **Chưa siết mặc định**: chương trình **không khai `cap` ⇒ không hạn chế**; cần `--strict-caps` + cập nhật
    toàn fixture. `cap[Net]` vẫn bị luật **loopback** ST-63; `fs.exists` bị chặn trả **`false`** (không trap);
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

## 6. VD-100..VD-107 (mỗi quyết định một dòng)

- **VD-100** — **Net/HTTP**: socket thô ở `NetHost` (host), **giao thức HTTP trong `vis-interp`** ⇒ hai đường
  cùng byte, codegen không sửa; **chỉ loopback**, timeout ≤400 ms; lỗi mạng = `Err`, thiếu host = `VS0604`,
  **không mã mới**.
- **VD-101** — **Gói + registry**: **không mạng**, registry là **thư mục local**; phụ thuộc **phiên bản chính
  xác** (không semver range); manifest/lock JSON bằng `vis_interp::json`; `files[]` tường minh + SHA-256;
  cài vào `.vis/pkg/<tên>/<ver>/`, store content-addressed.
- **VD-102** — **LSP**: tự viết framing `Content-Length` trên stdin/stdout nhị phân (không `println!`), thoát khi
  `exit` hoặc EOF; JSON dùng lại `vis_interp::json`; nguồn chẩn đoán = `parse`+`lex`+`lower`+`check`; vị trí
  **0-based UTF-16, tab=1**; **không** thêm crate.
- **VD-103** — **Quyền ở biên host**: cưỡng chế trong `FsHost`/`NetHost` (không ở interp/VM) ⇒ hai đường cùng
  byte; vi phạm ⇒ **`VS0604`** nêu quyền thiếu + đường dẫn; `--cap-mock` là ý tưởng test tất định; Landlock thật
  **chỉ** Linux và phải ghi "chưa đo" trên macOS — **cấm** nói "đã có Landlock".
- **VD-104** — **Đa nền tảng theo mức**: windows/wasm chỉ đòi **`cargo check` xanh**, **không** tuyên bố "chạy";
  crate khung host **ngoài `viscript/`**; lõi **không** `cfg(target_os)`; `host-conformance` ≥50 ca chạy **thật**
  trên macOS, 3 nền còn lại ghi **"chưa đo"**; không thêm dependency.
- **VD-105** — **`Host` mở rộng**: thêm `NetHost` cạnh `FsHost` (handle `str` mờ), `Host::net` default `None` ⇒
  `VS0604`; HTTP trong `vis-interp`; **chỉ loopback**; không mã lỗi mới; tham số tuỳ chọn **tách tên hàm**;
  **R1 trước, code sau** (ST-63 tách `interp/goi.rs` trước khi thêm net); ca net/http ở Rust, **không** vào sem.
- **VD-106** — **`cap[...]` là kiểu thật, cưỡng chế chỉ khi khai báo**: chỉ nhận qua **tham số `main`**; `@` +
  `(write)` giữ ở CST; `--dry` in quyền rồi thoát 0; cưỡng chế ở `HostFs`/`HostNet`; **không khai ⇒ không hạn
  chế** (nợ siết mặc định); `--grant <tên>=<path>` ghi đè, **không** thêm quyền; Landlock chỉ Linux, macOS
  **không giả vờ**.
- **VD-107** — **Landlock chỉ probe + khung**: macOS không đo được, R2 cấm `unsafe`, allowlist đóng ⇒ **không**
  gọi syscall; `vis run --sandbox-check` chỉ **đọc** `/sys/kernel/security/lsm`; `vis-host-linux` ngoài
  `viscript/`, `cfg(target_os="linux")`, không syscall; bằng chứng chỉ **check chéo**; muốn thật cần **một**
  trong hai: crate `landlock` vào allowlist **hoặc** `unsafe` đóng + runner Linux.

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **11 báo cáo**, **86 hàng QUEUE** phủ đủ;
  `R11-viscript.json` đúng **11 khoá**, `round=11`, `head=3b96011e` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check trong
  cổng PASS; differential **100k 0 lệch**; fuzz parser 60 s **0 vi phạm**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã **32**
  (vòng 11 **không** thêm mã).
- **Đề xuất Agent 0**: (1) lật **ST-76 = DONE**; (2) ưu tiên **siết mặc định `cap`** (`--strict-caps`) và
  **Landlock thật** theo điều kiện VD-107 (4) khi có runner Linux; (3) mở parser cho `async` trên method/`cmd`
  (đổi kỳ vọng `async_vis.rs` `VS0201` → `VS0412`); (4) giữ **net ngoài sem** và **54 ca host** làm chuẩn R12;
  (5) chưa có runner Windows/wasm — mọi báo cáo tiếp tục ghi **mức `check`**, **không** nói "chạy".
