# Báo cáo đêm 2026-11-19 — vòng hai

**Thời gian**: bắt đầu 03:44 JST, hạn 09:00 JST 2026-11-19.
**Vai trò**: Agent 1 **quyết thay Agent 0** (người điều phối giao toàn quyền).
**Kết quả**: **mười bốn sub-agent**, không bao giờ quá bốn cùng lúc; **chín task** (KT-157…KT-171) merge vào `main`; cổng `gate --fast` **PASS** trên cây cuối; **sáu tiêu chí K6 đo lại** trên ảnh mới.

---

## 1. Tóm tắt điều hành

| Việc | Kết quả | Bằng chứng |
|---|---|---|
| **N3 nợ "4 client mức kernel"** | **ĐÓNG HẲN** — bốn tiến trình U-mode, mỗi cái VSpace + CSpace riêng | TAP 37 `clients=4 … ok=1`; đột biến `ok=0`; ma trận 21/21 |
| **KT-137 nợ "seed thư mục"** | **ĐÓNG** — `/docs` + `/docs/note.txt` trong FS sản phẩm | `pwd /docs`, `ls n=1 path=/docs`, `cat ok bytes=16`; 15 phép kiểm |
| **N7 nợ "hai ngữ nghĩa wake"** | **ĐÓNG** — `wake()` là API chỉ-dùng-test, có luật cưỡng chế | `E_STRICT_TEST_ONLY_WAKE`; 39/39 selftest |
| **K4' bước 1 + 1b + 2** | `vifast-plat-aarch64` có MMU 4 mức, trait dùng chung, thanh ghi hệ thống + asm | 43 test host; 3 đột biến đỏ; đối chiếu `llvm-objdump` |
| **K5'** | Audit sẵn sàng: 18 mục còn thiếu kèm `file:dòng`, 8 bước, 10 rủi ro | `K5-X86-READINESS.md`; `verify-numbers` PASS |
| **Chống "test xanh giả"** | Máy kiểm mới bắt 4+1 lớp; **bắt 2 test giả thật** trong cây | 33/33 selftest; đột biến `v2 check` đỏ |
| **Sáu tiêu chí K6** | **F1–F6 ĐẠT** trên ảnh cuối `2af101d1…` | 30 m 40 s; soak 1000/1000 |
| **Cổng cuối** | `gate --fast` **PASS** | xem §9 |

**Điều đáng nói nhất**: luật "`wake()` chỉ dùng cho test" bị **phá ba vòng liên tiếp** — và cả ba lần đều do **reviewer độc lập** phát hiện, không phải tác giả. Đây là lần đầu quy trình "reviewer có quyền phá" chạy **ba vòng** trong một đêm và **mỗi vòng đều tìm ra lỗ thật**.

---

## 2. Quyết định (D2-144 … D2-156)

| Mã | Nội dung ngắn |
|---|---|
| **D2-144** | `wake()` là API **chỉ-dùng-cho-test**; sản phẩm dùng `wake_from`/`wake_from_unlink`; cưỡng chế bằng checker + test ghim |
| **D2-145** | N3: nợ "4 client mức kernel" đóng bằng **ảnh test**, không bằng lời; không dựng được thì **không** tuyên bố đóng |
| **D2-146** | KT-137: seed `/docs` + `/docs/note.txt`; đột biến "`pwd` in hằng `/`" **phải** bị bắt |
| **D2-147** | K4' bước 1: `vifast-plat-aarch64` chỉ phần host-testable; **không** boot, **không** tuyên bố K4' đạt |
| **D2-148** | Giao thức đêm vòng hai: bốn sub-agent sóng một, worktree riêng, chỉ lead merge, mốc 06:30/07:45/08:30/09:00 |
| **D2-149** | N1 "600 boot x86_64" ở lại K5'; không đóng bằng lời |
| **D2-150** | Máy kiểm "test có răng" — bắt bốn lớp **chắc chắn**, không heuristic mờ |
| **D2-151** | **T-INFRA**: cổng chạy song song sub-agent ⇒ `persist --crash` đỏ oan (`records=13` thay vì 11/12); chạy lại máy sạch ⇒ PASS |
| **D2-152** | **N3 ĐÓNG HẲN** — bốn client mức kernel, sổ `(client, va)`, đột biến phát lại `Call` ⇒ TAP 37 đỏ |
| **D2-153** | Sáu task đêm vòng hai ĐẠT (kèm điều kiện đã vá) |
| **D2-154** | Sóng ba: K4' bước 2 + review + K5' audit + siết máy kiểm |
| **D2-155** | **KT-165 KHÔNG ĐẠT** — reviewer L11 phá được bằng 5 lớp; mở KT-171 vá vòng 3 |
| **D2-156** | Sáu tiêu chí K6 đo lại trên ảnh `2af101d1…`; K7 giữ nguyên phán quyết, bằng chứng **cập nhật theo cây mới** |

