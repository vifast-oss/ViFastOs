# KT-262 — K4' B3.6: **`TLBI` + shootdown đa hart + smp 4**

Trạng thái: **DONE** (nhánh `task/kt262-tlbishoot`, base `main` @ `540d21db`), 2026-11-21.
Sub-agent `tlbi`. Quyết định: **D2-203** (nhiệm vụ), **D2-195** (phạm vi bước 3), **D2-199**
(nửa cao trong crate arch), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K4' đạt: tiêu chí K4' là "**như K3**" (TAP đầy đủ smp 1|2|4 + soak 1.000).
KT-262 làm **smp 4** + shootdown `TLBI`; **không** timer per-hart, **không** preemption,
**không** U-mode, **không** soak.

## 1. Phạm vi đã làm

| Việc | Kết quả |
|---|---|
| `shootdown` (crate arch): chuỗi bước, tập hart đích, bộ đếm ack, `invalidate`, `handle_ipi` | **DONE** — 19 ca host |
| `VSpace::unmap_shootdown` — `unmap` + `TLBI` + SGI + chờ ack | **DONE** |
| `dsb ishst` + `wfe` (asm) + `mmio_write_u8` | **DONE** |
| Ảnh: smp **4** hart (`SMP_HARTS = 4`), lệnh per-hart, `wfe` (không spin) | **DONE** |
| Ảnh: fault `FaultKind::Vm` **theo hart** (`faults.rs`) | **DONE** |
| TAP mới 13/14/15 | **DONE** — tổng **15** |
| Runner: `-smp 4`, `EXPECT_TESTS=15`, `check_tlbi`, **9** ca âm | **DONE** — 11/11 |

## 2. Thiết kế

### 2.1 Ba SGI **khác nhau** (điều kiện sống còn)

| SGI | Tên | Gửi → nhận | Việc |
|---|---|---|---|
| `0` | `SGI_IPI` | hart 0 → hart 1 | IPI của KT-259 (in + đếm `# smp: ipi recv=`) |
| `1` | `SGI_SHOOTDOWN` | hart 0 → **mọi** hart phụ | `tlbi vae1is` + `dsb ish` + `isb` + **ack** |
| `2` | `SGI_COMMAND` | hart 0 → **một** hart phụ | đánh thức hart phụ thực thi lệnh trong `SMP_CMD` |

Dùng chung một số thì `note_ipi` của KT-259 đếm cả shootdown/lệnh ⇒ dòng `# smp: ipi recv=1`
xuất hiện nhiều lần ⇒ runner ĐỎ (đúng ca âm (g)). Test host `sgi_numbers_are_distinct_and_in_sgi_range`
chốt ba số.

### 2.2 Chuỗi bước là **dữ liệu** (`shootdown::LOCAL_SEQUENCE`/`REMOTE_SEQUENCE`)

```text
hart 0 (sửa bảng trang):  dsb ishst → tlbi vae1is → dsb ish → isb
hart phụ (handler SGI):    tlbi vae1is → dsb ish → isb → ack
```

Hart phụ **không** có `dsb ishst` (nó không ghi bảng trang). Chuỗi là **mảng hằng** nên test host
khẳng định **thứ tự tuyệt đối** — đột biến "bỏ `dsb ish`" hay "bỏ `tlbi vae1is`" làm test **không
biên dịch được** (index out of bounds), tức ĐỎ ở tầng host.

### 2.3 Bộ đếm ack + trần

`AckCounter(AtomicU32)` **toàn cục**; mỗi hart phụ `fetch_add(1)` **sau** khi chuỗi xong; hart 0
chờ `count == targets.count()` với trần `ACK_WAIT_LIMIT = 50e6` vòng. Hết trần ⇒
`Shootdown::timed_out = true` ⇒ TAP 14 đỏ, **không** treo.

`collect_acks(targets, acks, send)` tách phần **gửi + chờ** khỏi asm/MMIO ⇒ test host chạy được
giao thức ack với bộ gửi giả (3 ca: đủ ack, lỗi gửi, không ack ⇒ timeout).

### 2.4 Vì sao vẫn gửi SGI khi `vae1is` đã broadcast

