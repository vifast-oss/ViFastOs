# K5' — Bản nháp `[calling.x86_64]` (KT-186 / D2-163)

**Task**: KT-186 — việc một của D2-163 (nháp `[calling.x86_64]` + M2).
**Trạng thái**: **NHÁP** — **KHÔNG** phải `spec/`, **KHÔNG** được duyệt, **KHÔNG** đóng băng.
**Nền**: `9921f606`, nhánh `night2/L27-x86m2`, worktree `/private/tmp/night2-L27-x86m2`.
**Ngày**: 2026-11-19.
**Phạm vi**: chỉ quy ước gọi hàm + syscall x86_64. **Không** mở `spec/`, **không** sửa
`spec/abi.toml`, **không** tuyên bố ABI x86_64 đã chốt.

## 0. Vì sao có tệp này

D2-163 chốt: `spec/abi.toml` `[calling.x86_64]` **CHƯA mở**; Agent 1 soạn **bản nháp ngoài
`spec/`** (trong `v2/agent/reports/`), gắn task có tên, mở **một lần** theo khuôn **KT-05** tại
bước **M7**. Tệp này là **đầu vào cho Agent 0** ở M7 — không phải ABI, không phải nguồn sự thật.
Mọi khẳng định dưới đây dẫn nguồn ở §1; chỗ nào là **đề xuất của ViFastOs** thì ghi rõ `[TK]`.

## 1. Nguồn

| Nguồn | Vị trí | Dùng cho |
|---|---|---|
| System V AMD64 ABI (x86-64 psABI) | §3.2 "Function Calling Sequence": §3.2.1 "Registers", §3.2.2 "The Stack Frame", §3.2.3 "Parameter Passing" (gồm "Returning of Values"), §3.2.4 "Variable Argument Lists" | Thanh ghi tham số/trả về/callee-saved, căn ngăn xếp, red zone, `DF`, `MXCSR`, `AL` |
| Intel SDM Vol 3A | mục "Control Registers" (`CR0`/`CR4`), "Extended Feature Enable Register (IA32_EFER)", "4-Level Paging and 5-Level Paging", "Page-Attribute Table (PAT)", "Initializing IA-32e Mode" (§9.8.5), "Exception and Interrupt Reference" | Bit `CR0/CR4/EFER`, descriptor `P/RW/US/PWT/PCD/A/D/PS/PAT/G/NX`, PAT, long mode, vector |
| Intel SDM Vol 2B | mục "SYSCALL—Fast System Call", "SYSRET—Return From Fast System Call", "INT n/INTO/INT3/INT1—Call to Interrupt Procedure", "SWAPGS—Swap GS Base Register" | `syscall`/`sysret` clobber `RCX`/`R11`, `int 0x80`, `swapgs` |
| Intel SDM Vol bốn | chương hai "Model-Specific Registers (MSRs)" | Địa chỉ `IA32_EFER`, `IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK`, `IA32_PAT`, `IA32_GS_BASE`, `IA32_KERNEL_GS_BASE` |
| AMD64 APM Vol hai | mục "User/Supervisor (U/S) Bit" | Bit `U/S` của PML4E kiểm soát toàn bộ vùng nó phủ ⇒ bảng trung gian phải `US = 0x1` |
| `spec/abi.toml` | `[calling.riscv64]` (đóng băng), `[message]`, `[message_info]`, `[debug_info]`, `[frozen]` | Đối chiếu — xem §3 |

**Không** dùng số đo nào trong tệp này: mọi con số là **hằng đặc tả** hoặc **đề xuất `[TK]`**.
Bản nháp **không** thay thế việc đọc đặc tả gốc; khi mở `spec/` ở M7, Agent 0 phải đối chiếu lại
từng dòng với bản SDM/psABI đang hành.

## 2. Quy ước gọi hàm x86_64 (System V AMD64 ABI)

