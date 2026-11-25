# KT-406 (Đợt W2) — UART RX IRQ thật (`PL011`/`COM1`) + runner `product-{aarch64,x86_64}.sh` + nhánh `LD_PRELOAD`

- **Nhánh**: `task/kt406-uart-irq`, base `79af0ab9` (W1 đã merge).
- **Ngân sách**: 2 h. Không sửa `run-common.sh`; không chạm `spec/`, `docs/v1|v2/`, QD.
- **Hợp đồng**: `v2/agent/QUEUE.md` dòng KT-406 + `KE-HOACH-KIEN-TRUC.md` §4 dòng KT-406.

## 1. UART RX IRQ thật

### aarch64 — `PL011 → GIC SPI 33 → ring KT-69 → shell`

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/gic.rs` | thêm `GICD_ICENABLER`/`GICD_IROUTER`, hằng SPI (`SPI_FIRST=32`, `SPI_END=1020`, `SPI_PRIORITY`), `is_spi`/`spi_intid`/`spi_word`/`spi_bit`/`spi_router`, `SpiInfo`, `enable_spi`/`disable_spi`/`spi_enabled` (đọc lại thanh ghi + chờ `RWP`), `GicError::InvalidSpi` |
| `v2/kernel/vifast-plat-aarch64/src/pl011.rs` | `UARTMIS=0x040`, `RX_DRAIN_LIMIT=64`, `rx_enable`/`rx_disable`/`masked_status`/`rx_pending`/`rx_drain` |
| `v2/kernel/vifast-plat-aarch64/src/uart_rx.rs` (**mới**) | `PL011 → ring`: `init(pa)`, `on_irq()` (drain → `conchan::pack` → `Ring::send`, bộ đếm irqs/bytes/chunks/drops/empty/no_ring/last), `ring_check`, `ring_stats`, `geometry` |
| `v2/kernel/vifast-plat-aarch64/src/resources.rs` | `UART0_SPI=1`, `UART0_IRQ=33` (DTB `pl011 interrupts = <0 1 4>`) |
| `v2/kernel/vifast-plat-aarch64/src/umode.rs` | `spsr_el0_irq()` — EL0t với `DAIF.I=0` (mask D/A/F) |
| `v2/kernel/vifast-kbin-aarch64/src/pshell.rs` | map `GICD`+`GICR` (Device), `bringup_uart_irq()` (distributor → wake hart 0 → `SRE` → `enable_spi(33)` → `IMSC.RXIM` → ring), handler IRQ ô **1/5/9** (`0x480` = IRQ từ EL0 — thiếu ô 9 thì halt `# exc: slot=9 ec=0x15`), `# uart:` evidence lúc bật + lúc `done()` |

### x86_64 — `COM1 (IRQ 4) → IOAPIC → LAPIC → IDT 0x24 → ring → shell`

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-x86_64/src/exc.rs` | `VEC_SERIAL=0x24`, `STUB_SERIAL=19`, `STUB_COUNT 19→20`, `stub_slot`, tên `com1` |
| `v2/kernel/vifast-plat-x86_64/src/asm.rs` | stub `vifast_isr_serial` (`push 0; push 0x24; jmp common`) + `stub_addr` + extern |
| `v2/kernel/vifast-plat-x86_64/src/serial.rs` | `IER_RX=0x01`, `RX_DRAIN_LIMIT=64` |
| `v2/kernel/vifast-kbin-x86_64/src/comirq.rs` (**mới**) | LAPIC (SVR/TPR/LINT mask) + IOAPIC (mask hết → pin 4 vector `0x24`, dest = LAPIC ID **đọc lại**) + `IER.RX`; `on_irq()` drain → ring → `EOI`; `Report::ok()` |
| `v2/kernel/vifast-kbin-x86_64/src/product.rs` | `exc_dispatch`: vector `0x24` → `comirq::on_irq()` + `FrameReturn::resume(rip)`; spurious 255 → resume |
| `v2/kernel/vifast-kbin-x86_64/src/shellpath.rs` | `comirq::init(ring_pa)` trước `iretq`, in `# uart: rx on=… ok=…`; `finish()` in `# uart: irq=… rx=… drops=… ok=…` |
| `v2/kernel/vifast-kbin-x86_64/src/main.rs` | `mod comirq` (chỉ `product-shell`, không `ktest`) |

### Đường vào **không còn** là kernel bơm script

Thêm feature **`script-input`** (cả hai kbin): script `help/echo/ps/exit` chỉ được bơm khi bật
feature này. Ảnh sản phẩm **mặc định** khởi tạo ring **rỗng**; byte **duy nhất** tới shell là
UART RX IRQ. `run-aarch64.sh`/`run-x86_64.sh` (T1, stdin `/dev/null`) build
`--features product-shell,script-input` để giữ nguyên phép kiểm hồi quy — **không** đổi một dòng
phép kiểm nào, chỉ đổi danh sách feature ở dòng build.

