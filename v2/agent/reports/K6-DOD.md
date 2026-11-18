<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K6-DOD — cổng K6: "tương đương v1" theo danh sách đo được + soak + L1 `seeds_total` bốn trăm nghìn

Người ghi: **Agent 2 dòng OS** · Ngày: **2026-11-17** · Task: **KT-76** (cổng K6, task cuối của K6)
· HEAD khi mở task: `3776689d` (`[KT-00] D2-117`) · Nhánh `main` · Máy `ps` (mười hai nhân, arm64);
QEMU/OpenSBI: xem khối §5.0/§5.8 (script in ra, **không** chép tay).

> **Kết luận — đọc §3.2 trước khi trích bất cứ dòng nào.**
>
> - **Tám hạng mục đo được** của thiết kế §10.1 dòng K6 (VFS journal/fsck/rename/truncate/
>   double-indirect; pager nhiều client; exec chain; “test-persist 7 pha”): **ĐẠT** — mỗi hạng mục
>   có `log:dòng` **nằm trong repo** và trích nguyên văn (§3.1 + §3.3).
> - **"Tương đương v1" ở mức NGƯỜI DÙNG: KHÔNG ĐẠT.** Ảnh **sản phẩm** vẫn không có U-mode/shell
>   (kernel `halt` sau boot), trong khi v1 có shell tương tác **trên chính ảnh đó** (§3.2 — dòng
>   **S1**). Vì vậy tài liệu này **không** tuyên bố K6 "tương đương v1"; đề xuất **KT-77** hoặc hạ
>   phạm vi **có văn bản của Agent 0**.
> - Bảy lỗi v1 (§4): **ĐẠT** N4/N6; **KHÔNG ĐẠT** N1/N2/N3/N7 (N1 **một phần**); **KHÔNG ÁP DỤNG
  ở K6** N5.
> - Cổng nặng đã chạy, số **nguyên văn trong khối** (§5): `gate --fast`, matrix ảnh × smp,
>   soak ảnh sản phẩm, `test-persist`, conformance, **L1 bốn cấu hình × `seeds` một trăm nghìn**.

**Phạm vi KT-76**: **chỉ** kiểm + viết DOD trung thực + chạy cổng. **Không** thêm tính năng,
không sửa `spec/`, không sửa test/ngưỡng, không sửa `docs/v2/ViFastOs-v2-thiet-ke.md`, không sửa
`docs/v1/` (đọc-only), không chạm `viscript/`/`review/`.

**Đề xuất trạng thái (khớp quyết định Agent 0 sau review)**: **KT-76 = DONE** (task cổng: DOD trung
thực + cổng xanh), **nhưng phase K6 = KHÔNG ĐẠT** ở nghĩa "tương đương v1" vì **S1**. Agent 0
**mở KT-77** (đưa U-mode/shell vào ảnh sản phẩm); cho tới khi KT-77 xong, **không** được viết
"K6 đạt tương đương v1". Mục BLOCKED tương ứng: `BLOCKED.md` §KT-76.

## 0. TL;DR — D0..D9

| # | Việc | Kết quả | Bằng chứng |
|---|---|---|---|
| D0 | HEAD + ảnh sản phẩm **không feature** (D2-108A) + QEMU orphan (D2-116) | sha/BUILD_ID trong khối §5.0; **tái lập byte CHƯA chứng minh** (clean-build khác sha — P20) | `kt76-build-sha.log`, `kt76-clean-build.log`, `kt76-orphan.log` |
| D1 | `./v2/bin/v2 gate --fast` | **PASS**; số test/suite + L1 (số trong §5.1) | `kt76-baseline-gate.log`, `kt76-baseline-gate-summary.log`, `kt76-gate.log` |
| D2 | `ktest.sh` (`NBOOT` một, `SMP` matrix) | **mọi ô ảnh × smp sạch** (§5.2) | `kt76-ktest-matrix.log` |
| D3 | `soak.sh` (`N` một nghìn, `LANES` ba) | **`fail` bằng không**, sha **không đổi** (§5.3) | `kt76-soak.log` |
| D4 | `bash v2/test/boot/persist.sh` | **PASS**, đủ pha, mỗi pha hai tiến trình khác nhau (§5.4) | `kt76-persist-run.log` |
| D5 | conformance `run.sh` (`--boot`, `SMP` hai) | **PASS** (§5.5) | `kt76-conformance.log` |
| D6 | **L1 `seeds_total` bốn trăm nghìn** (bốn cấu hình × `seeds` một trăm nghìn × `op` sáu mươi tư) | **bốn tiến trình song song**, `EXIT` bằng không từng cấu hình + wall (§5.6) | `kt76-l1-smp1.log` … `kt76-l1-smp8.log`, `kt76-l1-timing.txt` |
| D7 | "tương đương v1" **theo danh sách đo được** | **ĐẠT** C1..C8 (§3.1) | §3.3 (trích log) |
| D8 | shell/U-mode trên **ảnh sản phẩm** | **KHÔNG ĐẠT** — dòng S1 (§3.2) | `kt76-product-umode.log`, `kt76-product-serial.log` |
| D9 | Bảy lỗi v1 | N4/N6 ĐẠT; N1/N2/N3/N7 **KHÔNG ĐẠT** (N1 một phần); N5 N/A (§4) | §4 |

## 1. Tiêu chí NGUYÊN VĂN (máy kiểm, không chép tay)

Tiêu chí của DOD này **không** do tôi viết lại: chúng được trích **nguyên văn** từ hai nguồn
**đọc-only** bằng `JOURNAL/kt76-extract-criteria.sh` (gọi `JOURNAL/kt76_extract_criteria.py`):

- `docs/v2/ViFastOs-v2-thiet-ke.md` §10.1 — **dòng K6** của bảng DÒNG LÕI (cấm sửa);
- `docs/v1/KNOWN-ISSUES.md` — **bảy** dòng bảng, **đủ sáu cột** — gồm cột **“Ca kiểm tương ứng
  trong v2”** và **“Trạng thái”** (cấm sửa). Bỏ cột nào cũng là **cắt tiêu chí**: cột “ca kiểm”
  chính là cột nói **test nào phải có**.

Script kiểm `JOURNAL/kt76-verify-numbers.sh` **sinh lại** tệp rồi so **từng byte** với
`JOURNAL/kt76-criteria.txt`; ai sửa tiêu chí (hoặc sửa tệp trích) ⇒ cổng kiểm **đỏ**. Mã dùng
trong bảng: **C1..C8** = tám hạng mục của dòng K6; **N1..N7** = bảy dòng sổ lỗi v1; **S1** = dòng
"tương đương v1 ở mức người dùng" (§3.2 — không có trong §10.1, đây là **phát hiện của KT-76**).

