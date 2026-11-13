<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R8-viscript — báo cáo vòng 8 (chốt **VSH1c**), dòng ViScript

Agent 2 · **2026-11-14** · `head` = **`a44c357`** (QUEUE@head có đủ **58 hàng**, 6 cột; ST-48..ST-51
`DONE`, ST-52 `READY` báo cáo này; **lưu ý**: hàng ST-51 trong QUEUE@`a44c357` vẫn ghi `BLOCKED` dù
commit lật `a44c357` nói DONE — xem §7). Số đo chạy trên **mã của `e721858`**. Bản máy đọc:
[`R8-viscript.json`](R8-viscript.json) — **đúng 11 khoá** Khối E (`project = "viscript"`, `round = 8`).

## 1. Tóm tắt (5 dòng)

1. **VSH1c chạy được đầu–cuối**: thân `cmd` bằng **pipeline lệnh** (ST-48), nhóm `( … )` có **một
   đường hạ** + cờ + nhiều dòng (ST-49), **một mã** field-access đúng sổ `VS0405` + `where` kiểm
   **tĩnh** cho lambda trong ngoặc (ST-50), **stage đầu là biểu thức đóng** `[1,2] | take 1` (ST-51).
2. **Hai đường trùng ngữ nghĩa**: differential **100.000 chương trình / 0 lệch**; parity **344/344**;
   `VBC1` **53 tag** không đổi.
3. **704 test** workspace (**21** diag + **96** syntax + **18** fmt + **143** types + **24** interp +
   **50** bytecode + **9** vm + **343** cli); **370 ca ngữ nghĩa / 18 nhóm** (**327** `.vis` + **43** `.vish`, **52** `.err`).
4. **`vis test`**: TAP `1..370` — **370 ok**; `vis test --vm` = **370 ok / 6 SKIP / 0 đỏ**
   (SKIP = ca cần `--fs-mock`/host thật, không phải nợ ngữ nghĩa); `gate --fast` và `nightly` **PASS**.
5. **Bảy sổ / 29 mã**; `VS0405` (không có trường trên bản ghi) nằm ở **sổ kiểu `04xx`**; allowlist
   doc-examples giữ **6**. **ViScript chưa hoàn thiện toàn bộ** — xem §5/§6.

## 2. Bảng vòng 8

| Task | Commit | Kết quả (1 dòng) |
|---|---|---|
| ST-48 | `739fc3e` | thân `cmd` = **pipeline lệnh** (khối thụt thử `Mode::Script`, không sạch ⇒ lệnh; con `Lowerer` `lower_than`); **666 test**, interp ↔ vm cùng byte |
| ST-49 | `e0c084a` | nhóm `( … )`: **một** đường hạ (`lower_nhom`), nhóm-lệnh **có cờ** (`--long`) + **nhiều dòng**; **680 test** |
| ST-50 | `da50f28` | một mã đúng sổ: `VS0605` → **`VS0405`** (`04xx`), `06xx` **5→4**, `04xx` **4→5**, `all_codes` **29**; `where ( x => x )` kiểm **tĩnh**; `tyck/truong.rs` (R1); **692 test** |
| ST-51 | `e721858` | **stage đầu là biểu thức đóng** (`[1,2] | take 1`), nới **hẹp nhất**, `{…}` bọc list 1 phần tử; **704 test** |
| ST-52 | (commit này) | báo cáo **R8** — số liệu tái lập, `report-check --all` PASS |

## 3. Số liệu chạy lại (mã `e721858`, đo 2026-11-13/14)

