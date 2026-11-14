<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R15-viscript — báo cáo vòng 15 (`cap[Proc]` dưới `--strict-caps`, `.vish` `main` chỉ khai cap, kênh `chan(0)` bị từ chối), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`dc754a6b`** (commit nền: `QUEUE@head` đủ **108 hàng**, **107 `DONE`**
+ ST-95 `READY` — báo cáo này). Số đo chạy trên **mã `fe6d459b`** (commit cuối vòng 15; sau đó chỉ có
`QUEUE.md`). Bản máy đọc: [`R15-viscript.json`](R15-viscript.json) — **đúng 11 khoá** Khối E
(`project = "viscript"`, `round = 15`). Như tiền lệ R11–R14: `head` = commit **nền** của báo cáo (không thể
tự chứa hash của chính nó), nên `head` **không** phải `fe6d459b`.

## 1. Tóm tắt (5 dòng)

1. **ST-93 — `cap[Proc]`**: `--strict-caps` buộc chương trình ngoài phải khai `cap[Proc]`; `Ngoai::Runner`
   kiểm **tại biên host** dùng chung interp + VM, thiếu ⇒ **`VS0604: [quyền] cần khai \`cap[Proc]\``,
   exit 2; mặc định **không đổi**. Đọc được `cap` từ chữ ký `main` của `.vish`; pipeline cấp tệp là entry.
2. **ST-93b — đóng chỗ im lặng**: `.vish` `fn main` có **câu lệnh** ⇒ **`VS0201`** tại câu lệnh đầu
   (“chỉ dùng để khai `cap`…”), exit 2, **không chạy**; thân rỗng/`pass` vẫn hợp lệ; `.vis` không đổi.
3. **ST-94 — kênh**: chọn **nhánh A** — `chan[T](0)` **bị từ chối ngay khi tạo** (`VS0505`, không treo,
   đo **0,01 s**), **không** giả vờ rendezvous; `recv` sau close ⇒ `none`, `send` sau close ⇒ `VS0504`,
   `recv` rỗng không task ⇒ `VS0505`; drain task `Moi` 100 task **đúng thứ tự spawn**.
4. **Số đo**: **999 test**, sem **447/22 nhóm**, `vis test` **447 ok** (`--vm` **447 ok + 0 SKIP**), host
   **54/54 × 2**, parity **398/398 + 49**, differential **100.000 / 0 lệch** (163,4 s), cache **520×4 /
   0 lệch**, fuzz parser 60 s **2.462.470 ca / 0 vi phạm**, targets **18/18 `check`**, fmt **497 tệp**.
5. **Trung thực**: `chan(0)` runtime trả **exit 1**, không phải 2 như đề — static `VS0412` đã thử nhưng đẩy
   **50 lỗi biên dịch** (parity 397+50) và `.err VS05` quy ước exit 1, nên chọn runtime + giữ **398+49**;
   không nới ngưỡng, không `#[ignore]`, không sửa golden/allowlist; `.vish` `main`+pipeline **bỏ main** khi
   thân rỗng (ghi nợ 69).

## 2. Bảng vòng 15

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-93 | `3e1ea655` | **`cap[Proc]` dưới `--strict-caps`**: `Quyen::cho_phep_proc` + `Ngoai::Runner` trap **`VS0604`** trước spawn (interp+VM), `cap[Proc]` = quyền chạy tiến trình **không allowlist**; `ds_cap` đọc `main` `.vish`; 6 ca; **984 test** |
| ST-93b | `f79589ee`, `fc8717b4` | **`.vish` `fn main` chỉ khai cap**: thân có câu lệnh ⇒ **`VS0201`** tại câu lệnh đầu (bỏ qua `pass`), không chạy; `.vis` không đổi; 5 ca; **989 test**; fixture `cap_proc` `let`→`pass` |
| ST-94 | `fe6d459b` | **Kênh nhánh A**: `chan[T](0)` **bị từ chối runtime** `VS0505` (exit 1, **0,01 s**), literal/biến/trong hàm; close/recv/send; drain task `Moi` 100 task đúng thứ tự; 10 ca; **999 test** |
| ST-95 | (commit này) | báo cáo **R15**; `report-check --all` PASS; số liệu **chạy lại** trong §3 |

