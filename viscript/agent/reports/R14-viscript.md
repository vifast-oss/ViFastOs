<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R14-viscript — báo cáo vòng 14 (đóng khoảng lệch cuối interp↔VM, `entry`/`pkg remove`, LSP sau `.` theo kiểu), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`f4627621`** (commit nền: `QUEUE@head` đủ **104 hàng**, **103 `DONE`**
+ ST-92 `READY` — báo cáo này). Số đo chạy trên **mã `a2eb53e5`** (commit cuối vòng 14; sau đó chỉ có
`QUEUE.md`). Bản máy đọc: [`R14-viscript.json`](R14-viscript.json) — **đúng 11 khoá** Khối E
(`project = "viscript"`, `round = 14`). Như tiền lệ R11–R13: `head` = commit **nền** của báo cáo (không thể
tự chứa hash của chính nó), nên `head` **không** phải `a2eb53e5`.

## 1. Tóm tắt (5 dòng)

1. **ST-89 đóng khoảng lệch cuối interp ↔ VM**: `vis-vm` nhận **cùng trait `Ngoai`** của interp (VM **không**
   gọi `std::process`; CLI truyền host như interp) ⇒ `--vm` **7 SKIP → 0**, `vis test --vm` **447 ok + 0 SKIP**,
   host suite **54/54 × 2 đường**, kể cả 13 ca chương trình ngoài.
2. **ST-90**: `vis.lock` lưu `entry` (đường dẫn tương đối); `pkg add --entry` override khi manifest thiếu;
   lock **cũ** thiếu `entry` ⇒ **fallback quy ước**, không cảnh báo; entry sai ⇒ **`VS0701` nêu tên tệp**.
   `pkg remove <tên>` xoá lock + `.vis/pkg` + blob `.vis/store` **chỉ khi không còn gói khác dùng**;
   **không** sửa `vis.toml`; tên lạ ⇒ **exit 2**.
3. **ST-91**: LSP completion sau `.` theo **kiểu tĩnh** — `p.` từ **50 mục fallback → 2 trường** (`x`, `ten`,
   kind 5), `impl Show for P` thêm `show` (kind 2); kiểu biết nhưng không có bảng (`str`) ⇒ **rỗng**; không
   rõ kiểu ⇒ **fallback 50**; tất định 5 lần.
4. **Số đo**: **978 test**, sem **447/22 nhóm**, `vis test` **447 ok** (`--vm` **447 ok + 0 SKIP**), parity
   **398/398 + 49 lỗi biên dịch**, differential **100.000 / 0 lệch** (161,3 s, seed 20261115), cache
   **520 tệp × 4 lần / 0 lệch**, host **54/54 × 2**, fuzz parser 60 s **2.517.572 ca / 0 vi phạm**, targets
   **18/18 `check`**, `fmt --check test/` **497 tệp**. ViScript **chưa hoàn thiện toàn bộ** — nợ §5, kết luận §8.
5. **Trung thực**: **“parity 405” của VD-110 là đếm trùng** — phân hoạch thật là **447 = 398 chạy được + 49
   lỗi biên dịch**; 7 ca host vốn **đã nằm trong 398** (trước khớp `VS0604`, nay khớp thật), nên R14 giữ
   **398/398**, không đếm 405. Không nới ngưỡng, không `#[ignore]`, không sửa golden/`.out`/`.err` để qua.

## 2. Bảng vòng 14

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-89 | `865342a8` | **VM có host `Ngoai`** (cùng trait `vis_interp::builtin::Ngoai`, VM không chạm OS; API `run_with_full_hosts_caps`, chữ ký cũ giữ làm wrapper): `--vm` **7 SKIP → 0**, `vis test --vm` **447 ok + 0 SKIP**; host **54/54 × 2 đường** (13 ca ngoài bỏ `vm:vs0604`); 5 ca `ngoai_vm`; **966 test** |
| ST-90 | `bdfd122b` | **`entry` trong lock** (`GoiKhoa.entry`, `--entry`, lock cũ fallback, entry sai `VS0701` nêu tệp) + **`pkg remove`** (lock + `.vis/pkg` + blob không còn dùng; không sửa `vis.toml`; tên lạ exit 2); **7 ca** `pkg.rs`; **973 test** |
| ST-91 | `a2eb53e5` | **LSP completion sau `.`** theo kiểu tĩnh: `p.` **50 → 2 mục** (field kind 5, detail `x: int`), `impl` thêm method kind 2, sentinel cho `p.` dở, fallback 50 khi không rõ kiểu, 5 ca tất định; **39 LSP**; **978 test** |
| ST-92 | (commit này) | báo cáo **R14**; `report-check --all` PASS; số liệu **chạy lại** trong §3 |

