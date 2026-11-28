<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# BÀN GIAO DÒNG ViScript → **AGENT 0**

**Ngày**: 2026-11-16 (sáng thứ Hai) · **Người bàn giao**: lead (điều phối đợt chạy liên tục) · **Người nhận**: **Agent 0**
**Tài liệu này tự chứa**: người nhận **không** cần ngữ cảnh trước đó. Mọi con số đều kèm **lệnh tái lập**.

> **Một câu tóm tắt.** Dòng ViScript đã chạy **22 vòng** liên tục (ST-01 → ST-118): **134/134 hàng QUEUE DONE**, **22 báo cáo R1..R22**,
> code xanh và **đo được** trên máy macOS. Nhưng **VS2 CHƯA ĐÓNG**: còn **9 điều kiện**, **104 nợ** (đã liệt kê nhóm), và **12 quyết
> định chỉ Agent 0 mới được phép ra** — vì chúng chạm luật cấm (`unsafe`, crate ngoài allowlist, nới ngưỡng, đổi ngữ nghĩa) hoặc
> chạm tài liệu **chỉ Agent 0/1 được sửa** (`docs/`). Việc của bạn nằm ở **§7–§9**.

---

## 0. Cách đọc (5 phút)

| Mục | Nội dung | Bạn cần |
|---|---|---|
| §1 | Dự án là gì, hai dòng sản phẩm, vai Agent 0 / Agent 2 | đọc |
| §2 | Luật bắt buộc (trích `AGENTS.md`, QD) | **thuộc** |
| §3 | Hiện trạng đo được (HEAD, số test, worktree) | đọc |
| §4 | 22 vòng đã làm gì | tra khi cần |
| §5 | 8 hạng mục VS2 × **mức thật** | **thuộc** |
| §6 | Lệnh tự kiểm chứng | chạy 1 lần |
| **§7** | **12 quyết định cần bạn ký** (Q1..Q12) | **LÀM** |
| **§8** | **8 việc quản trị chỉ bạn làm được** (G1..G8) | **LÀM** |
| **§9** | 104 nợ — nhóm + top 10 ưu tiên | chọn |
| §10 | Rủi ro / điểm cần review độc lập | đọc |
| §11 | Quy trình vận hành lại (cadence + brief mẫu) | đọc |
| §12 | **8 lần chỉ đạo của lead SAI, agent đo lại ĐÚNG** | **thuộc** (tránh lặp) |
| §13 | Nơi đọc + phụ lục (hash, VD-100..118) | tra |

---

## 1. Bối cảnh dự án

**ViFastOs** là một hệ điều hành viết bằng Rust, trong **một** repo git. Repo có **hai dòng sản phẩm độc lập**:

| Dòng | Thư mục | Task ID | Ai điều phối | Trạng thái |
|---|---|---|---|---|
| OS (kernel, sim, U-mode, driver) | `crates/`, `v2/`, `spec/` | `KT-*` | Agent 0 khác / Agent 2 dòng OS | đang chạy song song, commit xen liên tục |
| **ViScript** (ngôn ngữ + công cụ) | **`viscript/`** | **`ST-*`** | **bạn (Agent 0)** | **đợt này: 22 vòng, 134 hàng DONE** |

**ViScript là gì** (để bạn hình dung phạm vi): một ngôn ngữ lập trình hướng shell, gồm

- **Front end**: lexer theo `docs/viscript/SPEC-lex.md` → parser/CST **lossless** → giải tên + kiểm kiểu → HIR.
  Hai chế độ nguồn: `.vis` (script: `fn`, `type`, `enum`, `trait`, `impl`, `derives`, `async fn`, `chan[T]`, `cap[...]`) và
  `.vish` (chế độ lệnh: pipeline `ls | where … | take 3`, `cmd` khai báo, `type`/`let x: T`, `to json`).
- **Ba đường chạy**: interpreter (`vis run`), VM bytecode `VBC1` (`vis run --vm`), và **bộ kiểm chứng vi sai** giữa hai đường.
- **Công cụ**: `vis check` (TAP + `--json`), `vis test` (TAP/`--json`/`--timeout`), `vis fmt` (`--check`/`--write`/`--indent-report`),
  `vis cache`, `vis explain` (vi/en/ja, `--list`), `vis pkg` (add/install/remove/update/list, registry **cục bộ**),
  `vis lsp` (`--stdio`, diagnostics/completion/hover), `vis run --dry/--sandbox-check/--strict-caps/--grant`.
- **Runtime**: `std.*` 9 module / **52 hàm**; sổ lỗi **7 khối / 32 mã** (`VS0101`..`VS0702`); `VBC1` **53 tag**;
  quyền `cap[Dir|File|Net|Proc]` cưỡng chế ở biên host; async/kênh; HTTP client loopback.

**Ai ghi tệp nào** (luật cứng, `AGENTS.md` §3):

| Tệp | Người ghi | Ghi chú |
|---|---|---|
| `viscript/agent/QUEUE.md`, `DECISIONS.md` | **chỉ Agent 0** | Agent 2 không sửa; muốn đổi thì BLOCKED + đề xuất |
| `AGENTS.md` (root), `viscript/AGENTS.md` | **chỉ người điều phối / Agent 0** | |
| `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md` | **chỉ Agent 0 / Agent 1** (QD) | `docs/QUYET-DINH-sau-review.md` thì **cấm sửa tuyệt đối** |
| `viscript/crates/**`, `viscript/test/**`, `viscript/bin/**` | Agent 2 | code + fixture |
| `viscript/agent/STATE.md`, `JOURNAL/`, `reports/` | Agent 2 | báo cáo, log đỏ, trạng thái |

> Hai QUEUE **độc lập**: task OS `BLOCKED` **không** chặn dòng ViScript và ngược lại.

---

## 2. Luật bắt buộc (phải thuộc; chi tiết `AGENTS.md`)

**Thứ tự nguồn sự thật** (khi hai nguồn nói khác nhau, nguồn đứng trước thắng):
1. **Code + harness** (thứ chạy được, đo được: `crates/`, `viscript/`, log cổng, TAP)
2. `spec/` + `docs/viscript/SPEC-lex.md`
3. `viscript/agent/DECISIONS.md` (VD-01..**VD-118**)
4. Tài liệu thiết kế (`docs/viscript/ViScript-thiet-ke.md`)

**Cấm tuyệt đối** (không có ngoại lệ "tạm thời"):
1. Sửa `spec/` trừ task được ghi rõ trong QUEUE.
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. Thêm `#[ignore]`, `#[should_panic]` để né, tắt test, bỏ case khỏi danh sách chạy.
4. **Nới ngưỡng** (thời gian, seed, số ca, giới hạn IO/TCB/object).
5. **Retry để qua cổng**. Flake ⇒ ghi `T-INFRA` + log; chạy lại **tối đa một lần** chỉ để phân loại, **không** để lấy xanh.
6. Sửa `docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/viscript/*` (trừ Agent 0/1 theo QD).
7. Sửa bất kỳ tệp nào trong `docs/v1/`.
8. Thêm `unsafe` ngoài danh sách crate đóng (**hiện danh sách rỗng**).
9. Thêm crate ngoài allowlist giấy phép (D2-21, QD §9.3).
10. Đưa **số đo viết tay** vào tài liệu (số đo chỉ do `vis-dev`/`v2 nightly` sinh).

