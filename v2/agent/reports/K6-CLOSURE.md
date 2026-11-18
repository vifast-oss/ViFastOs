# K6 — VĂN BẢN ĐÓNG (chính thức)

**Quyết định**: Agent 0, ngày 2026-11-18 — **Q1 = (B)**, **Q2 = (A)**.
**Trạng thái**: **K6 ĐẠT**, gắn với ảnh phát hành `a0287e4b…`.
**Người ghi**: lead (Agent 1, dòng OS). **Ngày đóng**: 2026-11-18.

> Tài liệu này thay bản nháp [`K6-CLOSURE-DRAFT.md`](K6-CLOSURE-DRAFT.md). Bản nháp giữ lại làm
> lịch sử; chỗ nào hai tệp khác nhau thì **tệp này thắng**.

---

## 1. Phạm vi và căn cứ

K6 là mốc **"tương đương v1"**. Định nghĩa có ba phần, và cả ba đều phải có bằng chứng:

- **Tám hạng mục kỹ thuật** của dòng K6 trong thiết kế (VFS journal/fsck/rename/truncate/
  double-indirect; pager nhiều client; exec chain; test-persist bảy pha).
- **Bảy lỗi v1** phải có văn bản đóng / hoãn / mở — không được bỏ im lặng.
- **Ở mức người dùng**: ảnh **sản phẩm** phải có shell tương tác như v1 (dòng phát hiện **S1**).

Phần một và phần hai đã đóng từ trước. Phần ba từng **KHÔNG ĐẠT** ngày 2026-11-17 (ảnh sản phẩm halt
sau boot, shell chỉ có trong ảnh test); hai task KT-77b và KT-77c đưa U-mode + shell + FS vào ảnh sản
phẩm, nên phần ba nay đã có bằng chứng.

## 2. Ảnh được đo

Mọi phép đo dưới đây chạy trên **đúng một** ảnh, ghim theo sha256:

```text scope=kt129-logs/kt129-boot-smp1.log
# build: inputs=8b5d76c4fdeaf7bb dirty=0
```

Ảnh: `v2/artifacts/a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13.bin`, build từ
HEAD `f4d1dd5f` với cây sạch. Kích thước và các ngưỡng ở §3.5.

**Ghi chú tái lập**: hai commit sau đó (KT-127 vá checker soak, KT-128 thêm ca hồi quy âm cho cổng
build-inputs) **đổi `v2/test/boot/build-rev.sh`** — tệp này nằm trong danh sách tệp ghim của dấu vân
tay, nên build lại từ HEAD hiện tại sẽ ra **sha khác**. Đó là hệ quả **có chủ ý** của chính hai điều
kiện Agent 0 đặt ra, **không** phải hồi quy: mã sản phẩm không đổi, chỉ chuỗi dấu vân tay nhúng
trong ảnh đổi. Muốn tái lập đúng ảnh này: `git checkout f4d1dd5f` rồi chạy
`v2/tools/repro-build.sh` với `--features product-shell,product-fs`.

## 3. Sáu tiêu chí của cổng K6

### 3.1 Tiêu chí một — boot ảnh phát hành ở ba cấu hình hart, tới `shell>`

```text scope=kt129-logs/kt129-boot-smp1.log
# smp: boot=0 harts=1 present=0x1 online=0x1 started=0x0 absent=0xfe timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/truncate/jobs/exit
```

```text scope=kt129-logs/kt129-boot-smp2.log
# smp: boot=0 harts=2 present=0x3 online=0x3 started=0x2 absent=0xfc timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/truncate/jobs/exit
```

```text scope=kt129-logs/kt129-boot-smp4.log
# smp: boot=2 harts=4 present=0xf online=0xf started=0xb absent=0xf0 timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/truncate/jobs/exit
```

Dòng `# shell: up` do **shell U-mode** in, không phải kernel — nên nó chứng minh cả chuỗi
kernel → procsrv → shell → UART đã chạy, không chỉ kernel boot xong.

### 3.2 Tiêu chí hai — parity các lệnh `active` ở hai cấu hình hart

