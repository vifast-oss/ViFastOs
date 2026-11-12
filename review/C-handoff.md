# C — Review quy trình/bàn giao: ViFastOs v2 + ViScript → "Agent 2"

Người review: AI agent (delegated, review độc lập) · Ngày: 2026-11-12 · Repo: `/Users/mps/dvchd/ViFastOs` (nhánh `main`, HEAD `ca2eaf5`)
Vai: reviewer quy trình, **không** review nội dung kỹ thuật kernel/ngôn ngữ. Không sửa tệp nguồn. Không dùng web, không chạy QEMU/build.

**Ký hiệu dẫn chứng:** `V:` = `review/src/ViFastOs-v2-thiet-ke.md`, `S:` = `review/src/ViScript-thiet-ke.md`, `D:` = `review/src/ViFastOs-dossier.md`; số sau dấu `:` là **số dòng**. `soak.py:` = `scripts/soak.py` (repo thật).

---

## 0. Phán quyết

> **Chưa bàn giao được ngay — cần vá 5 chỗ bắt buộc (1 BLOCKER + 4 MAJOR) rồi mới gửi Agent 2.** Hai tài liệu *nhất quán về nội dung kỹ thuật* ở mức đủ dùng, nhưng **chưa đủ để bàn giao**: queue không tự chứa, ABI lõi thiếu đường thoát tiến trình (T04 sắp chốt), 3 task không thể "xong" (vòng phụ thuộc / phụ thuộc duyệt), và bàn giao chưa tạo trước thư mục + tệp điều khiển mà chính nó giả định.

Số đếm phát hiện: **1 BLOCKER · 13 MAJOR · 9 MINOR · 5 NIT**.
5 chỗ phải vá trước khi gửi (chi tiết ở §7):

1. **BLK-1** — ABI lõi không có thao tác `Exit`/thông báo thoát ⇒ T04 sẽ chốt ABI thiếu (`V:387-427`, `V:510`, `V:578`).
2. **MAJ-1** — QUEUE.md không tự chứa: 12 tham chiếu ra ngoài tệp (`V:953,954,955,963,966,967`, `S:843,844,852`).
3. **MAJ-2** — 3 task không thể đạt tiêu chí: T10 ↔ T13 vòng, T16 phụ thuộc "tất cả", T03 cần gen-check của T05.
4. **MAJ-3** — T01 không đo được như văn bản (script 3 luồng không có, N≠600, stale "commit r323", vượt ngân sách 3h, `pkill` giết QEMU nhánh khác).
5. **MAJ-9** — Bàn giao chưa tạo `v2/agent/`, `viscript/agent/`, `DECISIONS.md`, `STATE.md`; T02/VT01 lại là task tạo chúng ⇒ nguy cơ ghi đè đúng tệp vừa chép.

Ngoài ra phải quyết trước khi gửi (không phải lỗi tài liệu, là lỗ hổng kế hoạch): **thẩm quyền SPEC** (MAJ-7), **quy ước commit `[T]`/`[VT]`** (MAJ-10), **thiếu D-01..D-08 trong DECISIONS** (MAJ-8).

---

## 1. Tính tự chứa của hai queue (câu hỏi 1)

### 1.1 Tham chiếu mà người chỉ có `QUEUE.md` không thấy

| # | Trong queue | Tham chiếu tới | Người đọc QUEUE.md có thấy? | Sửa |
|---|---|---|---|---|
| 1 | `V:953` T02 "Khung workspace `v2/` **theo mục 8**" | `V:592-611` (cây crate + 6 luật cứng) | ❌ | Chép cây crate + 6 luật vào QUEUE (Phụ lục A) |
| 2 | `V:953` T02 "`v2/agent/` với **các tệp mục 11.2**" | `V:846-857` | ❌ | Bảng 6 tệp + ai ghi, chép vào QUEUE |
| 3 | `V:954` T03 "**Luật cấu trúc**" (không nêu số) | `V:613-619` | ❌ | Liệt kê 6 luật + số case âm từng luật |
| 4 | `V:955` T04 "đầy đủ **mục 4.20, 4.21**" | `V:381-429` | ❌ (nhưng đây là *nội dung* phải viết — chấp nhận được nếu QUEUE nói rõ "nguồn: V §4.20–4.21") | Ghi rõ "nguồn sự thật: V §4.20–4.21, không tự thêm/bớt" |
| 5 | `V:963` T12 "checker **1, 2, 3, 9**" | `V:716-728` (§9.6, 10 checker) | ❌ **thuần vô nghĩa** nếu không có §9.6 | Chép nguyên văn 10 checker vào QUEUE, gạch đầu dòng 1/2/3/9 |
| 6 | `V:966` T15 "`v2 repro`" | `V:757-768` (§9.13) | ❌ | Chép bảng 6 lệnh cổng |
| 7 | `V:967` T16 + `S:852` VT10 "Theo **mục 11.7**" | `V:901-907` | ❌ | Chép schema `R<n>.json` (8 khoá) vào QUEUE |
| 8 | Ngân sách | `V:887` (§11.5, 3h/task) — chỉ có **một dòng văn xuôi** ở `V:948` | ⚠️ v2: có 1 dòng; **ViScript: không có dòng nào** (`S:839-854`) | Thêm cột "ngân sách" vào bảng task, cả hai queue |
| 9 | "Định nghĩa xong" | `V:909-915` (§11.8) | ❌ | Chép 4 gạch đầu dòng §11.8 vào đầu QUEUE |
| 10 | Vòng lặp 8 bước + luật cấm | `V:859-881` (§11.3–11.4) | ❌ | Chép vào QUEUE (hoặc `AGENTS.md` — xem §6) |
| 11 | Lệnh cổng | `V:757-768`; `S:799-808` | ❌ | Ghi lệnh cổng **theo từng task** trong bảng |
| 12 | `S:832` "Dùng nguyên quy trình **mục 11 tài liệu ViFastOs**" | cả §11 của **tệp khác** | ❌ (và là tham chiếu **chéo tài liệu**) | Không được trỏ chéo; phải inline |

**Kết luận 1:** cả hai queue **không tự chứa**. Nặng hơn ở ViScript: `S:832` trỏ nguyên một chương của tài liệu *kia*, và `S:839-854` **không có ngân sách, không có lệnh cổng, không có định nghĩa "xong"**. Người đọc `viscript/agent/QUEUE.md` mà chỉ có tệp đó **không thể biết khi nào dừng task**.

**Cách sửa tối thiểu (đủ để tự chứa):** mỗi `QUEUE.md` phải có 5 khối inlined trước bảng task:
(a) **Luật chơi** — 8 bước vòng lặp + 9 điều cấm (`V:861-881`);
(b) **Ngân sách & dừng** — 3h/task, 2 lần kẹt cùng ý ⇒ BLOCKED, spec mâu thuẫn ⇒ BLOCKED (`V:885-892`);
(c) **Định nghĩa xong** — 4 điều `V:911-914`;
(d) **Lệnh cổng** — `v2 check|gate --fast|gate|nightly|repro|mutants` (`V:759-768`) / `vis-dev check|gate --fast|gate|nightly` (`S:801-808`);
(e) **Định dạng báo cáo** — `R<n>.md` 6 mục + `R<n>.json` 8 khoá (`V:903-905`), **kèm trường `project`** (xem MIN-5).

### 1.2 Tiêu chí "xong" có đo được không (26 task)

