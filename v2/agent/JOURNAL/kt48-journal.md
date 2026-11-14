# KT-48 — PLIC + UART IRQ (K3): claim/complete, context `2*hart+1`, 0 IRQ giả

Ngày: 2026-11-15 (Asia/Tokyo). Người làm: Agent 2 dòng OS. Task: **KT-48** (`v2/agent/QUEUE.md`),
theo K3-PLAN §4.1 + D2-72 **Q7/Q8**.

## 0. Trạng thái khi vào

- HEAD `b0e9f91b` (`[KT-00] lật KT-47 DONE (TAP 4/4, 1000 tick, 50 boot); mở KT-48 PLIC/UART IRQ`);
  `git rev-list --count HEAD..origin/main = 0`; working tree có `viscript/` + `review/` của tab
  khác — **không chạm**, chỉ stage `v2/`.
- KT-44..KT-47 DONE; KT-48 READY. Baseline `gate --fast`: **569 test PASS**
  ([`kt48-baseline-gate-fast.log`](kt48-baseline-gate-fast.log)); baseline ktest KT-47 **12/12 +
  3/3** ([`kt48-baseline-ktest.log`](kt48-baseline-ktest.log)).

## 1. Đỏ trước

Sửa `v2/test/boot/ktest.sh` đòi `1..15` + **3 TAP PLIC/UART** + 4 dòng bằng chứng mới, chạy khi
**chưa** có code PLIC ⇒ [`kt48-red-ktest.log`](kt48-red-ktest.log):

```text
not ok 1 - ktest boot 1: thiếu plan-1..15 ok8-plic-window ok9-uart-irq ok10-plic-no-fake-irq
ok11-timer-monotonic ok12-timer-no-lost-tick ok13-timer-irq ok14-timer-drift ok15-nested
plic-window-evidence plic-map-evidence uart-irq-evidence plic-quiet-evidence
# 1/2 boot ảnh test sạch (smp=1)
EXIT=1
```

Đỏ đúng lý do: ảnh KT-47 chỉ có `1..12`, chưa enable PLIC/UART nên `uart irq = 0`.

## 2. Việc đã làm

### 2.1 `vifast-plat-common::plic` (mới, `#![forbid(unsafe_code)]`)

- Hình học PLIC spec: priority `0x0`, pending `0x1000`, enable `0x2000 + 0x80*ctx`, context
  `0x20_0000 + 0x1000*ctx` (threshold `+0`, claim/complete `+4`); max source id **1023**.
- `context_of_hart(h) = 2*h+1` (S-mode) / `machine_context_of_hart(h) = 2*h` (M-mode — chỉ kiểm
  "không bật nhầm"); `enable_word_index`/`enable_bit`/`enable_offset`/`priority_offset`/
  `threshold_offset`/`claim_offset`.
- Hai cửa sổ mapping 2 MiB phủ context 15 của `MAX_HARTS = 8` + `const` assert chặn đổi hằng số.
- `classify_claim(claim, uart_irq)` → `Spurious` (0) / `Uart` (10) / `Unexpected(line)`;
  `UART_IRQ_LIMIT = 8`; `uart_limit_reached`; `ndev_covers_uart`.
- `PlicStats`/`PLIC_STATS` + `PlicSnapshot`: `traps/claims/completions/uart_irqs/spurious/
  unexpected/last_irq/last_iir/uart_ier_off`, `balanced()`, `fake_irqs()`; `note_iir` lưu
  **mã nguồn** `IIR & 0x0f` (0x2 = TX-empty) vì bit FIFO 7:6 do firmware quyết định.

### 2.2 `trap` + `Platform`

- `trap`: thêm `INTERRUPT_SUPERVISOR_EXTERNAL = 9` (PLIC cổ điển — D2-72 Q8); doc ghi rõ mã 5 =
  timer (KT-47), mã 9 = external (KT-48 claim/complete), mã khác vẫn fatal.
