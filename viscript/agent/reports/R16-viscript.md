<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R16-viscript — báo cáo vòng 16 (nhất quán sổ mã, `--grant` Proc allowlist, soak), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`a9892676`** (commit nền: `QUEUE@head` đủ **112 hàng**, **111
`DONE`** + ST-99 `READY` — báo cáo này). Số đo chạy trên mã **`3840d463`** (commit cuối ST-98; sau đó chỉ
`QUEUE.md` + báo cáo). Bản máy đọc: [`R16-viscript.json`](R16-viscript.json) — **đúng 11 khoá** Khối E
(`project = "viscript"`, `round = 16`). Như tiền lệ R11–R15: `head` = commit **nền** của báo cáo (không thể
tự chứa hash của chính nó).

**Bàn giao**: [`../ROADMAP.md`](../ROADMAP.md) — **55 dòng**, 8 hạng mục VS2 × mức thật/bằng chứng/điều
kiện đóng 100% + 3 mục “cách chạy tiếp”.

## 1. Tóm tắt (5 dòng)

1. **ST-96 — sổ mã ↔ mã dùng**: quét literal `"VS0ddd"` trong `crates/*/src` được **A=33**, `all_codes`
   **B=32**; **A\B = [`VS0001`]** chỉ là ví dụ định dạng trong unit test (không phải mã chẩn đoán) ⇒ bỏ
   literal, **không** thêm/đổi mã; `B\A = []` ⇒ `DU_PHONG` rỗng; test khoá **A⊆B** + **B\A==DU_PHONG**.
2. **ST-97 — `--grant` cho `cap[Proc]`**: lặp `--grant p=<lệnh>` ⇒ **allowlist**; khớp **basename HOẶC
   canonicalize**; ngoài allowlist ⇒ **`VS0604` exit 2**, không chạy; grant lệnh cho `cap[Dir]` ⇒ lỗi kiểu;
   **không grant ⇒ không giới hạn** (tương thích).
3. **ST-98 — soak 8 vòng tuần tự**: **8/8 OK, 818 s** (gate 224+211, diff 160+158, host 9+7, cache 24+25);
   **0 flaky sản phẩm**; lần đầu dừng `cache-1 exit 127` do **PATH** (`cargo: command not found`) — ghi
   **T-INFRA**, sửa script, **chạy lại toàn bộ**, không retry để che đỏ.
4. **Số đo**: **1010 test**, sem **447/22**, `vis test` **447 ok** (`--vm` **447 ok + 0 SKIP**), host
   **54/54 ×2**, parity **398/398 + 49**, differential **100.000 / 0 lệch** (164,9 s), cache **520×4 /
   0 lệch** (24,28 s), fuzz parser **2.453.833 ca / 0 vi phạm**, targets **18/18 `check`**, fmt **497 tệp**.
5. **Trung thực**: `chan(0)` vẫn **bị từ chối** (không rendezvous, exit 1); net **loopback không TLS**;
   **chưa Landlock thật**, **chưa siết mặc định caps**, **chưa runner** Windows/Linux/wasm; soak **818 s**
   chưa đủ 30 phút thực nếu chỉ 8 vòng; ROADMAP ghi đúng các mức này.

## 2. Bảng vòng 16

| Task | Commit | Kết quả (1 dòng, số đo tại lúc đóng) |
|---|---|---|
| ST-96 | `d442721b` | **Sổ mã ↔ mã dùng**: A\B=`VS0001` (literal test) ⇒ bỏ literal; B\A=[] ⇒ `DU_PHONG` rỗng; 4 ca (A⊆B, B\A==DU_PHONG, 3 ngôn ngữ, DU_PHONG) ; **1003 test** |
| ST-97 | `0397c5c6` | **`--grant` Proc allowlist**: lặp grant, khớp basename/canonical, `VS0604` exit 2; grant lệnh cho `cap[Dir]` lỗi kiểu; 6 ca (a–f); **1009 test** |
| ST-98 | `3840d463` | **Soak 8 vòng**: 8/8 OK **818 s**, 0 flaky; `bin/vis-soak` 61 dòng + test; **1010 test**; T-INFRA PATH ghi trung thực, chạy lại hợp lệ |
| ST-99 | (commit này) | **R16 + ROADMAP** 55 dòng; `report-check --all` PASS; số liệu chạy lại §3 |

## 3. Số liệu chạy lại (mã `3840d463`, đo 2026-11-14, macOS aarch64)