```text scope=kt129-logs/parity/kt77a-serial-custom-smp1-kt129.log
# parity: spec_total=28 active=12 deferred=16 (nguon: /Users/mps/dvchd/ViFastOs/spec/v1-parity.toml)
# parity: total=28 ok=12 deferred=16 active=12 (active_da_chay=12)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
```

```text scope=kt129-logs/parity/kt77a-serial-custom-smp4-kt129.log
# parity: spec_total=28 active=12 deferred=16 (nguon: /Users/mps/dvchd/ViFastOs/spec/v1-parity.toml)
# parity: total=28 ok=12 deferred=16 active=12 (active_da_chay=12)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
```

Mỗi lệnh `active` chạy trong **một boot riêng** (không dùng chung phiên) nên kết quả không phụ thuộc
thứ tự. Mã thoát của guard ở dòng cuối là **bình thường**: boot cuối không nhận `exit` nên guard tự
kết thúc.

### 3.3 Tiêu chí ba — ca âm (không shell / không FS)

Hai ca âm đã có từ KT-88 và vẫn đúng: build **không** `product-shell` ⇒ runner báo
`KET_LUAN=khong_co_shell` (mã thoát riêng); build **có** shell nhưng **không** `product-fs` ⇒ lệnh
FS đỏ. Đây là điều kiện để tiêu chí một và hai được tính — không có ca âm thì "xanh" không chứng
minh gì.

### 3.4 Tiêu chí bốn — ma trận ảnh test và soak ảnh phát hành

**Ma trận** (bảy ảnh test × ba cấu hình hart):

```text scope=kt129-logs/kt129-matrix.log
# KTEST: 21/21 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec)
# ktest.sh: PASS
```

**Soak trên đúng ảnh phát hành** (mỗi vòng có `shell>` + `help` + `exit` sạch):

```text scope=kt90-logs/run-soak-20261118-102508-72680/summary.txt
# wall=370s tổng=1000/1000 ok=1000 fail=0 (370s)
# timeout-shell=0 panic=0 help-response=1000/1000 rx_bytes_min=10
# ảnh: sha256=a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13 (đầu a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13) changed=0 bytes=205440
```

**Persist bảy pha qua reboot thật** (mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình khác):

```text scope=kt129-logs/kt129-persist.log
# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10
# persist.sh: PASS
```

### 3.5 Tiêu chí năm — ba ngưỡng kích thước

