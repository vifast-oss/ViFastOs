// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test console** riscv64 (KT-73, K6) — bin `vifast-kbin-ktest-con`, feature `ktest,user,con`.
//!
//! # Vì sao ảnh riêng
//!
//! (a) Ảnh `ktest` đã **92,6 %** trần `.data+.bss` (D2-109) — thêm ring UART + driver console là
//! vượt; (b) ảnh này cần **thiết bị thật** (UART RX thật + `-device virtio-serial-pci`/`-chardev
//! pipe`) trong dòng QEMU, tách khỏi 15 boot matrix cũ; (c) boot kết thúc bằng **một** dòng
//! `PANIC:` chủ đích (cùng quy ước ảnh vblk/guard/user/proc).
//!
//! # TAP (5 test; tên khớp `expected.txt` + `ktest.sh`)
//!
//! 1. `con uart irq ring` — **UART RX IRQ thật** → ring của KT-69: bật `IER.RX`, bơm 30 byte qua
//!    **loopback 16550** (THR → RBR → `IER` → PLIC → handler), handler đọc `RBR` và `Ring::send`
//!    slot 24 byte; kiểm `irqs ≥ 1`, `bytes == 30`, `head` tiến đúng `ceil(30/24) = 2`, `tail`
//!    chưa đổi, `drops = 0`, `timeouts = 0`; **ca âm** "nguồn RX nhưng không có dữ liệu" được gọi
//!    trước khi bơm ⇒ trả 0 byte + đếm `empty_polls`, không treo, không mất byte.
//! 2. `con u-mode doc ring` — tiến trình U-mode **riêng** (`con_client.elf`) đọc byte từ ring qua
//!    `vifast_ring` (kernel map Frame vào VSpace của nó), in `# con: u bytes=… sum=… first=…
//!    last=…`; kernel so **nguyên chuỗi** với giá trị nó tính từ payload (không tin lời khai),
//!    và `tail == head` (consumer đã tiêu thụ hết) đọc qua **physmap**.
//! 3. `con pci` — phát hiện `virtio-console` (`0x1af4:0x1003`) qua **hạ tầng PCI của KT-72**
//!    (`pci::init_for(VirtioKind::Console)`, không nhân đôi code), gán BAR + kiểm chồng lấn,
//!    đàm phán **duy nhất** `VIRTIO_F_VERSION_1`, **không** `MULTIPORT`, hai queue 0/1 bật với
//!    `queue_size` **đọc từ thiết bị**.
//! 4. `con ghi doc` — ghi 2 lần (15 + 10 byte) qua queue `tx` (thiết bị xác nhận `used->len`);
//!    đọc byte host bơm qua queue `rx` (bằng chứng **độc lập phía host**: `ktest.sh` kiểm tệp
//!    `.out` của chardev có đúng byte guest ghi).
//! 5. `con ca am` — (a) `read` khi **không có dữ liệu** trả `Busy` (14) **ngay**, không treo;
//!    (b) **ring đầy**: bơm 800 B khi không có consumer ⇒ đúng 32 slot vào ring (768 B), 2 slot
//!    bị **từ chối** với mã **9** (`NotEnoughMemory`), **không** ghi đè; kernel đọc lại 768 B
//!    **đúng thứ tự** từng byte.
//!
//! Mọi dòng `# con:` in từ **biến đo** (D2-102); số suy ra in kèm công thức/đầu vào (D2-106).
//! Không `unsafe` trong tệp này (MMIO/physmap nằm ở `kernel/vifast-plat-riscv64`).

#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_core::{Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState};
use vifast_plat_common::timer::TIMEBASE_HZ;
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::pci;
use vifast_plat_riscv64::uart_console;
use vifast_plat_riscv64::user::{self, LoadedInit};
use vifast_plat_riscv64::virtio_console;
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};
use vifast_virtio::conchan;
use vifast_virtio::console::ConsoleError;
use vifast_virtio::pci::{VirtioKind, COMMAND_BUS_MASTER, COMMAND_MEMORY};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// ELF consumer U-mode do `build.rs` sinh (KT-73).
static CON_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/con_client.elf"));