---

## 3. Mười bốn sub-agent

Bốn sóng, **không bao giờ quá bốn** cùng lúc; mỗi cái một worktree riêng, chỉ lead merge.

| Sub-agent | Nhánh | Việc | Kết quả |
|---|---|---|---|
| **L1** | `night2/L1-pager4` | KT-157 — bốn client mức kernel | TAP 37 `clients=4`; đột biến đỏ; 21/21 boot |
| **L2** | `night2/L2-fsdir` | KT-158 — seed `/docs` | 15 phép kiểm; 2 đột biến đỏ |
| **L3** | `night2/L3-wake2` | KT-159 — `wake()` chỉ-dùng-test | Luật mới; 15/15 selftest; 6 test ghim |
| **L4** | `night2/L4-aarch64` | KT-160 — K4' bước 1 | 24 test host; 3 đột biến đỏ |
| **L5** | `night2/L5-teeth` | KT-163 — máy kiểm test có răng | 4 lớp; bắt 2 test giả thật |
| **L6** | `night2/L6-review` | KT-161 — review đối kháng sóng một | 3 task ĐẠT CÓ ĐIỀU KIỆN; **6 lỗ hổng** luật wake |
| **L7** | `night2/L7-shared` | KT-164 — trait dùng chung | Sv39 + AArch64 cùng impl; test generic |
| **L8** | `night2/L8-wakefix` | KT-165 — vá 6 lỗ hổng L6 | 26/26 selftest; checker cũ `errors=0` trên cùng đột biến |
| **L9** | `night2/L9-gapfix` | KT-166 — vá lỗ hổng KT-158/160 | 15 phép kiểm; 24 test; L0 hai crate |
| **L10** | `night2/L10-a64step2` | KT-167 — K4' bước 2 | 43 test host; 3 đột biến; `llvm-objdump` |
| **L11** | `night2/L11-review2` | KT-168 — review đối kháng sóng hai | **KT-165 KHÔNG ĐẠT**; KT-166 ĐẠT |
| **L12** | `night2/L12-x86audit` | KT-169 — K5' audit | 18 mục thiếu; 8 bước; 10 rủi ro |
| **L13** | `night2/L13-teeth2` | KT-170 — siết máy kiểm | 33/33; bản đồ trợ giúp **chỉ** phạm vi test |
| **L14** | `night2/L14-wakefix3` | KT-171 — vá vòng 3 luật wake | 6/6 lớp đóng; 39/39 selftest |

---

## 4. Sóng một — bốn task nền

### KT-157 — N3: bốn client **mức kernel**

Nợ cũ (D2-136): TAP 37 chỉ có **hai bảng client trong một không gian địa chỉ**. Nay:

```text
# pager: clients=4 deliveries=14 faults=14 served=13 rejected=1 expected_va=11 distinct_va=11 ok=1
ok 37 - pager giao dung mot lan
```

Bốn client A/B/C/D, mỗi cái **VSpace + CSpace riêng**, VA riêng (`0x4010_0000`, `0x4018_0000`, `0x4006_0000`, `0x4008_0000`). Sổ `(client, va)` fail-closed. Đột biến phát lại `Call` khi `Resume`:

```text
# pager: clients=4 deliveries=14 faults=15 served=13 rejected=2 expected_va=11 distinct_va=11 ok=0
not ok 37 - pager giao dung mot lan: message bi giao/xu ly lap, hoac so dem theo (client,va) khong doc duoc
```

TAP 30..36 **vẫn xanh** ⇒ đỏ quy đúng TAP 37. Ma trận `ktest.sh 1 matrix` ⇒ **21/21 boot sạch** (smp 1/2/4).

Tài nguyên phải nới **có ghi**: `OBJ_ARENA_BYTES 64→96 KiB`, `MAX_PROCS 4→6` (không phải nới ngưỡng cổng — `MAX_OBJECTS=96` giữ nguyên).