| Task | Tiêu chí (nguyên văn rút gọn) | Đo được? | Vấn đề / cách sửa |
|---|---|---|---|
| T01 `V:952` | "Báo cáo n, số treo, số `TIME MONOTONIC BAD`, BUILD_ID" | ❌ | Báo cáo ≠ tiêu chí đạt/không đạt; thiếu N, máy, thời lượng, cách chạy 3 luồng. Xem MAJ-3 |
| T02 `V:953` | "`cargo build` toàn workspace" | ⚠️ | Thiếu "bao nhiêu crate", "gồm `agent/` không", "không ghi đè QUEUE/DECISIONS" |
| T03 `V:954` | "Cổng chạy, có case âm cho từng luật" | ⚠️ | "từng luật" = 6 luật `V:614-619` nhưng T03 chỉ nêu 5; luật 2500 dòng & "code sinh trùng spec" không thuộc T03 ⇒ ghi rõ 5 luật + **5 case âm**, phần còn lại thuộc T05 |
| T04 `V:955` | "Gửi Agent 0 duyệt (**BLOCKED tới khi duyệt**)" | ❌ | "Xong" do **người ngoài** quyết; không ai ghi APPROVED ở đâu; xem MAJ-12 |
| T05 `V:956` | "Code sinh biên dịch; sửa tay bị cổng bắt" | ✅ | Thêm: 1 case âm sửa tay + `--check` sạch |
| T06 `V:957` | "Dùng được cho chính vòng này" | ❌ | Vô nghĩa. Sửa: "`next/start/done/block/report` chạy trên **2** project; `report` sinh JSON đúng 8 khoá; có test host" |
| T07 `V:958` | "Miri xanh; fuzz chuỗi place/get/destroy" | ⚠️ | Thiếu thời lượng/seed (đề nghị "fuzz 10 phút, ≥ 10⁵ thao tác") |
| T08 `V:959` | "Property test so Vec" | ⚠️ | Thiếu số seed (đề nghị "10.000 seed") |
| T09 `V:960` | "Test ép preempt **mọi bước**" | ✅ | Nên nêu "checker invariant chạy ở **mỗi** điểm preempt, ≥ 1 điểm/thao tác" |
| T10 `V:961` | "Property test so **mô hình cây**" | ❌ **vòng** | Mô hình CNode là T13 `V:964`, lại phụ thuộc T10. Xem MAJ-2 |
| T11 `V:962` | "Mọi cặp cấm bị từ chối" | ✅ | Nêu số cặp (bảng `ALLOWED` có bao nhiêu cặp) |
| T12 `V:963` | "Seed tái hiện được; dump trace" | ⚠️ | "checker 1,2,3,9" cần §9.6 inline; nêu "cùng seed ⇒ cùng trace hash" |
| T13 `V:964` | "Khớp core trên 10.000 seed" | ✅ | — |
| T14 `V:965` | "Giải mã dump từ sim" | ✅ | — |
| T15 `V:966` | "Thu nhỏ được một đột biến cố ý" | ✅ | Nêu "kích thước giảm ≥ 50%" để đo được |
| T16 `V:967` | "Theo mục 11.7" | ❌ | Phụ thuộc "tất cả" ⇒ không bao giờ xong khi T04 chờ. Xem MAJ-2 |
| VT01 `S:843` | "Cổng chạy" | ❌ | "Cổng" gồm 6 bước (`S:804`) nhưng chưa có test/crate nào; và `vis-host-vifast` có thể không build (MAJ-5) |
| VT02 `S:844` | "Gửi Agent 0 duyệt; không chặn VT03..VT09" | ❌ | Như T04 + mâu thuẫn thẩm quyền (MAJ-7) |
| VT03 `S:845` | "Snapshot test" | ✅ | — |
| VT04 `S:846` | "Test bảng + fuzz 10 phút" | ✅ | — |
| VT05 `S:847` | "In lại CST đúng nguồn; fuzz" | ⚠️ | Fuzz thiếu thời lượng. Thêm "1 giờ, 0 panic" (khớp `S:817` VS0) |
| VT06 `S:848` | "Hai chế độ ra cùng AST" | ✅ | — |
| VT07 `S:849` | "parse → fmt → parse bằng nhau" | ✅ | — |
| VT08 `S:850` | "Chạy được với backend giả" | ✅ | — |
| VT09 `S:851` | "≥ 100 tệp … Xanh" | ✅ | — |
| VT10 `S:852` | "Theo mục 11.7 **tài liệu ViFastOs**" | ⚠️ | Đo được *nếu* inline; nhưng là **tham chiếu chéo tài liệu** (MAJ-1) |

**Tổng:** 10/26 đo được ngay; 7/26 mơ hồ; **9/26 không đo được hoặc không thể đạt** (T01, T04, T06, T10, T16, VT01, VT02 + T03/T12 phụ thuộc tài liệu ngoài). Từ 6/26 trở lên là dấu hiệu queue được viết như *bản kế hoạch* chứ chưa phải *hợp đồng thi công*.

---

## 2. Thứ tự và phụ thuộc task (câu hỏi 2)

### 2.1 Bản đồ trạng thái khi T04 đang chờ duyệt

```
T01 (v1 soak) ────────────────────────────────┐  ← chạy được, nhưng xem MAJ-3
T02 (khung v2 + agent/) ─┬─ T03 (luật + cổng) ┤  ← T03 cần gen-check của T05 ⇒ MAJ-2
                         ├─ T06 (agentctl)    │
                         └─ T07 (objmem) ─ T08 (ilist) ─ T11 (ThreadState)
T04 (abi.toml) ⛔CHỜ DUYỆT
   └─ T05 (gen) ⛔
        ├─ T09 (untyped) ⛔ ─┬─ T10 (CNode) ⛔ ─┬─ T12 (scenario+sim) ⛔ ─ T14, T15 ⛔
        │                    │                 └─ T13 (model) ⛔
        └────────────────────┴──────────────────────────────────────────
T16 (R1) ⛔ phụ thuộc "tất cả"
```

| Nhóm | Task | Số |
|---|---|---|
| **Chạy được ngay** (không phụ thuộc T04) | T01, T02, T03\*, T06, T07, T08, T11 | **7** |
| **Chờ duyệt** | T04 | 1 |
| **Bị chặn bởi T04** (trực tiếp/gián tiếp) | T05, T09, T10, T12, T13, T14, T15 | **7** |
| **Không thể xong** trong vòng 1 | T16 (+T03\*, xem MAJ-2) | 1–2 |
| **Dòng ViScript** (độc lập) | VT01 … VT10 (VT02 không chặn) | **10** |

\* T03 phụ thuộc T02 về mặt khai báo nhưng **tiêu chí của nó cần `gen-check` của T05** (T05 nằm sau T04) ⇒ T03 có thể kẹt vô hạn ở "đang làm".

**Tổng công chạy được lúc bàn giao: 7 (lõi) + 10 (ViScript) = 17 task.** Đủ cho §11.6 (`V:896`: ≥8 task, ≥3 nhánh) — **nhưng chỉ khi tính cả dòng ViScript**, đúng như `V:969` dự đoán.

### 2.2 Số nhánh song song thật

| Nhánh | Nội dung | Điều kiện |
|---|---|---|
| A | T01 (soak v1) | **phải chạy một mình** (MAJ-3) — nếu chạy song song thì số đo vô hiệu |
| B | T02 → T03 | T03 kẹt nếu cổng đòi gen-check |
| C | T02 → T06 | độc lập |
| D | T02 → T07 → T08 → T11 | **không độc lập với T02** (khác với cách viết ở `V:969`) |
| E | VT01 → (VT03, VT04 → VT05 → {VT06, VT07}, VT08) → VT09 → VT10 | độc lập hoàn toàn với A–D |

⇒ **4 nhánh thực** (A, B/C, D, E) nhưng **B, C, D cùng khởi từ T02** ⇒ 2–3 giờ đầu là **chuỗi tuần tự**, không phải 3 nhánh. Nên ghi rõ trong QUEUE: "T02 là nút cổ chai; đừng lên kế hoạch qua đêm trước khi T02 xanh".

### 2.3 Task không thể xong vì phụ thuộc vòng