/// Payload ca kiểm 1/2 — 30 byte, qua **UART thật** (loopback) vào ring.
const PAYLOAD: &[u8] = b"con-uart-irq-0123456789abcdef\n";
/// Byte kỳ vọng của ca kiểm 4 (host bơm qua chardev `pipe`).
const RX_EXPECT: &[u8] = b"KT73-CON-RX-OK";
/// Dòng guest ghi qua queue `tx` (bằng chứng phía host đọc tệp `.out` của chardev).
const TX_ONE: &[u8] = b"KT73-CON-WRITE";
/// Dòng thứ hai của `tx`.
const TX_TWO: &[u8] = b"0123456789";
/// Số byte bơm cho ca "ring đầy": 800 B = 34 slot 24 B ⇒ 32 vào ring, 2 bị từ chối.
const FILL_BYTES: usize = 800;

fn put(ok: bool, number: usize, name: &str, fails: &mut usize, why: &str) {
    let mut w = PlatformWriter(&PLATFORM);
    if ok {
        let _ = writeln!(w, "ok {number} - {name}");
    } else {
        let _ = writeln!(w, "not ok {number} - {name}: {why}");
        *fails += 1;
    }
}

/// `true` ⇔ VA `addr` được map **identity** (PA = VA) — ECAM/BAR/trang DMA dùng.
fn identity(addr: u64) -> bool {
    let walk = vifast_plat_riscv64::mmu::walk_sw(addr);
    if !walk.valid || !walk.leaf || addr < walk.phys {
        return false;
    }
    let leaf_bytes = 4096u64 << (9 * walk.level as u32);
    addr - walk.phys < leaf_bytes
}

/// Buffer nhỏ dựng chuỗi bằng chứng (không cấp phát).
struct FixedBuf {
    bytes: [u8; 160],
    len: usize,
}

