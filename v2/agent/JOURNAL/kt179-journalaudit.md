# KT-179 — audit nhật ký mười bốn sub-agent (nguồn gốc báo cáo đêm vòng hai)

**Ngày**: 2026-11-19 (vòng hai, sóng bốn) · **Agent 2 (thợ) L21** · nhánh
`night2/L21-journalaudit` · worktree `/private/tmp/night2-L21-journalaudit` · HEAD `d68d7eb8`
(từ `main`).
**Ngân sách**: hạn cứng 08:10 JST. Không merge, không push, không `gate --fast`, không tạo
worktree mới, không QEMU.
**Phạm vi**: **đọc-only** 17 nhật ký `kt157..kt176` + log trong `kt1*-logs/`; **không sửa**
nhật ký của người khác, `spec/`, `docs/QUYET-DINH-sau-review.md`,
`docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`, `BAO-CAO-DEM-2026-11-19-VONG2.md`.

---

## 0. Kết luận trước

| Việc | Kết quả |
|---|---|
| Nhật ký đọc | **17** tệp trong dải `kt157..kt176` (không phải 14 — xem §1.1) |
| Khẳng định kiểm | **152** (bảng §2 + phụ lục A) |
| Khớp nguồn | **117** |
| **Không nguồn trong repo** (số chỉ nằm trong nhật ký, không có dòng log trong `kt1*-logs/`) | **25** (§4) |
| **Lệch** (nguồn nói khác / tự ĐẠT quá mức / sai sự thật / lệch nhãn) | **10** (§3) |
| `./v2/bin/v2 check` | **PASS** (`RC=0`, log `/tmp/kt179-v2check.log`) |
| Sửa nhật ký | **KHÔNG** (đúng luật task — lead quyết) |

**Lớp lỗi nặng nhất — tự tuyên bố ĐẠT trong khi reviewer chấm KHÔNG ĐẠT**:

1. **KT-165 (L8)** — nhật ký §1 bảng "ĐẠT" toàn bộ; §5 coi macro sinh `wake`, `include!` tệp
   ngoài, `stringify!` là "lý thuyết, không có trong cây"; reviewer **L11 (KT-168)** chấm
   **KHÔNG ĐẠT** (còn 5 lớp lọt, B8/B9 là lỗ hổng thật).
2. **KT-171 (L14)** — nhật ký §0 "6/6 ĐÃ ĐÓNG", "Không lớp nào phải tuyên bố 'chưa đóng'";
   reviewer **L16 (KT-174)** chấm **KHÔNG ĐẠT** (8 cách lọt, H5 nặng nhất).

Đây đúng là lớp lỗi task yêu cầu ghi rõ: **tác giả tự chấm ĐẠT, reviewer độc lập chấm KHÔNG
ĐẠT**. Hai nhật ký này không nói sai về **6 lỗ hổng được giao** (đều đóng thật, có đột biến đỏ),
nhưng **tự chấm ĐẠT cho cả mục tiêu** ("mọi tham chiếu `wake` trong mã sản phẩm ⇒ ĐỎ") khi
reviewer còn phá được — tức là **kết luận rộng hơn bằng chứng**.

---

## 1. Phương pháp

1. Đọc toàn bộ 17 nhật ký trong dải `kt157..kt176` và liệt kê mọi con số/khẳng định kiểm được.
2. Với mỗi con số, tìm **dòng log thật** theo thứ tự: `v2/agent/JOURNAL/kt1*-logs/` → log thô
   `/tmp` (ghi rõ **ngoài repo**) → mã nguồn (`grep`/`git show`) → chỉ có trong nhật ký.
3. Đánh **KHỚP** khi tìm được dòng chứa đúng số/chuỗi; **KHÔNG NGUỒN** khi không tệp nào trong
   repo chứa; **LỆCH** khi nguồn ghi khác hoặc phán quyết reviewer ngược lại.
4. Với khẳng định "đã đóng/ĐẠT", kiểm **đột biến đỏ**: log có `FAILED`/`errors=1`/`not ok`
   không, hay chỉ có test xanh.
5. Đối chiếu phán quyết với bốn reviewer: **L6** (`kt161-review.md`), **L11**
   (`kt168-review2.md`), **L16** (`kt174-review3.md`), **L17** (`kt175-a64review.md`).

### 1.1 Vì sao 17 nhật ký, không phải 14

Dải `kt157..kt176` có **17** tệp: `kt157-pager4`, `kt158-fsdir`, `kt159-wake2`, `kt160-aarch64`,
`kt161-review`, `kt163-teeth`, `kt164-shared`, `kt165-wakefix`, `kt166-gapfix`, `kt167-a64step2`,
`kt168-review2`, `kt170-teeth2`, `kt171-wakefix3`, `kt172-testfile`, `kt174-review3`,
`kt175-a64review`, `kt176-reportaudit`. **KT-169 không có nhật ký** (chỉ có `kt169-*.log` và
`v2/agent/reports/K5-X86-READINESS.md`); **KT-162 không tồn tại**. Con số "mười bốn" trong báo
cáo là số **nhánh merge** tới `fda1500d` (`git log --merges 6b2fdce7^..fda1500d` = **14**), không
phải số nhật ký. Audit này đọc **tất cả** 17 tệp có mặt.

---

## 2. Bảng audit tổng hợp

Ký hiệu: **K** = khớp; **N** = không nguồn trong repo; **L** = lệch. Chi tiết từng khẳng định ở
phụ lục A; các chỗ N/L liệt kê đầy đủ ở §3–§4.

