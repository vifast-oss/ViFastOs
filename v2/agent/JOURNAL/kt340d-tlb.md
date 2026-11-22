# KT-340d — `x86_64`: TLB shootdown đa hart (`IpiMailbox`) + đóng lỗi v1 **N5**

**Chu kỳ 2 — Đợt 2.** Người làm: sub-agent `x64tlb`. Ngày: 2026-11-23 (giờ máy Asia/Tokyo).
Quyết định: **D2-238** (chèn `KT-340a`/`KT-340b`/`KT-340d`, đóng v1 `N5`), **D2-244** (kiến trúc hai
tầng `IpiMailbox` + `Mailbox<N>`; tầng 1 = **3/3 port**), **D2-247** (DAG trên `7b60723`), **D2-68**
(`TlbFlush`, không self-target, chờ ACK phải **drain**), **D2-77** (hart chưa online ⇒ `Invalid`;
IPI lỗi ⇒ **gỡ bit**), **D2-80** (bằng chứng neo ảnh), **D2-05** (đỏ trước), **D2-17**
(`unsafe` chỉ trong `vifast-plat-*`), **D2-245** (sub-agent chỉ T0/T1; journal **một** tệp).
Kế hoạch: `KE-HOACH-CHU-KY-2-2026-11-22.md` dòng 120 mục **KT-340d**.

| | |
|---|---|
| Worktree | `/private/tmp/kt340d-s1` (nhánh `task/kt340d-tlb`) |
| Base | `main` @ `a89b2b05` (`KT-340b` vừa merge) |
| Máy | macOS Apple Silicon, 12 nhân, rustc/cargo `1.100.0`, QEMU 11.1.3 |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong `/private/tmp/kt340d-s1` |
| Nhãn số liệu | **`[ĐO KT-340d]`** — đo từ log QEMU/`llvm-size` trong worktree này |

Mục tiêu (task): (1) khảo sát APIC/IPI hiện có; (2) `IpiMailbox` cho `x86_64` — tệp **`ipi.rs`**
(không dùng từ cấm D2-131), bit-trước-IPI-sau, coalesce, IPI lỗi ⇒ gỡ bit, không self-target, handler drain
mọi kind, dòng bằng chứng `# ipi: mailbox …`; (3) **TLB shootdown** khi một hart đổi bảng trang của
`VSpace` đang dùng ở hart khác — IPI `TlbFlush(va)` + **chờ ACK có trần**, hart nhận `invlpg` + ACK,
**≥ 2 ca kiểm** (dương + âm); (4) đóng **N5** (`PCID`/`-cpu max`); (5) **2 đột biến ĐỎ**; (6) journal
+ commit, cây sạch. Giữ nguyên: `.data+.bss` sản phẩm **< 65.536 B**, ktest ≤ 262.144, R1 ≤ 1.500/tệp,
TAP x86_64 **36** + runner **36/36** + **32 ca âm** (nay **37** + 36/36 + 32/32 — xem §7), không
`#[ignore]`/retry, không `gate --fast`.

---

## 1. Khảo sát (bước 1)

### 1.1 APIC/IPI `x86_64` **đã có** gì trước task này

| Thành phần | Ở đâu | Trạng thái trước `KT-340d` |
|---|---|---|
| Hằng LAPIC/IOAPIC (offset, bit, bảng chia, PIT) | `vifast-plat-x86_64/src/apic.rs` (KT-266) | đủ cho MMIO + timer, **không** có ICR |
| `ICR` (offset `0x300`/`0x310`, `icr_high`, `icr_fixed_low`, `delivery_pending`, trần chờ) | `vifast-plat-x86_64/src/smp.rs` (KT-278) | **có** — dùng cho `INIT`/`SIPI` + **một** IPI fixed-delivery vector `0x30` |
| Gửi IPI | `vifast-kbin-x86_64/src/smp.rs::send_ipi` (KT-278) | gửi được, **chỉ** dùng một lần cho TAP 20 (`# smp: ipi send dest=1 …`) |
| Nhận IPI | `vifast-kbin-x86_64/src/trap.rs` nhánh `vec == VEC_IPI` | **chỉ** `smp::note_ipi()` (đếm + in `# smp: ipi recv=<n> intid=0x30`) + `EOI` — **không** có hộp thư, **không** có việc gì để làm |
| EOI | `lapic_eoi()` (`main.rs`) | timer + IPI + spurious đều `EOI` đúng chỗ |
| Vector | `exc::VEC_IPI = 0x30` (stub riêng trong `exc.rs`, `KT-278`) | một vector duy nhất, **không** mang kind |
| Hộp thư kind | `vifast-plat-common/src/ipi.rs::IpiMailbox` (`KT-50`) | **có** (bitmask `AtomicU64`/hart, coalesce, không bao giờ đầy) nhưng **chưa** port nào của `x86_64` dùng |
| `wait_with_drain` | `vifast-plat-common/src/ipi.rs` | **có** — dùng ở `riscv64`/`aarch64`, `x86_64` chưa có ai gọi |
| `VSpace` động | `vifast-plat-x86_64/src/vspace.rs` (`KT-340b`) | `create`/`map`/`unmap`/`walk`/`activate`/`restore` + `INVLPG` **cục bộ**; **không** có khái niệm "hart khác đang dùng cây này" |