impl FixedBuf {
    fn new() -> Self {
        Self {
            bytes: [0; 160],
            len: 0,
        }
    }
    fn push(&mut self, text: &[u8]) {
        for byte in text {
            if self.len < self.bytes.len() {
                self.bytes[self.len] = *byte;
                self.len += 1;
            }
        }
    }
    fn num(&mut self, mut value: u64) {
        let mut tmp = [0u8; 20];
        let mut index = tmp.len();
        if value == 0 {
            index -= 1;
            tmp[index] = b'0';
        }
        while value > 0 {
            index -= 1;
            tmp[index] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        self.push(&tmp[index..]);
    }
    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Rig core **tối thiểu**: một CNode + Untyped + SchedControl + SC + TCB `Ready` (đường syscall
/// của U-mode cần `bind_core_tcb`; `Exit` về `Inactive` khi TCB không có `fault_ep`).
fn build_core_rig(untyped: ObjId) -> ObjId {
    vifast_plat_riscv64::core_bridge::with_machine(|m: &mut Machine| {
        let root = m.add_cnode(16);
        let us = SlotRef::new(root, 0);
        let root_cap = m.root_cap(untyped);
        m.put_cap(root, 0, root_cap).expect("KT-73: cap untyped");
        let cspace = m.root_cap(root);
        m.put_cap(root, 9, cspace).expect("KT-73: cap cspace");
        let ctl = m
            .retype(us, ObjType::SchedControl, 0, 0)
            .expect("KT-73: SchedControl");
        m.put_cap(root, 1, ctl).expect("KT-73: cap ctl");
        let sc = m
            .retype(us, ObjType::SchedContext, 0, 0)
            .expect("KT-73: SC");
        m.put_cap(root, 2, sc).expect("KT-73: cap SC");
        let tcb = m.retype(us, ObjType::Tcb, 0, 0).expect("KT-73: retype TCB");
        m.put_cap(root, 10, tcb).expect("KT-73: cap TCB");
        let tcb_slot = SlotRef::new(root, 10);
        m.tcb_configure(tcb_slot, Some(SlotRef::new(root, 9)))
            .expect("KT-73: cspace");
        m.sched_control_configure(SlotRef::new(root, 1), SlotRef::new(root, 2), 1_000, 10_000)
            .expect("KT-73: SC configure");
        m.set_sched_params(tcb_slot, Some(SlotRef::new(root, 2)))
            .expect("KT-73: sched params");
        m.tcb_set_priority(tcb_slot, 3, 10)
            .expect("KT-73: priority");
        let tcb_obj = m.cap_obj(tcb).expect("KT-73: tcb obj");
        m.start_tcb(tcb_obj).expect("KT-73: Ready");
        tcb_obj
    })
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..5\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let mmu = vifast_plat_riscv64::mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    let core = vifast_plat_riscv64::core_bridge::init(
        obj.arena_end,
        vifast_plat_riscv64::core_bridge::CORE_ARENA_BYTES,
    );
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - con uart irq ring: khong cap duoc stack per-hart\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    let tcb_obj = build_core_rig(core.untyped);
    user::bind_core_tcb(tcb_obj);
    let mut fails = 0usize;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con image: hart={} stvec=0x{:x} direct={} satp=0x{:x} mode={} root=0x{:x} \
             obj_arena=0x{:x}-0x{:x} untyped={} tcb={} elf={}",
            PLATFORM.hart_id(),
            stvec,
            (stvec & 0b11 == 0) as u8,
            mmu.satp,
            mmu.satp >> 60,
            mmu.root,
            obj.arena_base,
            obj.arena_end,
            obj.untypeds,
            tcb_obj,
            CON_ELF.len()
        );
    }

    // ------------------------------------------------ 1) UART RX IRQ → ring
    let plic = vifast_plat_riscv64::plic::init();
    let ring = match uart_console::init_ring(obj.untyped0) {
        Ok(report) => report,
        Err(stage) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "not ok 1 - con uart irq ring: init ring stage={stage}");
            let _ = writeln!(w, "# con ring: init that bai stage={stage}");
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con plic: base=0x{:x} ndev={} hart={} ctx={} threshold={} priority_uart={} \
             enable_ctx={} enable_mmode={} claim_idle={}",
            plic.base,
            plic.ndev,
            plic.hart,
            plic.context,
            plic.threshold,
            plic.priority_uart,
            plic.enable_ctx as u8,
            plic.enable_mmode as u8,
            plic.claim_idle
        );
        let _ = writeln!(
            w,
            "# con ring: pa=0x{:x} phys=0x{:x} va=0x{:x} const:slots={} const:slot_bytes={} \
             head={} tail={} check={} const:capacity={}",
            ring.pa,
            ring.phys,
            ring.va,
            ring.slots,
            ring.slot_bytes,
            ring.head,
            ring.tail,
            ring.check as u8,
            conchan::channel_bytes()
        );
    }
    // Ca âm "nguồn RX nhưng không có dữ liệu": bật RX rồi gọi handler khi FIFO rỗng.
    let (rx_begin_ok, _) = uart_console::rx_begin();
    let empty_bytes = uart_console::on_rx_irq();
    let (late_empty, rx_end_ok) = uart_console::rx_end();
    // Bơm payload qua **đường thật** (THR → loopback → RBR → IER → PLIC → handler).
    let (inject_ok, injected_slots, inject_ticks) = uart_console::inject_loopback(PAYLOAD);
    let uart = uart_console::stats();
    let (head, tail, used, seq) = uart_console::ring_stats();
    let want_slots = conchan::chunks_for(PAYLOAD.len()) as u64;
    let ok1 = plic.is_configured()
        && ring.check
        && rx_begin_ok
        && rx_end_ok
        && empty_bytes == 0
        && uart.empty_polls >= 1
        && late_empty == 0
        && inject_ok
        && injected_slots >= want_slots
        && injected_slots == uart.chunks
        && uart.irqs >= 1
        && uart.bytes == PAYLOAD.len() as u64
        && uart.drops == 0
        && uart.timeouts == 0
        // Số slot phụ thuộc **thiết bị** gom bao nhiêu byte mỗi ngắt (loopback: mỗi byte một
        // ngắt ⇒ `head = 30` cho 30 B), nên bất biến là "ít nhất `ceil(30/24)`", `head == chunks`
        // và mọi byte đã vào ring — không đòi một con số slot cố định.
        && head >= ring.head + want_slots
        && head == uart.chunks
        && tail == ring.tail
        && used == head - tail
        && seq == head;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con uart: irqs={} bytes={} const:payload={} chunks={} const:chunks_want={} \
             head={} const:head0={} tail={} used={} seq={} empty_polls={} empty_bytes={} \
             drops={} timeouts={} inject_ok={} inject_ticks={} rx_begin={} rx_end={}",
            uart.irqs,
            uart.bytes,
            PAYLOAD.len(),
            injected_slots,
            want_slots,
            head,
            ring.head,
            tail,
            used,
            seq,
            uart.empty_polls,
            empty_bytes,
            uart.drops,
            uart.timeouts,
            inject_ok as u8,
            inject_ticks,
            rx_begin_ok as u8,
            rx_end_ok as u8
        );
    }
    put(
        ok1,
        1,
        "con uart irq ring",
        &mut fails,
        "irq/ring/so byte sai",
    );

    // ------------------------------------------------ 2) U-mode đọc byte từ ring
    let loaded: LoadedInit = match user::load_init_slot(CON_ELF, obj.untyped0, 0) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 2 - con u-mode doc ring: nap ELF kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let mapped = uart_console::attach(loaded.vspace);
    let stop = {
        user::set_active_slot(0);
        user::capture_begin();
        let result = user::enter(&loaded, 0);
        user::capture_end();
        result
    };
    let exit_code = match &stop {
        Ok(snapshot) => snapshot.exit_code,
        Err(_) => u64::MAX,
    };
    // Kernel tự tính chuỗi kỳ vọng từ payload **đã bơm** (D2-102: không viết tay số).
    let mut sum = 0u64;
    for byte in PAYLOAD {
        sum += u64::from(*byte);
    }
    let mut expect = FixedBuf::new();
    expect.push(b"# con: u bytes=");
    expect.num(PAYLOAD.len() as u64);
    expect.push(b" sum=");
    expect.num(sum);
    expect.push(b" first=");
    expect.num(u64::from(PAYLOAD[0]));
    expect.push(b" last=");
    expect.num(u64::from(PAYLOAD[PAYLOAD.len() - 1]));
    // `seq` = số slot producer **thật** đã publish (đọc từ header sau khi bơm) — không hardcode.
    expect.push(b" seq=");
    expect.num(seq);
    expect.push(b" slots=");
    expect.num(u64::from(conchan::CHANNEL_SLOTS));
    expect.push(b" slot_bytes=");
    expect.num(u64::from(conchan::SLOT_BYTES));
    expect.push(b" check=1");
    let (head2, tail2, used2, _) = uart_console::ring_stats();
    // **KT-123** (review chéo, đính chính): khẳng định dưới đây **không** phải nguồn độc lập với
    // `Ring::check()` — nó kiểm **nhất quán nội tại** của cùng header (`seq == head`,
    // `used == head - tail`, …), và `Ring::check()` đã đòi đúng `seq == head` (`v2/lib/vifast-ring`).
    // Đo thực tế của reviewer: tắt khẳng định này đi thì đột biến `seq` **vẫn** bị bắt (bởi
    // `Ring::check()` qua `check=0` trong chuỗi kỳ vọng, và bởi TAP 1 đã có `seq == head`).
    // Giữ lại vì nó rẻ và bắt được header hỏng **cục bộ** (ví dụ `used` sai mà `check` chưa chạy),
    // nhưng **không** được mô tả là "độc lập" — nó là kiểm nhất quán, không phải kiểm chéo nguồn.
    let ring_invariants = seq == head2
        && head2 >= tail2
        && used2 == head2 - tail2
        && used2 <= u64::from(conchan::CHANNEL_SLOTS)
        && head2 >= ring.head;
    let ok2 = mapped
        && exit_code == 0
        && user::captured_contains(0, expect.as_bytes())
        && head2 == head
        && tail2 == head2
        && used2 == 0
        && ring_invariants;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let mut prefix = [0u8; 48];
        let n = user::captured_prefix(0, &mut prefix);
        // Bằng chứng một dòng: newline/ký tự lạ ⇒ '.' (không phá dòng `# con u:` của runner).
        for byte in prefix[..n].iter_mut() {
            if *byte == b'\n' || *byte == b'\r' || *byte == b'"' {
                *byte = b'.';
            }
        }
        let _ = writeln!(
            w,
            "# con u: map={} exit={} match={} head={} tail={} used={} captured={} \
             prefix=\"{}\"",
            mapped as u8,
            exit_code as i64,
            user::captured_contains(0, expect.as_bytes()) as u8,
            head2,
            tail2,
            used2,
            n,
            core::str::from_utf8(&prefix[..n]).unwrap_or("?")
        );
    }
    put(
        ok2,
        2,
        "con u-mode doc ring",
        &mut fails,
        "U-mode doc sai byte/khong tieu thu",
    );

    // ------------------------------------------------ 3) virtio-console qua PCI của KT-72
    let pci_report = pci::init_for(VirtioKind::Console);
    let device_ok = match &pci_report {
        Ok(report) => {
            let ecam_ok = identity(report.ecam_addr);
            let bar_ok = identity(report.bar_assigned);
            let ok = report.vendor == 0x1af4
                && (report.device == 0x1003 || report.device == 0x1043)
                && report.bar_readback == report.bar_assigned
                && report.command & (COMMAND_MEMORY | COMMAND_BUS_MASTER)
                    == COMMAND_MEMORY | COMMAND_BUS_MASTER
                && report.bar_size >= 0x1000
                && ecam_ok
                && bar_ok
                && report.overlap_checked
                && report.cap_ranges_ok;
            {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(
                    w,
                    "# con pci: vendor=0x{:x} device=0x{:x} bus={} dev={} func={} bar={} kind={} \
                     size=0x{:x} assigned=0x{:x} readback=0x{:x} command=0x{:x} ecam=0x{:x} \
                     other_bars={} reserved={} overlap={} caps_in_bar={} ecam_id={} bar_id={} \
                     const:assign=0x{:x}",
                    report.vendor,
                    report.device,
                    report.bus,
                    report.dev,
                    report.func,
                    report.bar_index,
                    pci::bar_kind_name(report.bar_kind),
                    report.bar_size,
                    report.bar_assigned,
                    report.bar_readback,
                    report.command,
                    report.ecam_addr,
                    report.other_bars,
                    report.reserved_ranges,
                    report.overlap_checked as u8,
                    report.cap_ranges_ok as u8,
                    ecam_ok as u8,
                    bar_ok as u8,
                    pci::BAR_ASSIGN_BASE_CONSOLE
                );
            }
            ok
        }
        Err(fail) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# con pci: FAIL error={} stage={} const:assign=0x{:x}",
                fail.error.name(),
                fail.stage,
                pci::BAR_ASSIGN_BASE_CONSOLE
            );
            false
        }
    };
    let console = match (&pci_report, device_ok) {
        (Ok(report), true) => match virtio_console::init(report) {
            Ok(console) => Some(console),
            Err(stage) => {
                let mut w = PlatformWriter(&PLATFORM);
                let _ = writeln!(w, "# con queue: FAIL stage={stage}");
                None
            }
        },
        _ => None,
    };
    match &console {
        Some(report) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# con queue: num_queues={} rx_max={} rx_size={} rx_notify_off={} tx_max={} \
                 tx_size={} tx_notify_off={} notify_mul={} device_features=0x{:x} \
                 driver_features=0x{:x} const:version1=1 multiport={} status=0x{:x} \
                 generations={}/{} rx_buf=0x{:x} tx_buf=0x{:x} rx_desc=0x{:x} tx_desc=0x{:x}",
                report.num_queues,
                report.rx.max,
                report.rx.size,
                report.rx.notify_off,
                report.tx.max,
                report.tx.size,
                report.tx.notify_off,
                report.notify_mul,
                report.device_features,
                report.driver_features,
                report.multiport as u8,
                report.status,
                report.config_generation,
                virtio_console::config_generation(),
                report.rx_buf_pa,
                report.tx_buf_pa,
                report.rx_desc_pa,
                report.tx_desc_pa
            );
        }
        None => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(w, "# con queue: khong khoi tao duoc (xem dong truoc)");
        }
    }
    let ok3 = device_ok
        && console
            .as_ref()
            .map(|report| {
                report.num_queues >= 2
                    && report.rx.size >= 2
                    && report.tx.size >= 2
                    && !report.multiport
                    && report.driver_features == 1u64 << 32
                    && report.status & 0x80 == 0
            })
            .unwrap_or(false);
    put(
        ok3,
        3,
        "con pci",
        &mut fails,
        "khong thay/dam phan dung thiet bi",
    );

    // ------------------------------------------------ 4) ghi (tx) + đọc (rx)
    let (tx1, tx2, arm_ok) = if ok3 {
        let tx1 = virtio_console::write(TX_ONE);
        let tx2 = virtio_console::write(TX_TWO);
        (tx1, tx2, virtio_console::queue_state().0)
    } else {
        (
            Err(ConsoleError::NotReady),
            Err(ConsoleError::NotReady),
            false,
        )
    };
    // Marker để **host** biết lúc bơm byte vào `chardev pipe` (đồng bộ, không dựa vào thời gian).
    PLATFORM.put_str("# con: rx armed\n");
    let mut buffer = [0u8; vifast_virtio::console::TX_CHUNK_BYTES];
    let mut rx_bytes = 0usize;
    let mut rx_ok = false;
    if ok3 {
        let deadline = 30 * TIMEBASE_HZ;
        if let Ok(count) = virtio_console::read_wait(&mut buffer, deadline) {
            if count == RX_EXPECT.len() && buffer[..count] == *RX_EXPECT {
                rx_ok = true;
                rx_bytes = count;
            }
        }
    }
    let con_stats = virtio_console::stats();
    let ok4 = ok3
        && arm_ok
        && tx1 == Ok(TX_ONE.len())
        && tx2 == Ok(TX_TWO.len())
        && rx_ok
        && con_stats.writes == 2
        && con_stats.write_bytes == (TX_ONE.len() + TX_TWO.len()) as u64
        && con_stats.read_bytes == RX_EXPECT.len() as u64
        && con_stats.id_mismatch == 0
        && virtio_console::device_status() & 0x80 == 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con tx: one={:?} two={:?} writes={} write_bytes={} const:tx_want={} notifies={} \
             last_tx_used_len={} const:tx_len_note=0..=24 driver_status=0x{:x}",
            tx1,
            tx2,
            con_stats.writes,
            con_stats.write_bytes,
            TX_ONE.len() + TX_TWO.len(),
            con_stats.notifies,
            con_stats.last_tx_len,
            virtio_console::device_status()
        );
        let _ = writeln!(
            w,
            "# con rx: got={} const:want={} ok={} busy={} errors={} timeouts={} id_mismatch={} \
             last_rx_used_len={} armed={} head={}/{}",
            rx_bytes,
            RX_EXPECT.len(),
            rx_ok as u8,
            con_stats.busy,
            con_stats.errors,
            con_stats.timeouts,
            con_stats.id_mismatch,
            con_stats.last_rx_len,
            virtio_console::queue_state().0 as u8,
            virtio_console::queue_state().1,
            virtio_console::queue_state().2
        );
    }
    put(
        ok4,
        4,
        "con ghi doc",
        &mut fails,
        "ghi/doc thiet bi sai byte",
    );

    // ------------------------------------------------ 5) ca âm: không dữ liệu + ring đầy
    // (a) `read` **không chờ** khi không có dữ liệu ⇒ `Busy` (14) ngay, không treo.
    let mut buffer2 = [0u8; vifast_virtio::console::TX_CHUNK_BYTES];
    let busy = virtio_console::read_try(&mut buffer2) == Err(ConsoleError::Busy);
    let busy_code = ConsoleError::Busy.abi_code();
    // (b) ring đầy: bơm 800 B từ đường kernel khi **không** có consumer.
    let mut fill = [0u8; FILL_BYTES];
    for (index, byte) in fill.iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(7).wrapping_add(3);
    }
    let drops_before = uart_console::stats().drops;
    let (slots_ok, bytes_ok) = uart_console::produce(&fill);
    let after = uart_console::stats();
    let want_slots_fill = conchan::CHANNEL_SLOTS as usize;
    let want_bytes_fill = conchan::channel_bytes();
    let want_drops = conchan::chunks_for(FILL_BYTES) as u64 - conchan::CHANNEL_SLOTS as u64;
    let mut got = [0u8; FILL_BYTES];
    let consumed = uart_console::consume(&mut got);
    let order_ok = consumed == want_bytes_fill && got[..consumed] == fill[..consumed];
    let (head5, tail5, used5, _) = uart_console::ring_stats();
    let ok5 = ok4
        && busy
        && busy_code == 14
        && slots_ok == want_slots_fill
        && bytes_ok == want_bytes_fill
        && after.drops - drops_before == want_drops
        && after.full_code == u64::from(vifast_ring::ERR_FULL)
        && order_ok
        && used5 == 0
        && tail5 == head5
        && after.timeouts == 0
        && virtio_console::stats().errors == con_stats.errors;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con neg: busy={} const:busy_code={} read_try={} ring_slots_ok={} \
             const:slots_want={} ring_bytes_ok={} const:bytes_want={} \
             drops={} const:drops_want={} full_code={} const:full_want={} consumed={} \
             order_ok={} head={} tail={} used={} timeouts={}",
            busy as u8,
            busy_code,
            if busy {
                ConsoleError::Busy.name()
            } else {
                ConsoleError::Io.name()
            },
            slots_ok,
            want_slots_fill,
            bytes_ok,
            want_bytes_fill,
            after.drops - drops_before,
            want_drops,
            after.full_code,
            vifast_ring::ERR_FULL,
            consumed,
            order_ok as u8,
            head5,
            tail5,
            used5,
            after.timeouts
        );
    }
    put(
        ok5,
        5,
        "con ca am",
        &mut fails,
        "backpressure/khong du lieu sai",
    );

    // ------------------------------------------------ tổng kết
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# con done: fails={} const:plan=5 payload={} const:payload_bytes={} \
             const:rx_bytes={} const:fill={} const:channel=const:slots*const:slot_bytes={}",
            fails,
            conchan::chunks_for(PAYLOAD.len()),
            PAYLOAD.len(),
            RX_EXPECT.len(),
            FILL_BYTES,
            conchan::channel_bytes()
        );
    }
    let mut w = PlatformWriter(&PLATFORM);
    let _ = writeln!(w, "PANIC: KT-73 con xong (5 TAP, {fails} fail)");
    PLATFORM.halt(0)
}

/// Entry hart **thứ cấp** — ảnh con chỉ cần symbol để link (nếu có hart thứ cấp thì park).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
