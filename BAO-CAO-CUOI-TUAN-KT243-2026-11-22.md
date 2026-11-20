# BÁO CÁO CUỐI TUẦN KT-243 — 2026-11-21 → 2026-11-22

Người thực hiện: **Agent 1, toàn quyền Agent 0** (uỷ quyền trực tiếp của người dùng, 2026-11-21 00:05 JST).
Hạn: hết **Chủ nhật 2026-11-22 23:59 JST**.
Kế hoạch: [`KE-HOACH-CUOI-TUAN-2026-11-21.md`](KE-HOACH-CUOI-TUAN-2026-11-21.md). Quyết định: D2-195..D2-198.

> **Trạng thái: HOÀN TẤT (hai vòng).** Vòng 1: KT-243..KT-252. Vòng 2: KT-253..KT-257.
> HEAD cuối `14df1639` (đã push), T1 + T2 xanh, cây sạch.

---

## 0. TL;DR (điền cuối)

| # | Việc | Kết quả | Bằng chứng |
|---|---|---|---|
| 1 | K4' bước 3 — aarch64 boot | **ĐẠT** (B3.1–B3.3 + B3.8 đầu) | `# viFastOs aarch64: boot ok`, MMU bật, TAP 5/5, 5 ca âm |
| 2 | D1 — mạng (virtio-net + DHCP + ping) | **ĐẠT** | `dhcp ip=10.0.2.15`, `ping 10.0.2.2 ok n=3`, 7 TAP |
| 3 | B4/B6/B7 đóng | **ĐÓNG cả ba** | parity 14/14, build-inputs (l6), `smp` 9/9 + gate |
| 4 | K6 chốt (S1) | **S1 = ĐẠT** | `help cmds=17`, `ls`, `cat`, `exit` trên ảnh sản phẩm |
| 5 | Luật `wake` — vòng tấn công mới | **WB-20 tìm + vá**; KT-184/194 **vẫn REVIEW** | 19 biến thể/6 nhóm; selftest 406→411 |
| 6 | **K4' B3.4** — nửa cao + physmap | **ĐẠT** | `# high: va=0xffffffc0… pa=0x4008… match=1`; `# physmap: … match=1` |
| 7 | **K4' B3.5** — `VSpace` thật | **ĐẠT** | map/walk/activate + `fault: kind=Vm esr=0x96000006`; TAP 9/9 |
| 8 | **K5' bước 1** — x86_64 boot | **ĐẠT** | `# viFastOs x86_64: boot ok`, long mode `lma=1`, PVH `0x336ec578` |
| 9 | **D1 phần 3** — virtio-input (bàn phím) | **ĐẠT** | `sendkey a` ⇒ `ascii=0x61`, `shift-a` ⇒ `0x41`, `f1` ⇒ none |
| 10 | **Đóng nợ luật `wake`** | **L2 + DEBT-KT227-1 đóng; `--cfg test` kiểm máy** | selftest 411→420, 65→67, cfg-test 10 |

---

## 1. Bối cảnh và phạm vi

- Xuất phát: HEAD `9719c720` (KT-242 đóng), cây sạch, build-inputs `54f3b59fc08f3221`.
- K6 đã xong phần đo được; còn **S1** (shell trên ảnh sản phẩm) và **N1..N7**.
- K4' bước 1+2 đã DONE (KT-160/164/167); bước 3 (boot) **chưa mở** trước cuối tuần này.
- D1 (virtio-net) **chưa có** dòng mã nào.

Phạm vi đã mở (D2-195..D2-198): K4' B3.1–B3.3 + B3.8; D1 (net); B4/B6/B7; K6 S1; luật wake vòng mới.
**Không** làm: B3.4–B3.7, K5', v2.0, đổi ABI/`spec/`, nới ngưỡng.

---

## 2. Cách chia việc

| Sub-agent | Worktree | Nhánh | Task |
|---|---|---|---|
| `a64` | `/tmp/kt243-s1` | `task/kt243-a64` | KT-247 → 248 → 249 → 250 |
| `netlib` | `/tmp/kt243-s2` | `task/kt243-net` | KT-244, KT-251 |
| `infra` | `/tmp/kt243-s3` | `task/kt243-infra` | KT-243, KT-252 |
| `netdrv` | `/tmp/kt243-s4` | `task/kt243-netdrv` | KT-245, KT-246 |

---

## 3. Kết quả từng task