- `Platform::enable_irq`/`disable_irq`: hợp đồng đổi từ stub KT-45 → **PLIC thật** (set/clear bit
  enable context `2*hart+1` + đọc lại); cổng CPU `sie.SEIE + sstatus.SIE` do
  `plic::enable_external` (đối xứng `timer::enable_timer`, P2a).

### 2.3 `vifast-plat-riscv64::plic` (mới, `unsafe` thuộc D2-17)

- `probe_ndev`: PLIC spec **không** có thanh ghi `ndev`; QEMU chỉ hiện thực priority cho
  `1..=ndev` ⇒ ghi 1/đọc lại/trả 0, dừng ở nguồn đầu tiên không giữ giá trị (không giả định 95).
- `init`: `threshold = 0`, `priority[UART0_IRQ] = 1`, set enable context `2*hart+1`, đọc lại
  context S-mode/M-mode + claim rỗng; panic nếu `ndev` không phủ IRQ 10. Trả `PlicReport` toàn giá
  trị **đọc lại** (không phải hằng số viết tay).
- `claim`/`complete` (chỉ complete claim ≠ 0), `enable_irq`/`disable_irq` + `irq_enabled`,
  `enable_external`/`disable_external` (`sie.SEIE` bit 9 trước `sstatus.SIE`), UART `IER`/`IIR`
  (`IER.TX-empty = 0x02`, `IIR` mã nguồn `0x2`).
- `on_external_interrupt`: claim context hiện tại → `PLIC_STATS.begin_claim` → line 10: đọc
  `IIR`, **complete**, rồi **re-arm** `IER 0→2` (PLIC level-triggered có gateway: nguồn còn cao
  sau claim không tự pending lại ⇒ cần level transition). Đủ 8 ngắt: tắt `IER` + clear enable
  rồi complete. Line lạ: đếm + complete; claim 0: spurious (không complete). **Không in** handler.

### 2.4 MMU + `trap_dispatch` + ảnh kernel

- `mmu::map_mmio`: PLIC từ **4 KiB** → **2 cửa sổ 2 MiB Device RW NX** `0x0c00_0000` +
  `0x0c20_0000` (KT-46 doc §8.1 đã cập nhật); UART vẫn 4 KiB.
- `trap_dispatch`: `Event::Interrupt { code: 9 }` → `plic::on_external_interrupt()` +
  `TRAP_STATS.record_interrupt(9)` (không fatal); mã khác 5/9 vẫn `report_interrupt` + halt.
- Ảnh test chính: **15 TAP** — 8..10 PLIC/UART, 11..14 timer (chạy **sau** PLIC ⇒ hồi quy timer
  cùng boot), 15 trap lồng fatal. Ảnh sản phẩm: bật PLIC + UART IRQ thật **không icount**, in
  `# plic:`/`# uart irq:`, sai ⇒ `PANIC`.
- `ktest.sh`: `1..15`, 3 TAP mới + 4 bằng chứng (`# plic:`, `# plic map:`, `# uart irq:`,
  `# plic quiet:`), timer đánh số lại 11..14, nested 15.

### 2.5 Host tests (D2-72 Q12)

`vifast-plat-common/tests/plic.rs` mới **12 test**: hằng số hình học, context `2*hart+1` cho hart
0..3, word/bit enable (biên 31/32/33/1023), offset threshold/claim, hai cửa sổ phủ `MAX_HARTS=8`,
phân loại claim 3 nhánh, claim/complete cân bằng + 0 IRQ giả, limit tắt sau N, `ndev` không giả
định 95, priority > threshold. Tổng `vifast-plat-common` **46/46** (13 trap + 1 API + 8 MMU +
12 timer + 12 PLIC) — [`kt48-host-tests.log`](kt48-host-tests.log).

## 3. Kết quả đo (do script/cargo in — log, không chép tay vào tài liệu)