| Cặp | Dẫn chứng | Bản chất |
|---|---|---|
| **T10 ↔ T13** | `V:961` (T10 cần "mô hình cây"), `V:964` (T13 = `vifast-model` phần Untyped/CNode, phụ thuộc T10) | **Vòng thật ở mức tiêu chí.** T10 không thể chứng minh "khớp mô hình" trước khi mô hình tồn tại |
| **T03 → T05 → T04** | `V:954` (T03 làm `v2 gate --fast` khung), `V:762` (`gate --fast` **gồm** gen-check), `V:956` (gen-check do T05 làm) | T03 phải "xanh" một cổng có bước chưa tồn tại |
| **T16 → tất cả** | `V:967` | Không xong khi còn bất kỳ task BLOCKED ⇒ **R1 không tồn tại**; mà `V:898` lại yêu cầu "cuối phiên: `agentctl report`" |
| **VT10 → VT01..VT09** | `S:852` | Không vòng (cả 9 task chạy được), nhưng nếu VT02 bị trả lại thì VT05/VT06 phải làm lại (đã ghi nhận ở `S:854`) |

**Cách sửa (chọn 1 cho mỗi cặp):**
- T10: đổi tiêu chí thành "property test so với **mô hình cây tối giản viết trong `test/`** của chính T10; T13 sau đó so **core ↔ vifast-model**"; hoặc chuyển T13 lên trước T10 (T13 chỉ cần `abi.toml` + API CNode).
- T03: tách "khung cổng" (5 bước: fmt, clippy, luật cấu trúc, L0, L1) khỏi "gen-check" — gen-check thêm ở T05; tiêu chí T03 ghi rõ "5 bước, 5 case âm".
- T16: đổi thành "R1 phủ **các task đã mở**; task BLOCKED ghi vào `blocked[]` của JSON" ⇒ R1 luôn sinh được (`V:905` đã có khoá `blocked` — dùng nó).

### 2.4 T01 và môi trường x86_64 (câu hỏi trọng tâm)

| Vấn đề | Dẫn chứng | Hệ quả |
|---|---|---|
| T33a **chưa đóng**, tỉ lệ 0,3–0,5 %/boot khi soak song song 3 luồng | `D:1614-1616`, `D:107` ("Chặn mọi tuyên bố ổn định") | 750 boot ⇒ **kỳ vọng 2,2–3,8 treo**. Nếu tiêu chí ngầm là "0 treo" thì **bất khả thi** |
| Tiêu chí T33a là **600** boot, T01 là **750** | `D:1667` vs `V:952` | Hai con số cho **cùng một phép đo**; phải chọn một (hoặc nêu lý do: `V:751-755` cho n≥3/p ⇒ 750 ứng với p<0,4 %) |
| 600 boot ≈ "vài giờ máy" | `D:1671` | 750 boot > 3h ngân sách mặc định (`V:887`) ⇒ task **tự động bị dừng giữa đường** theo luật |
| Không có script chạy 3 luồng | `soak.py:84-114` (không có `--threads/--parallel`), `soak.py:132-170` (vòng lặp **tuần tự** theo arch), `soak.py:178-179` (settle 3 s) | Agent 2 phải **tự viết** hạ tầng đo — việc này **không** nằm trong mô tả T01 |
| `VIFAST_NO_RETRY=1` có trong `test-qemu.py` nhưng không có trong `soak.py` | `test-qemu.py:508-510` | Phải export env; nếu không, soak dùng retry mặc định 1 ⇒ số liệu sai |
| `pkill -f qemu-system` | `V:952`, `V:892` | Giết **mọi** QEMU, kể cả của nhánh khác (`V:899` cho phép 3 QEMU song song) ⇒ mất số đo/artifact của nhánh khác |
| Máy đo | `D:1499-1508` (tỉ lệ khác nhau giữa `mps` và `ps`), `D:2021` (D-06: soak trên máy `ps`) | T01 **không ghi máy** ⇒ số liệu không so sánh được, vi phạm `D:1510-1517` (luật đo) |
| "commit r323" | `V:952` bước 1 | **Đã xong**: r323 = `ca2eaf5` (parent `8077d71`); chưa có tag `v1-final` (`git tag -l` = `overnight-start`, `v0.1.0`). Bước 1 là **no-op/stale** |

**Kết luận 2.4:** T01 **chạy được nhưng không thể "xong" theo nghĩa đạt tiêu chí**, và **xung đột với chính luật vận hành** (ngân sách 3h, cấm làm sai số đo, cấm pkill bừa). Phải viết lại T01 thành 3 task con (xem §8, bước 5).

### 2.5 "Toàn bộ dòng ViScript VT01 tới VT09" có khớp queue ViScript?

**Không.** `V:969` viết "toàn bộ dòng ViScript **VT01 tới VT09**", còn queue ViScript có **VT10** (`S:852`, báo cáo R1). Hệ quả:
- Nếu Agent 2 hiểu theo `V:969` thì **VT10 không được làm** ⇒ thiếu R1 của dòng ViScript, trong khi `S:852` là task bắt buộc.
- Đồng thời `V:969` liệt kê task BLOCKED "T05, T09, T10, T12 tới T15" — **thiếu T13? không** (nằm trong "T12 tới T15"), nhưng **thiếu T16** (cũng BLOCKED) và **thiếu chính T04** (đang chờ duyệt, không phải BLOCKED). Danh sách không khớp bảng.

Sửa: `V:969` → "VT01 … **VT10**", và câu BLOCKED → "T04 chờ duyệt; T05, T09, T10, T12–T15 **và T16** sẽ BLOCKED".

---

## 3. Tích hợp giữa hai tài liệu (câu hỏi 3)

### 3.1 `V §7.3` — chỉ còn phần tích hợp

Nhất quán về **phân công**: `S:4` nói "V §7.3 chỉ mô tả phần tích hợp; mọi quyết định về ngôn ngữ nằm ở đây", `V:568` nói ngược lại "ngôn ngữ, VM, ViShell, lộ trình và kiểm thử nằm trong `ViScript-thiet-ke.md`". ✅ Không mâu thuẫn.

Nhưng 4 điểm lệch:

| # | Vấn đề | Dẫn chứng | Sửa |
|---|---|---|---|
| 1 | `term.vidl` được xếp mốc **"VS3 + G3"** — một ô "mốc" chứa **2 mốc** | `V:575` | Tách: "sinh ở VS3, dùng ở G3" hoặc "G3 (sau khi VS3 xong)" |
| 2 | ViTerm trong ViScript là **tuỳ chọn** ("giao thức hiển thị có cấu trúc **tuỳ chọn**"), trong ViFastOs là **bắt buộc** (`term.vidl`) | `S:726`, `S:473` vs `V:575` | Chọn một: bắt buộc ⇒ sửa `S:726`; tuỳ chọn ⇒ bỏ dòng `V:575` khỏi G3 |
| 3 | Chiều phụ thuộc workspace **ngược nhau** | `V:572` ("`viscript/` **dùng** `vifast-urt`") vs `V:610` ("**v2 chỉ phụ thuộc** `vis-host-vifast` từ VSH2") | Quy định 1 chiều: `viscript` **không** path-dep vào `v2`; `v2` trỏ vào `viscript` (hoặc ngược lại) — không cả hai |
| 4 | Yêu cầu từ ViScript lên OS nằm rải 2 nơi | `V:578` (Rust std, WaitAddr, trang thời gian, procsrv `kill/wait/list`) vs `S:578-579` (`Host::spawn`, "wait, kill, list_procs" chỉ là **comment** trong trait) | `S:579` phải liệt kê **chữ ký cụ thể** (`fn wait(&self, p: &ProcHandle) -> HResult<ExitStatus>` …) để U1 kiểm được |

### 3.2 `V §10.2` — lộ trình song song