| Phép đo | Kết quả | Lệnh |
|---|---|---|
| `cargo test --workspace` | **1010** = 21 diag + 96 syntax + **27 fmt** + 154 types + 26 interp + 62 bytecode + 9 vm + **615 cli** | `cd viscript && cargo test --workspace` |
| Ca ngữ nghĩa | **447 / 22 nhóm** (397 `.vis` + 50 `.vish`; **83** `.err`) — không đổi | `cargo test -p vis-cli --test sem` (khoá 447/22) |
| `vis test` / `--vm` | **447 ok**, 0 `not ok` / **447 ok, 0 SKIP, 0 đỏ** | `vis test test/sem` / `vis test --vm test/sem` |
| Parity interp ↔ VM | **398/398** ca chạy được khớp byte + **49** ca lỗi biên dịch (tổng 447) | `cargo test -p vis-vm --test vs_interp` |
| `diag_cmp` ba tầng | **364 xanh / 49 lỗi biên dịch / 34 lỗi lúc chạy** (=447) | `cargo test -p vis-cli --test diag_cmp` |
| Differential | **`tong=100000 loi_tinh=6230 chay=93770 lech=0 giay=164.9 toc_do=606/s seed=20261115`** | `VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff` |
| Fuzz parser 60 s | **2.453.833 ca, 55.832.883 token, 40.897 ca/s, 930.548 token/s, 60,0 s, vi phạm 0** (PASS) | `target/release/vis-parse-fuzz --seconds 60 --seed 20261115` |
| Cache harness | **520 tệp × 4 lần**; **520 ca so đủ 4 đường, 0 lệch**, **0 ca loại trừ**; **24,28 s** | `cargo test -p vis-cli --test cache_khong_che -- --nocapture` |
| Host suite | **54/54 ca xanh, 120 lần chạy, 0 lệch; VM 54/54** (ST-98: host-1 9 s, host-2 7 s) | `./bin/vis-host-suite` / `host_conformance` |
| **Soak** | **8/8 vòng OK, 818 s**, 0 flaky; gate 224+211, diff 160+158, host 9+7, cache 24+25; load 7,48→4,31 | [`st98-soak.log`](../JOURNAL/st98-soak.log) / `bin/vis-soak` |
| `VBC1` | **53 tag** (không bump ở vòng 16) | `Instr::tag` trong `crates/vis-bytecode/src/format.rs` |
| Sổ mã | **7 sổ / 32 mã** (không mã mới); A⊆B, B\A=[] | `vis explain --list`; test `so_ma` |
| Allowlist doc-examples | **6** mục | `grep -v '^#' /Users/mps/dvchd/ViFastOs/docs/viscript/doc-examples-allow.txt` |
| Ma trận target | **18/18 ô `PASS`** — **chỉ `cargo check` biên dịch chéo**, **chưa chạy** | `./bin/vis-targets` |
| `fmt --check test/` | **exit 0**, **497 tệp** kiểm (**429** `.vis` + **68** `.vish`), 136 input bố trí bỏ; **8 cảnh báo** thụt lề `.vish` (không đỏ) | `vis fmt --check test` |
| `--indent-report test` | **8 tệp / 10 dòng lệch** (chuẩn 4 × độ sâu khối) — **không sửa** | `vis fmt --indent-report test` |
| Báo cáo | **16 báo cáo**, **112 hàng QUEUE** phủ đủ; `R16` đúng 11 khoá, `round=16`, `head=a9892676` | `python3 viscript/test/report/check_report.py --all` |
| Cổng | `gate --fast` **PASS** (không retry); fmt/clippy/R1..R4 OK; report-check PASS | `./vis-dev gate --fast` |

**Tái lập**: mọi số do cột 3 sinh; không số viết tay. Thứ tự: `cargo test --workspace` → `vis test`/`--vm`
→ parity/`diag_cmp` → host → targets → `fmt --check` → indent → cache → differential → fuzz → gate. Soak
là kết quả ST-98 (`818 s`), không chạy lại trong task báo cáo (tránh chồng tải).

## 4. §19 — đối chiếu **VS2** sau 16 vòng: mức thật