| Nhật ký | Số khẳng định kiểm | Khớp | Lệch / không nguồn | Ghi chú |
|---|---:|---:|---:|---|
| `kt157-pager4.md` (L1) | 8 | 5 | 2 N + 1 L | TAP 37 + đột biến có log (đã chép vào `kt171-logs/`); `7/7 boot` và `v2 check` **không log**; báo cáo gán "21/21" cho L1 nhưng nhật ký nói **chưa chạy matrix** |
| `kt158-fsdir.md` (L2) | 11 | 10 | 1 L | 14 phép kiểm, 1 đột biến đỏ, 42/42 selftest, ảnh `735e1030…` đều khớp; "không còn nợ" **lệch** L6 (M2 còn xanh) |
| `kt159-wake2.md` (L3) | 9 | 7 | 1 N + 1 L | 15/15, 6 test ghim, đột biến `errors=1` khớp; baseline `11/11` **không log**; "không còn nợ" **lệch** L6 (6 lỗ hổng) |
| `kt160-aarch64.md` (L4) | 10 | 3 | 6 N + 1 L | 22 test + build `Compiling` khớp (qua log reviewer); **3 đột biến, 115 L0, v2 check, rlib 218360 đều không log**; "ĐẠT bước 1" **lệch** L6 (T1/T2/T3) |
| `kt161-review.md` (L6) | 13 | 11 | 1 N + 1 LN | Reviewer: 14/14, M1/M2, 15/15, 14/15, M3/M4, 6 lỗ hổng, 22 test, 4 đột biến, v2 check đều khớp; `rlib 218360` **không log**; nhãn H1–H6 **lệch** nhãn B1–B6 trong log |
| `kt163-teeth.md` (L5) | 9 | 7 | 2 N | 21/21, 944 test, 124 tệp, 20 allowlist, đột biến `2 bước FAIL`, 4 đột biến, 2 test giả đều khớp; **11,7→2,6 s** và **291/291** không log |
| `kt164-shared.md` (L7) | 7 | 2 | 5 N | Chỉ 2 khẳng định kiểm được bằng mã (một định nghĩa trait, Sv39 impl); **22 test, 118 L0, 3 mmu_shared, 2 đột biến, v2 check đều không log** |
| `kt165-wakefix.md` (L8) | 9 | 8 | 1 L | 26/26, 6 `E`, checker cũ `errors=0`, 7 ca selftest chết, v2 check, M1/M2 đều khớp; **"ĐẠT" toàn bộ lệch L11 KHÔNG ĐẠT** |
| `kt166-gapfix.md` (L9) | 8 | 7 | 1 N | 15 phép kiểm, 4 FAIL, 24 test, 3 đột biến, 42/42, L0 hai crate, "ĐÓNG" khớp (qua log L11); **v2 check không log riêng** |
| `kt167-a64step2.md` (L10) | 11 | 9 | 2 L | 43 test, 3 đột biến, v2 check, 11 mã objdump, TCR 17, MAIR 8, TLBI 6, barrier 13 khớp; **claim "đủ 39 trường" SAI** (L17); **"ĐẠT" lệch** L17 ĐẠT CÓ ĐIỀU KIỆN |
| `kt168-review2.md` (L11) | 9 | 9 | 0 | KT-165 KHÔNG ĐẠT, KT-166 ĐẠT, 5 lớp, 36 log, 26/26, v2 check, L0 24, B8/B9, 4 đột biến bit — đều khớp |
| `kt170-teeth2.md` (L13) | 9 | 8 | 1 N | 33/33, 977, 127, 80, 319→80, 0→0, đột biến, v2 check khớp; **2,64→3,11 s** không log |
| `kt171-wakefix3.md` (L14) | 8 | 3 | 3 N + 2 L | 6 đột biến + checker cũ + v2 check khớp (log **ngoài repo** `/tmp/kt171-mut/`); **39/39, 97 tệp, 4,6→6,0 s không log trong repo**; **"6/6 đóng" lệch L16 KHÔNG ĐẠT** |
| `kt172-testfile.md` (L15) | 7 | 5 | 2 N | 46/46, 2 đột biến, checker cũ, v2 check, 4 call site khớp; **97 vs 94** và **`E_STRICT_STALE errors=2`** không log |
| `kt174-review3.md` (L16) | 7 | 7 | 0 | 8 lỗ hổng, H5, C1–C5, v2 check, KT-171 KHÔNG ĐẠT, 2 đường trong cây — khớp (log **ngoài repo**) |
| `kt175-a64review.md` (L17) | 11 | 11 | 0 | 64 đột biến (21 XANH/43 ĐỎ), 43 passed, 6/6 TLBI, 13/13 barrier, build + rlib 625242, 14 hằng chết, 3/3 setter, 3/3 stub — khớp |
| `kt176-reportaudit.md` (L18) | 6 | 5 | 1 N | 64 khẳng định, 56 khớp, 8 lệch, STATE đã cập nhật, 6 nhóm nguồn ngoài — khớp; **v2 check không log** |
| **Tổng** | **152** | **117** | **25 N + 10 L** | |

---

## 3. Mười chỗ **LỆCH** (liệt kê đầy đủ)

### L1. KT-157 — báo cáo gán "ma trận 21/21" cho L1, nhật ký nói **chưa chạy matrix**

- `kt157-pager4.md:201-204`: "**Không** chạy matrix `-smp 1/2/4` cho cả 7 ảnh trong ngân sách
  này — đã chạy `bash v2/test/boot/ktest.sh` (smp=1, NBOOT=1, **7/7 PASS**). `-smp 2/4` không
  đổi đường pager … nhưng **chưa đo**; lead nên chạy `bash v2/test/boot/ktest.sh 1 matrix` sau
  merge."
- Báo cáo §3 bảng L1: "TAP 37 `clients=4`; đột biến đỏ; **21/21 boot**".
- Nguồn thật của 21/21: `kt171-logs/kt171-measure.log:486` — do **lead** chạy sau merge, không
  phải L1. Nhật ký L1 **trung thực**; lỗi nằm ở **báo cáo** (gán bằng chứng của lead cho L1).
- **Đề nghị**: báo cáo ghi "L1: 7/7 smp=1; 21/21 do lead chạy sau merge".

### L2. KT-158 — "không còn nợ kỹ thuật" trong khi reviewer còn điều kiện

- `kt158-fsdir.md:260`: "**Không** còn nợ kỹ thuật nào của phạm vi task."
- `kt161-review.md:21`: KT-158 **ĐẠT CÓ ĐIỀU KIỆN** — đột biến "`resolve_path` bỏ qua `Cwd`"
  **vẫn XANH**; phải mở KT-166 mới bắt được.
- Nhật ký chỉ chạy **một** đột biến (`pwd` in hằng `/`), không thử đột biến `Cwd` — nên kết luận
  "không còn nợ" **rộng hơn** phép đo.

### L3. KT-159 — "không còn nợ kỹ thuật" trong khi reviewer phá 6 cách

- `kt159-wake2.md:272-273`: "**Không** còn nợ kỹ thuật trong phạm vi KT-159. Ba mục (2)(3)(4)
  của D2-144 đã đóng bằng máy kiểm + test chạy, không bằng lời."
- `kt161-review.md:22`: KT-159 **ĐẠT CÓ ĐIỀU KIỆN** — luật bị lách **6** cách (H1–H6), nặng
  nhất là **không quét crate ảnh sản phẩm** (`vifast-kbin-riscv64/src`, `vifast-plat-riscv64/src`).
- Nhật ký §9 có ghi "luật mới **hẹp có chủ ý** — chỉ hai thư mục" và "nếu sau này có crate kernel
  mới … phải mở rộng" — tức tác giả **biết** phạm vi hẹp nhưng vẫn kết luận "không còn nợ".

### L4. KT-160 — "ĐẠT bước 1" trong khi reviewer chấm ĐẠT CÓ ĐIỀU KIỆN

- `kt160-aarch64.md:20-31`: mọi mục "**ĐẠT**" (crate, test host, đột biến, L0, cổng nhánh).
- `kt161-review.md:23`: KT-160 **ĐẠT CÓ ĐIỀU KIỆN** — (T1) test descriptor tự so hằng của chính
  crate (`DESC_ATTR_SHIFT 2→3`, `DESC_NG 1<<11→1<<60` vẫn 22/22 xanh); (T2) `translate` bỏ kiểm
  bảng-mức-lá không test nào bắt; (T3) crate không nằm trong `gate --fast`.
- Nhật ký **không** thử hai đột biến vị trí bit (T1) và **không** thử xoá nhánh table-at-leaf
  (T2) — nên "ĐẠT đột biến" chỉ đúng với **3 đột biến đã chọn**, không đúng với cả lớp.

### L5. KT-161 — nhãn H1–H6 **lệch** nhãn B1–B6 trong log