| Mốc | Định nghĩa ở đâu | Có tiêu chí xong? | Nhất quán? |
|---|---|---|---|
| **U1** | `V:796` (nội dung + phụ thuộc K6) + `V:578` (5 yêu cầu) | ❌ **không có cột tiêu chí** (§10.2–10.5 chỉ có "Mốc/Nội dung/Phụ thuộc"; chỉ §10.1 có "Tiêu chí xong") | U1 = "Rust std + job control" ở `V:796`, nhưng `V:578` đòi 5 thứ (thread, time, fs, net, WaitAddr, trang thời gian, `kill/wait/list`) ⇒ **định nghĩa hẹp hơn yêu cầu** |
| **VSH2** | `V:797` (nội dung + phụ thuộc) + `V:572-574` (3 việc) + `S:821` (tiêu chí "thay shell tạm") | ⚠️ chỉ ở **tài liệu kia** | Nội dung khớp; **tiêu chí chỉ có ở `S:821`** ⇒ đọc V một mình không biết khi nào VSH2 xong |
| **VS3** | `V:798` (phụ thuộc **VSH2, G3**) vs `S:822` (phụ thuộc **VS2, G3**) | ⚠️ tiêu chí ở `S:822`: "ứng dụng desktop nhỏ viết bằng ViScript" — **đây là tiêu chí của G3**, không phải của VS3 | **Lệch cạnh phụ thuộc** (VSH2 vs VS2) và **trùng tiêu chí** với G3 (`V:817`) |
| **G3** | `V:817` (nội dung + G2) | ❌ **không có tiêu chí xong ở bất kỳ tài liệu nào** | Tiêu chí của G3 bị "mượn" làm tiêu chí VS3 (`S:822`) |

Ngoài ra: `G1→U1` (`V:815-816`) và `VS3→G3→G2→U1→K6` tạo chuỗi dài **hoàn toàn phụ thuộc dòng lõi**; câu "dòng ViScript là nhánh độc lập" (`V:896`, `S:900`) chỉ đúng **tới VS2** (`S:824`). Sau VS2, dòng ViScript **không còn nhánh độc lập** — nên ghi rõ để người điều phối không hứa hẹn sai.

### 3.3 `V §6.4` + `V §4.21` — job control không signal: ABI có đủ?

| Thao tác procsrv cần (`V:510`, `V:576`, `V:578`) | Có trong ABI `V:387-429`? | Ghi chú |
|---|---|---|
| **suspend** | ✅ `Tcb.Suspend` (`V:414`) | Nhưng **ngữ nghĩa chưa định nghĩa** khi TCB đang `BlockedOnRecv/Reply/Notification/Address` (`V:222-235`): Resume đưa về **Ready** hay về **trạng thái chờ cũ**? Sim lại **có** tiêm lỗi "Suspend khi đang chờ" (`V:714`) ⇒ hành vi được test mà không được đặc tả |
| **resume** | ✅ `Tcb.Resume` (`V:414`) | Như trên |
| **kill** | ⚠️ **không có method**; phải hiểu là `Revoke/Delete` cap TCB (`V:213-217`) + finalise (`V:195`) | Chấp nhận được **nếu** ghi rõ "kill = thu hồi cap TCB + finalise; procsrv là chủ object" — hiện **không** ghi |
| **wait** (chờ tiến trình thoát) | ❌ **không có gì**: `ThreadState` không có trạng thái "đã thoát" trừ `Dead` (`V:234`), và `Dead` chỉ đạt được khi **bị xoá** | Đây là **lỗ thật**: kernel không có sự kiện "thread kết thúc" ⇒ procsrv không biết ai thoát |
| **exit** (tiến trình tự kết thúc) | ❌ **không có syscall `Exit`** trong danh sách 0–13 (`V:391-406`) | v1 **có** `SyscallId::Exit = 34` (`crates/vifast-abi/src/lib.rs:1171`) — v2 **bỏ mà không thay thế** |
| **list** | ✅ user-space (procsrv tự giữ sổ) | OK |

⇒ **BLK-1**: bảng ABI `V:387-427` thiếu `Exit` (hoặc `Tcb.Exit`) và thiếu cơ chế **thông báo thoát** (bound notification / fault-style exit message). Hệ quả dây chuyền: `Host::spawn/wait/kill` (`S:578-579`) → ViShell `&`, `jobs`, `fg`, `bg`, `kill` (`S:721`) → Ctrl-C (`S:722`) → **U1/VSH2 không cài được mà không đổi ABI**, vi phạm **D-22** (`V:935`: "sau K0 chỉ thêm, không đổi"). Vì T04 chính là task chốt ABI, **không được duyệt T04 trước khi vá chỗ này**.

### 3.4 D-27 + SPDX

| Vấn đề | Dẫn chứng | Hệ quả |
|---|---|---|
| Luật SPDX viết **một cú pháp cho mọi tệp**: `// SPDX-License-Identifier: …` | `V:881`, lặp ở `S:658` | `.vis`, `.vish`, `vis.toml`, `.vidl`, `.toml` dùng `#`, không dùng `//` ⇒ cổng SPDX hoặc **bỏ sót** hoặc **báo sai**. Sửa: "Rust/asm: `//`; `.vis/.vish/.vidl/.toml: `#`" |
| Kiểm SPDX có trong cổng v2 nhưng **không có task riêng** ở ViScript | `V:954` (T03 nêu "header SPDX") vs `S:843` (VT01 **không** nêu) — trong khi `S:804` đưa "header SPDX" vào `vis-dev gate --fast` | VT01 phải dựng cổng SPDX mà không có trong tiêu chí ⇒ **luật có, task không** (xem MAJ-6) |
| Luật ViScript khác **không có cổng nào** | `S:658` ("tệp ≤ 1.500 dòng", "`unsafe` chỉ `vis-vm`/`vis-plugin`") vs `S:804` (gate: fmt, clippy, wasm32, SPDX, L0, L1) | 2 luật **không được thi hành**; v2 có T03 cho đúng việc này, ViScript **không có tương đương** |
| `LICENSE-MIT + LICENSE-APACHE` được giao cho T02/VT01 dù **repo đã có** | `V:953`, `S:843` vs `ls` (root có `LICENSE-MIT`, `LICENSE-APACHE`, `TRADEMARKS.md`) | Không rõ "một bản ở root" hay "mỗi workspace một bản"; **chưa có `LICENSE`** (không hậu tố), **chưa có `NOTICE`/`THIRD-PARTY`** dù D-27 (`V:940`) và `S:617` đòi kiểm giấy phép phụ thuộc |
| `TRADEMARKS.md` không được nhắc ở bất kỳ tài liệu nào | `ls`, không có trong V/S | Agent 2 không biết chính sách nhãn hiệu tồn tại |

### 3.5 T16 vs VT10 (báo cáo R1)

| Vấn đề | Dẫn chứng | Sửa |
|---|---|---|
| Hai báo cáo **cùng tên `R1`** ở hai dòng | `V:967`, `S:852` | Đặt `R1-v2` / `R1-viscript`, hoặc thêm trường `project` vào `R<n>.json` (`V:905` hiện **không có**) |
| `R<n>.json` không có trường nhận dạng dòng | `V:905` (8 khoá: round, head, build_id, tasks, gates, sim, coverage, mutants, perf, blocked) | Thêm `project: "v2"|"viscript"` (**bắt buộc** nếu agentctl dùng chung) |
| T16 không thể xong (phụ thuộc `tất cả`) | `V:967` | Xem MAJ-2 |
| Đụng quy ước báo cáo **đã có** trong repo: `docs/agent2-report-<N>.md` (101 tệp) + `docs/overnight-log.md` + `docs/agent0-inbox.md` | `ls docs` | Nói rõ: `v2/agent/reports/R<n>` **thay thế** `docs/agent2-report-*` cho dòng v2 (và `docs/decisions/log.md` chỉ còn cho v1) |

### 3.6 Mốc VSH2/U1/VS3/G3 — tổng hợp

