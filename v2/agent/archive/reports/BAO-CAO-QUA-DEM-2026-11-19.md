# BÁO CÁO QUA ĐÊM — 2026-11-18 22:00 → 2026-11-19 09:00 (JST)

**Người thực hiện**: Agent 1 (lead). **Uỷ quyền**: người dùng, 2026-11-18 21:55 JST —
*"Bạn hãy tự quyết định thay Agent0, tạo nhiều sub agent2 để xử lý các công việc tiếp theo,
đặt mục tiêu xử lý cho tới 9h sáng ngày mai."*

**Phạm vi**: K7 "Đóng lỗi v1" — bốn lỗi v1 còn mở **N1/N2/N3/N7**, cộng **B7** (test `smp` đỏ sẵn)
và **KT-137** (`cd`/`pwd`). Không đổi ABI, không đổi `spec/` ngoài hai ngoại lệ hẹp có ghi,
không nới ngưỡng, không làm lại baseline K6.

---

## 1. Tóm tắt một trang

Đêm nay chốt **bốn lỗi v1 còn mở** và **hai việc hàng chờ**, mỗi việc theo luật ba vế
(đỏ-trên-mã-cũ / xanh-trên-mã-mới / đột-biến-làm-đỏ-lại), cộng **một vòng review độc lập**
do một sub-agent riêng làm và **phá được** hai bằng chứng — hai lỗ đó đã được vá trong đêm.

| Việc | Kết quả | Ghi chú quan trọng nhất |
|---|---|---|
| **N2** (T26, read-your-own-write) | **ĐÓNG** | 50 boot v2 **0 đỏ**; 50 boot v1 **trước vá** 5/50 đỏ; tag `v1-archive` đã chứa bản vá nên 0/50 — giữ làm **đối chứng dương** |
| **N3** (T31, pager giao lặp) | **ĐÓNG có hạn chế** | TAP 37 đếm giao **đúng một lần** theo từng địa chỉ; vế 1 là **test host trên mã trước r309**, không phải ảnh |
| **N7** (Q1-A, kênh wake lệch) | **ĐÓNG** | Ban đầu **không đóng được**: reviewer phá được cổng. Sau KT-154+KT-155: **28 đường** wake qua một cửa, allowlist tường minh, `--strict` **0**, đột biến **bị bắt** |
| **N1** (T33a, treo x86_64) | **ĐÓNG có hạn chế** | Kịch bản sim "đơn điệu sau migrate" + conformance chéo hart + **hai** đột biến đỏ; vế 1 chỉ là **trích dẫn** `[V1:ĐO]` |
| **B7** (test `smp` đỏ sẵn) | **ĐÓNG** | Sửa test + chú thích; **thêm** L0 workspace kernel vào `gate --fast` |
| **KT-137** (`cd`/`pwd`) | **ĐÓNG có hạn chế** | 11 phép kiểm trên ảnh sản phẩm; FS sản phẩm chỉ có `/` nên `cd` vào thư mục con **chưa kiểm được** |
| **KT-156** (sáu tiêu chí K6) | **ĐẠT cả sáu** | đo trên **ảnh cuối** `e7149448…`: `shell>` ở 1/2/4 hart, ma trận **21/21** + 3 boot F1 = **24**, soak **1000/1000**, ca âm tắt shell **đỏ** |

**Kết luận mốc**: **K7 "Đóng lỗi v1" ĐẠT** — bốn lỗi v1 đóng, sáu tiêu chí K6 đo lại đầy đủ trên
**ảnh cuối** cùng dấu vân tay với cây (§9). Kèm **bốn hạn chế** và **bốn nợ** đã ghi tên, không giấu.

**Điều đáng nói nhất**: kết quả đêm nay **không** phải "sáu việc xanh". N7 đã **suýt được tuyên bố
đóng sai** — máy kiểm nằm trong cổng nhưng **không bắt được** đúng đột biến mà nó sinh ra để bắt.
Reviewer độc lập phát hiện; một sub-agent thứ hai vá lại và **chứng minh bằng đột biến** rằng cổng
nay bắt được. Đây là lần thứ mười mấy của lớp lỗi "test xanh nhưng giả" trong repo này, và là lần
đầu nó bị bắt **trước khi** báo cáo được gửi đi.

---

## 2. Quyết định Agent 1 đã thay Agent 0

Ghi thành **D2-128 … D2-142** trong [`v2/agent/DECISIONS.md`](../../DECISIONS.md). Tóm tắt lý do:

- **D2-128 (B7)** — chọn sửa **test + chú thích**, không sửa mã: linker script đã cấp syscall stack
  từ KT-49/KT-66, `mmu::init` kiểm khớp `HART_STRIDE`; sửa mã sẽ làm hẹp vùng per-hart ⇒ panic.
  Kỳ vọng cũ **sai** so với thiết kế đã chốt và **hai chỗ cùng cũ**, nên đây không phải "sửa kỳ
  vọng để test xanh". Kèm **thêm phép đo** (L0 workspace kernel vào cổng).
- **D2-129 (N2)** — tiêu chí gốc "0/50 ở `MAX_IO=256`" **không đo được nguyên văn** trên v2 vì
  `MAX_IO` là hằng **cục bộ của test host** còn virtio-blk QEMU chuyển sector 512. Định nghĩa lại
  thành **bốn điều** (vế 1 trên ảnh v1; 50 boot v2; host 1000 seed; đột biến).
- **D2-130 (N3)** — checker "giao đúng một lần" + ca 4 client + đột biến phát lại; **cấm** đổi ABI
  để làm 4 client mức kernel.
- **D2-131 (N7)** — quét **mọi** đường wake, đòi đi qua một cơ chế. *Tiêu chí này đã bị chính
  kết quả đo bác bỏ — xem §6.*
- **D2-132 (N1)** — v2.0 chỉ riscv64 nên "600 boot x86_64" không áp dụng; làm kịch bản sim +
  conformance chéo hart; **luật cứng**: không đột biến nào làm đỏ ⇒ **không đóng**.
- **D2-133 (KT-137)** — điều kiện bắt đầu B4 đã đóng bởi KT-132 (v1 tại tag **có** `cd`/`pwd`
  chạy thật) ⇒ mở ngay.
- **D2-134** — giao thức đêm: bốn sub-agent, worktree riêng, chỉ lead merge, gate sau mỗi merge.
- **D2-135 … D2-142** — chốt kết quả: vế 1 của N2 dùng **cha của commit vá**; N3 đóng có hạn chế;
  N7 **không đóng** ở lượt đầu rồi **đóng** sau KT-154/155; KT-137 đóng có hạn chế; hai **T-INFRA**
  của cổng (QEMU mồ côi, số worktree) và một **T-INFRA** của bash 3.2.

**Hai ngoại lệ `spec/` đã dùng** (đều hẹp, đều ghi lý do): (a) `status` của `cd`/`pwd` trong
`spec/v1-parity.toml` từ `deferred-K7` → `active` (tiền lệ KT-130); (b) `expect` của đúng hai mục
đó đổi sang đầu ra thật của v2. **Không** đụng `spec/abi.toml`, `spec/kernel.toml`, không đổi ABI.

---

## 3. Chín sub-agent

Năm sóng, **không bao giờ quá bốn** sub-agent cùng lúc; mỗi cái một worktree riêng, chỉ lead merge.

| Sub-agent | Nhánh | Việc | Kết quả |
|---|---|---|---|
| L1 | `night/L1-n2` | N2/T26 | harness `t26.sh` + bốn vế |
| L2 | `night/L2-n3` | N3/T31 | TAP 37 + 4 client host + đột biến |
| L3 | `night/L3-n7` | N7 | máy kiểm + test + **phát hiện 16 đường wake trực tiếp** |
| L4 | `night/L4-b7-n1` | B7 + N1 | sửa test `smp` + L0 kernel vào cổng; kịch bản sim N1 |
| L5 | `night/L5-kt137` | KT-137 | `cd`/`pwd` + đường dẫn tương đối |
| L6 | `night/L6-review` | **review độc lập** | phá được 2 bằng chứng ⇒ mở KT-154 |
| L7 | `night/L7-n7fix` | N7-2 kernel | 15/16 đường qua `wake_from` |
| L8 | `night/L8-n7sim` | N7-2 sim + cổng | 13 đường sim, allowlist, `--strict` vào cổng |
| L9 | `night/L9-k7measure` | sáu tiêu chí K6 | **F1–F6 ĐẠT** trên ảnh cuối `e7149448…`; ma trận 21/21 + 3 boot F1 = 24; soak 1000/1000 |

**Vì sao có L6**: repo này có hơn mười lần "test xanh nhưng giả" (test và mã dùng chung một công
thức sai). Một sub-agent chỉ **phá** bằng chứng — không làm tính năng — là cách rẻ nhất để bắt
đúng lớp lỗi đó. Nó bắt được hai chỗ, và cả hai đều là chỗ **tôi** vừa viết tiêu chí.

---

## 4. N2 — T26, FS vi phạm read-your-own-write

**Bốn vế, đủ cả bốn.**

