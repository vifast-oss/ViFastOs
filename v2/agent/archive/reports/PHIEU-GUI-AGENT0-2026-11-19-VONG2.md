# PHIẾU GỬI AGENT 0 — đêm 2026-11-19, vòng hai

**Từ**: Agent 1 (lead), quyết thay Agent 0 theo uỷ quyền của người dùng 2026-11-18 21:55 JST.
**Báo cáo đầy đủ**: [`BAO-CAO-DEM-2026-11-19-VONG2.md`](BAO-CAO-DEM-2026-11-19-VONG2.md) (314 dòng).
**Phiếu này**: một khối trạng thái, bảng quyết định, **bốn câu cần chốt**, ba việc tiếp, nợ còn lại.
Chỗ nào phiếu khác báo cáo thì phiếu là bản **đã kiểm lại** — ghi rõ ở Q1.

---

## 1. Trạng thái một khối

```text
HEAD         56373184f8b2 (main)   chưa push: 0
build-inputs 226 mục   dấu vân tay: 40f902a41ecd3979   dirty: 0
ảnh sản phẩm 2af101d141fb9a2864a92941cd60030e86a382873eb0b184ba945be25a460896
             size=213632   # build=d6b1bf144b141cf5 inputs=40f902a41ecd3979 dirty=0
             data+bss=224288 / trần 262144 (F5, không nới)
ảnh ca âm    f38fc77a0bae2729ff341a71b081524f816713e2d515105ee554e41ca4296ea0
             size=82552    (shell tắt; parity not_ok=14 khong_co_shell=14, exit=5)
gate --fast  PASS trên 56373184 (sau 18 merge); fda1500d..HEAD: diff v2/kernel|v2/lib RỖNG
task         DONE=168  READY=0  BLOCKED=0  REVIEW=0
```

Nguồn: `./v2/bin/v2 status` (worktree này, 07:06 JST); ảnh sản phẩm
`JOURNAL/kt171-logs/kt171-measure.log:2,34,56`; ảnh ca âm `…/kt171-measure.log:310-313`;
cổng `BAO-CAO…§9` + `git diff --stat fda1500d..HEAD -- v2/kernel v2/lib` (rỗng).
`v2/artifacts/` là thư mục gitignore và **không có** trong worktree này — sha lấy từ log đã commit.
Ba chế độ checker chạy lại trên cây sạch: `errors=0` / `--strict errors=0` / `--selftest 56/56`.

**Sổ đã lật hết**: `READY=0`. KT-162/174/175/177/178/179 đều đã DONE.

**Điều đáng nhớ**: luật `wake` bị phá **ba vòng** trong một đêm, cả ba lần đều do **reviewer độc
lập** — không phải tác giả. Giữ quy trình "reviewer có quyền phá" là thứ đáng giữ nhất.

---

## 2. Bảng quyết định D2-144 … D2-158

| Mã | Việc Agent 0 cần biết |
|---|---|
| D2-144 | `wake()` thành API **chỉ-dùng-test**; sản phẩm dùng `wake_from`; cưỡng chế bằng checker + test ghim. |
| D2-145 | Nợ "4 client mức kernel" chỉ được đóng bằng **ảnh test**, không bằng lời. |
| D2-146 | Seed `/docs` + `/docs/note.txt`; đột biến "`pwd` in hằng `/`" **phải** bị bắt. |
| D2-147 | K4' bước 1 chỉ phần host-testable; **không** boot, **không** tuyên bố K4' đạt. |
| D2-148 | Giao thức đêm vòng hai: 4 sub-agent/worktree riêng, chỉ lead merge, mốc 06:30–09:00. |
| D2-149 | N1 "600 boot x86_64" **ở lại K5'**; không đóng bằng lời. |
| D2-150 | Máy kiểm "test có răng": bốn lớp **chắc chắn**, không heuristic mờ. |
| D2-151 | T-INFRA: cổng chạy song song ⇒ `persist --crash` đỏ oan; chạy lại máy sạch PASS. |
| D2-152 | **N3 ĐÓNG HẲN**: 4 client mức kernel, sổ `(client,va)`, đột biến làm TAP 37 đỏ; nới `OBJ_ARENA_BYTES`/`MAX_PROCS` **có ghi**. |
| D2-153 | Sáu task ĐẠT (kèm điều kiện đã vá) — **số trong mục này sai, xem D2-158**. |
| D2-154 | Sóng ba: K4' bước 2 + review + K5' audit + siết máy kiểm. |
| D2-155 | **KT-165 KHÔNG ĐẠT** (L11 phá 5 lớp) ⇒ mở KT-171 vá vòng 3. |
| D2-156 | Sáu tiêu chí K6 đo lại trên ảnh `2af101d1…`; K7 giữ nguyên ĐẠT. |
| D2-157 | Sóng bốn chỉ chạm **công cụ/test/tài liệu**, không chạm `v2/kernel`/`v2/lib` để ảnh K6 không mất hiệu lực. |
| D2-158 | Đính chính D2-153 sau audit: KT-158 **11→14** phép kiểm (15 sau KT-166), **một** đột biến đỏ, KT-160 **22** test (24 sau KT-166). |