## 3. Số liệu chạy lại (mã `fe6d459b`, đo 2026-11-14, macOS aarch64, `debug` trừ khi ghi khác)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **999** = 21 diag + 96 syntax + **27 fmt** + 154 types + 26 interp + 62 bytecode + 9 vm + **604 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) — không đổi | `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 0 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447) | `cargo test -p vis-cli --test diag_cmp` |
| Differential (bộ sinh mới) | **`tong=100000 loi_tinh=6230 chay=93770 lech=0 giay=163.4 toc_do=612/s seed=20261115`** | `VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff` |
| Fuzz parser 60 s | **2.462.470 ca, 56.028.401 token, 41.041 ca/s, 933.807 token/s, 60,0 s, vi phạm 0** (PASS) | `target/release/vis-parse-fuzz --seconds 60 --seed 20261115` |
| Cache harness | **520 tệp × 4 lần**; **520 ca so đủ 4 đường, 0 lệch**, **0 ca loại trừ**; **23,82 s** | `cargo test -p vis-cli --test cache_khong_che -- --nocapture` |
| Host suite | **54/54 ca xanh, 120 lần chạy, 0 lệch; VM 54/54** | `./bin/vis-host-suite` / `host_conformance` |
| 10k task async | **interp 316 ms, vm 168 ms**, hai đường cùng byte, không tràn | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| `VBC1` | **53 tag** (không bump ở vòng 15) | `Instr::tag` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới ở vòng 15) | `vis explain --list` (32 dòng) |
| Allowlist doc-examples | **6** mục | `grep -v '^#' /Users/mps/dvchd/ViFastOs/docs/viscript/doc-examples-allow.txt` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `fmt --check test/` | **exit 0**, **497 tệp** kiểm (**429** `.vis` + **68** `.vish`), 136 input cố ý bỏ; **8 cảnh báo** thụt lề `.vish` (không đỏ) | `vis fmt --check test` |
| `--indent-report test` | **8 tệp / 10 dòng lệch** (chuẩn 4 × độ sâu khối) — **không sửa** | `vis fmt --indent-report test` |
| Báo cáo | **15 báo cáo**, **108 hàng QUEUE** phủ đủ; `R15` đúng 11 khoá, `round=15`, `head=dc754a6b` | `python3 viscript/test/report/check_report.py --all` |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK; report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số trên do cột 3 sinh; không có số viết tay. Thứ tự đo: `cargo test --workspace` →
`vis test`/`--vm` → parity/`diag_cmp` → `async_10k` → host suite → targets → `fmt --check` → indent-report
→ differential → fuzz parser → cache harness → gate/report-check.

## 4. §19 — đối chiếu **VS2** sau 15 vòng: mức thật