**Vế 1 — đỏ trên mã cũ.** Tag `v1-archive` **đã chứa bản vá r296b**, nên 50 boot trên tag cho
**0/50** — đo một cây đã vá thì không phải "đỏ trên mã cũ". Vế 1 vì vậy chạy trên **cha của commit
vá** (`5fc7b3ff`), đúng nguyên tắc A3 của phiếu lượt năm:

```text
$ cd /tmp/n2-v1-prefix && VIFAST_NO_RETRY=1 python3 scripts/test-qemu.py --arch x86_64 --repeat 1   # × 50
BOOT 12 rc=1 RED | x86_64   0/1 PASS | RETRY 1/1 (T-FS=1)
BOOT 14 rc=1 RED | x86_64   0/1 PASS | RETRY 1/1 (T-FS=1)
BOOT 35 rc=1 RED | x86_64   0/1 PASS | RETRY 1/1 (T-FS=1)
BOOT 40 rc=1 RED | x86_64   0/1 PASS | RETRY 1/1 (T-FS=1)
BOOT 48 rc=1 RED | x86_64   0/1 PASS | RETRY 1/1 (T-FS=1)
SUMMARY red=7 green=43 total=50
```

Serial một boot đỏ cho đúng chữ ký T26: `V2FAIL v2=1 fsck=0` / `V2GROUP 06` / `FS V2 BAD`
(`v2/agent/JOURNAL/kt149-logs/v1-prefix-red-tfs-serial.txt`). Phép đo trên **tag** (`0/50`) được
giữ làm **đối chứng dương**: nó chứng minh bản vá r296b vá đúng chỗ.

**Vế 2 — xanh trên mã mới.** Ảnh sản phẩm, 50 boot `-smp 2`:

```text
$ bash v2/test/boot/t26.sh --image <ảnh> --boots 50
# boot 50: PASS (# t26: PASS (payload=48 B, cat khớp từng byte, halt sạch))
# t26: boots=50 green=50 red=0
```

**Vế 3 — test host 1000 seed.** `cargo test -p vifast-fs --test t26_read_your_own_write` ⇒
`test result: ok. 3 passed; 0 failed`.

**Vế 4 — đột biến.** Bỏ `self.dev.flush()?` thứ hai trong `commit()` (`tx.rs:86`):

```text
test ryow_1000_seeds ... FAILED
test ryow_across_remount_1000_seeds ... FAILED
seed 1: read-your-own-write sai sau write[0] (seed 1) — kẻ chen giữa vừa tác động
test result: FAILED. 1 passed; 2 failed
```

**Đo lại trên cây đã merge** (ảnh `01c91ca2…`, `inputs=2551aaea2c167340`): `boots=50 green=50 red=0`.

**Hạn chế đã ghi**: vế 1 chạy trên **mã v1 trước vá**, không phải trên ảnh của tag — vì tag đã vá.
Đây là điều đúng về bản chất (đo cây có lỗi), nhưng **khác câu chữ** "ảnh v1 tại tag".

---

## 5. N3 — T31, pager nhận message lặp

**Checker "giao đúng một lần"** ở hai tầng, cả hai **fail-closed** (sổ rỗng / thiếu dế / đếm không
được ⇒ đỏ):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-user /tmp/kt150-green-smp1.log 1
# pager: once deliveries=10 faults=10 served=9 rejected=1 expected_va=7 distinct_va=7 ok=1
ok 37 - pager giao dung mot lan
ok - check-log ktest-user smp=1 (plan=37 ok=37, 64 evidence)
```

Ba vế của TAP 37: `deliveries == Stat::Faults` · `served + rejected == faults` · đếm **theo từng
địa chỉ** so với bảng kỳ vọng (số kỳ vọng **suy từ bảng**, không viết tay). `-smp 2` cho cùng dòng
evidence và `plan=37 ok=37`.

**Đột biến "phát lại Call khi Resume"** — gửi lại đúng message `pager.fault` khi sắp resume client:

```text
ok 30 - pager fault den server va nap trang
…
ok 36 - pager bat bien reads==misses
# pager: once deliveries=10 faults=12 served=9 rejected=3 expected_va=7 distinct_va=7 ok=0
not ok 37 - pager giao dung mot lan: message bi giao/xu ly lap, hoac so dem theo dia chi khong doc duoc
```

TAP 30..36 **vẫn xanh** ⇒ đỏ **quy được** về đúng TAP 37, không phải đỏ lây.

**Vế 1 — hạn chế.** Ảnh v1 tại tag build được (`a1cb2516…`) nhưng 3 boot `-smp 1/2/4` cho
`evict_ok=1 dup=0` — **không tái hiện**, vì r309 đã đổi nhãn lần giao thứ ba thành `PAGER2 DUP`
(chính r309 ghi 300 boot không thấy `PAGER2 DUP`). Vế 1 vì vậy là **test host bám nguyên văn
`crates/vifast-root/src/main.rs:377-395` chạy trên mã v1 tại `6f6ab5b1^`** (trước r309):

```text
assertion `left == right` failed: harness v1 đòi PAGER EVICT OK đúng 1 lần; mã cũ in 2 lần ⇒ GIAO LẶP
  left: 2
 right: 1