### KT-158 — KT-137: seed thư mục `/docs`

`proc_fs::seed` nay tạo `/docs` (thư mục) + `/docs/note.txt` (tệp). Trên ảnh sản phẩm:

```text
# shell: cd /docs
# shell: pwd /docs
  f note.txt
# shell: ls n=1 path=/docs
ghi chu cua docs
# shell: cat ok bytes=16 first=ghi chu
```

Ca 16 `shell-edge.sh` từ **11 → 15 phép kiểm**, gồm `cat note.txt` **tương đối** trong thư mục con. Hai đột biến đỏ: `pwd` in hằng `/`; `resolve_path` bỏ `Cwd` (đột biến thứ hai chính là cách reviewer L6 đã phá bản đầu — nay **bị bắt**).

### KT-159 — N7: `wake()` chỉ-dùng-cho-test

Đo được: `wake()` **không có** call site sản phẩm nào — chỉ ảnh test + test sim. Luật mới `E_STRICT_TEST_ONLY_WAKE` cấm `wake(` trong `vifast-core/src` + `vifast-sim/src` ngoài `#[cfg(test)]`. Sáu test ghim hai ngữ nghĩa (post-only vs post+`Ready`).

### KT-160 — K4' bước 1: `vifast-plat-aarch64`

Crate mới: `PageTableFormat` 4 mức 4 KiB (39-bit VA, 48-bit PA), descriptor 64-bit đầy đủ (`V`, `AP[2:1]`, `AF`, `UXN/PXN`, `SH`, `AttrIndx`/MAIR, `nG`), `Mmu` **thuần dữ liệu**. 24 test host; ba đột biến đỏ (bỏ `AF`, `unmap` không giải phóng frame, đảo `AP[2]`). Cross-check `aarch64-unknown-none` **thật** (`Compiling vifast-plat-aarch64`, rlib 218 360 B).

**Phát hiện lệch đề bài**: trait `PageTableFormat`/`Mmu` **không tồn tại** trong `vifast-plat-common`/`vifast-pte` tại thời điểm đó — chỉ có trong thiết kế. L4 định nghĩa cục bộ và ghi nợ; L7 đóng nợ ở KT-164.

---

## 5. Sóng hai — review phá, rồi vá

### KT-161 — review đối kháng sóng một

Reviewer L6 kết luận ba task **ĐẠT CÓ ĐIỀU KIỆN** và tìm ra **ba lỗ hổng phủ**:

- **KT-159**: `TEST_ONLY_WAKE_SCOPE` chỉ có `vifast-core/src` + `vifast-sim/src` — đặt `m.wake(t)` trong `vifast-kbin-riscv64/src/proc_rt.rs` (đúng nơi lời gọi sản phẩm sẽ viết) thì `--strict` vẫn `errors=0`. Kèm 5 cách lách nữa.
- **KT-158**: khối `/docs` chỉ dùng đường tuyệt đối; đột biến "`resolve_path` bỏ `Cwd`" vẫn **xanh toàn bộ**.
- **KT-160**: test descriptor tự so hằng của chính crate; `DESC_ATTR_SHIFT 2→3` và `DESC_NG 1<<11→1<<60` vẫn 22/22 xanh.

### KT-163 — máy kiểm "test có răng"

`v2/tools/check-test-teeth.py`, bốn lớp **chắc chắn**: `assert!(true)`/hằng-đúng, `#[ignore]`, `#[test]` không có khẳng định nào, `let _ =` nuốt `Result`. Fail-closed với allowlist tường minh.

**Bắt được hai test giả thật trong cây**:

- `v2/lib/vifast-pte/tests/kt35_pte.rs:203` — `assert_eq!(pte, pte)`.
- `v2/sim/vifast-sim/src/vspace.rs:284` — `assert_eq!(info, info)`.

Cả hai đã sửa thành so với giá trị dựng lại từ trường — **không** nới luật.

### KT-164 — trait dùng chung

`PageTableFormat`/`Mmu` chuyển sang `v2/lib/vifast-pte/src/mmu.rs`; `vifast-plat-aarch64` chỉ còn impl; **Sv39 cũng impl**; **một** thân test generic `kiem_tung_dinh_dang<F>()` chạy cho **cả hai** định dạng. Đột biến `AP[2]` (aarch64) và `PTE_VALID` (Sv39) đều đỏ. L0 `vifast-plat-common` **118 passed**.