**Kết luận khảo sát:** phần *chuông* (ICR + vector `0x30` + stub + `EOI`) đã đúng và đã có bằng
chứng ở TAP 20; thiếu **ba** thứ: (a) hộp thư kind dùng chung + đường phục vụ trong handler;
(b) khái niệm `VSpace` **đang chạy trên hart nào** (`active_on`); (c) bất kỳ phép chờ ACK nào.
Không có `PCID`/`INVPCID`, không có flush theo range, không có `free_frame`.

### 1.2 `IpiMailbox` của `vifast-plat-common` có dùng được cho `x86_64` không?

**Dùng được, và đúng là thứ cần** — nhưng phải bổ sung **ba** chỗ *quanh* nó (không sửa nó):

1. **Không bao giờ đầy** (bitmask coalesce) ⇒ đúng D2-68/D2-77: gửi trùng một kind **đang chờ** là
   gộp, không `Busy`, không mất đánh thức. Đây chính là lý do D2-244 chọn nó làm **tầng 1** thay
   `vifast_sync::Mailbox<N>` (FIFO 7 message, đầy ⇒ `Busy` ⇒ mất wake).
2. **Kind là chuông, không phải payload** ⇒ tầng 2 (`Mailbox<N>`) mới mang `{hart, tcb}`. Ở task này
   payload của `TlbFlush` là **một VA** nên tôi giữ **một ô yêu cầu mỗi hart** (`FlushSlot`, §2.2) —
   đúng tinh thần tầng 2, chưa cần FIFO.
3. **Hart chưa online / IPI lỗi** ⇒ `IpiMailbox` **không** biết gì về hart; phía arch phải tự
   fail-closed: `post` tra bảng `LAPIC_ID` do ảnh công bố, hart chưa công bố ⇒ `Invalid` **và
   không post bit** (D2-77); `ICR` không gửi được ⇒ `unpost` (gỡ bit) + `invalid += 1`.

Hai điều **phải đổi so với `riscv64`/`aarch64`**: (i) `x86_64` chỉ có **một** vector `0x30` cho
**mọi** kind (SGI aarch64 có 3 số khác nhau), nên kind **chỉ** nằm ở bitmask — thuận cho việc thêm
kind; (ii) "hart đang chạy" phải đọc `LAPIC_ID` từ MMIO + tra bảng (arch crate không biết topology) —
thêm `ipi::set_lapic_id` để ảnh công bố (xem §2.1).

### 1.3 `VSpace` đa hart **thiếu** gì

- **`active_on`** — không biết cây bảng trang nào đang được hart nào dùng ⇒ không biết phải gửi
  `TlbFlush` cho ai. Thêm `AtomicU32` + `mark_active`/`mark_inactive`; `activate`/`restore` tự cập
  nhật hart **gọi**.
- **Đường shootdown** — `flush()` chỉ `INVLPG` **cục bộ**: hart khác vẫn giữ entry TLB **cũ** ⇒ đọc
  dữ liệu cũ. Đây **đúng** là lỗi v1 **N5** (xem §1.4).
- **Nguồn `seq`** — cần số thứ tự cho ACK. §4.4 ghi lại một **race thật** phát hiện qua boot GPU.
- **Không** cần hart mới: ca kiểm dùng hart 1 (đã online ở TAP 18) + hart 0.

### 1.4 Lỗi v1 **N5** là gì (nguyên văn tiêu chí đóng)

`docs/v1/KNOWN-ISSUES.md` #5: *"x86 AP→BSP + PCID — TLB cũ. Lỗi **đúng đắn**: sau unmap, TLB của
BSP có thể còn cũ ⇒ tiến trình đọc được trang đã thu hồi. Không lộ trên TCG mặc định nên mọi cổng
vẫn xanh. §15.6: có test (host hoặc QEMU `-cpu max`/KVM) chứng minh TLB cũ gây **đọc sai**; bản vá
làm nó xanh; **hoặc** tắt PCID có kiểm soát + ghi lý do. Đột biến **"bỏ xử lý hộp thư khi chờ
ack"**"*.