### 3.1 KT-247 — K4' B3.1: crate ảnh aarch64 + linker + `_start` + PL011
**XONG.** Crate mới `v2/kernel/vifast-kbin-aarch64` (linker nạp `0x4008_0000`, `.stack` NOLOAD 64 KiB
+ guard 4 KiB), PL011 driver + bảng tài nguyên QEMU `virt` trong `vifast-plat-aarch64`, halt qua
PSCI `SYSTEM_OFF`. **Đo**: build `--target aarch64-unknown-none` xanh; `llvm-objdump -f` start
`0x4008_0000`; QEMU in `# viFastOs aarch64: boot ok` **3/3 lần**, `qg_boot rc=0` (tự thoát, không
treo). 13 test host PL011/resources. Journal `JOURNAL/kt247-aarch64-boot.md`.

### 3.2 KT-248 — K4' B3.2: bảng vector + handler đồng bộ
**XONG.** 16 ô × `0x80`, căn `0x800`, trong `.text` (không `.bss`), `VBAR_EL1`; handler lưu
`ESR/FAR/ELR/SPSR`. **Đo**: `brk #0` ⇒ `ec=0x3c`, `elr` khớp `expect`, `trap: other=0`; 8 test
host khẳng định **offset tuyệt đối** từng ô; **2 đột biến ĐỎ** (lệch `0x80` ở tầng host và ở asm
`.org`). Journal `JOURNAL/kt248-aarch64-vector.md`.

### 3.3 KT-249 — K4' B3.3: MMU + identity map
**XONG.** Chuỗi `TCR → MAIR → TTBR0 → SCTLR → ISB` với marker **trước/sau từng bước**; arena bảng
trang `AtomicU64` căn 4 KiB (**không `unsafe`**). **Đo**: `SCTLR_EL1.M=1`; marker qua VA không đổi;
**probe VA `0x5000_0000` (ngoài RAM) đọc ra magic** ⇒ translation thật, không chỉ bật bit; 10 test
host. **3 đột biến**: (ii) `T0SZ` 25→24 ⇒ **treo có kiểm soát** `rc=124` dừng đúng bước 4/5;
(i) bỏ `ISB` và (iii) `MAIR` sai **chỉ đỏ ở tầng host** — QEMU TCG không mô hình hoá, **ghi trung
thực** là chưa kiểm được trên phần cứng. Arena 64→32 frame ⇒ `.data+.bss` **140 KiB** (trần 256 KiB).
Journal `JOURNAL/kt249-aarch64-mmu.md`.

### 3.4 KT-250 — K4' B3.8: runner QEMU aarch64 + TAP
**XONG.** `v2/test/boot/run-aarch64.sh` (dùng `qemu_guard.sh`: process group + timeout). **Đo**:
**7/7 kiểm sạch**, TAP **5/5**, **5 ca âm** (marker sai, thiếu `ok`, có `not ok`, plan sai, log
rỗng) đều **ĐỎ đúng**; ca âm đầu-cuối `EXPECT_MARKER` sai ⇒ `EXIT=1`. Đã nối vào cổng: bước **56**
(chạy ở **cả T1 và T2**). Journal `JOURNAL/kt250-runner.log`.

### 3.5 KT-244 — `vifast-net` codec
**XONG.** Crate `v2/lib/vifast-net` (`no_std`, `deny(unsafe_code)`, không dependency):
`addr`/`checksum`/`eth`/`arp`/`ipv4`/`icmp`/`udp`/`dhcp`/`frame`. **58 test host XANH**; log
đỏ-trước **18 đỏ** (icmp/dhcp còn `unimplemented!`); vector chuẩn RFC 1071 + frame ARP + DHCP
DISCOVER byte-exact; IPv4 **từ chối phân mảnh**. Journal `JOURNAL/kt244-net-lib.log`.

### 3.6 KT-245 — driver virtio-net
**XONG.** `vifast-virtio::net` (config/feature/header thuần, 23 test host) + `plat-riscv64::virtio_net`
(MMIO, 2 queue rx/tx, DMA 1 trang, MAC thật) + `VirtioKind::Net` trong `pci.rs` + ảnh `ktest-net`.
**Đo**: MAC `52:54:00:12:34:56` đọc từ thiết bị; TX/RX ARP **thật của slirp**; **4 TAP**; ca âm
"không thiết bị" ⇒ `net: no device` + TAP đỏ. Journal `JOURNAL/kt245-journal.md`.

### 3.7 KT-246 — DHCP + ARP + ICMP echo
**XONG.** **Đo nguyên văn**:
```text
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# net: arp gw=52:55:0a:00:02:02
# net: ping 10.0.2.2 ok n=3 rtt=0
ok 1..7 - net device/mac/dhcp/arp/ping/tx/rx
PANIC: KT-246 net xong (7 TAP, 0 fail)
```
Hai ca âm: `no device` và `no link` (timeout **3 s thật**, `elapsed_ms=3000`, QEMU tự thoát).
Runner `v2/test/boot/net.sh`: 3/3 ca + `--red-selftest` **12 kiểm**. **Lỗi thật tự bắt được**:
`ar_tip` của ARP reply là IP **người hỏi** — sai thì ARP timeout dù thiết bị đã trả lời
(`kt246-red-arp-bug.log`). Journal `JOURNAL/kt246-journal.md`.