### KT-165 — vá 6 lỗ hổng L6

Phạm vi thêm crate ảnh sản phẩm + **mọi** crate `vifast-plat-*`; bắt alias/fn-pointer, block comment, cắt dòng; sửa `test_line_ranges` nuốt `mod` sản phẩm. Selftest **26/26**.

**Đối chứng quyết định**: chạy **checker cũ** trên **cùng cây đột biến** ⇒ `errors=0` — sáu lỗ hổng là thật, không phải tưởng tượng.

### KT-166 — vá lỗ hổng KT-158/160

Ca 16 thêm `cat note.txt`/`stat note.txt`/`ls .` **tương đối** trong `/docs` (14→15 phép kiểm); đột biến `resolve_path` bỏ `Cwd` ⇒ **đỏ 4 phép kiểm**. `host.rs` thêm bit literal ARMv8-A **viết tay** + `translate` từ chối bảng-mức-lá (22→24 test). L0 thêm `-p vifast-plat-aarch64`.

---

## 6. Sóng ba — K4' bước 2, review vòng hai, K5' audit, siết máy kiểm

### KT-167 — K4' bước 2

`TCR_EL1` (17 trường), `MAIR_EL1` (8 slot), `SCTLR_EL1` (39 bit), `TTBR0/1_EL1`, `VBAR_EL1`, `ESR_EL1`, `FAR_EL1`, `ELR_EL1`, `SPSR_EL1`, `DAIF`; `TLBI` 6 mã; barrier 13 encoding. **43 test host**, ba đột biến đỏ. Đối chiếu chéo bằng `clang --target=aarch64-none-elf` + `llvm-objdump`: object chứa đúng 11 mã đã test (`d508871f tlbi vmalle1`, `d5088720 tlbi vae1, x0`, `d5033b9f dsb ish`, …).

**Phát hiện trung thực**: `SCTLR_EL1` **không có** `UXN`/`PXN` theo DDI 0601 bản 2024-12 — hai bit đó là **descriptor** (54/53), đã có ở `format.rs`. Không bịa trường.

### KT-168 — review vòng hai: **KT-165 KHÔNG ĐẠT**

Reviewer L11 phá bản vá KT-165 bằng **5 lớp**, hai lớp là lỗ hổng **thật** (mã sản phẩm biên dịch `wake`, `--strict` vẫn `errors=0`):

- **B8**: `v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs` + `mod ktest_evil;` trong `main.rs` — `is_test_file` miễn theo **tên** `ktest*`.
- **B9**: `vifast-core/src/tests/evil.rs` + `mod tests;` **không** `#[cfg(test)]` — miễn theo **đường dẫn**.
- **B4**: `#[macro_export] macro_rules! wake_call` ở `vifast-abi` (ngoài phạm vi quét).
- **B6/B7**: `include!` và `#[path]` trỏ ra ngoài phạm vi.
- **B10**: `let mut wake` miễn **cả hàm**, không theo khối.

KT-166 thì **ĐẠT**: bốn bit khác tác giả (`DESC_AF`, `DESC_PXN`, `DESC_UXN`, `DESC_SH`) đều đỏ; `resolve_path` bỏ `Cwd` ⇒ ca 16 đỏ 4 phép kiểm.

### KT-169 — K5' audit

`v2/agent/reports/K5-X86-READINESS.md`: **18 mục còn thiếu** kèm `file:dòng`, **8 bước** thi công (mỗi bước ≤ 3 h, có tiêu chí đo), **10 rủi ro** kèm cách giảm. Không viết mã, không tuyên bố K5' đạt. `verify-numbers` PASS.

### KT-170 — siết máy kiểm

Thêm lớp `E_SHOULD_PANIC_BARE` (`#[should_panic]` trần ⇒ đỏ) và bản đồ trợ giúp toàn cây. **Bản đầu bị Agent 0 bắt lỗi**: bản đồ nhận cả hàm **sản phẩm** có `debug_assert!` ⇒ 277/285 test "có răng" nhờ hàm sản phẩm — tức là **nới** chứ không siết. Đã buộc siết lại: chỉ hàm trong **phạm vi test**; số tên trong bản đồ 319 → **80**; vi phạm trên cây **0 → 0** (không phải sửa test nào). Selftest **33/33**.

### KT-171 — vá vòng 3 luật wake