| # | Hạng mục VS2 | Trạng thái | Mức thật (đo được) | Còn thiếu |
|---|---|---|---|---|
| 1 | **Async có cấu trúc** | **CÓ MẶT MỘT PHẦN** | `async fn`/`await`/`spawn`, scheduler **một luồng tất định**, join cuối; 10k task **316/168 ms**; drain **100 task đúng thứ tự spawn** | **Chưa song song thật**, **không GC**; `async` trên method/`cmd` còn `VS0201`; `async fn` trong `test` chưa |
| 2 | **Kênh** | **CÓ MẶT MỘT PHẦN** — **`chan(0)` bị từ chối** | `chan[T](n≥1)` send/recv/close; `chan(0)` ⇒ **`VS0505` ngay khi tạo** (exit 1, **0,01 s**), **không rendezvous**; `recv` sau close `none`; `send` sau close `VS0504`; deadlock `VS0505`; drain `Moi` 100 task | **Chưa wait-set/rendezvous**; `chan(0)` exit 1 (**không 2** — xung đột parity/`.err`); chưa `select`/timeout |
| 3 | **Net/HTTP** | **CÓ MẶT MỘT PHẦN** | TCP client/server **loopback** thật, `http.get`; **24 ca** net/http; net **không** trong host suite | **Chưa TLS/redirect/keep-alive/chunked**, **một luồng**, phục vụ **tuần tự**, cap body 8 MiB |
| 4 | **Quyền + Landlock thật** | **CÓ MẶT MỘT PHẦN** — Landlock **CHỈ PROBE** | `cap[Dir]/File/Net/Proc` cưỡng chế biên host (interp+VM); **`cap[Proc]` allowlist** qua `--grant` (ST-97: lặp grant, basename/canonical, `VS0604` exit 2); grant lệnh cho `cap[Dir]` lỗi kiểu; 8+7+6 ca | **Chưa siết mặc định**; **chưa cưỡng chế kernel**; Linux **chưa chạy thật**; tiến trình con **không sandbox**; chưa giới hạn args |
| 5 | **LSP** | **CÓ MẶT MỘT PHẦN** | diagnostics **module-aware** = `vis check`, UTF-16; completion sau `.` (field kind 5 + method kind 2; 50→2 mục); **39 ca** | **Hover field** chưa; `a.b.`/`(x).`/`f().`/generic/`list[P]` chưa; chưa bảng `std.str`; tệp phụ không publish; full sync |
| 6 | **Gói + registry** | **CÓ MẶT MỘT PHẦN** | Registry **local**, manifest `files[]`, lock SHA-256, `.vis/store` content-addressed, `use pkg.*`, `VS0701/0702`; **`entry` + `pkg remove`** | **Chưa ký số/HTTP/resolver dải**; chưa `pkg update`; lock cũ không tự nâng; store không GC |
| 7 | **windows/wasm** | **CHỈ CHECK** | Ma trận **18/18 ô PASS** bằng `cargo check --target`; `vis-host-wasm` khung; `vis-cli`×wasm check xanh nhưng **không chạy được** | **Chưa runner** nền nào ngoài macOS; chưa build/link Windows; `vis-host-linux` chưa nối `vis-cli` |
| 8 | **Hợp chuẩn Host 4 nền** | **CÓ MẶT MỘT PHẦN** (macOS) | **54 ca chạy thật trên macOS, 120 lần, 0 lệch; VM 54/54**; cache 520×4 0 lệch; soak host 9+7 s | **Windows/Linux/wasm: chưa đo**; net **không** nằm trong suite (24 ca riêng) |

## 5. Lỗ hổng còn mở (nợ có chủ đích — R14 **67** + R15 **68..74** + mới **75..77** = **77 mục**)

**1..74** — giữ nguyên [`R15-viscript.md`](R15-viscript.md) §5 (R14 67 + R15 68..74; #60 đóng **một
phần**). Ba mục mới vòng 16:

75. **Quét sổ mã chỉ `src`** (ST-96): test `so_ma` chỉ quét `crates/*/src/**/*.rs`, **không** test/script/
    `build.rs`; vì vậy `VS0000`, `VS0605`, `VS0801`, `VS0999` trong test/format **không** bị coi là mã
    chẩn đoán; `DU_PHONG` **rỗng** (chưa có mã dự phòng); chưa đối chiếu mã trong **tài liệu/spec**.
76. **Allowlist `cap[Proc]` còn rộng** (ST-97): khớp **basename** nên binary **cùng tên ở đường dẫn khác**
    vẫn qua; **không** giới hạn **đối số**; không hỗ trợ dạng `:`; grant áp **cả khi không** `--strict-caps`;
    không grant ⇒ unrestricted; heuristic “tệp thực thi cho `cap[Dir]`” có thể từ chối grant thư mục trỏ
    tệp-exec hiếm; non-Unix bỏ heuristic.