### 3.7b KT-246b — de-dup codec
**XONG.** Xoá **420 dòng** codec trùng + 20 hằng trùng khỏi `virtio_net.rs` (**1.494 → 1.074** dòng,
99,6 % → 71,6 % trần R1), dùng `vifast-net`. Chạy lại **đúng** runner: mọi dòng `ok`/`not ok`/
`PANIC`/`# net:` **giống từng byte**; chỉ 2 dòng nhiễu (địa chỉ DMA dịch `0x1000`, `elapsed_ms`
dưới `-icount`). Journal `JOURNAL/kt246b-journal.md`.

### 3.8 KT-243 — B4/B6/B7
**ĐÓNG cả ba** (xem `JOURNAL/kt243-b4b6b7.md`):
- **B4**: parity `cd`/`pwd` **14/14** trên ảnh `product-fs` (`kt243-b4-parity.log:622,732`).
- **B6**: `build-rev.sh --selftest` PASS (a..k) + ca dương **(l6)** ở `build-identity.sh`; 3 lần
  build (sạch / chỉ sửa `v2/docs`+`v2/agent` / target dir lạnh) **cùng sha `5bd28bec…`**.
- **B7**: `smp` **9/9**; bước gate `L0 kernel workspace (KT-148)` phủ ở **cả T1 và T2**; ca âm tái
  lập đúng `86016/118784` và **bước gate ĐỎ**. B7 **đã được KT-148 sửa từ 2026-11-18** — không
  thêm bước trùng.

### 3.9 KT-251 — chốt K6 (S1)
**S1 = ĐẠT.** Ảnh sản phẩm `product-shell,product-fs`, boot smp 1 và 2; log serial:
`shell>` (`kt251-s1-smp1.log:91`), `# shell: help cmds=17` (`:92`), `# shell: ls n=2 path=/` (`:97`),
`# shell: cat ok bytes=79 first=ViFastOs` (`:102`), `# shell: exit code=0` (`:104`).
`K6-DOD.md` §3.2 cập nhật kèm `log:dòng` + sha ảnh + `# build:`. Journal `JOURNAL/kt251-k6-closure.md`.

### 3.10 KT-252 — luật `wake`: vòng tấn công mới
**Tìm + vá WB-20; KHÔNG tuyên bố đóng.** 19 biến thể / 6 nhóm (alias/UFCS/fn-pointer; helper/
closure/const/static; macro; cfg/feature; nguồn sinh; đường dẫn). **WB-20** = "cửa thứ bảy qua
**trùng tên**": `fn wake_step` ở `impl` kiểu khác / module con / trait impl / free fn trong
`state_write.rs` gọi `write_state(t, Ready)` ⇒ `cargo` RC=0 + **cả ba lớp XANH**. Vá:
`_check_state_write` đòi cửa là **method cùng kiểu** với `write_state` (không hardcode `Machine`;
lớp 2 import lại nên một nguồn luật). Selftest lớp 1 **406→411**, lớp 2 **65/65**; a5–a8 **ĐỎ cả ba
lớp** sau vá. **KT-184/KT-194 vẫn REVIEW** (D2-190). Journal `JOURNAL/kt252-wake-attack.md`.

---

### 3.11 KT-253 — K4' B3.4: nửa cao (`TTBR1_EL1`) + physmap
**XONG.** Theo **D2-199** (Q4 = (c)): **không** đổi trait `PageTableFormat`; dựng **hai** `PageTable`
+ `SplitMmu` **định tuyến theo VA** (tái dùng toàn bộ logic `PageTable`). **Đo**:
`# high: kmain va=0xffffffc08000231c pa=0x4008231c match=1`; `# high: jump ok`;
`# physmap: pa=0x4008a000 read=0x504859534d415001 match=1`; `# high: vbar=… match=1`. 16 ca host.
**3 đột biến**: (i) `T1SZ` 25→24 ⇒ host ĐỎ + QEMU **treo có kiểm soát** `rc=124`; (ii) `TTBR1` ←
bảng thấp ⇒ treo có kiểm soát; (iii) bỏ map physmap ⇒ **ĐỎ quan sát được** (`ec=0x25`), runner 6/7.
Journal `JOURNAL/kt253-aarch64-high.md`.