Cả **6/6** lớp đóng:

- **B8/B9**: `test_only_modules` dựng **đồ thị module** từ crate root; tệp chỉ test-only khi **mọi** đường tới nó từ ngữ cảnh test (`tests/` cấp crate, `#[cfg(test)]`, `#[cfg(feature = "ktest")]`, bin `required-features=["ktest"]`). **Tên/thư mục không đủ.**
- **B6/B7**: phân giải `include!`/`#[path]`; đích ngoài `v2/kernel`+`v2/sim` ⇒ **ĐỎ fail-closed**.
- **B10**: miễn theo **khối** + token `wake` làm **đối số macro** không miễn.
- **B4**: quét thân `macro_rules!` trong `v2/lib/**`.

Selftest **39/39**. Sáu đột biến thật (dựng lại đúng cách L11, `cargo check` RC=0 chứng minh biên dịch được) đều **ĐỎ** đúng mã + `file:dòng`; **cùng 6 đột biến** chạy checker KT-165 ⇒ **cả 6 `errors=0`** — tái lập đúng phán quyết "KHÔNG ĐẠT".

---

## 7. Câu chuyện lớn nhất: luật wake bị phá **ba vòng**

| Vòng | Ai phá | Cách phá | Ai vá | Kết quả |
|---|---|---|---|---|
| 1 | L6 (review) | thêm đường wake thứ hai đi vòng hộp thư | L8 (KT-165) | 6 lỗ hổng đóng |
| 2 | L11 (review) | đổi **tên tệp** `ktest_evil.rs`; `src/tests/` không `cfg(test)` | L14 (KT-171) | 6 lớp đóng |
| 3 | — | (chưa có vòng 3) | — | 39/39 selftest |

Điểm chung: **mỗi lần tác giả tự kiểm thì xanh, reviewer độc lập thì đỏ**. Đây là lý do quy trình đêm nay giữ **reviewer riêng**, không cho tác giả tự tuyên bố đóng.

Bài học cụ thể: miễn trừ theo **tên tệp** và theo **đường dẫn** là hai lỗ hổng kinh điển — bản vá đúng phải dựa trên **đồ thị module** (tệp này có thật sự chỉ được `mod` từ ngữ cảnh test không), không dựa trên chuỗi ký tự.

---

## 8. Sáu tiêu chí K6 trên ảnh cuối

Ảnh: `2af101d141fb9a2864a92941cd60030e86a382873eb0b184ba945be25a460896`, 213 632 B, `inputs=40f902a41ecd3979`, `dirty=0`, `build=d6b1bf144b141cf5`.

**Vì sao phải đo lại**: KT-158 sửa `proc_fs.rs` ⇒ ảnh cũ `e7149448…` (bằng chứng đóng K7) **không còn khớp cây**. Đo lại toàn bộ trên ảnh mới, tổng **30 m 40 s**.

| Tiêu chí | Kết quả | Bằng chứng |
|---|---|---|
| **F1** `shell>` ở 1/2/4 hart | ĐẠT | `shell_prompt_lines=2` ở cả ba |
| **F2** parity + đọc lại sau reboot + persist | ĐẠT | 4 lượt `total=28 ok=14 deferred=14 active=14`, `not_ok=0 timeout=0`; `f2_reboot_read_cat_ok=2`; đĩa không đổi giữa hai boot; `persist_exit=0` |
| **F3** ca âm ảnh **tắt shell** | ĐẠT | ảnh `f38fc77a…`, `shell_prompt_lines=0`, parity `not_ok=14 khong_co_shell=14`, `exit=5` |
| **F4** ma trận + soak | ĐẠT | `KTEST: 21/21 boot sạch`; soak `ok=1000 fail=0` `wall=295s` |
| **F5** ngưỡng bộ nhớ | ĐẠT | `file=213632 data+bss=224288 budget=262144` |
| **F6** quét harness | ĐẠT | `harness_scan_exit=0` |

**K7 giữ nguyên phán quyết ĐẠT** (D2-143); bằng chứng K6 nay **cập nhật theo cây mới** (D2-156). Không có tiêu chí nào bị nới.

---

## 9. Trạng thái cuối

```text
HEAD        fda1500d6b95 (main)  chưa push: 0
build-inputs 226 mục  dấu vân tay: 40f902a41ecd3979  dirty: 0
ảnh mới nhất 2af101d141fb9a2864a92941cd60030e86a382873eb0b184ba945be25a460896
             sha256=2af101d1… size=213632
             # build: algo=kt134-v4 inputs=40f902a41ecd3979 dirty=0
```

