# U1a / UEFI — hợp đồng `.vidl`, kiểm chéo ViFastOs ↔ ViScript, lộ trình firmware

Trạng thái: **KT-347** (PoC `U1a`, D2-252 Đợt 3C). Tệp này là **một** tài liệu thiết kế; mã của
PoC nằm ở `v2/user/rt/` (crate `vifast-rt`), `v2/user/runtime/lib.rs` (`mod heap`), chương trình
`v2/user/programs/u1a.rs` ⇒ `u1a.elf`. Việc hoàn thiện là **`KT-362`** (`U1a-Full`, Chu kỳ 3) và
**`KT-364`** (`U1b + UEFI`).

Nhãn số liệu theo QD §2.2: `[TK]` thiết kế, `[GT]` giả thuyết, `[ƯL]` ước lượng cần đo. Tài liệu
này **không** chứa số đo viết tay (QD §9 Q12); mọi số đo phải do `./v2/bin/v2 nightly` hoặc log
cổng sinh ra.

---

## 1. Phạm vi và hiện trạng (nói thẳng)

| Hạng mục | KT-347 (PoC) | KT-362 (`U1a-Full`) |
|---|---|---|
| `vifast-rt` | `no_std + alloc` trên **arena tĩnh** 8 KiB [TK], `println!`, `main`, panic handler, OOM fail-closed có tên | allocator trên **`Untyped`**/`Frame` thật (khi U-mode có đường cấp phát), `Tcb.Exec(path)` trong shell |
| ELF U-mode | `u1a.elf` build bằng stable `1.100.0` (ghim `v2/rust-toolchain.toml`), chạy trên ảnh sản phẩm riscv64 | cùng target, thêm chương trình `ViShell` port từ ViScript |
| `.vidl` | **thiết kế** hợp đồng (tài liệu này); stub đã có từ KT-69 cho `ringchan` | bộ `proc`/`fs`/`net`/`term`/`socket.vidl` + stub sinh + client typed trong `vifast-rt` |
| exec | **đọc ELF từ FS** (đĩa seed bằng `vifast-fsutil`), không còn `include_bytes!` | `Tcb.Exec(path)` (ABI v1.2, `KT-360`) — chờ `APPROVED` của A0 |
| UEFI | — | `KT-364`: OVMF/AAVMF + DTB/ACPI |

Ràng buộc bất di bất dịch (không nới ở bất kỳ mục nào dưới đây): `spec/abi.toml` /
`spec/kernel.toml` **khoá** (D2-04/D2-252); 16 syscall và `MessageInfo` 35 bit **đóng băng**
(D2-15); `.data+.bss` ảnh sản phẩm riscv64 ≤ **262.144 B** [TK]; `size_of::<Machine>()` =
**184.792 B** [TK] không đổi; mỗi tệp nguồn ≤ **1.500** dòng (R1); **không** GitHub Actions
(D2-241).

---

## 2. Hợp đồng `.vidl` giữa userland và dịch vụ

### 2.1 Vì sao `.vidl` chứ không mở `spec/`

`spec/abi.toml` là nguồn sự thật của **ABI lõi** (syscall, object, mã lỗi, chữ ký method kernel).
Nó **không** chứa — và không nên chứa — giao thức của từng dịch vụ (FS, mạng, tiến trình…).
`KT-69` đã chốt khuôn: một interface/một tệp `.vidl`, method id sinh **tuần tự từ tệp** (0..N−1),
generator `./v2/bin/vifast-idl gen` sinh hằng + `encode/decode`; `--check` bắt lệch trong `v2
check` (D2-99, D2-104). KT-362 **kế thừa nguyên khuôn này** — không thêm syscall, không đổi
`MessageInfo`, không đụng `spec/`.

Cú pháp (đang dùng, `v2/lib/vifast-idl/idl/ring.vidl`):