**Nhịp làm việc bắt buộc**:
- Mỗi task **≤ 3 giờ** (QD §5.1); lớn hơn **phải tách** (đợt này đã tách 6 lần: ST-72/72b, ST-85/85b, ST-87/87b, ST-93/93b, ST-101/101b, ST-114a/114b, ST-116/116b).
- **Red-first**: log đỏ **trước** khi sửa, có `sha256`; nếu task không có log đỏ thật (ví dụ refactor, hoặc "đã đúng sẵn") thì **ghi rõ lý do** thay vì giả vờ.
- **Gate** cuối mỗi task: `cd viscript && ../vis-dev gate --fast` → dòng cuối `vis-dev gate --fast: PASS (...)`.
- **Commit**: `[ST-##] <tóm tắt>`; **chỉ** đường dẫn trong `viscript/`; dùng `git commit --only -- <paths>`; `git fetch` + `rebase --autostash` rồi `push`.
- **Duyệt**: **chỉ** bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` **ghi trong `QUEUE.md`** ngay dưới task, kèm VD mới nếu quyết định đổi nội dung. Duyệt bằng chat **không** có giá trị.

---

## 3. Hiện trạng (đo tại 2026-11-16 09:40 JST)

| Hạng mục | Giá trị | Cách kiểm |
|---|---|---|
| HEAD repo | `3f02d72d` (commit dòng **OS** `KT-66`) | `git log -1 --format='%h %s'` |
| Commit cuối **dòng ViScript** | **`c1587398`** (2026-11-15) | `git log -1 -- viscript/` |
| `viscript/` thay đổi sau đó | **0 commit** | `git log c1587398..HEAD --oneline -- viscript/ \| wc -l` |
| Worktree `viscript/` | **sạch** | `git status --short -- viscript \| wc -l` → `0` |
| `HEAD` vs `origin/main` | **bằng nhau** | `git rev-parse HEAD origin/main` |
| Hàng QUEUE | **134**, tất cả `DONE` | mở `QUEUE.md`, đếm hàng bắt đầu bằng `ST-` (mỗi hàng 6 cột) |
| Báo cáo | **22** (`R1..R22`) | `ls viscript/agent/reports/*.md \| wc -l` |
| `report-check` | **PASS (22 báo cáo, 134 hàng QUEUE phủ đủ)** | `cd viscript && python3 test/report/check_report.py --all` |
| Quyết định | **VD-01..VD-118** (đủ, liên tục) | `grep -o 'VD-[0-9]\+' viscript/agent/DECISIONS.md \| sort -u \| wc -l` → `118` |

**Crate (workspace `viscript/`)**: `vis-diag` (sổ mã + render) · `vis-syntax` (lexer/parser/CST/`file-id`) · `vis-fmt` ·
`vis-types` (giải tên, tyck, HIR, `stdlib`) · `vis-interp` (interpreter, `Host`/`FsHost`/`NetHost`, async/kênh, HTTP client) ·
`vis-bytecode` (VBC1) · `vis-vm` · `vis-cli` (`vis`).
**Ngoài workspace `viscript/`**: `vis-host-linux/` và `vis-host-wasm/` (chỉ hàm `probe()`, **không** cưỡng chế gì; xem Q1/Q3).

### 3.1 Số đo cuối (đợt này, chạy lại được — §6)

| Phép đo | Kết quả | Ghi chú |
|---|---|---|
| `cargo test --workspace` | **1066 test** = 21 diag + 96 syntax + 27 fmt + 154 types + 27 interp + 62 bytecode + 9 vm + **670 cli** | |
| Ca ngôn ngữ (`test/sem`) | **447 ca / 22 nhóm** (397 `.vis` + 50 `.vish`; 83 ca `.err`) | |
| `vis test test/sem` (interp) | **447 ok** | |
| `vis test --vm test/sem` | **447 ok + 0 SKIP** | 0 SKIP là thành quả ST-89 |
| Parity interp ↔ VM | **398/398 chạy được + 49 lỗi biên dịch** | **không** phải 405 (xem §12.1) |
| `diag_cmp` | **364 xanh / 49 biên dịch / 34 lúc chạy** | |
| Differential (fuzz vi sai) | **100.000 chương trình / `lech=0`** seed `20261115` | bộ sinh phủ 8 nhóm cú pháp mới |
| Cache không che lỗi | **520 tệp × 4 lần chạy / 0 lệch** | interp, `--vm`, `--vm` (hit), `--vm --no-cache` |
| Host conformance | **54 ca × 2 đường / 0 lệch** (macOS) | 120 lần chạy |
| Soak | **10/10 vòng OK, 1297 s** | gate×3, diff×3, host×2, cache×2 |
| Fuzz parser 60 s | **~2,5 triệu ca / ~57 triệu token / 0 vi phạm** | |
| Ma trận target | **18/18 ô `cargo check` PASS** (windows-msvc, wasm32) | **chỉ check**, chưa chạy (§12.4) |
| R1 (≤1.500 dòng/tệp) | max **1060** (`lower/cmd.rs`) | `parse/cmd.rs` 1007, `parse/mod.rs` 1069, `main.rs` 591 |
| `FUNCS` / `all_codes` / `VBC1` / allowlist doc | **52 / 32 / 53 / 6** | |

---

## 4. 22 vòng — đã làm gì

| Vòng | Nội dung chính | Mốc đo |
|---|---|---|
| 1 | Lexer theo SPEC-lex, parser/CST lossless, `vis check` TAP, `vis fmt` lossless, `CmdCST` cho `.vish`, cổng doc-examples, `vis-diag` vi/en/ja | **157 test**, gate PASS |
| 2 | **VS0**: kiểu, giải tên, HIR, chẩn đoán, chạy `vis run` | VS0 chốt |
| 3 | Khung **VS1** | |
| 4 | **VS1b** | |
| 5 | **VS1c**: `json.decode`, `std.fs`, pipe→stdin + `complete`, generic thật | **507 test**, 346 ca, parity 320/320 |
| 6 | **VSH1a**: khe ống hợp nhất, closure-arg, `where/each/first/count`, `sort_by --desc`, prelude `FileEntry`/`ProcEntry` + `VS0605`, `get`/`select`, **`Render`**, `--fs-mock` | **606 test**, 370 ca/18 nhóm |
| 7 | **VSH1b**: khai kiểu trong `.vish`, `where` kiểm tĩnh, `ls --long`, `( … )` làm đối số | **651 test** |
| 8 | **VSH1c**: thân `cmd` bằng pipeline, nhóm-lệnh có cờ + nhiều dòng, `VS0405` | **704 test** |
| 9 | **VSH2**: `to json`, một comparator cho hai `sort_by`, `CODEGEN_TIN_TYCK` 21 → 0 | **730 test** |
| 10 | VS2 giai đoạn 1: async (builtin), kênh, ref-identity, derives ở HIR, `Ty::Any`, ngữ nghĩa gán, `VS050X` | xem `R10` |
| 11 | **VS2 #1–#8**: `cap[...]` là **kiểu thật** + `@`/`(write)` + `--dry`; cưỡng chế ở host (`VS0604`, `--grant`); probe Landlock + khung Linux; ma trận `cargo check` 18/18; **host suite 54 ca**; `report-check` | **874 test**, host 54/54 |
| 12 | Nhất quán hoá: `vis fmt` cho `.vish` + cú pháp mới (`--check` **497 tệp** xanh), `explain` 3 ngôn ngữ, **`vis test` chạy `test fn` thật** + `assert`, LSP module-aware, `--strict-caps` | **914 test** |
| 13 | Đóng nợ nhỏ (`exists`⇒`VS0604`, `assert <đk>`), **fuzz differential cho 8 nhóm cú pháp mới** (20k/100k 0 lệch), **cache không che lỗi** (520×4) | **961 test** |
| 14 | **VM có host `Ngoai`** ⇒ `--vm` **0 SKIP** (hết lệch interp/VM); `pkg entry` trong lock + `pkg remove`; LSP completion sau `.` (50 → 2 mục) | **978 test** |
| 15 | `cap[Proc]` + allowlist lệnh (`--grant p=/bin/echo`); kênh `close`/drain; **`chan(0)` bị từ chối rõ** (không giả vờ rendezvous) | **999 test** |
| 16 | Bất biến **sổ mã ↔ mã dùng** (`A⊆B`, `B\A=DU_PHONG`); `--grant` cho Proc; soak 8 vòng; **ROADMAP bàn giao** | **1010 test** |
| 17 | LSP hover trường + cache (**4,862 → 0,466 ms**); **`vis check/test --json`**; escape JSON đúng chuẩn (roundtrip `json.decode`); `pkg update` + `list --json` | **1033 test** |
| 18 | HTTP **chunked** (+`;ext`), **redirect ≤5**, timeout đọc 2 s; **`vis test --timeout`** (giết ca treo ~300 ms, không rò tiến trình); tách `lower/cmd.rs` | **1045 test** |
| 19 | Tách `lower.rs` 1482→993 ⇒ **R1 max ≤1.200**; `Location` đầy đủ (`..`, query-only, fragment) + `http.head`; `check --json` **theo tệp + severity** | **1056 test** |
| 20 | **Chẩn đoán đa tệp sai tệp**: tái hiện + chỉ đúng chỗ, **quyết định KHÔNG sửa** (>150 dòng) + kế hoạch; soak 10 vòng + `R20` | **1056 test** |
| 21 | **Đóng nợ correctness**: `Span.file` + luồn file-id (114a, hành vi y hệt), rồi map chẩn đoán theo file-id (114b) ⇒ **JSON + TAP đúng tệp/dòng** | **1062 test**, 6 ca red-first |
| 22 | Tách `parse/mod.rs` 1452→1069, `main.rs` 1254→**591**, LSP đa tệp (đã đúng sẵn, +4 ca), soak 10/10, **R22 + ROADMAP chốt** | **1066 test**, soak 1297 s |

Chi tiết từng task + hash: `viscript/agent/reports/R1..R22` (bảng ở mỗi báo cáo) và `git log -- viscript/`.

---

## 5. Tám hạng mục VS2 — **mức thật** (dán từ `viscript/agent/ROADMAP.md` §A)

> Nguyên tắc: cột "mức thật" là thứ **đã đo**. Không được nói cao hơn. Cột "còn thiếu" là **điều kiện đóng 100%**.

| # | Hạng mục | Mức thật (đo được) | Còn thiếu để đóng 100% |
|---|---|---|---|
| 1 | **Async** | `async fn`/`await`/`spawn`/join; scheduler **một luồng, run-to-completion**, **không song song**, **không GC**; 10k task interp **316 ms** / VM **168 ms**; drain 100 đúng thứ tự spawn | **Song song thật** + suspend/resume; test tăng tốc ≥2 core; async method; GC |
| 2 | **Kênh** | `chan(n≥1)` send/recv/close; **`chan(0)` bị từ chối `VS0505` ngay khi tạo**; `recv` sau close `none`; `send` sau close `VS0504`; deadlock `VS0505` không treo (0,01 s) | **Wait-set/rendezvous** cho `chan(0)` **hoặc** tuyên bố từ chối **vĩnh viễn có văn bản**; `select`/timeout |
| 3 | **Net/HTTP** | Loopback TCP thật; **chunked** (`;ext`, trailer bỏ), **redirect GET ≤5** (Location `..`/query-only/fragment/rỗng), **`http.head`** không thân/không redirect, timeout đọc 2 s, cap 8 MiB; **không TLS/HTTP2/keep-alive**; 21 ca | **TLS** (cần duyệt crate), HTTP2, keep-alive, cookie/auth, ≥2 kết nối đồng thời, test Internet thật |
| 4 | **Quyền** | `cap[Dir/File/Net/Proc]`; `--strict-caps`; `--grant` (Dir/File/Net = đường dẫn, **Proc = allowlist**); `VS0604` exit 2; **chưa siết mặc định**; **chưa Landlock thật**; tiến trình con **không sandbox** | **Landlock thật** Linux + **runner Linux**; siết mặc định; sandbox con |
| 5 | **LSP** | `--stdio`; diagnostics **module-aware**, UTF-16; **chẩn đoán đa tệp đúng tài liệu đang mở** (4 ca); completion sau `.`; **hover trường**; `didClose` dọn cache; cache module phụ **0,466 ms** | **Publish chẩn đoán tệp phụ** (hiện một-tài-liệu), hover method, generic, bảng `std.str`, incremental sync |
| 6 | **Gói** | Registry **cục bộ**, manifest `files[]`, lock **SHA-256**, `.vis/store` content-addressed, `use pkg.*`, `VS0701/0702`, lock **`entry`**, `pkg update` idempotent, `list --json`, `remove`; **chưa** resolver/HTTP/ký số | **Semver ranges + resolver**; registry HTTP; chữ ký; GC store; migration lock cũ |
| 7 | **Đa nền tảng** | **CHỈ `cargo check`**: 18/18 ô PASS; **chưa có runner** Windows/Linux/wasm | **Runner thật** (wasmtime wasip1, Linux+Landlock, Windows); CI 54 host trên ≥2 nền |
| 8 | **Hợp chuẩn host** | **54 ca chạy thật trên macOS ×2 đường, 0 lệch**; VM 54/54; cache 520×4 0 lệch; **net không nằm trong suite** | 54 ca × **4 nền thật**; net/TLS trong suite; CI lặp |

---

## 6. Tự kiểm chứng (đừng tin tài liệu — chạy 1 lần)

```bash
cd /Users/mps/dvchd/ViFastOs/viscript

# 1) Toàn bộ test (~6–10 phút): kỳ vọng "test result: ok. … 1066 passed" tổng các crate
~/.cargo/bin/cargo test --workspace            # LƯU Ý: cargo KHÔNG có trong PATH mặc định

# 2) Cổng bắt buộc (~4 phút). Dòng cuối phải là:
#    vis-dev gate --fast: PASS (L1 chưa có tới khi có sim ViScript)
../vis-dev gate --fast

# 3) Parity + số ca (nhanh)
target/debug/vis test test/sem                 # 447 ok (interp)
target/debug/vis test --vm test/sem            # 447 ok + 0 SKIP (VM)

# 4) Differential 100k (~2,5–7 phút tuỳ tải). Kỳ vọng: lech=0
VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff

# 5) Host suite (~10 giây): 54/54
./bin/vis-host-suite

# 6) Ma trận target (~1 phút): 18/18 PASS — CHỈ check
./bin/vis-targets

# 7) Soak 10 vòng (~22 phút, TUẦN TỰ, không chạy song song build/cổng khác)
./bin/vis-soak

# 8) Báo cáo khớp QUEUE
python3 test/report/check_report.py --all      # PASS (22 báo cáo, 134 hàng QUEUE phủ đủ)
```

**Cảnh báo môi trường** (đã gặp thật):
- `cargo` không có trong PATH của shell/background job ⇒ exit 127. Dùng `~/.cargo/bin/cargo` hoặc export PATH.
- Máy 12 core/36 GB, nhưng dòng OS chạy QEMU song song ⇒ thời gian test/diff **dao động 100–450 s** tuỳ tải. Đừng dùng thời gian làm tiêu chí tuyệt đối; flake do tải ⇒ `T-INFRA` + log, **không** retry.
- `git commit --only -- <paths>` là **bắt buộc** (nếu không sẽ nuốt thay đổi của dòng OS đang dở).
- `git fetch` + `git rebase --autostash` trước `push`.

---

## 7. VIỆC 1 — **12 quyết định chính cần Agent 0 ký** (Q1..Q12) + 3 quyết định phụ (Q13..Q15)

> Mỗi mục: hiện trạng → vì sao **chỉ bạn** quyết → lựa chọn → chi phí → **đề xuất**. Khi quyết, hãy thêm 1 hàng task vào QUEUE
> (kèm tiêu chí đo) + 1 mục `VD-###` trong `DECISIONS.md`, rồi giao Agent 2.

### Q1. **Landlock thật** (chặn bởi: cấm `unsafe`, cấm crate ngoài allowlist, máy chỉ macOS)
- **Hiện trạng**: quyền đã **cưỡng chế ở tầng runtime** (`FsGuard`/`NetGuard`/`Runner`, `VS0604`, exit 2) — **đã đo**. Landlock **chỉ probe**:
  `vis run --sandbox-check` trên macOS in `landlock: không hỗ trợ trên macos`, exit 3. Crate `vis-host-linux/` mới có `probe()` đọc
  `/sys/kernel/security/lsm`, **chưa cưỡng chế gì**; bằng chứng duy nhất là `cargo check --target x86_64-unknown-linux-gnu` xanh.
- **Vì sao chỉ bạn**: gọi syscall Landlock cần **`unsafe`** (danh sách crate đóng **rỗng**) **hoặc** crate `landlock` (ngoài allowlist D2-21).
  `AGENTS.md` §4 cấm cả hai; chỉ Agent 0 duyệt được. Thêm nữa **không có runner Linux** ⇒ kể cả có code cũng **không đo được**.
- **Lựa chọn**: (a) duyệt crate `landlock` vào allowlist + (a2) dựng runner Linux; (b) duyệt `unsafe` **đóng** trong `vis-host-linux` + case âm + runner Linux; (c) **không làm** và tuyên bố trong SPEC/ROADMAP: "cưỡng chế ở tầng runtime; kernel sandbox **không** hỗ trợ".
- **Chi phí**: (a)+(a2) ≈ 6–10 h + môi trường; (b) như (a) nhưng rủi ro `unsafe` cao hơn; (c) 1 h (văn bản).
- **Đề xuất**: **(c) ngay bây giờ** (vì không có runner Linux thì không có bằng chứng), và mở **Q2 trước**. Khi có runner Linux ⇒ làm (a).

### Q2. **Runner Linux** (chặn bởi: máy dev là macOS, repo chưa có CI)
- **Hiện trạng**: mọi số đo là macOS aarch64. Linux chỉ có `cargo check` chéo.
- **Lựa chọn**: (a) Docker/Podman trên máy dev (`linux/arm64`) + script `bin/vis-linux` chạy gate/host suite; (b) CI (GitHub Actions) chạy `ubuntu-latest`; (c) thuê/thêm máy; (d) **không**.
- **Chi phí**: (a) 3–5 h (Dockerfile + mount cache + sửa script giả định đường dẫn macOS); (b) 4–6 h + cần quyền repo/CI; (d) 0.
- **Đề xuất**: **(a)** nếu máy có Docker — nó mở khoá Q1 thật, Q7 (host 54 ca trên nền 2), và là điều kiện của hạng mục 7/8.

### Q3. **Runner wasm32-wasip1** (chặn bởi: cần cài `wasmtime`, và `vis-cli` không chạy được trên wasm)
- **Hiện trạng**: `cargo check -p vis-cli --target wasm32-unknown-unknown` **xanh** nhưng **không chạy được** (std chỉ có stub cho process/net/fs/thread).
  `vis-host-wasm/` mới là khung `probe()` trả chuỗi.
- **Lựa chọn**: (a) tách host để `vis-cli` chạy được trên `wasm32-wasip1` + wasmtime runner; (b) chỉ giữ `vis-interp`/`vis-types` chạy được trên wasm (thư viện nhúng) + runner cho **thư viện**, không cho CLI; (c) **không**.
- **Chi phí**: (a) **lớn** (2–4 ngày: tách `Host`, `std::fs`, `std::process`, threads, time); (b) 6–10 h; (c) 0.
- **Đề xuất**: **(c)** trong đợt này; ghi rõ "wasm = chỉ check" (đúng như hiện tại). Nếu muốn có "4 nền" thật thì ưu tiên **Linux (Q2)** trước.

### Q4. **Runner Windows** — chặn bởi: không có máy/CI Windows; `cargo build` msvc cần linker MSVC.
- **Đề xuất**: **không** trong đợt này; giữ mức "chỉ check".

### Q5. **TLS cho HTTP** (chặn bởi: cần crate ngoài allowlist: `rustls` + `webpki-roots`/`ring`)
- **Hiện trạng**: HTTP client chỉ **loopback, không TLS**. `Location`/chunked/HEAD/timeout đã thật.
- **Lựa chọn**: (a) duyệt `rustls` (+ provider) ⇒ HTTPS thật; (b) **không**, tuyên bố rõ "ViScript chỉ nói HTTP loopback; HTTPS ngoài phạm vi".
- **Chi phí**: (a) 8–12 h (TLS handshake qua trait host, không `unsafe` nếu dùng `rustls`; cần test server TLS cục bộ); (b) 1 h văn bản.
- **Đề xuất**: **(b)** cho đợt này — vì ngôn ngữ chưa cần Internet thật, và (a) kéo theo Q2 (đo trên Linux) để có ý nghĩa.

### Q6. **Async song song thật** (chặn bởi: **đổi ngữ nghĩa** — VD-90 đã chốt một luồng run-to-completion)
- **Hiện trạng**: `spawn` tạo task **chạy tuần tự** khi join; không có suspend giữa thân hàm; không GC.
  Nghĩa là: `async` hiện là **cấu trúc** (task + join + lỗi `VS0506`), **không** phải **đồng thời**.
- **Vì sao chỉ bạn**: đây là **thay đổi ngữ nghĩa ngôn ngữ** (VD-90/VD-95), ảnh hưởng: interp, VM bytecode (thêm op suspend/resume?), scheduler,
  `VBC1` (bump format?), determinism test, và **toàn bộ tài liệu**.
- **Lựa chọn**: (a) **giữ** một luồng + **ghi rõ trong tài liệu/SPEC** "async = task tuần tự có join, không song song"; (b) làm scheduler thật
  (work-stealing hoặc ít nhất cooperative qua `await`/`recv`), chấp nhận đổi `VBC1` + test.
- **Chi phí**: (a) 2 h văn bản (nhưng **đúng sự thật**); (b) **1–2 tuần** + thiết kế + review, rủi ro hồi quy cao.
- **Đề xuất**: **(a)** — và biến (b) thành dự án riêng có thiết kế (không làm trong vòng task 3 h).

### Q7. **`chan(0)` rendezvous** — chặn bởi cùng lý do Q6.
- **Hiện trạng**: `chan[T](0)` bị **từ chối ngay khi tạo** (`VS0505`, exit 1, 0,01 s) — **không** treo, **không** giả vờ. Có 10 ca.
- **Lựa chọn**: (a) **tuyên bố từ chối vĩnh viễn** (ghi vào SPEC/thiết kế + giữ `VS0505`); (b) làm wait-set (đi cùng Q6b).
- **Đề xuất**: **(a)**; nếu chọn Q6b thì (b) đi kèm.

### Q8. **Siết mặc định caps** (chặn bởi: **đổi hành vi toàn bộ** + cần migration)
- **Hiện trạng**: chương trình **không khai `cap`** ⇒ **không hạn chế** (tương thích 1066 test). Chỉ khi `--strict-caps` mới bắt buộc.
- **Lựa chọn**: (a) giữ nguyên (an toàn cho tương thích, yếu về bảo mật); (b) đổi mặc định sang **siết** + thêm `--no-strict-caps` để tương thích,
  và cập nhật **toàn bộ** fixture/host suite (54 ca + 447 sem có thể phải khai `cap` ở nhiều chỗ); (c) siết mặc định **chỉ** khi chạy từ `vis pkg`
  (chương trình cài từ gói luôn bị siết).
- **Chi phí**: (b) 8–14 h + rủi ro vỡ nhiều fixture; (c) 4–6 h.
- **Đề xuất**: **(c)** — bước trung gian đúng hướng bảo mật mà không phá tương thích; ghi nợ cho (b).

### Q9. **Gói: semver ranges + resolver + registry HTTP + ký số**
- **Hiện trạng**: registry **cục bộ**, mỗi gói 1 version, lock SHA-256, `update` idempotent. **Không** ranges, **không** HTTP, **không** chữ ký.
- **Lựa chọn**: (a) chỉ **ranges + resolver tối giản** (`^`, `~`, `>=`, chọn cao nhất thoả, ghi lock đầy đủ) — không mạng; (b)
  (a) + registry HTTP + ký số (Ed25519 — cần crate); (c) không làm.
- **Chi phí**: (a) 8–12 h; (b) +2–3 ngày + duyệt crate.
- **Đề xuất**: **(a)** — nó làm `pkg` dùng được thật; (b) để sau khi có Q2/Q5.

### Q10. **LSP publish chẩn đoán tệp phụ** (nợ #104)
- **Hiện trạng**: LSP publish **một tài liệu** (đúng chuẩn cho tài liệu đang mở); tệp phụ **không** được publish ⇒ IDE không thấy lỗi của module chưa mở.
- **Lựa chọn**: (a) giữ một-tài-liệu (đúng chuẩn; **đóng nợ thành "won't fix có lý do"**); (b) publish thêm cho tệp phụ (một số client hiển thị sai/không mở được).
- **Đề xuất**: **(a)** — ghi lý do vào `ROADMAP.md`; đổi nợ #104 thành "quyết định: không làm".

### Q11. **Audit `Span` tổng hợp `file=0`** (nợ #103)
- **Hiện trạng**: node do lower **sinh** (không copy từ CST) vẫn `file = 0`; nếu chẩn đoán tyck rơi đúng node đó **ở module** thì vẫn báo về tệp chính.
  **Chưa tìm được ca thực tế** (đã thử), nhưng là lỗ tiềm ẩn.
- **Lựa chọn**: (a) audit + sửa (truyền `file` khi sinh node) 2–3 h; (b) để nguyên + ghi rõ.
- **Đề xuất**: **(a)** — rẻ, và làm chẩn đoán "đúng hoàn toàn" mới dám tuyên bố.

### Q12. **Mục tiêu nội bộ R1 ≤1.000** (`lower/cmd.rs` 1060, `parse/cmd.rs` 1007)
- **Hiện trạng**: **luật** là ≤1.500 (đang max **1060** ⇒ **đạt luật**). Mục tiêu ≤1.000 là **tôi tự đặt**, chưa đạt.
- **Lựa chọn**: (a) tiếp tục tách (2 task × 2,5 h) để đạt ≤1.000; (b) **bỏ mục tiêu nội bộ**, sửa `ROADMAP.md` cho đúng (luật là 1.500).
- **Đề xuất**: **(b)** — đừng tiêu thời gian vào mục tiêu không có trong luật; giữ max 1060.

**Quyết định phụ (nhỏ, có thể gộp 1 lần):**
- **Q13**: `VBC1` bump để `Span` mang `file` (debug đa tệp trong VM)? *Đề xuất: không* (chỉ khi cần).
- **Q14**: **Chốt API `std`** (`FUNCS` 52, có `http.head` thêm ở ST-109) — review 1 lần rồi "đóng băng API", mọi thêm mới phải qua bạn.
- **Q15**: `FUNCS`/`all_codes`/`VBC1` có phải là "cam kết tương thích" không (nếu có ⇒ cần quy trình bump).

---

## 8. VIỆC 2 — **8 việc quản trị chỉ bạn làm được** (G1..G8)

| # | Việc | Vì sao chỉ bạn | Ưu tiên |
|---|---|---|---|
| **G1** | **`AGENTS.md` (root) đã lỗi thời**: dòng 14 ghi "VD-01..**VD-88**", §8 ghi "VÒNG 1..**9** đều DONE" (thực tế **22 vòng, VD-118**, 1066 test). Cần cập nhật để người mới không đọc sai. | `AGENTS.md` thuộc "người điều phối / Agent 0" | **cao** |
| **G2** | **`viscript/AGENTS.md` lỗi thời**: dòng 10/22 ghi "VD-01..VD-88", dòng 30–32 mô tả vòng 1/8/9 là mới nhất. | cùng lý do | **cao** |
| **G3** | **`viscript/agent/STATE.md` lỗi thời ở đầu tệp**: tệp dài **1903 dòng** (nửa đầu là lịch sử), nhưng **header** vẫn "vòng 2 / VS0" và `Task` ghi "ST-118 … **chưa commit**" (đã commit `326cdfa0`). Agent 2 sở hữu ⇒ bạn **yêu cầu** Agent 2 viết lại **header** (60 dòng) và giữ phần lịch sử bên dưới. | tệp của Agent 2 | **cao** |
| **G4** | **R23**: sau khi bạn ra quyết định Q1..Q12, cần 1 báo cáo xác nhận lại **toàn bộ số đo** + ghi các quyết định (đừng để R22 là báo cáo cuối trong khi trạng thái đã đổi). | chỉ Agent 0 mở task | trung bình |
| **G5** | **`ROADMAP.md`**: cập nhật sau mỗi quyết định (điều kiện đóng → "đã đóng"/"không làm có lý do"). Đây là tài liệu bàn giao cho người sau. | nội dung chính sách | trung bình |
| **G6** | **`docs/viscript/ViScript-thiet-ke.md` đã lệch thực tế** ở vài chỗ: (i) `cap[...]` được viết như đã có từ đầu nhưng **thực tế mới có từ vòng 11**; (ii) `chan(0)` không nói bị từ chối; (iii) `assert <đk>` mới có từ ST-83; (iv) `.vish`: `fn main` **chỉ** để khai `cap`, thân có câu lệnh ⇒ `VS0201` (ST-93b). Đồng bộ để tài liệu không dạy sai. | QD: **chỉ Agent 0/1** được sửa `docs/` | **cao** |
| **G7** | **Chốt API `std`** (Q14) — review một lần 52 hàm + 9 module, ghi VD "API đóng băng" nếu đồng ý. | quyết định kiến trúc | trung bình |
| **G8** | **Thêm hàng QUEUE cho từng quyết định** (READY + tiêu chí đo + ngân sách ≤3 h + lệnh cổng) — nếu không, Agent 2 không có gì để làm và trạng thái treo. | chỉ bạn ghi QUEUE | **cao** |

**Mẫu hàng QUEUE** (dán + sửa):
```
| ST-119 | **Landlock thật (Linux)**: cưỡng chế bằng kernel trong `vis-host-linux` (crate `landlock` đã duyệt) + 8 ca Linux | ST-118 | 3 h | **READY** | 8 ca `cfg(linux)` xanh trên runner; macOS ghi "chưa đo"; gate PASS |
```

---

## 9. VIỆC 3 — **104 nợ**: nhóm theo chủ đề + top 10

Toàn bộ danh sách đánh số ở **`viscript/agent/reports/R22-viscript.md` §5** (1..104). Nhóm lại:

| Nhóm | Số mục (xấp xỉ) | Ví dụ tiêu biểu | Ai quyết |
|---|---|---|---|
| **A. Bị chặn bởi môi trường/quyền** | 12 | Landlock thật; runner Linux/wasm/Windows; TLS; CI 4 nền | **bạn (Q1–Q5)** |
| **B. Đổi ngữ nghĩa (lớn)** | 6 | async song song; rendezvous `chan(0)`; siết mặc định caps; `select`/timeout kênh | **bạn (Q6–Q8)** |
| **C. Correctness/ngữ nghĩa nhỏ** | 18 | `Span` tổng hợp `file=0`; `assert` không có message; biến tên `assert`; `pass` nhận diện theo tên; `.vish` có `main` + pipeline **bỏ main** khi thân rỗng; `--indent-report` fallback ước lượng; `dong_thut_sai` là heuristic văn bản | agent2 được |
| **D. API/feature thiếu** | 22 | pkg ranges/resolver/HTTP/ký/GC; LSP publish tệp phụ + hover method + generic + `std.str`; HTTP TLS/HTTP2/keep-alive/cookie; HEAD redirect | **bạn (Q9, Q10)** |
| **E. Hiệu năng/chất lượng** | 14 | front end chạy mỗi lần (~1 ms, không fast-path); cache LSP là global; `vis test` dùng `--no-cache`; `--timeout` tính cả compile; pipe có thể nghẽn | agent2 được |
| **F. Tooling/chẩn đoán** | 12 | `check --json` thiếu metadata main/phụ; TAP chỉ báo chẩn đoán đầu; `usage_explain` dịch tay; `--list` có cột severity; ca chống nhánh chết **đọc mã nguồn** (brittle) | agent2 được |
| **G. Văn bản/tài liệu** | 10 | AGENTS.md & STATE.md lỗi thời; docs lệch; ROADMAP; hợp đồng JSON chưa ghi vào SPEC | **bạn (G1–G6)** |
| **H. Nợ "won't fix có lý do"** | 10 | `vis-cli` không chạy trên wasm; `chan(0)` từ chối; LSP một tài liệu | **bạn (Q7, Q10)** |

**Top 10 nên làm trước** (theo mức "chặn tuyên bố đúng" chứ không theo độ khó):

| # | Việc | Vì sao trước | Chi phí |
|---|---|---|---|
| 1 | **G1/G2/G3**: sửa `AGENTS.md`, `viscript/AGENTS.md`, `STATE.md` | Người mới đọc sai ngay từ bước 1 | 1–2 h |
| 2 | **Q1+Q2**: runner Linux (+ Landlock nếu duyệt) | Mở khoá hạng mục 4, 7, 8 | 1 ngày |
| 3 | **Q6+Q7**: quyết định + **văn bản hoá** async/rendezvous | Đang là chỗ tài liệu dễ nói quá | 2 h |
| 4 | **Q10**: đóng nợ LSP tệp phụ (hoặc won't fix) | Xoá 1 nợ vĩnh viễn | 0,5 h |
| 5 | **Q11**: audit `Span` tổng hợp | Để được phép nói "chẩn đoán đa tệp đúng" | 2–3 h |
| 6 | **Q9a**: semver ranges + resolver | `pkg` mới dùng được thật | 1,5 ngày |
| 7 | **Q8c**: siết caps cho chương trình từ gói | Bước bảo mật đúng, không phá tương thích | 6 h |
| 8 | **32 `FUNCS` API freeze** (Q14) | Tránh trôi API | 1 h |
| 9 | **D**: `select`/timeout cho kênh | Feature người dùng sẽ hỏi | 6 h |
| 10 | **R23** | Xác nhận số đo sau khi đổi | 1,5 h |

---

## 10. Rủi ro & điểm cần **review độc lập**

1. **Mọi số đo là macOS aarch64, 1 máy.** Không có CI. Thời gian dao động mạnh theo tải dòng OS.
2. **`18/18 check` ≠ "hỗ trợ 4 nền".** `vis-cli` *biên dịch* được ra wasm nhưng **không chạy** (std stub). Đừng tuyên bố "đa nền tảng xong".
3. **`--sandbox-check` trên macOS trả exit 3** ("không hỗ trợ") — **đúng thiết kế**, không phải lỗi.
4. **Async không song song**; **kênh không rendezvous**; **net không TLS**; **quyền chưa siết mặc định**; **chẩn đoán chưa phủ tệp phụ**. Bốn câu này phải xuất hiện ở mọi báo cáo.
5. **Cặp số dễ nhầm**: parity = **398 + 49** (không phải 405); test = **1066**; sem = **447/22**; `all_codes` = **32**; `FUNCS` = **52**; `VBC1` = **53**; allowlist = **6**.
6. **Khoá đếm nằm rải rác**: thêm/bớt ca phải sửa **đồng thời** ≥7 chỗ (`sem.rs` TONG + nhóm, `vis_test.rs`, `diag_cmp.rs`, `roundtrip.rs` ×2, `vs_interp.rs` ×4, `fuzz_mutate.rs`, `tin_tyck.rs`). Agent 2 đã làm đúng 22 lần; nếu thấy lệch ⇒ hỏi trước khi "sửa cho xanh".
7. **`test/sem` có 83 ca `.err`** — phần lớn kiểm chẩn đoán, không kiểm chạy. Đừng đếm chúng vào "ca chạy".
8. **Điểm cần review độc lập (chưa ai review từ vòng 10)**: (a) `Value::Cap` có phá bất biến `Value` không? (b) `FsGuard` canonicalize có TOCTOU không?
   (c) `--grant` allowlist so **basename** ⇒ có thể chạy binary cùng tên ở path khác (rộng có chủ đích, nhưng cần bạn xác nhận);
   (d) `vis test` biến đổi **văn bản** để chạy `test fn` ⇒ câu lệnh cấp tệp chạy lại cho mỗi test (đã ghi nợ);
   (e) `json_esc` tự viết — đã roundtrip qua `json.decode` + `python3`, nhưng chưa fuzz chuỗi ngẫu nhiên.

---

## 11. Quy trình vận hành lại (cadence đã hiệu quả 22 vòng)

**Vòng lặp chuẩn** (mỗi task ≈ 40–75 phút):

1. **Bạn**: thêm hàng `ST-##` vào `QUEUE.md` (READY) + mục `VD-###` nếu có quyết định mới → commit `[ST-00]` **chỉ** `viscript/agent/QUEUE.md` + `DECISIONS.md` → push.
2. **Bạn**: giao Agent 2 bằng `send_message` với **brief** gồm: mục tiêu, **tiêu chí đo**, ràng buộc (giữ các số khoá), **cấm**, và **cách báo cáo**.
3. **Agent 2**: log đỏ (`st##-red-*.log` + sha256) → code → `gate --fast` → `vis-diff` 100k → commit `[ST-##]` **chỉ** `viscript/` → push → báo cáo (hash, số đo, nợ).
4. **Bạn**: đọc báo cáo, **đối chiếu số đo với yêu cầu**; nếu ổn ⇒ lật hàng QUEUE sang `**DONE**`, thêm khối `APPROVED <ngày> Agent 0` ghi hash + số đo + **nợ** + mở task kế; commit `[ST-00]`.
5. Lặp. Cuối mỗi ~5 task ⇒ 1 báo cáo `R##` + soak.

**Brief mẫu** (dùng lại được):
```
Task: ST-### — <mục tiêu 1 câu>.
Đo trước (bắt buộc): <lệnh cụ thể> → ghi `st###-red-*.log` + sha256.
Việc: <3–6 gạch đầu dòng, KHÔNG mô tả ngữ nghĩa chi tiết — để agent đo>.
Ca: ≥N ca <ở đâu>, mỗi ca chạy CẢ `run` và `run --vm --no-cache`, so byte.
Ràng buộc: giữ <các số khoá>; không `unsafe`; không crate mới; R1 ≤1.500; red-first; gate PASS; diff 100k; commit `[ST-###]`; STATE+JOURNAL.
Không chạm: docs/, v2/, spec/, QUEUE/DECISIONS/reports.
Báo cáo: <danh sách mục cần có>.
```

**Pitfalls cụ thể** (đã gặp, đừng lặp):
- `awk -F'|'` **vỡ** trên `QUEUE.md` vì mô tả có `\|` ⇒ sửa theo **dòng** (`python3` đọc/ghi list dòng), đừng split theo `|`.
- `cargo` không có trong PATH ⇒ `~/.cargo/bin/cargo`; background job mất PATH ⇒ `T-INFRA`, sửa script rồi chạy lại **toàn bộ** (không retry cục bộ).
- `git commit --only -- <paths>`; `git fetch` + `rebase --autostash` trước push (repo có dòng OS commit liên tục).
- Test đo **thời gian** dưới tải // hay flake ⇒ đo in-process, lấy **min nhiều vòng**, hoặc so **phần thêm** so với nền (đã dùng 2 lần).
- `fmt --check` / clippy đỏ ⇒ sửa **code**, không `#[allow]`, không nới.
- Khi agent nói "đo được khác brief" ⇒ **số đo thắng** (§12).

---

## 12. Bài học: **8 lần chỉ đạo của lead SAI, agent đo lại ĐÚNG**

> Đây là phần quan trọng nhất để bạn viết brief tốt: **đừng suy đoán ngữ nghĩa trong brief**, hãy nêu mục tiêu + tiêu chí đo.

| # | Lead chỉ đạo | Thực tế đo được | Bài học |
|---|---|---|---|
| 1 | "Nâng parity từ **398 lên 405** khi VM có host `Ngoai`" | 7 ca đó **đã nằm trong 398** (trước khớp bằng `VS0604`); đúng là **398 + 49**. 405 là **đếm trùng** | Đừng tự cộng số; hỏi agent phân hoạch |
| 2 | "`chan(0)` phải **exit 2** (chẩn đoán tĩnh `VS0412`)" | Làm vậy đẩy 1 ca sang **lỗi biên dịch** ⇒ parity thành **397+50**, phá ràng buộc; chọn **exit 1 runtime** để giữ 447 sem + 398+49 | Ràng buộc số đo > ý thích của lead |
| 3 | "`print(p)` in `P(x: 1, ten: a)`" | In **dạng vô danh** `{x: 1, ten: a}` | Đừng đoán định dạng in |
| 4 | "Constructor là `some(5)`" | Là **`Some`** | Cú pháp phải tra, không nhớ |
| 5 | "`type P: x: int, ten: str` một dòng + `P(x:…)`" | **Không hợp lệ**; phải `type P:` nhiều dòng + `let p: P = {x: 1, ten: "a"}` | |
| 6 | "LSP đang **báo `VS0701` giả**" | LSP **bỏ sót** chẩn đoán (diagnostics rỗng) — lỗi ngược lại | Đừng chẩn đoán hộ agent |
| 7 | "`.vish` `fn main` có thân nhưng không pipeline ⇒ **không in gì**" | **In `x`** (im lặng kiểu khác) | |
| 8 | "`vis fmt` nên **chuẩn hoá thụt lề** `.vish`" | Đo được: thụt lề **cấp tệp CÓ ngữ nghĩa** (tệp thụt hết ⇒ không in gì) ⇒ chọn nhánh **không đổi** | Đo trước, quyết sau |

---

## 13. Nơi đọc + phụ lục

### 13.1 Tệp quan trọng (theo thứ tự nên đọc)

| Tệp | Nội dung | Dòng |
|---|---|---|
| [`AGENTS.md`](AGENTS.md) | luật chung (nhưng **lỗi thời** ở §8 — xem G1) | 116 |
| [`viscript/agent/ROADMAP.md`](viscript/agent/ROADMAP.md) | **bàn giao 1 trang**: 8 hạng mục × mức thật × điều kiện đóng + lệnh chạy | 64 |
| [`viscript/agent/reports/R22-viscript.md`](viscript/agent/reports/R22-viscript.md) | báo cáo mới nhất (số liệu, §5 nợ 104 mục, §9 bàn giao) | 142 |
| [`viscript/agent/QUEUE.md`](viscript/agent/QUEUE.md) | 134 hàng task + khối `APPROVED` (nguồn sự thật về tiến độ) | 1669 |
| [`viscript/agent/DECISIONS.md`](viscript/agent/DECISIONS.md) | **VD-01..VD-118** (mọi quyết định kỹ thuật) | — |
| [`viscript/agent/HOST-BAO-DAM.md`](viscript/agent/HOST-BAO-DAM.md) | mức bảo đảm host/HTTP theo nền tảng + **cấm ngôn từ** | 93 |
| [`viscript/agent/STATE.md`](viscript/agent/STATE.md) | trạng thái (1903 dòng, **đầu tệp lỗi thời** — G3) | 1903 |
| [`viscript/agent/JOURNAL/st112-ke-hoach.md`](viscript/agent/JOURNAL/st112-ke-hoach.md) | kế hoạch `Span.file` (đã dùng ở ST-114a/b) | 30 |
| `viscript/agent/reports/R1..R21` | lịch sử từng vòng | — |
| `docs/viscript/ViScript-thiet-ke.md` | thiết kế ngôn ngữ (**được sửa bởi bạn**, xem G6) | — |
| `docs/QUYET-DINH-sau-review.md` | QD — **cấm sửa** | — |

### 13.2 Tóm tắt **VD-100..VD-118** (các quyết định của đợt này)

| VD | Nội dung 1 dòng |
|---|---|
| 100/105 | `NetHost` + HTTP đặt **trong** `vis-interp`; chỉ **loopback**; R1 kiểm trước khi viết code |
| 101 | `pkg` **cục bộ, tất định**, tiền tố `pkg.` |
| 102 | LSP là công cụ riêng (`vis lsp --stdio`) |
| 103 | **Cưỡng chế quyền ở biên host** (không trong tyck) |
| 104 | **Mức bằng chứng theo nền**: check / biên dịch / chạy — không được nói vượt |
| 106 | `cap[...]` là **kiểu thật**; cưỡng chế **chỉ khi khai báo**; `--dry` in quyền từ CST |
| 107 | Landlock: **chỉ probe** + khung; **cấm** nói "đã cưỡng chế bằng Landlock" |
| 108–110 | Phạm vi vòng 12/13/14 (nhất quán hoá công cụ, kiểm chứng sâu, đóng lệch interp↔VM + pkg entry) |
| 111 | `cap[Proc]` + kênh `close`/drain; `chan(0)` **từ chối** |
| 112 | Bất biến **sổ mã↔mã dùng**; soak tuần tự; ROADMAP là bàn giao |
| 113 | `--json` **ổn định** cho check/test; chỉ JSON ra stdout |
| 114 | HTTP chunked/redirect/timeout; `test --timeout`; **R1 headroom** |
| 115 | HTTP `Location` đầy đủ + `HEAD`; `--json` **theo tệp + severity** |
| 116 | Chẩn đoán trước–sửa sau; nếu >150 dòng thì **kế hoạch**, không làm nửa vời |
| 117 | `Span.file` + map chẩn đoán theo **file-id** (đóng nợ correctness) |
| 118 | Vòng 22: R1 ≤1.200, LSP đa tệp, soak chốt + R22 |

### 13.3 Hash các task cuối đợt này (để tra nhanh)

```
ST-104 2b55b102  ST-105 c3afa7b1  ST-106 5d7b1353  ST-107 d0fcf09d
ST-108 0cd6d471  ST-109 28c441cb  ST-110 5ae46d04  ST-111 3c67c298
ST-112 7c3f5a49  ST-113 36ed5922  ST-114a bf11a21f ST-114b 614d55e0
ST-115 6ca45ef0  ST-116 167d7cc4  ST-116b e34a8984 ST-117 5183b81a
ST-118 326cdfa0  (lật DONE: c1587398 = commit cuối của dòng ViScript)
```

### 13.4 Bảng chọn nhanh: bạn muốn gì → đọc mục nào

| Bạn muốn | Xem |
|---|---|
| Biết "xong đến đâu" trong 2 phút | §3 + §5 |
| Biết "còn thiếu gì để gọi là hoàn thiện" | §5 cột cuối + §9 |
| Ra quyết định | §7 (Q1..Q15) + §8 (G1..G8) |
| Giao việc cho Agent 2 | §11 (brief mẫu + pitfalls) |
| Tránh nói sai | §10 + §12 |
| Kiểm chứng số | §6 |

---

## 14. Ba việc **đề nghị làm ngay** (nếu bạn chỉ có 1 giờ)

1. **G1 + G2 + G3** (1–2 h): sửa `AGENTS.md`, `viscript/AGENTS.md`, yêu cầu Agent 2 sửa `STATE.md`. → Người tiếp theo không đọc sai.
2. **Q6a + Q7a + Q10a** (2 h): *ghi văn bản* cho 3 chỗ dễ bị nói quá (async tuần tự, `chan(0)` từ chối vĩnh viễn, LSP một tài liệu) vào `docs/viscript/ViScript-thiet-ke.md` + `ROADMAP.md`. → Chặn tuyên bố sai.
3. **G8** (0,5 h): thêm 3–5 hàng QUEUE `READY` cho các việc bạn đã chọn ở §7. → Agent 2 có việc ngay, không chờ.

> **Không** cần làm gì với code: dòng ViScript đang **xanh, sạch, đo được**. Việc còn lại là **quyết định + văn bản + môi trường đo**, không phải sửa bug đã biết.