| Hạng mục | Kết quả | Log |
|---|---|---|
| Host test `vifast-plat-common` | **46/46** | [`kt48-host-tests.log`](kt48-host-tests.log) |
| Budget `.data+.bss` | product **344 B**, ktest **352 B**, ktest-guard **344 B** / 262.144 | [`kt48-budget.log`](kt48-budget.log) |
| TAP ảnh test (`-icount shift=6`) | **15/15** chính + **3/3** guard, **2 vòng liên tiếp** (4/4 boot) | [`kt48-ktest.log`](kt48-ktest.log), [`kt48-ktest-2rounds.log`](kt48-ktest-2rounds.log), raw [`kt48-ktest-main-raw.log`](kt48-ktest-main-raw.log), [`kt48-ktest-guard-raw.log`](kt48-ktest-guard-raw.log) |
| PLIC/UART evidence | `# plic: ndev=<đo> ctx=1 plat_off=1 plat_on=1 threshold=0 priority_uart=1 enable_ctx=1 enable_mmode=0 claim_idle=0`; `# uart irq: irqs=8 claims=8 completes=8 traps_delta=8 balanced=1 spurious=0 unexpected=0 last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0`; `# plic quiet: claim_idle=0 quiet=1 stats_unchanged=1 external_traps=8 interrupts_delta=8 spurious=0 unexpected=0` (`interrupts_delta` = **hiệu** từ trước `plic::init`) | raw trên |
| Timer sau PLIC (hồi quy KT-47 cùng boot) | `ok 11..14`, `ticks=1000 late=missed=0`, `scause=0x8000000000000005`, drift `in_range=1` | raw trên |
| Product 50 boot (**không icount**, có PLIC + UART IRQ) | **50/50** sạch | [`kt48-boot50.log`](kt48-boot50.log) |
| Product 1 boot raw | `# plic:` + `# uart irq: irqs=8 …` + `# timer: ticks=1 …` + `ok 1 - k3 boot` | [`kt48-product-boot-nographic-raw.log`](kt48-product-boot-nographic-raw.log) |
| Đỏ trước | `ktest.sh` thiếu 3 TAP PLIC + `plan-1..15` + 4 bằng chứng | [`kt48-red-ktest.log`](kt48-red-ktest.log) |
| Baseline gate | **569 PASS** | [`kt48-baseline-gate-fast.log`](kt48-baseline-gate-fast.log) |
| Gate cuối | xem [`kt48-gate-fast.log`](kt48-gate-fast.log) | — |

## 4. Ghi chú kỹ thuật / bài học

- **`IIR` thô = `0xc2` trên QEMU** vì OpenSBI bật FIFO (bit 7:6); TAP so **mã nguồn**
  `IIR & 0x0f = 0x2` (TX-empty) — nếu so `0xc2`/`0x2` tuyệt đối sẽ đỏ oan.
- **Level-triggered + gateway**: claim/complete thuần chỉ cho **1** ngắt TX-empty (nguồn giữ mức
  cao). Handler phải re-arm `IER 0→2` **sau** complete để có level transition; nếu re-arm trước
  complete, phần cứng thật có thể coi là "trong flight" và bỏ.
- **Ngắt có thể đến ngay trong `enable_external`** (SIE bật là trap): snapshot `TRAP_STATS` phải
  lấy **trước** khi mở cổng, nếu lấy sau sẽ thấy `traps_delta=0` dù đã xử lý đủ 8 IRQ.
- **PLIC map 2×2 MiB**: leaf 2 MiB cho `0x0c00_0000` + `0x0c20_0000` phủ priority/enable + context
  tới context 15 (`MAX_HARTS = 8`); TAP 6 đổi `plic_dev` từ "4 KiB leaf" sang "2 leaf 2 MiB" và
  vẫn kiểm Device RW NX đúng PA.
- **Product cũng bật PLIC**: chứng minh đường boot thật (không icount) có IRQ thật, không chỉ ảnh
  test; handler tự tắt sau 8 ngắt nên không ảnh hưởng TAP/timer sau đó.
- **Timer vẫn chạy sau PLIC**: TAP timer (11..14) chạy **sau** TAP 8..10 trong cùng boot;
  `TRAP_STATS.interrupts=1008 = 8 external + 1000 timer` chứng minh không "nuốt" ngắt timer.