- `kt161-review.md:156-163` đặt H1 = `vifast-kbin-riscv64/src/proc_rt.rs`, H2 =
  `vifast-plat-riscv64/src/lib.rs`, H3 = alias `Machine::wake`, H4 = block comment, H5 = cắt
  dòng, H6 = `#[cfg(test)]` trên `use`.
- Log `kt161-logs/kt159-bypass.log` lại đánh **B1** = alias, **B2** = plat-riscv64, **B3** =
  cfg(test), **B4** = block comment, **B5** = cắt dòng, **B6** = kbin.
- **Sáu lỗ hổng là đúng** (6 dòng `errors=0`), nhưng **không đối chiếu 1:1** được H↔B nếu chỉ
  đọc log. Đây là lỗi **truy vết**, không phải lỗi nội dung. Về sau KT-165 dùng lại nhãn H1–H6
  theo nghĩa của `kt161-review.md`, nên nhãn đã ổn định từ đó.

### L6. KT-165 — **tự ĐẠT, reviewer KHÔNG ĐẠT** (nặng)

- `kt165-wakefix.md:33-46`: bảng §1 toàn "**ĐẠT**"; §5: "**Không** còn nợ kỹ thuật trong phạm vi
  KT-165."
- `kt168-review2.md:13,130-146`: KT-165 **KHÔNG ĐẠT** — 6/6 lỗ hổng **được giao** đã đóng, nhưng
  luật vẫn lọt **5 lớp** (B4, B6, B7, B8/B9, B10); **B8/B9 là lỗ hổng thật**: module sản phẩm tên
  `ktest_evil.rs` hoặc dưới `src/tests/` được biên dịch vào ảnh sản phẩm, `--strict` vẫn
  `errors=0`.
- Nhật ký §5 có ghi "còn hở về lý thuyết: (a) macro sinh `wake`, (b) `include!` tệp ngoài,
  (c) `stringify!`/`concat_idents!`" — nhưng đánh giá chúng là "**không có trong cây hiện tại**,
  đều là mã kỳ dị". Reviewer chứng minh (a) và (b) **có thật** và **biên dịch được** (B4, B6/B7),
  cộng thêm B8/B9/B10. Vì vậy kết luận "ĐẠT" là **sai** (dù 6 mục được giao đóng thật).

### L7. KT-167 — claim "cài **đủ 39 trường** `SCTLR_EL1`" **SAI sự thật**

- `kt167-a64step2.md:174-176`: "cài **đủ 39 trường** có thật của `SCTLR_EL1`".
- `kt175-a64review.md:144-167`: Linux `arch/arm64/tools/sysreg` liệt kê **54** trường; tác giả
  thiếu **15**, trong đó **≥8 trường có thật** từ ARMv8.x: `nAA` (6), `EnRCTX` (10), `CMOW` (32),
  `TCF0` (39:38), `TCF` (41:40), `ATA0` (42), `TWEDEn` (45), `TWEDEL` (49:46). "39 trường **đã
  cài**" thì đúng; "**đủ** 39 trường" thì sai.
- Đây là **lỗi tài liệu**, không phải lỗi mã (39 trường đã cài đều đúng vị trí — L17 xác nhận
  39/39).

### L8. KT-167 — "ĐẠT" trong khi reviewer chấm ĐẠT CÓ ĐIỀU KIỆN

- `kt167-a64step2.md:28-46`: mọi mục "**ĐẠT**" (sysreg, tlb, barrier, asm, host, aarch64, đột
  biến, cổng, đối chiếu chéo).
- `kt175-a64review.md:12-21`: KT-167 **ĐẠT CÓ ĐIỀU KIỆN** — C1: **14 hằng công khai "chết"**
  (11 `TCR_*_MASK` + `TTBR_BADDR_SHIFT` + `VBAR_BASE_SHIFT` + `SPSR_M_SHIFT`), 14/14 đột biến vẫn
  43/43 xanh; C2: `sctlr_builder` so OR hằng của chính crate, 3/3 hoán vị setter vẫn xanh; C3:
  chỉ **3/33** stub host được kiểm; C4: claim "đủ 39 trường" sai; C5: log build đã commit chỉ có
  `Finished`, thiếu `Compiling`/rlib.
- Nhật ký có ghi "ĐẠT đột biến 3/3" — đúng với **3 đột biến đã chọn**, nhưng **không** phủ 14
  hằng chết và 3 hoán vị setter.

### L9. KT-171 — **tự ĐẠT, reviewer KHÔNG ĐẠT** (nặng)

- `kt171-wakefix3.md:10-20`: bảng §0 sáu dòng "**ĐÃ ĐÓNG**"; "Không lớp nào phải tuyên bố 'chưa
  đóng'."
- `kt174-review3.md:9-30`: KT-171 **KHÔNG ĐẠT** — còn **8 cách lọt** (H1–H8), trong đó **2 cách
  chỉ dùng tệp trong cây** (H5, H8), `cargo check -p vifast-kbin-riscv64 --target
  riscv64gc-unknown-none-elf` **RC=0** và `--strict` **errors=0**. Nặng nhất **H5**:
  `include!("evil_b.rs")` từ tệp sản phẩm + `#[cfg(test)] mod evil_b;` ⇒ ảnh sản phẩm biên dịch
  `wake` mà checker vẫn xanh.
- Nhật ký §4 có ghi "còn hở" (is_test_file_heuristic cho luật 1–5, `include!`/`#[path]` chỉ phân
  giải trong `--strict`, `#[path]` trong `mod` inline, macro ngoài `v2/lib`) — nhưng **thiếu**
  đúng các đường H1/H2/H3/H5/H8 mà reviewer phá: chuỗi thô `r"…"`, `include!(concat!(env!(…)))`,
  `include!` tệp test-only trong cây, `#[path]` chuỗi thô tới tệp test-only. Vì vậy "6/6 đóng" chỉ
  đúng với **6 lớp được giao**, không đúng với **mục tiêu** của luật.

### L10. KT-171 — "không lớp nào phải tuyên bố chưa đóng" **lệch** cùng gốc L9

- Câu này mạnh hơn "6/6 lớp được giao đóng": nó khẳng định **không còn lớp nào**. Reviewer tìm
  thêm 8 lớp. Đây là **cùng một lỗi kết luận** với L9, tách ra vì nó là câu riêng trong §0.
- **Đề nghị** (không tự sửa): hạ thành "6/6 lớp **được giao** đã đóng; còn hở … (liệt kê §4)".

---

## 4. Hai mươi lăm chỗ **KHÔNG NGUỒN trong repo**

Đây là các con số **chỉ nằm trong nhật ký**, không có dòng log tương ứng trong
`v2/agent/JOURNAL/kt1*-logs/`. Không phải "số sai" — nhưng **không tái lập được bằng máy** từ
repo, đúng thứ luật bằng chứng của repo đòi. (Một số có log **ngoài repo** ở `/tmp`; ghi rõ.)