| Phép đo | Kết quả |
|---|---|
| `cargo test --workspace` | **704** = 21 + 96 + 18 + 143 + 24 + 50 + 9 + **343** |
| Ca ngữ nghĩa | **370 / 18 nhóm** (**327** `.vis` + **43** `.vish`; **52** `.err`) |
| `vis test` / `--vm` | **370 ok** / **370 ok, 6 SKIP, 0 đỏ** |
| Parity (26 ca lỗi biên dịch) | **344/344** |
| Differential | **`tong=100000 loi_tinh=0 chay=100000 lech=0 giay=211.5 toc_do=473/s seed=3523158054`** |
| Fuzz parser (`nightly --fuzz-minutes 1`) | **3.023.954 ca / 68.748.686 token**, 50.399 ca/s, 1.145.811 token/s, 60,0 s, **0 vi phạm** |
| `VBC1` | **53 tag** (không đổi từ R7) |
| Sổ mã | **7 sổ / 29 mã**; `04xx` **5** (`VS0405` ở sổ **kiểu**), `06xx` **4** |
| Khởi động ấm (15 mẫu, debug) | interp **10,87 ms**, vm+cache **10,70 ms**, vm không cache **10,96 ms** |
| Allowlist doc-examples | **6** |
| Cổng | `gate --fast` **PASS** (sau khi ghi báo cáo); `nightly` **PASS** (`2026-11-13T16:14:11Z`) |

## 4. §19 — VSH1c đạt gì

- **Thân `cmd` là lệnh thật**: `cmd f() -> list[FileEntry]:` + khối `ls | where …` chạy được **cả**
  đường thông dịch và VM, chẩn đoán `dòng:cột` tuyệt đối; thân **script** cũ vẫn nguyên.
- **Nhóm lệnh trọn vẹn**: `( ls --long )`, `( ls | where … )`, nhóm **nhiều dòng**, `( … )` làm **đối số**
  và làm **stage**; **một** đường hạ nhóm duy nhất ⇒ hết lệch "hai lối".
- **Chẩn đoán đúng sổ**: trường không có ⇒ **`VS0405`** ở sổ **kiểu** (không phải sổ lệnh), câu
  vi/en/ja trung tính dùng cho cả tyck + hạ; `VS0403` giữ 6 ngữ cảnh **khai báo**.
- **Tĩnh hơn**: `where ( x => 1 )` ⇒ `VS0412`; `where ( x => x.khong_co )` ⇒ `VS0405` **trước khi chạy**.
- **Stage đầu biểu thức**: `[1,2] | take 1`, `{…} | where …`, `"a b" | lines` — không cần ngoặc, **không**
  phá §9.5 (không nhận số/biến/tên lệnh; không `|` vẫn `VS0201`).
- **Chưa hoàn thiện toàn bộ ViScript**: khoảng cách còn lại là **9 nợ lõi** + **VS2** (async/kênh/net/
  Landlock thật/LSP/gói/registry/4 nền tảng) — xem §5.

## 5. Lỗ hổng đã biết — cập nhật đủ

**Đã đóng ở vòng 8**

1. Thân `cmd` **pipeline lệnh** (ST-48) — VSH1b-2.
2. Nhóm-lệnh **có cờ** + nhóm **nhiều dòng** + **một** đường hạ nhóm (ST-49).
3. **Một mã** field-access đúng sổ (`VS0405`, sổ kiểu) (ST-50).
4. `where ( x => x )` kiểm **tĩnh** (kể cả lambda trong ngoặc) (ST-50).
5. **Stage đầu là biểu thức** đóng (ST-51) — nợ VSH1b-2 còn lại.

**Còn mở**

1. Thân `cmd` **chưa dùng tham số** của `cmd` trong pipeline (tham số chỉ có ở khai báo).
2. Thân `cmd` "vừa script vừa lệnh" (lẫn let + dòng lệnh) ⇒ `VS0201` (phân loại **hẹp**).
3. Nhóm chỉ nhận **một** pipeline; kiểu tĩnh của nhóm lấy **stage đầu**.
4. **Biến** `x | …` chưa nhận (giữ lệnh ngoài; nhập nhằng §9.5).
5. Bọc-list chỉ cho **stage đầu** (`{…}` giữa pipeline không có).
6. `VS0403` còn nghĩa rộng (trường/kiểu/phương thức ở ngữ cảnh **khai báo**).
7. `where` chưa kiểm **kiểu tham số** khi lambda **không chú thích** (dựa pty ở hạ).
8. **Nợ lõi (9)**: `select` chưa `to json`; **hai** `sort_by`; `CODEGEN_TIN_TYCK = 21`; `Ty::Any`;
   `&self`; mồ côi **đa tệp**; **siêu trait**; `derives` **sinh mã**; `Hop[Hop[int]]`.