### riscv64 — giữ nguyên (đã có IRQ từ KT-73)

`uart_console` (PLIC line 10 → `IER.RX` → ring) đã là đường thật; chỉ thêm **kịch bản chung**
`--scenario common` cho runner `product-riscv64.sh` (không đổi phép kiểm `full`).

### Bằng chứng đo được (QEMU thật, QEMU 11.x)

```text
# riscv64 (FIFO stdin, cùng 4 lệnh) — log `kt406-common.log`
# uart irq: irqs=8 claims=8 completes=8 balanced=1 spurious=0 unexpected=0 last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0
# product halt: shell_exit=0 … rx_windows=111 rx_irqs=2 rx_bytes=24 … ring_ok=1
# shell: help cmds=32 / echo KT406 / ps n=3 / exit code=0

# aarch64 (FIFO stdin, gõ help/echo/ps/exit)
# uart: irq=33 rx=1 mis=0x00 spi_enabled=1 router=0x0 prio=0xa0 imsc_rx=1 gicd_ctlr=0x52 lines=288 waker=0x0 sre=0x7 ring_pa=0x4017b000 slots=32 slot=32 ring_check=1 ok=1
# uart: irq=4 rx=24 chunks=4 drops=0 empty=0 no_ring=0 last=5 head=4 tail=4 used=0 ok=1

# x86_64 (FIFO stdin, gõ help/echo/ps/exit)
# uart: rx on=1 irq=4 vector=0x24 lapic_id=0 svr=0x1ff tpr=0 ioapic_id=0 ioapic_ver=0x170020 entries=24 redir_low=0x0024 redir_high=0x00000000 dest=0 ier=0x01 lsr=0x60 ring_pa=0x1dc000 slots=32 slot=32 ring_check=1 ok=1
# uart: irq=4 rx=24 chunks=4 drops=0 empty=0 no_ring=0 last=5 head=4 tail=4 used=0 ok=1
```

## 2. Runner `product-{aarch64,x86_64}.sh` — cùng kịch bản 3 arch

- `v2/test/boot/lib/product-scenario.sh` (**mới**): **một** danh sách `lệnh<TAB>regex` dùng chung
  3 runner + `product_scenario_check` + `product_uart_irq_check` (đòi `irq≥1 rx≥1 drops=0 ok=1`).
- `v2/test/boot/product-aarch64.sh` (**mới**): build ảnh product-shell **không** `script-input`,
  budget `aarch64-product-shell`, quét harness marker, boot FIFO, gõ kịch bản, chờ evidence từng
  lệnh, kiểm `# uart:` + marker + halt + 0 TAP; ca âm `--negative minimal`.
- `v2/test/boot/product-x86_64.sh` (**mới**): như trên cho x86_64 + `# conf image:` khớp hash ELF.
- `v2/test/boot/product-riscv64.sh`: thêm `--scenario common` (dùng chung lib; nhánh `full`
  không đổi) + **sửa lỗi thứ tự có sẵn**: `build_fsutil`/`seed_disk` được **gọi trước khi định
  nghĩa** (bash: `command not found` trên mọi lần chạy, kể cả `--image`) ⇒ dời hai hàm lên trước
  điểm gọi; không đổi phép kiểm nào.

Kết quả (đo thật, trên worktree):

```text
# product-aarch64: checks=11 fails=0
# product-x86_64:  checks=12 fails=0
# product-riscv64 --scenario common: checks=0 fails=0 (bộ đếm ok của runner cũ bắt đầu từ 0)
#   ok - kịch bản chung 3 arch: help/echo/ps/exit đủ evidence
#   ok - uart: RX IRQ thật trên riscv64 (rx_irqs>0)
#   ok - halt: shell_exit=0 (thoát sạch)
```

## 3. Nhánh `LD_PRELOAD` (`.so`) song song Mach-O `.dylib`

- `v2/test/boot/dns-interpose.sh` (**mới**): **một** nguồn C hai nhánh
  - macOS: `DYLD_INTERPOSE` trên `res_9_ninit`/`res_9_getservers`/`res_9_ndestroy` → `.dylib`;
  - Linux: `LD_PRELOAD` + `dlsym(RTLD_NEXT)` bọc `res_init`/`res_ninit`, ghi `_res.nsaddr_list[0]` → `.so`.
  Tự kiểm: dựng chương trình dò **không mở socket**, chạy dưới injection với cổng **5353**
  (không phải resolver mặc định) ⇒ `ok=1` chỉ khi interpose **thật sự** đổi resolver.
- `v2/test/boot/soak-aarch64.sh`: bỏ bản C/dylib nội tuyến, gọi `dns-interpose.sh --lib`; wrapper
  QEMU đặt **biến nạp theo OS** (`DYLD_INSERT_LIBRARIES` hoặc `LD_PRELOAD`); dòng evidence thêm
  `kind=`/`var=` (giữ `dylib=` cũ).