| # | Nhật ký | Con số / khẳng định | Tình trạng nguồn |
|---|---|---|---|
| N1 | KT-157 | `7/7 boot sạch` (smp=1, NBOOT=1) | Chỉ có trong `kt157-pager4.md:160-170`; log `kt142-logs/kt142-gate-f50c6dea.log:1724` là của **KT-142**, không phải KT-157 |
| N2 | KT-157 | `./v2/bin/v2 check` PASS | Không có `kt157-logs/`; không log nào trong repo |
| N3 | KT-159 | baseline `--selftest 11/11` | Chỉ có trong `kt159-wake2.md:49`; log `kt159-selftest.log` là **15/15** sau khi thêm 4 ca |
| N4 | KT-160 | đột biến bỏ `AF` ⇒ `2 failed` | Chỉ có trong `kt160-aarch64.md:192-197`; không có `kt160-logs/` |
| N5 | KT-160 | đột biến `unmap` không `free_frame` ⇒ `4 failed` | `kt160-aarch64.md:203-210`; không log |
| N6 | KT-160 | đột biến đảo `AP[2]` ⇒ `3 failed` | `kt160-aarch64.md:216-222`; không log |
| N7 | KT-160 | L0 `vifast-plat-common` **115 passed** | `kt160-aarch64.md:230-261`; không log |
| N8 | KT-160 | `./v2/bin/v2 check` PASS | `kt160-aarch64.md:265-274`; không log |
| N9 | KT-160 | rlib **218 360 B** | Chỉ có trong `kt161-review.md:196` (nhật ký), **không** log thô; `kt161-logs/kt160-build-aarch64.log` chỉ có `Compiling` + `Finished`, không liệt kê rlib |
| N10 | KT-161 | rlib **218 360 B** | Cùng nguồn N9 — reviewer cũng chỉ chép từ nhật ký, không có log `ls -l` |
| N11 | KT-163 | quét cây **11,7 s → 2,6 s** | `kt163-teeth.md:239-242`; không log thời gian |
| N12 | KT-163 | `mask_source` giống hệt **291/291** tệp | `kt163-teeth.md:241-242`; không log |
| N13 | KT-164 | `cargo test -p vifast-plat-aarch64` **22 passed** | `kt164-shared.md:87-90`; **không có `kt164-logs/`** |
| N14 | KT-164 | L0 `vifast-plat-common` **118 passed** | `kt164-shared.md:36,109`; không log (L18 cũng đã ghi "nguồn yếu") |
| N15 | KT-164 | `tests/mmu_shared.rs` **3 test** | `kt164-shared.md:113-121`; không log (mã hiện có 3 `#[test]`, nhưng không có log chạy) |
| N16 | KT-164 | 2 đột biến (`AP[2]`, `PTE_VALID`) ⇒ đỏ | `kt164-shared.md:127-158`; không log |
| N17 | KT-164 | `./v2/bin/v2 check` PASS | `kt164-shared.md:162-170`; không log |
| N18 | KT-166 | `./v2/bin/v2 check` PASS | Không có `kt166-logs/`; log `kt168-logs/v2-check-clean.log` là của **reviewer L11**, không phải tác giả |
| N19 | KT-170 | quét cây **2,64 s → 3,11 s** | `kt170-teeth2.md:271-273`; không log thời gian |
| N20 | KT-171 | `--selftest 39/39` | Chỉ có trong `/tmp/kt171-mut/logs/v2-check-postcommit.log:287` (**ngoài repo**); không có log trong `kt171-logs/` |
| N21 | KT-171 | **97** tệp test-only (25 `ktest*`, 67 `tests/*`, 2 `*_tests.rs`, …) | `kt171-wakefix3.md:151-156`; không log |
| N22 | KT-171 | `--strict` **4,6 s → 6,0 s** | `kt171-wakefix3.md:157-158`; không log |
| N23 | KT-172 | **97** tệp test-only vs **94** theo heuristic | `kt172-testfile.md:74-76`; không log |
| N24 | KT-172 | `E_STRICT_STALE` **errors=2** khi bỏ 2 mục `proc_demo.rs` | `kt172-testfile.md:58-59` ("đã đo đúng như dự đoán") nhưng **không có log**; `kt172-logs/` chỉ có clean/mutation A/B/selftest/v2-check |
| N25 | KT-176 | `./v2/bin/v2 check` PASS | `kt176-reportaudit.md:266-269`; không có `kt176-logs/` |

**Ghi chú về N20/N21/N22**: KT-171 tự ghi "log đầy đủ ở `/tmp/kt171-mut/logs/` (không commit —
log tạm)". Đã kiểm: `/tmp/kt171-mut/logs/` **còn nguyên** (22 tệp), 6 log `*-strict.log` có đúng
1 `E` mỗi tệp, 6 log `*-old.log` có 0 `E` — **bằng chứng thật**, nhưng **ngoài repo**. Báo cáo
§11 mới chép `kt171-measure.log` + 2 log TAP 37 vào `kt171-logs/`, **chưa** chép 6 cặp log đột
biến KT-171. Đề nghị lead chép nốt (một commit riêng) — **không** tự làm vì ngoài phạm vi task.

---

## 5. Lớp lỗi nặng nhất — tự ĐẠT vs reviewer KHÔNG ĐẠT

| Nhật ký | Tự tuyên bố | Reviewer chấm | Bằng chứng reviewer |
|---|---|---|---|
| **KT-165** (L8) | §1: mọi mục "ĐẠT"; §5 coi macro/`include!`/`stringify!` là "lý thuyết, không có trong cây" | **L11 (KT-168): KHÔNG ĐẠT** | 5 lớp lọt (B4, B6, B7, B8/B9, B10); B8/B9 là module **sản phẩm** tên `ktest_evil.rs` / `src/tests/` — `cargo check` PASS, `--strict` `errors=0` (`kt168-logs/b8-ktest-name.log`, `b9-src-tests-dir.log`) |
| **KT-171** (L14) | §0: "6/6 ĐÃ ĐÓNG"; "Không lớp nào phải tuyên bố 'chưa đóng'" | **L16 (KT-174): KHÔNG ĐẠT** | 8 cách lọt (H1–H8); H5: `include!("evil_b.rs")` từ `core_sched.rs` (sản phẩm) + `#[cfg(test)] mod evil_b;` ⇒ `cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf` **RC=0**, `--strict` **errors=0** |

**Vì sao đây là lớp lỗi quan trọng nhất**: cả hai nhật ký đều **không bịa** — 6 lỗ hổng được
giao của KT-165 và 6 lớp B4/B6/B7/B8/B9/B10 của KT-171 **đóng thật**, có đột biến đỏ thật. Lỗi
nằm ở **bước nhảy kết luận**: từ "6/6 mục được giao đóng" nhảy sang "ĐẠT", trong
khi **mục tiêu** của luật là "**mọi** tham chiếu `wake` trong **mã sản phẩm** ⇒ ĐỎ". Reviewer chỉ
cần một đường **đơn giản** (đổi tên tệp, `include!` một dòng) là phá được. Đúng bài học báo cáo
§7: "mỗi lần tác giả tự kiểm thì xanh, reviewer độc lập thì đỏ".

**Lớp nhẹ hơn (ĐẠT vs ĐẠT CÓ ĐIỀU KIỆN)** — cùng bản chất nhưng reviewer vẫn cho qua có điều
kiện: KT-158 (L2), KT-159 (L3), KT-160 (L4), KT-167 (L10). Trong bốn cái này, **KT-167** nặng
nhất vì kèm **một claim sai sự thật** ("đủ 39 trường").

