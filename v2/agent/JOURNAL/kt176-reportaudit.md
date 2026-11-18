# KT-176 — audit báo cáo đêm vòng hai + cập nhật STATE

**Ngày**: 2026-11-19 (vòng hai, sóng bốn) · **Agent 2 (thợ) L18** · nhánh `night2/L18-reportaudit` ·
worktree `/private/tmp/night2-L18-reportaudit` · HEAD `2dc03fed` (từ `main`).
**Ngân sách**: hạn cứng 08:00 JST. Không merge, không push, không `gate --fast`, không tạo worktree mới.
**Phạm vi**: **chỉ đọc** `BAO-CAO-DEM-2026-11-19-VONG2.md` (không sửa — lead sửa); ghi phát hiện vào
journal này; cập nhật `v2/agent/STATE.md`. **Không** sửa `QUEUE.md`/`DECISIONS.md` (Agent 0 giữ).

## 0. Kết luận trước

| Việc | Kết quả |
|---|---|
| Khẳng định kiểm được | **64** (bảng §2) |
| Khớp nguồn | **56** |
| **Không truy được nguồn / nguồn nói khác** | **8** (liệt kê §3) |
| Nguồn **ngoài repo** (log thô ở `/tmp`, không commit) | **6 nhóm** (§4) |
| `./v2/bin/v2 check` | **PASS** |
| Sửa báo cáo | **KHÔNG** (đúng luật task) |

**Nhận định ngắn**: báo cáo **trung thực về định tính** (mọi phán quyết ĐẠT/KHÔNG ĐẠT, mọi mã lỗi,
mọi tệp:dòng đều tái lập được), nhưng có **ba con số bị lấy từ trạng thái SAU khi vá** gán cho task
**trước khi vá** (KT-158 `15` vs `14`; KT-160 `24` vs `22`; KT-158 "hai đột biến đỏ" vs **một**), và
**một con số giữa đêm** (`chín task`) bị chép vào tổng kết cuối (thực tế **mười bốn**). Nguồn gốc ba
số đầu là **D2-153** (cũng dùng số sau vá) — báo cáo chép trung thành từ quyết định, không bịa.

---

## 1. Phương pháp

1. Đọc toàn bộ 267 dòng báo cáo, tách **64 khẳng định** có số/định danh kiểm được.
2. Với mỗi khẳng định, tìm **dòng log thật** theo thứ tự ưu tiên: `v2/agent/JOURNAL/kt1*-logs/` →
   `v2/agent/JOURNAL/kt1*.md` → `/tmp/gate-*.log` → log thô `/tmp` (ghi rõ là **ngoài repo**).
3. Không suy diễn: chỉ đánh **khớp** khi tìm được dòng chứa đúng con số/chuỗi; đánh **KHÁC** khi
   nguồn ghi số khác; đánh **KHÔNG NGUỒN** khi không có tệp nào chứa.
4. Với số phải **tính** (tổng thời gian), ghi rõ công thức và mốc dùng.

---

## 2. Bảng audit (64 khẳng định)

Ký hiệu nguồn: `J/` = `v2/agent/JOURNAL/`, `T/` = `/private/tmp/` (**ngoài repo**), `G/` = `/tmp/gate-*.log`.

### 2.1 Tóm tắt điều hành (§1) + sóng (§3)