```
interface <tên> version <u32>
method <tên>
  param <tên> <u32|u64>      # tối đa 4 (MR0..MR3)
  cap <tên> <frame|buffer>   # tối đa MỘT cap trên mỗi method
  returns <u32|u64|unit>
```

### 2.2 Bộ interface chuẩn của `U1a-Full` [TK]

Năm interface dưới đây phủ đúng các đường đang chạy trong ảnh sản phẩm hôm nay (label thật ghi
trong ngoặc) — mục tiêu là **thay hằng số rời rạc bằng stub sinh**, không đổi ngữ nghĩa trên dây:

| Tệp | Interface | Method (id tuần tự) | Nguồn hiện tại |
|---|---|---|---|
| `proc.vidl` | `proc` v1 | `spawn(path, caps) -> pid`, `wait(pid) -> code`, `kill(pid, how) -> u32`, `list(cursor) -> u32` | `EP_PROC`/`PS_REQ`, giao thức procsrv (`proc_abi.rs`) |
| `fs.vidl` | `fs` v1 | `open(path) -> u32`, `read(off, len) -> u32` + buffer, `write(off, len) -> u32`, `stat() -> (kind,size,ino)`, `readdir(index) -> u32`, `sync()`, `statfs() -> (used,total,block)` | `EP_FS` + `FS_REQ`/`OP_*` (`proc_fs.rs`), `fsstat.vidl` 4C của KT-345b |
| `net.vidl` | `net` v1 | `ping(ip) -> (rtt_ms, err)`, `ifconfig() -> u32` + buffer | `EP_NET`/`NET_REQ` (`proc_net.rs`), nợ `KT-77a` (`ifconfig`) |
| `term.vidl` | `term` v1 | `put(bytes) -> unit`, `get(bytes) -> u32` | `DebugPutStr` (12) + ring UART (`conchan`) |
| `socket.vidl` | `socket` v1 | `open(domain, kind) -> u32`, `send(fd, len) -> u32`, `recv(fd, len) -> u32`, `close(fd)` | **mới** — chỉ đặc tả, cài ở Chu kỳ 3+; `net.vidl` phục vụ trước |

Quy tắc hợp đồng (áp cho **mọi** tệp `.vidl` mới):

1. **Version** tăng khi đổi bố cục tham số; method id **không** tái sử dụng sau khi bỏ.
2. **Tối đa 4 tham số vô hướng** (MR0..MR3) và **tối đa 1 cap**/method — đúng `MessageInfo` 35
   bit hiện có; dữ liệu dài đi qua **buffer/Frame** hoặc nhiều lượt (như `OP_CAT` 24 B/lượt).
3. **Reply bit** (bit 19 của label 20 bit) do generator đặt; dịch vụ trả lời trên cap `Reply`
   của người gọi, không tự đặt label khác.
4. **Lỗi có tên**: mã lỗi dùng bảng `vifast-abi` (không bịa số mới); `decode` kiểm **arity
   chính xác** và tràn kiểu ⇒ `None` (D2-104), không cắt ngầm.
5. **Sinh tự động**: hằng/stub **chỉ** sinh từ `.vidl`; sửa tay stub ⇒ `./v2/bin/vifast-idl
   --check` đỏ (đã có fixture âm ở `v2/test/gen/run_idl_fixtures.sh`).
6. **Không `unsafe`** trong stub sinh ra (kiểm bởi R5) và **không** phụ thuộc crate ngoài.

### 2.3 Phía `vifast-rt`

`vifast-rt` là **client** của các interface trên. KT-347 chỉ đặt nền:

- `entry(main)` — dựng IPC buffer rồi chạy `main` (cùng hợp đồng `_start` của `hello.rs`);
- `println!`/`print!` — `core::fmt` → syscall `DebugPutStr` (12), **không** cấp phát;
- `alloc` — `#[global_allocator]` arena tĩnh (PoC) — API `heap_stats() -> (used, cap)`;
- panic handler + OOM **fail-closed có tên** (`# u1a: panic …`/`# u1a: alloc fail …` rồi `Exit`
  khác 0) — không treo, không bỏ qua.