---

## 6. Bằng chứng **đột biến đỏ** — kiểm từng nhật ký

| Nhật ký | "Đã đóng/ĐẠT" dựa trên | Đột biến đỏ thật? | Ghi chú |
|---|---|---|---|
| KT-157 | TAP 37 + sổ `(client,va)` | **CÓ** — `kt171-logs/kt157-red-mutation-smp1.log:257` `faults=15 … ok=0`; TAP 30..36 vẫn xanh | Đột biến đỏ đúng chỗ; log đã chép vào repo |
| KT-158 | 14 phép kiểm + `pwd` hằng `/` | **CÓ** — `kt158-logs/kt158-shell-edge-mut.log` 1 `FAIL`, 13 `PASS` | Chỉ 1 đột biến; đột biến `Cwd` **không thử** (L6 phát hiện) |
| KT-159 | luật checker + 6 test ghim | **CÓ** — `kt159-strict-mutation.log` `errors=1`; `kt159-test-bites.log` 2 đột biến ngữ nghĩa `FAILED` | Đột biến đỏ thật; nhưng phạm vi luật hẹp (L6) |
| KT-160 | 22 test + 3 đột biến | **KHÔNG CÓ LOG** — 3 đột biến chỉ nằm trong nhật ký; reviewer L6 chạy **4 đột biến khác** đều đỏ (`kt161-logs/kt160-mutations.log`) | Bằng chứng **gián tiếp** qua reviewer; không có log tác giả |
| KT-161 | review 3 task | **CÓ** — M1 `FAIL`, M3/M4 `4 passed; 2 failed`, 6 bypass `errors=0` | Reviewer tự chạy, log đầy đủ |
| KT-163 | 4 lớp + 2 test giả | **CÓ** — `kt163-check-mutation.log` `2 bước FAIL`; `kt163-mutations.log` M1–M4 `exit=1` | Đột biến đỏ thật |
| KT-164 | trait dùng chung + 2 đột biến | **KHÔNG CÓ LOG** — không có `kt164-logs/` | Chỉ có nhật ký; không tái lập được từ repo |
| KT-165 | 6 lỗ hổng H1–H6 | **CÓ** — `m3-sau-cach-lach.log` 6 `E`; checker cũ `errors=0`; 7 ca selftest chết | Đột biến đỏ thật cho **6 mục được giao**; nhưng "ĐẠT" bị L11 bác |
| KT-166 | 15 phép kiểm + 3 đột biến | **CÓ** (qua reviewer) — `kt168-logs/shell-edge-mut1.log` 4 `FAIL`; `a64-mut-{attr,ng,table-at-leaf}.log` `FAILED` | Tác giả không có log riêng; reviewer tái lập độc lập |
| KT-167 | 43 test + 3 đột biến | **CÓ** — `kt167-mut{1,2,3}*.log` `FAILED` (2/2/1) | Đột biến đỏ thật nhưng **thiếu** 14 hằng chết + 3 setter (L17) |
| KT-168 | review vòng 2 | **CÓ** — B8/B9 `errors=0` (lỗ hổng), 4 bit mutation `FAILED` | Reviewer tự chạy |
| KT-170 | lớp (e) + bản đồ trợ giúp | **CÓ** — `kt170-check-mutation.log` `E_SHOULD_PANIC_BARE`, `1 bước FAIL` | Đột biến đỏ thật |
| KT-171 | 6 lớp B4/B6/B7/B8/B9/B10 | **CÓ** (ngoài repo) — `/tmp/kt171-mut/logs/*-strict.log` 6 `E`; `*-old.log` 0 `E` | Bằng chứng thật nhưng **không commit**; reviewer L16 phá thêm 8 lớp |
| KT-172 | 4 call site + 2 đột biến | **CÓ** — `kt172-mutation-A/B.log` `errors=2`; checker cũ `errors=0` | Đột biến đỏ thật |
| KT-174 | review vòng 3 | **CÓ** (ngoài repo) — `/tmp/kt174-mut/logs/` 13 cặp log; 9 log `errors=0` (lỗ hổng), 4 log `errors=1` (đã đóng) | Bằng chứng thật nhưng **không commit**; không có bảng ánh xạ H↔m |
| KT-175 | review KT-167 | **CÓ** — `kt175-mutations.log` 64 đột biến, 21 XANH/43 ĐỎ; 6/6 TLBI, 13/13 barrier | Đầy đủ, log trong repo |
| KT-176 | audit báo cáo | **KHÔNG CẦN** — audit đọc-only; `v2 check` PASS không log | — |

**Nhận xét**: 12/17 nhật ký có đột biến đỏ **thật** trong repo hoặc qua reviewer; **KT-160 và
KT-164 không có log đột biến nào**; **KT-171 và KT-174 có đột biến thật nhưng ngoài repo**. Như
vậy câu "đã đóng bằng đột biến đỏ" **đúng về bản chất** ở phần lớn nhật ký, nhưng **không phải
mọi** con số đều có dòng log trong `kt1*-logs/` — đúng 25 chỗ ở §4.

---

## 7. Phán quyết nhật ký vs phán quyết reviewer

| Task | Nhật ký tự chấm | Reviewer chấm | Nhất quán? |
|---|---|---|---|
| KT-157 | ĐẠT (5/5 vế) | **không có reviewer** | — (không kiểm được) |
| KT-158 | ĐẠT (5/5 vế) | L6: **ĐẠT CÓ ĐIỀU KIỆN** | **KHÔNG** — nhật ký mạnh hơn |
| KT-159 | ĐẠT (8/8 mục) | L6: **ĐẠT CÓ ĐIỀU KIỆN** | **KHÔNG** |
| KT-160 | ĐẠT bước 1 | L6: **ĐẠT CÓ ĐIỀU KIỆN** | **KHÔNG** |
| KT-161 | (reviewer) 3 task ĐẠT CÓ ĐIỀU KIỆN | L11/L16/L17 không review lại KT-161 | — |
| KT-163 | ĐẠT cả 4 lớp | **không có reviewer** | — |
| KT-164 | ĐẠT bước 2–3 | **không có reviewer** | — |
| KT-165 | **ĐẠT** | L11: **KHÔNG ĐẠT** | **KHÔNG — nặng** |
| KT-166 | ĐÓNG | L11: **ĐẠT** | **CÓ** |
| KT-167 | **ĐẠT** | L17: **ĐẠT CÓ ĐIỀU KIỆN** | **KHÔNG** |
| KT-168 | (reviewer) KT-165 KHÔNG ĐẠT, KT-166 ĐẠT | Báo cáo §6/§7 khớp | **CÓ** |
| KT-170 | ĐÓNG | **không có reviewer** (Agent 0 bắt lỗi vòng 1, tác giả đã siết) | — |
| KT-171 | **ĐẠT / 6/6 đóng** | L16: **KHÔNG ĐẠT** | **KHÔNG — nặng** |
| KT-172 | XONG | **không có reviewer** | — |
| KT-174 | (reviewer) KT-171 KHÔNG ĐẠT | Báo cáo §6/§7 khớp | **CÓ** |
| KT-175 | (reviewer) KT-167 ĐẠT CÓ ĐIỀU KIỆN | Báo cáo §6 khớp | **CÓ** |
| KT-176 | (audit) 8 lệch | Báo cáo §11 khớp 8 lệch | **CÓ** |