77. **Soak chưa đủ 30 phút + phụ thuộc PATH** (ST-98): 8 vòng chỉ **818 s** (đúng danh sách đề, chưa đủ
    30 phút thực); script giả định `cargo` ở `~/.cargo/bin` (đã thêm PATH nhưng vẫn giả định vị trí);
    test chỉ kiểm tĩnh script, **không** đo thời gian; chưa soak Linux/Windows; lần đầu `cache-1` exit
    127 là **T-INFRA** đã ghi (`st98-soak-attempt1-infra.log`), không tính vào 8 vòng.

## 6. VD-112

**VD-112 — Vòng 16 đóng nhất quán sổ mã, allowlist Proc, soak 8 vòng (ST-96..ST-99)**: (1) **sổ mã ↔
mã dùng** — A (literal `VS0ddd` trong `src`) = **33**, B (`all_codes`) = **32**; **A\B=`VS0001`** chỉ là
ví dụ định dạng, bỏ literal; **B\A=[]** ⇒ `DU_PHONG` rỗng, test `A⊆B`/`B\A==DU_PHONG`; **không** thêm mã;
(2) **`--grant` Proc** — lặp `--grant p=<lệnh>` = allowlist, khớp **basename HOẶC canonical**, ngoài ⇒
`VS0604` exit 2 tại biên host (interp+VM); grant lệnh cho `cap[Dir]` ⇒ lỗi kiểu; **không grant** ⇒ không
giới hạn; (3) **soak** — 8 vòng tuần tự **8/8 OK 818 s**, 0 flaky sản phẩm, T-INFRA PATH ghi trung thực và
chạy lại toàn bộ; (4) **R16 + ROADMAP 55 dòng**. **Đính chính**: soak **818 s**, không phải 30 phút; net
vẫn **không TLS**; `chan(0)` vẫn **bị từ chối**; quyền **chưa** mặc định/Landlock.

## 7. Bàn giao

Xem [`../ROADMAP.md`](../ROADMAP.md) — **55 dòng** (≤120): bảng 8 hạng mục VS2 × **mức thật · bằng chứng ·
điều kiện đóng 100%**; **B1** lệnh cổng/soak; **B2** 5 việc trước (Landlock thật + runner Linux, runner
wasm, TLS, siết mặc định caps, async song song); **B3** nơi đọc (`QUEUE`, `DECISIONS` VD-01..VD-112,
`reports/R1..R16`, `HOST-BAO-DAM.md`, `STATE.md`). Báo cáo này + ROADMAP là gói bàn giao vòng 16.

## 8. Ghi chú máy kiểm + kết luận

- `python3 test/report/check_report.py --all`: **PASS** — **16 báo cáo**, **112 hàng QUEUE** phủ đủ;
  `R16-viscript.json` đúng **11 khoá**, `round=16`, `head=a9892676` (tồn tại + là tổ tiên HEAD).
- Cổng: `./vis-dev gate --fast` **PASS**, không retry; `fmt`/`clippy -D warnings`/R1..R4 OK; report-check
  trong cổng PASS; differential 100k **0 lệch**; fuzz parser 60 s **0 vi phạm**; cache **0 lệch**.
- Không thêm `#[ignore]`, không nới ngưỡng, không sửa golden/allowlist; `VBC1` giữ **53**; **32 mã**;
  không sửa `R1..R15`; không sửa `docs/`, `v2/`, `spec/`, QUEUE/DECISIONS.
- **Kết luận trung thực**: sau **16 vòng**, phần đo được vẫn **1010 test**, **447 sem**, `--vm` **0 SKIP**,
  parity **398+49**, differential **0 lệch**, cache **520×4 0 lệch**, host **54/54 ×2**, soak **8/8**,
  fuzz **0 vi phạm**; sổ mã **nhất quán** (A⊆B, B\A rỗng) và `cap[Proc]` có **allowlist**. Nhưng ViScript
  **chưa hoàn thiện**: (1) **Landlock thật** chưa — **chặn bởi môi trường** + duyệt crate/`unsafe`;
  (2) **runner Windows/Linux/wasm** chưa — 18/18 chỉ `cargo check`; (3) net **chưa TLS**; (4) async **chưa
  song song thật**, kênh **chưa rendezvous**; (5) quyền **chưa mặc định**, `cap[Proc]` còn rộng, con
  **không sandbox**; (6) soak **818 s** chưa đủ 30 phút thực. Đây là **nợ có chủ đích** đánh số 1..**77**.