Ba nhánh, task này đóng **cả ba**: (a) test **chứng minh đọc sai** — §4 ca (a) + đột biến (i)/(ii);
(b) **tắt PCID có kiểm soát + lý do đo được** — §5; (c) **đột biến "bỏ xử lý hộp thư khi chờ ACK"** —
`mut-no-wait-drain`, §4.3.

---

## 2. Thiết kế đã cài

### 2.1 `vifast-plat-x86_64/src/ipi.rs` (mới, 857 dòng) — hộp thư + shootdown

- **Bảng hart ↔ `LAPIC_ID`**: `set_lapic_id(hart, id)` (ảnh công bố: BSP lúc `lapic_init`, AP sau khi
  tự kiểm `LAPIC_ID`), `lapic_id_of`, `online_mask`, `current_hart()` = đọc `LAPIC_ID` MMIO của
  **chính hart này** rồi tra bảng. **Không** có biến `static` "hart hiện tại" — đúng bài học
  `KT-298`/`KT-310` (ô dùng chung bốn hart là sai ngay khi hai hart chạy).
- **`post(hart, kind)`**: `posted += 1`; self-target ⇒ `Local` (**không** IPI, D2-68); hart chưa
  công bố `LAPIC_ID` ⇒ `Invalid` (**không** đặt bit, D2-77); **bit vào hộp thư trước**
  (`mailbox.post`), `false` (đã chờ) ⇒ `Coalesced`; rồi `send_ipi` (ICR fixed-delivery, chờ
  `Delivery Status` về 0 hai lần); IPI lỗi ⇒ `mailbox.unpost(kind)` + `Invalid` (D2-77).
- **`drain_current()`/`drain_waiting()`**: hai đường drain **một** thân; đường handler đếm `empty`
  ("IPI tới mà không có kind" = mất đánh thức), đường **chờ ACK** không đếm (hộp thư rỗng ở phần lớn
  bước lặp là bình thường — nếu đếm thì `empty` thành nhiễu, đo được `empty=165` ở bản đầu).
- **`service_flush(hart)`**: đọc ô yêu cầu → (nếu `root != 0` và `CR3` khác) `write_cr3(root)` →
  (`MODE_REMAP`) ghi `remap_val` vào ô lá `remap_pa` → **`invlpg(va)`** → (`MODE_OBSERVE`) **đọc lại
  `va`** và ghi `obs` → (`MODE_CHAIN`) nối **một** yêu cầu sang `chain` hart → **rồi mới**
  `acked = seq`. Thứ tự "việc trước, ACK sau" là hợp đồng.
- **`issue`/`wait_ack`/`shootdown`**: `issue` **giữ chỗ** ô bằng CAS (`claim`), xoá `acked` cũ, ghi
  payload, rồi `post`; người gửi chờ ACK bằng `wait_with_drain` (crate dùng chung) — **mỗi bước lặp
  `drain` hộp thư của chính mình trước khi kiểm ACK** (D2-68, đúng lỗi N5); trần
  `ACK_SPIN_LIMIT = 60.000.000` **vòng** (không phải thời gian — bài học `KT-301`).
- **`shootdown_targets(active_on, me)`** = `active_on & !(1 << me)` — **không** self-target (D2-68).
- **`PcidStatus`/`pcid_status()`**: đo `CPUID.01H` (`ECX[17]` PCID, `EDX[13]` PGE) + đọc `CR4`; hàm
  thuần `full_flush_ok()` = `!cr4_pcid && !cr4_pge`.
- Bộ đếm `IpiStats`: `posted/coalesced/sent/drained/empty/acked/invalid/local/timeouts/busy`.

### 2.2 `FlushSlot` — payload tối thiểu của **một** yêu cầu đang bay tới một hart

`{ claim, seq, va, mode, root, remap_pa, remap_val, chain, chain_seq, chain_va, obs, acked }` —
`MAX_HARTS × 88 B = 704 B` toàn ảnh (đo được: `.data+.bss` sản phẩm **không đổi**, xem §7 vì
`VSpace` không có trong ảnh sản phẩm ⇒ linker bỏ cả cụm).

**Vì sao cần ô riêng** (không nhét VA vào bitmask): bitmask coalesce theo kind, nên hai yêu cầu khác
VA gửi sát nhau sẽ **gộp bit** và hart nhận chỉ `invlpg` **một** VA ⇒ mất một flush. Giao thức: người
gửi **giữ chỗ** (CAS) trước khi ghi, hart nhận đọc sau `drain`, chỗ chỉ trả khi ACK (hoặc hết trần) —
nên trên mỗi hart **không bao giờ** có hai yêu cầu cùng bay.

### 2.3 `VSpace` + `flush_va`