**Kết luận**: 4 task có reviewer chấm **thấp hơn** nhật ký (KT-158/159/160/167 — ĐẠT vs ĐẠT CÓ
ĐIỀU KIỆN) và **2 task reviewer chấm ngược hẳn** (KT-165, KT-171 — ĐẠT vs KHÔNG ĐẠT). Không có
trường hợp nào reviewer chấm **cao hơn** nhật ký. Điều này khớp với kết luận của L18: báo cáo
trung thực về định tính nhưng **kế thừa** kết luận tự chấm của tác giả ở các mục chưa review.

---

## 8. `./v2/bin/v2 check`

```text
$ ./v2/bin/v2 check
...
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
RC=0
```

Log đầy đủ: `/tmp/kt179-v2check.log` (ngoài repo; không commit vì là log máy). Không chạy
`gate --fast` (luật task cấm).

---

## 9. Luật đã giữ

- Không merge, không push, không `gate --fast`, không tạo worktree mới, không QEMU.
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`, `BAO-CAO-DEM-2026-11-19-VONG2.md`, nhật ký của người khác.
- Không `pkill -f qemu-system`; task không dùng QEMU.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không
  `local a=1 b="$a"`.
- Commit `[KT-179] …` trên nhánh riêng với `VIFAST_ROLE=lead` (chỉ để hook cho phép commit).

---

## Phụ lục A — Danh sách 152 khẳng định đã kiểm

Ký hiệu: **K** khớp, **N** không nguồn trong repo, **L** lệch. Nguồn ghi tắt: `J/` =
`v2/agent/JOURNAL/`, `T/` = `/tmp` (ngoài repo), `G/` = mã nguồn.

### A.1 KT-157 (8)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 1 | TAP 37 `clients=4 deliveries=14 faults=14 served=13 rejected=1 expected_va=11 distinct_va=11 ok=1` | `J/kt171-logs/kt157-green-smp1.log:257` | K |
| 2 | TAP 30..36 vẫn xanh dưới đột biến | `J/kt171-logs/kt157-red-mutation-smp1.log:233-240` | K |
| 3 | Đột biến `faults=15 … ok=0` | `J/kt171-logs/kt157-red-mutation-smp1.log:257` | K |
| 4 | `7/7 boot sạch` smp=1 | chỉ `J/kt157-pager4.md:160-170` | N |
| 5 | `OBJ_ARENA_BYTES 64→96 KiB` | `git show 6dadf4a1 -- …/obj.rs` | K |
| 6 | `MAX_PROCS 4→6` | `git show 6dadf4a1 -- …/user.rs` | K |
| 7 | `v2 check` PASS | chỉ `J/kt157-pager4.md:187-194` | N |
| 8 | "ma trận 21/21" (báo cáo gán cho L1) | `J/kt171-logs/kt171-measure.log:486` (lead chạy sau merge); nhật ký nói **chưa chạy** | L |

### A.2 KT-158 (11)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 9 | ca 16 `11 → 14` phép kiểm | `J/kt158-logs/kt158-shell-edge-green.log` = 14 dòng | K |
| 10 | `pwd` đòi 8 dòng `/` + 2 dòng `/docs` | `G/v2/test/boot/shell-edge.sh:1035-1043` | K |
| 11 | `cd-thu-tu` so 11 dòng | `G/v2/test/boot/shell-edge.sh:1085-1095` | K |
| 12 | `--selftest 42/42` (23 sai→ĐỎ + 19 đúng→XANH) | `J/kt158-logs/kt158-v2-check-final.log:104-110` | K |
| 13 | đột biến `pwd` hằng `/` ⇒ `FAIL` "thấy 10" | `J/kt158-logs/kt158-shell-edge-mut.log` | K |
| 14 | 13 phép kiểm còn lại xanh | `J/kt158-logs/kt158-shell-edge-mut.log` = 13 `PASS` | K |
| 15 | `used=4` | `J/kt158-logs/run-clean/qemu.log:87` | K |
| 16 | ảnh `sha256=735e1030… bytes=209536` | `J/kt158-logs/kt158-shell-edge-clean.log:3` | K |
| 17 | `inputs=0dd9e10ec24a0024 dirty=0` | `J/kt158-logs/kt158-shell-edge-clean.log:4` | K |
| 18 | `v2 check` PASS | `J/kt158-logs/kt158-v2-check-final.log` | K |
| 19 | "Không còn nợ kỹ thuật nào" | `J/kt161-review.md:21` (M2 vẫn XANH) | L |

### A.3 KT-159 (9)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 20 | `--selftest 15/15` | `J/kt159-logs/kt159-selftest.log` | K |
| 21 | 6 test ghim `6 passed` | `J/kt159-logs/kt159-test-pin.log:6,14` | K |
| 22 | đột biến `errors=1` @ `core_ipc.rs:697` | `J/kt159-logs/kt159-strict-mutation.log:13` | K |
| 23 | bẫy thật: `6 passed; 1 failed` | `J/kt159-logs/kt159-trap-real.log:12` | K |
| 24 | `v2 check` PASS | `J/kt159-logs/kt159-check.log` | K |
| 25 | `wake()` không có call site sản phẩm | `G/grep` — chỉ doc-comment | K |
| 26 | baseline `11/11` | chỉ `J/kt159-wake2.md:49` | N |
| 27 | 4 ca selftest mới | `J/kt159-logs/kt159-selftest.log:9-12` | K |
| 28 | "Không còn nợ kỹ thuật" | `J/kt161-review.md:22` (6 lỗ hổng) | L |

### A.4 KT-160 (10)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 29 | `22 passed` | `J/kt161-logs/kt160-baseline.log:11,35` | K |
| 30 | đột biến bỏ `AF` ⇒ `2 failed` | chỉ `J/kt160-aarch64.md:192-197` | N |
| 31 | đột biến `unmap` không `free_frame` ⇒ `4 failed` | chỉ `J/kt160-aarch64.md:203-210` | N |
| 32 | đột biến đảo `AP[2]` ⇒ `3 failed` | chỉ `J/kt160-aarch64.md:216-222` | N |
| 33 | L0 `115 passed` | chỉ `J/kt160-aarch64.md:230-261` | N |
| 34 | `v2 check` PASS | chỉ `J/kt160-aarch64.md:265-274` | N |
| 35 | build `Compiling vifast-plat-aarch64` | `J/kt161-logs/kt160-build-aarch64.log:1` | K |
| 36 | rlib `218 360 B` | chỉ `J/kt161-review.md:196` | N |
| 37 | trait không tồn tại ở `6b2fdce7` | `git show 6b2fdce7:…` — grep 0 | K |
| 38 | "ĐẠT bước 1" | `J/kt161-review.md:23` (ĐẠT CÓ ĐIỀU KIỆN) | L |

### A.5 KT-161 (13)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 39 | 14/14 ca 16 PASS trên ảnh sạch | `J/kt161-logs/kt158-clean.log` = 14 | K |
| 40 | M1 `pwd` ⇒ `FAIL` | `J/kt161-logs/kt158-m1.log` | K |
| 41 | M2 `resolve_path` bỏ `Cwd` ⇒ XANH | `J/kt161-logs/kt158-m2.log` | K |
| 42 | `--selftest 15/15` | `J/kt161-logs/kt159-selftest.log` | K |
| 43 | selftest bị phá ⇒ `14/15` | `J/kt161-logs/kt159-selftest-broken.log:16` | K |
| 44 | M3 `4 passed; 2 failed` | `J/kt161-logs/kt159-m3-wake-post-ready.log:36` | K |
| 45 | M4 `4 passed; 2 failed` | `J/kt161-logs/kt159-m4-wake-from-no-mail.log:36` | K |
| 46 | 6 lỗ hổng lách luật | `J/kt161-logs/kt159-bypass.log` = 6 dòng `errors=0` | K |
| 47 | 22 test host | `J/kt161-logs/kt160-baseline.log` | K |
| 48 | 4 đột biến encoder đỏ | `J/kt161-logs/kt160-mutations.log` | K |
| 49 | `v2 check` PASS | `J/kt161-logs/v2-check-clean.log` | K |
| 50 | rlib `218 360 B` | chỉ `J/kt161-review.md:196` | N |
| 51 | nhãn H1–H6 | `J/kt161-logs/kt159-bypass.log` dùng B1–B6 khác thứ tự | L (nhãn) |

### A.6 KT-163 (9)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 52 | `--selftest 21/21` | `J/kt163-logs/kt163-selftest.log` | K |
| 53 | 944 `#[test]` | `J/kt163-logs/kt163-scan.log` | K |
| 54 | 124 tệp có phạm vi test | `J/kt163-logs/kt163-scan.log` | K |
| 55 | 20 mục allowlist | `J/kt163-logs/kt163-scan.log` | K |
| 56 | đột biến `assert!(true)` ⇒ `2 bước FAIL` | `J/kt163-logs/kt163-check-mutation.log:309,388` | K |
| 57 | 4 đột biến lớp khác `exit=1` | `J/kt163-logs/kt163-mutations.log` | K |
| 58 | 2 test giả đã sửa | `G/v2/lib/vifast-pte/tests/kt35_pte.rs:203-206`; `G/v2/sim/vifast-sim/src/vspace.rs:284-288` | K |
| 59 | quét `11,7 s → 2,6 s` | chỉ `J/kt163-teeth.md:239-242` | N |
| 60 | `291/291` tệp giống hệt | chỉ `J/kt163-teeth.md:241-242` | N |