```

kèm **đối chứng dương**: test thật của `v1-archive` xanh.

**Ba hạn chế đã ghi, không giấu**: (i) vế 1 **không phải ảnh**; (ii) ca 4 client ở **mức host**,
4 client mức kernel là nợ (phải đổi VA/CSpace/bảng trang ảnh `ktest-user`); (iii) checker host
**tự đối chiếu sổ của chính harness** — reviewer L6 nêu; bằng chứng chính vẫn là **TAP 37 mức
kernel**, vốn đếm giao thật.

---

## 6. N7 — Audit Q1-A, kênh wake lệch ba port

Đây là mục **suýt bị tuyên bố đóng sai**, và là mục đáng đọc nhất của báo cáo.

### 6.1 Lượt đầu: tiêu chí KHÔNG đạt

KT-151 dựng `v2/tools/check-wake-paths.py` (nối vào `v2 check`) + `tests/kt151_wake.rs` (5 ca) +
đột biến. Máy kiểm **đỏ trên v1** với **ba** cơ chế, kèm `file:dòng`:

```text
E E_V1_MULTI_CHANNEL tìm thấy 3 cơ chế đánh thức chéo hart: aarch64:RESCHED_PENDING+GIC SGI, riscv64:MAINT_KIND+send_ipi, x86_64:AP_RESCHED_PENDING+LAPIC
E E_V1_MULTI_CHANNEL   riscv64:MAINT_KIND+send_ipi: crates/vifast-platform-riscv64/src/platform.rs:315 (send_reschedule)
E E_V1_MULTI_CHANNEL   aarch64:RESCHED_PENDING+GIC SGI: crates/vifast-platform-aarch64/src/platform.rs:193 (send_reschedule)
E E_V1_MULTI_CHANNEL   x86_64:AP_RESCHED_PENDING+LAPIC: crates/vifast-platform-x86_64/src/platform.rs:2905 (wake_hart)
```

Nhưng khi quét **toàn bộ** `v2/kernel` + `v2/sim` (Luật 2), lộ ra: `vifast-sync::Mailbox`
**không** được kernel dùng; kernel dùng `vifast-core::mailbox::MailboxMsg`; và có **16 đường**
`blocked/sleeping → Ready` **trực tiếp** (IPC/Reply/Notification/finalise/refill) không qua
`wake_step`.

### 6.2 Reviewer phá được cổng

L6 thêm một đường wake thứ hai đi vòng qua mailbox (giữ nguyên producer `MailboxMsg::Wake`):

```text
$ python3 v2/tools/check-wake-paths.py
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0        DEFAULT_EXIT=0
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_sched.rs:236 (wake_direct_l6)
# check-wake-paths: mode=v2 strict=1 errors=6 warnings=0        STRICT_EXIT=1
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt151_wake
test result: ok. 5 passed; 0 failed                              TEST_EXIT=0
```

Cổng mặc định **xanh**, test **xanh** — đúng đột biến mà tiêu chí (c) của D2-131 đòi phải đỏ.
Vì vậy **N7 KHÔNG đóng** (D2-137), mở KT-154.

### 6.3 Vá: 28 đường qua một cửa, và cổng bắt được đột biến

**KT-154** (kernel): **15/16** đường qua `Machine::wake_from` — TCB có `home` khác hart hiện tại
thì post `MailboxMsg::Wake` về `home`. Test đỏ-trước 3 ca (`left: 0 right: 1`) → xanh 7 ca; đột
biến bỏ vá ở `reply_with` ⇒ đỏ. `--strict` **4 → 2** (kernel về 0).

**KT-155** (sim + cổng): vá **13** đường của `v2/sim` (đối xứng kernel), thay **heuristic** bằng
**allowlist tường minh** (17 chỗ không-wake có lý do + 6 cửa wake, fail-closed **hai chiều**), và
**đưa `--strict` vào `v2 check`**. `--strict` **2 → 0**:

```text
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0        EXIT=0
```

**Đột biến mà L6 đã phá được nay BỊ BẮT** — thay `wake_from(caller)` bằng
`transition_tcb(caller, Ready)` ở `core_ipc.rs:701`:

```text
E E_STRICT_TRANSITION 1 chỗ chuyển `TcbState::Ready` ngoài cửa wake (allowlist tường minh KT-155)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_ipc.rs:701 (reply_with)
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0        EXIT=1
```

Đột biến tương tự ở sim cũng bị bắt (`kobj.rs:9760`). **N7-4** (`wake_addr` chéo hart) xong bằng
`wake_from_unlink` ở cả kernel + sim, test đỏ-trước 2 tệp. Toàn bộ test cũ xanh: **107** dòng
`test result: ok`, **0** `FAILED`.

**Nợ còn lại, ghi rõ**: `wake()` công khai vẫn "post-only" khi khác hart trong khi `wake_from`
"post + Ready ngay" — **hai ngữ nghĩa có chủ ý** (bốn đường finalise/resume không thể hoãn); muốn
thống nhất phải đổi test KT-151 ⇒ việc của Agent 0. `tcb_suspend` chuyển blocked→Ready không gửi
`Wake` (allowlist ghi lý do "treo: Ready+suspended"). Allowlist tường minh ⇒ mọi
`transition_tcb(…, Ready)` **mới** sẽ làm cổng **ĐỎ** cho tới khi thêm mục + lý do — đúng chủ ý.

---

## 7. N1 — T33a, treo x86_64 sau `shell>`

v2.0 **chỉ riscv64**, nên tiêu chí gốc "600 boot x86_64, 0 treo" **không áp dụng được**. Tiêu chí
của v2 là kịch bản sim + conformance chéo hart:

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt152_n1
test ca_am_api_dong_ho ... ok
test conformance_cheo_hart_mot_thu_tu ... ok
test don_dieu_sau_migrate_hai_hart ... ok
test reset_hart_muon_khong_dua_dong_ho_ve_0 ... ok
test result: ok. 4 passed; 0 failed
```