`vae1is` broadcast tới mọi PE **inner-shareable** nhưng **không** cho biết khi nào chúng xoá xong,
và không tới PE ngoài domain. SGI + ack biến "tôi đã broadcast" thành "**mọi hart đã xoá xong**".
Trên QEMU TCG, `vae1is` **đã** flush TLB mọi CPU — xem §5, ghi trung thực.

### 2.5 Hart phụ chờ bằng `wfe` (không `wfi`, không spin)

`wfi` **mất đánh thức** nếu SGI tới giữa lúc đọc cờ và lúc vào `wfi` (ngắt đã xử lý xong ⇒ không
còn ngắt chờ). `sev` đặt **event register** kể cả khi hart phụ chưa vào `wfe` ⇒ không mất. Spin
thì đúng nhưng đốt CPU TCG (4 hart quay vòng) và làm phép đo thời gian vô nghĩa.

## 3. Hai **lỗi thật** phát hiện trong lúc làm (không phải "đột biến")

### 3.1 Stub vector **xoá `x9`** của mã bị ngắt (lỗi từ KT-248)

Stub cũ: `mov x9, #idx` **trước** `stp x8, x9, [sp, #64]` ⇒ giá trị `x9` gốc bị **mất**; sau
`eret`, `x9` = **chỉ số ô** (4 với `brk`/data abort ở EL1h). KT-248..KT-259 không lộ vì mã sau
điểm fault không giữ `x9` là con trỏ sống. KT-262 lộ ngay:

```text
# trap: slot=4 off=0x200 ec=0x25 il=1 far=0x4 elr=0xffffffc080004a90 spsr=0x600003c5
```

`ldar x9, [x9]` với `x9 = 4` (chỉ số ô) trong `faults::get` ⇒ data abort `FAR=0x4`.

**Vá**: stub cất `x9` vào khung 16 B **trước** khi dùng nó (`str x9, [sp, #-16]!`), và `vec_common`
khôi phục sau `add sp, sp, #176` (`ldr x9, [sp], #16`). Khung 16 B giữ `sp` căn 16.

### 3.2 `GICR_IPRIORITYR<n>` là thanh ghi **theo BYTE** — ghi 32 bit bị **alignment fault**

`gic::enable_sgi` cũ ghi `mmio_write_u32(base + 0x400 + sgi, …)`. KT-259 chỉ bật `SGI 0` nên địa
chỉ tình cờ căn 4. KT-262 bật thêm `SGI 1`/`SGI 2` ⇒ địa chỉ `0x080B_0401` lệch lề ⇒ trên Device
memory là **alignment fault**:

```text
# trap: slot=4 off=0x200 ec=0x25 il=1 far=0x80b0401 elr=0xffffffc0800017ec spsr=0x800003c5
```

**Vá**: thêm `asm::mmio_write_u8` và ghi **một byte** cho mỗi INTID (đúng IHI 0069 §9.3.7).

## 4. Đo được

### 4.1 TAP (ảnh, `kt262-boot.log`)

```text
ok 10 - smp: hart 1 online
ok 11 - smp: ipi 0->1
ok 12 - smp: all harts report
ok 13 - tlbi: hart 1 sees unmap
ok 14 - tlbi: ack from all harts
ok 15 - smp: 4 harts online
```

Dòng bằng chứng (nguyên văn):

```text
# smp: hart=1 online
# smp: hart=2 online
# smp: hart=3 online
# smp: hart=1 alive
# smp: hart=2 alive
# smp: hart=3 alive
# smp: hart=0 alive
# tlbi: remap va=0x60000000 pa=0x40093000 magic=0x5653504143450001
# tlbi: hart=1 read1=0x5653504143450001 magic=0x5653504143450001 match=1 sent=true done=1
# tlbi: shootdown va=0x60000000 targets=0xe expected=3 acks=3 waited=49859 timeout=0 send_failed=0x0 ok=1
# tlbi: hart=1 read2 fault=1 va=0x60000000 access=Read reason=NotMapped esr=0x0000000096000006 far=0x60000000 sent=true done=1 seen=0x3
# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 mpidr=0x80000000,0x80000001,0x80000002,0x80000003
```