### A.7 KT-164 (7)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 61 | `22 passed` aarch64 | chỉ `J/kt164-shared.md:87-90` | N |
| 62 | L0 `118 passed` | chỉ `J/kt164-shared.md:36,109` | N |
| 63 | 3 test `mmu_shared` | chỉ `J/kt164-shared.md:113-121` | N |
| 64 | 2 đột biến đỏ | chỉ `J/kt164-shared.md:127-158` | N |
| 65 | `v2 check` PASS | chỉ `J/kt164-shared.md:162-170` | N |
| 66 | đúng **một** định nghĩa `trait PageTableFormat` | `G/v2/lib/vifast-pte/src/mmu.rs:253` | K |
| 67 | `Sv39` impl `Mmu` | `G/v2/kernel/vifast-plat-common/src/mmu.rs` | K |

### A.8 KT-165 (9)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 68 | `--selftest 26/26` | `J/kt165-logs/selftest.log` | K |
| 69 | 6 dòng `E_STRICT_TEST_ONLY_WAKE` | `J/kt165-logs/m3-sau-cach-lach.log` | K |
| 70 | checker cũ `errors=0` trên cùng cây | `J/kt165-logs/m3-checker-cu-xanh.log` | K |
| 71 | 7 ca selftest chết khi gỡ vá | `J/kt165-logs/selftest-teeth.log` | K |
| 72 | `v2 check` PASS | `J/kt165-logs/v2-check.log` | K |
| 73 | M1 @ `proc_rt.rs:215` | `J/kt165-logs/m1-proc-rt.log` | K |
| 74 | M2 alias @ `core_sched.rs:247` | `J/kt165-logs/m2-alias.log` | K |
| 75 | 6/6 lỗ hổng được giao đóng | `J/kt165-logs/m3-sau-cach-lach.log` | K |
| 76 | "Không còn nợ kỹ thuật" / ĐẠT | `J/kt168-review2.md:13` (KHÔNG ĐẠT) | L |

### A.9 KT-166 (8)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 77 | 15 phép kiểm ca 16 | `J/kt168-logs/shell-edge-clean.log` = 15 | K |
| 78 | đột biến `Cwd` ⇒ 4 `FAIL` | `J/kt168-logs/shell-edge-mut1.log` | K |
| 79 | 24 test host | `J/kt168-logs/a64-baseline.log:38` | K |
| 80 | 3 đột biến bắt buộc đỏ | `J/kt168-logs/a64-mut-{attr,ng,table-at-leaf}.log` | K |
| 81 | `--selftest 42/42` | `J/kt168-logs/shell-edge-selftest.log:44` | K |
| 82 | `v2 check` PASS | không có log tác giả; `J/kt168-logs/v2-check-clean.log` là của reviewer | N |
| 83 | L0 hai crate `-p … -p …` | `G/v2/bin/v2:427` | K |
| 84 | "ĐÓNG" | `J/kt168-review2.md:14` (ĐẠT) | K |

### A.10 KT-167 (11)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 85 | `43 passed` (24+15+4) | `J/kt167-logs/kt167-host-test.log` | K |
| 86 | 3 đột biến đỏ (2/2/1) | `J/kt167-logs/kt167-mut{1,2,3}*.log` | K |
| 87 | `v2 check` PASS | `J/kt167-logs/kt167-v2-check.log` | K |
| 88 | build `Finished` | `J/kt167-logs/kt167-aarch64-build.log` | K |
| 89 | 11 mã objdump | `J/kt167-logs/kt167-aarch64-objdump.log` = 11 | K |
| 90 | `TCR_EL1` 17 trường | `G/src/sysreg.rs` 17 `_SHIFT`; L17 xác nhận 17/17 | K |
| 91 | `MAIR_EL1` 8 slot | `G/src/sysreg.rs`; L17 xác nhận 8/8 | K |
| 92 | "cài **đủ 39 trường** `SCTLR_EL1`" | `J/kt175-a64review.md:144-167` (thiếu ≥8) | L |
| 93 | `TLBI` 6 mã | `G/src/tlb.rs`; L17 xác nhận 6/6 | K |
| 94 | barrier 13 encoding | `G/src/barrier.rs` 13; L17 xác nhận 13/13 | K |
| 95 | "ĐẠT" toàn bộ | `J/kt175-a64review.md:12-21` (ĐẠT CÓ ĐIỀU KIỆN) | L |