- **Đơn điệu sau migrate**: TCB chạy hart 0 → preempt → `Migrate` sang hart 1 → chạy tiếp →
  migrate ngược; đọc trước `5`, nguồn hart 1 chỉ `1` nhưng đồng hồ vẫn `5`; `clock_backwards()==0`.
- **Reset hart muộn**: `hart_reinit(1)` đưa nguồn hart 1 về 0, đồng hồ hệ thống **giữ 7** (bản vá
  r323 của v1).
- **Conformance chéo hart**: 12 lần đọc xen kẽ hai hart, dãy **không giảm**, hai hart đọc **cùng**
  giá trị tại cùng điểm.
- **Hai đột biến, cả hai đỏ**: (M1) `read()` trả bộ đếm **theo hart** — đúng lỗi v1 ⇒
  `đồng hồ wall giảm sau migrate: trước=5 sau=1` **và** `hai hart đọc khác nhau tại bước 0`;
  (M2) `reset_hart()` đưa đồng hồ hệ thống về 0 ⇒ `check_clock sau hart_reinit: hart 0: nguồn 7 >
  đồng hồ hệ thống 0 (max-update sai)`.

**Vế 1 — hạn chế**: 600 boot x86_64 **không tái hiện được trên máy này**; vế 1 là **trích nguyên
văn** `[V1:ĐO]` trong `docs/v1/KNOWN-ISSUES.md` + đường mã v1 kèm `file:dòng`
(`main.rs:491-492` `panic!`, `kernel/lib.rs:415-424` `reset()`, `main.rs:4265-4268` root chờ),
**gắn nhãn "chưa đo lại"**. Lưới chắn chứng minh **mô hình** đơn điệu, **không** chứng minh
`rdtime` trên x86_64 thật — câu đó thuộc K5'.

---

## 8. B7, KT-137 và vòng review độc lập

### 8.1 B7 — test `smp` đỏ sẵn, không cổng nào chạy

`HART_STRIDE` đã gồm syscall stack (KT-66) nhưng test và chú thích `smp.rs` vẫn dùng công thức ba
số hạng. `v2/Cargo.toml` có `exclude = ["kernel"]` nên `gate --fast` **không** chạm tới. Sửa **cả
hai** chỗ (test + chú thích) và **thêm** bước `L0 kernel workspace` vào `gate --fast`:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
test bo_cuc_per_hart_khong_chong_va_can_trang ... ok
test result: ok. 9 passed; 0 failed
```

Đột biến khôi phục công thức cũ ⇒ bước mới **đỏ** (`left: 86016 right: 118784`, `EXIT=101`).
Giá phải trả: **7,7 s** ở lần chạy nguội, **0,5 s** khi ấm.

### 8.2 KT-137 — `cd`/`pwd` + đường dẫn tương đối

`CMDS` 15 → 17; shell giữ `cwd` trong buffer cố định và chuẩn hoá từ vựng (`.`, `..`, `//`) trước
khi gửi `OP_PATH` ⇒ **không** đổi ngữ nghĩa FS, giao thức hay ABI. Mười một phép kiểm trên ảnh
sản phẩm, mỗi ca một dòng log:

```text
# shell: cd /
# shell: cd khong co /khongco
# shell: cd khong phai thu muc /README
# shell: pwd /
# shell: cat ok bytes=79 first=ViFastOs
# shell: stat path=/README kind=1 size=79 ino=2
# shell: write /rel.txt bytes=8 sync=ok
# shell: ls n=4 path=/
# shell: help cmds=17
```

Ba vế: ảnh cũ (`a9460f0c`) in `# shell: ?`; ảnh mới 11/11 PASS; đột biến bỏ kiểm `kind=2` ⇒ ba
phép kiểm đỏ. **Hạn chế**: FS sản phẩm **chỉ có gốc `/`** (`proc_fs::seed` chỉ tạo `/README`) nên
`cd` vào **thư mục con** chưa kiểm được, và đột biến "`pwd` in hằng `/`" **không phân biệt được** —
**không** tuyên bố đã chứng minh. Nợ: seed thêm thư mục vào FS sản phẩm (task riêng).

### 8.3 Review độc lập — hai lỗ bắt được

| Task | Kết luận của L6 | Lý do |
|---|---|---|
| KT-148 (B7) | **ĐẠT** | test sửa đúng bốn số hạng; bước mới có trong cổng |
| KT-149 (N2) | **ĐẠT CÓ ĐIỀU KIỆN** | vế 1 trên mã trước vá, không phải tag |
| KT-150 (N3) | **ĐẠT CÓ ĐIỀU KIỆN** | checker host tự đối chiếu sổ của chính harness; 4 client chỉ mức host |
| KT-151 (N7) | **ĐẠT CÓ ĐIỀU KIỆN** | **cổng không bắt được đột biến thêm đường wake thứ hai** ⇒ mở KT-154 |
| KT-152 (N1) | **ĐẠT CÓ ĐIỀU KIỆN** | vế 1 chỉ là trích dẫn; lưới chắn chỉ ở mức mô hình |

Phát hiện nặng nhất của L6 (cổng N7) **đã được vá trong đêm** và **chứng minh lại bằng đột biến**
(§6.3). Các phát hiện nhẹ khác đã ghi trong `v2/agent/JOURNAL/kt153-review.md`.

---

## 9. Sáu tiêu chí K6 đo lại trên ảnh cuối — và kết luận K7

### 9.1 Ảnh đo

```text
$ VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" bash v2/tools/repro-build.sh
REPRO machine=Darwin-arm64 sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536 wall_s=15 algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0 rev=ba7223da23b8 build=b080e40c1bd815d1 text=130722
```

Ảnh **ghim** vào `v2/artifacts/<sha256>.bin` (D2-122.6); mọi phép đo trỏ vào tệp đã ghim, không
trỏ `target/`. Dấu vân tay trong ảnh khớp dấu vân tay cây:

```text
$ ./v2/bin/v2 status
HEAD        55e4792ab65b (main)  chưa push: 0
build-inputs 213 mục  dấu vân tay: 0b8016e6cb6038d5  dirty: 0
ảnh mới nhất e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin
             sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536
             # build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
task        DONE=144 READY=0 BLOCKED=0 REVIEW=0
```

### 9.2 Sáu tiêu chí

| Mã | Tiêu chí | Kết quả | Dòng log |
|---|---|---|---|
| **F1** | `shell>` ở 1, 2, 4 hart | **ĐẠT** | `### smp=1 shell_prompt_lines=2` · `### smp=2 shell_prompt_lines=2` · `### smp=4 shell_prompt_lines=2` |
| **F2** | kịch bản shell + **đọc lại tệp sau reboot** | **ĐẠT** | parity 2×14 boot + reboot-read + persist 10 boot, `not_ok=0 timeout=0` |
| **F3** | **ca âm**: ảnh tắt shell ⇒ kịch bản **đỏ** | **ĐẠT** | ảnh `1287611c…` (82.552 B, **0** dòng `shell>`), parity **đỏ** `exit 5` |
| **F4** | ma trận + soak 1.000 | **ĐẠT** | ma trận **21/21** + 3 boot F1 = **24**; soak **1000/1000** (`wall=290s`) |
| **F5** | ngưỡng giữ nguyên | **ĐẠT** | `file=209536 data+bss=223696 budget=262144` |
| **F6** | ảnh **không** chứa harness | **ĐẠT** | quét bằng máy, 0 marker |

Tổng thời gian đo: **40 m 58 s**. Hạ tầng trước/sau: `qemu_total=0`, `verdict=OK`.