---

## 3. Bốn câu cần Agent 0 chốt

### Q1 — Vòng vá 5 cho luật `wake`, hay chấp nhận nợ tới v2.1?

**Đã đóng**: `is_test_file` cho luật 1–5 (KT-172: 46/46 selftest, 2 đột biến đỏ, checker cũ
`errors=0` trên cùng cây). Vá của lead đóng thêm **H1/H2/H3/H5** (raw string, `include!` không
literal, "sản phẩm thắng").

**Kiểm lại H8** (worktree L22, dựng đúng §2.8 của `JOURNAL/kt174-review3.md`): đột biến
`#[path = r"evil_b.rs"]` từ mã sản phẩm nay **ĐỎ** `E_STRICT_TEST_ONLY_WAKE … evil_b.rs:3`,
`errors=1`; đã gỡ đột biến, cây sạch. **H8 đã đóng** — báo cáo §6 còn ghi H8 là nợ, **sai**;
sửa khi duyệt.

**Còn hở**: **H4** (`include!` lồng hai lớp — dựng lại: `errors=0`, RC=0); **H6/H7** (macro
`v2/lib` che `concat!` / chứa `include!`) — vá của lead không chạm `scan_lib_macros`, nên vẫn hở
theo review L16 (chưa dựng lại hai ca này). Cả ba đều đòi **viết mã mới** trong tệp sản phẩm.

- **(a) Mở vòng vá 5** — **thu hẹp: chỉ H4** (fixpoint đồ thị `include!`, G4, ~1 h, cùng vùng mã
  với Q4); **H6/H7** (macro) thành KT-177 cho v2.1.
- **(b) Chấp nhận nợ cả ba tới v2.1** (~0 h): ghi KT-177 với 3 repro đã có.

**Khuyến nghị: (a) thu hẹp** — H4 cùng lớp với H5 (lỗ nặng nhất) và vá được bằng fixpoint ~1 h,
gộp chung việc 1 với Q4 vì cùng vùng mã; để H4 mở trong khi tuyên bố "luật đã đóng" đúng là lớp
"test xanh giả" đã gặp hơn mười lần. H6/H7 cần phân tích thân macro — đắt hơn, ít đường khai
thác hơn, để v2.1 **có tên** (KT-177 + repro).

### Q2 — KT-167 (K4' bước 2) ĐẠT CÓ ĐIỀU KIỆN: vá ngay hay để bước 3?

Năm điều kiện C1–C5 (`JOURNAL/kt175-a64review.md` §7): **C1** 14 hằng chết (11 `TCR_*_MASK` +
3 shift) — 14/14 đột biến vẫn 43/43 xanh; **C2** `sctlr_builder` tautology — 3/3 hoán vị setter
xanh; **C3** chỉ 3/33 stub host được kiểm; **C4** claim "đủ 39 trường" **SAI** (thiếu ≥ 8 trường
thật); **C5** log build thiếu `Compiling`.

- **(a) Vá ngay trước bước 3** (~1,5 h cho C1/C2/C4/C5; C3 để bước 3).
- **(b) Để bước 3** (~0 h): hằng chết thành sống khi bước 3 dùng mask, nhưng test tautology vẫn không bắt được sai bit.

**Khuyến nghị: (a), nhưng chỉ C1/C2/C4/C5** — C1 nguy hiểm nhất: bước 3 sẽ **im lặng sai bit**
nếu mask sai; C2 đúng lớp "test xanh giả" đã gặp hơn mười lần; C4 là **sai tài liệu**, phải sửa
trước khi ai trích dẫn. C3 (30 stub) cần asm thật — để bước 3.

### Q3 — K5': duyệt M1 và mở `spec/abi.toml` `[calling.x86_64]`?

Audit `reports/K5-X86-READINESS.md`: 18 mục thiếu (M1–M18, kèm `file:dòng`), 8 bước (M1–M8, mỗi
bước ≤ 3 h), 10 rủi ro. Bước **M1** (nửa giờ) chốt ba thứ: phạm vi K5' "như K3" (không
VT-x/KPTI/UEFI/PCIe-MSIX), đường boot tạm (`-kernel` **hay** UEFI sớm), ai mở `[calling.x86_64]`
và khi nào.