### 3.12 KT-254 — K4' B3.5: `VSpace` thật
**XONG.** `create`/`map_obj`/`unmap`/`walk`/`activate`; fault `FaultKind::Vm` dựng từ
`ESR_EL1`/`FAR_EL1`. **Đo**: `# vspace: map va=0x60000000 pa=0x4008d000 walk=0x4008d000 match=1`;
`# vspace: activate ttbr0=0x400a4000 ttbr1=0x4009f000 asid=1`; đọc sau `unmap` ⇒
`# fault: kind=Vm va=0x60000000 access=Read reason=NotMapped esr=0x0000000096000006 far=0x60000000`.
TAP **9/9**, runner **7/7**, test host **78 → 112**. **Hai phát hiện thật**: (1) `CPACR_EL1.FPEN=0`
⇒ compiler phát NEON (`movi v0.2d`) trong `PageTable::unmap` ⇒ `Undefined Instruction` (`EC=0x07`);
`_start` nay bật `CPACR_EL1=0x0033_0000`. (2) Ảnh liên kết ở PA ⇒ vtable `core::fmt` là địa chỉ nửa
thấp ⇒ sau `activate` phải identity-map ảnh kernel. Journal `JOURNAL/kt254-aarch64-vspace.md`.

### 3.13 KT-257 — K5' bước 1: x86_64 boot
**XONG.** **Phát hiện quan trọng**: QEMU x86 `-kernel` **chỉ** tìm multiboot **1** và **từ chối
ELF64**; đường thật để nạp ELF64 là **PVH** (note `"Xen"` type 18). Ảnh mang **cả hai** header.
**Đo**: `# viFastOs x86_64: long mode cr0=0x80000011 cr4=0x20 cr3=0x105000 efer=0x500 lma=1`;
`# viFastOs x86_64: boot eax=0x0 ebx=0x1580 pvh_magic=0x336ec578 pvh_ok=1`;
`# viFastOs x86_64: identity map 0x0..0x40000000 pages=512`; `# viFastOs x86_64: boot ok` + halt.
Runner **7/7** + **5 ca âm** (gồm ảnh thật build `--features wrong-marker` in `boot BAD`).
Đánh đổi ghi ở **D2-201**: `deny(unsafe_code)` (chỉ `asm.rs` allow) + `-no-pie`. Journal
`JOURNAL/kt257-x86_64-boot.md`; tài liệu `v2/docs/k5-x86_64-boot.md`.