| # | Hạng mục VS2 | Trạng thái | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối; 10k task **316/168 ms**, hai đường cùng byte; drain **100 task đúng thứ tự spawn** (ST-94) | **Chưa song song thật** (run-to-completion), **không GC**; `async` trên method/`cmd` còn `VS0201`; `async fn` trong `test` không hỗ trợ |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** — **`chan(0)` bị từ chối** | `chan[T](n)` với **n ≥ 1**, `send`/`recv`/`close`; **`chan(0)`** ⇒ `VS0505` ngay khi tạo (exit 1, **0,01 s**), **không** rendezvous; `recv` sau close `none`; `send` sau close `VS0504`; deadlock `VS0505`; drain `Moi` | **Chưa mô hình wait-set/rendezvous**; `chan(0)` exit 1 (**không 2** — xung đột parity/`.err`); chưa `select`/timeout |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | `NetHost` + TCP client/server **loopback** thật, `http.get`; **24 ca** net/http; net **không** nằm trong host suite | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự**, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** (runtime) — Landlock **CHỈ PROBE** | `cap[Dir]`/`cap[Net]` cưỡng chế biên host (`FsGuard`/`NetGuard`); **`cap[Proc]`** cưỡng chế `Ngoai::Runner` dưới `--strict-caps` (ST-93), **cả hai đường**; `--strict-caps` + `--grant`; 8+7 ca interp+VM | **Chưa siết mặc định**; **chưa cưỡng chế kernel**; Linux **chưa chạy thật**; `cap[Proc]` **không allowlist**; tiến trình con **không sandbox** |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | `vis lsp --stdio`, diagnostics UTF-16, module-aware; completion sau `.` theo kiểu tĩnh cho receiver đơn giản — field kind 5 + method kind 2, **50 → 2 mục** (ST-91); **39 ca** | **Hover field** chưa; `a.b.`/`(x).`/`f().`/generic/`list[P]` chưa; chưa bảng `std.str`; tệp phụ không publish; full sync |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local**, manifest `files[]`, lock SHA-256, `.vis/store` content-addressed, `use pkg.*`, `VS0701`/`VS0702`; **`entry` trong lock + `pkg remove`** (ST-90) | **Chưa ký số/HTTP/resolver dải**; chưa `pkg update`; lock cũ không tự nâng cấp; store không GC |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; `vis-cli`×wasm check xanh nhưng **không chạy được** | **Chưa runner** nền nào ngoài macOS; chưa build/link Windows; `vis-host-linux` chưa nối `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS, 120 lần, 0 lệch; VM 54/54** — đầy đủ **cả hai đường**, kể cả 13 ca chương trình ngoài; cache 520×4 0 lệch | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng) |

## 5. Lỗ hổng còn mở (nợ có chủ đích — R14 **67 mục** + nợ mới vòng 15)

**1..67** — giữ nguyên [`R14-viscript.md`](R14-viscript.md) §5, **trừ mục 60 đóng một phần**:

- **#60 “lệnh ngoài chưa qua `cap`”** — **đóng một phần** (ST-93): dưới `--strict-caps`, chương trình ngoài
  **bắt buộc** `cap[Proc]` ở **cả hai đường**; **mặc định** vẫn chưa siết (giữ tương thích) — phần còn lại
  chuyển thành nợ **68** (allowlist) và ghi rõ “chưa siết mặc định” ở §19 hàng 4.

Các mục còn lại (assert/runner test/LSP/package/Landlock/đa nền tảng/host) vẫn mở như R14.

**Nợ mới ST-93..ST-94 (đánh số tiếp):**

68. **`cap[Proc]` không allowlist** (ST-93): quyền chạy **mọi** tiến trình, không giới hạn danh sách/đường
    dẫn; chưa có `cap[Proc] @ "…"`/allowlist; cũng chưa áp cho tiến trình con do chương trình ngoài sinh ra.
69. **`.vish` `main` + pipeline: bỏ `main`** (ST-93): khi tệp có **cả** pipeline cấp tệp và `fn main`, HIR
    bỏ `main` để pipeline chạy; `main` chỉ khai `cap` — thay đổi ngữ nghĩa so trước, có thể ảnh hưởng tệp
    lạm dụng `main`; chỉ xảy ra khi thân `main` rỗng/`pass`.
70. **`pass` nhận diện theo tên** (ST-93b): thân `main` chỉ có `HStmt::Expr(Var("pass"))` mới được coi là
    rỗng; nếu `pass` thành câu lệnh thật/đổi lowering phải sửa; chưa có kiểm “khai báo kiểu trong thân”.
71. **Không rendezvous + `chan(0)` exit 1** (ST-94): scheduler một luồng **không** hỗ trợ capacity 0; đã
    từ chối runtime `VS0505` nhưng **exit 1**, không phải 2 (giữ parity **398+49**/`.err VS05`); chưa có
    wait-set để rendezvous thật.
72. **Drain không wait-set** (ST-94): join cuối chạy các task `Moi` theo hàng đợi, không có cơ chế chờ
    `recv`/`send` đúng nghĩa; thứ tự 100 task đã khoá nhưng phụ thuộc run-to-completion.