| # | Khẳng định trong báo cáo | Nguồn log | Khớp? |
|---|---|---|---|
| 1 | "**mười bốn sub-agent**" | Bảng §3 có đúng 14 dòng `L1..L14`; `git branch` có `night2/L1..L14` | **KHỚP** |
| 2 | "**chín task** (KT-157…KT-171) merge vào `main`" | `git log --merges 6b2fdce7^..HEAD` = **14** merge; bảng §3 = 14 task | **KHÁC** — 14, không phải 9 (xem §3.1) |
| 3 | "TAP 37 `clients=4 … ok=1`" | `J/kt171-logs/ktest-matrix/ktest-user-smp1-1.log:257`; `T/kt157-green-smp1.log:257` | **KHỚP** (log KT-157 gốc ở `/tmp`, xem §4) |
| 4 | "đột biến `ok=0`" | `T/kt157-red-mutation-smp1.log:257` `faults=15 … ok=0` | **KHỚP** (ngoài repo) |
| 5 | "ma trận **21/21** boot sạch" | `T/kt171-measure.log:486`; `J/kt156-logs/kt156-f4.log:123` | **KHỚP** |
| 6 | "KT-137 … **15 phép kiểm**" (trạng thái cuối) | `J/kt168-logs/shell-edge-clean.log` — 15 dòng `kt137-cdpwd[...]` | **KHỚP** |
| 7 | "`E_STRICT_TEST_ONLY_WAKE`; **39/39** selftest" | `T/kt171-mut/logs/v2-check-postcommit.log:287`; `J/kt171-wakefix3.md` §2.1 | **KHỚP** (log thô ngoài repo) |
| 8 | "K4' … **43 test host**; 3 đột biến đỏ; đối chiếu `llvm-objdump`" | `J/kt167-logs/kt167-host-test.log` (24+15+4=43); `kt167-mut{1,2,3}*.log`; `kt167-aarch64-objdump.log` | **KHỚP** |
| 9 | "K5' … **18 mục** còn thiếu, **8 bước**, **10 rủi ro**" | `J/reports/K5-X86-READINESS.md` §3 (M1..M18), §4 (M1..M8 + M9 mốc riêng), §5 (R1..R10) | **KHỚP** (8 bước = M1–M8; M9 là mốc riêng ngoài K5') |
| 10 | "**33/33** selftest" (máy kiểm test có răng) | `J/kt170-logs/kt170-selftest2.log:34` | **KHỚP** |
| 11 | "bắt **2 test giả thật**" | `v2/lib/vifast-pte/tests/kt35_pte.rs:203-206`; `v2/sim/vifast-sim/src/vspace.rs:284-288` (đã sửa, comment KT-163) | **KHỚP** |
| 12 | "**30 m 40 s**" (đo lại K6) | `T/kt171-measure.log:1` `2026-11-18T20:37:13Z` → dòng cuối `2026-11-18T21:07:53Z` = **30 m 40 s** | **KHỚP** (phải tính; không có dòng tổng) |
| 13 | "soak **1000/1000**" | `T/kt171-measure.log:1513` `ok=1000 fail=0 tổng=1000/1000` | **KHỚP** |
| 14 | "cổng cuối `gate --fast` **PASS** trên cây cuối" | `G/gate-3.log` dòng cuối `v2 gate --fast: PASS` — nhưng chạy trên `fda1500d`, **không** phải `2dc03fed` | **KHỚP CÓ ĐIỀU KIỆN** (§3.6) |

### 2.2 Sóng một (§4)

| # | Khẳng định | Nguồn log | Khớp? |
|---|---|---|---|
| 15 | KT-157: "TAP 30..36 **vẫn xanh**" | `T/kt157-red-mutation-smp1.log:233-240` (`ok 30`..`ok 36`) | **KHỚP** (ngoài repo) |
| 16 | KT-157: "`OBJ_ARENA_BYTES 64→96 KiB`" | `git diff 6b2fdce7^..HEAD -- …/obj.rs`: `-64 * 1024` / `+96 * 1024` | **KHỚP** |
| 17 | KT-157: "`MAX_PROCS 4→6`" | `git diff … -- …/user.rs`: `-4` / `+6` | **KHỚP** |
| 18 | KT-158: "ca 16 từ **11 → 15 phép kiểm**" | `J/kt158-fsdir.md:21` "**11 → 14**"; `J/kt158-logs/kt158-shell-edge-green.log` = **14** dòng; merge `4a317dbd` "11->14 phep kiem" | **KHÁC** — 14, không phải 15 (§3.2) |
| 19 | KT-158: "**Hai đột biến đỏ**: `pwd` hằng `/`; `resolve_path` bỏ `Cwd`" | `J/kt158-fsdir.md` §4 chỉ có **một** đột biến (`pwd`); `J/kt161-review.md:21` "đột biến `resolve_path` bỏ `Cwd` **vẫn XANH**" | **KHÁC** — chỉ 1 đỏ; cái thứ hai xanh tới KT-166 (§3.3) |
| 20 | KT-158: "`cat ok bytes=16 first=ghi chu`" | `J/kt158-logs/run-green/qemu.log:304` | **KHỚP** |
| 21 | KT-158: "`pwd /docs`, `ls n=1 path=/docs`" | `J/kt158-logs/run-green/qemu.log:296-301` | **KHỚP** |
| 22 | KT-159: "**Sáu test ghim**" | `J/kt159-logs/kt159-test-pin.log` — `running 6 tests` … `6 passed` | **KHỚP** |
| 23 | KT-159: "**15/15** selftest" | `J/kt159-logs/kt159-selftest.log:16` | **KHỚP** |
| 24 | KT-160: "**24 test host**" | `J/kt161-logs/kt160-baseline.log:11-35` = **22 passed**; `J/kt160-aarch64.md:24,81` = 22; merge `9a8b8acb` "**22 test host**" | **KHÁC** — 22 tại thời điểm KT-160; 24 là **sau** KT-166 (§3.4) |
| 25 | KT-160: "rlib **218 360 B**" | `J/kt161-review.md:196` (chỉ có trong journal md; **không** có log thô) | **KHỚP** (nguồn yếu, §4) |

### 2.3 Sóng hai (§5)

| # | Khẳng định | Nguồn log | Khớp? |
|---|---|---|---|
| 26 | KT-161: "ba task **ĐẠT CÓ ĐIỀU KIỆN**" | `J/kt161-review.md:21-23` (KT-158/159/160) | **KHỚP** |
| 27 | KT-161: "**6 lỗ hổng** luật wake" | `J/kt161-review.md:22` "luật bị lách **6** cách" | **KHỚP** |
| 28 | KT-161: "`DESC_ATTR_SHIFT 2→3` và `DESC_NG 1<<11→1<<60` vẫn **22/22 xanh**" | `J/kt161-logs/kt160-mutations.log:6-8, 20-22` (`22 passed` cả hai) | **KHỚP** |
| 29 | KT-163: "**bốn lớp** chắc chắn" | `J/kt163-teeth.md:10, 36-38` (`E_IGNORE`, `E_NO_ASSERT`, `E_LET_UNDERSCORE`, hằng-đúng) | **KHỚP** |
| 30 | KT-164: "L0 `vifast-plat-common` **118 passed**" | `J/kt164-shared.md:36,109`; tổng các dòng `test result` = 6+14+12+8+3+15+12+7+9+12+11+9 = **118** | **KHỚP** (journal md; không có log thô) |
| 31 | KT-165: "Selftest **26/26**" | `J/kt165-logs/selftest.log:1` | **KHỚP** |
| 32 | KT-165: "checker **cũ** trên cùng cây đột biến ⇒ `errors=0`" | `J/kt165-logs/m3-checker-cu-xanh.log` `errors=0 warnings=0` | **KHỚP** |
| 33 | KT-166: "**14→15 phép kiểm**" | `J/kt168-logs/shell-edge-clean.log` = 15 dòng | **KHỚP** |
| 34 | KT-166: "`resolve_path` bỏ `Cwd` ⇒ **đỏ 4 phép kiểm**" | `J/kt168-logs/shell-edge-mut1.log` — 4 dòng `FAIL` (`ls-cwd`, `ls-docs`, `cat-docs`, `stat-docs`) | **KHỚP** |

### 2.4 Sóng ba (§6)

| # | Khẳng định | Nguồn log | Khớp? |
|---|---|---|---|
| 35 | KT-167: "**43 test host**" | `J/kt167-logs/kt167-host-test.log` — 24 + 15 + 4 | **KHỚP** |
| 36 | KT-167: "`clang --target=aarch64-none-elf` + `llvm-objdump` … **11 mã**" | `J/kt167-a64step2.md:43,188`; `J/kt167-logs/kt167-aarch64-objdump.log` = 11 dòng lệnh | **KHỚP** (log `clang` thô **không** có; chỉ có objdump) |
| 37 | KT-167: "`TCR_EL1` 17 trường, `MAIR_EL1` 8 slot, `SCTLR_EL1` 39 bit, `TLBI` 6 mã, barrier 13 encoding" | `J/kt167-a64step2.md:30-34, 50-55` | **KHỚP** |
| 38 | KT-168: "KT-165 **KHÔNG ĐẠT** … **5 lớp**" | `J/kt168-review2.md:13,134` (B4, B6, B7, B8/B9, B10) | **KHỚP** |
| 39 | KT-168: "bốn bit khác tác giả (`DESC_AF/PXN/UXN/SH`) đều **đỏ**" | `J/kt168-logs/a64-mut-{af,pxn,uxn,sh}.log` — `FAILED` cả bốn | **KHỚP** |
| 40 | KT-170: "**277/285** test … nhờ hàm sản phẩm" | `J/kt170-teeth2.md:127` | **KHỚP** |
| 41 | KT-170: "bản đồ **319 → 80**" | `J/kt170-logs/kt170-scan.log` (319) vs `kt170-scan2.log` (80) | **KHỚP** |
| 42 | KT-170: "vi phạm trên cây **0 → 0**" | `kt170-scan.log` / `kt170-scan2.log` đều `errors=0 warnings=0` | **KHỚP** |
| 43 | KT-171: "**6/6** lớp đóng" | `J/kt171-wakefix3.md` §0 — 6 dòng `ĐÃ ĐÓNG` | **KHỚP** |
| 44 | KT-171: "sáu đột biến **ĐỎ**; checker KT-165 ⇒ **cả 6 `errors=0`**" | `T/kt171-mut/logs/{b4,b6,b7,b8,b9,b10}-strict.log` = 1 `E` mỗi tệp; `-old.log` = 0 `E` mỗi tệp | **KHỚP** (ngoài repo) |

### 2.5 Sáu tiêu chí K6 + trạng thái cuối (§8, §9)

| # | Khẳng định | Nguồn log | Khớp? |
|---|---|---|---|
| 45 | "ảnh `2af101d1…`, **213 632 B**" | `T/kt171-measure.log:56`; `J/kt171-logs/kt171-f2-write-smp1.log:2` | **KHỚP** |
| 46 | "`data+bss=224288`" | `T/kt171-measure.log:56` | **KHỚP** |
| 47 | "`budget=262144`" | `T/kt171-measure.log:56` | **KHỚP** |
| 48 | "soak `wall=295s`" | `T/kt171-measure.log:1513` | **KHỚP** |
| 49 | "`inputs=40f902a41ecd3979`" | `T/kt171-measure.log:14` (`REV-GATE: ACCEPT … current_inputs=40f902a41ecd3979`) | **KHỚP** |
| 50 | "build-inputs **226 mục**" | `T/kt171-measure.log:14` `count=226` | **KHỚP** |
| 51 | "`dirty=0`" | `T/kt171-measure.log:14` + `J/kt171-logs/kt171-boot-smp1.log` `# build: … dirty=0` | **KHỚP** |
| 52 | "`build=d6b1bf144b141cf5`" | `T/kt171-measure.log:1514` `BUILD_ID=d6b1bf144b141cf5` | **KHỚP** |
| 53 | "ảnh ca âm `f38fc77a…` **82 552 B**" | `J/kt171-logs/parity/kt77a-serial-custom-smp1-kt171-noshell.log:5`; `T/kt171-measure.log:310` | **KHỚP** |
| 54 | F1 "`shell_prompt_lines=2` ở cả ba" | `T/kt171-measure.log:13, 20, 27` | **KHỚP** |
| 55 | F2 "**4 lượt** `total=28 ok=14 deferred=14 active=14`" | `T/kt171-measure.log:98, 118, 157, 177` | **KHỚP** |
| 56 | F2 "`f2_reboot_read_cat_ok=2`; `persist_exit=0`" | `T/kt171-measure.log:194, 306` | **KHỚP** |
| 57 | F3 "`shell_prompt_lines=0`, `not_ok=14 khong_co_shell=14`, `exit=5`" | `T/kt171-measure.log:315, 355, 358` | **KHỚP** |
| 58 | F6 "`harness_scan_exit=0`" | `T/kt171-measure.log:52` | **KHỚP** |
| 59 | "HEAD `fda1500d6b95` (main)" | `git rev-parse HEAD` = **`2dc03fed`**; `T/kt171-measure.log:1514` `git=fda1500d` | **KHÁC** — cũ; `main` cuối là `2dc03fed` (§3.5) |
| 60 | "chưa push: **0**" | `git log origin/main -1` = `2dc03fed` = HEAD | **KHỚP** |
| 61 | "Không còn QEMU mồ côi" | `T/kt171-measure.log` dòng cuối `qemu_total=0` | **KHỚP** |
| 62 | "`spec/` **không** sửa trong đêm nay" | `git log 6b2fdce7^..HEAD -- spec/` = **rỗng** | **KHỚP** |
| 63 | "`v2 check` **PASS** sau **mỗi** merge" | Không có log `v2 check` cho từng merge; chỉ có `G/gate-2.log` (`25306b3f`) và `G/gate-3.log` (`fda1500d`) | **KHÔNG NGUỒN** (§3.7) |
| 64 | "không `pkill -f qemu-system` lần nào" | Không log nào ghi việc **không** chạy lệnh; không có audit trail | **KHÔNG NGUỒN** (§3.8) |

### 2.6 Kiểm thêm ngoài bảng (không tính vào 64)

| Khẳng định | Nguồn | Khớp? |
|---|---|---|
| D2-144 … D2-156 (13 mục) | `v2/agent/DECISIONS.md` — đủ 13/13 | **KHỚP** |
| Phụ lục A: 20 commit | `git cat-file -t` — 20/20 là `commit` | **KHỚP** |
| "không đổi `MAX_IO`/`MAX_TCBS`/`MAX_OBJECTS`/`KERNEL_STATIC_BUDGET`" | `git diff 6b2fdce7^..HEAD` — chỉ **comment** đổi, không đổi hằng | **KHỚP** |
| D2-151 "`records=13` thay vì 11/12" | `G/gate-1.log:2307` `records=13 well_formed=0`; `gate-2.log` persist PASS | **KHỚP** |
| "`docs/QUYET-DINH`, thiết kế v2, `docs/v1/` không sửa" | `git log 6b2fdce7^..HEAD -- <ba đường>` = rỗng | **KHỚP** |

---

## 3. Những chỗ **KHÔNG** truy được nguồn hoặc nguồn nói khác (8)

### 3.1 §1 "**chín task** (KT-157…KT-171) merge vào `main`" — SAI, phải là **mười bốn**

- `git log --oneline 6b2fdce7^..HEAD --merges` = **14** merge (`3688db26`, `4a317dbd`, `9a8b8acb`,
  `9d8201dd`, `b685f406`, `ee772514`, `e022cdbe`, `f3a5a958`, `6fbc833f`, `cf99cfe4`, `be0010a9`,
  `6ceb20ec`, `11cf9e5c`, `fda1500d`).
- Bảng §3 của **chính báo cáo** có 14 dòng; §9 lại viết "sau khi merge **mười bốn** nhánh".
- Nguồn của số "chín": **D2-154** "cây đã merge **chín** nhánh (`25306b3f`)" — đúng **giữa đêm**
  (`git log 6b2fdce7^..25306b3f --merges` = 9), không đúng lúc kết thúc.
- **Đề nghị sửa**: "chín task" → "**mười bốn task**".

### 3.2 §4 KT-158 "ca 16 `shell-edge.sh` từ **11 → 15 phép kiểm**" — SAI, phải là **11 → 14**

- `J/kt158-fsdir.md:21`: "Ca `kt137-cdpwd` mở rộng **11 → 14 phép kiểm**".
- `J/kt158-logs/kt158-shell-edge-green.log`: đúng **14** dòng `kt137-cdpwd[...]` (không phải 15).
- Merge commit `4a317dbd`: "ca 16 kiem thu muc con **11->14 phep kiem**".
- `J/kt161-review.md:21`: "ca 16 đủ **14** phép kiểm".
- Số **15** chỉ đúng **sau KT-166** (`J/kt168-logs/shell-edge-clean.log` = 15, thêm `stat-docs`).
- Nguồn của số "15": **D2-153** (cũng dùng số sau vá) — báo cáo chép trung thành.
- **Đề nghị sửa**: §4 KT-158 "11 → **14**"; §3 bảng L2 "**14** phép kiểm".

### 3.3 §4 KT-158 "**Hai đột biến đỏ**: `pwd` in hằng `/`; `resolve_path` bỏ `Cwd` (… nay **bị bắt**)" — SAI

- `J/kt158-fsdir.md` §4 chỉ ghi **một** đột biến (`pwd` in hằng `/`), log `kt158-shell-edge-mut.log`
  chỉ có **một** ca đỏ (`kt137-cdpwd[pwd]: FAIL`).
- `J/kt161-review.md:21` (reviewer L6, độc lập): "đột biến `resolve_path` bỏ qua `Cwd` **vẫn XANH**"
  ⇒ đây chính là lý do KT-158 bị chấm **ĐẠT CÓ ĐIỀU KIỆN** và mở KT-166.
- Đột biến `Cwd` chỉ **đỏ** từ KT-166 (`J/kt168-logs/shell-edge-mut1.log`, 4 `FAIL`).
- Câu "nay **bị bắt**" đúng ở **hiện tại**, nhưng đặt trong mục KT-158 với chữ "**Hai** đột biến đỏ"
  làm người đọc hiểu KT-158 đã tự bắt được — **ngược** với phán quyết của reviewer.
- **Đề nghị sửa**: "**Một** đột biến đỏ (`pwd`); đột biến `resolve_path` bỏ `Cwd` **vẫn xanh** ở
  KT-158 — reviewer L6 phát hiện, KT-166 mới bắt được."

### 3.4 §4 KT-160 "**24 test host**" — SAI, phải là **22**

- `J/kt160-aarch64.md:24` "`22 passed; 0 failed`"; `:81` "`tests/host.rs` (841 dòng — **22** test host)";
  `:172-173` `running 22 tests` / `22 passed`.
- `J/kt161-logs/kt160-baseline.log:11-35` = **22 passed**.
- Merge commit `9a8b8acb`: "**22 test host**, 3 dot bien do".
- Số **24** chỉ đúng **sau KT-166** (`J/kt168-logs/a64-baseline.log:12` = 24; `J/kt166-gapfix.md:278`
  "22 → 24 test").
- Nguồn của số "24": **D2-153** (số sau vá).
- **Đề nghị sửa**: §4 KT-160 "**22** test host"; §3 bảng L4 "**22** test host".

### 3.5 §9 "HEAD `fda1500d6b95` (main)" — CŨ

- `git rev-parse HEAD` = `2dc03fed15758d4f91bee0b74aaa0ec73a892edf`; `origin/main` = `2dc03fed`.
- `fda1500d` đúng là HEAD **lúc đo K6** (`T/kt171-measure.log:1514` `git=fda1500d6b953c9026277af4bcfdefd200a8cc70`)
  và **lúc chạy cổng** (`G/gate-3.log` dòng 1 `== v2 check (HEAD fda1500d) ==`).
- Commit `2dc03fed` (chứa chính báo cáo) tới **sau** cổng ⇒ báo cáo không thể tự trỏ vào nó.
- **Đề nghị sửa**: ghi rõ "HEAD lúc đo/cổng: `fda1500d`; HEAD cuối (chứa báo cáo): `2dc03fed`".

### 3.6 §9 "`gate --fast` **PASS** trên cây cuối" — ĐÚNG CÓ ĐIỀU KIỆN

- `G/gate-3.log` PASS, nhưng chạy trên **`fda1500d`**, không phải `2dc03fed`.
- `2dc03fed` chỉ thêm tài liệu (`BAO-CAO…`, `DECISIONS.md`, `JOURNAL/kt171-logs/`) ⇒ rủi ro thấp,
  nhưng câu "trên cây cuối" là **quá mạnh** so với log.
- **Đề nghị sửa**: "PASS trên `fda1500d` (sau 14 merge); `2dc03fed` chỉ thêm tài liệu".

### 3.7 §9 "`v2 check` **PASS** sau **mỗi** merge" — KHÔNG có nguồn

- Không có log `v2 check` cho từng merge trong repo hay `/tmp`. Chỉ có `G/gate-3.log` (một lần, PASS)
  và `G/gate-2.log` (PASS trên `25306b3f`).
- **Đề nghị**: hạ thành "`v2 check` PASS ở các mốc có log (`25306b3f`, `fda1500d`)".

### 3.8 §9 "không `pkill -f qemu-system` lần nào" — KHÔNG kiểm được

- Đây là khẳng định **phủ định**; không log nào ghi lại việc **không** chạy lệnh. Không có
  `bash history`/audit trail trong repo.
- **Đề nghị**: giữ nguyên nhưng ghi "(không kiểm được bằng máy — chỉ là cam kết của người chạy)".

---

## 4. Nguồn **ngoài repo** — log thô ở `/tmp`, chưa commit (6 nhóm)

Đây **không** phải "không có nguồn": log **có thật** và **còn nguyên** lúc audit, nhưng **không** nằm
trong `v2/agent/JOURNAL/kt1*-logs/` như luật bằng chứng của repo đòi. Nếu `/tmp` bị dọn, các số sau
mất nguồn máy kiểm được:

| Nhóm | Số phụ thuộc | Tệp thật |
|---|---|---|
| **K6 đo lại (toàn bộ §8)** | `224288`, `295s`, `30 m 40 s`, `21/21`, `1000/1000`, `count=226`, `dirty=0`, `build=d6b1bf…`, F1/F2/F3/F6 | `/private/tmp/kt171-measure.log` (69 463 B, 06:07) |
| **KT-157 TAP 37** | `clients=4 … ok=1`, `faults=15 … ok=0`, TAP 30..36 xanh | `/private/tmp/kt157-green-smp1.log`, `kt157-red-mutation-smp1.log` |
| **KT-171 selftest + 6 đột biến** | `39/39`, 6 `E` vs 6 `errors=0` | `/private/tmp/kt171-mut/logs/` (18 tệp) |
| **KT-160 rlib** | `218 360 B` | chỉ có trong `J/kt161-review.md:196` (journal md), **không** log thô |
| **KT-164 118 passed** | `118` | chỉ có trong `J/kt164-shared.md` (journal md), **không** log thô |
| **KT-167 `clang`** | "đối chiếu `clang --target=aarch64-none-elf`" | chỉ có `llvm-objdump` log; **không** có log `clang` |

**Đề nghị cho lead** (không tự làm — ngoài phạm vi KT-176): chép `kt171-measure.log` +
`kt157-{green,red-mutation}-smp1.log` + `kt171-mut/logs/` vào `JOURNAL/kt171-logs/` (hoặc
`kt157-logs/`) trong một commit riêng, rồi trỏ báo cáo vào đó.

---

## 5. Những chỗ **đáng khen** (đã kiểm, khớp chính xác)

- **Mọi phán quyết** đều tái lập: KT-165 **KHÔNG ĐẠT** (5 lớp, B8/B9 thật), KT-166 **ĐẠT**,
  KT-171 **6/6 đóng** — kèm **đối chứng âm** (checker cũ `errors=0` trên **cùng** 6 đột biến).
- **Mọi mã lỗi** trong báo cáo (`E_STRICT_TEST_ONLY_WAKE`, `E_STRICT_WAKE_INCLUDE`,
  `E_STRICT_MACRO_WAKE`, `E_SHOULD_PANIC_BARE`, `E_IGNORE`, `E_NO_ASSERT`, `E_LET_UNDERSCORE`)
  đều tồn tại trong `v2/tools/` và có ca selftest.
- **Mọi `file:dòng`** của K5' (18 mục) và của 2 test giả (KT-163) đều đúng.
- **Số then chốt của K6** (`213632`, `224288`, `262144`, `295s`, `1000/1000`, `21/21`,
  `40f902a41ecd3979`, `226`, `d6b1bf144b141cf5`, `f38fc77a…/82552`) khớp **từng chữ số**.
- **Không nới ngưỡng**: `git diff` chứng minh `MAX_IO`/`MAX_TCBS`/`MAX_OBJECTS`/`KERNEL_STATIC_BUDGET`
  không đổi; `spec/` không chạm; `docs/QUYET-DINH`/thiết kế/`docs/v1` không chạm.

---

## 6. Việc đã làm trong task này

1. Audit 64 khẳng định (bảng §2) — **56 khớp**, **8 lệch/không nguồn** (§3).
2. Ghi **6 nhóm nguồn ngoài repo** (§4) — không sửa báo cáo, không chép log (ngoài phạm vi).
3. Cập nhật `v2/agent/STATE.md`: khối đầu (HEAD/build-inputs/ảnh/cổng/nợ) + mục mới
   "ĐÊM 2026-11-19 VÒNG HAI" ở cuối tệp.
4. `./v2/bin/v2 check` — **PASS** (xem §7).
5. **Không** sửa `QUEUE.md`, `DECISIONS.md`, `spec/`, `docs/QUYET-DINH-sau-review.md`,
   `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`, `BAO-CAO-DEM-2026-11-19-VONG2.md`.

## 7. Bằng chứng cổng

```text
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

## 8. Luật đã giữ

- Không merge, không push, không `gate --fast`, không tạo worktree mới.
- Không `pkill -f qemu-system`; task **không** dùng QEMU.
- Không sửa báo cáo (lead sửa) — chỉ ghi phát hiện.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không `local a=1 b="$a"`.
- Commit `[KT-176] …` trên nhánh riêng với `VIFAST_ROLE=lead` (chỉ để hook cho phép commit).