- **(a) Duyệt M1 ngay + mở `[calling.x86_64]` ngay.**
- **(b) Duyệt M1, chưa mở `spec/`** — chỉ mở khi có bản nháp gọi hàm kèm task có tên.

**Khuyến nghị: (b)** — M1 là quyết định **rẽ nhánh** (UEFI sớm ⇒ M3 phải làm `vifast-boot`
trước), nửa giờ, nên duyệt ngay. Nhưng `spec/abi.toml` là nguồn sự thật ABI (AGENTS §4.1,
D2-04/D2-22): mở "quyền sửa" mà chưa có nháp `[calling.x86_64]` là mời trôi. Mở **một lần** theo
đúng khuôn KT-05, gắn với bước M7.

### Q4 — Luật cứng cho `include!`/`#[path]` (ngoài `--strict`)?

Bốn vòng vá đều bị phá bằng **cùng một lớp**: đường dẫn gián tiếp (`include!`, `#[path]`, raw
string, macro). Hiện phân giải chỉ chạy trong `--strict` (tức `gate --fast`), **không** chạy ở
`v2 check` thường.

- **(a) Luật cứng**: mọi `include!`/`#[path]` trong `v2/kernel`+`v2/sim` phải **literal + trong
  cây**, kiểm ở **mọi** chế độ; ghi một dòng vào `AGENTS.md` (Agent 0 giữ tệp).
- **(b) Giữ nguyên**: chỉ `--strict` (đã nằm trong cổng).

**Khuyến nghị: (a)** — cây hiện **sạch** (`--strict errors=0`, selftest 46/46), nên bật ở mọi
chế độ **không** làm đỏ gì; chi phí ~1 h; và nó là điều kiện để chấp nhận nợ H6/H7 ở Q1.
Không đổi ngữ nghĩa luật nào, chỉ đổi **phạm vi thi hành**.

---

## 4. Ba việc đề nghị làm tiếp

| # | Việc | Ưu tiên | Ngân sách | Vì sao trước |
|---|---|---|---|---|
| 1 | Luật cứng `include!`/`#[path]` ở **mọi** chế độ (Q4a) + fixpoint H4 (Q1a) + một dòng `AGENTS.md` | **P0** | 2 h | Cùng vùng mã; đóng trọn lớp include đã phải vá 4 vòng, không làm đỏ cây. |
| 2 | Vá C1/C2/C4/C5 của KT-167 (Q2a) | **P0** | 1,5 h | Bước 3 sẽ dùng mask; sửa sau = sửa trên nền sai bit. |
| 3 | K5' M1 → bản nháp `[calling.x86_64]` (chưa mở `spec/`) + bước M2 | **P1** | 0,5 h + 3 h | M1 là rẽ nhánh; M2 không cần QEMU, đo bằng `cargo test`. |

Việc 1–2 làm **trước** khi mở bước 3; việc 3 cần Agent 0 duyệt M1 trước.

---

## 5. Nợ còn lại (8 mục, cập nhật sau KT-172/KT-174)

1. **H4** — vòng vá 5 hẹp (việc 1); **H6/H7** — KT-177 cho v2.1 (H8 **đã đóng**, kiểm lại
   07:06 JST).
2. `include!`/`#[path]` phân giải **chỉ trong `--strict`** — Q4 quyết.
3. `macro_rules!` ngoài `v2/lib` + ngoài `v2/kernel/*/src`/`v2/sim/*/src` (ví dụ `init/`) không
   quét thân.
4. Bản đồ trợ giúp cùng tệp (KT-163) vẫn nhận hàm không-phải-`#[test]` trong cùng tệp — chưa đo
   tác động.
5. **K4' chưa đạt**: mới bước 1–2; chưa linker script, vector table, `eret`, đường boot, `VSpace`
   thật, `TLBI` sau `unmap`.
6. **K5' chưa mở**: chờ duyệt M1 + `[calling.x86_64]`.
7. **N1 "600 boot x86_64"** vẫn ở K5' (D2-149).
8. `cd` vào thư mục con **sâu** chưa kiểm — FS sản phẩm mới một cấp `/docs`.

---

**Ký**: Agent 1 (lead) — soạn `[KT-180]` L22, số liệu kiểm lại 07:06 JST 2026-11-19.
Phiếu ở gốc repo nên **không** thuộc `verify-numbers.py`; mọi số vẫn dẫn log như trên.
