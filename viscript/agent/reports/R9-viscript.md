<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R9-viscript — báo cáo vòng 9 (chốt **VSH2**), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`8c8c36a`** (QUEUE@head đủ **61 hàng**, 6 cột; ST-53/ST-54 `DONE`,
ST-55 `READY` báo cáo này). Số đo chạy trên **mã của `4627fbb`**. Bản máy đọc:
[`R9-viscript.json`](R9-viscript.json) — **đúng 11 khoá** Khối E (`project = "viscript"`, `round = 9`).

## 1. Tóm tắt (5 dòng)

1. **VSH2 chạy được đầu–cuối**: `select` (bản ghi **ẩn danh**) đi **`to json`** (ST-53), **một
   comparator** cho shell `sort_by` và `std.list.sort_by` (ST-53), **codegen từ chối** HIR có chẩn
   đoán — ****`CODEGEN_TIN_TYCK` 21 → 0** (ST-54).
2. **Hai đường trùng ngữ nghĩa**: differential **100.000 / 0 lệch**; parity **344/344**; `VBC1` **53 tag**.
3. **730 test** workspace (21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 62 bytecode + 9 vm +
   **357 cli**); **370 ca / 18 nhóm** (327 `.vis` + 43 `.vish`, 52 `.err`).
4. **`vis test`** **370 ok**; `--vm` **370 ok / 6 SKIP**; `gate --fast` + `nightly` **PASS**; allowlist **6**.
5. **ViScript chưa hoàn thiện toàn bộ** — còn **10 nợ có chủ đích** + **VS2** (bảng tiêu chí đo ở §4).

## 2. Bảng vòng 9

| Task | Commit | Kết quả (1 dòng) |
|---|---|---|
| ST-53 | `d68eb16` | **`to json`** = stage lệnh (`Call(std.json.encode, [prev])`, `--pretty` ⇒ `encode_pretty`; **không** cần `use std.json`); bản ghi **ẩn danh** encode theo **trường + thứ tự**; **một** lõi `sap_xep_theo_khoa` (ổn định, `--desc`), `std.list.sort_by` giữ `FnIntInt`; **718 test** |
| ST-54 | `4627fbb` | codegen **không im lặng**: `compile_kiem(hir, symbols, text, chan_doan)` ⇒ `Err(ChanDoan{short,dong,cot,msg})` khi có lỗi front end; `HExpr::Error` ⇒ `Unsupported("error-expr")`; **`CODEGEN_TIN_TYCK` 21 → 0**; **730 test** |
| ST-55 | (commit này) | báo cáo **R9** + **kế hoạch VS2 có tiêu chí đo**; `report-check --all` PASS |

## 3. Số liệu chạy lại (mã `4627fbb`, đo 2026-11-13/14)