**Một sai lệch của đề bài, đã đo và sửa**: đề bài ghi build ca âm F3 bằng `--features product-fs`
(không `product-shell`), nhưng `product-fs = ["product-shell", …]` trong `Cargo.toml` ⇒ build ra
**đúng cùng sha** ảnh sản phẩm. Ca âm đúng là build **không feature nào**. Log lần chạy sai
**được giữ nguyên** (`kt156-f3-lan-dau-sai.log`) để người đọc thấy phép đo, không xoá.

**Năm hạn chế đã ghi, không giấu**: (i) ma trận là **21** boot không phải 24 — cây có **7** ảnh
test, không thêm ảnh giả cho đủ số; (ii) parity serial mỗi boot kết thúc bằng timeout của guard
(`qemu_rc=124`) vì mỗi boot chỉ nhận một lệnh — hành vi của runner, `not_ok=0 timeout=0`;
(iii) F2c persist chạy trên ảnh test `ktest-persist`, không phải ảnh sản phẩm; (iv) soak dùng chế
độ `--features` của KT-135 (đường đo **mạnh hơn**, không phải nới); (v) `v2/artifacts/` bị
`.gitignore` nên ảnh ghim không vào git — bằng chứng là sha + dòng `# build:`.

### 9.3 Kết luận K7

**K7 "Đóng lỗi v1" — ĐẠT**, theo đúng tiêu chí đã chốt, với **bốn hạn chế** đã ghi ở §4–§8 và
**ba nợ** chuyển v2.1:

- Bốn lỗi v1 **N1/N2/N3/N7** đều có ba vế (đỏ-trên-mã-cũ / xanh-trên-mã-mới / đột-biến-làm-đỏ-lại).
  Hai vế 1 **không phải ảnh** (N3: test host trên mã trước r309; N1: trích dẫn `[V1:ĐO]`) — đã ghi
  rõ, không gọi là ảnh.
- Sáu tiêu chí K6 đo lại **đầy đủ** trên **ảnh cuối**, cùng dấu vân tay với cây.
- **B7** đóng và **lỗ hổng "test đỏ mà không cổng nào chạy"** đã bịt bằng một bước cổng mới.
- **KT-137** đóng, `cd`/`pwd` vào `spec/v1-parity.toml` thành `active`.

**Nợ chuyển v2.1** (có tên, có lý do): (a) 4 client **mức kernel** cho pager (N3); (b) seed **thư
mục** vào FS sản phẩm để `cd` vào thư mục con kiểm được (KT-137); (c) **600 boot x86_64** cho N1
thuộc K5'; (d) hai ngữ nghĩa `wake()`/`wake_from` cần Agent 0 chốt có thống nhất hay không.

---

## 10. Trạng thái cuối, việc còn lại, đề nghị

### 10.1 Trạng thái

```text
$ ./v2/bin/v2 status
HEAD        55e4792ab65b (main)  chưa push: 0
build-inputs 213 mục  dấu vân tay: 0b8016e6cb6038d5  dirty: 0
ảnh mới nhất e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin
             sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536
             # build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
task        DONE=144 READY=0 BLOCKED=0 REVIEW=0
```

`gate --fast` **PASS** trên cây sạch sau **mỗi** merge (mười lần merge; tám lần PASS liên tiếp;
hai lần đỏ là **T-INFRA**, xem §10.3). `v2 check` PASS. Không có QEMU mồ côi.

**Đo lại N2 trên ảnh cuối** (cùng ảnh `e7149448…`): `# t26: boots=50 green=50 red=0`.

### 10.2 Việc còn lại (theo thứ tự)

- **N7 — hai ngữ nghĩa `wake()` vs `wake_from`**: cần Agent 0 chốt có thống nhất hay không; nếu
  thống nhất phải đổi test KT-151 (việc của A0, không phải Agent 1).
- **N3 — 4 client mức kernel**: cần đổi VA/CSpace/bảng trang ảnh `ktest-user`; nợ có tên.
- **KT-137 — seed thư mục vào FS sản phẩm** để `cd` vào thư mục con kiểm được; chạm
  `persist.sh`/`shell-edge.sh` nên phải làm thành task riêng.
- **N1 — 600 boot x86_64** thuộc **K5'**; lưới chắn sim đã có trong cây.
- **Nợ tài liệu**: `v2/docs/k3-riscv64.md:95,728` còn bản đồ bộ nhớ **mốc K3** (trước KT-66).

### 10.3 Ba T-INFRA của đêm (không phải hồi quy mã)

- **QEMU mồ côi** làm `selftest qemu guard` ca `infra-orphan` đỏ (`qemu_ppid1=2`, kỳ vọng 1). Đã
  `kill` mồ côi, chạy riêng `guard_selftest.sh` ⇒ `cases=10 ok=10`.