KT-362 thêm `rt::idl` (module sinh từ stub): `fs::Client`, `net::Client`, … là **wrapper an
toàn** trên `call/nb_send/nb_recv` của shim; chữ ký lấy từ stub sinh, **không** viết tay số
method. Điều kiện vào: (a) bộ `.vidl` §2.2 có fixture dương/âm; (b) `v2 check` xanh với gen-check;
(c) không tăng `.data+.bss` quá trần; (d) T2 trên máy đo.

---

## 3. Kịch bản kiểm chéo cục bộ ViFastOs ↔ ViScript (không CI)

**Ràng buộc D2-241**: không `.github/workflows/**`, không dịch vụ CI ngoài. Kiểm chéo chạy **trên
máy đo**, bằng script + bộ hẹn giờ cục bộ. Lý do đã ghi ở `AGENTS.md` §4.11: cổng có bước QEMU
nhiều phút và ngưỡng thời gian chỉ đúng trên máy đo; CI ngoài sinh bằng chứng **không hợp lệ** mà
vẫn xanh.

### 3.1 Script (KT-363 sẽ thêm; đây là hợp đồng)

`v2/test/crosscheck/viscript-vifast.sh` [TK] — một lượt kiểm chéo, **fail-closed**, không mạng:

1. **Chốt đầu vào**: `git -C <ViScript> rev-parse HEAD`, `git -C <ViFastOs> rev-parse HEAD`,
   `algo`/`inputs` từ `v2/test/boot/build-rev.sh` (dòng `# build:` của ảnh), hash của mọi tệp
   `.vidl` hai phía. Thiếu bất kỳ khoá nào ⇒ **TỪ CHỐI ĐO** (exit 3), không PASS mặc định.
2. **Đối chiếu hợp đồng**: so **tên + version + method id + arity + cap** của từng interface giữa
   `.vidl` của ViFastOs và stub/bản sao trong ViScript. Lệch ⇒ đỏ kèm tên interface/method.
3. **Build `ViShell`**: biên dịch `ViShell` (ViScript) cho `riscv64gc-unknown-none-elf` bằng
   `vifast-rt` (cùng toolchain ghim `1.100.0`), xuất ELF; kiểm ELF64 (`e_machine=243`,
   entry trong dải U-mode) trước khi chạy.
4. **Seed + boot một lượt**: `vifast-fsutil mkfs` + `put-file` ELF `ViShell` (cùng khuôn KT-347
   đang dùng cho `/hello.elf` + `/rt.elf`), boot ảnh sản phẩm riscv64 với `-drive` + `-netdev`,
   bơm kịch bản qua UART, chờ **evidence của khách** (không `sleep` mù).
5. **So vàng (golden)**: stdout của `ViShell` so với `v2/test/crosscheck/golden/<ca>.txt`; mỗi ca
   là một dòng lệnh + dòng evidence. Sai một dòng ⇒ đỏ; không retry (D2-07).
6. **Kết quả**: in một dòng tổng kết `# crosscheck: viscript=<rev> vifast=<rev> vidl=<n>
   ca=<pass>/<all>`; exit 0 chỉ khi mọi ca xanh.

Không dùng mạng, không tải gói: mọi thứ từ hai worktree cục bộ. Bằng chứng lưu **ngoài** repo
(`${TMPDIR:-/tmp}/v2-crosscheck-$$`), không sinh tệp `??` (bài học D2-249).

### 3.2 Hẹn giờ trên máy

- **macOS**: `launchd` (`~/Library/LaunchAgents/org.vifast.crosscheck.plist`, `StartCalendarInterval`
  hằng ngày) **hoặc** `cron`; **Linux**: systemd timer. Nội dung chỉ là gọi script §3.1.