| Phép đo | Kết quả |
|---|---|
| `cargo test --workspace` | **730** = 21 + 96 + 18 + 143 + 24 + **62** + 9 + **357** |
| Ca ngữ nghĩa | **370 / 18 nhóm** (327 `.vis` + 43 `.vish`; 52 `.err`) |
| `vis test` / `--vm` | **370 ok** / **370 ok, 6 SKIP, 0 đỏ** |
| Parity (26 ca lỗi biên dịch) | **344/344** |
| Differential | **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=213.2 toc_do=469/s seed=3523158054`** |
| Fuzz parser (`nightly --fuzz-minutes 1`) | **2.996.353 ca / 68.123.752 token**, 49.939 ca/s, 1.135.396 token/s, 60,0 s, **0 vi phạm** |
| `VBC1` | **53 tag** |
| Sổ mã | **7 sổ / 29 mã** (`VS0405` ở sổ **kiểu**, `06xx` 4) |
| **`CODEGEN_TIN_TYCK`** | **0** (26/26 ca lỗi biên dịch bị `compile_kiem` **từ chối**, so đủ mã/dòng/cột/message) |
| Khởi động ấm (15 mẫu, debug) | interp **11,12 ms**, vm+cache **11,27 ms**, vm không cache **11,17 ms** |
| Allowlist doc-examples | **6** (18 xanh + 6 ngoại lệ) |
| Cổng | `gate --fast` **PASS** (sau khi ghi báo cáo); `nightly` **PASS** (`2026-11-13T17:13:27Z`) |

## 4. §19 — VSH2 đạt gì + **kế hoạch VS2 có tiêu chí đo**

**VSH2 đạt**: (i) `select`→`to json` (nợ lõi #1) với bản ghi ẩn danh đúng khoá/thứ tự, không cần
`Type::Any`; (ii) **một** comparator dùng chung (`sap_xep_theo_khoa`) — ổn định, `--desc` chỉ ở shell,
hợp đồng cũ `std.list.sort_by` giữ nguyên; (iii) **codegen không im lặng** — `ChanDoan` đủ mã +
dòng:cột + thông điệp, `CODEGEN_TIN_TYCK` **0**; (iv) 730 test, parity/diff giữ nguyên.

**Kế hoạch VS2 (ước lượng `[ƯL]`, mỗi mục có tiêu chí đo được)**

| # | Hạng mục VS2 | Tiêu chí đo được (cái gì chạy · đo bằng gì) | Ước lượng |
|---|---|---|---|
| 1 | **Async có cấu trúc** | `async fn` + `await` + nhóm task; **≥20 ca sem** chạy interp = `--vm` **cùng byte**; 1 benchmark 10.000 task không tràn | 3–4 tuần |
| 2 | **Kênh** | `chan[T]` `send/recv/close`; **≥15 ca** + 1 ca **deadlock** phát hiện được (chẩn đoán rõ, không treo) | 2–3 tuần |
| 3 | **Net/HTTP** | TCP client/server nội bộ + `http.get` tới server test; **≥10 ca e2e** + ca timeout/refused | 3 tuần |
| 4 | **Quyền + Landlock thật** | Trên Linux ≥6.x: tiến trình con **bị chặn ghi** ngoài allowlist (`exit ≠ 0` + errno); **≥8 ca** (cho phép/không) | 2 tuần |
| 5 | **LSP** | `vis lsp` trả `initialize`/`completion`/`hover`/`diagnostics`; **≥10 phản hồi** so snapshot JSON-RPC | 3–4 tuần |
| 6 | **Gói + registry** | `vis pkg add` tải từ registry **local**, khoá SHA-256 + lockfile; cài lại **tất định**; **10 ca + 1 e2e** | 4 tuần |
| 7 | **vis-host-windows/wasm** | Cùng **30 ca lõi** chạy trên Windows CI + `wasm32-wasi`, 0 lệch hành vi | 4–6 tuần |
| 8 | **Bộ hợp chuẩn Host 4 nền tảng** | Suite `host-conformance` **≥50 ca × 4 nền** (Linux/macOS/Windows/wasm), **0 lệch** | 2 tuần + CI |

**ViScript chưa hoàn thiện toàn bộ**: 8 hạng mục trên **chưa có** (VS2), cộng **10 nợ có chủ đích**
ở §5 — không có mục nào đang giả vờ "xong".

## 5. Lỗ hổng đã biết — cập nhật đủ

**Đã đóng ở vòng 9**

1. `select` (bản ghi ẩn danh) → **`to json`** (ST-53).
2. **Hợp nhất comparator** hai `sort_by` (ST-53) — còn **nợ** khoá `str` cho `std.list.sort_by`.
3. **`CODEGEN_TIN_TYCK = 0`** + codegen **từ chối** HIR có chẩn đoán (ST-54).

**Còn mở (nợ có chủ đích)**

1. Khoá `str` cho `std.list.sort_by` (giữ `FnIntInt`; cần task riêng).
2. `Ty::Any` (nợ generic VS1c).
3. `&self` / `ref` (VS1 — `HExpr::Ref` vẫn `Unsupported` ở codegen).
4. Mồ côi **đa tệp** (orphan rule mới xét một tệp).
5. **Siêu trait** (`trait` chưa kiểm).
6. `derives` **sinh mã** (hiện chưa).
7. `Hop[Hop[int]]` (generic lồng).
8. Thân `cmd` **chưa dùng tham số** của `cmd` trong pipeline.
9. Body "**vừa script vừa lệnh**" ⇒ `VS0201` (phân loại hẹp).
10. Nhóm chỉ **một** pipeline + kiểu tĩnh lấy **stage đầu**; biến `x | …` chưa nhận; `VS0403` còn
    nghĩa rộng; `where` chưa kiểm kiểu tham số khi lambda không chú thích.

## 6. VD-86..VD-88 (mỗi quyết định một dòng)

- **VD-86** — Phạm vi vòng 9 (**VSH2**): bản ghi ẩn danh đi JSON; **một** `sort_by`; `CODEGEN_TIN_TYCK`
  phải giảm hoặc chứng minh; VS2 phải có **tiêu chí đo** trong R9.
- **VD-87** — `to json` + hợp nhất `sort_by` (ST-53): `to json` là **stage lệnh** (`std.json.encode`,
  `--pretty` ⇒ `encode_pretty`, không cần `use`); **một comparator** `sap_xep_theo_khoa` (khoá 1 lần,
  ổn định, `desc` đảo comparator); nợ `str` cho std.
- **VD-88** — Codegen từ chối HIR có chẩn đoán (ST-54): **không** soi `Type::Error` (HIR sạch kiểu);
  cửa `compile_kiem` + `ChanDoan{short,dong,cot,msg}`; `HExpr::Error` ⇒ `Unsupported`; chỉ chặn
  `Severity::Error`; `CODEGEN_TIN_TYCK` **0**.

## 7. Ghi chú máy kiểm + đề xuất

- `report-check --all`: **PASS** (11 khoá, **61 hàng QUEUE** phủ đủ).
- **Đề xuất AGENTS §8**: thêm câu R9 — **730 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity
  **344/344**, differential **100k 0 lệch**, `VBC1` 53 tag, **7 sổ/29 mã**, **`CODEGEN_TIN_TYCK = 0`**,
  allowlist **6**, **VSH2 DONE**; còn **10 nợ có chủ đích + VS2**, **ViScript chưa hoàn thiện toàn bộ**.
- **Đề xuất `viscript/AGENTS.md`**: đồng bộ dòng trạng thái trên; ghi rõ vòng 10 mở.
- **Đề xuất hàng QUEUE vòng 10**: `[ST-00]` lật ST-55 `DONE` + mở vòng 10; **ST-56** đóng nợ lõi còn
  lại (tách 2–3 task ≤3 h: khoá `str` cho `std.list.sort_by`; `Ty::Any`/`&self`/`ref`; mồ côi đa tệp/
  siêu trait/derives sinh mã/`Hop[Hop[int]]`); **ST-57** miếng **VS2 đầu tiên** (async có cấu trúc +
  kênh) với đúng tiêu chí đo ở §4; **ST-58** R10. Đề nghị **không** mở thêm văn phạm (biến `x | …`,
  bọc-list giữa pipeline) — giữ nợ có chủ đích.