`VSpace` thêm `active_on: AtomicU32`, `flush_seq: AtomicU64`; `map`/`map_device` gọi
`flush_va(va, MODE_OBSERVE)` (hart nhận **đọc lại** VA — bằng chứng "thấy mapping mới"), `unmap` gọi
`flush_va(va, 0)` (VA vừa bỏ map, đọc lại sẽ fault ⇒ **không** đọc). `flush_va` = `INVLPG` cục bộ →
`targets = shootdown_targets(active_on, me)` → `ipi::shootdown(targets, va, mode, seq)`.
`activate`/`restore` cập nhật `active_on` cho hart **gọi**; hart **nhận** cây bảng trang qua `CR3`
(không có đối tượng `VSpace`) được `mark_active` (hàm thuần trạng thái).

### 2.4 `trap.rs` — handler vector `0x30` **drain trên mọi kind**

`if vec == VEC_IPI { smp::note_ipi(); ipi::drain_current(); lapic_eoi(); return resume }` — dòng
`drain_current()` là chỗ **duy nhất** hart nhận phục vụ hộp thư khi bị ngắt; `Wake`/`VmKick` chỉ rút
khỏi bitmask và đếm (tầng arch **không** tự đánh thức tác vụ — D2-244 tầng 2 / `E_ARCH_DOORBELL`).

### 2.5 `vifast-kbin-x86_64/src/tlbshoot.rs` (mới, 373 dòng) — ca kiểm `# tlb:` / TAP 37

Bốn pha trên **một** `VSpace` (id 3) với VA `0x10_0000_6000` (TLB_VA) và `0x10_0000_7000`
(CHAIN_VA), bốn frame dữ liệu `WORD_A/B/C/D` từ **cùng** arena 32 frame của `bootpt` (`KT-340b`):

| Pha | Ai làm gì | Bằng chứng |
|---|---|---|
| 0 `adopt` | BSP gửi `TlbFlush(TLB_VA)` mang `root` + `OBSERVE`; hart 1 nạp `CR3` của VSpace rồi `invlpg` + đọc lại | `adopt_ack` != 0, `obs = WORD_A` (hart 1 **đọc được** qua cây của BSP) |
| 1 **(a) BSP → hart 1** | `VSpace::unmap` + `VSpace::map` VA→frame B: **đường sản phẩm** tự shootdown hart 1 (đang `active_on`) và chờ ACK | `remap_obs = WORD_B` **và** `remap_ack == remap_seq` (ACK mang **đúng** `seq` của `map`) |
| 2 **(b) hart 1 → BSP** (v1 **N5**) | yêu cầu `MODE_REMAP\|MODE_CHAIN`: hart 1 **ghi ô lá** của `CHAIN_VA` → frame D rồi tự `TlbFlush(CHAIN_VA)` cho **hart 0**; BSP chờ ACK trong **`IF = 0`** ⇒ chỉ `drain` trong vòng chờ mới phục vụ được | `chain_ack == chain_seq`, `chain_obs = WORD_D`, `peer_wait=acked own_wait=acked`, BSP đọc `CHAIN_VA` ra `WORD_D` |
| 3 `restore` | hart 1 trả `CR3` về cây gốc **trước**, rồi BSP | `restored=1` (`CR3` == boot root) |

**Vì sao chạy ngay sau TAP 21:** shootdown cần hart phụ **nạp `CR3`** của `VSpace` dùng chung; điều
đó chỉ an toàn khi hart phụ ở **kernel context** (vòng `hlt` của `kmain_secondary`). Sau
`sched::run`, pha nhiều hart (`KT-310`) có thể để hart phụ chạy `TASK_U` **ring 3**; đổi `CR3` lúc ấy
làm mã user không còn map ⇒ `#PF` thật, đỏ oan và làm bẩn bất biến đếm `# exc:` của runner (đo được
ở lần chạy thử đầu: `# exc: vec=14 err=0x0 cr2=0x1000007000` — xem §4.4).

**Vì sao TAP 37 phát *sớm* (giữa TAP 21 và 22) mà không dịch TAP 22..36:** TAP 13 cho phép số hiệu
**không theo thứ tự** miễn đủ/không trùng/khớp plan `1..N`; giữ số hiệu cũ tránh sửa ~40 comment +
6 grep của runner + toàn bộ fixture soak đang gắn với TAP 22..36. Plan in `1..37`, log có
`ok 37` **trước** `ok 22`.

---

## 3. Đo — bằng chứng xanh (nguyên văn log)

Boot `ktest` (`bash v2/test/boot/run-x86_64.sh`, ảnh `x86_64-ktest`):