Đây là quy ước **gọi hàm** (C/user ↔ kernel C), **không** phải quy ước syscall. Kernel và user
Rust/C dùng chung quy ước này; stub syscall là một hàm bình thường ở biên.

| Hạng mục | Giá trị | Nguồn |
|---|---|---|
| Tham số nguyên | `RDI`, `RSI`, `RDX`, `RCX`, `R8`, `R9` (đúng thứ tự) | psABI §3.2.3 |
| Trả về nguyên | `RAX`, rồi `RDX` | psABI §3.2.3 "Returning of Values" |
| Callee-saved | `RBX`, `RBP`, `R12`–`R15` (và `RSP`) | psABI §3.2.1 |
| Caller-saved | `RAX`, `RCX`, `RDX`, `RSI`, `RDI`, `R8`–`R11` | psABI §3.2.1 |
| Căn ngăn xếp | `(RSP + 0x8)` là bội của mười sáu khi vào hàm; 16-byte aligned ngay trước `call` | psABI §3.2.2 |
| Red zone | 128-byte dưới `RSP`; **signal/interrupt handler không được ghi** | psABI §3.2.2 |
| `DF` | phải bằng 0x0 khi vào và khi ra hàm | psABI §3.2.1 |
| `MXCSR` | control bit callee-saved; status bit caller-saved | psABI §3.2.1 |
| x87 | control word callee-saved; status word caller-saved | psABI §3.2.1 |
| Vector | `XMM0`–`XMM7` nhận tham số; `XMM0`/`XMM1` trả về | psABI §3.2.3 |
| Variadic | `AL` = số thanh ghi vector đã dùng (0x0–0x8) | psABI §3.2.4 |

**Hệ quả cho kernel ViFastOs** (đề xuất `[TK]`, chưa duyệt):

- Kernel **không** được dùng red zone: biên nhận interrupt/syscall không được ghi dưới `RSP`
  user. Cờ biên dịch `-mno-red-zone` cho mọi mã kernel (M3/M4).