73. **`--grant` không áp cho `Proc`** (ST-93): `--grant tên=path` chỉ có nghĩa Dir/File/Net; `cap[Proc]`
    không có tham số đường dẫn nên không nhận grant.
74. **Tiến trình con không sandbox** (ST-93): `cap[Proc]` cho phép spawn, nhưng tiến trình con **không** bị
    Landlock/seccomp/namespace; mới chỉ kiểm ở biên host.

## 6. VD-111

**VD-111 — Vòng 15 đóng mô hình quyền + ngữ nghĩa kênh (ST-93..ST-95)**: (1) **`cap[Proc]`** dưới
`--strict-caps` — `Ngoai::Runner` kiểm **tại biên host**, dùng chung interp + VM, thiếu ⇒ `VS0604` exit 2;
có `cap[Proc]` ⇒ chạy **mọi** lệnh (**không allowlist** — quyết định); `ds_cap` đọc chữ ký `main` `.vish`;
(2) **`.vish` `fn main` chỉ khai `cap`** — thân có câu lệnh ⇒ `VS0201` tại câu lệnh đầu, không chạy; thân
rỗng/`pass` hợp lệ; (3) **kênh** — chọn **nhánh A**: scheduler một luồng **không thể** rendezvous nên
`chan[T](0)` **bị từ chối** ngay (`VS0505`, **0,01 s**, không treo), không giả vờ chạy; `recv` sau close
`none`, `send` sau close `VS0504`, deadlock `VS0505`, drain task `Moi` **100 task đúng thứ tự**. **Đính
chính**: `chan(0)` runtime **exit 1**, không phải 2 — static `VS0412` đã thử nhưng phá parity **398+49**
(49→50 lỗi biên dịch) và quy ước `.err VS05 ⇒ exit 1`; giữ runtime.

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **15 báo cáo**, **108 hàng QUEUE** phủ đủ;
  `R15-viscript.json` đúng **11 khoá**, `round=15`, `head=dc754a6b` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check
  trong cổng PASS; differential 100k **0 lệch**; fuzz parser 60 s **0 vi phạm**; cache harness **0 lệch**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã **32**
  (vòng 15 **không** thêm mã); không sửa `R1..R14`.
- **Đề xuất Agent 0**: (1) lật **ST-95 = DONE**; (2) nợ lớn nhất: **siết mặc định `cap`** + **allowlist
  `cap[Proc]`** (nợ 68) + **sandbox tiến trình con** (nợ 74); (3) **rendezvous/wait-set** cho kênh (nợ 71)
  chỉ khi scheduler đổi; (4) **Landlock thật** khi có runner Linux + duyệt crate/`unsafe`; (5) giữ **999
  test**, **447 sem**, **parity 398+49**, **54 host ×2**, **497 fmt**, **520×4 cache** làm chuẩn R16.

## 8. Kết luận trung thực — còn gì để gọi là “hoàn thiện”

Sau **15 vòng**, phần **đo được** tiếp tục nhất quán: **999 test**, **447 sem**, `--vm` **0 SKIP**, parity
**398/398 + 49**, differential **100k 0 lệch**, cache **520×4 0 lệch**, host **54/54 × 2**, fuzz parser **0
vi phạm**; **mô hình quyền** có thêm `cap[Proc]` dưới `--strict-caps` và **kênh** không còn giả vờ
rendezvous. Nhưng ViScript **chưa hoàn thiện**: (1) **Landlock thật** chưa — **chặn bởi môi trường**
(cần Linux + duyệt crate/`unsafe`); (2) **runner Windows/wasm/Linux** chưa có — 18/18 ô chỉ `cargo check`;
(3) net **chưa TLS** (cần crate), chỉ loopback/một luồng; (4) async **chưa song song thật**, kênh **chưa**
rendezvous/wait-set; (5) **quyền chưa siết mặc định**, `cap[Proc]` **không allowlist**, tiến trình con
**không sandbox**. Đây là **nợ có chủ đích** đã đánh số ở §5 (R14 **67** + mới **68..74**), không phải
tuyên bố “đã xong”.