```text scope=kt76-criteria.txt
# Nguồn 1: docs/v2/ViFastOs-v2-thiet-ke.md §10.1 — dòng K6 (nguyên văn, đọc-only)
| K6 | memsrv, vifast-alloc, capalloc, IDL + ring, VFS (pager, page cache, journal), virtio-blk/console, SetTls | "Tương đương v1" theo danh sách đo được (VFS journal/fsck/rename/truncate/double-indirect; pager nhiều client; exec chain; test-persist 7 pha) |
# Nguồn 2: docs/v1/KNOWN-ISSUES.md — bảy dòng bảng (nguyên văn, đọc-only)
N1 | **T33a** — treo x86_64 sau `shell>` `[V1:ĐO]` | ~0,3–0,5 % lần boot x86_64 (TCG, `-smp 2`, soak song song 3 luồng) đứng sau `shell>`, không in `PREEMPT OK`. Gốc đã lộ một nửa: worker C thấy `TIME MONOTONIC BAD` → `panic!` → root chờ mãi | §15.1: **600 boot** x86_64 song song 3 luồng, `VIFAST_NO_RETRY=1`, **0 treo**, **0** `TIME MONOTONIC BAD`, **0** `ROOT PANIC`, **và** cơ chế được viết thành test host (§15.10) | Kịch bản sim **"đơn điệu sau migrate"** + conformance chéo hart; v2 không có trạng thái reset (QD §6) | **Mang sang v2** (Q3: không sửa trên v1; Q4: thành kịch bản đơn điệu đồng hồ)
N2 | **T26** — FS vi phạm read-your-own-write `[V1:ĐO]` | Ở `MAX_IO=256`, `smp2`: **10/50 = 20 %** boot đỏ; entry thư mục ghi xong, liệt kê ngay sau không thấy, lần sau thấy. `-accel tcg,thread=single` vẫn 1/3 ⇒ race **logic**, không phải race bộ nhớ host | §15.2: có phép đo **tất định** tái hiện vi phạm; bản vá làm nó xanh **và** tỉ lệ boot đỏ ở `MAX_IO=256` về **0/50**; sau đó mới bàn nâng `MAX_IO` về 256 | **KT-T26**: host test `vifast-fs` + block giả + cache, đan xen theo seed, `MAX_IO=256`, kiểm read-your-own-write bằng đọc **RAW**; đánh **"ai chen giữa"** theo D-03 | **Mang sang v2** (Q5; D-04 giữ cấu hình tái hiện)
N3 | **T31** — PAGER2 nhận message lặp `[V1:ĐO]` | `PAGER EVICT OK` in **hai lần** ⇒ nhánh xử lý chạy ≥ 3 lần ⇒ có message bị giao/xử lý lặp. r309 đã thêm bảng `pager2_step(seen)` + marker `PAGER2 DUP`, **chưa** tìm đường giao lặp | §15.3 + §15.10: tìm được đường giao lặp và có test tất định canh cơ chế (đếm `USER FAULT … addr=0x1001001000`: 2 lần ⇒ replay phía server; 3 lần ⇒ phía client) | Kịch bản sim **pager 4 client** + checker "giao đúng một lần" + **đột biến "phát lại Call khi Resume"** (QD §6) | **Mang sang v2**
N4 | **T-IPC** — lỗi cổng đo `[V1:ĐO]` | "Boot đỏ" r307 thực chất là cổng `test-exit` thoát đúng lúc client đang chạy user. r308 vá: hoãn exit một tick khi task còn `Running` ⇒ 0/80 boot thiếu marker. Đã đóng, giữ làm bài học | §15.4: 0/80 boot thiếu marker (đã đạt ở r308) + bài học "kiểm đường thoát trước khi nghi kernel" | **Luật**: mọi harness mới phải có **case âm** chứng minh nó bắt được lỗi trước khi dùng để tuyên bố (QD §6) — hiện thực ở `harness-v2` (`FORBIDDEN`, `EXACT_ONE`, bộ case âm cho chính harness) | **Đã đóng ở v1**; bài học **mang sang v2** dưới dạng luật harness
N5 | **x86 AP→BSP + PCID** — TLB cũ `[V1:CODE]` | Lỗi **đúng đắn**: sau unmap, TLB của BSP có thể còn cũ ⇒ tiến trình đọc được trang đã thu hồi. Không lộ trên TCG mặc định nên mọi cổng vẫn xanh | §15.6: có test (host hoặc QEMU `-cpu max`/KVM) chứng minh TLB cũ gây **đọc sai**; bản vá làm nó xanh; **hoặc** tắt PCID có kiểm soát + ghi lý do | Đột biến **"bỏ xử lý hộp thư khi chờ ack"** + conformance TLB khi tới x86 (QD §6); x86 chỉ làm sau K6 (D2-10) | **Mang sang v2**, mở ở mốc K5' (x86_64)
N6 | **Audit P1-5** — `Ready` mà không trong runqueue `[V1:DOC]` | `enqueue` trả sớm ⇒ TCB mang trạng thái `Ready` nhưng **không** nằm trong runqueue (liên quan K37–K40). Audit ghi **OPEN** | §15.8: đọc lại `enqueue`, đóng theo invariant "ở trong hàng X khi và chỉ khi trạng thái mang X"; §15.10 cho điều kiện "đã sửa" | Invariant **hàng đợi khớp trạng thái** (đã có trong thiết kế) + kịch bản riêng (QD §6) | **Mang sang v2** dưới dạng invariant + kịch bản
N7 | **Audit Q1-A** — kênh wake lệch 3 port `[V1:DOC]` | Trong `Mailbox`, kênh đánh thức chưa thống nhất 3 port: riscv64 có doorbell riêng cho reschedule, arm/x86 dùng mailbox chung. Nợ từ r293 | §15.8: thống nhất sau r293–r298; đóng khi một cơ chế chung cho cả 3 port | **`vifast-sync` dùng chung cho mọi arch** (QD §6); v2 chỉ riscv64 tới K6 nên ca kiểm chạy ở sim + riscv64 trước | **Mang sang v2** (thiết kế lại hộp thư theo hart)
```

## 2. Nội dung K6 đã giao (cột "Nội dung" §10.1) — bản đồ bằng chứng