## 3. Số liệu chạy lại (mã `a2eb53e5`, đo 2026-11-14, macOS aarch64, `debug` trừ khi ghi khác)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **978** = 21 diag + 96 syntax + **27 fmt** + 154 types + 26 interp + 62 bytecode + 9 vm + **583 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) — không đổi | `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 0 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447); 7 ca host nay khớp **thật**, không còn `VS0604` | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447); host cần chương trình ngoài **0/0** | `cargo test -p vis-cli --test diag_cmp` |
| Differential (bộ sinh mới) | **`tong=100000 loi_tinh=6230 chay=93770 lech=0 giay=161.3 toc_do=620/s seed=20261115`** | `VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff` |
| Fuzz parser 60 s | **2.517.572 ca, 57.280.584 token, 41.960 ca/s, 954.676 token/s, 60,0 s, vi phạm 0** (PASS) | `target/release/vis-parse-fuzz --seconds 60 --seed 20261115` |
| Cache harness | **520 tệp × 4 lần**; **520 ca so đủ 4 đường, 0 lệch**; **0 ca loại trừ** (ST-89 đóng 7 ca `VS0604`); **24,17 s** | `cargo test -p vis-cli --test cache_khong_che -- --nocapture` |
| Host suite | **54/54 ca xanh, 120 lần chạy, 0 lệch; VM 54/54** (cả 13 ca chương trình ngoài) | `./bin/vis-host-suite` / `host_conformance` |
| 10k task async | **interp 383 ms, vm 168 ms**, hai đường cùng byte, không tràn | `cargo test -p vis-cli --test async_10k -- --nocapture` |
| `VBC1` | **53 tag** (không bump ở vòng 14) | `Instr::tag` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới ở vòng 14) | `vis explain --list` (32 dòng) |
| Allowlist doc-examples | **6** mục | `grep -v '^#' /Users/mps/dvchd/ViFastOs/docs/viscript/doc-examples-allow.txt` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `fmt --check test/` | **exit 0**, **497 tệp** kiểm (**429** `.vis` + **68** `.vish`), 136 input cố ý bỏ; **8 cảnh báo** thụt lề `.vish` (không đỏ) | `vis fmt --check test` |
| `--indent-report test` | **8 tệp / 10 dòng lệch** (chuẩn 4 × độ sâu khối) — **không sửa** | `vis fmt --indent-report test` |
| Báo cáo | **14 báo cáo**, **104 hàng QUEUE** phủ đủ; `R14` đúng 11 khoá, `round=14`, `head=f4627621` | `python3 viscript/test/report/check_report.py --all` |
| Cổng | `gate --fast` **PASS** (không retry); `fmt`/`clippy -D warnings`/R1..R4 OK; report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số trên do cột 3 sinh; không có số viết tay. Thứ tự đo: `cargo test --workspace` →
`vis test`/`--vm` → parity/`diag_cmp` → `async_10k` → host suite → targets → `fmt --check` → indent-report
→ differential → fuzz parser → cache harness → gate/report-check.

## 4. §19 — đối chiếu **VS2** sau 14 vòng: mức thật