- **Định nghĩa:** `U1` `V:796`+`V:578`; `VSH2` `V:797`+`V:572-574`+`S:821`; `VS3` `V:798`+`S:822`; `G3` `V:817`.
- **Nhất quán:** nội dung khớp; **cạnh phụ thuộc VS3 lệch** (`VSH2,G3` vs `VS2,G3`); **G3 không có tiêu chí**; **U1 không có tiêu chí**; **tiêu chí VS3 là của G3**; **VSH2 chỉ có tiêu chí ở tài liệu kia**.
- **Cách sửa:** thêm cột "Tiêu chí xong" cho §10.2–10.5 (hoặc gom một bảng mốc duy nhất ở V §10.7), và ở mỗi mốc ghi "định nghĩa tại: S §19" thay vì để lửng.

---

## 4. Tệp/thư mục mà bàn giao giả định nhưng chưa có (câu hỏi 4)

Bằng chứng: `ls -1` root, `ls -1 review`, `git status --short` (chỉ `?? ViFastOs-dossier.md`, `?? review/`), `git tag -l`, `git check-ignore`.

| # | Giả định | Thực tế | Ai tạo? | Ghi chú |
|---|---|---|---|---|
| 1 | `v2/agent/QUEUE.md` + 5 tệp khác (`V:846-855`, `V:608`) | ❌ **không có `v2/`** | **Không ai** ở bước bàn giao; T02 (`V:953`) "tạo `v2/agent/` với các tệp mục 11.2" | **Chicken-and-egg**: QUEUE/DECISIONS phải có **trước** T02, nhưng T02 là task tạo chúng ⇒ T02 có thể **ghi đè** đúng tệp điều khiển (MAJ-9) |
| 2 | `viscript/agent/` (`S:834`) | ❌ không có | VT01 (`S:843`) | Như trên |
| 3 | `v2/agent/DECISIONS.md` (kế hoạch bước 3) | ❌ không có, **không task nào** tạo | **Không ai** | Phải do người điều phối tạo trước; nội dung = `V §12` (**D-09…D-27**) + **D-01…D-08** (xem MAJ-8) |
| 4 | `viscript/agent/DECISIONS.md` | ❌ không có | **Không ai** | = `S §22` (VD-01…VD-10) |
| 5 | `STATE.md` khởi tạo (`V:851`) | ❌ không có, không ai tạo | **Không ai** | Không có STATE.md thì bước 1 của `V:861` ("đọc STATE.md") **thất bại ngay** |
| 6 | `JOURNAL/`, `reports/`, `BLOCKED.md` | ❌ | T02/VT01 (nếu đọc kỹ) | Nhưng T02 có thể tạo **rỗng** ⇒ mất nội dung đã chép |
| 7 | Chỗ đặt **hai tài liệu thiết kế** | ⚠️ `review/src/*.md` là **untracked** (`git status`: `?? review/`) | **Không ai** | Nếu không commit + không có đường dẫn ổn định thì mọi tham chiếu "mục 8/mục 11.7" trong QUEUE **treo**; `review/` còn mang nghĩa "nháp" |
| 8 | `LICENSE`, `NOTICE`, `THIRD-PARTY` | ⚠️ có `LICENSE-MIT`, `LICENSE-APACHE`, `TRADEMARKS.md`; **thiếu `LICENSE`, `NOTICE`, `THIRD-PARTY`** | T02/VT01 (mơ hồ) | Xem §3.4 |
| 9 | `.gitignore` cho `v2/`/`viscript/` | ✅ **đã đủ** — `target/`, `artifacts/`, `*.serial` không có `/` đầu nên áp dụng mọi cấp | **Không cần tạo** | Kiểm: `git check-ignore v2/target/debug/x` → IGNORED; `v2/agent/QUEUE.md` → tracked-ok. Chỉ nên thêm `**/*.vbc` nếu cache bytecode đặt trong cây |
| 10 | Quy ước commit `[T<id>]` (`V:866`) vs `[VT<id>]` | ⚠️ `V:866` chỉ định nghĩa `[T<id>]`; `S:832-837` liệt kê "điểm riêng" **không có** commit tag | **Không ai** | Lịch sử repo còn dùng **kiểu thứ ba**: `r323 (T33a): …` (`git log`). ID `T26/T31/T33a` (bug v1) **trùng không gian** với task `T##` |
| 11 | `vifast-agentctl --project` (`V:857`, `S:834`) | ⚠️ chỉ **một dòng**; tool nằm ở `v2/tools/vifast-agentctl` (`V:595`), do T06 (= sau T02) tạo | T06 | **Ngữ nghĩa `--project` chưa định nghĩa** (đường dẫn tệp, thư mục report, lọc commit theo regex nào). Dòng ViScript **không có công cụ** cho tới khi T06 xong |
| 12 | `v2` / `vis-dev` là lệnh gì | ⚠️ `V:759`, `S:803` dùng như lệnh; **không nói** là xtask, script, hay alias `.cargo/config.toml` | T03/VT01 | Ảnh hưởng trực tiếp tiêu chí "cổng chạy" |
| 13 | `AGENTS.md` / "hai tài liệu là nguồn sự thật" | ❌ repo **không có** `AGENTS.md`, `CLAUDE.md`… ; `README.md` và `docs/README.md` **không** trỏ tới tài liệu v2/ViScript | **Không ai** | Xem §6 |
| 14 | Tag `v1-final` | ❌ chưa có (`git tag -l`) | T01 | Nhưng T01 phải tag **`ca2eaf5`** (r323 đã commit) — xem MAJ-3 |

---

## 5. Các bước bàn giao còn thiếu (câu hỏi 5)

Kế hoạch 4 bước hiện tại (gửi 2 tệp → chép §13/§20.2 → chép §12/§22 → thông báo T04/VT02) **thiếu 12 bước**. Xếp theo thứ tự thời gian:

1. **Chốt vị trí tài liệu trong repo** (commit `docs/v2/…`, `docs/viscript/…` hoặc `v2/docs/`…), không để `review/` untracked.
2. **Vá tài liệu trước khi gửi** (hoặc phát hành `ERRATA.md`): BLK-1, MAJ-1..MAJ-3, MAJ-5..MAJ-9 — hoặc chấp nhận và ghi rõ "biết là thiếu, sẽ xử lý ở vòng 2".
3. **Tạo trước thư mục + tệp điều khiển** (người điều phối, không phải Agent 2): `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `v2/agent/{JOURNAL,reports}/`, `viscript/agent/…` tương tự.
4. **`QUEUE.md` tự chứa**: chép §13/§20.2 **+ 5 khối inlined** ở §1.1 (luật, ngân sách, DoD, cổng, định dạng báo cáo).
5. **`DECISIONS.md` đầy đủ**: v2 = **D-01…D-08** (từ `D:2012-2023`, "vẫn hiệu lực") **+ D-09…D-27** (`V:918-942`); ViScript = VD-01…VD-10 (`S:875-886`).
6. **`STATE.md` khởi tạo**: HEAD `ca2eaf5`, BUILD_ID (nếu có), "task hiện tại: chưa", "cổng gần nhất: —", "việc dở: —".
7. **`BLOCKED.md` khởi tạo**: `T04 — chờ Agent 0 duyệt abi.toml (kèm BLK-1)`; `VT02 — chờ duyệt SPEC.md, KHÔNG chặn VT03..VT09`.
8. **`AGENTS.md`** ở root + `v2/` + `viscript/` (nội dung tối thiểu ở §9.2) — đây là chỗ duy nhất ghi "hai tài liệu này là nguồn sự thật **thiết kế**; `spec/` là nguồn sự thật **ABI**; code+harness là nguồn cuối khi lệch".
9. **Cập nhật `README.md` + `docs/README.md`** trỏ tới 2 tài liệu (và ghi rõ chúng thuộc loại "thiết kế tham chiếu" hay "sống").
10. **Quyết định thẩm quyền SPEC** (MAJ-7) và **quy ước commit/tag** (MAJ-10) — ghi vào `DECISIONS.md`.
11. **Chỉ định người/điều kiện duyệt** T04 & VT02 + mẫu ghi nhận: `APPROVED <ngày> bởi Agent 0 — ghi D-28` (để `agentctl` và Agent 2 đọc được máy).
12. **Gửi + yêu cầu ack**: Agent 2 trả lời 3 câu (task đầu tiên, task bị chặn, câu hỏi) **bằng cách ghi `STATE.md` + `BLOCKED.md`**, không chỉ bằng chat.

Ngoài ra (không bắt buộc cho vòng 1, nhưng phải ghi để không hứa sai): `.gitignore` **không cần** tạo mới; `LICENSE/NOTICE/THIRD-PARTY` cần quyết định; báo cáo `R1` cần phân biệt hai dòng.

---

## 6. Danh sách phát hiện đầy đủ

### 6.1 BLOCKER

**BLK-1 — ABI lõi không có đường "thoát tiến trình" ⇒ T04 chốt ABI thiếu (chặn T04, không chặn T01/T02/T07/T08/T11/VT\*)**
- Dẫn chứng: `V:391-406` (14 syscall, **không có `Exit`**), `V:414` (`Tcb.*` không có `Exit`/`Terminate`/`Wait`), `V:222-235` (`Dead` chỉ đạt khi bị xoá), `V:510` ("procsrv … theo dõi thoát; cung cấp `wait`, `kill`"), `V:578` ("procsrv có `kill/wait/list`. Tất cả nằm trong U1"), `S:578-579` (`Host::spawn` + "wait, kill, list_procs" chỉ là comment), `S:721` (ViShell `&`, `jobs`, `fg`, `bg`, `kill`), v1 có `SyscallId::Exit = 34` (`crates/vifast-abi/src/lib.rs:1171`).
- Hệ quả: (a) procsrv **không biết** tiến trình đã thoát ⇒ `wait` không cài được; (b) kill phải là "thu hồi cap + finalise" nhưng **không được đặc tả**; (c) nếu thêm `Exit` **sau** K0 thì vi phạm **D-22** (`V:935`).
- Cách sửa (chọn 1, ghi vào V §4.21 + §6.4):
  1. Thêm syscall **14 `Exit(code)`** (giống v1) + quy định: procsrv bind `Notification`/`fault_ep` cho từng tiến trình để nhận sự kiện thoát; hoặc
  2. Thêm method **`Tcb.Exit`** + `Tcb.BindExit` (notification nhận `{tid, code}`); hoặc
  3. Quy định **rõ** "exit là giao thức user-space: tiến trình `Call(procsrv.exit_ep)`; procsrv finalise TCB" + thêm `procsrv` vào §6.1 IDL + sửa `S:579` thành chữ ký cụ thể.
- Kèm: đặc tả **ngữ nghĩa `Suspend` khi TCB đang chờ** (Ready vs trạng thái chờ cũ) — hiện được test ở `V:714` mà không được định nghĩa.

### 6.2 MAJOR

| Mã | Phát hiện | Dẫn chứng | Cách sửa |
|---|---|---|---|
| **MAJ-1** | QUEUE không tự chứa (12 tham chiếu ngoài; ViScript queue thiếu cả ngân sách/DoD/cổng, và trỏ sang **tài liệu khác**) | `V:953,954,955,963,966,967`; `S:832,843,844,852`; §1.1 | Inline 5 khối ở §1.1 vào mỗi QUEUE.md |
| **MAJ-2** | 3 task không thể xong: T10↔T13 (vòng mức tiêu chí), T03 cần gen-check của T05, T16 phụ thuộc "tất cả" | `V:961,964`; `V:954,762,956`; `V:967` | Đổi tiêu chí T10 (mô hình tối giản trong test) hoặc đảo T13 trước T10; tách "khung cổng" khỏi gen-check ở T03; T16 = "R1 phủ task đã mở + `blocked[]`" |
| **MAJ-3** | T01: (a) bước "commit r323" **đã xong** (`ca2eaf5`); (b) 750 ≠ 600 cho cùng phép đo; (c) không có script 3 luồng (soak.py tuần tự); (d) > 3 h ngân sách; (e) `pkill -f qemu-system` giết QEMU nhánh khác; (f) không ghi máy; (g) "0 treo" không có lực thống kê (3/750 = 0,4 % = đúng tỉ lệ hiện tại) | `V:952`; `D:1667`; `D:1671`; `soak.py:84-114,132-179`; `test-qemu.py:508-510`; `V:899`; `D:1499-1508`; `D:2021`; `V:751-755` | Chia T01 thành **T01a** (dựng `tools/soak3.sh` chạy 3 tiến trình, `VIFAST_NO_RETRY=1`, ghi `machine`, `BUILD_ID`, N) + **T01b** (chạy N=600, báo tỉ lệ + CI 3/N) + **T01c** (quyết định tag `v1-final` trên `ca2eaf5` kèm văn bản "v1-final có T33a mở"); cấm `pkill` toàn cục, dùng `pkill -f "qemu-system.*x86_64"` sau khi xác nhận không còn job khác |
| **MAJ-4** | Ngân sách & DoD chỉ tồn tại ở §11; ViScript **không có dòng ngân sách nào** | `V:887,909-915`; `S:839-854` | Thêm cột "Ngân sách" + khối DoD vào cả hai QUEUE |
| **MAJ-5** | ViScript: `vis-host-vifast` phụ thuộc v2 (`vifast-urt`) nhưng VT01 phải "biên dịch được"; chiều phụ thuộc workspace mâu thuẫn | `S:649`; `V:572` vs `V:610`; `S:824`; `S:843` | VT01: `vis-host-vifast` là **stub** trả `NotSupported`, không path-dep vào v2; quy định 1 chiều phụ thuộc duy nhất |
| **MAJ-6** | Luật ViScript (≤1500 dòng, `unsafe` chỉ 2 crate, SPDX) **không có cổng, không có task** (khác v2 có T03) | `S:658` vs `S:804`; `V:954` | Thêm **VT01b**: script kiểm luật + **3 case âm**, hoặc gộp vào tiêu chí VT01 ("cổng gồm 6 bước, trong đó có kiểm 1500 dòng/unsafe/SPDX") |
| **MAJ-7** | Mâu thuẫn thẩm quyền SPEC: "chỉ Agent 0 đổi SPEC" vs VT02 giao Agent 2 viết SPEC; và "không sửa spec" (§11.4) vs T04 là task ghi spec | `S:884`, `S:836` vs `S:844`; `V:872` vs `V:955` | Câu chuẩn: "**VT02/T04 là bản nháp do Agent 2 soạn; Agent 0 duyệt thành nguồn sự thật; sau duyệt chỉ Agent 0 sửa**" + thêm dòng "T04/VT02: task này ĐƯỢC phép ghi đúng các tệp spec/…, không gì khác" |
| **MAJ-8** | DECISIONS.md sẽ thiếu **D-01…D-08** (vẫn hiệu lực: gate mềm, đóng băng tính năng, MAX_IO=256, soak trên `ps`) trong khi kế hoạch chỉ chép §12 (bắt đầu **D-09**) | `V:918-942` (D-09…); `D:2012-2023`; `D:1472,1517` | Chép D-01…D-08 vào đầu `v2/agent/DECISIONS.md`, ghi "vẫn hiệu lực cho v1; v2 thay D-01 bằng D-15" |
| **MAJ-9** | Bàn giao chưa tạo `v2/agent/`, `viscript/agent/`, `STATE.md`, `DECISIONS.md`; T02/VT01 lại là task tạo chúng ⇒ **nguy cơ ghi đè** | `V:846-855,953`; `S:843`; `ls` (không có `v2/`) | Người điều phối tạo trước; sửa T02/VT01: "**không ghi đè** QUEUE/STATE/BLOCKED/DECISIONS nếu đã tồn tại; chỉ tạo thiếu" |
| **MAJ-10** | Quy ước commit `[T<id>]` không có tương đương `[VT<id>]`; `R<n>.json` không có trường `project`; lịch sử dùng kiểu thứ ba; ID trùng với bug `T26/T31/T33a` | `V:866`; `S:832-837`; `V:905`; `git log` | Ghi vào AGENTS.md: "v2: `[T##]`; viscript: `[VT##]`; v1 bug: `T33a`/`T26`" + thêm `project` vào JSON + `agentctl report` lọc theo `--project` |
| **MAJ-11** | Lệnh vòng lặp không có task sở hữu: `v2 gate` (đầy đủ), `v2 nightly`, `v2 mutants`, `v2 repro`; `vis-dev gate`/`nightly`; mutation (`test/mutants.toml`, K1c "mọi đột biến chuẩn phải bị bắt") | `V:763-766`, `V:739`, `V:784`; `S:805-806` | Thêm **T17** (gate đầy đủ + nightly + mutants) và **VT11** (gate/nightly) vào vòng 1, hoặc hạ §11.3 bước 5 xuống "chỉ `--fast` trong vòng 1" và ghi rõ |
| **MAJ-12** | T04 không có trạng thái "chờ duyệt" trong mô hình agentctl: `next` sẽ trả T04 (không có phụ thuộc), không có cột trạng thái, không ai ghi APPROVED | `V:955,857`; `S:844` | Thêm cột `Trạng thái` (`READY/BLOCKED/REVIEW`) vào QUEUE; `agentctl next` **bỏ qua** `REVIEW`; mẫu `APPROVED <ngày> <người>` |
| **MAJ-13** | Mốc tích hợp lệch: `term.vidl` "VS3 + G3" tạo vòng artifact với G3 ("ViTerm chạy ViShell"); VS3 phụ thuộc VSH2 (V) vs VS2 (S); G3/U1 **không có tiêu chí**; tiêu chí VS3 là của G3 | `V:575,817,798`; `S:822,821` | Tách `term.vidl` thành mốc riêng (VD: `T-UI`) trước G3; thống nhất cạnh VS3; thêm cột tiêu chí cho §10.2–10.5 |

### 6.3 MINOR

| Mã | Phát hiện | Dẫn chứng | Sửa |
|---|---|---|---|
| MIN-1 | T03 "có case âm cho **từng luật**" không nêu số; §8 có 6 luật, T03 kiểm 5 | `V:954,614-619` | "5 luật, 5 case âm; luật 1500 dòng & 'code sinh trùng spec' giao T05/kiểm dòng" |
| MIN-2 | Câu "≥8 task, ≥3 nhánh" chỉ đạt **nếu tính dòng ViScript**; nhánh `(T07→T08→T11)` thực ra phụ thuộc T02 | `V:896` vs `V:969` | Vẽ lại sơ đồ nhánh có T02 là nút; ghi "3 nhánh = B, D, E" |
| MIN-3 | Tiêu chí K1a/K1b/K1c (100.000 seed, bao phủ 100 % cặp, Miri) **không có trong tiêu chí task** ⇒ không thể tuyên bố K1a xong từ queue | `V:782-784` vs `V:958,962,963` | Thêm vào T12/T11/T07: "100.000 seed, 0 lỗi; bao phủ 100 % cặp ALLOWED; Miri toàn bộ" |
| MIN-4 | `v2` / `vis-dev` không định nghĩa là lệnh gì | `V:759`, `S:803` | "xtask `cargo xtask v2 …`" hoặc "`./scripts/v2.sh`" — ghi ở AGENTS.md |
| MIN-5 | Hai báo cáo `R1` trùng tên; JSON thiếu `project`; đụng quy ước `docs/agent2-report-*` (101 tệp) + `docs/overnight-log.md` | `V:905`, `V:967`, `S:852`, `ls docs` | `R1-v2`/`R1-viscript`; thêm `project`; ghi rõ quy ước cũ chỉ còn cho v1 |
| MIN-6 | Mô hình quyết định **đảo vai**: §11.2 nói `DECISIONS.md` do **Agent 0** ghi, nhưng repo đang có `docs/decisions/log.md` = "quyết định **tự động của agent2**" | `V:855`; `docs/decisions/log.md` | Ghi rõ thay đổi quy trình trong `AGENTS.md`; `docs/decisions/*` chỉ còn v1 |
| MIN-7 | `.gitignore` **đã đủ** (kiểm bằng `git check-ignore`); chỉ thiếu mục cho bytecode cache trong cây | `.gitignore`; `git check-ignore` | Thêm `**/*.vbc` nếu cần; **không** tạo `.gitignore` mới cho `v2/`/`viscript/` |
| MIN-8 | T02/VT01 tạo `LICENSE-MIT/-APACHE` trong khi root đã có; thiếu `LICENSE`, `NOTICE`, `THIRD-PARTY` | `V:953`, `S:843`, `ls` | Quyết: "một bản ở root"; thêm `NOTICE` + `THIRD-PARTY.md`; thêm luật SPDX theo loại tệp |
| MIN-9 | Danh sách BLOCKED ở `V:969` thiếu T16; "VT01 tới VT09" thiếu VT10 | `V:969`, `S:852` | Sửa thành "VT01…VT10" và "…T12–T15 **và T16**" |

### 6.4 NIT

| Mã | Phát hiện | Dẫn chứng | Sửa |
|---|---|---|---|
| NIT-1 | §12 bắt đầu D-09 ⇒ người đọc tưởng thiếu | `V:922` | Thêm dòng "D-01…D-08: xem dossier §19.1, vẫn hiệu lực" |
| NIT-2 | "Vòng 1" chưa định nghĩa; quan hệ với `R<n>` không nói | `V:946`, `V:901` | "vòng = một chu kỳ QUEUE → R<n>; hai dòng đánh số độc lập" |
| NIT-3 | "VS3 + G3" viết như **một mốc** | `V:575` | "sinh ở VS3 (sau G3)" |
| NIT-4 | `S:4` "mọi quyết định về ngôn ngữ nằm ở đây" mâu thuẫn VD-08 (Agent 0 quyết) | `S:4`, `S:884` | "mọi **đặc tả** ngôn ngữ ở đây; quyết định do Agent 0" |
| NIT-5 | `review/` là tên thư mục nháp nhưng sẽ chứa nguồn sự thật | `git status` (`?? review/`) | Đặt lại `docs/v2/`, `docs/viscript/` trước khi commit |

---

## 7. Checklist bàn giao đã sửa (bản cuối — dùng thay kế hoạch 4 bước)

**Định nghĩa "bàn giao xong" (Definition of Ready):** Agent 2 mở máy, đọc **một** thư mục `agent/` và **hai** tài liệu là **chạy được vòng lặp** mà không phải hỏi ai; mọi task trong QUEUE có (i) tiêu chí đo được, (ii) ngân sách, (iii) lệnh cổng, (iv) trạng thái (`READY|BLOCKED|REVIEW`).

### 7.1 Chuẩn bị (người điều phối; 1–2 giờ)

- [ ] **B1. Chốt chỗ đặt tài liệu + commit.** `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/viscript/ViScript-thiet-ke.md` (hoặc `v2/docs/`, `viscript/docs/`). Cập nhật `README.md` + `docs/README.md`. *Xong khi:* `git ls-files` thấy cả hai; `docs/README.md` có 2 dòng trỏ tới.
- [ ] **B2. Vá tài liệu hoặc phát hành `ERRATA.md`** cho BLK-1, MAJ-1…MAJ-3, MAJ-5…MAJ-9. *Xong khi:* mỗi phát hiện có quyết định "vá / chấp nhận + lý do".
- [ ] **B3. Tạo cây điều khiển** (người điều phối, **không** phải Agent 2):
      `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `v2/agent/JOURNAL/`, `v2/agent/reports/`;
      `viscript/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `viscript/agent/JOURNAL/`, `viscript/agent/reports/`.
      *Xong khi:* `ls` đủ 12 mục; `git status` không có tệp rỗng chưa commit.
- [ ] **B4. `v2/agent/QUEUE.md`** = `V §13` **+ Phụ lục A** (5 khối ở §1.1) **+ cột Trạng thái/Ngân sách** **+** sửa T01→T01a/b/c, T03 (gen-check), T10 (mô hình tối giản), T16 ("task đã mở" + `blocked[]`), thêm T17 (gate/nightly/mutants) nếu chấp nhận MAJ-11.
- [ ] **B5. `v2/agent/DECISIONS.md`** = **D-01…D-08** (dossier §19.1) + **D-09…D-27** (`V §12`) + 2 dòng mới: "D-28: quy ước commit `[T##]`" và "D-29: APPROVED T04 = điều kiện mở T05".
- [ ] **B6. `v2/agent/STATE.md`** khởi tạo: `HEAD=ca2eaf5`, `BUILD_ID=—`, `task=—`, `bước=—`, `cổng gần nhất=—`, `việc dở=—`, `nhánh=main`.
- [ ] **B7. `v2/agent/BLOCKED.md`** khởi tạo: `T04 — REVIEW: abi.toml chờ Agent 0 duyệt; kèm BLK-1 (Exit)`; `T05,T09,T10,T12–T16 — BLOCKED bởi T04`.
- [ ] **B8. `viscript/agent/QUEUE.md`** = `S §20.2` **+ Phụ lục A** (luật/ngân sách/DoD/cổng `vis-dev`/định dạng R) **+** VT01 (SPDX + luật cấu trúc + stub `vis-host-vifast`), VT02 (bản nháp, không chặn), VT10 (đường dẫn inlined).
- [ ] **B9. `viscript/agent/DECISIONS.md`** = VD-01…VD-10 (`S §22`) + dòng "quy ước commit `[VT##]`".
- [ ] **B10. `AGENTS.md`** (root, `v2/`, `viscript/`) — nội dung tối thiểu:
      (1) thứ tự nguồn sự thật: **code+harness > spec/ > DECISIONS.md > tài liệu thiết kế**;
      (2) cấm: sửa `spec/` (trừ T04/VT02), sửa `accept()`, `#[ignore]`, nới ngưỡng, retry;
      (3) lệnh: `v2 check|gate --fast` / `vis-dev check|gate --fast` (**chưa** có `gate` đầy đủ trong vòng 1);
      (4) commit: `[T##]` / `[VT##]`; SPDX theo loại tệp;
      (5) T04 = REVIEW, không tự sửa ABI; VT02 = REVIEW, không chặn;
      (6) **không** `pkill -f qemu-system` khi còn nhánh khác đang chạy QEMU;
      (7) `docs/decisions/*`, `docs/agent2-report-*`, `docs/overnight-log.md` = lịch sử v1, không dùng cho v2.
- [ ] **B11. Quyết định git**: tag `v1-final` **trên `ca2eaf5`** (sau T01b), chính sách nhánh (v2 trên `main`), `.gitignore` (đã đủ — chỉ thêm `**/*.vbc` nếu cần), `LICENSE`/`NOTICE`/`THIRD-PARTY` (một bản ở root).
- [ ] **B12. Định nghĩa duyệt**: mẫu trong QUEUE `APPROVED <YYYY-MM-DD> bởi Agent 0 — ghi D-30`; ai kiểm (Agent 0), SLA (≤1 ngày), và T04 ⇒ mở T05 **chỉ** khi có dòng APPROVED.

### 7.2 Gửi và mở vòng (khi B1–B12 xong)

- [ ] **B13. Gửi 1 gói** (không gửi rời): 2 tài liệu + 2 `QUEUE` + 2 `DECISIONS` + 2 `STATE` + 2 `BLOCKED` + `AGENTS.md`.
- [ ] **B14. Thông điệp mở đầu cho Agent 2** (5 dòng): làm **T01a → T02 → T06 → T07→T08→T11** và **VT01 → VT03/VT04** trước; T04/VT02 là REVIEW, **không** tự sửa spec; nhánh nào cũng phải chạy `gate --fast` cuối task; ghi STATE sau mỗi bước.
- [ ] **B15. Ack có kiểm chứng**: Agent 2 trả lời bằng **nội dung tệp** (STATE.md có task đầu tiên; BLOCKED.md có T04 REVIEW), không chỉ bằng chat.
- [ ] **B16. Đo trước khi rời máy** (§11.6): ≥8 task READY, ≥3 nhánh thực (T03/T06, T07-chain, VT-line), mỗi task ≤3 h, **soak chạy riêng** không song song với build.

### 7.3 Trong vòng (nhịp điều phối)

- [ ] Kiểm `R<n>.json` **trước** MD (`V:907`); JSON phải có `project`.
- [ ] T04/VT02 duyệt bằng dòng APPROVED + D-entry, không bằng chat.
- [ ] Mọi tài liệu sửa trong vòng ⇒ bump số mục + ghi vào JOURNAL (không sửa lén).

---

## 8. Phụ lục

### 8.1 Lệnh đã dùng để kiểm chứng (tái lập được)

```
wc -l review/src/*.md                     # 2220 / 986 / 984
git log --format="%h %p %ad %s" -3        # ca2eaf5 (r323) ← 8077d71 ← bce9856 ; r323 ĐÃ commit
git tag -l                                # overnight-start, v0.1.0  (chưa có v1-final)
git status --short                        # ?? ViFastOs-dossier.md ; ?? review/
ls -1                                     # KHÔNG có v2/, viscript/, AGENTS.md, NOTICE, THIRD-PARTY
grep -n "SyscallId::Exit" crates/vifast-abi/src/lib.rs   # 1171: Exit = 34  (v1 có, v2 không)
grep -n "VIFAST_NO_RETRY" scripts/test-qemu.py           # 508-510
grep -n "for iteration\|--repeat\|settle" scripts/soak.py # 132,142,178-179 (tuần tự, không song song)
git check-ignore v2/target/debug/x viscript/target/debug/x v2/agent/QUEUE.md
#   → IGNORED , IGNORED , tracked-ok   ⇒ .gitignore ĐÃ đủ
```

### 8.2 Giới hạn của review này

- **Không** chạy build/QEMU/soak, không đo thời gian thực ⇒ các con số thời lượng chỉ suy từ `D:1671` và `V:887`.
- **Không** đánh giá nội dung kỹ thuật kernel/ngôn ngữ (ABI seL4, MCS, VM, kiểu…). BLK-1 là phát hiện **về tính đầy đủ của ABI so với yêu cầu của chính tài liệu**, không phải tranh luận thiết kế.
- **Không** đọc hết 2220 dòng dossier; chỉ đọc §13, §15.1, §19 và các mục được dẫn.
- Không kiểm được "Agent 0 có duyệt đúng hạn" hay năng lực thực tế của Agent 2.

### 8.3 5 câu cần Agent 0 trả lời trước khi gửi

1. `Exit` vào ABI bằng **syscall 14**, bằng `Tcb.Exit`, hay bằng giao thức procsrv? (chốt trong D-28)
2. N của T01: **600** (khớp tiêu chí T33a) hay **750** (khớp §9.12 p<0,4 %)? Chạy trên **máy nào** (`mps`/`ps`)?
3. Tag `v1-final` khi **còn T33a mở** — chấp nhận (ghi rõ trong tag message) hay **không tag**?
4. `spec/` ai được ghi: T04/VT02 có được phép ghi **đúng** `spec/abi.toml` + `spec/kernel.toml` (và `SPEC.md`) không?
5. `vifast-agentctl` đặt ở đâu để **cả hai dòng** dùng được từ ngày đầu (hiện ở `v2/tools`, tạo bởi T06 sau T02)?

---

*Hết báo cáo. Không sửa tệp nguồn nào; chỉ tạo tệp này.*