| Hạng mục của K6 | Task | Ảnh/bằng chứng đo | Ghi chú |
|---|---|---|---|
| memsrv (U-mode) + `vifast-alloc` | KT-67, KT-67a, KT-67b | `ktest`, `ktest-user` — §5.2 + gate §5.1 | allocator Frame, chuyển cap hai tiến trình |
| capalloc (cấp/thu hồi theo cây MDB) | KT-68 | `ktest-user` — §5.2 | `Revoke` cây, `Delete` reparent |
| IDL tối thiểu + ring chia sẻ | KT-69 | `ktest-user` — §5.2 | method id sinh từ `.vidl` |
| VFS: pager + page cache | KT-70 | `ktest-user` — C6 §3.1 | backend thay được (RAM → virtio) |
| VFS: journal/fsck/rename/truncate/double-indirect | KT-71 | `ktest` — C1..C5 §3.1 | bảy pha crash + redo |
| virtio-blk thật | KT-72 | `ktest-vblk` — §5.2 | **chỉ ảnh test** |
| virtio-console + UART RX IRQ | KT-73 | `ktest-con` — §5.2 | **chỉ ảnh test** |
| `Tcb.SetTls` + `exec` chain | KT-74 | `ktest-exec` — C7 §3.1 | TLS quan sát tại trap |
| test-persist bảy pha | KT-75 | `ktest-persist` — C8 §3.1 | mỗi pha: ghi/đọc ở tiến trình khác |

## 3. Bảng DOD — từng tiêu chí (§10.1) + dòng phát hiện S1

### 3.1 Tám hạng mục đo được của §10.1 dòng K6

| Mã | Tiêu chí (nguyên văn ở §1) | Kết quả | Bằng chứng (`log:dòng`) | Ghi chú |
|---|---|---|---|---|
| C1 | “VFS journal” | **ĐẠT** | `kt71-green-qemu-ktest.log:309` | commit record sau flush thân log; replay sau crash; ca âm CRC (`crc_reject`) |
| C2 | “fsck” | **ĐẠT** | `kt71-green-qemu-ktest.log:320` | bốn loại hỏng; ca **hai chủ** không sửa mù (`unfixed2`, `second_bad`) |
| C3 | “rename” | **ĐẠT** | `kt71-green-qemu-ktest.log:302` | rename **đè** lên đích, `replaced_gone` |
| C4 | “truncate” | **ĐẠT** | `kt71-green-qemu-ktest.log:303` | giải phóng block + **dùng lại** (`reuse_ok`) |
| C5 | “double-indirect” | **ĐẠT** | `kt71-green-qemu-ktest.log:304` | đi qua **L2 thật** (`index` một trăm bốn mươi ba), không phải mô phỏng |
| C6 | “pager nhiều client” | **ĐẠT** | `kt70-green-qemu-ktest-user.log:231` | **hai** client cùng một Frame PA; hit không đọc backend |
| C7 | “exec chain” | **ĐẠT** | `kt74-qemu-ktest-exec.log:95` | chuỗi ba bước thật, `objects/frames/caps` **không đổi** |
| C8 | “test-persist 7 pha” | **ĐẠT** | `kt76-persist-run.log:81` | bảy pha, mỗi pha **ghi ở một tiến trình, đọc ở tiến trình khác** |

**Hành vi còn xanh trên cây HÔM NAY** (không chỉ log của task gốc): matrix §5.2 chạy lại **cả
bảy** ảnh test — trong đó `ktest` (C1..C5), `ktest-user` (C6) và `ktest-exec` (C7) — với **đủ dãy
TAP** mà `v2/test/boot/expected.txt` quy định; `ktest-persist` (C8) do runner riêng điều khiển ở
§5.4. Vì vậy bằng chứng C1..C8 không phải "log cũ chưa kiểm lại".

### 3.2 Dòng phát hiện — "tương đương v1" ở mức NGƯỜI DÙNG

| Mã | Tiêu chí | Kết quả | Bằng chứng (`log:dòng`) | Ghi chú |
|---|---|---|---|---|
| **S1** | **Ảnh SẢN PHẨM có U-mode/shell tương tác như v1** (điều kiện để gọi "tương đương v1" ở mức người dùng) | **KHÔNG ĐẠT** | `kt76-product-umode.log:3`, `kt76-product-umode.log:7`, `kt76-product-serial.log:83` | Ảnh sản phẩm `halt` sau `# k3 halt`; U-mode nằm sau feature `user`; **shell chỉ có trong ảnh test** `ktest-proc`. v1 có `shell>` + `--input-check` trên **chính** ảnh boot (`docs/v1/agent2-report-21.md:83`, `docs/v1/overnight-log.md:1499`) |

**Vì sao S1 là KHÔNG ĐẠT (không phải N/A):**
- `v2/kernel/vifast-kbin-riscv64/src/main.rs:361-362`: ảnh sản phẩm in dòng `ok` cho `k3 boot`,
  `# k3 halt` rồi `halt(0)` — không có đường xuống U-mode.
- `v2/kernel/vifast-plat-riscv64/src/lib.rs:104-105`: module `user` (U-mode) chỉ biên dịch với
  `feature = "user"` — mà ảnh sản phẩm build **không feature** (D2-108A). Cùng lý do:
  console/UART (`lib.rs:99-100`, `lib.rs:123-124`).
- Shell thật (**có** U-mode, **có** đọc bàn phím) chỉ tồn tại ở ảnh test `ktest-proc` — xem
  `ktest-proc` trong §5.2. **Bằng chứng ảnh test không thay được bằng chứng ảnh sản phẩm** —
  đúng cảnh báo D2-113, và tôi **không** dùng nó để nhận "tương đương v1".
- v1 **có** shell trên ảnh boot: client shell với `help/time/echo/ls/cat/mkdir/rm/mv/ln/stat/
  sync/truncate/fsck` (`docs/v1/agent2-report-21.md:83`) và cổng `--input-check` đòi echo
  `shell> e` trên riscv (`docs/v1/overnight-log.md:1499`).

**Đề xuất (chọn một, Agent 0 quyết):**

- **Cách một — KT-77 — đưa U-mode/shell vào ảnh sản phẩm**: bật `feature = "user"` cho bin sản phẩm + nhúng
   `procsrv`/`shell` ELF + bật `vcon`/UART IRQ; đo lại BUILD_ID/soak/trần ảnh (hiện driver console
   đã có trong ảnh test, trần **không** phải lý do — `kt76-budget.log`).