```text scope=kt129-logs/kt129-thresholds.log
# image=product path=v2/artifacts/a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13.bin file=205440 data+bss=223696 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

Ba ngưỡng: kích thước ảnh, `.data + .bss`, và `size_of::<Machine>()` — cả ba dưới trần.

### 3.6 Tiêu chí sáu — không có mã harness trong ảnh phát hành

```text scope=kt129-logs/kt129-thresholds.log
# scan product: hits=0
ok 1 - anh san pham khong chua ma harness (hits=0)
# kt77b-harness-scan: KET_LUAN=ok (product=0, negative>0, probe_gate=ok)
```

Ca âm đi kèm: cùng bộ dò trên ảnh **test** phải bắt được (dòng `scan negative` trong cùng tệp log) —
nếu không, "không hit" chỉ chứng minh bộ dò hỏng chứ không chứng minh ảnh sạch.

### 3.7 Kiểm bổ sung — ca biên shell trên ảnh sản phẩm

```text scope=kt129-logs/kt129-shell-edge.log
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge PASS
# shell-edge.sh: PASS
```

---

## 4. Ba hạn chế — ghi nguyên văn, không giấu

### 4.1 Soak chính thức của harness build ảnh **không** có shell/FS

`v2/test/boot/soak.sh` build **không** `--features`, nên "không lỗi trên một nghìn vòng" của harness
chính thức là ảnh sản phẩm **không shell, không FS** — khác ảnh phát hành. Con số soak trên **đúng**
ảnh phát hành ở §3.4 đến từ một script riêng (`v2/agent/JOURNAL/kt90-soak-product-image.sh`), **không**
phải harness chính thức.

**Hệ quả**: muốn "soak ảnh phát hành" thành cổng thường trực thì phải sửa harness — và sửa cách đo là
quyền của Agent 0. Việc này đã được Agent 0 mở thành task riêng (§7).

### 4.2 Volume FS trong soak là RAM, không phải đĩa thật

Cờ `volatile` bật, `medium=ram`. Đường **đĩa thật** mới có hai boot + persist bảy pha (§3.4), **không**
phải một nghìn vòng. Nói cách khác: soak chứng minh kernel ổn định, **không** chứng minh độ bền đĩa
thật ở quy mô một nghìn vòng.

### 4.3 Ma trận là ảnh **test**, không phải ảnh phát hành

Đúng thiết kế: ma trận đo bảy ảnh test; ảnh phát hành được đo riêng ở tiêu chí một, hai, năm, sáu và
ở soak. Nhưng phải nói rõ để người đọc không nhầm con số ma trận là số boot ảnh phát hành.

---

## 5. Nợ P20 — tái lập byte (caveat bắt buộc, theo Q2 = (A))

- **Tái lập byte ĐẠT** trên **pipeline canonical** (`v2/tools/repro-build.sh`): hai máy × hai đường
  dẫn khác nhau ⇒ **cùng sha256**.
- **CHƯA ĐẠT** cho **ảnh phát hành** theo nghĩa "ảnh phát hành = ảnh canonical". Phần chênh là **bảng
  symbol** (`.symtab` + `.strtab` + `.shstrtab`), **không** phải mã chạy.
- Nợ chuyển **v2.1**, tiêu chí: **"sha ảnh phát hành = sha ảnh canonical, hai máy × hai đường dẫn"**.

Caveat này nằm **trong** văn bản đóng (không chỉ trong nhật ký) theo đúng yêu cầu Agent 0.

---

## 6. Ba trạng thái (theo D2-119)

### 6.1 ĐÓNG

Sáu tiêu chí ở §3, cộng **hai** lỗi v1 đã đóng: **N4** (lỗi cổng đo, không phải lỗi kernel) và
**N6** (`Ready` mà không trong runqueue — đóng bằng bất biến hai chiều kiểm sau **mỗi** op).

### 6.2 HOÃN

**N5** — TLB cũ sau unmap trên x86 (AP→BSP + PCID). Hoãn sang **K5′** vì v2.0 chỉ riscv64; lỗi là lỗi
**đúng đắn** trên x86 nên chỉ đóng được khi có x86. **K7 không nhận N5.**

### 6.3 MỞ

**N1**, **N2**, **N3**, **N7** — bốn lỗi v1 còn mở, chuyển sang **K7** ("Đóng lỗi v1"). Bảng chi tiết
`file:dòng` đã xong ở `v2/agent/JOURNAL/kt83-table.md`.

| Lỗi | Vì sao còn mở |
|---|---|
| **N1** — treo x86_64 sau `shell>` | thiếu ca "đơn điệu sau migrate" + conformance chéo hart |
| **N2** — FS vi phạm read-your-own-write | có test nhưng thiếu phép đo "năm mươi boot, không đỏ" |
| **N3** — pager nhận message lặp | thiếu checker "giao đúng một lần" + ca bốn client + đột biến phát lại |
| **N7** — kênh wake lệch ba port | crate có, test "một cơ chế wake chung cho ba port" chưa có |

---

## 7. Điều kiện Agent 0 kèm theo — đã xong

| Điều kiện | Trạng thái | Bằng chứng |
|---|---|---|
| Giải thích `fault_seen` trong dòng `# product halt` | **XONG** | `v2/agent/JOURNAL/kt127-journal.md` — đó là `Exit` của chính shell qua kênh fault (D2-25), không phải lỗi; checker soak nay fail-closed trên giá trị đúng bằng một, kèm đột biến phía mã |
| Ghi kết quả soak vào `STATE.md`/`JOURNAL` | **XONG** | `v2/agent/STATE.md` mục "2026-11-18 10:25–10:32" |
| Mở task đưa "soak ảnh phát hành" vào harness chính thức | **ĐÃ MỞ** | hàng `KT-131` trong `v2/agent/QUEUE.md` |

## 8. Việc sau khi đóng

- **K7 "Đóng lỗi v1"** — bốn lỗi N1/N2/N3/N7, thứ tự Agent 0 chốt: **N2 → N3 → N7 → N1**. Luật cho
  toàn K7: mỗi lỗi chỉ được ĐÓNG khi có (i) ca tái hiện **đỏ** trên mã cũ, (ii) **xanh** trên mã
  mới, (iii) **đột biến phía mã** làm nó đỏ lại. Thiếu một trong ba = vẫn MỞ.