- `ndev` probe ghi/đọc toàn bộ 1023 priority rồi trả 0 — nằm trong cửa sổ thấp đã map; QEMU trả 0
  cho nguồn không tồn tại nên dừng đúng ở `<đo>` (không giả định 95).

## 5. Giới hạn phép đo (ghi trong `k3-riscv64.md` §11.6)

1. Chỉ **1 hart** (SMP là KT-49): "context đúng hart 0..3" = công thức + host test + boot hart 0
   `ctx=1`, **không** phải 4 hart cùng lúc.
2. `ndev` probe dựa hành vi QEMU; phần cứng khác cần DTB/khác (chưa có `vifast-dtb`).
3. Chỉ UART0 line 10; IPI/AIA/MSI/EOI động chưa làm.
4. "0 IRQ giả" là **bằng chứng đếm** (spurious/unexpected = 0, claim rỗng, im lặng sau tắt cổng).
5. Timer sau PLIC đo dưới `-icount shift=6` (D2-74) — cùng giới hạn §10.7.

## 6. Review độc lập

`reviewer-kt48` (read-only, lead spawn): **DRIFT nhẹ** — code/DoD đạt, **không UNSAFE**; tự chạy
host **46/46**, ktest **15/15 + 3/3**, product **50/50**, gate **569**; xác nhận offset/context đúng
`2*hart+1` (không lẫn M-mode), đường level-triggered thật, timer chạy **sau** PLIC cùng boot,
2×2 MiB Device không W+X không đè vùng khác, product `ktest-strings=0`. 4 mục cần vá (đã vá trước
commit, không đổi ngưỡng):

1. `interrupts_delta` trước là **tổng** `interrupts` (đúng tình cờ vì timer chưa chạy) ⇒ đổi thành
   **hiệu thật**: chụp `interrupts` **trước `plic::init`** rồi trừ; TAP 10 so `interrupts_delta ==
   external_traps` vẫn giữ nghĩa.
2. Doc claim TAP 8 kiểm `Platform::enable_irq/disable_irq` nhưng code chỉ gọi `plic::irq_enabled`
   ⇒ **thêm lời gọi wrapper thật** trên HW: `PLATFORM.disable_irq(10)` → đọc lại tắt,
   `PLATFORM.enable_irq(10)` → đọc lại bật; bằng chứng `plat_off=1 plat_on=1`, có trong điều kiện
   TAP 8.
3. Cửa sổ quiet docs nói ~1 ms, code 100 µs ⇒ chốt **1 ms** (`TICK_TICKS / 10`), docs giữ "~1 ms".
4. **D2-75** (Agent 0, `5db8eaae`): `probe_ndev` được phép ở **cả ảnh sản phẩm** với điều kiện
   **không phá trạng thái** ⇒ code probe mỗi nguồn `write-1 → read-back → ghi 0 → đọc lại 0`,
   **panic** nếu khôi phục thất bại; docs `k3-riscv64.md` §11.2 ghi "chẩn đoán không phá huỷ".
   Nit: comment TAP số cũ (8/9/10/11) trong phần timer đã sửa thành 11..14.

Sau vá: host **46/46** ([`kt48-host-tests.log`](kt48-host-tests.log)), ktest **15/15 + 3/3 × 2 vòng**
([`kt48-ktest.log`](kt48-ktest.log), [`kt48-ktest-2rounds.log`](kt48-ktest-2rounds.log)), product
**50/50** ([`kt48-boot50.log`](kt48-boot50.log)), budget 344/352/344 B
([`kt48-budget.log`](kt48-budget.log)), gate **569**
([`kt48-gate-fast.log`](kt48-gate-fast.log)).

## 7. Không làm (đúng phạm vi)

SMP (KT-49), IPI (KT-50), ASID/TLB (KT-51), object layer/Retype (KT-52), U-mode/3 syscall
(KT-53), harness matrix/`expected.txt` (KT-54), conformance (KT-55), `FaultKind`/W^X/spec (KT-56),
soak/K3-DOD (KT-57), port core (K3b), AIA/MSI; `spec/`, sim, `.scn`, QUEUE/DECISIONS/AGENTS,
`viscript/`, `review/` **không chạm**.