```text
ok 21 - smp: all harts report
# smp: ipi recv=2 intid=0x30
# smp: ipi recv=3 intid=0x30
# smp: ipi recv=4 intid=0x30
# smp: ipi recv=5 intid=0x30
# smp: ipi recv=6 intid=0x30
# tlb: shootdown va=0x1000006000 stale=0xa1a1111122223333 remap_obs=0xb2b2444455556666 expect=(0xa1a1111122223333,0xb2b2444455556666) adopt_ack=1 remap_ack=3 remap_seq=3 chain_va=0x1000007000 chain_obs=0xd4d4aaaabbbbcccc chain_ack=5 chain_seq=5 chain_read=0xd4d4aaaabbbbcccc expect_chain=(0xc3c3777788889999,0xd4d4aaaabbbbcccc) peer_wait=acked own_wait=acked remap_ok=1 chain_ok=1 restored=1 ok=1
# ipi: mailbox posted=6 coalesced=0 sent=6 drained=6 empty=2 acked=6 invalid=0 local=0 timeouts=0 busy=0 vector=0x30 target=1 online=0xf ok=1
# ipi: pcid supported=0 pge_supported=1 cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg
ok 37 - tlb: shootdown ap->bsp
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

Đọc dòng `# tlb:` (mỗi trường là **một** phép đo):

| Trường | Nghĩa | Giá trị xanh |
|---|---|---|
| `stale=` | BSP đọc `TLB_VA` **trước** khi đổi mapping (nạp TLB cũ) | `0xa1a1…` = `WORD_A` |
| `remap_obs=` | hart 1 **đọc lại** `TLB_VA` ngay sau `TlbFlush` do `VSpace::map` phát | `0xb2b2…` = `WORD_B` ⇒ **thấy mapping mới** |
| `remap_ack` / `remap_seq` | ACK của hart 1 / `seq` của yêu cầu `map` | bằng nhau (`3 = 3`) |
| `chain_obs` | BSP **đọc lại** `CHAIN_VA` sau khi hart 1 sửa ô lá → frame D | `0xd4d4…` = `WORD_D` |
| `chain_ack` / `chain_seq` | ACK của BSP cho yêu cầu **hart 1** gửi | bằng nhau (`5 = 5`) |
| `peer_wait` / `own_wait` | kết quả hai vòng chờ ACK ở pha (b) | `acked` / `acked` |
| `remap_ok chain_ok restored ok` | hợp đồng TAP 37 | `1 1 1 1` |

`# ipi: mailbox`: `posted=6 = sent=6 = drained=6 = acked=6`, `coalesced=0`, `invalid=0`,
`timeouts=0`, `vector=0x30`, `target=1`, `online=0xf` — **sáu** yêu cầu (adopt, unmap, map, chain của
hart 1, chain của hart 0, restore) và **sáu** ACK. `empty=2` là **bình thường**: hai IPI "thừa" còn
treo trong `IRR` của LAPIC được giao sau `EOI` khi bitmask đã rỗng (edge-triggered fixed-delivery
không gộp được sau khi `ISR` bật) — đường handler đếm chúng, đường chờ thì không (xem §2.1).

Cùng một bản chạy trên ảnh **có** feature `gpu` (plan `1..38`): `# tlb: … ok=1`, `ok 37 - tlb:
shootdown ap->bsp`, `ok 38 - gpu: 2d ppm`.

---

## 4. Hai ca kiểm + **ba** đột biến (nguyên văn)

### 4.1 Ca (a) — dương: hart B **thấy** mapping mới sau shootdown

Ở §3: `stale=0xa1a1…` (cũ) → `remap_obs=0xb2b2…` (mới) với `remap_ack == remap_seq`. Phép đo có
răng vì `remap_obs` do **hart 1 tự đọc** (không phải BSP đọc hộ) và chỉ có sau `invlpg` của handler.

### 4.2 Ca (b) — âm (đột biến i): **tắt shootdown** ⇒ hart B vẫn thấy **giá trị cũ** ⇒ TAP ĐỎ

`cargo build … --features ktest,mut-no-shootdown` (bỏ **đúng** phần gửi `TlbFlush` trong
`VSpace::flush_va`, vẫn `INVLPG` cục bộ):

```text
# tlb: shootdown va=0x1000006000 stale=0xa1a1111122223333 remap_obs=0xa1a1111122223333 expect=(0xa1a1111122223333,0xb2b2444455556666) adopt_ack=1 remap_ack=1 remap_seq=0 chain_va=0x1000007000 chain_obs=0x0000000000000000 chain_ack=0 chain_seq=0 chain_read=0x0000000000000000 expect_chain=(0xc3c3777788889999,0xd4d4aaaabbbbcccc) peer_wait=not_sent own_wait=not_sent remap_ok=0 chain_ok=0 restored=1 ok=0
# ipi: mailbox posted=2 coalesced=0 sent=2 drained=2 empty=1 acked=2 invalid=0 local=0 timeouts=0 busy=0 vector=0x30 target=1 online=0xf ok=0
# tlb: lý do ĐỎ — hart 1 không thấy mapping mới sau shootdown
not ok 37 - tlb: shootdown ap->bsp
# TAP 37 FAIL — dừng (không in marker)
```