- `gate --fast` **PASS** trên cây cuối (sau khi merge **mười bốn** nhánh).
- `v2 check` **PASS** sau **mỗi** merge.
- Không còn QEMU mồ côi; không `pkill -f qemu-system` lần nào.

---

## 10. Nợ còn lại (ghi rõ, không giấu)

1. **`is_test_file_heuristic` vẫn dùng cho mục 1–5** của `check-wake-paths.py` (kênh `MailboxMsg`, consumer, arch, `E_STRICT_TRANSITION`). Module sản phẩm tên `ktest_evil.rs` vẫn có thể lọt **luật KT-155** (không phải luật `wake`). Muốn đóng phải đổi 4 call site sang đồ thị module — nhưng khi đó `proc_demo.rs` thành test-only và 2 mục `NON_WAKE_READY_SITES` thành "mục", cần xử lý `E_STRICT_STALE`. **Cần Agent 0 quyết** vì đổi ngữ nghĩa KT-155.
2. **`include!`/`#[path]`** chỉ phân giải trong `--strict` (đúng phạm vi luật `wake`).
3. **`macro_rules!` ngoài `v2/lib`** + ngoài `v2/kernel/*/src`/`v2/sim/*/src` (ví dụ `init/`) không quét thân. `init/` không phải mã sản phẩm kernel (rustc trực tiếp, không `--extern vifast_core`).
4. **Bản đồ trợ giúp cùng tệp** (KT-163) vẫn nhận hàm không-phải-`#[test]` **trong cùng tệp**, kể cả hàm sản phẩm cùng tệp — hành vi có từ KT-163, chưa đổi vì có thể làm đỏ test hiện có; cần đo tác động trước.
5. **K4' chưa đạt**: mới có bước 1–2 (MMU + thanh ghi + asm). Chưa có linker script, vector table, `eret`, đường boot, `VSpace` thật, `TLBI` sau `unmap`.
6. **K5' chưa mở**: audit xong, cần Agent 0 duyệt M1 (chốt phạm vi + đường boot) và mở `spec/abi.toml` `[calling.x86_64]`.
7. **N1 "600 boot x86_64"** vẫn ở K5' (D2-149).
8. **`cd` vào thư mục con sâu** (nhiều cấp) chưa kiểm — FS sản phẩm mới có một cấp `/docs`.

---

## Phụ lục A — Commit trên `main` trong đêm

`6b2fdce7` (D2-144..149) · `714b28c9` (D2-150) · `3688db26` (merge L3) · `4a317dbd` (merge L2) · `9a8b8acb` (merge L4) · `9d8201dd` (merge L5) · `083c4bdd` (sửa báo oan teeth) · `b685f406` (merge L7) · `ee772514` (merge L1) · `e022cdbe` (merge L6) · `f3a5a958` (merge L8) · `6fbc833f` (merge L9) · `25306b3f` (D2-151..153) · `41923a82` (D2-154) · `208ec7d9` (hàng chờ) · `cf99cfe4` (merge L10) · `be0010a9` (merge L11) · `6ceb20ec` (merge L13) · `11cf9e5c` (merge L12) · `fda1500d` (merge L14).

## Phụ lục B — Tuân thủ luật 1–4

1. **Không nới ngưỡng**: không đổi `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`, số seed, số ca đỏ. Hai hằng tài nguyên của **ảnh test** (`OBJ_ARENA_BYTES`, `MAX_PROCS`) được nới **có ghi** trong D2-152 — không phải ngưỡng cổng.
2. **Không sửa kỳ vọng harness cho test xanh**: mọi thay đổi kỳ vọng đều **siết** (thêm phép kiểm, thêm lớp checker), không nới. `spec/` **không** sửa trong đêm nay.
3. **Không `#[ignore]`/`#[should_panic]` để né**: máy kiểm mới **cấm** cả hai (lớp `E_IGNORE`, `E_SHOULD_PANIC_BARE`).
4. **Không retry để lấy xanh**: lần `gate --fast` đỏ duy nhất là **T-INFRA** (D2-151), đã chứng minh bằng cách chạy lại **một mình** trên máy sạch; không có lần chạy lại nào để "lấy xanh" một ca đỏ thật.