- **Không chạy song song T2/soak**: script đọc `v2/tools/measure-lock.sh`; khoá đang giữ ⇒ in
  `TỪ CHỐI ĐO` và exit 2 (đúng D2-245/D2-248), lần hẹn sau chạy lại.
- Kết quả ghi `v2/agent/reports/crosscheck-<YYYYMMDD>.md` (một tệp/lượt) + **một** dòng trong
  journal của task đang mở; lịch sử giữ tối đa 30 tệp (xoá cũ nhất).

### 3.3 Điều kiện vào của kiểm chéo

(a) `ViShell` build được bằng `vifast-rt` (KT-362 xong `rt::idl`); (b) `.vidl` hai phía cùng
version; (c) máy đo rảnh (`qemu_foreign=0`); (d) golden có ít nhất một ca **âm** (lệnh sai ⇒ mã
lỗi có tên, không PANIC). Thiếu một điều kiện ⇒ **chưa** mở, không đánh dấu "đã kiểm chéo".

---

## 4. Lộ trình UEFI (`KT-364`)

### 4.1 Hiện trạng boot (để biết phải thay gì)

- **riscv64**: OpenSBI `fw_dynamic` + `-kernel` (QEMU `virt`), kernel nhận mô tả từ DTB do
  OpenSBI chuyển; UART 16550 ở địa chỉ cố định. Không có bảng ACPI.
- **aarch64**: QEMU `virt` + `-kernel` (DTB do QEMU cấp, `x0` trỏ DTB), GIC + generic timer.
- **x86_64**: boot trực tiếp kiểu `-kernel`/multiboot-like hiện có (xem
  `v2/docs/k5-x86_64-boot.md`); không đi qua firmware UEFI.

`KT-364` **không** thay các đường trên; nó thêm **một** đường vào thứ ba, chọn bằng cấu hình
runner (`--firmware uefi`), để ảnh cũ giữ nguyên từng byte (D2-109/D2-111).

### 4.2 Đường UEFI [TK]

| Thành phần | x86_64 | aarch64 |
|---|---|---|
| Firmware QEMU | OVMF (`edk2-x86_64-code.fd` + `vars`) | AAVMF (`edk2-aarch64-code.fd`) |
| Target Rust | `x86_64-unknown-uefi` (stable 1.100.0) | `aarch64-unknown-uefi` |
| Ứng dụng | `boot/uefi-x86_64` — PE/COFF `efi_main` | `boot/uefi-aarch64` — PE/COFF `efi_main` |
| Bảng bộ nhớ | `GetMemoryMap` → **E820-tương đương** (type/pages/attr) | `GetMemoryMap` (cùng khuôn) |
| Bảng cấu hình | `RSDP` từ EFI config table → `FADT`/`MADT` (nếu có) | `RSDP` + DTB (`EFI_DTB_TABLE_GUID` hoặc `/chosen`) |
| Console | `EFI_SIMPLE_TEXT_OUTPUT` trước `ExitBootServices`, rồi UART/GOP tuỳ chọn | cùng khuôn |
| Bàn giao kernel | `ExitBootServices` → dựng `BootInfo` (**một** struct versioned) → nhảy `e_entry` | cùng khuôn |

`BootInfo` là **một** nguồn sự thật cho kernel khi vào từ firmware: `magic/version`, con trỏ bảng
bộ nhớ, con trỏ `RSDP`, con trỏ DTB (aarch64), địa chỉ UART, `highest_phys`. Kernel hiện đọc tham
số trực tiếp từ thanh ghi/DTB; KT-364 thêm nhánh **chỉ đọc `BootInfo`** khi magic khớp, không
sửa đường cũ.

### 4.3 DTB / ACPI: cái gì bắt buộc, cái gì tuỳ chọn

- **Bắt buộc cho cả hai**: bảng bộ nhớ (vùng **usable** phải loại trừ firmware/runtime
  services/ACPI reclaim) và **nguồn ngắt timer** (aarch64: generic timer; x86_64: HPET/LAPIC
  timer qua `MADT`/`FADT`).