- **Cách hai — hạ phạm vi có văn bản**: Agent 0 sửa thiết kế/ra D2 nói rõ "tương đương v1 **ở mức tính năng
   kernel**" — khi đó S1 thành N/A **có nguồn**, không phải do tôi tự định nghĩa lại.
- **Không có cách thứ ba**: **không** được tính "tương đương v1" khi S1 còn KHÔNG ĐẠT.

### 3.3 Trích NGUYÊN VĂN dòng bằng chứng (C1..C8)

```text scope=kt71-green-qemu-ktest.log
# fs: rename src=2 exp=2 replaced_gone=1 dir_over=1 used=6->2 impl=rename-overwrite
# fs: truncate used=6->2 freed=4 read_empty=true reuse=4 (=used_again-used_t0) used_again=6 used_t0=2 reuse_ok=true const:from_blocks=4
# fs: double index=143 used=6->9->6 delta=3 (=3: root+bang L2+data) root=37 l2=38 data_ptr=36 single=0 read=4 data=2CAP fsck=true
# fs: rename_ok=1 trunc_ok=true double_ok=true fsck=true const:disk_blocks=48 const:direct=10
# fs: journal writes=7 issued=7 commit_prefix=4 (=writes-3: 2 target + 1 clear) replay=1 new=1 size=7 keep=true fsck=true crc_reject=2 crc_applied=0 crc_disk=old reorder_writes=5 kept=true pre=old replay_r=1 new_r=1 post=new keep_r=true
# fs: fsck detect=1 found=4 fixed=4 unfixed=0 orphan_blocks=1 orphan_inodes=1 bad_nlink=1 bad_blocks=1 used=2->4->2 keep=true fsck_ok=true second=0 unfixed_pos=true unfixed2=1 double_blocks2=1 second_bad=true const:disk_blocks=48
```

```text scope=kt70-green-qemu-ktest-user.log
# pager: cache const:capacity=3 hits=2 misses=7 evictions=4 reads=7 sectors=56 served=9 rejected=1 entries=3 unique=3 (=entries(3) nếu keys_unique) shared=2 reloads=2 const:clients=2 magic=0x4b54373050414745 const:magic_expected=0x4b54373050414745
# pager: map p2_a=0x82034000 p2_b=0x82034000 p2_same=1 (=p2_a==p2_b) hit_delta0=1 (reads_before=7 reads_after=7) mapping_bad=0 walks_ok=9
ok 33 - pager hai client chia se mot trang
```

```text scope=kt74-qemu-ktest-exec.log
# exec chain: execs=3 pid=0 tcb=3 exits=3,4,1 recycled=33 freed=33 objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 -> objects=17 frames=11 caps=14 vspaces=1 page_tables=3 mappings=11 frames_same=1 objects_same=1 caps_same=1 total=4 ok=1
ok 4 - exec chain 3 buoc
```

```text scope=kt76-persist-run.log
# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10
# reboot: boots=10 pids=10 phase_reboots=7 (7 pha; mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình KHÁC)
# phase_reboot: phase=1 write_boot=b01 write_pid=70352 verify_boot=b02 verify_pid=70386 distinct_process=1
# phase_reboot: phase=7 write_boot=b07 write_pid=70502 verify_boot=b08 verify_pid=70535 distinct_process=1
# persist.sh: PASS
```

## 4. Bảy lỗi v1 (KNOWN-ISSUES) — trạng thái ở K6

> **ĐÍNH CHÍNH (KT-83, 2026-11-17)** — ba chỗ sai trong bản gốc, giữ nguyên nội dung cũ để tra:
>
> - **N2**: dẫn `kt76-baseline-gate.log:250`, `kt76-baseline-gate.log:251`, `kt76-baseline-gate.log:252`
>   — dòng cuối **trống**; ba test nằm ở `kt76-baseline-gate.log:249-251`.
> - **N7**: dẫn `kt76-baseline-gate.log:1202` — đó là dòng `Running tests/kt30_sync.rs`; kết quả test
>   ở `kt76-baseline-gate.log:1204-1215`.
> - **N5**: bản gốc ghi "KHÔNG ÁP DỤNG ở K6" / "N/A". Theo **D2-119** và **D2-120**, trạng thái
>   đúng là **HOÃN sang K5′** (x86_64 ngoài phạm vi v2.0), **không** phải N/A — và **K7 không nhận N5**.
>
> Bảng đầy đủ với `file:dòng` đã kiểm: `v2/agent/JOURNAL/kt83-table.md`.

| Mã | Lỗi v1 / tiêu chí đóng gốc (nguyên văn §1) | Kết quả | Bằng chứng (`log:dòng`) | Ghi chú |
|---|---|---|---|---|
| **N1** | Treo sau `shell>` trên `x86_64` (T33a) — cột “Ca kiểm tương ứng trong v2”: **kịch bản sim “đơn điệu sau migrate” + conformance chéo hart** | **KHÔNG ĐẠT (một phần)** | `kt76-baseline-gate.log:1096` | Phần **phần cứng x86** hoãn **K5'** (D2-10) **nhưng ca kiểm sim KHÔNG bị hoãn**: hiện chỉ có `ClockRead` đơn điệu **một hart** (`v2/sim/vifast-sim/tests/kt62_syscall.rs:862-871`), **không** có migrate ⇒ chưa đạt “đơn điệu **sau migrate**” ⇒ nợ **sim/K5'** |
| **N2** | FS vi phạm read-your-own-write ở `MAX_IO` lớn (T26) — cần phép đo **tất định tái hiện vi phạm** + “0/50” boot đỏ | **KHÔNG ĐẠT** | `kt76-baseline-gate.log:250`, `kt76-baseline-gate.log:251`, `kt76-baseline-gate.log:252` | Đèn host **có** và xanh (`seeds` một nghìn + ca âm mất ghi im lặng) nhưng **không** tái hiện vi phạm (v2 ghi qua `commit()`+`flush()` ngay) và **không** có phép đo `boots` năm mươi với `boot_red` bằng không ⇒ chưa đóng tiêu chí gốc |
| **N3** | PAGER2 nhận message lặp (T31) — cần tìm **đường giao lặp** + test đếm `USER FAULT` | **KHÔNG ĐẠT** | `kt70-green-qemu-ktest-user.log:230`, `kt70-green-qemu-ktest-user.log:231` | Pager v2 có **hai** client + đối chiếu `faults` với `served + rejected`, nhưng **không** có checker "giao **đúng một lần**", **không** có ca `clients` bốn, **không** có đột biến "phát lại Call khi Resume" |
| **N4** | Lỗi cổng đo `test-exit` (T-IPC) — bài học thành **luật harness có ca âm** | **ĐẠT** | `kt76-selftest.log:35` | Harness tự kiểm bằng **ca âm** (mutation log/expected.txt phải đỏ **kèm đúng token**); mọi cổng boot đòi đúng dãy TAP + evidence (không chấp nhận “không boot nào mà PASS”) |
| **N5** | TLB cũ sau unmap + PCID trên `x86` | **KHÔNG ÁP DỤNG ở K6** | — | D2-10: `x86` sau K6; mở ở **K5'** |
| **N6** | `Ready` mà không trong runqueue (Audit P1-5) — invariant "trong hàng ⇔ trạng thái" | **ĐẠT** | `kt76-baseline-gate.log:668` | `check_sched` kiểm **hai chiều** (Ready ⇔ trong hàng; Running ⇔ current) — `v2/sim/vifast-sim/src/kobj.rs:7188`; test chạy sau **mỗi** op công khai |
| **N7** | Kênh wake lệch ba port (Audit Q1-A) — cột “Ca kiểm tương ứng trong v2”: `vifast-sync` dùng chung; **ca kiểm chạy ở sim + riscv64** | **KHÔNG ĐẠT** | `kt76-baseline-gate.log:1202` | Crate/test **có** (`v2/lib/vifast-sync/tests/kt30_sync.rs`: mailbox + MCS, mười test) nhưng **không** có test “**một cơ chế wake chung**/ba port” ⇒ chưa đóng; D2-10 chỉ hoãn **phần cứng** x86, **không** hoãn test sim |