| # | Hạng mục VS2 | Trạng thái | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối; 10k task **383/168 ms** (interp/VM), hai đường cùng byte | **Chưa song song thật** (run-to-completion), **không GC**; `async` trên method/`cmd` còn `VS0201`; `async fn` trong thân `test` không hỗ trợ |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** | `chan[T](n)`/`send`/`recv`/`close`; deadlock `VS0505` (không treo); fuzz R13 20k/100k sinh cặp cân bằng + `close` + spawn sender, 0 lệch; 520 tệp × 4 lần gồm 15 ca kênh, 0 lệch | **Chưa `select`/timeout**; `cap=0` chỉ deadlock có chẩn đoán; `Chan` không qua biên host |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | `NetHost` + TCP client/server **loopback** thật, `http.get`; **24 ca** net/http; net **không** nằm trong host suite | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự**, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** (runtime) — Landlock **CHỈ PROBE** | `cap[...]` là kiểu thật; cưỡng chế **ở biên host** (`FsGuard`/`NetGuard`), vi phạm ⇒ **trap `VS0604`** exit 2; `--strict-caps`; 8+7 ca interp+VM | **Chưa siết mặc định**; **chưa cưỡng chế bằng kernel** ở **bất kỳ** nền nào; Linux **chưa chạy thật**; `--sandbox-check` **chỉ đọc** `/sys/.../lsm` |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | `vis lsp --stdio`, diagnostics UTF-16, module-aware; **completion sau `.` theo kiểu tĩnh** cho receiver **đơn giản** — field `type` (kind 5) + method `impl` (kind 2), 50 → 2 mục (ST-91); **39 ca** | **Hover field** chưa; `a.b.`/`(x).`/`f().`/generic/`list[P]` chưa suy; chưa bảng `std.str`; tệp phụ không publish; full sync |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local**, manifest `files[]`, lock SHA-256 tất định, `.vis/store` content-addressed, `use pkg.*`, `VS0701`/`VS0702`; **`entry` trong lock + `pkg remove`** (ST-90) | **Chưa ký số**, **chưa registry HTTP**, **chưa resolver dải** (ghim chính xác), chưa `pkg update`, lock cũ không tự nâng cấp, store không GC |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; `vis-cli`×wasm check xanh nhưng **không chạy được** | **Chưa runner** nền nào ngoài macOS; chưa đo build/link Windows; `vis-host-linux` chưa nối `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS, 120 lần, 0 lệch; VM 54/54** — **đầy đủ cả hai đường**, kể cả **13 ca chương trình ngoài** (ST-89 đóng khoảng lệch cuối); cache 520×4 0 lệch | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng) |

## 5. Lỗ hổng còn mở (nợ có chủ đích — R13 **58 mục** + nợ mới vòng 14)

**1..58** — giữ nguyên [`R13-viscript.md`](R13-viscript.md) §5, **trừ hai mục đã đóng**:

- **#49 “completion chưa theo ngữ cảnh sau `.`”** — **đóng một phần** (ST-91): đã theo kiểu cho receiver đơn
  giản; phần chưa làm chuyển thành nợ **65** dưới đây.
- **#56 “7 ca `.vish` khác hai đường có chủ đích”** — **đóng hoàn toàn** (ST-89): VM có host `Ngoai`,
  `--vm` 0 SKIP, cả 7 ca khớp thật.

Các mục còn lại (assert/runner test/LSP module/package/Landlock/đa nền tảng/host) vẫn mở như R13.

**Nợ mới ST-89..ST-91 (đánh số tiếp):**

59. **`--vm` vẫn một luồng** (ST-89): host `Ngoai` không đổi scheduler; lệnh ngoài chạy tuần tự, không có
    song song/async cho process.
60. **Lệnh ngoài chưa qua `cap`** (ST-89/ST-90): `exec.*`/`Ngoai` chạy **không** cần khai quyền (như interp
    hiện tại); chưa có `cap[Exec]` hay kiểm `--strict-caps` cho chương trình ngoài.
61. **`vis test` dùng `--no-cache`** (ST-89): mọi ca test chạy `--vm --no-cache` để tránh cache ảnh hưởng;
    chưa có chế độ test cache ấm.
62. **Lock cũ không tự nâng cấp** (ST-90): lock thiếu `entry` chỉ fallback; chỉ khi `add`/`remove` ghi lại
    mới có `entry`; không có `pkg migrate`/chuẩn hoá lock.
63. **`entry` chưa kiểm đuôi** (ST-90): chỉ kiểm đường dẫn tương đối + có trong `files`; chưa bắt buộc
    `.vis`/tồn tại đúng kiểu tệp.
64. **`remove` không dọn dependency mồ côi** (ST-90): xoá `http` để lại `util` trong lock/store (đúng
    “chỉ xoá tên được gọi”); chưa có `pkg remove --prune`/GC tham chiếu.