Đọc được từ log: hart 1 **đã nạp** entry (`read1 == magic`), shootdown **đủ 3 ack** từ 3 hart phụ,
hart 1 **fault** `FaultKind::Vm` (`va=0x6000_0000`, `access=Read`, `reason=NotMapped`) sau
`unmap`; `seen=0x3` = hart 0 (TAP 9) **và** hart 1 (TAP 13) — hai hart khác nhau, đúng bản ghi
theo hart; bốn `MPIDR` đúng `0x8000_0000..0x8000_0003`.

### 4.2 Runner (`bash v2/test/boot/run-aarch64.sh`)

```text
ok 1 - elf: .text tại 0x40080000, .text.vectors TEXT (không BSS), 16 stub đúng offset
ok 2 - boot 1: TAP 15/15 + marker 1 lần
ok 3..11 - 9 ca âm (7 ca cũ + (h) thiếu dòng `# tlbi:` + (i) thiếu ack shootdown)
# ca âm: 9/9 ĐỎ đúng như phải
# 11/11 kiểm sạch (boot=1, tests=15)
```

### 4.3 Test host

`cargo test -p vifast-plat-aarch64`: **155** ca (trước KT-262: 136) — **+19** ở
`tests/shootdown.rs` (thứ tự bước, tên bước, ba SGI, `Targets` (6), `AckCounter` (4),
`collect_acks` (3), `Shootdown::ok` (2), trần ack).

## 5. Đột biến (log: `kt262-mutation.log`) — **ghi trung thực tầng nào**

| # | Đột biến | Tầng host | Tầng QEMU |
|---|---|---|---|
| (i) | hart phụ **không** chạy `tlbi vae1is` (chỉ ack) | **ĐỎ** (bỏ `Step::TlbiVae1is` khỏi `REMOTE_SEQUENCE` ⇒ test không biên dịch được) | **KHÔNG quan sát được** — TAP 13 vẫn XANH |
| (ii) | bỏ `dsb ish` | **ĐỎ** (độ dài chuỗi 4→3 / 3→2 ⇒ index out of bounds) | **KHÔNG quan sát được** — TAP 15/15 XANH |
| (iii) | **không chờ ack** | — (đường chờ là asm/MMIO) | **ĐỎ** — `not ok 14`, `acks=0 expected=3 ok=0` |

**Vì sao (i) không đỏ trên QEMU**: QEMU TCG cài `TLBI VAE1IS` bằng
`tlb_flush_page_by_mmuidx_all_cpus_synced` (`target/arm/helper.c`) — nó flush TLB của **mọi** CPU,
nên `tlbi vae1is` của hart 0 đã xoá entry trong TLB của hart 1 **trước khi** SGI tới. Đo được:
bỏ `tlbi` ở hart phụ, `read2` **vẫn** `fault=1`. **Không** được gọi (i) là "đã kiểm trên phần cứng".

**Vì sao (ii) không đỏ trên QEMU**: TCG thực thi tuần tự và không mô hình hoá thời gian của
barrier; `dsb ish` không có hiệu ứng quan sát được trong mô hình ấy.

### 5.1 Đối chứng **dương** — chứng minh đường SGI thật sự có tác dụng (QEMU **quan sát được**)

Vì (i) không đỏ, tôi đo **đối chứng dương**: bỏ `tlbi vae1is` **của hart 0** (chỉ giữ SGI) khi có
hart đích. Kết quả: TAP 13 **vẫn XANH** (`read2 fault=1`) ⇒ **một mình đường SGI** đã xoá TLB của
hart 1. Đây là bằng chứng QEMU-observable rằng hart phụ **thật sự** chạy `tlbi vae1is` trong
handler — mạnh hơn một đột biến đỏ, vì nó chứng minh đường remote **đủ**.

### 5.2 Đối chứng **âm** — `tlbi` cục bộ là thứ có răng

Bỏ **toàn bộ** chuỗi cục bộ (kể cả đường `Targets::NONE` của TAP 9) ⇒ `not ok 9` (TLB của chính
hart 0 còn entry ⇒ đọc lại **không** fault). Vậy chuỗi cục bộ **không** phải trang trí.

## 6. Còn thiếu (ghi rõ, không giấu)

1. **Soak 1.000** boot: chưa chạy (thuộc "như K3").
2. **Timer per-hart / preemption**: chưa — chưa có `CNTP`/`CNTV` per-hart, chưa có scheduler.
3. **U-mode** (B3.7): vẫn chặn bởi Q2 (`spec/abi.toml [calling.aarch64]` chưa mở).
4. **Mỗi hart một address space/ASID**: chưa — bốn hart dùng **cùng** bảng trang `VSpace`
   (hart 0 ở VA nửa cao, hart phụ ở VA thấp).
5. **Shootdown theo **dải** VA**: chỉ một VA mỗi lần (`vae1is`); `vmalle1is`/`aside1is` có hằng
   encoding nhưng chưa dùng.
6. **Không lock**: một shootdown tại một thời điểm (hart 0 chờ đủ ack trước khi trả về). Có
   preemption/ngắt lồng nhau thì cần lock — chưa có.
7. **`GICR_WAKER` không cưỡng chế được trên QEMU** (nợ KT-259 §5): vẫn giữ bước wake (đúng đặc
   tả) nhưng không có bằng chứng QEMU cho nó.

## 7. Rủi ro

- **Trần chờ ack** (`ACK_WAIT_LIMIT = 50e6` vòng): đo được ~1,0×10^4..6,8×10^4 vòng cho 3 hart ⇒
  trần rộng hơn **hai bậc độ lớn**, nhưng đây là ngưỡng **thời gian**, không phải ngưỡng đúng/sai.
- **In xen dòng**: chỉ tránh được nhờ quy tắc "in trước, đặt cờ sau" + hart 0 gửi lệnh **tuần
  tự**. Mọi dòng in mới của hart phụ **phải** theo quy tắc ấy; PL011 không có lock.
- **`wfe` + `sev`**: nếu một hart phụ bỏ lỡ cả `sev` lẫn SGI (không xảy ra trong mô hình hiện
  tại) thì nó ngủ tới lần `sev` sau; runner có trần 20 s nên treo là ĐỎ, không im lặng.
- **QEMU flush TLB mọi CPU cho `*IS`**: trên phần cứng thật, `vae1is` chỉ tới **inner-shareable
  domain**; SGI + ack là phần bảo đảm "mọi hart đã xong" — nhưng **chưa** có bằng chứng phần cứng.

## 8. Tệp

| Tệp | Việc |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/shootdown.rs` | **mới** — chuỗi bước, `Targets`, `AckCounter`, `Shootdown`, `invalidate`, `handle_ipi` |
| `v2/kernel/vifast-plat-aarch64/tests/shootdown.rs` | **mới** — 19 ca host (đỏ trước: `kt262-red.log`) |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | +`dsb_ishst`, +`wfe`, +`mmio_write_u8` (+ stub host) |
| `v2/kernel/vifast-plat-aarch64/src/barrier.rs` | +`dsb_ishst()` |
| `v2/kernel/vifast-plat-aarch64/src/gic.rs` | +`SGI_SHOOTDOWN`/`SGI_COMMAND`; `enable_sgi` ghi **byte** ưu tiên |
| `v2/kernel/vifast-plat-aarch64/src/vspace.rs` | +`VSpace::unmap_shootdown` |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | +`pub mod shootdown` + re-export |
| `v2/kernel/vifast-kbin-aarch64/src/faults.rs` | **mới** — fault `Vm` **theo hart** |
| `v2/kernel/vifast-kbin-aarch64/src/smpboot.rs` | smp **4** hart, lệnh per-hart, `wfe`, TAP 10..15 |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | stub vector giữ `x9`; IRQ phân phối SGI 1/0/2; TAP 15; gọi shootdown test |
| `v2/test/boot/run-aarch64.sh` | `-smp 4`, `EXPECT_TESTS=15`, `check_tlbi`, 9 ca âm |
| `v2/docs/k4-aarch64-boot.md` | §1/§2/§6/§7 cập nhật B3.6 |