## 5. Cổng nặng — lệnh + số nguyên văn

### 5.0 Baseline, ảnh sản phẩm build SẠCH, QEMU orphan

- HEAD khi mở: `3776689d`; trong lúc làm, dòng ViScript commit thêm hai lần (`c9249b79`, `1f522686`)
  — mỗi lần **chỉ** chạm `viscript/agent/**` (`git show --stat`), `dirty-kernel-paths` bằng không ở
  `kt76-soak.log:5` ⇒ `v2/` **không** đổi. Ảnh sản phẩm **không đổi byte** so KT-75 ⇒
  soak/soak-sha dùng đúng ảnh đó.
- **Ảnh sản phẩm**: artifact `target/release/vifast-kbin-riscv64` — sha `fd202276…` (đủ ở khối
  dưới), `build=ef566cfa583e34bb`, `bytes` một trăm tám mươi tám nghìn năm trăm năm mươi hai, **không feature** (D2-108A: ảnh **không** lẫn
  `ktest`, `kt76-soak.log:10`), **không đổi byte** qua các lần build ảnh test (soak log `kt76-soak.log:11` và
  `kt76-soak.log:1016`) và **trùng** KT-75. **NHƯNG** dòng `Finished … in 0.03s` trong log build = cargo **tái
  dùng** artifact có sẵn (**không** compile lại) ⇒ đây **không** phải bằng chứng “build sạch sinh
  ra đúng sha này”. Dựng lại **sạch** bằng `git archive` tại `1f522686` trong **target dir riêng**
  cho **sha khác** (`29d7e875…`, `build=317ffc48e0e71354`) — cùng kích thước nhưng `.text` **khác
  digest** ⇒ **tính tái lập byte của build CHƯA được chứng minh**; soak chạy trên artifact của
  **cây làm việc**, không phải artifact clean. Ghi **nợ P20** (§6).
- **QEMU orphan (D2-116)**: trước chuỗi cổng `qemu_ppid1` bằng không (`kt76-orphan.log:4`); lúc build
  (07:08) đếm được hai tiến trình QEMU **không** phải của tôi (không `ppid` bằng một) — ghi lại, **không**
  dùng số thời gian của lần đo nào trong lúc đó để kết luận hiệu năng. Sau chuỗi: **dòng cuối** `kt76-orphan.log`
  (khối `kt76-orphan.log`).

```text scope=kt76-build-sha.log
# KT-76 — baseline: HEAD + máy + ảnh SẢN PHẨM build SẠCH (KHÔNG feature, D2-108A)
# HEAD=3776689d
# head_subject=[KT-00] D2-117: KT-75 DONE (persist qua reboot that, flush ca am dung tang); mo KT-76 cong K6
# origin_main_delta=0
# date_utc=2026-11-16T22:08:21Z
# qemu=QEMU emulator version 11.1.3
# opensbi: OpenSBI tự in trong serial boot — `kt76-product-serial.log:4` (không chép tay)
# cpu=12 host=arm64 os=27.0.1
# qemu_orphan_before=2
# cargo=cargo 1.99.1 (5f94df478 2026-08-27)
# lệnh: cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --release
    Finished `release` profile [optimized] target(s) in 0.03s
BUILD_EXIT=0
# bytes=188552
fd20227658832a3718b390b0cd7aab563396e23e01ecbd04632f7a9c4393a6da  v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
build=ef566cfa583e34bb text=49874
```

```text scope=kt76-clean-build.log
# KT-76 — D (reviewer KT-76): dựng bản SẠCH bằng git archive tại 1f522686, build trong TARGET DIR RIÊNG
# lệnh: git archive 1f522686 | tar -x -C /tmp/kt76-clean ; CARGO_TARGET_DIR=/tmp/kt76-clean-target cargo build ... --release
# date_utc=2026-11-16T23:27:27Z
# worktree_git_status_v2=0 (nguồn của artifact soak: cây làm việc)
   Compiling vifast-abi v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-abi)
   Compiling vifast-pte v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-pte)
   Compiling vifast-alloc v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-alloc)
   Compiling vifast-virtio v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-virtio)
   Compiling vifast-ring v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-ring)
   Compiling vifast-elf v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-elf)
   Compiling vifast-kbin-riscv64 v0.1.0 (/private/tmp/kt76-clean/v2/kernel/vifast-kbin-riscv64)
   Compiling vifast-fs v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-fs)
   Compiling vifast-idl v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-idl)
   Compiling vifast-traceformat v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-traceformat)
   Compiling vifast-scenario v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-scenario)
   Compiling vifast-core v0.1.0 (/private/tmp/kt76-clean/v2/kernel/vifast-core)
   Compiling vifast-vfs v0.1.0 (/private/tmp/kt76-clean/v2/lib/vifast-vfs)
   Compiling vifast-plat-common v0.1.0 (/private/tmp/kt76-clean/v2/kernel/vifast-plat-common)
   Compiling vifast-plat-riscv64 v0.1.0 (/private/tmp/kt76-clean/v2/kernel/vifast-plat-riscv64)
    Finished `release` profile [optimized] target(s) in 6.44s
CLEAN_BUILD_EXIT=0
# bytes=188552
29d7e875d034dc1eadc6ee2e0571631b581798b2c44819e29eab096fcd6ba146  /tmp/kt76-clean-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
build=317ffc48e0e71354 text=49874
```