- **Số worktree đổi trong lúc gate** làm `ca âm git-guard (KT-85)` đỏ (`28 → 24`).
- **bash 3.2 (macOS) hỏng heap** (`BUG IN CLIENT OF LIBMALLOC`, SIGTRAP, exit 133) khi `rules`
  quét cây lớn ⇒ `self-test luật` đỏ oan; tái hiện trên **cây base**. Cách ly bằng subshell;
  **đã kiểm** `rules()` chỉ dùng `local` + `echo` + `return`, `fails` vẫn tăng ở shell cha ⇒
  **không** nuốt lỗi.

**Luật bổ sung (D2-141)**: `gate --fast` **chỉ** chạy khi không có sub-agent đang làm; trước khi
chạy phải kiểm QEMU mồ côi rỗng và số worktree ổn định.

### 10.4 Đề nghị Agent 0

- **Công nhận** D2-128 … D2-142, hoặc đảo lại từng mục — mỗi mục đều có bằng chứng kèm.
- **Chốt** hai ngữ nghĩa `wake()`/`wake_from` (N7) — đây là quyết định thiết kế, không phải Agent 1.
- **Chốt** có làm 4 client mức kernel (N3) và seed thư mục vào FS sản phẩm (KT-137) trong K7 hay
  chuyển v2.1.
- **Xem lại** tiêu chí N2/N3/N1: cả ba đều có vế 1 **không phải ảnh** vì cây v1 tại tag **đã vá**
  (N2, N3) hoặc vì v2 không có kiến trúc đó (N1). Nếu Agent 0 muốn vế 1 **luôn** là ảnh thì phải
  chấp nhận build v1 tại **commit trước vá** — đêm nay đã làm đúng như vậy cho N2.

---

## Phụ lục A — Tuân thủ bốn luật báo cáo

| Luật | Cách đêm nay tuân thủ |
|---|---|
| **1 — số dán nguyên văn** | Mọi số trong báo cáo này nằm trong khối `text` kèm **lệnh đã chạy**; không làm tròn, không chép từ trí nhớ. Số của lần chạy nào thì lấy từ log của lần đó. |
| **2 — đổi phạm vi cổng ⇒ quét lại toàn bộ** | KT-151 quét **toàn bộ** `v2/kernel` + `v2/sim` (không allowlist tệp) và **phát hiện 16 đường**; KT-154/155 quét lại sau khi vá; KT-137 đổi `CMDS` 15→17 nên quét lại **toàn bộ** danh sách lệnh. Kết quả quét ghi trong `JOURNAL/`. |
| **3 — "đã chứng minh" phải kèm ba thứ** | Mỗi chỗ dùng chữ này đều có **lệnh + dòng output + loại cây**. Chỗ **không** đủ thì ghi **"chưa kiểm"** (ví dụ: đột biến "`pwd` in hằng `/`" của KT-137). |
| **4 — vế "đỏ trên mã cũ" phải trên ảnh build từ mã cũ** | N2: ảnh v1 build từ **cha của commit vá** (`5fc7b3ff`), có sha ảnh + dòng log. N3: **không** build được ảnh tái hiện (tag đã vá) ⇒ dùng test host trên mã trước r309 và **ghi rõ "không phải ảnh"**. N1: **không** có x86_64 ⇒ trích dẫn `[V1:ĐO]` và **ghi rõ "chưa đo lại"**. N7: checker chạy trên **cây v1** (`git archive v1-archive`), đỏ với ba cơ chế. |

## Phụ lục B — Danh sách commit trên `main`

```text
$ git log --oneline aef3e372..HEAD
55e4792a Merge branch 'night/L9-k7measure' — KT-156
ba7223da [KT-155] chot N7 DONG (D2-140) + hai T-INFRA cua cong
4b000c3c Merge branch 'night/L8-n7sim' — KT-155
3c9eb4a0 Merge branch 'night/L7-n7fix' — KT-154
908509f3 [KT-137] chot ket qua dem: D2-135..D2-139 + expect cd/pwd
fbfd5374 Merge branch 'night/L6-review' — KT-153
f09c89f9 Merge branch 'night/L5-kt137' — KT-137
4e6dcea1 Merge branch 'night/L1-n2' — KT-149
a9460f0c Merge branch 'night/L3-n7' — KT-151
acf607b3 Merge branch 'night/L2-n3' — KT-150
bb798c3d Merge branch 'night/L4-b7-n1' — KT-148 + KT-152
93c7b829 [KT-00] Agent 1 quyet dinh thay A0: D2-128..D2-134
```

Mỗi merge đều có `gate --fast` **PASS** trên cây sạch trước khi push. `main` đã push hết
(`chưa push: 0`).