65. **LSP chỉ receiver đơn giản** (ST-91): `a.b.`, `(x).`, `f().`, generic/`list[P]`/`Opt[P]`, `impl` cho
    alias/enum chưa mở thành viên; chưa có bảng phương thức `std.str` (trả rỗng).
66. **`entry`/thành viên chưa cache** (ST-91): mỗi completion sau `.` parse/lower lại với sentinel `__vis`
    (~1 lần phân tích); chưa cache theo phiên/tệp.
67. **`self` in gọn không kiểu** (ST-91): chữ ký method hiển thị `fn show(self) -> str` (bỏ `: {error}`);
    chưa suy kiểu `self` thật cho hover/detail.

## 6. VD-110

**VD-110 — Vòng 14 đóng khoảng lệch cuối (ST-89..ST-92)**: (1) **`Ngoai` cho VM** — VM nhận **cùng trait**
của interp, không chạm OS; CLI truyền host; `--vm` **7 SKIP → 0**, host **54/54 × 2 đường**; **đính chính**:
parity thật là **398/398 + 49 lỗi biên dịch**, không phải 405 (7 ca host đã nằm trong 398, đếm thêm là
**đếm trùng**). (2) **`vis pkg`**: lock lưu **`entry`** (+`--entry`, lock cũ fallback, entry sai `VS0701` nêu
tệp) + **`pkg remove`** (lock + `.vis/pkg` + blob không còn dùng; **không** sửa `vis.toml`; tên lạ exit 2).
(3) **LSP completion sau `.`** theo kiểu tĩnh: field `type` kind 5 + method `impl` kind 2, sentinel cho cú
pháp dở, fallback khi không rõ kiểu, **không phá 34 ca LSP** (nay 39). (4) Không thêm hạng mục VS2 mới;
**không** nới ngưỡng.

## 7. Ghi chú máy kiểm + đề xuất

- `python3 test/report/check_report.py --all`: **PASS** — **14 báo cáo**, **104 hàng QUEUE** phủ đủ;
  `R14-viscript.json` đúng **11 khoá**, `round=14`, `head=f4627621` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check
  trong cổng PASS; differential 100k **0 lệch**; fuzz parser 60 s **0 vi phạm**; cache harness **0 lệch**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53** tag; sổ mã **32**
  (vòng 14 **không** thêm mã); không sửa `R1..R13`.
- **Đề xuất Agent 0**: (1) lật **ST-92 = DONE**; (2) nợ lớn nhất còn lại là **siết mặc định `cap`** và
  **`cap` cho lệnh ngoài** (nợ 60); (3) **hover field + receiver phức tạp** cho LSP (nợ 65); (4) **Landlock
  thật** chỉ khi có runner Linux + duyệt crate/`unsafe` (VD-107); (5) giữ **978 test**, **447 sem**, **parity
  398+49**, **54 host ×2**, **497 fmt**, **520×4 cache** làm chuẩn R15.

## 8. Kết luận trung thực — còn gì để gọi là “hoàn thiện”

Sau **14 vòng**, phần **đo được** đã nhất quán và đầy đủ hơn: **978 test**, **447 sem**, `--vm` **0 SKIP**,
parity **398/398 + 49**, differential **100k 0 lệch**, cache **520×4 0 lệch**, host **54/54 × 2 đường**,
fuzz parser **0 vi phạm** — **khoảng lệch interp↔VM cuối đã đóng**. Nhưng ViScript **chưa hoàn thiện**:
(1) **Landlock thật** chưa có — **chặn bởi môi trường** (cần Linux + duyệt crate/`unsafe`); (2) **runner
Windows/wasm/Linux** chưa có — 18/18 ô chỉ là **`cargo check`**; (3) net **chưa TLS** (cần crate) và chỉ
loopback/một luồng; (4) async **chưa song song thật**; (5) **quyền chưa siết mặc định**, lệnh ngoài chưa qua
`cap`. Đây là **nợ có chủ đích** đã đánh số ở §5 (còn **65 mục mở**: 56 mục R13 + 9 mục mới), không phải
tuyên bố “đã xong”.