### 3.14 KT-255 — D1 phần 3: virtio-input (bàn phím)
**XONG.** `vifast-virtio::input` (config/event/`key_to_ascii`, **30 test host**) + driver MMIO một
queue event + ảnh `ktest-input` + runner `input.sh`. **Đo**: `sendkey a` ⇒ `code=30 ascii=0x61`;
`sendkey shift-a` ⇒ `ascii=0x41`; `sendkey f1` ⇒ `ascii=none`; `# input name: QEMU Virtio Keyboard`;
`# input stats: events=13 syncs=6 key_downs=4 key_ups=3 errors=0`. **5 TAP** + ca âm "không thiết
bị" + `--red-selftest` **15 kiểm**. **Hai bẫy thật bắt được ở test đỏ-trước**: `KEY_A..=KEY_Z`
**không** liền (30..=50 còn `;` `'` `` ` `` Shift `\`), và mã phím **không** theo alphabet (hàng trên
`q w e r t y u i o p`) ⇒ phải dùng ba bảng hàng QWERTY tường minh. QEMU 11 báo device id `0x1052`
(modern-only) — driver nhận cả `0x1012` và `0x1052`. Journal `JOURNAL/kt255-input.md`.

### 3.15 KT-256 — đóng nợ luật `wake`
**XONG (ba nợ).** (1) **L2**: xoá ba mẫu phạm vi vênh nhau → **một** nguồn `v2/tools/wake_scope.py`
sinh từ `cargo metadata` của hai workspace + bao đóng path-dependency; ba view `contains`/
`token_scope`/`macro_roots` dùng cho **cả hai** checker; ba mã lỗi fail-closed `E_SCOPE_METADATA`,
`E_SCOPE_REDUCTION`, `E_SCOPE_LIB_CORE`. (2) **DEBT-KT227-1**: đo lại — **không** crate `v2/lib` nào
phụ thuộc `vifast-core` (normal dep); đóng bằng ca canh `E_SCOPE_LIB_CORE` (ĐỎ nếu mai kia có) +
đối chứng dương dev-dep. (3) **`--cfg test`**: `v2/tools/check-no-cfg-test.py` — build **thật** ảnh
sản phẩm, đọc `cargo build -v`, ĐỎ nếu có `--cfg test`/`--test`; 10 ca đơn vị + ca âm end-to-end
(`RUSTFLAGS=--cfg test`). Selftest: lớp tĩnh **411→420**, dep-info **65→67**, cfg-test **10**.
**KT-184/KT-194 vẫn REVIEW** — D2-190 còn mục 5 (vòng tấn công mới trên HEAD sau KT-256) và mục 6
(T2 trên commit tích hợp). Journal `JOURNAL/kt256-wake-debt.md`.

### 3.16 KT-258 — vòng tấn công **mới** cho luật `wake` (vòng 3)
**27 biến thể / 8 nhóm → 4 đường lọt thuộc 3 gốc**, đã vá:
- **WB-21** — crate `v2/drivers/evil` là path-dep **tuỳ chọn** của `vifast-kbin-riscv64`, chỉ bật bởi
  `product-fs`, gọi `wake` ⇒ trước vá **XANH cả ba** (kể cả `cargo --features product-fs`).
- **WB-22** — `include!` trong crate ở **gốc mới** (đích trong/ngoài crate) ⇒ trước vá XANH cả ba.
- **WB-23** — path-dep **ra ngoài gốc repo** (`/tmp/…`), tuỳ chọn bởi `product-fs` ⇒ trước vá XANH cả ba.
- **Gốc chung**: phạm vi **định nghĩa** (từ `cargo metadata`) rộng hơn **vòng lặp** của lớp tĩnh
  (`V2_SCAN`) và rộng hơn **cấu hình build** của lớp dep-info. Ba lỗ: `token_scope` trả `False` cho
  crate `workspace="closure"`; worklist `include!`/`#[path]` không seed crate ngoài `V2_SCAN`;
  `_crate_from_package` **bỏ im lặng** path-dep ngoài gốc repo.
- **Vá**: `product_closure()`, `E_SCOPE_OUTSIDE`, seed worklist theo phạm vi. Selftest **420→430**
  (+10 ca có răng, mỗi ca kèm đối chứng dương), dep-info **67/67**, cây thật 0 lỗi.
- **KT-184/194 vẫn REVIEW**: còn nợ §7.3 (luật **cấu trúc** chưa mở cho crate ngoài `V2_SCAN` — cùng
  lớp lỗi với WB-22, có thể dựng lại đường lọt bằng `mod` graph) + T2 trên commit tích hợp.
Journal `JOURNAL/kt258-wake-attack.md`, bảng `JOURNAL/kt258-attack/kt258-attack.tsv`.

### 3.17 KT-259 — aarch64 **smp 2 hart**
**XONG.** PSCI `CPU_ON` + stack per-hart (arena có guard) + **GICv3** (distributor + redistributor +
SGI) + IPI. **Đo**:
```text
# smp: cpu_on rc=0 (success) online=1 waited=156060 hart1_mpidr=0x0000000080000001 ctx=1
# smp: hart=1 online
# smp: ipi recv=1 intid=0
# smp: ipi send=Ok(()) count=1 waited=15159 sgi=0 target=1
# smp: report hart0=1 hart1=1 waited=9859 online=1 ipi=1
ok 10 - smp: hart 1 online
ok 11 - smp: ipi 0->1
ok 12 - smp: both harts report
```
TAP **12/12**, runner **9/9** (7 ca âm), host test **112→136**. **Ba phát hiện**: QEMU `virt` mặc
định **GICv2** ⇒ phải `-M virt,gic-version=3`; **quy tắc "in trước, đặt cờ sau"** (hai hart in xen ký
tự: `# smp: # smp: ipiipi send=recv=1`); `GICR` (0x080A_0000) **chưa** được map trong `VSpace` ⇒ map
thêm 1 MiB. `main.rs` vượt R1 ⇒ tách `smpboot.rs` (511 dòng). **Đột biến**: (i) bỏ clear
`GICR_WAKER.ProcessorSleep` **không** đỏ được trên QEMU 11.1.3 (ghi trung thực) ⇒ thay bằng (i') bỏ
`GICR_ISENABLER0` bit 0 (**ĐỎ** thật); (ii) `CPU_ON` entry+4 ⇒ **PANIC có kiểm soát**; (iii) bỏ guard
stack ⇒ host **ĐỎ** 5 ca + QEMU `layout=Err(Overlap)`.

### 3.18 KT-260 — UDP echo + DNS query (N1 phần đầu)
**XONG.** `vifast-net::dns` (RFC 1035, **21 test host**, có phát hiện **con trỏ vòng lặp**). **Đo**:
```text
# net udp echo: target=10.0.2.3:53 via=dns-question-echo payload_len=15 reply_len=33 rcode=0 echoed=1 rtt_ms=6
# net: udp echo ok len=15 rtt=6
ok 8 - net udp echo
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=61 rtt_ms=6
# net: dns example.com a=172.66.147.243 ttl=205
ok 9 - net dns a
```
**Trung thực**: libslirp **không** có echo service UDP ở `10.0.2.2:7` (`udp_input` chỉ xử lý
BOOTP/TFTP) ⇒ bằng chứng UDP hai chiều lấy từ **DNS question echo** (server chép nguyên question
section, RFC 1035 §4.1.2); `udp_echo` tới `10.0.2.2:7` vẫn hiện thực đầy đủ và **timeout 3 s** đúng.
Ca âm: DNS tới `192.0.2.1:53` (TEST-NET-1) ⇒ timeout 3 s. Runner **9 TAP** + 2 ca âm +
`--red-selftest` **21 kiểm**.

### 3.19 KT-261 — K5' bước 2: x86_64 GDT/TSS/IDT + exception
**XONG.** GDT 5 descriptor + TSS (IST1 cho `#DF`) + IDT 256 ô; **5 exception đo được**:
```text
# exc: vec=3 err=0x0 rip=0x100281 cs=0x8 rflags=0x2
# exc: vec=6 err=0x0 rip=0x100290 cs=0x8 rflags=0x10002
# exc: vec=14 err=0x0 rip=0x1002a0 cs=0x8 rflags=0x10002
# exc: cr2=0x700000000000
# exc: vec=13 err=0x38 rip=0x1002b4 cs=0x8 rflags=0x10002
# exc: vec=255 err=0x0 rip=0x1002c2 cs=0x8 rflags=0x2
```
Mỗi ca **resume bằng `iretq`** rồi chạy tiếp (không cần tách boot). TAP **7/7**, runner **9/9**
(7 ca âm), 26 test host mới. **Ba phát hiện thật**: (1) **`.got` phải GIỮ** trong `linker.ld` — bản
KT-257 `DISCARD` cả `.got`; khi crate khác tham chiếu stub asm qua `extern "C"`, rustc phát sinh
tham chiếu GOT-gián-tiếp ⇒ mọi entry thành **địa chỉ 0** (`callq *0x0`); (2) **`INT3`/`INT n` lưu RIP
lệnh KẾ TIẾP**, fault (`#UD`/`#PF`/`#GP`) lưu RIP **chính** lệnh gây lỗi — resume khác nhau;
(3) **`ltr` đánh dấu descriptor TSS busy** (`0x89 → 0x8B`) ⇒ TAP 1 là bằng chứng `ltr` nạp thật.

### 3.20 KT-262 — K4' B3.6: `TLBI` + shootdown đa hart + smp 4
**XONG.** **4 hart online** + **shootdown thật**:
```text
# tlbi: remap va=0x60000000 pa=0x40093000 magic=0x5653504143450001
# tlbi: hart=1 read1=0x5653504143450001 magic=0x5653504143450001 match=1
# tlbi: shootdown va=0x60000000 targets=0xe expected=3 acks=3 waited=32283 timeout=0 ok=1
# tlbi: hart=1 read2 fault=1 va=0x60000000 access=Read reason=NotMapped esr=0x96000006
# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1
ok 13 - tlbi: hart 1 sees unmap
ok 14 - tlbi: ack from all harts
ok 15 - smp: 4 harts online
```
`seen=0x3` = hart 0 (TAP 9) **và** hart 1 (TAP 13) — hai hart khác nhau, đúng bản ghi fault theo
hart. TAP **15/15**, runner **11/11** (9 ca âm), host test **136→155**. **Hai lỗi thật**: (1) stub
vector **xoá `x9`** (từ KT-248) ⇒ `ldar x9,[x9]` với `x9=4` ⇒ `far=0x4`; vá bằng khung 16 B giữ `x9`
gốc; (2) **`GICR_IPRIORITYR<n>` là thanh ghi theo BYTE** — ghi u32 lệch lề với `sgi=1` ⇒ **alignment
fault** `far=0x080b0401`. **Đột biến**: (i) hart phụ không `tlbi` và (ii) bỏ `dsb ish` **không** đỏ
được trên QEMU TCG (nó flush TLB **mọi** CPU) — ghi trung thực, thay bằng **đối chứng dương** "bỏ
`tlbi` của hart 0, chỉ giữ SGI ⇒ TAP 13 vẫn xanh" ⇒ chứng minh đường SGI thật sự xoá TLB hart 1.

### 3.21 KT-263 — D1 phần 4: virtio-gpu 2D
**XONG.** `# gpu display: w=1280 h=800`, `# gpu resource: id=1 format=1 w=640 h=480`,
`# gpu scanout: … enabled=1 bogus=InvalidResourceId`, `# gpu draw: rect=40,40,200,120 color=0xff0000
text=ViFast`. **Bằng chứng thật** (không chỉ "lệnh gửi xong"): `screendump` PPM `P6 640 480` —
(40,40)=(255,0,0) trong hình chữ nhật, (240,40)=(0,0,0) ngoài, (40,200)=(255,255,255) bit 0 glyph
`'V'`, khối chữ **122** pixel trắng = đúng tổng popcount 6 glyph "ViFast". 5 TAP, **33 test host**,
`--red-selftest` **25 kiểm**, ca âm "không thiết bị". **Trung thực**: `GET_DISPLAY_INFO` của QEMU trả
mode của **host** nên không đổi khi `SET_SCANOUT` ⇒ TAP `scanout` dùng ca âm đối chứng
(`InvalidResourceId`) thay vì so mode.

## 4. Cổng và bằng chứng

| Phép đo | Kết quả | Log |
|---|---|---|
| Baseline `v2 check` (HEAD `9719c720`) | **PASS 74,6 s** | `/tmp/base-check.log` |
| **T2** `gate --fast` trên `14df1639` (cuối) | **PASS 62/62, 534,3 s** | `/tmp/t2-r2b.log` |
| **T1** `gate --dev` trên `14df1639` (cuối) | **PASS 301,5 s** | `/tmp/t1-r2c.log` |
| T2 vòng 1 trên `6309b6a6` | PASS 57/57, 493,1 s | `/tmp/t2-final.log` |
| Bước mới trong cổng | 55 net validator, 56 **aarch64** (T1+T2), 57 virtio-net (T2), 58 cfg-test (T2), 59 **x86_64** (T1+T2), 60 virtio-input (T2), 61/62 validator (T1+T2), 63 QEMU smoke (T1) | `v2/bin/v2` |
| Build-inputs | **257 mục**, `f4934dc81f0c903e`, `dirty=0` | `repro-build.sh` |
| Ảnh sản phẩm (HEAD cuối) | `e78f58d6…`, 213 632 B, `build=f861bd283066120f`, `text=131728` | `/tmp/kt251b/…` |
| S1 trên ảnh HEAD cuối | **ĐẠT** smp 1 và 2, `REV-GATE ACCEPT` | `JOURNAL/kt251b-s1-smp{1,2}.log` |

**Vì sao KHÔNG chạy lại soak/L1-full:** đầu vào tạo ảnh **có đổi** (thêm hai crate) ⇒ bằng chứng K6
cũ **không** được ghép cho HEAD mới; nhưng phép kiểm S1 đã **đo lại** trên ảnh mới, và
`build=f861bd283066120f` + `text=131728` **trùng** bản trước ⇒ mã sản phẩm không đổi. Soak 1.000 và
L1 400k **không** chạy lại trong cuối tuần (quy tắc ≤ 1 đợt T2/24 h; ba phép đo này thuộc đợt bàn
giao sau, khi K4'/K5' cần). Ghi rõ để không ai đọc thành "K6 đã đo lại toàn bộ".

---

## 5. Việc còn mở sau cuối tuần (cập nhật sau vòng 2)

1. **KT-184/KT-194 vẫn REVIEW.** KT-252 tìm + vá **WB-20** nhưng D2-190 đòi 7 mục; còn nợ:
   DEBT-KT227-1 (`v2/lib` ngoài luật token `wake`, latent), **L2** (ba mẫu phạm vi vênh nhau),
   giả định "build sản phẩm không có `--cfg test`" **chưa kiểm máy**, closure trong thân cửa.
2. **K4' bước 3 phần sau**: B3.4 (nửa cao/physmap — **Q4 chưa chốt**: thêm vào `PageTableFormat`
   hay làm trong crate arch), B3.5 (`VSpace` thật), B3.6 (TLBI shootdown), B3.7 (U-mode — **Q2**:
   mở `spec/abi.toml [calling.aarch64]`). Thêm: smp>1 cho aarch64 (`_start_secondary` + GICv3 +
   PSCI CPU_ON), soak 1.000, `budget.sh` bản aarch64, đọc DTB.
3. **D1 phần sau**: virtio-gpu, virtio-input; TCP; net server U-mode (N1); `ping`/`ifconfig` trong
   shell sản phẩm.
4. **Nợ P20** (sha ảnh phụ thuộc thư mục build) — chưa xử lý.
5. **K5'** — **bước 1 DONE** (boot tới marker); còn IDT/GDT/TSS, APIC, timer, U-mode, PCI, SMP,
   `-cpu max`, soak.
6. **K4' B3.6** (TLBI shootdown đa hart) + **B3.7** (U-mode — Q2 vẫn hoãn) + smp>1 aarch64.
7. **Vòng tấn công mới cho luật `wake`** trên HEAD sau KT-256 (D2-190 mục 5) — KT-256 đã đóng L2 và
   DEBT-KT227-1 nhưng **chưa** có vòng tấn công mới sau khi đổi phạm vi.
8. **Arena aarch64 dùng 28/32 frame** — VSpace thứ hai sẽ `PANIC` fail-loud; tăng lên 48 frame vẫn
   ĐẠT I7 (`.data+.bss` 155.680 B / 262.144 B) nhưng phải đo lại.
9. **Ảnh sản phẩm cuối tuần chưa build lại sau vòng 2** — `build-inputs` đã đổi; nếu cần bằng chứng
   K6 trên HEAD cuối thì phải build + đo lại S1 (quy trình đã có ở `JOURNAL/kt251-s1-measure.sh`).

---

## 6. Bài học

1. **Trùng số thứ tự bước trong cổng là lỗi im lặng.** KT-245 thêm `lane_step B 55` vào
   `do_gate_fast`; `do_gate_dev` đã dùng **55** cho QEMU smoke ⇒ `lane_done` coi bước đã chạy và
   **bỏ qua QEMU smoke của T1** suốt từ lúc merge đến khi lead phát hiện. Đã sửa (QEMU smoke →
   58) và ghi lý do ngay trong mã. **Bài học**: mọi bước mới phải kiểm `grep "lane_step.* <id> "`
   trên **cả** `do_gate_fast` và `do_gate_dev`.
2. **`perl -e 'alarm N; exec @ARGV'` không giết được QEMU** — qemu bỏ qua `SIGALRM`. Mọi lần chạy
   QEMU phải qua `qemu_guard.sh` (process group + timeout). Đã báo cho sub-agent từ sớm và tiết
   kiệm hàng chục phút treo.
3. **Đột biến chỉ đỏ ở tầng host là đột biến yếu** — KT-249 ghi trung thực: bỏ `ISB` và `MAIR` sai
   **không** quan sát được trên QEMU TCG (chỉ đỏ ở test host). Không được gọi là "đã kiểm trên
   phần cứng".
4. **Reviewer khác tác giả vẫn tìm ra đường lọt**: WB-20 (trùng tên) sống qua **ba** vòng vá trước
   vì các vòng đó sửa theo *danh sách* chứ không theo *lớp*. Xác nhận lại D2-178(iv)/D2-190.
5. **Hai bản codec là nợ thật**: `netdrv` dựng codec tạm để chứng minh E2E trước (đúng), rồi
   `[KT-246b]` xoá 420 dòng và chạy lại **đúng** runner — chênh lệch chỉ 2 dòng nhiễu. Cách làm
   "E2E trước, de-dup sau, chứng minh bằng log byte-identical" nên dùng lại.
6. **B4/B6/B7 đã xong từ trước** (KT-137/KT-148/D2-127) — phiếu `BLOCKED-CHO-A0.md` cũ. Việc đúng
   là **đo lại + đóng sổ**, không phải viết lại mã. Tiết kiệm hai task.
7. **QEMU x86 `-kernel` không nhận multiboot2** — chỉ tìm multiboot **1** và từ chối ELF64; đường
   thật là **PVH** (note `"Xen"`). Bài học: đọc mã QEMU trước khi tin đặc tả; ảnh mang **cả hai**
   header để vừa đúng đặc tả vừa chạy được.
8. **Compiler phát NEON khi `CPACR_EL1.FPEN=0`** ⇒ `Undefined Instruction` (`EC=0x07`) trong
   `PageTable::unmap`. Bài học: bật `CPACR_EL1` **trước mọi mã Rust** trên aarch64, không chỉ trước
   mã dùng float.
9. **Vtable `core::fmt` là địa chỉ liên kết (PA)** ⇒ sau `activate` phải identity-map ảnh kernel.
   Bài học: "chạy nửa cao" không chỉ là đổi `TTBR1`; mọi con trỏ hàm trong ảnh vẫn trỏ PA.
10. **Ảnh sản phẩm không đổi khi thêm crate mới**: `build=f861bd283066120f`/`text=131728` trùng
   trước và sau cuối tuần ⇒ bằng chứng ảnh cũ vẫn đúng **về byte**, chỉ dòng `# build:` (đầu vào)
   đổi. Đây là lý do S1 phải đo lại nhưng soak **không** cần chạy lại.