```text scope=kt76-orphan.log
# KT-76 — kiểm QEMU orphan (D2-116) trước chuỗi cổng
# date_utc=2026-11-16T22:14:41Z
# qemu_all=0
# qemu_ppid1=0
# ghi chú: lúc 07:08 (build) đếm được 2 tiến trình qemu KHÔNG phải của tôi (không có PPID 1); nay 0.
# sau chuỗi cổng + L1 (date_utc=2026-11-16T23:03:03Z):
# qemu_all=0 qemu_ppid1=0
```

### 5.1 `./v2/bin/v2 gate --fast`

Lệnh chạy **không retry** (D2-07) trên cây có `v2/` sạch (chỉ thêm tài liệu/log của task này).
Số L1 của cổng là **wall cả lệnh `cargo test`** (D2-100) — không dùng làm số hiệu năng; số L1
thuần của cổng cuối K6 nằm ở §5.6:

```text scope=kt76-baseline-gate-summary.log
TESTS=729 / SUITES=100
```

```text scope=kt76-baseline-gate.log
# L1 OK: 1000 seed, 999 trace khác nhau
--- L1: OK (1069 ms, đo bởi cổng)
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
EXIT=0
```

Gate **cuối** (sau khi tài liệu/log của task đã có mặt — chứng minh cây cuối vẫn xanh):

```text scope=kt76-gate-summary.log
TESTS=729 / SUITES=100
```

```text scope=kt76-gate.log
--- L1: OK (915 ms, đo bởi cổng)
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
EXIT=0
```

### 5.2 Matrix `ktest.sh` (`NBOOT` một, `SMP` matrix)

```text scope=kt76-ktest-matrix.log
# KTEST: 21/21 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec)
# ktest.sh: PASS
MATRIX_EXIT=0
```

### 5.3 Soak ảnh sản phẩm `soak.sh` (`N` một nghìn, `LANES` ba)

```text scope=kt76-soak.log
# image=product bytes=188552 sha256=fd20227658832a3718b390b0cd7aab563396e23e01ecbd04632f7a9c4393a6da
# BUILD_ID (D2-80) = build=ef566cfa583e34bb text=49874
# soak: xong wall=227s (4 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=ef566cfa583e34bb sha256=fd20227658832a3718b390b0cd7aab563396e23e01ecbd04632f7a9c4393a6da git=c9249b7989a5978a8010faf96121e3b608606bec
# soak: 0/1000 lỗi (PASS)
# soak.sh: PASS
SOAK_EXIT=0
```

### 5.4 Persist `bash v2/test/boot/persist.sh`

```text scope=kt76-persist-run.log
# host: phase=7 records=14 files=3 fsck=1 fail=0 img_sha256=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389 fsutil_mutated=0
# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10
# reboot: boots=10 pids=10 phase_reboots=7 (7 pha; mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình KHÁC)
# phase_reboot: phase=7 write_boot=b07 write_pid=70502 verify_boot=b08 verify_pid=70535 distinct_process=1
# persist.sh: PASS
PERSIST_EXIT=0
```

### 5.5 Conformance `run.sh` (`--boot`, `SMP` hai)

```text scope=kt76-conformance.log
scn-check: PASS (27 tệp, 0 blocked, 16 nhóm)
ok - trace-compare: 61/61 dòng khớp, 5/5 ca (22:16 23:19 24:7 25:7 26:12), build=7304a40491224327
# trace-compare PASS (/tmp/k3-kt55/sim-trace.log ↔ /tmp/k3-ktest/ktest-smp2-1.log)
CONF_EXIT=0
```

### 5.6 L1 `seeds_total` bốn trăm nghìn — bốn cấu hình, bốn tiến trình SONG SONG

```text scope=kt76-l1-timing.txt
# KT-76 — L1 400.000 lượt: bốn cấu hình smp 1/2/4/8 × 100.000 seed × 64 op, BỐN tiến trình SONG SONG
# lệnh mỗi cấu hình: ./v2/bin/v2 nightly --l1 --seeds 100000 --smp <N>  (log riêng kt76-l1-smp<N>.log, có EXIT=…)
# qemu_orphan_before=0
start_utc=2026-11-16T22:21:20Z
end_utc=2026-11-16T22:56:09Z
wall_s=2089
# qemu_orphan_after=0
# xong: mỗi log phải có '# L1 OK: 100000 lượt' + 'EXIT=0'
```

```text scope=kt76-l1-smp1.log
# L1 OK: smp 1, 100000 seed × 64 op (2000 seed đầu chạy 2 lần; ipc_min 2, ipc_median 12; ipc_sent 1212128, ipc_delivered 226334, ipc_dropped 782515; vm map 257149, walk 71791, unmap 135089, fault 237472, shootdown 135089)
# L1 OK: 100000 lượt (smp [1], seeds 100000)
EXIT=0
```

```text scope=kt76-l1-smp2.log
# L1 OK: smp 2, 100000 seed × 64 op (2000 seed đầu chạy 2 lần; ipc_min 1, ipc_median 12; ipc_sent 1165183, ipc_delivered 217375, ipc_dropped 734940; vm map 257080, walk 71601, unmap 135157, fault 232545, shootdown 135157)
# L1 OK: 100000 lượt (smp [2], seeds 100000)
EXIT=0
```

```text scope=kt76-l1-smp4.log
# L1 OK: smp 4, 100000 seed × 64 op (2000 seed đầu chạy 2 lần; ipc_min 1, ipc_median 11; ipc_sent 1137251, ipc_delivered 211851, ipc_dropped 707418; vm map 257644, walk 71499, unmap 135394, fault 228758, shootdown 135394)
# L1 OK: 100000 lượt (smp [4], seeds 100000)
EXIT=0
```