`remap_obs = 0xa1a1…` = **đúng dữ liệu cũ** — nguyên văn yêu cầu của task ("hart B **vẫn thấy** giá
trị cũ"), `remap_seq=0` (không có yêu cầu nào được phát), `posted=2` (chỉ adopt + restore).

### 4.3 Đột biến (ii): bỏ `invlpg` trong handler IPI ⇒ ca (a) ĐỎ

`--features ktest,mut-no-ipi-invlpg` (handler vẫn ACK, **không** `invlpg`):

```text
# tlb: shootdown va=0x1000006000 stale=0xa1a1111122223333 remap_obs=0xa1a1111122223333 expect=(0xa1a1111122223333,0xb2b2444455556666) adopt_ack=1 remap_ack=2 remap_seq=2 chain_va=0x1000007000 chain_obs=0x0000000000000000 chain_ack=0 chain_seq=0 chain_read=0x0000000000000000 expect_chain=(0xc3c3777788889999,0xd4d4aaaabbbbcccc) peer_wait=not_sent own_wait=not_sent remap_ok=0 chain_ok=0 restored=1 ok=0
# ipi: mailbox posted=4 coalesced=0 sent=4 drained=4 empty=1 acked=4 invalid=0 local=0 timeouts=0 busy=0 vector=0x30 target=1 online=0xf ok=0
# tlb: lý do ĐỎ — hart 1 không thấy mapping mới sau shootdown
not ok 37 - tlb: shootdown ap->bsp
```

Bài học đo được: **`remap_ack=2 == remap_seq=2`** (ACK **đã** tới) mà `remap_obs` vẫn là dữ liệu
**cũ** ⇒ "đã ACK" **không** phải bằng chứng "đã xoá TLB". Vì vậy TAP 37 đòi **cả** ACK **và** giá
trị đọc lại.

### 4.4 Đột biến (iii) — đúng lỗi v1 **N5**: bỏ **drain khi chờ ACK**

`--features ktest,mut-no-wait-drain` (vòng chờ ACK **không** gọi `drain`):

```text
# tlb: shootdown va=0x1000006000 stale=0xa1a1111122223333 remap_obs=0xb2b2444455556666 expect=(0xa1a1111122223333,0xb2b2444455556666) adopt_ack=1 remap_ack=3 remap_seq=3 chain_va=0x1000007000 chain_obs=0xd4d4aaaabbbbcccc chain_ack=5 chain_seq=5 chain_read=0xd4d4aaaabbbbcccc expect_chain=(0xc3c3777788889999,0xd4d4aaaabbbbcccc) peer_wait=acked own_wait=timeout remap_ok=1 chain_ok=0 restored=1 ok=0
# ipi: mailbox posted=6 coalesced=0 sent=6 drained=6 empty=1 acked=6 invalid=0 local=0 timeouts=1 busy=0 vector=0x30 target=1 online=0xf ok=0
# tlb: lý do ĐỎ — BSP không phục vụ TlbFlush(CHAIN_VA) trong lúc chờ ACK (N5)
not ok 37 - tlb: shootdown ap->bsp
```

`own_wait=timeout` + `timeouts=1` là **nguyên văn** lỗi N5: hart chờ ACK trong `IF = 0` mà không xử
lý hộp thư của mình ⇒ yêu cầu của hart kia **không bao giờ** được phục vụ. (`chain_ack=5` trong dòng
là giá trị **sau** khi `sti` giao nốt IPI đang treo — `chain_ok=0` mới là kết luận của phép đo; đây là
lý do dòng bằng chứng in **cả** `own_wait` bên cạnh `chain_ack`.)

### 4.5 Ba lỗi **thật** bắt được trong lúc làm (ghi lại vì đều là bẫy cho task sau)

1. **`#PF` khi đọc VA của `VSpace` sau `restore()`** — `cr2=0x1000007000`, `err=0x0`, `cs=0x8`:
   `CHAIN_VA` chỉ map trong `VSpace`, đọc sau khi trả `CR3` về cây gốc là fault thật. Sửa: đọc
   **trước** `restore()` và giữ trong biến (`chain_read`).
2. **`empty` bị đếm trong vòng chờ** — `empty=165` ở bản đầu vì `wait_ack` gọi `drain` mỗi bước lặp.
   Sửa: tách `drain_current` (handler, **đếm** `empty`) và `drain_waiting` (vòng chờ, **không** đếm) —
   ngữ nghĩa `empty` của `riscv64`/`aarch64` ("SGI tới mà bitmask rỗng") được giữ nguyên.
3. **Trùng `seq` giữa hai bộ đếm** (bắt được trên **boot GPU**, không phải boot thường!): ca kiểm
   đếm `1,2,3…` và `VSpace` cũng đếm `1,2,…` ⇒ ACK **cũ** (`acked=1`) thoả phép chờ của yêu cầu
   **mới** (`seq=1`) ⇒ BSP chạy trước khi hart 1 kịp `invlpg` ⇒ `post` kế tiếp thấy bit còn treo
   ⇒ `coalesced=1 sent=5` ⇒ TAP 37 ĐỎ **không tất định** (xanh ở boot thường, đỏ ở ảnh `gpu` vì thời
   điểm khác). Sửa: **một** nguồn `seq` toàn ảnh (`ipi::next_seq()`) + xoá `acked` cũ khi giữ chỗ.
   Đây đúng là lớp lỗi mà một cổng chỉ chạy **một** cấu hình sẽ bỏ lọt.

---

## 5. Đóng **N5** — `PCID`/`-cpu max` (nhánh "tắt có kiểm soát + ghi lý do")

**Chốt: giữ `CR4.PCIDE = 0` và `CR4.PGE = 0`, dùng `INVLPG` theo VA + `CR3` toàn phần. Lý do (đo
được, in ở dòng `# ipi: pcid …`):**

1. Khi `PCID` **tắt**, entry TLB **không** gắn thẻ theo `CR3`: `invlpg(va)` xoá **đúng** entry ấy
   **bất kể** nó được nạp dưới cây bảng trang nào, và một lần ghi `CR3` xoá **toàn bộ** entry
   không-`G`. Đó là cơ chế **mạnh hơn** cho shootdown theo VA: không cần `INVPCID` theo `PCID`,
   không có nguy cơ "flush nhầm thẻ PCID"/"quên `INVPCID` khi tái dùng ASID".
2. Bật `PCID` đòi thêm: `CR3[11:0] = PCID` cho từng không gian, cấp/thu hồi PCID, `INVPCID` theo
   PCID cho **mọi** đường flush, và một mô hình "hart nào giữ PCID nào" — đó là **ASID/PCID** đầy
   đủ, đã được tách khỏi phạm vi task này (`KT-340b` journal §"Phạm vi" cũng ghi nợ này).
3. `PGE = 0` ⇒ **không** có entry `G` nào sống qua `CR3`; bật `PGE` mà quên xử lý bit `G` là cách
   chắc chắn nhất để tạo lại đúng lỗi N5.

Đo trên **hai** model CPU (hành vi **phải giống nhau**, chỉ `supported=` đổi):

| Model | `# ipi: pcid …` (nguyên văn) |
|---|---|
| `qemu64` (mặc định runner) | `supported=0 pge_supported=1 cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg` |
| `max` (`V2_X64_CPU=max`) | `supported=1 pge_supported=1 cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg` |

`V2_X64_CPU=max bash v2/test/boot/run-x86_64.sh` → **36/36 kiểm sạch, 32/32 ca âm, TAP 37 XANH**
(xem §7) ⇒ trên CPU **có** `PCID` ta vẫn chạy đúng với lựa chọn "tắt có kiểm soát": `INVLPG` + `CR3`
là toàn bộ cơ chế xoá TLB. **Không** đổi hành vi TAP cũ: `ok 1..36` giữ nguyên tên + số hiệu.

---

## 6. `IpiMailbox` nối mấy đường (kể tên)

Port `x86_64` dùng `vifast_plat_common::ipi` ở **bốn** đường (tầng 1 của Audit Q1-A):

1. `ipi::post(hart, kind)` — **bit trước** (`IpiMailbox::post`) rồi **IPI** (ICR fixed-delivery);
   coalesce; IPI lỗi ⇒ `IpiMailbox::unpost` (D2-77).
2. `ipi::drain_current()` — handler vector `0x30` (`trap.rs`): `IpiMailbox::drain()` rồi phục vụ
   `TlbFlush` (`invlpg`, `CR3`, `REMAP`, `OBSERVE`, `CHAIN`).
3. `ipi::wait_ack()` → `wait_with_drain` (crate dùng chung) — vòng chờ ACK **drain** hộp thư của
   chính mình (`drain_waiting`), đúng D2-68/N5.
4. `ipi::post_decision()` — bọc `classify_post` của crate dùng chung cho test host (InvalidHart /
   Offline / Local / Remote) — hart chưa online **không** nhận `TlbFlush`.

Cộng với `IrqMcsLock` (đã có từ `KT-336a`), `x86_64` là port **3/3** của tầng 1 (D2-244).

---

## 7. Cổng

| Lệnh | Kết quả (nguyên văn dòng tổng kết) |
|---|---|
| `cargo test -p vifast-plat-x86_64 --test ipi` | `test result: ok. 8 passed; 0 failed` |
| `./v2/bin/v2 check` | `exit=0` — `check-wake-paths: mode=v2 strict=1 errors=0 warnings=0` (không có `E_ARCH_DOORBELL`: tệp mới **không** chứa từ cấm của D2-131) |
| `./v2/bin/v2 gate --dev` (T1) | xem §8 (dòng tổng kết dán nguyên văn) |
| `bash v2/test/boot/run-x86_64.sh` | `# 36/36 kiểm sạch (boot=1, tests=37, exc=6)` + `# ca âm: 32/32 ĐỎ đúng như phải (+ 1 ca dương KT-313)` |
| `bash v2/test/boot/budget-x86_64.sh <ảnh sản phẩm> x86_64-product` | `# image=x86_64-product … file=93656 … data+bss=43072 budget=262144 label_budget=65536 …` (**43072 < 65536**, không đổi so với `KT-340b`) |
| `bash v2/test/boot/gpu-x86_64.sh` | `# gpu-x86_64.sh: 0 lỗi` (plan `1..38`, `ok 37 - tlb: shootdown ap->bsp`, `ok 38 - gpu: 2d ppm`) |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | `# soak-x86_64 --selftest: 53/53 ca đúng như phải (2 dương + 51 âm)` + `PASS` |
| `V2_X64_CPU=max bash v2/test/boot/run-x86_64.sh` | `# 36/36 kiểm sạch (boot=1, tests=37, exc=6)` (TAP 37 XANH trên CPU **có** PCID) |

**TAP: 36 → 37** (`TAP_TESTS = 37 + gpu`); runner `EXPECT_TESTS` 36 → **37**; soak `TAP_TESTS` 36 →
**37** (+ 3 ca âm mới cho bộ đọc: mất `# tlb:`, `remap_obs` quay về dữ liệu cũ, `own_wait=timeout`);
`gpu-x86_64.sh` `EXPECT_TESTS` 37 → **38** (TAP `gpu` là 38). **32 ca âm của runner không đổi** (vẫn
32/32) — 3 ca âm **mới** nằm trong `--selftest` của soak (51 âm + 2 dương = 53).

**Không nới ngưỡng nào:** `.data+.bss` sản phẩm **43.072 B < 65.536 B** (y nguyên), ktest
**125.800 B ≤ 262.144**, R1: `main.rs` **1.006** dòng, `smp.rs` **1.128**, `asm.rs` **1.409**, `vspace.rs` **310**,
`ipi.rs` (arch) **857**, `trap.rs` **325**, `net.rs` **561**, `tlbshoot.rs` **373** — đều < 1.500. `size_of::<Machine>()` **không** thuộc task này (`KT-340c`).

---

## 8. Việc còn lại / nợ đã biết (không giấu)

1. **`gate --dev` (T1) và log**: dán ở mục §8.1 (lead chạy T2 trên ứng viên tích hợp — D2-245).
2. **`free_frame`/`destroy`**: arena boot **không thu hồi** (giữ quyết định `KT-340b`); `VSpace` của
   ca kiểm để lại frame trong arena (32 frame, dùng 19). `KT-340c` mở rộng khi có physmap.
3. **Shootdown theo VA, không theo range**: `unmap` từng trang; range-unmap cần thêm một kiểu yêu cầu
   (tầng 2 `Mailbox<N>` sẽ làm ở Chu kỳ 3/`SMP-1`).
4. **Chỉ một yêu cầu đang bay mỗi hart** (`FlushSlot.claim`): hai **hart gửi** khác nhau cùng
   shootdown một hart đích sẽ thấy `busy=1` và yêu cầu thứ hai **không** phát (fail-closed, đếm được).
   Trong ảnh này chỉ BSP + hart 1 phát, và giao thức tuần tự hoá ở ca kiểm; `SMP-1` sẽ thay bằng hàng
   FIFO có payload (D2-244 tầng 2).
5. **PCID/ASID**: xem §5 — cố ý tắt, có ghi lý do + đo trên `qemu64` và `max`.
6. **`# ipi: mailbox empty=2`**: hai IPI "thừa" của LAPIC (edge fixed-delivery sau `EOI`); **không**
   phải mất đánh thức — đường handler đếm `empty`, đường chờ không. Nếu một ngày `empty` tăng theo
   số yêu cầu thì đó **là** mất đánh thức (đột biến `IPI` lỗi đã có `invalid` để bắt riêng).
7. **`MEASUREMENT_INVALID` ở lượt `gpu-x86_64.sh` đầu** (`loadavg_1m=26.18 > ncpu=12` — máy có 3 agent
   khác chạy): script vẫn `0 lỗi`; lượt chạy lại sau đó (`loadavg` thấp hơn) cũng `0 lỗi`. Không
   dùng lượt ấy làm số đo thời gian.