- `DF = 0x0` phải được bảo đảm ở biên syscall/interrupt (kernel không giả định user giữ `DF`).
- `XMM`/x87: bước M2 **không** chạm; M8 mới có XSAVE/FPU (audit K5' M14). Nếu user Rust dùng
  SSE trước M8 thì kéo XSAVE lên M7 (audit §5 R8).
- `syscall` **không** đổi `RSP`: stub phải lấy ngăn xếp kernel từ per-CPU (qua `GS`) **trước**
  khi `push` — nếu không sẽ ghi vào red zone của user.

## 3. Đối chiếu `spec/abi.toml` hiện có (riscv64)

`spec/abi.toml` `[calling.riscv64]` đóng băng **ngay** (D2-15); `[calling.aarch64]` và
`[calling.x86_64]` chỉ giữ chỗ (D2-10). Bảng dưới đối chiếu từng trường.

| Trường `[calling.riscv64]` | riscv64 (đóng băng) | Đề xuất x86_64 `[TK]` | Giữ / Khác |
|---|---|---|---|
| `syscall_number` | `a7` | `RAX` | **Khác tên**; giữ "một thanh ghi riêng cho số hiệu" |
| `args` | `a0`, `a1` | `RDI`, `RSI` | **Khác tên**; giữ "hai tham số" (`cap`, `reply_cptr`/`arg1`) |
| `message_info` | `a2` | `RDX` | **Khác tên**; giữ "một thanh ghi" |
| `mr_registers` | `a3`–`a6` | `R10`, `R8`, `R9`, `R12` | **Khác tên**; **giữ bốn MR trong thanh ghi** (`[frozen].message_layout`) |
| `return_error` | `a0` | `RAX` | **Khác tên**; giữ "0x0 = thành công, khác 0x0 = mã lỗi 0x1–0x13" |
| `return_value` | `a1` | `RDX` | **Khác tên**; giữ "một thanh ghi" |
| `ipc_buffer` | Frame bind qua `Tcb.Configure`; header chứa receive window (CPTR, ≤ bốn cap) | **giữ nguyên** | **Giữ** — không phụ thuộc arch |
| `preserved` | `sp`, `gp`, `tp`, `s0`–`s11`, `fs0`–`fs11` | `RSP`, `RBX`, `RBP`, `R12`–`R15` | **Khác tập**; giữ "ngăn xếp + callee-saved" |

### 3.1 Phải **giữ nguyên** (không được đổi khi mở `[calling.x86_64]`)

một. **Năm thứ đóng băng** D2-15 / QD §1 Q-B: (i) số syscall `0x0`–`0xF` + quy ước thanh ghi;
   (ii) layout `MessageInfo` + MR trong thanh ghi + receive window; (iii) mười chín mã lỗi +
   quy ước trả lỗi; (iv) kích thước/căn lề object theo arch; (v) ID không bao giờ tái dùng.
   Với x86_64, "quy ước thanh ghi" **đổi tên** nhưng **không** đổi số/ngữ nghĩa.
hai. `[message]`: `max_words = 0x40`, `max_caps = 0x4`, `words_in_registers = 0x4`,
   `words_in_ipc_buffer = 0x3C`, `ipc_buffer_bytes = 0x1000`, `ipc_buffer_align = 0x10`.
   **Bốn MR trong thanh ghi là bắt buộc** — đây là lý do §4 phải chọn `R12` làm MR3.
ba. `[message_info]`: 35-bit, layout `label:0x0–0x13, caps:0x14–0x16, no_reply:0x17,
   extra_caps_unwrapped:0x18–0x1B, length:0x1C–0x22` (hex của cùng dải bit như `spec/abi.toml`)
   — **giữ nguyên**; chỉ đổi thanh ghi chứa nó.
bốn. `[debug_info].return_channel`: `a1 = abi_version`, `a3 = min_compat`, `a4 = features`,
   `a5 = num_harts` — **giữ thứ tự**, ánh xạ sang thanh ghi x86 (đề xuất §4.5).
năm. `[error]` 0x1–0x13 và `[syscall]` 0x0–0xF: **không** thêm/bớt số.

### 3.2 Phải **khác** (đặc thù x86_64)

- **Tên thanh ghi** (bảng trên) — hiển nhiên.
- **`syscall` clobber `RCX`/`R11`**: `RCX` = RIP kế tiếp, `R11` = `RFLAGS` (Intel SDM Vol 2B,
  mục SYSCALL). Vì vậy `RCX`/`R11` **không** được làm thanh ghi vào của syscall.
- **Red zone 128-byte**: riscv64 không có; kernel x86 phải `-mno-red-zone` và biên không ghi
  dưới `RSP` user.
- **Căn ngăn xếp 16-byte**: riscv64 ABI cũng căn 16-byte nhưng quy tắc "`(RSP + 0x8)` bội mười sáu" là
  đặc thù `call`/`ret` x86.
- **`syscall` vs `int 0x80`**: riscv64 chỉ có `ecall`; x86 có hai đường (xem §4).
- **`swapgs` + MSR `STAR/LSTAR/FMASK`**: riscv64 dùng `stvec`/`sepc`/`scause`; x86 dùng MSR.
- **`TSS.RSP0`** cho `int 0x80` (chuyển ngăn xếp phần cứng) — riscv64 không có TSS.

## 4. Syscall entry: `syscall` vs `int 0x80`

### 4.1 So sánh

| Tiêu chí | `syscall`/`sysret` | `int 0x80` |
|---|---|---|
| Cơ chế | MSR `STAR`/`LSTAR`/`FMASK`; không qua IDT | Cổng IDT vector `0x80`, DPL = 0x3 |
| Clobber | `RCX` ← RIP kế tiếp, `R11` ← `RFLAGS` | Không clobber `RCX`/`R11`; đẩy `SS:RSP/RFLAGS/CS/RIP` lên ngăn xếp |
| Chuyển ngăn xếp | **Không** — stub phải tự lấy kernel stack (per-CPU qua `GS`) | **Có** — phần cứng dùng `TSS.RSP0` |
| Tốc độ | Nhanh (đường nóng) | Chậm hơn (đi qua IDT, nhiều lần đọc bộ nhớ) |
| `swapgs` | Cần (entry/exit) | Cần (entry/exit) hoặc dùng `TSS` |
| Rủi ro | `sysret` với `RCX` không canonical (CVE-2012-0217) — phải kiểm canonical hoặc dùng `iretq` | Chậm; vẫn phải kiểm `RIP`/`RFLAGS` |
| Dùng cho | **Đường sản phẩm** (đề xuất) | **Dự phòng/debug** (đề xuất) |

**Đề xuất `[TK]`**: `syscall`/`sysret` là đường sản phẩm; `int 0x80` chỉ là đường dự phòng
(hoặc bỏ hẳn) — không mở hai ABI song song trong sản phẩm. Lý do: `syscall` nhanh hơn, không
cần IDT gate cho đường nóng, và MSR `STAR/LSTAR/FMASK` đã nằm trong kế hoạch M7 (audit K5' M7).

### 4.2 Bảng thanh ghi đề xuất cho `syscall` `[TK]`

| Vai trò | Thanh ghi | Ghi chú |
|---|---|---|
| Số hiệu syscall | `RAX` | `syscall` không clobber `RAX` |
| `cap` (arg0) | `RDI` | |
| `reply_cptr`/`arg1` | `RSI` | |
| `MessageInfo` | `RDX` | |
| MR0–MR3 | `R10`, `R8`, `R9`, `R12` | `R12` là callee-saved — xem §4.4 |
| Trả lỗi | `RAX` | `0x0` = thành công |
| Trả giá trị | `RDX` | |
| Trả `MessageInfo` (`Recv`/`ReplyRecv`) | `R13` | **đề xuất**, cần Agent 0 chốt ở M7 |
| MR0–MR3 khi trả | `R10`, `R8`, `R9`, `R12` | cùng bộ với entry |

### 4.3 Bảng thanh ghi đề xuất cho `int 0x80` `[TK]`

| Vai trò | Thanh ghi | Ghi chú |
|---|---|---|
| Số hiệu syscall | `RAX` | |
| `cap` / `arg1` | `RDI` / `RSI` | |
| `MessageInfo` | `RDX` | |
| MR0–MR3 | `RCX`, `R8`, `R9`, `R10` | `int 0x80` **không** clobber `RCX` |
| Trả lỗi / giá trị | `RAX` / `RDX` | |
| Trả `MessageInfo` | `R11` | `int 0x80` không clobber `R11` |

### 4.4 Vì sao MR3 là `R12` (không phải `RCX`)

Sau khi dùng `RAX` (số hiệu), `RDI`/`RSI` (hai tham số), `RDX` (`MessageInfo`), chỉ còn **ba**
thanh ghi caller-saved (`R8`, `R9`, `R10`) cho **bốn** MR — vì `syscall` đã lấy `RCX`/`R11`.
Đề xuất dùng `R12` (callee-saved) làm MR3: stub syscall là một hàm C/asm bình thường, theo SysV
nó **phải** giữ `R12`; quy ước **syscall** là hợp đồng riêng, không phải quy ước gọi hàm. Đánh
đổi: user wrapper phải đọc `R12` trước khi clobber; nếu Agent 0 không muốn dùng callee-saved
thì chọn `int 0x80` (MR0–MR3 = `RCX`, `R8`, `R9`, `R10`) — nhưng mất tốc độ.

### 4.5 `DebugGetInfo` (syscall `0xF`) — đề xuất ánh xạ `[TK]`

`spec/abi.toml` `[debug_info].return_channel` ghi `a1 = abi_version`, `a3 = min_compat`,
`a4 = features`, `a5 = num_harts` (riscv64). Đề xuất x86_64 giữ **đúng thứ tự** đó:

| Trường | riscv64 | Đề xuất x86_64 |
|---|---|---|
| `error` | `a0` | `RAX` |
| `abi_version` | `a1` | `RDX` |
| `min_compat` | `a3` | `R8` |
| `features` | `a4` | `R9` |
| `num_harts` | `a5` | `R10` |

Đây là **đề xuất**, không phải ABI: `spec/` chỉ mở ở M7, và `[debug_info]` là một trong năm thứ
đóng băng (D2-15) nên **không** được đổi thứ tự/ngữ nghĩa — chỉ đổi thanh ghi.

### 4.6 Rủi ro phải xử ở M7

- **`sysret` + `RCX` không canonical** (CVE-2012-0217): kiểm `RCX` canonical trước `sysret`,
  hoặc dùng `iretq` cho đường chậm. Không được để fault ở CPL0.
- **`swapgs`**: entry `swapgs` để lấy per-CPU; exit `swapgs` trả lại. Nếu user kiểm soát
  `GS.base` thì phải chặn `wrgsbase` (CPUID/CR4.FSGSBASE) — bước M7.
- **`FMASK`**: che `IF`, `TF`, `DF`, `AC` (và `NT`) khi vào kernel; `DF = 0x0` là bắt buộc.
- **Red zone**: stub `syscall` không được `push` trước khi đổi ngăn xếp.
- **`TSS.RSP0`** cho `int 0x80`: phải là ngăn xếp kernel hợp lệ **trước** khi bật IDT gate DPL3.

## 5. Bảng bit/thanh ghi (đối chiếu crate `vifast-plat-x86_64`)

Crate `v2/kernel/vifast-plat-x86_64` (KT-186, M2) cài các hằng dưới đây và test **giá trị tuyệt
đối** viết tay từ Intel SDM (xem `tests/host.rs`, module `intel_sdm`). Bảng này để Agent 0 đối
chiếu khi mở `spec/` — **không** phải nguồn sự thật.

| Nhóm | Hằng (rút gọn) | Giá trị | Nguồn |
|---|---|---|---|
| `CR0` | `PE`, `MP`, `EM`, `TS`, `ET`, `NE`, `WP`, `AM`, `NW`, `CD`, `PG` | bit `0x0`, `0x1`, `0x2`, `0x3`, `0x4`, `0x5`, `0x10`, `0x12`, `0x1D`, `0x1E`, `0x1F` | Intel SDM Vol 3A, "Control Registers" |
| `CR4` | `PAE`, `PGE`, `OSFXSR`, `OSXMMEXCPT`, `FSGSBASE`, `PCIDE`, `OSXSAVE`, `SMEP`, `SMAP`, `PKE`, `CET`, `PKS` | bit `0x5`, `0x7`, `0x9`, `0xA`, `0x10`, `0x11`, `0x12`, `0x14`, `0x15`, `0x16`, `0x17`, `0x18` | Intel SDM Vol 3A, "Control Registers" |
| `IA32_EFER` | `SCE`, `LME`, `LMA`, `NXE`, `SVME`, `LMSLE`, `FFXSR`, `TCE` | bit `0x0`, `0x8`, `0xA`, `0xB`, `0xC`, `0xD`, `0xE`, `0xF` | Intel SDM Vol 3A, "IA32_EFER" |
| MSR | `IA32_EFER`, `STAR`, `LSTAR`, `FMASK`, `FS_BASE`, `GS_BASE`, `KERNEL_GS_BASE`, `PAT`, `APIC_BASE` | `0xC0000080`, `0xC0000081`, `0xC0000082`, `0xC0000084`, `0xC0000100`, `0xC0000101`, `0xC0000102`, `0x277`, `0x1B` | Intel SDM Vol bốn, chương hai |
| PTE | `P`, `RW`, `US`, `PWT`, `PCD`, `A`, `D`, `PS` (mức 0x0–0x2) / `PAT` (mức 0x3), `G`, `PAT` (lá lớn), `NX` | bit `0x0`, `0x1`, `0x2`, `0x3`, `0x4`, `0x5`, `0x6`, `0x7`, `0x8`, `0xC`, `0x3F` | Intel SDM Vol 3A, "4-Level Paging" |
| Hình học | bốn mức, 4-KiB, 9-bit/mức, VA 39-bit canonical (cửa sổ kernel), PA 48-bit | — | D2-147 + audit K5' bước M2 |
| PAT mặc định | `IA32_PAT_DEFAULT` | `0x0007040600070406` | Intel SDM Vol 3A, "PAT" |

**Ghi chú quan trọng**: với lá 4-KiB, bit `PAT` là **bit `0x7`** (cùng vị trí `PS` ở mức
0x0–0x2); với lá 2-MiB/1-GiB, bit `PAT` là **bit `0xC`**. Đây là cái bẫy đã làm test M2 đỏ lần
đầu — crate đã sửa và có test `descriptor_ps_bit_by_level` chốt.

## 6. Việc M7 phải làm khi mở `spec/`

một. Mở `[calling.x86_64]` **một lần** theo khuôn **KT-05** (D2-33/D2-35): thêm khối
   `[calling.x86_64]` với các trường như `[calling.riscv64]` (tên trường giữ nguyên, giá trị là
   thanh ghi x86); **không** đổi `[calling.riscv64]`; **không** đổi năm thứ đóng băng.
hai. Cập nhật `docs/abi.md` qua `vifast-gen`; `gen-check` phải xanh.
ba. Chốt ba chỗ còn mở: (i) kênh trả `MessageInfo` của `Recv` (`R13`?); (ii) `int 0x80` có tồn
   tại trong sản phẩm hay không; (iii) `DebugGetInfo` ánh xạ `a1/a3/a4/a5` sang thanh ghi nào.
bốn. Test M7: `init` ELF chạy; **mười sáu** syscall; fault cách ly; `DebugGetInfo` khớp golden;
   **năm mươi** boot (audit K5' M7).
năm. Người mở: **Agent 0** (D2-22); bản nháp này chỉ là đầu vào, **không** tự duyệt.

## 7. Việc bản nháp này **không** làm

- **Không** sửa `spec/`, **không** sửa `docs/v2/ViFastOs-v2-thiet-ke.md`, **không** sửa
  `docs/v1/`, **không** sửa `AGENTS.md`.
- **Không** tuyên bố `[calling.x86_64]` đã chốt/đóng băng; **không** tuyên bố K5' đạt.
- **Không** boot, **không** QEMU, **không** đo — việc hai (M2) đo bằng `cargo test` trên host.
- **Không** đặt số đo viết tay: mọi số ở đây là hằng đặc tả hoặc `[TK]`.

## 8. Bằng chứng (việc một + việc hai)

- Bản nháp này: `v2/agent/reports/K5-X86-CALLING-DRAFT.md` (tệp đang đọc).
- Crate M2: `v2/kernel/vifast-plat-x86_64/` — `cargo test -p vifast-plat-x86_64` xanh
  **mười tám** test host; thân test generic `kiem_tung_dinh_dang::<X86_64>()` chạy trong
  `v2/kernel/vifast-plat-common/tests/mmu_shared.rs` (bốn test, có x86_64).
- Đột biến: **năm** ca, tất cả **ĐỎ** — log ở `v2/agent/JOURNAL/kt186-logs/` (xem journal
  `v2/agent/JOURNAL/kt186-x86m2.md`).
- `./v2/bin/v2 check`: **PASS** (log trong journal).
- `spec/` không đổi: `git diff --stat -- spec/` rỗng.