```text scope=kt76-l1-smp8.log
# L1 OK: smp 8, 100000 seed × 64 op (2000 seed đầu chạy 2 lần; ipc_min 1, ipc_median 11; ipc_sent 1125024, ipc_delivered 209918, ipc_dropped 693268; vm map 257069, walk 71693, unmap 135075, fault 226058, shootdown 135075)
# L1 OK: 100000 lượt (smp [8], seeds 100000)
EXIT=0
```

### 5.7 Ngân sách ảnh (không nới) + pin `size_of::<Machine>()`

```text scope=kt76-budget.log
# KT-76 — ngân sách ảnh (trần .data+.bss = 262.144 B [TK], KHÔNG nới)
# đo bằng v2/test/boot/budget.sh (llvm-size -A), ảnh release dựng bởi matrix/soak trong task này
# image=riscv64 path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 file=188552 data+bss=200696 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=49874 .rodata=14432 .data=7824 .bss=192872 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7824 .bss=192872 
BUDGET_EXIT_riscv64=0
# image=ktest path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest file=712968 data+bss=242840 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=225376 .rodata=69616 .data=7992 .bss=234848 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7992 .bss=234848 
BUDGET_EXIT_ktest=0
# image=ktest-guard path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-guard file=54072 data+bss=2680 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=11356 .rodata=2896 .bss=2680 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .bss=2680 
BUDGET_EXIT_ktest-guard=0
# image=ktest-user path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-user file=603184 data+bss=203352 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=156374 .rodata=147072 .data=7768 .bss=195584 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7768 .bss=195584 
BUDGET_EXIT_ktest-user=0
# image=ktest-proc path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc file=437672 data+bss=207256 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=113396 .rodata=75208 .data=7768 .bss=199488 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7768 .bss=199488 
BUDGET_EXIT_ktest-proc=0
# image=ktest-vblk path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-vblk file=132104 data+bss=10976 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=41512 .rodata=8816 .bss=10976 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .bss=10976 
BUDGET_EXIT_ktest-vblk=0
# image=ktest-con path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-con file=357856 data+bss=214160 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=106506 .rodata=31544 .data=7768 .bss=206392 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7768 .bss=206392 
BUDGET_EXIT_ktest-con=0
# image=ktest-exec path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-exec file=357480 data+bss=202992 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=98204 .rodata=54600 .data=7768 .bss=195224 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7768 .bss=195224 
BUDGET_EXIT_ktest-exec=0
# image=ktest-persist path=v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-persist file=221248 data+bss=27352 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=39866 .rodata=89976 .bss=27352 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .bss=27352 
BUDGET_EXIT_ktest-persist=0
```

```text scope=kt76-pin.log
# tls pin: size_of_machine=189384 pin_cu=188872 delta=512 size_of_tcbdata=784 max_tcbs=64 8xmax=512 khop=1 duoi_pin=1
```

### 5.8 Ảnh sản phẩm: U-mode/shell (S1) + harness tự kiểm

```text scope=kt76-product-umode.log
# KT-76 — S1: ảnh SẢN PHẨM có U-mode/shell chưa? (đối chiếu ảnh test ktest-proc)
# date_utc=2026-11-16T22:25:02Z
# product strings: procsrv=0 shell=0 init_elf=0
# ktest-proc strings: procsrv=17 shell=34 init_elf=2
# product: KHÔNG có chuỗi của chương trình U-mode ⇒ không có ai nhận để bật shell (D2-08/D2-113)
# nguồn code: main.rs:361-362 halt sau '# k3 halt'; lib.rs:104-105 U-mode sau feature "user"; lib.rs:99-100/123-124 console sau feature
# kết luận: S1 = KHÔNG ĐẠT — shell chỉ có trong ảnh test ktest-proc, KHÔNG phải ảnh sản phẩm
```

```text scope=kt76-product-boot.log
# opensbi v1.8.1
ok 1 - boot 1 (smp=1)
# 1/1 boot sạch (smp=1)
# run.sh: PASS
```

```text scope=kt76-product-serial.log
OpenSBI v1.8.1
TAP version 13
1..1
ok 1 - k3 boot
# k3 halt
```

```text scope=kt76-selftest.log
# selftest: 18/18 ca đúng như kỳ vọng
# selftest.sh: PASS
SELFTEST_EXIT=0
```

## 6. Nợ của K6 — liệt kê đủ, có `file:line` + đề xuất

| # | Nợ | `file:line` (nguồn) | Ảnh hưởng tới tuyên bố | Đề xuất |
|---|---|---|---|---|
| P1 | **S1**: ảnh sản phẩm không U-mode/shell | `main.rs:361-362`, `lib.rs:104-105`, `lib.rs:99-100` | **"tương đương v1" mức người dùng KHÔNG ĐẠT** | **KT-77** hoặc văn bản hạ phạm vi (Agent 0) |
| P2 | Chưa có `fsync` từ U-mode | `k6-persist.md:284` | bền vững chỉ đo qua API kernel | KT-77 (cùng S1) |
| P3 | `NoSpace`/torn-write chưa có ca | `k6-persist.md:281` | chưa chứng minh biên đầy đĩa/ghi dở | KT-78 |
| P4 | SMP/ngắt trên ảnh persist: chưa có | `k6-persist.md:283` | persist mới một hart, poll | KT-78 |
| P5 | Ca âm bộ đếm `overflow/size_bad/absent_bad/replayed/errors/recorded/fsync/durable` | `k6-persist.md:285-286`, `k6-persist.md:77` | các bộ đếm là **quan sát**, chưa phải bằng chứng | KT-78 |
| P6 | `id_mismatch`/`timeouts` chưa có ca âm | `k6-virtio-blk.md:228` | đã bị bỏ khỏi điều kiện `ok` (đúng) nhưng chưa chứng minh | KT-78 |
| P7 | `MR4..MR63` qua IPC buffer chưa có | `k6-idl.md:175` | IDL giới hạn bốn scalar | KT-79 |
| P8 | **Scheduler** đa hart **chưa dùng** IPI (IPI thật **đã có**) | `v2/kernel/vifast-plat-riscv64/src/ipi.rs:1-10` | IPI/SBI + hộp thư per-hart có, nhưng core dispatch vẫn một hart | KT-79 |
| P9 | Console/virtio **poll**, chưa ngắt | `k6-virtio-blk.md:223` | chưa có đường IRQ cho blk | KT-79 |
| P10 | Chưa có ABI `Tcb.Exec(path)` | `k6-exec.md:255` | exec dùng bảng ảnh nhúng, không VFS | KT-77 |
| P11 | Ca **lỗi-muộn** của `exec` chưa dựng được | `v2/kernel/vifast-plat-common/src/obj/mod.rs:720` | "nguyên tử" đã thu hẹp, chưa phủ hết | KT-79 |
| P12 | Matrix **không** phủ ảnh persist | `k6-persist.md:288` | "matrix xanh" **không** phải bằng chứng persist | KT-78 |
| P13 | Driver blk/console **không** vào ảnh sản phẩm | `k6-virtio-blk.md:236`, `k6-console.md:164` | ảnh sản phẩm không chạm thiết bị | KT-77 |
| P14 | U-mode/capalloc chạy một hart | `k6-capalloc.md:145` | đa hart U-mode chưa có | KT-79 |
| P15 | `exec` chưa có ca SMP thật | `k6-exec.md:258` | matrix không kiểm exec đồng thời | KT-79 |
| P16 | Biên `MAX_INIT_PAGES` chưa có ca | `k6-exec.md:260` | ảnh lớn hơn ⇒ lỗi nguyên tử nhưng chưa kiểm | KT-78 |
| P17 | Journal chỉ **redo**, write lớn = nhiều transaction | `k6-vfs-journal.md:116` | không nguyên tử toàn file (giữ ngữ nghĩa v1) | ghi nhận |
| P18 | `fsck_repair` không sửa block **hai chủ** / thư mục trùng tên | `k6-vfs-journal.md:118` | phạm vi bốn loại hỏng của KT-71 | ghi nhận |
| P19 | Kiểm chồng lấn BAR chưa đọc DT `ranges` | `k6-virtio-blk.md:244` | chưa phải PCI enumerator tổng quát | KT-79 |
| P20 | **Tái lập byte của build CHƯA chứng minh** (artifact soak tái dùng; clean-build khác sha/digest `.text`) | `kt76-build-sha.log:12`, `kt76-clean-build.log` | “sha từ build sạch” chỉ đúng nghĩa **không nhiễm feature**, **không** phải tái lập byte | D2-108A siết: tách target dir + `--remap-path-prefix`, rồi soak **đúng** artifact clean (KT-79) |