9. **VS2**: async/kênh/net/**Landlock thật**/LSP/gói/**registry**/4 nền tảng — **khoảng cách lớn nhất**.

## 6. VD-81..VD-85 (mỗi quyết định một dòng)

- **VD-81** — Phạm vi vòng 8 (**VSH1c**): thân `cmd` pipeline, nhóm-lệnh/cờ/nhiều dòng, một mã
  field-access, `where` tĩnh trong ngoặc, stage đầu là biểu thức; giữ "hẹp nhất", không phá §9.5.
- **VD-82** — Thân `cmd` bằng pipeline (ST-48): thử `Mode::Script` trên thân bỏ thụt; không sạch ⇒ lệnh;
  con `Lowerer` với `ThanCtx`, thay `HItem::Fn.body`.
- **VD-83** — Nhóm-lệnh + nhóm nhiều dòng (ST-49): **một** đường hạ nhóm (`lower_nhom`), pipe trong
  ngoặc không bị cắt dòng, `kieu_stage` lấy stage đầu; `lower/than.rs`.
- **VD-84** — Trường là lỗi kiểu ⇒ **`VS0405`** (ST-50): đổi tên từ `VS0605`, chuyển sang sổ `04xx`;
  `06xx` 5→4, `04xx` 4→5, `all_codes` 29; `ma_truong()` tra theo **id**.
- **VD-85** — Stage đầu là biểu thức (ST-51): chỉ token đầu `[`/`{`/chuỗi + `|` độ sâu 0 + tiền tố
  Script sạch; `{…}` bọc list một phần tử; không nhận số/biến.

## 7. Ghi chú máy kiểm + đề xuất

- `report-check --all`: **PASS** (11 khoá, 58 hàng QUEUE phủ đủ). Vì QUEUE@`a44c357` còn ghi
  **ST-51 = BLOCKED**, JSON ghi `status: "DONE"` **kèm** `queue: "BLOCKED"` và `blocked[]` nêu câu hỏi
  — đúng luật "sổ chưa lật". **Đề nghị Agent 0 lật hàng ST-51 khi mở vòng 9.**
- **Đề xuất AGENTS §8**: thêm câu R8 — **704 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity
  **344/344**, differential **100k 0 lệch**, `VBC1` 53 tag, **7 sổ/29 mã** (`VS0405` sổ kiểu),
  allowlist **6**, **VSH1c DONE**; còn **9 nợ lõi + VS2**, **ViScript chưa hoàn thiện toàn bộ**.
- **Đề xuất `viscript/AGENTS.md`**: đồng bộ dòng trạng thái trên (rút gọn 2 dòng), ghi rõ vòng 9 mở.
- **Đề xuất hàng QUEUE vòng 9**: (1) **ST-53** — đóng **9 nợ lõi** (tách 2–3 task ≤3 h: `select`→`to json`
  + hợp nhất hai `sort_by`; `CODEGEN_TIN_TYCK`/`Ty::Any`/`&self`; mồ côi đa tệp/siêu trait/derives sinh
  mã/`Hop[Hop[int]]`); (2) **ST-54** — khung **VS2** (async/kênh/net/Landlock thật) có tiêu chí đo;
  (3) hàng `[ST-00]` lật ST-51 `DONE` + mở vòng 9; (4) cân nhắc đưa "biến `x | …`" và "bọc-list giữa
  pipeline" vào nợ **có chủ đích** thay vì mở văn phạm.