### A.11 KT-168 (9)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 96 | KT-165 **KHÔNG ĐẠT** | `J/kt168-review2.md:13`; báo cáo §6 | K |
| 97 | KT-166 **ĐẠT** | `J/kt168-review2.md:14`; báo cáo §6 | K |
| 98 | 5 lớp lọt (B4, B6, B7, B8/B9, B10) | `J/kt168-review2.md:134` | K |
| 99 | 36 tệp log | `ls J/kt168-logs` = 36 | K |
| 100 | `--selftest 26/26` | `J/kt168-logs/selftest.log` | K |
| 101 | `v2 check` PASS | `J/kt168-logs/v2-check-clean.log` | K |
| 102 | L0 24 test | `J/kt168-logs/l0-kernel-workspace.log:41` | K |
| 103 | B8/B9 là lỗ hổng thật | `J/kt168-logs/b8-ktest-name.log`, `b9-src-tests-dir.log` `errors=0` | K |
| 104 | 4 đột biến bit đỏ | `J/kt168-logs/a64-mut-{af,pxn,uxn,sh}.log` | K |

### A.12 KT-170 (9)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 105 | `--selftest 33/33` | `J/kt170-logs/kt170-selftest2.log` | K |
| 106 | 977 `#[test]` | `J/kt170-logs/kt170-scan2.log` | K |
| 107 | 127 tệp | `J/kt170-logs/kt170-scan2.log` | K |
| 108 | 80 tên trong bản đồ | `J/kt170-logs/kt170-scan2.log` | K |
| 109 | 319 → 80 | `J/kt170-logs/kt170-scan.log:5` vs `scan2` | K |
| 110 | vi phạm 0 → 0 | `scan.log`/`scan2.log` `errors=0` | K |
| 111 | đột biến `E_SHOULD_PANIC_BARE` | `J/kt170-logs/kt170-check-mutation.log:316,408` | K |
| 112 | `v2 check` PASS | `J/kt170-logs/kt170-check-final3.log:408` | K |
| 113 | quét `2,64 → 3,11 s` | chỉ `J/kt170-teeth2.md:271-273` | N |

### A.13 KT-171 (8)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 114 | `--selftest 39/39` | `T/kt171-mut/logs/v2-check-postcommit.log:287` (ngoài repo) | N |
| 115 | 6 đột biến đỏ | `T/kt171-mut/logs/{b4,b6,b7,b8,b9,b10}-strict.log` = 1 `E` mỗi tệp | K |
| 116 | checker KT-165 ⇒ cả 6 `errors=0` | `T/kt171-mut/logs/*-old.log` = 0 `E` | K |
| 117 | `v2 check` PASS | `T/kt171-mut/logs/v2-check-postcommit.log` | K |
| 118 | 97 tệp test-only | chỉ `J/kt171-wakefix3.md:151-156` | N |
| 119 | `--strict` `4,6 → 6,0 s` | chỉ `J/kt171-wakefix3.md:157-158` | N |
| 120 | 6/6 lớp đóng | `J/kt174-review3.md:9-30` (KHÔNG ĐẠT) | L |
| 121 | "Không lớp nào phải tuyên bố chưa đóng" | `J/kt174-review3.md:9-30` (8 lớp) | L |

### A.14 KT-172 (7)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 122 | `--selftest 46/46` | `J/kt172-logs/kt172-selftest.log` | K |
| 123 | 2 đột biến `errors=2` | `J/kt172-logs/kt172-mutation-A/B.log` | K |
| 124 | checker cũ `errors=0` | `J/kt172-logs/kt172-mutation-A/B-truoc-fix.log` | K |
| 125 | `v2 check` PASS | `J/kt172-logs/kt172-v2-check.log` | K |
| 126 | 97 vs 94 tệp test-only | chỉ `J/kt172-testfile.md:74-76` | N |
| 127 | `E_STRICT_STALE errors=2` | chỉ `J/kt172-testfile.md:58-59` | N |
| 128 | 4 call site đổi sang `is_test_file` | `G/v2/tools/check-wake-paths.py:1181,1242,1277,1328` | K |

### A.15 KT-174 (7)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 129 | 8 cách lọt H1–H8 | `T/kt174-mut/logs/` 9 log `errors=0` (không có bảng ánh xạ 1:1) | K |
| 130 | H5 nặng nhất | `T/kt174-mut/logs/m5-strict.log` `errors=0`; `m5b-kbin.log` | K |
| 131 | C1–C5 đã đóng | `T/kt174-mut/logs/m6,m7,m10,m12b` `errors=1` | K |
| 132 | `v2 check` PASS | `T/kt174-mut/logs/v2check.log` | K |
| 133 | KT-171 **KHÔNG ĐẠT** | `J/kt174-review3.md:9-30`; báo cáo §6 | K |
| 134 | 2 cách chỉ dùng tệp trong cây | H5/H8 trong `J/kt174-review3.md:146-288` | K |
| 135 | log không commit | `J/kt174-review3.md:53` | K |

### A.16 KT-175 (11)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 136 | 64 phép đột biến | `J/kt175-logs/kt175-mutations.log` = 64 dòng | K |
| 137 | 21 XANH | `kt175-mutations.log` = 21 | K |
| 138 | 43 ĐỎ | `kt175-mutations.log` = 43 | K |
| 139 | baseline 43 passed | `kt175-mutations.log:2` | K |
| 140 | 6/6 TLBI khớp clang | `J/kt175-logs/kt175-clang-tlbi.log` | K |
| 141 | 13/13 barrier khớp | `J/kt175-logs/kt175-clang-tlbi.log` | K |
| 142 | build `Compiling` + rlib 625242 | `J/kt175-logs/kt175-build.log` | K |
| 143 | 14 hằng chết | `G/src/sysreg.rs`; `grep` chỉ thấy `pub use` | K |
| 144 | 3/3 hoán vị setter XANH | `kt175-mutations.log` | K |
| 145 | 3/3 stub no-op XANH | `kt175-mutations.log` | K |
| 146 | KT-167 ĐẠT CÓ ĐIỀU KIỆN | `J/kt175-a64review.md:12-21`; báo cáo §6 | K |

### A.17 KT-176 (6)

| # | Khẳng định | Nguồn | Kết quả |
|---|---|---|---|
| 147 | 64 khẳng định | `J/kt176-reportaudit.md` bảng §2 = 64 dòng | K |
| 148 | 56 khớp | `J/kt176-reportaudit.md:14` | K |
| 149 | 8 lệch/không nguồn | `J/kt176-reportaudit.md:15`; báo cáo §11 = 8 dòng | K |
| 150 | `v2 check` PASS | chỉ `J/kt176-reportaudit.md:266-269` | N |
| 151 | STATE đã cập nhật | `git show f02a8588 -- v2/agent/STATE.md` | K |
| 152 | 6 nhóm nguồn ngoài repo | `J/kt176-reportaudit.md:218-235` | K |

---

## Phụ lục B — Việc **không** làm

- Không sửa bất kỳ nhật ký nào của người khác (kể cả các chỗ lệch ở §3) — lead quyết.
- Không sửa báo cáo, `DECISIONS.md`, `QUEUE.md`, `STATE.md`.
- Không chép log từ `/tmp` vào repo (ngoài phạm vi; đề nghị lead làm ở commit riêng).
- Không chạy lại đột biến của bất kỳ task nào (chỉ đọc log đã có); không QEMU.