- **aarch64**: ưu tiên **DTB** (QEMU `virt` cấp); ACPI chỉ dùng khi DTB vắng. Nếu cả hai có, chọn
  **một** theo thứ tự cố định và ghi vào log (`# boot: fw=uefi dtb=1 acpi=0`) — không trộn.
- **x86_64**: **ACPI** là nguồn chính (`RSDP` → `XSDT` → `MADT`/`FADT`); E820 chỉ để so chéo khi
  firmware cấp. Không đọc `INT 15h` sau `ExitBootServices`.
- **Cấm** dùng lại bất kỳ vùng `EFI_LOADER_*`/`EFI_BOOT_SERVICES_*` sau `ExitBootServices`; bảng
  bộ nhớ phải **chụp lại ngay trước** khi thoát (map có thể đổi sau mỗi `AllocatePages`).

### 4.4 Rủi ro và điều kiện vào

| Rủi ro | Ảnh hưởng | Giảm thiểu |
|---|---|---|
| Bảng bộ nhớ cũ sau `ExitBootServices` | ghi đè firmware/ACPI ⇒ treo khó tái hiện | chụp lại **ngay trước** `ExitBootServices`; kiểm `desc_size`/`desc_version`; fail-closed có tên |
| Runtime services sau khi thoát | #GP/PANIC | mọi `GetTime`/`ResetSystem` chỉ **trước** `ExitBootServices`; sau đó tự cài |
| ACPI hỏng/không có `MADT` | mất timer/SMP | fallback `FADT` + `HPET`; thiếu nữa ⇒ boot 1 CPU, in lý do, không PANIC |
| DTB và ACPI mâu thuẫn (aarch64) | IRQ/GIC sai | một thứ tự ưu tiên cố định + log nguồn |
| `PE/COFF` + relocation | ảnh không nạp được | link `--subsystem efi`, `-C relocation-model=pic`; kiểm ELF/PE trước khi boot |
| Khác biệt firmware thật vs QEMU | xanh giả | bảng kiểm mang lên HW1 (`KT-365`), **không** tuyên bố "UEFI xong" chỉ từ QEMU |
| Ngân sách ảnh | vượt trần `.data+.bss` | boot stub **riêng** (ảnh `boot/uefi-*`), kernel giữ trần hiến định |

**Điều kiện vào `KT-364`** (đủ cả mới mở): (a) `KT-357` + `KT-362` xong; (b) `BootInfo` versioned
có test host (đỏ-trước) cho parse bảng bộ nhớ/`RSDP`; (c) runner `--firmware uefi` fail-closed khi
thiếu firmware (TỪ CHỐI ĐO, không PASS); (d) TAP ba arch **không đổi** (đường cũ vẫn là mặc định);
(e) T2 trên máy đo với `qemu_foreign=0`.

---

## 5. Việc còn lại sau KT-347 (nói thẳng)

1. **Allocator trên `Untyped`**: PoC dùng arena tĩnh 8 KiB [TK] vì U-mode **chưa** có cap
   `Untyped`/`Frame`. Khi `Tcb.Exec`/`Frame.Map` mở cho U-mode (KT-362/KT-360), thay lõi bump —
   API `vifast-rt` giữ nguyên.
2. **`Tcb.Exec(path)` trong shell**: KT-347 đóng phần *đọc ELF từ FS* ở đường kernel-mediated;
   method ABI v1.2 vẫn chờ `KT-360` + `APPROVED` của A0 (không tự mở `spec/`).
3. **Bộ `.vidl` + stub + `rt::idl`**: mới ở mức hợp đồng (§2.2); KT-362 cài và kiểm.
4. **Kiểm chéo ViScript**: script + hẹn giờ là đặc tả (§3); KT-363 cài.
5. **UEFI**: đặc tả + rủi ro (§4); KT-364 cài. Chưa có dòng nào trong bảng "đã chạy UEFI".