## 7. Cái K6 **KHÔNG** chứng minh (không được trích ngược)

- **Không** chứng minh "tương đương v1" ở mức người dùng (S1) — không có shell trên ảnh sản phẩm.
- **Không** chứng minh `flush` trên **đĩa cứng thật**; QEMU ghi thẳng file host, ca âm `flush` là
  **mô hình cache** trong ảnh test (`k6-persist.md:279-283`).
- **Không** chứng minh torn-write/`NoSpace` (P3), SMP persist (P4), IPI core (P8), `fsync` U-mode (P2).
- **Không** chứng minh giao **đúng một lần** cho pager (N3) — pager chỉ có đối chiếu đếm.
- **Không** chứng minh **phần cứng** `x86_64`/PCID (N5; phần x86 của N1) — ngoài phạm vi tới K6
  (D2-10); nhưng **không** được dùng D2-10 để miễn **ca kiểm sim** (N1/N7 vẫn tính — xem §4).
- Matrix xanh **không** phủ ảnh persist (P12); gate L1 **không** phải số hiệu năng (D2-100).
- **Không** chứng minh **tính tái lập byte** của build: artifact soak (`fd202276…`) khác clean-build
  (`29d7e875…`, `build=317ffc48e0e71354`) — nợ **P20**.

## 8. Phạm vi tệp, commit, freeze

- Tệp của task: `v2/agent/reports/K6-DOD.md` (tệp này), `v2/docs/k3b-core.md` §24,
  `v2/agent/STATE.md`, `v2/agent/BLOCKED.md`, `v2/agent/JOURNAL/kt76-*` (log + script kiểm).
- **Không** chạm: `spec/`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`, `viscript/`, `review/`,
  `v2/kernel/**`, `v2/lib/**` (KT-76 **không** sửa code).
- Commit do **lead** thực hiện theo pathspec; hash + `??`/staged thực tế ghi ở
  `JOURNAL/kt76-journal.md` §Freeze (D2-105a).

## 9. Reviewer — checklist (D2-102/D2-109/D2-110/D2-112)

- (a) Chạy `bash v2/agent/JOURNAL/kt76-verify-numbers.sh` — phải `fail` bằng không; đọc dòng tổng kết.
- (b) Chạy `bash v2/agent/JOURNAL/kt76-extract-criteria.sh --check` — tệp tiêu chí phải khớp
  **từng byte** bản sinh lại từ nguồn đọc-only.
- (c) Chạy `bash v2/agent/JOURNAL/kt76-verify-selfbreak.sh` — **năm** ca phá trên **bản copy**
  phải đỏ đúng lý do (gồm ca **số đo giấu trong heading**) + md5 cây thật **không đổi** (D2-113F).
- (d) Grep mọi tiền tố evidence trong log K6 tìm literal (D2-102d): `# fs:`, `# pager:`, `# exec`,
  `# persist:`, `# reboot:`, `# KTEST:`, `# soak:` — mọi giá trị phải là biến đo.
- (e) Kiểm **S1** bằng tay: `strings` ảnh sản phẩm **không** có `procsrv`/`shell`; ảnh `ktest-proc`
  **có**; boot sản phẩm dừng ở `# k3 halt` (`kt76-product-boot.log`).
- (f) Không có ngưỡng nào bị nới: trần ảnh/`size_of`/N soak/seed L1/smp — so §5.7 + §5.3 + §5.6.

## Bảng đối chiếu số ↔ khối `scope=` (KT-133, Q6 của Agent 0)

Mỗi trường dưới đây **có số đo nằm trong một khối `scope=`/`scope-red=`** của chính tệp này —
cột cuối chỉ ra khối đó. Nhờ vậy mọi con số đều trích được nguyên văn từ log, đúng luật số một,
mà không phải viết số vào prose.

| Trường đo | Loại khối | Tệp log |
|---|---|---|
| `fail` | `scope=` | `kt76-soak.log` |
| `index` | `scope=` | `kt71-green-qemu-ktest.log` |
| `MAX_IO` | `scope=` | `kt76-criteria.txt` |
| `boots` | `scope=` | `kt76-persist-run.log` |
| `clients` | `scope=` | `kt70-green-qemu-ktest-user.log` |
| `build` | `scope=` | `kt76-build-sha.log` |
| `bytes` | `scope=` | `kt76-build-sha.log` |
| `qemu_ppid1` | `scope=` | `kt76-orphan.log` |