Kết quả đo (không cần mạng thật):

```text
# macOS: # dns-interpose: os=macos kind=dylib var=DYLD_INSERT_LIBRARIES server=127.0.0.1 port=5353 ok=1
# Linux (Lima aarch64): # dns-interpose: os=linux kind=so var=LD_PRELOAD server=127.0.0.1 port=5353 ok=1
```

## 4. Lệnh kiểm

| Lệnh | Kết quả |
|---|---|
| `cargo fmt --manifest-path v2/kernel/Cargo.toml` | sạch (đã chạy) |
| `cargo test -p vifast-plat-aarch64` | PASS (14 ca gic, 7 ca pl011, …) |
| `cargo test -p vifast-plat-x86_64` | PASS (sau khi cập nhật `apic`/`exc`/`smp` cho `STUB_COUNT 20`) |
| `bash v2/test/boot/product-aarch64.sh --negative minimal` | 11/11 ok, exit 0 |
| `bash v2/test/boot/product-x86_64.sh --negative minimal` | 12/12 ok, exit 0 |
| `bash v2/test/boot/product-riscv64.sh --negative no-net` | 7/7 ok, exit 0 — đường `full` không hồi quy sau khi dời `build_fsutil`/`seed_disk` |
| `bash v2/test/boot/dns-interpose.sh` (macOS + Linux) | ok=1 cả hai |
| `env -u CARGO_TARGET_DIR ./v2/bin/v2 gate --dev` | **PASS mọi bước của KT-406**: `QEMU aarch64 (K4' bước 3): OK` (141,7 s), `QEMU x86_64 (K5' bước 1): OK` (278,3 s); còn **2 đỏ nền** (xem §5) |

## 5. Hai đỏ **nền** của T1 trên worktree bẩn (không do KT-406)

T1 (`gate --dev`, chạy 2 lần độc lập) chỉ còn đúng hai chỗ đỏ, **không** liên quan tệp KT-406 sửa:

1. `selftest pre-measure (KT-95): FAIL` — `# pre-measure-selftest (a) FAIL: mã thoát 7`.
   `v2/tools/pre-measure.sh:207-209` từ chối `refuse 7 tree-dirty` khi cây nguồn **bẩn**; worktree
   của worker luôn bẩn (chưa commit được), nên ca (a) — kỳ vọng OK — **không thể** xanh ở đây.
   Tệp này **không** bị KT-406 sửa (`git status --porcelain -- v2/tools/pre-measure.sh` rỗng).
2. `not ok - selftest: netsrv ca âm có dòng PANIC 'xong' (PASS giả) KHÔNG bị bắt (validator rỗng)`
   trong `v2/test/boot/net.sh --red-selftest` (34 ca; **chính script exit 0**, nhưng in một dòng
   `not ok` nên cổng quét dòng đó thành đỏ). Tệp `net.sh` **không** bị KT-406 sửa; lỗ hổng
   `validate_netsrv_negative` (không từ chối log có `PANIC`) có sẵn từ trước, nhiều khả năng là
   phần `task/kt401-r1` chưa merge.

Hệ quả: trên worktree bẩn, T1 không thể xanh tuyệt đối; bằng chứng KT-406 là **các bước QEMU
aarch64/x86_64 đều OK** (trước sửa: x86 treo TAP 44 `yield budget hết`).

## 6. Nợ / blocker đã biết

- **Index**: toàn bộ 30 tệp đã `git add -A` (không commit — hook đòi `VIFAST_ROLE=lead`).
- **GICR mapping**: ảnh aarch64 map cả cửa sổ redistributor của `MAX_HARTS` (`gic_pages=272`) thay
  vì chỉ hart 0; đúng chức năng nhưng tốn 272 trang bảng. Có thể siết ở task sau (không nới ngưỡng).
- **`script-input` (đường cũ)**: log T1 của ảnh `product-shell,script-input` có
  `# uart: irq=0 rx=0 … ok=0` — **đúng theo thiết kế** (stdin `/dev/null`, byte do kernel bơm
  script), không phải lỗi; hai runner mới đòi `ok=1` và đo được `irq=4 rx=24`.
- **`v2 t0` (T0) clippy**: `cargo clippy --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-aarch64
  --all-targets -- -D warnings` báo lỗi `needless_range_loop` trong **`vifast-core`** (tệp không bị
  KT-406 sửa) ⇒ nhiều khả năng là **nền có sẵn** của cây W1 (cùng toolchain 1.100.0). Ghi để Lead
  xác nhận; không sửa `vifast-core` ngoài phạm vi task.
- **`cargo test -p vifast-kbin-x86_64` / `-p vifast-kbin-aarch64`** (host) lỗi `E0152 duplicate lang item panic_impl` — bin
  kernel có `#[panic_handler]`, không phải lỗi của thay đổi này.