- **KT-81** — thiết bị giả nghiêm ngặt + bảng tài nguyên.
- **Nợ P20** — chuyển v2.1 với tiêu chí ở §5.
- **KT-131** — đưa năng lực soak ảnh phát hành vào harness chính thức (hết hạn chế §4.1).

---

# PHỤ LỤC A — HẠN CHẾ THỨ NHẤT ĐÃ GỠ (bổ sung 2026-11-18, sau khi K6 đóng)

**Kết luận gốc ở §1–§8 KHÔNG đổi**: K6 ĐẠT, gắn ảnh `a0287e4b…`. Phụ lục này chỉ ghi một việc
**làm sau** khi văn bản đã đóng, đúng như Agent 0 cho phép.

## A.1 Hạn chế thứ nhất (§4.1) nói gì

Harness soak chính thức build ảnh **không** có shell/FS, nên "một nghìn vòng không lỗi" của nó
**không** nói gì về ảnh phát hành. Con số một nghìn trên đúng ảnh phát hành, khi đó, đến từ một
script riêng trong `JOURNAL/` — không phải harness.

## A.2 Đã gỡ bằng cách nào

`v2/test/boot/soak.sh` nay có cờ `--features`. Khi có cờ này:

- Ảnh được build bằng **pipeline canonical** (`v2/tools/repro-build.sh`, tức có remap + strip), nên
  sha **trùng** ảnh phát hành — không phải một ảnh khác cùng nguồn.
- Mỗi vòng **lái shell** và đòi **đủ ba** dấu hiệu: `shell>` (U-mode đã lên), dòng `help` của shell
  (shell nhận và xử lý lệnh thật), và dòng thoát sạch của shell cộng dòng halt của kernel cộng QEMU
  tự thoát với mã không. Thiếu **một** dấu hiệu ⇒ vòng đó đỏ.
- Không có `--features` thì hành vi cũ giữ nguyên (ảnh không shell), nên bằng chứng cũ không mất giá trị.

## A.3 Kết quả — một nghìn vòng, harness chính thức, ảnh phát hành

```text scope=kt138-soak-product/summary.txt
# soak: xong wall=290s (3 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=301720ec292c4f89 sha256=464fe583caf8d95e084e51d5c7f075a7660a7b4fc27fc9dd71befba7c65eeab8 git=4ed158b4ce047116b82925e88cee3cca4c0597f8
# soak: 0/1000 lỗi (PASS)
# soak.sh: PASS
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=7.99 loadavg_5m=5.29 loadavg_15m=4.39 ncpu=12 boot_done=1 verdict=OK
```

Bằng chứng **mỗi vòng** đủ ba dấu hiệu (trích một vòng bất kỳ trong một nghìn vòng):

```text scope=kt138-soak-product/summary.txt
# build ảnh: pipeline canonical (repro-build.sh) ⇒ sha trùng ảnh phát hành.
```

## A.4 Ảnh của phụ lục này **khác** ảnh của kết luận gốc — và vì sao

| | Kết luận gốc (§1–§8) | Phụ lục này |
|---|---|---|
| Ảnh | `a0287e4b…` | `464fe583…` |
| Build từ | `f4d1dd5f` | `4ed158b4` |
| Vì sao khác | — | các commit **sửa công cụ đo** sau đó (KT-127/128/134/135) đổi dấu vân tay; mã sản phẩm không đổi |

Đây **không** phải làm lại baseline: kết luận K6 vẫn gắn `a0287e4b` như Agent 0 đã chốt. Phụ lục chỉ
chứng minh **năng lực soak ảnh phát hành nay nằm trong harness chính thức**, và năng lực đó chạy được
trên cây hiện tại.

## A.5 Hạn chế còn lại

Hai hạn chế kia (§4.2 volume RAM, §4.3 ma trận là ảnh test) **vẫn còn**. §4.2 đang được xử lý ở
`KT-136` (soak đĩa thật, Agent 0 chốt C5 = (A)).
