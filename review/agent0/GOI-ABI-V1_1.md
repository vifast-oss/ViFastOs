<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Gói duyệt **ABI v1.1** — RFC duy nhất (KT-344, **9 nhóm**)

**Ngày:** 2026-11-22 · **Nhánh:** `task/kt344-rfcabi` · **Base:** `main` @ `ee8f2ca2` (D2-236..D2-240)
**Người soạn:** sub-agent `rfcabi` (lead uỷ quyền) · **Người ký:** **Agent 0**

> **Đây KHÔNG phải duyệt.** Duyệt chỉ có giá trị bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` ghi trong
> [`v2/agent/QUEUE.md`](../../v2/agent/QUEUE.md) (AGENTS.md §8), kèm mục D2 mới nếu quyết định đổi nội dung ABI.
>
> **Tệp này tự chứa**: không cần đọc tệp nào khác trong `review/agent0/`.
> **KHÔNG có byte nào của `spec/` bị sửa bởi KT-344** (D2-04, D2-234). Mọi khối TOML/IDL dưới đây là
> **ĐỀ XUẤT — CHƯA ÁP DỤNG**; chỉ `KT-345a` (spec/gen) được áp sau khi Agent 0 ký — `KT-345b` (tính năng) mở sau đó theo DAG (D2-237).

**Nguồn yêu cầu:** `KE-HOACH-CHU-KY-2-2026-11-22.md` mục `KT-344`/`KT-345a`/`KT-345b` (bản `ee8f2ca2`);
`v2/agent/DECISIONS.md` **D2-234** (dòng 445), **D2-237** (dòng 451), D2-228 (dòng 433), D2-235 (dòng 447);
`v2/agent/QUEUE.md` dòng 427–429 (`KT-344`, `KT-345a`, `KT-345b`).

---

## 0. TL;DR — 9 nhóm, một bảng, một câu chốt

> **Đối chiếu `QUEUE.md`:** mục `KT-344` liệt 9 nhóm theo cách gộp khác (mục 5 gộp *calling + GDT*; mục 9 là *đánh giá tương thích*). RFC này giữ **9 nhóm** theo chỉ đạo trực tiếp của lead: 5 = GDT, 7 = hợp đồng trả về `x86_64`, 8 = `[debug_info]`/`[object_size]`, 9 = trạng thái object; phần tương thích ngược nằm ở **§0.1**.

| # | Nhóm nợ | Vấn đề một dòng | Khuyến nghị | Tệp `spec/` đề nghị chạm | Đổi **byte wire**? | `abi_version` |
|---|---|---|---|---|---|---|
| 1 | `spec/kernel.toml` | Tệp chưa tồn tại nhưng đã được 5 tài liệu/luật dẫn như nguồn sự thật; hằng số nằm rải ở 5 nơi | **Tạo tối thiểu + máy kiểm**, chưa sinh hằng, chưa đổi consumer | `spec/kernel.toml` (**mới**) | Không | 1 |
| 2 | `extra_caps_unwrapped` | Trường 4 bit **đã có chủ** (số cap unwrap, KT-63); số **extra caps** thật đang nằm ở vùng `[TK]` của IPC buffer, không phải ABI | **Đóng băng layout vùng extra caps trong IPC buffer** (`[ipc_buffer]`); **không** thêm bit vào `MessageInfo` | `spec/abi.toml` (+`[ipc_buffer]`) | Không (offset đang chạy) | 1 |
| 3 | Kênh lỗi bảng 8 `VSpace` + ô 0 `CSpace` | Hook `tcb_vspace` trả `bool` bị **vứt** (`let _ =`); bảng tràn ⇒ `Configure` vẫn trả `(0,0)`; ô 0 vừa là `CS_SELF` vừa bị KT-325 dùng làm "xoá" | Hook **3 trạng thái** + `Busy` (14); ghi luật **ô 0 CSpace** vào spec | `spec/abi.toml` (+`[cspace]`, +`errors` của `Tcb.Configure`) | Không | 1 |
| 4 | `MemInfo`/`STATFS` cho `free`/`df` | Hai lệnh `deferred-K7`; v2 **cố ý bỏ** `MemInfo` (thiết kế:494); dịch vụ FS đã là hợp đồng `[TK]` "không mở ABI mới" | **Không** thêm syscall/object; `free`/`df` đi qua **protocol IDL** (`spec/protocols/*.vidl`) | `spec/protocols/{meminfo,fsstat}.vidl` (**mới**) | Không | 1 |
| 5 | Thứ tự **GDT** cho `syscall`/`sysret` x86_64 | `sysret` đòi user data ở `CS−8`, GDT KT-261 đặt **ngược**; đổi thứ tự sẽ chạm TAP/đột biến đang ghim selector | Chốt **thứ tự đích** (đảo: user data trước user code) + ghi hình học GDT vào `kernel.toml`; **chưa** đổi `trap` trong v1.1 | `spec/kernel.toml` (`[arch.x86_64.gdt]`) | Không | 1 |
| 6 | Socket IDL cho `NetSvc` **đa caller** | `NetSvc` có đúng **một** `caller: ObjId` + một `Reply`; chưa có IDL socket; generator mới có `ring`/`pager` | `spec/protocols/socket.vidl` + demux theo **handle** ở netsrv U-mode; `NetSvc` kernel **giữ đơn caller** | `spec/protocols/socket.vidl` (**mới**) | Không | 1 |
| 7 | Hợp đồng trả về `x86_64` | `return_value` **trùng** `message_info` (`rdx`) ⇒ `Recv`/`ReplyRecv` không trả đồng thời cả hai; thiếu `return_message_info`; `r12` vừa `mr_registers` vừa `preserved` | Thêm `return_message_info = "r11"`; `rdx` **vẫn** = value; `r12` ra khỏi `preserved`, vào `clobbered` | `spec/abi.toml` (`[calling.x86_64]`) | Không | 1 |
| 8 | `[debug_info]`/`[object_size]` 3 arch | `return_channel` chỉ có tên thanh ghi riscv64; `[object_size]` thiếu `aarch64`/`x86_64` dù core dùng **chung** bộ `OBJ_*` cho mọi arch | **Sinh từ `vifast-gen`**: 3 khoá alias `same_as = "model"` + kênh trả theo **vai trò**, suy ra từ `[calling.*]` | `spec/abi.toml` (`[debug_info]`, `[object_size]`) | Không | 1 |
| 9 | Trạng thái `VSpace`/`PageTable`/`Frame` | Cả ba ghi `reserved` + `unsupported`, nhưng `Frame.Map` **đã chạy thật** (map PTE + `sfence.vma`); `VSpace`/`PageTable` là đích `Untyped.Retype` | `Frame` → `frozen`; `VSpace`/`PageTable` → `core` (method chưa cài giữ `unsupported` + `install_at`) | `spec/abi.toml` (`[object]`) | Không | 1 |

**Câu chốt của gói:** cả **9** khuyến nghị đều **không đổi byte wire** ⇒ giữ nguyên
`abi_version = 1`, `min_compat = 1` (`spec/abi.toml:15-16`). "v1.1" là **nhãn gói**, không phải giá trị
trường trong spec.

### 0.1 Nhóm nào đổi `abi_version` 1→2? (bảng bắt buộc — cho **cả 9 nhóm**)

| # | Nhóm | Khuyến nghị | Đổi **byte wire**? | `abi_version` sau khi duyệt | `min_compat` | Ghi chú |
|---|---|---|---|---|---|---|
| 1 | `spec/kernel.toml` | **1C** | Không | 1 | 1 | tệp cấu hình, không phải wire |
| 2 | `extra_caps_unwrapped` | **2C** | Không | 1 | 1 | đóng băng layout buffer **đang chạy**; `MessageInfo` không đổi |
| 3 | Kênh lỗi `VSpace` + ô 0 | **3.1A + 3.2A** | Không | 1 | 1 | thêm **giá trị lỗi** trong nhánh thất bại + ghi luật đang chạy |
| 4 | `MemInfo`/`STATFS` | **4C** | Không | 1 | 1 | IDL, **không** syscall/object mới |
| 5 | GDT `syscall`/`sysret` | **5A** (chốt hướng) | Không | 1 | 1 | `trap` vẫn `int 0x80`; chỉ ghi `planned` |
| 6 | Socket IDL `NetSvc` | **6A** | Không | 1 | 1 | protocol ngoài `abi.toml` |
| 7 | Hợp đồng trả về `x86_64` | **7C** | Không | **1** | 1 | thêm `return_message_info = "r11"`; `rdx` **vẫn** = value; sửa `clobbered`/`preserved` |
| 8 | `[debug_info]`/`[object_size]` 3 arch | **8C** | Không | 1 | 1 | metadata + sinh hằng; 3 arch **cùng** số (model) |
| 9 | Trạng thái `VSpace`/`PageTable`/`Frame` | **9B** | Không | 1 | 1 | chỉ đổi `stability`/`unsupported`; không đổi hành vi |

**Trả lời thẳng: nếu Agent 0 duyệt đúng 9 khuyến nghị trên thì `abi_version = 1`, `min_compat = 1`.**
Không có byte wire nào đổi, không chương trình U-mode nào phải biên dịch lại. "v1.1" chỉ là **nhãn gói**.

**Nhưng vẫn phải sửa các kỳ vọng cứng sau** — vì chúng ghim *nội dung* `spec/`, không phải số version:

| Tệp: dòng | Đang ghim | Phải thành | Do nhóm |
|---|---|---|---|
| `v2/test/abi/check_abi.py:235-238` | `return_channel` đúng `a1/a3/a4/a5` | kiểm **cả 3 arch** suy ra từ `[calling.*]` | 8C |
| `v2/test/abi/check_abi.py:326,328` | `value` = `rdx`, `message_info` = `rdx` | **giữ nguyên**, thêm kiểm `return_message_info` (= `r11`, ≠ `message_info`) | 7C |
| `v2/test/abi/check_abi.py:338-340` | tập khoá `x86_64` = riscv64 + `{trap,stack,clobbered}` | thêm `return_message_info` vào tập `extra` của x86_64 | 7C |
| `v2/test/abi/check_abi.py:345` | `return_error != return_value` | giữ, thêm `return_message_info != message_info` | 7C |
| `v2/test/abi/check_abi.py:363-364` | `X86_64_MR_REGS` nguyên văn `r10/r8/r9/r12` | giữ (7C **không** đổi MR) | 7C |
| `v2/test/abi/check_abi.py:368-370` | chỉ kiểm `slot.bytes` | thêm kiểm 3 khoá `[object_size].{riscv64,aarch64,x86_64}` **bằng nhau** | 8C |
| `v2/test/abi/check_reserved.py:17-26` | đúng **8** object reserved (9..16) | còn **5** (12..16); ba object 9..11 ra khỏi danh sách | 9B |
| `v2/test/abi/check_reserved.py:130` | `object_reserved` 17..31 | giữ (không thêm object id mới) | — |
| `v2/test/abi/check_abi.py:60-70` | `CORE_OBJECTS`/`CORE_IDS` chỉ 8 object 1..8 | thêm `VSpace` 9 / `PageTable` 10 / `Frame` 11 | 9B |
| `v2/kernel/vifast-plat-x86_64/src/calling.rs` | `INT80_CLOBBERED = [Rax, Rdx]` | `[Rax, Rdx, R11, R10, R8, R9, R12]`; `INT80_RETURN_MSGINFO_REG = R11` | 7C |
| `v2/kernel/vifast-plat-x86_64/tests/host.rs:736-768` | ghim `SYSCALL_RETURN_MSGINFO_REG = R13`, `INT80_CLOBBERED = [Rax,Rdx]` | cập nhật theo spec mới (`r11` + MR) | 7C |
| `v2/tools/vifast-gen/gen.py:205-214` | sinh 5 trường `[calling.*]` | thêm `return_message_info` → `RETURN_MESSAGE_INFO_REG` | 7C |
| `v2/tools/vifast-gen/gen.py:140` | `rv = osz.get("riscv64")` | đọc **cả 3** arch, assert bằng nhau, sinh `OBJ_*` một lần | 8C |
| `v2/lib/vifast-abi/src/lib.rs:116-121` | `RESERVED_OBJECTS` chứa `VSpace` id 9 | đổi sang object reserved còn lại (vd `IrqControl` id 12) | 9B |
| `v2/lib/vifast-abi/src/generated.rs` | sinh từ spec | **sinh lại**, không sửa tay | 7C/8C/9B |
| `v2/test/boot/ktest.sh:650` | `abi_version=1` | **giữ nguyên** | — |

**Nếu Agent 0 chọn phương án khác (đắt hơn), bảng version đổi như sau:**

| Phương án khác | `abi_version` (`min_compat`) | Kỳ vọng cứng phải sửa thêm |
|---|---|---|
| **2A** (thêm bit vào `MessageInfo`) | **2** (1) | `check_abi.py:278` (bits 35→38), `:137-138`, `ktest.sh:650`, toàn bộ sim pin KT-25/KT-63 |
| **4A** (syscall 16/17) | **2** (1) | `check_abi.py:137-138`, `:146-152`; `gen.py:33-37`; `ktest.sh:650` |
| **6C** (object `Socket` = 17) | 1 (1) | `check_abi.py:162-163` (17..31 → 18..31); `check_reserved.py:130`; `gen.py` |
| **7A** (`return_value` → `r13`) | **2** (**2**) | `check_abi.py:326`; `:338-340`; `:363`; `host.rs:736-768`; `calling.rs` |
| **8B** (hoãn `return_channel`) | 1 (1) | không (nhưng nợ lại cho `KT-338b`/`KT-340c`) |
| **9A** (cả ba `frozen`) | 1 (1) | `check_reserved.py:17-26`; `gen.py`; `lib.rs:116-121` |

---

**9 câu cần Agent 0 trả lời** (chi tiết + câu chữ D2 mẫu ở §14):

1. Có tạo `spec/kernel.toml` **bây giờ** (tối thiểu) hay để Chu kỳ 3? → khuyến nghị **tạo tối thiểu**.
2. `extra_caps` thành ABI bằng **layout IPC buffer** hay bằng **trường mới trong `MessageInfo`**? → khuyến nghị **buffer**.
3. `Configure` có được trả lỗi **`Busy` (14)** khi arch không giữ được `VSpace`? → khuyến nghị **có**.
4. `free`/`df` đi bằng **protocol IDL** (không đụng `abi.toml`) hay bằng **syscall/object mới**? → khuyến nghị **IDL**.
5. Thứ tự GDT đích: **đảo** (Linux-style) hay **descriptor lặp** (giữ selector cũ)? → khuyến nghị **đảo**, nhưng **chưa** đổi `trap` trong v1.1.
6. Socket IDL đặt ở **`spec/protocols/`** hay **`v2/lib/vifast-idl/idl/`**? → khuyến nghị **`spec/protocols/`**.
7. `[calling.x86_64]`: thêm **`return_message_info`** (`r11`) hay **đổi `return_value`** sang `r13`? → khuyến nghị **thêm trường** (giữ `rdx` = value).
8. `[debug_info]`/`[object_size]`: **sinh từ `vifast-gen`** (alias 3 arch + kênh theo vai trò) hay điền tay 3 arch? → khuyến nghị **sinh**.
9. `VSpace`/`PageTable`/`Frame`: **`Frame` frozen + 2 object `core`** hay cả ba `frozen`? → khuyến nghị **theo bằng chứng** (9B).

---

## 1. Bối cảnh, luật và cách đọc gói này

### 1.1 Vì sao có gói này

- **D2-234** gom **toàn bộ nợ chạm `spec/`** vào **một RFC duy nhất** (`KT-344`), và **cấm** mọi task
  Chu kỳ 2 khác sửa `spec/abi.toml`. **D2-237** mở rộng RFC từ **6 lên 9 nhóm**, tách `KT-345` thành
  **`KT-345a`** (áp `spec/` + `vifast-gen`, tiền đề của `KT-338b`/`KT-340c`) và **`KT-345b`** (`free`/`df`/`loop`
  + `NetSvc`); chỉ `KT-345a` được mở sau dòng `APPROVED` cho `KT-344`.
- **D2-04/D2-22**: `spec/` là nguồn sự thật của ABI; sau duyệt **chỉ Agent 0** sửa. KT-344 là **task thuần
  tài liệu**: không sửa `spec/`, `v2/kernel/**`, `v2/lib/**`, `v2/bin/v2`.
- **D2-235(3)**: các task Đợt 0 (gồm KT-344) **cấm chạm** `v2/kernel/**` và `v2/lib/**`.

### 1.2 Năm thứ đóng băng — trích nguyên văn

```text
spec/abi.toml:21-27
# Đóng băng ABI (D2-15 / QD §1 Q-B): đúng 5 thứ. Mọi thứ khác là `reserved`.
[frozen]
wire_syscalls = true          # (1) số syscall 0..15 + quy ước thanh ghi riscv64
message_layout = true         # (2) layout MessageInfo + MR trong thanh ghi + receive window
error_table = true            # (3) 19 mã lỗi + quy ước trả lỗi (a0 = error, a1 = value)
object_layout = true          # (4) kích thước/căn lề object theo arch
id_reuse = "never"            # (5) ID không bao giờ tái dùng; xoá = deprecated, giữ số
```

**Hệ quả dùng xuyên suốt RFC:** một đề xuất chỉ được coi là "rẻ" nếu **không** đụng (1) hoặc (2). Đụng (2)
là đụng `MessageInfo`; đụng (1) là đụng dải syscall 0..15. Hai nhóm 2A và 4A đụng đúng hai thứ này — đó là
lý do chúng **không** được khuyến nghị.

### 1.3 `min_compat` được hiểu thế nào trong gói này

`min_compat = 1` (`spec/abi.toml:16`) là **phiên bản ABI tối thiểu mà chương trình cần**; kernel báo nó qua
`DebugGetInfo` (`spec/abi.toml:158,170-172`). Quy ước đánh giá dùng trong cả **9** nhóm:

| Loại thay đổi | `abi_version` | `min_compat` | Ví dụ trong gói |
|---|---|---|---|
| Chỉ **thêm** thứ chưa ai dùng (bit trống, object id reserved, file protocol) | giữ 1 | giữ 1 | 1C, 5A, 6A |
| **Ghi rõ** hành vi đang chạy (không đổi byte) | giữ 1 | giữ 1 | 2C, 3.2A, 4C |
| **Thêm kênh trả / khai báo metadata** mà byte đang chạy không đổi | giữ 1 | giữ 1 | 7C, 8C, 9B |
| **Đổi nghĩa** một giá trị đang chạy (chương trình cũ **hết** đúng) | **2** | **2** | 7A, 7B |
| **Thêm** số syscall / mở rộng bit chưa dùng (chương trình cũ vẫn đúng) | **2** | 1 | 2A, 4A, 6C |
| Mở rộng `MessageInfo` vào bit **chưa dùng** (chương trình cũ vẫn đọc đúng) | **2** | 1 | 2A |

### 1.4 Bộ kiểm là **hợp đồng**, không phải "kỳ vọng để test xanh"

Mọi đề xuất dưới đây ghi rõ tệp kiểm nào phải sửa. Các điểm ghim hiện tại (đã đọc, chưa sửa):

| Tệp kiểm | Dòng | Đang ghim |
|---|---|---|
| `v2/test/abi/check_abi.py` | 137–142 | `abi_version == 1`, `min_compat == 1`, `frozen.*` |
| `v2/test/abi/check_abi.py` | 146–152 | syscall **đúng 0..15**, reserved **16..31** |
| `v2/test/abi/check_abi.py` | 277–283 | `message_info.bits == 35`, `max_words == 64`, `max_caps == 4` |
| `v2/test/abi/check_abi.py` | 321–341 | `[calling.x86_64]`: `trap == "int 0x80"` (dòng 331), đối xứng cấu trúc (dòng 338) |
| `v2/test/abi/check_reserved.py` | 130 | `object_reserved` **17..31**; 8 object reserved 9..16 phải `unsupported` |
| `v2/tools/vifast-gen/gen.py` | 33–37 | `die` nếu syscall khác `0..15` / lỗi khác `1..19` |
| `v2/test/scn/check_scn.py` | 576, 601 | **đúng 27** tệp `.scn`; tag phải thuộc whitelist |
| `v2/bin/v2` | 589–590, 597, 721–722 | `check_scn`, `check_scn_fixtures`, `vifast-idl --check` là bước cổng |

### 1.5 `Suspend(Defer)` **không** thuộc gói này

D2-228 đã chốt: `Restart` không có trong spec là **thiết kế**, không phải thiếu; đường khôi phục là
`Tcb.Resume` (12); ngữ nghĩa `Defer` = đặt cờ + dừng ở điểm an toàn. Spec hiện đã có đủ:

```text
spec/abi.toml:181
Tcb = { id = 3, stability = "frozen", method = { 0 = { name = "Configure" }, … 3 = { name = "Suspend", args = "mode: Abort|Defer" }, … 12 = { name = "Resume" }, … } }
spec/abi.toml:261-268
[method_signature."Tcb.Suspend"]
params = ["tcb", "mode"]
variants = { mode = ["Abort", "Defer"] }
returns = []

[method_signature."Tcb.Resume"]
params = ["tcb"]
returns = []
```

⇒ **Không có mục nào cho `Suspend(Defer)`** trong gói. Ghi ở đây để Agent 0 không đi tìm.

### 1.6 Xung đột đang chạy (phải xử lý lúc thi hành)

- **`KT-333a`/`KT-333b` đang sửa `v2/bin/v2`** (tối ưu T2; D2-235(5) đòi kiểm trùng `lane_step` trên **cả**
  `do_gate_fast` và `do_gate_dev`). ⇒ `KT-345a` **không** được chọn ID bước trước khi 2 task đó merge; phải
  rebase rồi mới thêm bước, và `grep -n "lane_step" v2/bin/v2` trước khi commit.
- **`KT-330` soak độc quyền máy đo** (`qemu_foreign = 0`). KT-344/KT-345a/KT-345b **không** chạy QEMU.
- **`KT-332`** đang sửa `README/STATE/BLOCKED` (không chạm `spec/`).
- **Thứ tự mới (D2-237):** `KT-345a` (áp `spec/` + `vifast-gen`) là **tiền đề** của `KT-338b`/`KT-340c` —
  nếu `KT-345a` chậm, hai task nối `aarch64`/`x86_64` vào `vifast-core` phải chờ. `KT-345b`
  (`free`/`df`/`loop` + `NetSvc` đa caller) mở **sau** `KT-345a`, `KT-342`, `KT-343`.

---

## 2. Nhóm 1 — `spec/kernel.toml` (chưa tồn tại)

### 2.1 Hiện trạng

`spec/` hiện chỉ có **hai** tệp (kiểm bằng `ls spec/`): `abi.toml`, `v1-parity.toml`. `spec/kernel.toml`
**không tồn tại**, nhưng đã được dẫn như nguồn sự thật ở **năm** chỗ:

```text
spec/abi.toml:9
# §3.9 (slot 48 B), §4.22 (kernel.toml); code v1 để lấy quy ước gọi riscv64:
```

```text
spec/v1-parity.toml:6
# `docs/v1/` là đọc-only; tệp này KHÔNG sửa ABI (`abi.toml`/`kernel.toml` giữ nguyên).
```

```text
docs/v2/ViFastOs-v2-thiet-ke.md:508   (§4.22 KernelConfig)
Khai báo trong `spec/kernel.toml`, sinh thành hằng số Rust: `MAX_HARTS` (mặc định **8**, trần **64** [TK]),
kích thước trace ring, kích thước hộp thư, số refill tối đa, định nghĩa cụm, feature (debug, test, kpti).
**Trace luôn bật, không có cờ tắt** (D2-09). Sim và cả 3 platform đọc cùng tệp; kịch bản sim có thể ghi đè.
```

```text
docs/v2/ViFastOs-v2-thiet-ke.md:680   (cây thư mục §8)
  spec/          abi.toml, kernel.toml, deps.toml, protocols/*.vidl, manifests/*.toml
```

```text
docs/v2/ViFastOs-v2-thiet-ke.md:870   (K0 — DoD "Spec đầy đủ")
| K0 | Spec đầy đủ (`abi.toml` …; `kernel.toml`; `deps.toml`), … | `./v2/bin/v2 gate --fast` chạy; Agent 0 duyệt spec (KT-04d APPROVED) |
```

`AGENTS.md` §1 cũng đã liệt `spec/abi.toml, spec/kernel.toml` là "một nguồn sự thật duy nhất cho ABI".

**Vì sao chưa có:** bốn quyết định liên tiếp **hoãn** nó — D2-53 Q3 (K1c), D2-65 Q1 (K2), D2-72 Q1 (K3),
D2-195 Q3 (K4' bước 3). Hệ quả: hằng số sống rải rác, **năm bản** của cùng một con số `MAX_HARTS`:

```text
v2/kernel/vifast-core/src/machine.rs:64          pub const MAX_HARTS: usize = 8;
v2/kernel/vifast-plat-common/src/smp.rs:31       pub const MAX_HARTS: usize = 8;
v2/kernel/vifast-plat-x86_64/src/smp.rs:54       pub const MAX_HARTS: usize = 8;
v2/kernel/vifast-plat-aarch64/src/smp.rs:48      pub const MAX_HARTS: usize = 8;
v2/sim/vifast-sim/src/kobj.rs:150                pub const MAX_HARTS: u16 = 8;
```

```text
v2/kernel/vifast-core/src/mailbox.rs:24          pub const MAILBOX_CAP: usize = 8;
v2/sim/vifast-sim/src/mailbox.rs:28              pub const MAILBOX_CAP: usize = 8;
```

và **hai** giá trị khác nhau cho cùng **tên** `MAX_OBJECTS` ở **hai tầng** (đây không phải lỗi, nhưng chứng
minh `kernel.toml` phải có **phạm vi** chứ không chỉ giá trị):

```text
v2/kernel/vifast-core/src/machine.rs:42          pub const MAX_OBJECTS: usize = 256;   // bảng object của model
v2/kernel/vifast-plat-common/src/obj/mod.rs:74   pub const MAX_OBJECTS: usize = 96;    // bảng object của nền tảng
```

```text
v2/sim/vifast-sim/src/kobj.rs:155-158
/// Số bucket của bảng băm WaitOnAddress — hằng số **model** `[TK]` (D2-53 Q3; K1C-PLAN §5
/// KT-28 DoD 1). `spec/kernel.toml` **chưa tồn tại** (K3), nên ngưỡng bucket ghi ở
/// `v2/docs/k1c-waitaddr.md` §1 chứ **không** phải `spec/`.
```

### 2.2 Vấn đề

1. **Nguồn sự thật bị tuyên bố trước khi tồn tại.** `spec/v1-parity.toml:6` nói "`kernel.toml` giữ nguyên"
   trong khi tệp không có — người mới đọc sẽ tưởng đã có và **không** tạo. K0 (thiết kế:870) ghi DoD đã
   đạt trong khi `kernel.toml` chưa từng tồn tại.
2. **Năm bản `MAX_HARTS`** chỉ được giữ khớp bằng **một** assert biên dịch
   (`v2/kernel/vifast-kbin-aarch64/src/main.rs:119`) + **hai** test host `assert!(MAX_HARTS == 8)`
   (`plat-common/src/smp.rs:373`, `plat-x86_64/src/smp.rs:501`); x86_64 chỉ có **comment** mô tả assert
   (`plat-x86_64/src/smp.rs:52-54`). Thêm arch thứ tư hoặc đổi trần là **sửa 5 tệp**.
3. **`DebugGetInfo.features` là mặt ABI nhưng chưa có bit-map ở đâu cả.** Spec chỉ nói trả `features`
   (`spec/abi.toml:158,170-172`), không nói bit nào nghĩa gì. Nhóm 4 và nhóm 6 sẽ cần đúng bit-map này
   nếu Agent 0 chọn phương án "thêm năng lực" — nên `kernel.toml` là chỗ tự nhiên để khai **tên** feature
   (giá trị số để sau, khi có năng lực thật).
4. **Ca thật:** KT-28 và KT-29 đã phải ghi "hằng model `[TK]` trong `v2/docs`, **không** tạo
   `spec/kernel.toml`" (D2-53 Q3) — tức mỗi task lại phải quyết định lần nữa; D2-195 Q3 lặp lại lần thứ tư.

### 2.3 Phương án

#### 1A — Tạo **đầy đủ** §4.22 ngay, sinh hằng, đổi consumer

- **`spec/kernel.toml`**: đủ 6 mục §4.22: `max_harts`, `trace_ring_bytes`, `mailbox_bytes`, `max_refills`,
  `cluster_defs`, `features` (debug/test/kpti) + trace-always-on.
- **`vifast-gen`**: thêm target `v2/lib/vifast-kconfig/src/generated.rs`; `gen.py --kernel`; `gen-check` mới.
- **3 arch**: `vifast-core`, `plat-common`, 3 `plat-*`, sim **cùng** đọc một crate sinh; xoá 6 bản trùng.
- **Chi phí di cư**: cao — 7 crate đổi nguồn hằng, mỗi ảnh phải build lại, 2 `const _: ()` assert đổi thành
  kiểm tự sinh; `trace_ring_bytes`/`refill`/`cluster` **chưa có mã** ⇒ phải **bịa giá trị** trước khi đo.
  Rủi ro: giá trị bịa thành "chuẩn" không đo được.
- `min_compat`: không đổi (không phải wire). `abi_version`: giữ 1.

#### 1B — **Không** tạo; giữ hằng trong crate + `v2/docs`

- **`spec/`**: không đổi. Ghi lại quyết định "hoãn tới Chu kỳ 3" bằng D2 mới; sửa `spec/v1-parity.toml:6`
  thành "`kernel.toml` **chưa có**" (một dòng, KT-345a làm khi được phép).
- **3 arch**: không đổi. **`vifast-gen`**: không đổi.
- **Chi phí**: 0 bây giờ; **nợ** vẫn nguyên: 6 bản `MAX_HARTS`, K0 vẫn không đạt, và mỗi task sau lại phải
  tự quyết lại (đúng cái đã xảy ra 4 lần).
- `min_compat`/`abi_version`: không đổi.

#### 1C — Tạo **tối thiểu** + máy kiểm, **chưa** sinh hằng, **chưa** đổi consumer  ← *khuyến nghị*

- **`spec/kernel.toml`** (mới) chỉ chứa thứ **đã có quyết định hoặc đã có trong mã**, mỗi mục ghi `source`:
  - `[limits]`: `max_harts = 8` (D2-72 Q4, D2-195 Q5 — có trong 5 tệp mã), trần `max_harts_hard = 64` [TK].
  - `[limits.core]` / `[limits.plat]`: `max_objects` **theo phạm vi** (256 model / 96 nền tảng), `max_tcbs`,
    `max_endpoints`, `max_replies`, `max_notifications`, `max_caps` — mỗi giá trị kèm `source = "đường-dẫn:dòng"`.
  - `[mailbox]`: `cap = 8`, `max_mail_log = 32` (hai bản: core + sim).
  - `[features]`: **tên** feature đang có trong Cargo (`debug`, `test`, `product-shell`, `product-fs`, …) +
    `trace_always_on = true` (D2-09); **chưa** gán bit cho `DebugGetInfo.features` — để `[reserved]`.
  - `[arch.x86_64.gdt]`: hình học GDT **hiện tại** (nhóm 5) — 7 entry, selector, `sysret_ready = false`.
  - `[reserved]`: các khoá §4.22 **chưa quyết** (`trace_ring_bytes`, `max_refills`, `cluster_defs`,
    `kpti`, `features.bits`) ⇒ **cấm** generator sinh, `check_kernel.py` báo "reserved" nếu ai đó sinh.
- **Máy kiểm**: `v2/test/kernel/check_kernel.py` (mới) đối chiếu **spec ↔ mã** bằng regex trên các tệp đã
  liệt ở §2.1; lệch ⇒ đỏ. Đây là kiểu kiểm **đã có tiền lệ**: `check_abi.py` cross-check `generated.rs`.
- **`vifast-gen`**: *chưa* thêm target (tránh "file cấu hình không ai dùng"); target `vifast-kconfig` để
  task di cư sau (ghi nợ, chủ: KT-334/KT-336 hoặc task riêng).
- **3 arch**: **không đổi mã**. Bước cổng mới phải chọn `lane_step` ID **sau KT-333** (§1.6).
- **Chi phí di cư**: thấp (một tệp spec + một checker + một bước cổng). Không có giá trị bịa.
- `min_compat`/`abi_version`: không đổi. **`vifast-gen` không đổi** trong gói này.

### 2.4 Đánh giá

| Tiêu chí | 1A (đầy đủ) | 1B (không tạo) | **1C (tối thiểu)** |
|---|---|---|---|
| Chấm dứt "5 nguồn dẫn một tệp không có" | Có | Không | **Có** |
| Chống lệch `MAX_HARTS` 5 bản | Có (tự sinh) | Không | **Có** (checker đỏ khi lệch) |
| Bịa giá trị chưa đo | **Có** | Không | **Không** |
| Đụng `v2/kernel`/`v2/lib` trong KT-345a | Có | Không | **Không** |
| Công sức KT-345a (2 h) | Vượt | 0 | **Vừa** |
| Nợ còn lại | Thấp | Cao | **Trung bình** (di cư generator sau) |

### 2.5 Khuyến nghị

**1C.** Lý do: (a) tệp đã được dẫn như nguồn sự thật ở 5 chỗ và là DoD K0 — tiếp tục hoãn là tự tạo tài
liệu sai; (b) chỉ ghi giá trị **đã có quyết định hoặc đã có trong mã** nên không bịa số; (c) checker
spec↔mã giải quyết đúng vấn đề "5 bản `MAX_HARTS`" mà **không** phải sửa `v2/kernel` (luật Đợt 0);
(d) để `vifast-gen --kernel` lại: một file cấu hình không có consumer sinh ra là nguồn sự thật thứ hai,
đúng thứ D2-11 muốn tránh.

### 2.6 Kịch bản kiểm (không phải `.scn` runtime — đây là cấu hình, không có bước sim)

```text
# Kịch bản kiểm cho nhóm 1 (KT-345a dựng; KHÔNG chạy QEMU)
# scope=v2/test/kernel/check_kernel.py --selftest
1. DƯƠNG: spec/kernel.toml parse được; mọi khoá [limits]/[mailbox] có `source` trỏ tới tệp:dòng tồn tại.
2. DƯƠNG: max_harts trong spec == 8 == cả 5 hằng trong mã (regex từng tệp ở §2.1).
3. ÂM-1: sửa max_harts = 16 trong bản sao spec  -> exit 1, in "lệch vifast-plat-common/src/smp.rs:31".
4. ÂM-2: xoá một `source` -> exit 1 ("thiếu source").
5. ÂM-3: thêm khoá vào [reserved] rồi bật generator -> exit 1 ("reserved không được sinh").
6. ÂM-4: trỏ source vào tệp không tồn tại -> exit 1 ("source không tồn tại").
```

---

## 3. Nhóm 2 — `extra_caps_unwrapped` trong `MessageInfo` (đã có chủ)

### 3.1 Hiện trạng

```text
spec/abi.toml:39-48
# MessageInfo nằm gọn trong MỘT thanh ghi (thiết kế §4.6): 20+3+1+4+7 = 35 bit.
[message_info]
bits = 35
label_bits = 20
caps_bits = 3                 # ≤ 4 cap
no_reply_bits = 1
extra_caps_unwrapped_bits = 4
length_bits = 7               # ≤ 64 MR
# Thứ tự bit trên thanh ghi (bit thấp trước) — [TK], chờ duyệt ở KT-04d.
layout = "label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34"
```

Trường `24..27` **đã có chủ từ KT-63**: nó là **số cap đã unwrap** (badge OR vào message), do
`deliver_caps` ghi. Đây là kết luận khảo sát của KT-309, không phải suy đoán:

```text
v2/agent/JOURNAL/kt309-captransfer.md:34
| Vậy `extra_caps_unwrapped` đang **dùng** cho gì? | **Đã cài** từ KT-63: nó là **số cap đã unwrap**
| (badge OR vào message), do `deliver_caps` ghi (`with_extra_caps_unwrapped(plan.unwrapped())`).
```

và mã hiện tại pin đúng nghĩa đó:

```text
v2/kernel/vifast-core/src/ipc.rs:305   pub fn extra_caps_unwrapped(self) -> u8
v2/kernel/vifast-core/src/ipc.rs:325   self.extra_caps_unwrapped = extra;
v2/kernel/vifast-core/src/core_capxfer.rs:120   .with_extra_caps_unwrapped(plan.unwrapped() as u8)?;
```

Số **extra caps** (số cap > 4 gửi qua IPC buffer, KT-309) hiện **không** nằm trong `MessageInfo`:

```text
v2/kernel/vifast-core/src/ipc.rs:60-85
/// Số **extra cap** tối đa của một message — `spec/abi.toml` `[message].max_caps = 4` …
pub const MAX_EXTRA_CAPS: usize = MAX_CAPS;
/// Byte offset của vùng **extra caps** trong IPC buffer — `[TK]` **KT-309**, ngay sau vùng
/// MR4..MR63 …
/// byte 560..568 : n (số extra cap, 0..=4)          — caller ghi trước Call/Send
/// byte 568..600 : n CPTR nguồn (CSpace caller)     — caller ghi
/// byte 600..608 : dst slot (CSpace receiver, 0=auto) — receiver ghi trước Recv
```

```text
v2/agent/JOURNAL/kt311-cfgfull.md:32
| Còn chỗ trong **4 MR** không? | … `MessageInfo` 35 bit cũng không còn trường trống (D2-99:
  `label`/`caps`/`no_reply`/`extra_caps_unwrapped`/`length` đều có chủ) |
```

### 3.2 Vấn đề

1. **Không có trường ABI cho số extra caps.** Nó nằm ở `byte 560` của IPC buffer, do `[TK]` của KT-309 quy
   định — nghĩa là **hợp đồng** giữa stub U-mode và kernel chỉ tồn tại trong mã, không có trong `spec/`.
   Đổi offset/độ rộng ở một phía là **im lặng sai**: kernel đọc `n` ở 560, stub ghi ở 568 ⇒ gửi 0 cap mà
   không lỗi.
2. **Ca thật:** KT-309 phải "ẩn" `n` vào buffer vì không còn bit; KT-311 lặp lại kết luận đó; KT-309 §7.1
   ghi rõ **"nếu muốn `extra_caps` thành trường ABI thật thì phải mở `spec/`"** — đúng việc của gói này.
3. **Không thể mở rộng > 4 cap** mà không có chỗ khai số (hiện `MAX_EXTRA_CAPS = MAX_CAPS = 4`).
4. **Nghịch lý đặt tên:** trường duy nhất "trông như" chỗ chứa extra caps lại là unwrap count; bất kỳ ai
   đọc `spec/abi.toml:45` mà không đọc KT-63 đều có thể gán sai nghĩa (chính tiền đề phiếu KT-309 đã sai
   như vậy).

### 3.3 Phương án

#### 2A — Thêm trường mới **`extra_caps: u3`** vào các bit **35..37** (bit trống)

- **`spec/abi.toml`**: `bits = 38`; thêm `extra_caps_bits = 3`; `layout = "…, length:28..34, extra_caps:35..37"`.
  `extra_caps_unwrapped` **giữ nguyên** `24..27`.
- **Mã**: `MessageInfo` thêm trường; `pack/unpack` đổi `INFO_BITS` 35→38, `INFO_MASK`; mọi chỗ dựng message
  giữ mặc định 0; `MAX_EXTRA_CAPS` vẫn 4 nhưng **số** đọc từ thanh ghi (buffer chỉ còn CPTR + dst).
- **`vifast-gen`**: hiện **không** sinh `[message_info]` ⇒ phải thêm hằng `MESSAGE_INFO_BITS`/`LAYOUT` (đóng
  lỗ hổng D2-11) — công thêm.
- **`min_compat`**: 1 (kernel mới nhận message cũ vì bit 35..37 = 0). **`abi_version`**: **2** (layout đóng
  băng (2) bị mở rộng; chương trình mới trên kernel cũ bị `InvalidArgument` vì bit ngoài 35).
- **3 arch**: không đổi thanh ghi (MessageInfo vẫn 1 thanh ghi); chỉ đổi hằng/struct trong `vifast-core` +
  3 stub U-mode (nếu stub tự pack) + sim. Chi phí **trung bình–cao**; rủi ro: đụng đúng thứ đóng băng (2).

#### 2B — **Gán lại** `24..27` thành số extra caps, chuyển unwrap đi nơi khác

- **`spec/abi.toml`**: đổi tên trường + nghĩa; `layout` đổi; hoặc bỏ unwrap khỏi `MessageInfo`.
- **`min_compat`**: **phải** nâng (chương trình cũ đọc `24..27` là unwrap sẽ hiểu thành số cap). `abi_version`: 2.
- **3 arch**: sim pin nghĩa unwrap ở `kt25_ipc2.rs:486`, `kt63_core_cap_ipc.rs:249,280` ⇒ **đỏ** hàng loạt.
- **Chi phí**: cao + phá bằng chứng KT-63. **Không khuyến nghị** — đây là phương án "dễ viết, đắt thi hành".

#### 2C — **Đóng băng layout vùng extra caps của IPC buffer** (`[ipc_buffer]`), **không** đụng `MessageInfo`  ← *khuyến nghị*

- **`spec/abi.toml`**: thêm mục mới (không đổi dòng nào của `[message_info]`):

  ```toml
  [ipc_buffer]
  bytes = 4096
  align = 16
  # header 0..80: magic + MessageInfo bản ghi + receive window 4 CPTR + send caps 4 CPTR (KT-67b)
  mr_offset = 80
  mr_words = 60
  # extra caps (KT-309): n + 4 CPTR + dst
  extra_caps_offset = 560
  extra_caps_max = 4
  extra_caps_dst_offset = 600
  # configure (KT-311/KT-320): cspace, vspace, entry, priority — giữ [TK], CHƯA đóng băng
  configure_offset = 608
  note = "Chỉ vùng extra caps là ABI; vùng configure còn [TK] tới khi KT-320 ổn định."
  ```

- **Ý nghĩa ABI:** `n` (0..=4) ở `560..568`; `n` CPTR nguồn ở `568..600`; `dst` ở `600..608`. Không đổi một
  byte nào đang chạy — chỉ **ghi lại** hợp đồng đang dùng.
- **`min_compat`**: 1 (byte không đổi). **`abi_version`**: 1.
- **`vifast-gen`**: sinh offset/hằng từ `[ipc_buffer]` (thay dần 13 hằng `IPC_BUFFER_*` trong `ipc.rs`);
  docs `/docs/abi.md` thêm mục "IPC buffer". Không sinh struct mới.
- **3 arch**: arch đọc buffer (`plat-riscv64/src/ipc_buffer.rs`, tương lai aarch64/x86_64) dùng hằng sinh;
  user stub 3 arch cũng vậy. Chi phí **thấp**, làm dần được (gói này chỉ đóng băng offset extra caps).
- **Giới hạn:** vẫn `max_caps = 4`; muốn > 4 phải mở rộng `extra_caps_max` + `[message].max_caps` (việc khác).

### 3.4 Đánh giá

| Tiêu chí | 2A (trường mới) | 2B (gán lại) | **2C (buffer ABI)** |
|---|---|---|---|
| Đụng 5 thứ đóng băng | **Có** (2) | **Có** (2) | Không |
| `abi_version` | 2 | 2 | **1** |
| Chương trình cũ chạy kernel mới | Được (bit = 0) | **Không** | Được |
| Chương trình mới chạy kernel cũ | Lỗi rõ (`InvalidArgument`) | Lỗi rõ | Được (cùng byte) |
| Sửa `v2/kernel` | Có (core + 3 arch + sim + stub) | Có | **Không bắt buộc ngay** |
| Công sinh hằng của `vifast-gen` | thêm mới | thêm mới | tái dùng |
| Phá bằng chứng KT-63/KT-309 | Không | **Có** | Không |

### 3.5 Khuyến nghị

**2C.** Lý do: (a) số extra caps **đã** đi trong buffer — đóng băng đúng chỗ đang chạy là thay đổi **không
byte**, không đụng `message_layout`; (b) `MessageInfo` 35 bit là thứ đóng băng số (2) và đã **hết chỗ**
(KT-311) — mở rộng nó là đổi ABI major cho một con số 3 bit; (c) 2C còn **đóng lỗ hổng sinh hằng** của
`vifast-gen` (hiện `[message_info]` không được sinh) theo hướng D2-11.

Nếu Agent 0 **muốn** trường nằm trong thanh ghi (lý do hợp lệ: arch không có IPC buffer vẫn biết số cap),
thì chọn **2A** và chấp nhận `abi_version = 2` + sửa 2 kỳ vọng đã liệt ở §0.

### 3.6 `.scn` nháp

```toml
# ĐỀ XUẤT — chưa thêm vào v2/test/scn (check_scn.py đang đòi đúng 27 tệp; KT-345a chọn số trống + tag).
# Kiểm: số extra caps đi qua vùng 560 và biên max_caps = 4 KHÔNG bị nới.
version = 1
name = "28-extra-caps-count"
tags = ["four-cap"]
caps = { u0 = "Untyped", c0 = "CNode", c1 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
thread = [
  { name = "t0", steps = [
    "call cap=ep1 extra_caps=0 => ok",
    "call cap=ep1 extra_caps=2 => ok",
    "call cap=ep1 extra_caps=4 => ok",
    "call cap=ep1 extra_caps=5 => InvalidArgument",
  ] },
]
```

```text
# Kịch bản kiểm bổ sung (host, không cần QEMU) — scope=ipc_buffer extra caps
1. DƯƠNG: ghi n=2 + 2 CPTR + dst=6 vào buffer; kernel đọc đúng n=2, cài 2 cap vào slot 6,7.
2. ÂM: n=5 -> InvalidArgument (5) TRƯỚC khi cài cap nào (không cài một phần).
3. ÂM: n=2 nhưng CPTR thứ 2 rỗng -> InvalidCapability (1), không cài cap thứ 1.
4. ÂM: dst=0 (auto) + không còn slot trống -> SlotOccupied (7).
5. Đột biến: đổi offset extra_caps 560 -> 568 trong bản sao hằng sinh -> ca 1 ĐỎ.
```

---

## 4. Nhóm 3 — Kênh lỗi cho bảng 8 `VSpace` và cho ô 0 `CSpace`

### 4.1 Hiện trạng

**(a) Hook arch trả `bool`, bị vứt bỏ.** `Tcb.Configure` gọi hook sau khi đã ghi `TcbData.vspace`, rồi
**không** dùng kết quả:

```text
v2/kernel/vifast-core/src/invoke.rs:562-575
// **KT-320** — giao **VSpace** đã cấu hình cho arch (D2-83: `satp`/`PTE` thuộc arch; core chỉ
// giữ danh tính). Cùng khuôn `SyscallEnv::tcb_resume` (KT-314): hook **giữ** giá trị, arch
// **tiêu thụ** ở lần vào U-mode kế tiếp của đúng TCB. Nền tảng không có U-mode trả `false`.
…
if params.vspace != crate::core_tcb::VspaceUpdate::Keep {
    if let Ok(vspace) = m.tcb_vspace(c.obj) {
        let _ = env.tcb_vspace(c.obj, vspace);
    }
}
bump(&CONFIGURED);
bump(&ALLOWED);
Ok((0, 0))
```

```text
v2/kernel/vifast-core/src/syscall.rs:238
fn tcb_vspace(&mut self, _tcb: ObjId, _vspace: ObjId) -> bool {
    false
}
```

Bảng của arch có **trần cứng** và **fail-closed**, nhưng "closed" chỉ tới mức trả `false`:

```text
v2/kernel/vifast-plat-riscv64/src/user_run.rs:155-159
/// Số khe của **hai** bảng dưới — **trần rõ ràng**: `user::MAX_PROCS` (6) ô tác vụ U-mode + 2 khe
/// dự phòng, đủ cho mọi tác vụ của một ảnh có khe riêng. Bảng **tràn** ⇒ **fail-closed**: không
/// ghi đè lên TCB khác (`arm_vspace` trả `false`; `vspace_applied` trả `None` — TAP đỏ, không
/// xanh oan). Không cấp phát động, không đụng `Machine` (pin D2-84 không đổi).
const VSPACE_SLOTS: usize = user::MAX_PROCS + 2;
```

```text
v2/kernel/vifast-plat-riscv64/src/user_run.rs:214
/// Trả `true` ⇔ đã giữ được (hook của `SyscallEnv`); bảng **tràn** ⇒ `false` (fail-closed).
```

**Nợ đã công bố** đúng chỗ này (nguồn đề bài):

```text
v2/agent/JOURNAL/kt325-vspace2.md §8 mục 2
2. **Bảng khe 8** (`.bss` 256 B) — tràn ⇒ fail-closed: `arm_vspace` trả `false` (hook `false`) và
   `vspace_applied` trả `None`. Ảnh hiện dùng **3** khe (T1/T2/T0). Hook **không** có kênh lỗi về
   U-mode (nợ KT-320 giữ nguyên, ngoài phạm vi KT-325) ⇒ Configure vẫn trả `(0,0)` dù arch không
   giữ được; TAP/`vspace_applied` sẽ đỏ nếu điều đó xảy ra trong ảnh này.
```

**(b) Ô 0 `CSpace` — không có một dòng nào trong spec.** `grep -n "cspace\|CSpace" spec/*.toml` chỉ ra **hai**
dòng, **không** dòng nào nói về ô 0: `spec/abi.toml:212` (chú thích `Untyped.Retype`) và `:248` (tham số
`cspace?`). Luật thật đang nằm trong **mã**, dưới dạng chú thích của KT-325:

```text
v2/kernel/vifast-core/src/invoke.rs:529-533
// **KT-325** — CPTR **rỗng** (`0`) = **xoá** `vspace?` ⇒ về "dùng VSpace của ảnh đã nạp"
// (`TcbData.vspace = 0`). `0` **không** phải phép tra ô 0: ô 0 của CSpace thường là cap `CNode`
// (`CS_SELF`) nên nếu tra sẽ ra `WrongObjectType` 2 — đây là **ngữ nghĩa mới có chủ đích** (task
// KT-325), không phải nới ngưỡng; CPTR **khác 0** mà không phải `VSpace` **vẫn** ⇒ 2.
```

Còn `cspace?` thì CPTR 0 **được** giải như ô 0:

```text
v2/kernel/vifast-core/src/invoke.rs:516-523
if mask & crate::ipc::CONFIGURE_MASK_CSPACE != 0 {
    let cptr = usize::try_from(extra.cspace)…;
    if cptr >= m.cnode_slot_count(cspace)… { … }
    params.cspace = Some(SlotRef::new(cspace, cptr));
}
v2/kernel/vifast-core/src/core_tcb.rs:297-298
let cspace_obj = match params.cspace {
    Some(slot) => Some(self.require_type(slot, ObjType::CNode)?),
```

⇒ `cspace? = 0` **hợp lệ** và đặt CSpace của TCB đích = **CNode gốc của caller** (`CS_SELF`). Trong khi
`vspace? = 0` là **xoá**. Hai tham số cùng một ngữ cảnh, cùng giá trị 0, **hai nghĩa khác nhau**, và **không
có mã lỗi** nào phân biệt "tôi muốn ô 0" với "tôi muốn xoá".

### 4.2 Vấn đề

1. **Arch không giữ được nhưng U-mode được báo thành công.** `Configure` trả `(0,0)` trong khi
   `TcbData.vspace` đã trỏ VSpace mà kernel **không** có khe cho nó. Lượt vào U-mode kế tiếp dùng
   VSpace của ảnh đã nạp ⇒ chạy sai địa chỉ, **không** có lỗi nào tới chương trình giám sát. Trong ảnh
   hiện tại TAP bắt được (vì TAP đọc `vspace_applied`), nhưng **sản phẩm** thì không có TAP.
2. **Không có `KError` nào biểu diễn "arch hết khe".** `Busy` (14, "tài nguyên đang bận") và
   `NotEnoughMemory` (9) đều có trong bảng (`spec/abi.toml:129,134`) nhưng **chưa** được gán nghĩa cho
   đường này; `[method_signature."Tcb.Configure"]` chỉ có `params`/`returns`, **không** khai tập lỗi.
3. **Ô 0 là "vùng mù" của spec.** Một chương trình đặt `VSpace` ở ô 0 (đúng luật CSpace) sẽ **không bao giờ**
   truyền được nó qua `vspace?`; nếu nó truyền `0` với ý "ô 0" thì kernel **xoá** VSpace và trả `ok` — sai
   lặng. Ngược lại `cspace? = 0` lại hợp lệ. Không có tài liệu ABI nào nói điều này.
4. **Tương lai gần:** KT-338/KT-340 (aarch64/x86_64 nối `vifast-core`) sẽ có `SyscallEnv` thật cho 2 arch
   nữa ⇒ nếu không có kiểu trả 3 trạng thái, mỗi arch phải tự phát minh cách báo lỗi.

### 4.3 Phương án cho **(a) bảng `VSpace`**

#### 3.1A — Hook **ba trạng thái** + `Busy` (14) về U-mode  ← *khuyến nghị*

- **`spec/abi.toml`**: thêm vào `[method_signature."Tcb.Configure"]`:

  ```toml
  [method_signature."Tcb.Configure"]
  params = ["tcb", "cspace?", "vspace?", "entry?", "ipc_buffer?", "priority?"]
  returns = []
  # ĐỀ XUẤT: tập lỗi tường minh + nghĩa của `Busy`
  errors = ["InvalidCapability", "WrongObjectType", "InsufficientRights", "InvalidArgument", "Busy"]
  note = "`Busy` (14) = nền tảng KHÔNG giữ được VSpace (bảng khe arch đầy) — fail-closed, KHÔNG ghi TcbData.vspace. Nền tảng không có U-mode: coi là Unsupported, vẫn trả ok (tương thích)."
  ```

- **Mã**: `SyscallEnv::tcb_vspace` đổi `bool` → `enum VspaceOutcome { Applied, Unsupported, Failed(KError) }`;
  `invoke::configure` đọc kết quả: `Failed(e)` ⇒ **rollback** `TcbData.vspace` về giá trị trước + trả `e.id()`;
  `Unsupported`/`Applied` ⇒ `(0,0)` như cũ. (Rollback bắt buộc: nếu không, vẫn lệch core/arch.)
- **`min_compat`**: 1. **`abi_version`**: 1 (wire `(error, value)` không đổi; chỉ thêm giá trị lỗi trong
  nhánh thất bại — chương trình cũ chỉ thấy `ok` như trước ở mọi nhánh thành công).
- **`vifast-gen`**: thêm cột "Lỗi" vào bảng `docs/abi.md` (đọc `errors` nếu có); không đổi `generated.rs`.
- **3 arch**: riscv64 đổi impl + test; aarch64/x86_64 **chưa** có `SyscallEnv` thật ⇒ dùng mặc định
  `Unsupported` (không phải `false` ⇒ không phá ảnh cũ); sim đổi `FakeEnv`/`NoEnv`.
- **Chi phí**: trung bình (trait + 1 impl thật + 3 test env + 3 ca âm). Rủi ro: nếu quên rollback thì core
  và arch lệch — phải có đột biến "bỏ rollback ⇒ ĐỎ".

#### 3.1B — **Loại bỏ** khả năng tràn (không cần kênh lỗi)

- **`spec/abi.toml`**: không đổi.
- **Mã**: bảng arch cấp theo `MAX_TCBS` thay `MAX_PROCS + 2` (mọi TCB đều có khe) — tràn **không biểu diễn
  được**; hoặc chuyển root/asid vào `TcbData` (core giữ, arch đọc) nếu ngân sách `Machine` cho phép.
- **`min_compat`/`abi_version`**: không đổi. **`vifast-gen`**: không đổi.
- **3 arch**: chỉ riscv64 bây giờ (`.bss` ảnh tăng — **phải đo lại** trần ảnh, không viết tay số).
- **Chi phí**: thấp về mã, **trung bình** về đo đạc; nhược điểm: vẫn còn `bool` hook (không có chẩn đoán),
  và nếu `.bss`/`Machine` không đủ thì quay lại 3.1A.

#### 3.1C — **Giữ nguyên** (`bool` bị vứt) + chỉ dựa TAP

- Không đổi gì. **Chi phí 0**, nhưng sản phẩm **im lặng** khi arch hết khe; nợ KT-325 §8.2 vẫn mở. Chỉ chấp
  nhận nếu Agent 0 tuyên bố "ảnh sản phẩm không bao giờ dùng quá 3 khe" — cần một bất biến và test canh.

### 4.4 Phương án cho **(b) ô 0 `CSpace`**

#### 3.2A — Ghi **luật ô 0** vào spec; giữ `0 = vắng/xoá`  ← *khuyến nghị*

- **`spec/abi.toml`**: thêm mục mới, **không** đổi byte:

  ```toml
  [cspace]
  # Ô 0 là cap `CNode` của chính CSpace (CS_SELF) — do kernel cài lúc tạo, KHÔNG chuyển nhượng.
  slot0 = "CS_SELF"
  # CPTR trong tham số method: 0 = "vắng mặt"; với `vspace?` 0 = "xoá" (KT-325).
  # VSpace/Frame đặt ở ô 0 KHÔNG truyền được qua CPTR; phải `CNode.Move` sang ô khác trước.
  cptr_zero = "absent"
  note = "Ghi lại ngữ nghĩa đang chạy (KT-325); không đổi wire."
  ```

- **`min_compat`/`abi_version`**: 1/1. **`vifast-gen`**: thêm mục "CSpace" vào `docs/abi.md`; không sinh mã.
- **3 arch**: không đổi mã. **Chi phí**: rất thấp (một mục spec + 1 rule checker + docs).
- **Hạn chế**: không sửa được **bất tiện** "không truyền được cap ở ô 0" — chỉ làm nó **tường minh**.

#### 3.2B — Thêm **bit hiện diện** + bit `CLEAR` riêng, bỏ "0 = xoá"

- **`spec/abi.toml`**: ghi rõ mask `Tcb.Configure` có thêm bit `VSPACE_CLEAR`; `vspace?` CPTR 0 = **ô 0**
  khi bit `VSPACE` bật (⇒ `WrongObjectType` 2 nếu ô 0 là CNode), xoá chỉ khi bit `VSPACE_CLEAR` bật.
- **Mã**: đổi `CONFIGURE_MASK_*`, `invoke::configure`, `VspaceUpdate`; đổi **mọi** caller dùng "0 = xoá"
  (KT-325/`ktest_ipc` TAP 8, userland, test core) ⇒ **di cư rộng**.
- **`min_compat`**: 1 (chương trình cũ không dùng bit mới; nhưng nếu nó truyền `vspace=0` với bit `VSPACE`
  thì hành vi **đổi** từ xoá → lỗi ⇒ **phải** coi là thay đổi ngữ nghĩa; cân nhắc `min_compat = 2`).
- **`vifast-gen`**: sinh bit mask; docs cập nhật. **3 arch**: riscv64 + 3 stub + sim.
- **Chi phí**: cao; **không khuyến nghị trong v1.1** (đây là ứng viên cho v1.2 khi có nhu cầu thật).

#### 3.2C — Chỉ ghi vào `v2/docs`, không vào spec

- **`spec/`**: không đổi; luật ô 0 ở `v2/docs/k1c-ipc-semantics.md`. **Chi phí 0**; nhược điểm: `spec/` vẫn
  không phải nguồn sự thật cho CPTR, vi phạm D2-11 về tinh thần; người viết userland vẫn phải đọc docs.

### 4.5 Đánh giá

| Tiêu chí | 3.1A (3 trạng thái) | 3.1B (bỏ tràn) | 3.1C (nguyên trạng) | 3.2A (luật ô 0) | 3.2B (bit CLEAR) | 3.2C (docs) |
|---|---|---|---|---|---|---|
| Sản phẩm biết lỗi | **Có** | Không cần | **Không** | — | — | — |
| Đụng wire | Không | Không | — | Không | **Có** (nghĩa `0`) | — |
| Đụng `Machine`/`.bss` | Không | **Có** (đo lại) | Không | Không | Không | Không |
| Sửa `v2/kernel` (trait `SyscallEnv` + impl + test) | Có | Có | Không | Không | Có | Không |
| Công sức | Trung bình | Trung bình | 0 | **Thấp** | Cao | 0 |
| Ghi được vào `spec/` (D2-234) | **Có** | Không | Không | **Có** | Có | Không |

### 4.6 Khuyến nghị

**3.1A + 3.2A.** Lý do: (a) 3.1A là kênh lỗi **nhỏ nhất** đi đúng wire `(error, value)` đã có, không mở
syscall/method mới; `Busy` đã có trong bảng lỗi và đúng nghĩa "tài nguyên đang bận"; (b) kiểu **3 trạng thái**
giữ được tương thích cho nền tảng **không có U-mode** (mặc định `Unsupported` thay vì `false` ⇒ không phá
ảnh cũ và test host); (c) 3.2A biến một luật đang nằm trong chú thích mã thành luật ABI **không đổi byte**,
đúng mục tiêu của cửa sổ này; (d) 3.2B đổi nghĩa giá trị 0 — quá đắt cho v1.1.

### 4.7 `.scn` nháp

```toml
# ĐỀ XUẤT — KT-345a chọn số/tag. Kiểm: (i) CPTR 0 = xoá vẫn xanh; (ii) arch hết khe -> Busy; (iii) ô 0 = CS_SELF.
version = 1
name = "29-vspace-arch-error"
tags = ["vm-map"]
caps = { u0 = "Untyped", c0 = "CNode", c1 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
thread = [
  { name = "t0", steps = [
    "invoke Tcb.Configure tcb=tcb0 vspace=0 => ok",
    "invoke Tcb.Configure tcb=tcb0 vspace=c0 => WrongObjectType",
    "invoke Tcb.Configure tcb=tcb1 cspace=0 => ok",
    "invoke Tcb.Configure tcb=tcb1 vspace=vs_da_day => Busy",
  ] },
]
```

```text
# Kịch bản kiểm bổ sung (host: KT-345a; QEMU: task nối core KT-338b/340c)
1. DƯƠNG: hook trả Unsupported (nền tảng không U-mode) -> Configure = (0,0) như trước.
2. DƯƠNG: hook trả Applied -> TcbData.vspace đúng object; arch giữ đúng (tcb, obj).
3. ÂM: hook trả Failed(Busy) -> Configure = (14,0) VÀ TcbData.vspace GIỮ NGUYÊN giá trị trước (rollback).
4. ĐỘT BIẾN: bỏ rollback -> ca 3 ĐỎ (TcbData.vspace đã đổi dù trả 14).
5. ĐỘT BIẾN: đổi `Busy` -> `NotSupported` trong mã -> ca 3 ĐỎ (đúng mã lỗi mới).
```

---

## 5. Nhóm 4 — `MemInfo` / `STATFS` cho `free` và `df`

### 5.1 Hiện trạng

v2 **không có** `MemInfo`/`STATFS` trong ABI. Dải syscall đang đóng:

```text
spec/abi.toml:141-164
# 16 syscall (0..15; D2-29 cấp 15 cho DebugGetInfo). Dải 16..31 reserved.
[syscall]
0 = { name = "Call", … note = "Endpoint: IPC; object khác: invocation (method = label), trả ngay" }
…
14 = { name = "Exit", … }
15 = { name = "DebugGetInfo", … }

[syscall_reserved]
from = 16
to = 31
note = "Dải reserved sau khi cấp 15 cho DebugGetInfo (D2-29); object chưa cài chỉ giữ số (QD §1 Q-B)."
```

`grep -n "MemInfo\|STATFS" spec/*.toml` **chỉ** ra hai dòng **lý do hoãn** trong `spec/v1-parity.toml`:

```text
spec/v1-parity.toml:277-287
[[command]]
name = 'free'
…
status = 'deferred-K7'
reason = 'can syscall MemInfo (v1) chua co tren v2; khong mo ABI trong KT-77a => K7'

spec/v1-parity.toml:289-299
[[command]]
name = 'df'
…
status = 'deferred-K7'
reason = 'can opcode STATFS/ABI FS tu shell chua co tren v2 => K7'
```

Thiết kế **cố ý bỏ** `MemInfo` của v1:

```text
docs/v2/ViFastOs-v2-thiet-ke.md:494
Bỏ hẳn so với v1: Open, Read, Write, Close, Stat, Seek, Block*, ConsoleRead, TcbSetArgs,
ModuleRelease, MemInfo/TcbInfo/TimerInfo, software IOMMU. …
```

Còn dịch vụ FS **đã có** một hợp đồng riêng, **không** phải ABI:

```text
v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs:33-47
//! # Giao thức (không mở ABI mới)
//!
//! Dùng đúng hai syscall đã có (`NB_SEND`/`NB_RECV`) trên endpoint slot
//! [`EP_FS`](crate::proc_abi::EP_FS) và endpoint reply của procsrv — **không** thêm method vào
//! `spec/abi.toml`. Một request = **một** reply 4 MR word:
//!
//! | `OP_PATH \| len<<40` … | `OP_LS …` | `OP_CAT …` | `OP_WRITE …` | `OP_TRUNC …` | `OP_STAT …` | `OP_SYNC …` |
```

### 5.2 Vấn đề

1. **Hai lệnh chặn parity.** `free`/`df` là `deferred-K7`; KT-343 chỉ đóng 8 lệnh **không cần mở ABI**
   (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`) ⇒ `free`/`df` **bắt buộc** thuộc KT-345
   (DoD: "đóng `free`/`df`"). Không có đường nào khác: shell hiện không có số liệu.
2. **Thiết kế đã bỏ `MemInfo`** ⇒ thêm lại syscall là **đi ngược thiết kế** và đụng thứ đóng băng (1).
   Nhưng *cách thay thế* mà thiết kế chỉ định — **memory server** — chưa tồn tại; `v2/` chưa có crate
   `memsrv` (kiểm `ls v2/`: `agent bin docs kernel lib sim test tools` + tệp cấu hình — **không** có `memsrv`, **không** có `user/`).
3. **`df` không cần kernel**: `vifast-fs` đã có `usage()`; thiếu đúng **một opcode** trong hợp đồng
   `[TK]` của `proc_fs` (giống `OP_STAT` đã có).
4. **`free` cần sự thật của kernel** (Untyped/arena/kernel static) — nhưng kernel **đã** phục vụ U-mode qua
   Endpoint được (KT-281/306: `net_svc`), nên không cần syscall mới để **trả lời** câu hỏi này.

### 5.3 Phương án

#### 4A — Thêm **syscall 16 `MemInfo`** (+ 17 `FsStat` nếu muốn cả `df` trong ABI)

- **`spec/abi.toml`**: thêm `[syscall].16` (và `.17`), `[syscall_reserved].from = 18`, note; cập nhật
  `[frozen].wire_syscalls` ghi rõ "mở rộng có kiểm soát".
- **`vifast-gen`**: `gen.py:33-37` **die** nếu khác `0..15` ⇒ sửa thành đọc `[syscall_reserved].from`;
  `SYSCALL_COUNT` 16→17/18; shim 3 arch thêm hằng; `docs/abi.md` đổi tiêu đề "Syscall (0..17)".
- **`check_abi.py`**: `SYSCALLS` dict + `abi_version == 1` (dòng 137) + reserved 16..31 (dòng 152) — **bắt
  buộc** sửa; `ktest.sh:650` (`abi_version=1`) **bắt buộc** sửa.
- **Tham số/trả:** chỉ 2 thanh ghi ⇒ `value` một `u64`: gói `total_kib<<32 | free_kib` (hoặc 3 lời gọi).
  **3 arch**: riscv64 cài; aarch64/x86_64 chưa có dispatch ⇒ nếu chưa cài thì phải trả `NotSupported` (18)
  hoặc `RangeError` (19) — **không** được im lặng; cần bit feature trong `DebugGetInfo.features` (xem nhóm 1).
- **`min_compat`**: 1; **`abi_version`**: **2** (đụng (1)). **Chi phí**: cao (5 chỗ như KT-281 đã phân tích
  + 2 checker + 2 kỳ vọng harness + 3 arch hoặc feature-gate).

#### 4B — Thêm **object `Memory`** (id 17) với method `Read`

- **`spec/abi.toml`**: `[object] Memory = { id = 17, stability = "core", method = { 0 = { name = "Read" } } }`;
  `[object_reserved].from = 18`; `[method_signature."Memory.Read"] params = [] returns = ["packed"]`.
- **Trả:** invocation trả `(error, value)` ⇒ một `u64` gói `total_kib<<32 | free_kib`; `free` gọi 1 lần.
- **`check_reserved.py`**: dải reserved 17..31 → 18..31; `gen.py` sinh object 17; `docs/abi.md` thêm hàng.
- **`min_compat`**: 1; **`abi_version`**: 1 (không đụng (1)) — **rẻ hơn 4A về version**, đắt hơn về "thêm
  object vào bảng lõi" (object 17 là **dải reserved**, không phải 5 thứ đóng băng).
- **3 arch**: chỉ cần dispatch invocation; arch chưa nối core trả `NotSupported` khi method 0 được gọi.
- **Chi phí**: trung bình; nhược điểm: `value` gói 2 số là hợp đồng ngầm (phải ghi vào spec), và `free` chỉ
  đọc được một lần/số.

#### 4C — **Không** đụng `abi.toml`: `free`/`df` là **protocol IDL**  ← *khuyến nghị*

- **`df`**: thêm `OP_STATFS` vào hợp đồng `[TK]` của `proc_fs` (trả `used`, `total`, `block_size` trong
  4 MR word — đúng khuôn `OP_STAT`), rồi shell in `# shell: df used=… total=… block=…`.
- **`free`**: thêm dịch vụ kernel `MemSvc` **cùng khuôn** `net_svc`/`proc_fs` trên một Endpoint (label hợp
  đồng `[TK]`), trả `total`/`free` trong MR; **không** syscall mới, **không** object mới.
- **`spec/`**: hai protocol khai bằng IDL (nhóm 6): `spec/protocols/meminfo.vidl`, `spec/protocols/fsstat.vidl`
  (nội dung nháp ở Phụ lục C). **Không** sửa `abi.toml`.
- **`v1-parity.toml`**: `free`/`df` → `status = 'active'` **sau khi** lệnh chạy thật (D2-234 cho phép vì
  không đổi ABI) — việc của KT-345b.
- **`min_compat`/`abi_version`**: 1/1. **`vifast-gen`**: `gen_idl.py` quét thêm `spec/protocols/` (nhóm 6).
- **3 arch**: dịch vụ nằm ở ảnh/ crate arch tương ứng; aarch64/x86_64 chưa có shell sản phẩm (KT-339/341)
  nên **chưa** cần; IDL stub 3 arch dùng chung.
- **Chi phí**: trung bình (mã dịch vụ + shell + TAP), **không** đụng hợp đồng đóng băng.

### 5.4 Đánh giá

| Tiêu chí | 4A (syscall) | 4B (object) | **4C (IDL, không abi.toml)** |
|---|---|---|---|
| Đụng 5 thứ đóng băng | **Có** (1) | Không | Không |
| `abi_version` | **2** | 1 | **1** |
| Sửa `gen.py` cứng 0..15 | **Có** | Không | Không |
| Sửa 2 kỳ vọng `abi_version=1` | **Có** | Không | Không |
| `free` đọc sự thật kernel | Có | Có | **Có** (dịch vụ kernel) |
| `df` đọc `vifast-fs::usage()` | Có | Có | **Có** |
| Đi ngược thiết kế (:494 bỏ `MemInfo`) | **Có** | Một phần | **Không** |
| Công sức KT-345b (3 h) | Vượt | Vượt | **Vừa** |

### 5.5 Khuyến nghị

**4C.** Lý do: (a) thiết kế:494 **đã bỏ** `MemInfo`/`TcbInfo`/`TimerInfo` có chủ đích — thêm lại syscall là
mở lại quyết định đó, không phải trả nợ; (b) `proc_fs` đã chứng minh mẫu "dịch vụ kernel qua Endpoint,
**không mở ABI mới**" chạy thật cho FS; `net_svc` chứng minh cùng mẫu cho dịch vụ mạng; (c) `free`/`df` là
**lệnh shell** — chúng cần *số liệu*, không cần *ABI ổn định vĩnh viễn*; hợp đồng IDL đủ ổn định và kiểm
được; (d) 4C giữ `abi_version = 1`, không phải sửa `gen.py`/`check_abi.py`/`ktest.sh` — đúng tinh thần
"cửa sổ ABI" là **không** mở khi không cần.

Nếu Agent 0 quyết `free` phải có ABI: chọn **4B** (object), **không** chọn 4A — vì 4B không đụng (1) và
không phải sửa `gen.py`/`SYSCALL_COUNT`.

### 5.6 `.scn` nháp

```text
# ĐỀ XUẤT — đây KHÔNG phải .scn sim: free/df là lệnh shell, cần dịch vụ runtime chứ không phải
# bước bài kiểm mô hình. Bước kiểm nằm ở parity runner + TAP của ảnh sản phẩm.
# scope=v2/test/parity/run.sh --mode serial   (KT-345b điền khi lệnh chạy thật)
```

```text
# Kịch bản kiểm thật (KT-345b), thay hai mục deferred-K7:
# scope=v2/test/parity/run.sh --mode serial
send "free\n"  -> khớp  ^# shell: free total=[0-9]+ free=[0-9]+$
send "df\n"    -> khớp  ^# shell: df used=[0-9]+ total=[0-9]+ block=[0-9]+$
CA ÂM: dịch vụ MemSvc trả lỗi -> shell in "# shell: free loi=<mã>" (không treo, không panic)
CA ÂM: FS chưa mount -> "# shell: df loi=..." ; parity vẫn đỏ nếu regex không khớp
```

---

## 6. Nhóm 5 — Thứ tự **GDT** cho `syscall`/`sysret` trên x86_64

### 6.1 Hiện trạng

Spec ghi rõ **vì sao** đang dùng `int 0x80` và **điều kiện** để nâng lên:

```text
spec/abi.toml:86-93
# Vì sao `int 0x80` chứ không `syscall`/`sysret`: `sysret` đòi user data ở `CS - 8`
# (`STAR[63:48] + 8` = SS), trong khi GDT của KT-261 đặt user code `0x18` **trước** user data
# `0x20` ⇒ `sysret` nạp SS = `0x13` (kernel data | RPL 3) và #GP. `syscall` khắc phục được phần
# đó nhưng **không** đổi stack: cần `swapgs`/per-CPU hoặc một static RSP + dựng lại frame `iretq`
# bằng tay. … Nâng lên `syscall`/`sysret` là bước sau (cần đổi thứ tự GDT + per-CPU).
```

```text
spec/abi.toml:101-103
trap = "int 0x80"              # cơ chế chuyển: `int 0x80` (IDT vector 0x80, gate DPL=3)
stack = "RSP user → RSP0 của TSS khi vào CPL0; …"
clobbered = ["rax", "rdx"]     # thanh ghi syscall được phép đổi (error/value); cờ được `iretq` khôi phục
```

Hình học GDT hiện tại (7 entry):

```text
v2/kernel/vifast-plat-x86_64/src/gdt.rs:33-38
//! | # | Nội dung | Selector | Access | Flags |
//! | 0 | null (bắt buộc) | — | `0x00` | `0x0` |
//! | 1 | kernel code 64-bit | `0x08` | `0x9A` | `0x2` (`L=1`, `D/B=0`) |
//! | 2 | kernel data | `0x10` | `0x92` | `0x0` |
//! | 3 | user code 64-bit | `0x18` (`0x1B` với RPL 3) | `0xFA` | `0x2` |
//! | 4 | user data | `0x20` (`0x23` với RPL 3) | `0xF2` | `0x0` |
//! | 5–6 | **TSS** (descriptor hệ thống 16 byte) | `0x28` | `0x89` | `0x0` |
```

```text
v2/kernel/vifast-plat-x86_64/src/gdt.rs:56   pub const GDT_ENTRIES: usize = 7;
```

Các test host **ghim từng số** (đổi hình học là đổi test — chi phí thật, phải kê):

```text
v2/kernel/vifast-plat-x86_64/tests/exc.rs:65-66
assert_eq!(GDT_ENTRIES, 7);
assert_eq!(GDT_BYTES, 56);
v2/kernel/vifast-plat-x86_64/tests/exc.rs:80-82
assert_eq!(INDEX_TSS, 5);
assert_eq!(INDEX_TSS + 1, GDT_ENTRIES - 1);
v2/kernel/vifast-plat-x86_64/tests/exc.rs:85-92
assert_eq!(SELECTOR_KERNEL_CODE, 0x08);  … assert_eq!(SELECTOR_TSS, 0x28);
assert_eq!(SELECTOR_USER_CODE_RPL3, 0x1B);  assert_eq!(SELECTOR_USER_DATA_RPL3, 0x23);
```

Runner QEMU cũng ghim selector trong log:

```text
v2/test/boot/run-x86_64.sh:697   tr=0x28 type=0x8b ok=1
v2/test/boot/run-x86_64.sh:743   ^# sched: umode counts first=… cs=0x1b kernel_stack=…$
v2/test/boot/run-x86_64.sh:1089  (đột biến ca (q): cs=0x1b -> cs=0x8 phải bị bắt)
```

Chưa có `swapgs`/per-CPU trong mã kernel; MSR (`STAR`/`LSTAR`/`FMASK`) mới có **hằng** ở `control.rs` (test host đối chiếu),
**chưa** có chỗ lập trình. `syscall` không đổi stack:

```text
v2/docs/k5-x86_64-boot.md:392-404
- **`sysret`** nạp `CS = STAR[63:48] + 16`, `SS = STAR[63:48] + 8` …
  Muốn dùng `sysret` phải **đảo thứ tự** hai descriptor user (đổi hợp đồng GDT + test của KT-261) …
- **`syscall`** … không đổi stack: … `swapgs` + per-CPU, hoặc một static `RSP` + dựng lại frame `iretq`
  bằng tay — thêm một tầng trạng thái chưa có (chưa có `GS` base/`swapgs`, chưa có per-CPU).
```

### 6.2 Vấn đề

1. **Chặn "K5' đầy đủ".** K5' đòi `syscall`/`sysret` (`v2/docs/k5-x86_64-boot.md:351-352`); thứ tự GDT là
   **điều kiện tiên quyết** đã được ghi trong chính spec (dòng 92–93) nhưng chưa có quyết định **đổi hay
   không, đổi theo kiểu nào**.
2. **Không thể đổi `trap` trong spec mà không đổi mã.** `check_abi.py:321-341` ghim `trap == "int 0x80"` (dòng 331) và
   `generated.rs` có `X86_64_TRAP`; đổi spec trước khi có stub ⇒ cổng đỏ hoặc sinh hằng sai. Mà KT-345
   **không** có ngân sách/DoD để viết stub `syscall`/`swapgs`/per-CPU.
3. **Di cư có địa chỉ cụ thể**, không phải "chạm nhẹ": 2 chỗ trong `run-x86_64.sh` + 1 đột biến + 4 assert
   selector/hình học trong `tests/exc.rs` (tuỳ phương án), cộng thêm kỳ vọng `tr=0x28`.
4. **Không có chỗ nào ghi hình học GDT là hợp đồng.** Nó đang là "sự thật của mã + test"; khi `kernel.toml`
   được tạo (nhóm 1), đây là chỗ tự nhiên để ghi và **máy kiểm** thay vì 4 assert rải rác.

### 6.3 Phương án (chốt **thứ tự đích**; gói này **không** đổi `trap`)

#### 5A — **Đảo** hai descriptor user (Linux-style: data **trước** code)  ← *khuyến nghị*

- **Thứ tự mới:** `null, kernel code 0x08, kernel data 0x10, **user data 0x18**, **user code 0x20**, TSS 0x28`
  (vẫn **7** entry). `STAR[63:48] = 0x10` ⇒ `sysret`: `CS = 0x20|3 = 0x23`, `SS = 0x18|3 = 0x1B` — **đúng**.
  `syscall`: `STAR[47:32] = 0x08` ⇒ vào `CS 0x08`, `SS 0x10`.
- **`spec/abi.toml` (v1.1):** **không đổi** `trap` (vẫn `int 0x80`); chỉ ghi **hình học GDT hiện tại** vào
  `kernel.toml` (`[arch.x86_64.gdt]`) + trường `planned = "user_data truoc user_code (D2 moi)"`.
- **`kernel.toml` + checker:** `check_kernel.py` đối chiếu hằng trong `gdt.rs` (7 entry, 5 selector, TSS index
  5) ⇒ lệch là đỏ. **Không** sửa mã.
- **Khi thi hành (task K5' full, không phải KT-345a):** đổi `gdt.rs` + `tests/exc.rs:85-92` (đổi 2 selector),
  `run-x86_64.sh:743,1089` (`cs=0x1b` → `cs=0x23`), viết stub `syscall`/`sysret` + `swapgs` + per-CPU; **sau
  đó** mới đổi `[calling.x86_64].trap/stack/clobbered/note` + `check_abi.py:331` + sinh lại `generated.rs` +
  `ktest.sh` (nếu có assert trap). **Giữ cổng `int 0x80`** làm đường cũ ⇒ `min_compat = 1`.
- **`min_compat`/`abi_version` trong v1.1:** 1/1. **3 arch:** chỉ x86_64.
- **Chi phí di cư (khi thi hành):** 2 assert selector + 2 kỳ vọng log + 1 đột biến; **thấp–trung bình**.

#### 5B — **Descriptor lặp**: thêm user code thứ hai ở `0x28`, TSS dời `0x30` (**8** entry)

- **Thứ tự:** `null, kcode 0x08, kdata 0x10, ucode 0x18, udata 0x20, ucode2 0x28, TSS 0x30/0x38`.
  `STAR[63:48] = 0x18` ⇒ `sysret`: `CS = 0x18+16 = 0x28|3 = 0x2B`, `SS = 0x20|3 = 0x23`.
- **Ưu:** giữ nguyên selector **user hiện tại** (0x18/0x20) ⇒ các kỳ vọng `cs=0x1b` **không** đổi **cho tới khi**
  bật `sysret`; `int 0x80`/`iretq` vẫn trả `cs=0x1b` như hôm nay.
- **Nhược:** **2** descriptor code user; `sysret` trả `CS = 0x2b` khác `USER_CS` ⇒ khó giải thích; GDT **8**
  entry ⇒ đổi `GDT_ENTRIES/GDT_BYTES/INDEX_TSS/SELECTOR_TSS` + các assert hình học trong `tests/exc.rs:65-82`
  + kỳ vọng `tr=0x28` → `tr=0x30` (`run-x86_64.sh:697`). Địa chỉ TSS per-hart (`plat-x86_64/tests/smp.rs:479`)
  tự dãn theo `GDT_BYTES` nên **không** phải sửa tay.
- **`min_compat`/`abi_version`:** 1/1 (không wire). **Chi phí:** trung bình, **nhiều chỗ ghim hơn 5A**.

#### 5C — **Giữ `int 0x80` vĩnh viễn**, không đổi GDT

- **`spec/`**: không đổi. **Chi phí 0**; nhưng K5' đầy đủ **không đạt** (đã là nợ công bố) và mọi lần đo
  hiệu năng syscall sau này lại phải mở lại quyết định. Chấp nhận được **chỉ** nếu Agent 0 tuyên bố K5'
  full lùi sang Chu kỳ 3.

### 6.4 Đánh giá

| Tiêu chí | **5A (đảo)** | 5B (lặp descriptor) | 5C (không đổi) |
|---|---|---|---|
| `sysret` chạy đúng | **Có** (chuẩn Linux/SDM) | Có | Không |
| Giữ selector user hiện tại | Không (0x23/0x1b) | **Có** (tới khi bật sysret) | Có |
| Số descriptor | 7 | **8** | 7 |
| Chỗ ghim phải sửa (khi thi hành) | 2 assert + 2 log + 1 đột biến | 4 assert + 1 log + trampoline assert | 0 |
| Đụng `spec/abi.toml` trong v1.1 | Không | Không | Không |
| Đóng nợ K5' full | **Có** (đường ngắn nhất) | Có | Không |

### 6.5 Khuyến nghị

**5A**, nhưng **chỉ chốt hướng** trong v1.1: ghi hình học GDT **hiện tại** + `planned` vào `kernel.toml`,
**không** đổi `trap`, **không** sửa mã. Lý do: (a) 5A là thứ tự chuẩn của Linux/SDM, một descriptor ít hơn,
và làm `CS − 8 = SS` thành **bất biến kiểm được**; (b) đổi `trap` bây giờ là **spec nói trước mã** — vi phạm
thứ tự nguồn sự thật (AGENTS §1) và làm đỏ `check_abi.py`/`generated.rs`; (c) KT-345a không có ngân sách viết
`swapgs`/per-CPU, nên việc đổi phải là task riêng **sau** — và khi đó 5A chỉ cần sửa 5 chỗ đã liệt; (d) 5B
giữ được `cs=0x1b` nhưng đẻ ra descriptor trùng và **nhiều chỗ ghim hơn**, không đáng cho một bước trung
gian.

### 6.6 Kịch bản kiểm (không có `.scn` runtime cho GDT; đây là host test + TAP)

```text
# Kịch bản kiểm cho nhóm 5 — hai tầng
# Tầng host (không QEMU): v2/kernel/vifast-plat-x86_64/tests/gdt.rs
1. DƯƠNG (v1.1): gdt khớp kernel.toml [arch.x86_64.gdt] (7 entry, 5 selector, TSS index 5, sysret_ready=false).
2. ÂM: sửa kernel.toml entry 4 -> 0x18 trong bản sao -> check_kernel ĐỎ.
# Tầng thi hành (task K5' full, KHÔNG phải KT-345a)
3. DƯƠNG: sau khi đảo, STAR[63:48]=0x10; sysret vào ring 3 -> log `cs=0x23 ss=0x1b`; syscall vào kernel -> cs=0x08.
4. ÂM: bỏ `swapgs` ở entry -> #GP hoặc rò GS user -> TAP ĐỎ.
5. ĐỘT BIẾN: đặt STAR[63:48]=0x0B (như cũ) -> `sysret` #GP, TAP ĐỎ (đúng lý do KT-270 đã ghi).
6. DƯƠNG: cổng `int 0x80` giữ nguyên -> chương trình cũ vẫn chạy (min_compat = 1).
```

---

## 7. Nhóm 6 — Socket IDL cho `NetSvc` phục vụ **≥ 2 caller** đồng thời

### 7.1 Hiện trạng

`NetSvc` là dịch vụ kernel **một caller**, một `Reply`:

```text
v2/kernel/vifast-kbin-riscv64/src/net_svc.rs:61-68
pub struct NetSvc {
    tcb_k: ObjId,
    ep: SlotRef,
    reply: SlotRef,
    /// Object `Reply` (để đọc `Reply.caller` — `reply_caller` nhận `ObjId`, không phải slot).
    reply_obj: ObjId,
    /// TCB **duy nhất** được phép gọi (net server U-mode) — kiểm qua `Reply.caller`.
    caller: ObjId,
    …
```

Hợp đồng hiện tại là **label + 1 MR**, không phải IDL:

```text
v2/kernel/vifast-kbin-riscv64/src/net_svc.rs:17-29
//! server U-mode (netsrv)                         kernel (dịch vụ NetPing)
//!   Call(EP_KERN, {label=0x291,length=1}, MR0=ip) ──►  recv(EP_KERN, REPLY)
//! …
//! | `Reply.caller` **không phải** TCB net server đã cấp cap | `InsufficientRights` (3) |
```

IDL hiện có **hai** interface, cả hai **ngoài `spec/`**:

```text
v2/lib/vifast-idl/idl/ring.vidl      interface ringchan version 1   (KT-69)
v2/lib/vifast-idl/idl/pager.vidl     interface pager   version 1   (KT-70)
```

Generator có **giới hạn cứng** (áp cho mọi thiết kế socket IDL):

```text
v2/lib/vifast-idl/idl/ring.vidl:4-12
param <tên> <u32|u64>      # tham số vô hướng, tối đa 4 (MR0..MR3)
cap <tên> <frame|buffer>   # tối đa MỘT cap/buffer trên mỗi method
returns <u32|u64|unit>
# Method id sinh tuần tự 0..N-1 … spec/abi.toml KHÔNG có bảng method cho interface mới (D2-102c)
```

và thiết kế **đã chỉ định** chỗ ở + mô hình:

```text
docs/v2/ViFastOs-v2-thiet-ke.md:555
- **IDL** (`spec/protocols/*.vidl`): `fn` (Call/Reply), `oneway` (Send), `ring<T, N>` …
docs/v2/ViFastOs-v2-thiet-ke.md:621
Net server dùng stack thuần Rust … qua ring với driver; socket API qua IDL; DHCP, DNS ở user.
docs/v2/ViFastOs-v2-thiet-ke.md:680
  spec/          abi.toml, kernel.toml, deps.toml, protocols/*.vidl, manifests/*.toml
```

**Trạng thái đã đạt:** KT-281 (netsrv U-mode, 1 client) + KT-306 (`NetPing` thành dịch vụ kernel kiểm quyền,
TAP 3/3) — nhưng **chưa** có ≥ 2 caller, **chưa** có `socket.vidl`, và `spec/` **không** có một dòng nào về
socket/IDL (`grep -n "socket\|vidl" spec/*.toml` = **0**).

### 7.2 Vấn đề

1. **"Đồng thời" bị chặn bởi cấu trúc, không phải bởi tải.** `caller: ObjId` + `reply: SlotRef` nghĩa là chỉ
   **một** TCB được phép gọi và chỉ **một** reply slot; caller thứ hai bị `InsufficientRights` (3) — đúng
   thiết kế KT-306 nhưng **không** đáp ứng DoD KT-345b ("≥ 2 caller đồng thời").
2. **Không có IDL cho socket.** Mọi thứ đang là label `0x291` + MR0 thô; thêm method thứ hai (connect/send/
   recv/close) bằng tay sẽ tái diễn đúng lỗi "số method chép tay" mà D2-102c muốn tránh.
3. **Không có chỗ trong `spec/` cho protocol.** Thiết kế:680 nói `spec/protocols/*.vidl`, thực tế IDL nằm ở
   `v2/lib/vifast-idl/idl/` — hai chỗ, chưa có quy tắc chọn.
4. **Ca thật:** KT-281 chỉ dựng **1 client**; KT-306 kiểm **3 request** nhưng **cùng một** caller. Muốn chứng
   minh ≥2 caller phải có caller thứ hai **và** trạng thái phiên tách biệt — hiện không có chỗ chứa trạng thái
   phiên nào trong `NetSvc`.

### 7.3 Phương án

#### 6A — `spec/protocols/socket.vidl` + **demux theo handle** trong netsrv U-mode  ← *khuyến nghị*

- **`spec/protocols/socket.vidl`** (mới; nháp ở Phụ lục B): `open`, `connect`, `send`, `recv`, `close`, `poll`.
  Mỗi method ≤ 4 scalar + ≤ 1 cap (đúng giới hạn generator). `handle` là `u32` **do server cấp**.
- **Đa caller:** netsrv giữ **bảng phiên** có biên (`SOCKET_SESSIONS`), mỗi phiên = `{handle, badge, state}`;
  **badge** (do `CNode.Mint` cấp cho client, thiết kế: "đưa cap ra applet") là danh tính; `handle` là khoá
  tra cứu. `open` khi bảng đầy ⇒ trả `Busy` (14) — **fail-closed**, không cấp handle giả.
- **Kernel `NetSvc`:** **giữ nguyên** đơn caller (nó là dịch vụ *thiết bị* cho netsrv, không phải socket API);
  netsrv là **caller duy nhất** của kernel — đúng kiến trúc "driver ở kernel, server điều phối" (KT-281 §2).
- **`spec/`:** thêm **một** tệp (không sửa `abi.toml`). `gen_idl.py` quét thêm `spec/protocols/`; quy tắc:
  protocol **mới** ở `spec/protocols/`, hai interface cũ (`ring`, `pager`) **giữ nguyên** tới KT-346.
- **`min_compat`/`abi_version`:** 1/1. **3 arch:** stub sinh dùng chung; netsrv hiện chỉ riscv64 (ảnh sản phẩm
  KT-342); aarch64/x86_64 dùng lại cùng tệp IDL khi có netsrv.
- **Chi phí:** trung bình — 1 tệp IDL + sửa `gen_idl.py` (đường dẫn) + bảng phiên trong netsrv + 2 client
  trong ảnh test + TAP. Không đụng `v2/kernel`.

#### 6B — `v2/lib/vifast-idl/idl/socket.vidl` (không đụng `spec/`) + cùng mô hình handle

- **`spec/`:** **không** đổi. Theo tiền lệ D2-102c ("interface dịch vụ **không** thuộc ABI").
- **Đa caller:** y như 6A (bảng phiên trong netsrv).
- **`vifast-gen`:** không đổi (đã quét đúng thư mục này). **Chi phí thấp nhất.**
- **Nhược:** tiếp tục để IDL ngoài `spec/` dù thiết kế:680 đã chỉ định `spec/protocols/`; protocol ổn định
  lâu dài (socket) sẽ sống ngoài nguồn sự thật.

#### 6C — Object **`Socket`** trong `abi.toml` (mỗi socket = một cap)

- **`spec/abi.toml`**: `[object] Socket = { id = 17, stability = "core", method = { 0 Open, 1 Bind, 2 Connect,
  3 Send, 4 Recv, 5 Close, 6 Poll } }` + `[object_reserved].from = 18` + `[method_signature."Socket.Send"]`…
- **Đa caller:** mỗi caller giữ **cap Socket riêng** ⇒ tự nhiên ≥2, không cần demux; kernel dispatch method
  theo `label`.
- **`min_compat`/`abi_version`:** 1/1 (object 17 nằm trong dải reserved **không** thuộc 5 thứ đóng băng) —
  nhưng **phải** sửa `check_reserved.py` (dải 17..31 → 18..31), `gen.py`, và **trái thiết kế**: §6.7 đặt
  socket API ở **user** ("socket API qua IDL"), kernel không giữ trạng thái TCP; 6C kéo state machine TCP
  vào kernel.
- **Chi phí:** cao nhất; **không khuyến nghị** trong Chu kỳ 2.

### 7.4 Đánh giá

| Tiêu chí | **6A (spec/protocols)** | 6B (v2/lib) | 6C (object) |
|---|---|---|---|
| Có IDL cho socket | **Có** | Có | Không (ABI method) |
| ≥ 2 caller đồng thời | **Có** (bảng phiên + badge) | Có | Có (cap riêng) |
| Đụng `abi.toml` | Không | Không | **Có** |
| Trái thiết kế socket-ở-user | Không | Không | **Có** |
| Đụng `v2/kernel` trong KT-345b | Không | Không | Có |
| Sửa checker | Thêm quét `spec/protocols/` | Không | `check_reserved` + `gen.py` |
| Công sức | Trung bình | **Thấp** | Cao |

### 7.5 Khuyến nghị

**6A.** Lý do: (a) thiết kế **đã chỉ định** `spec/protocols/*.vidl` (dòng 555/680) — đây là dịp duy nhất
trong Chu kỳ 2 để đặt đúng chỗ mà không phải mở lại; (b) socket là protocol **ổn định lâu dài**, khác
`ring`/`pager` (nội bộ ảnh test) — nó xứng đáng nằm trong nguồn sự thật; (c) 6A **không** sửa `abi.toml`,
**không** đụng kernel, giữ `min_compat = 1`; (d) 6C đi ngược kiến trúc "socket ở user" và đắt nhất.

Nếu Agent 0 ưu tiên **không** thêm bất kỳ tệp nào vào `spec/` trong v1.1: chọn **6B** — cùng mô hình, chỉ
khác chỗ đặt; KT-345b vẫn đạt DoD "≥ 2 caller".

### 7.6 `.scn` nháp

```toml
# ĐỀ XUẤT — KT-345b chọn số/tag. Kiểm: 2 client cùng gọi netsrv, phản hồi không lẫn; client thứ 3 khi bảng đầy -> Busy.
version = 1
name = "30-netsrv-two-callers"
tags = ["call-reply"]
caps = { u0 = "Untyped", c0 = "CNode", c1 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
thread = [
  { name = "client_a", steps = [
    "call cap=ep0 handle=1 => ok",
    "recv cap=ep1 => ok",
    "exit code=0 => halt",
  ] },
  { name = "client_b", steps = [
    "call cap=ep0 handle=2 => ok",
    "recv cap=ep1 => ok",
    "exit code=0 => halt",
  ] },
  { name = "netsrv", steps = [
    "recv cap=ep0 => ok",
    "send cap=rep0 => ok",
  ] },
]
```

```text
# Kịch bản kiểm thật cho nhóm 6 (KT-345b)
1. DƯƠNG: 2 client TCB khác nhau gọi `open` -> 2 handle khác nhau; gửi/recv xen kẽ -> mỗi client nhận đúng
   dữ liệu của mình (không lẫn), log `# netsrv: sessions=2 served=… denied=0`.
2. ÂM: client thứ 3 khi bảng phiên đầy -> `Busy` (14), không cấp handle, không gửi gói.
3. ÂM: `handle` không thuộc badge của caller -> `InvalidCapability` (1) hoặc `InvalidArgument` (5) — chọn một,
   ghi vào IDL; KHÔNG được fallback sang phiên khác.
4. ĐỘT BIẾN: bỏ kiểm badge -> ca 2/3 ĐỎ (client thứ 3 chiếm được phiên).
5. ĐỘT BIẾN: dùng `handle` toàn cục thay `(badge, handle)` -> ca 3 ĐỎ.
```

---

## 8. Nhóm 7 — Hợp đồng trả về của `x86_64` (ba lỗi thật trong `[calling.x86_64]`)

### 8.1 Hiện trạng

```text
spec/abi.toml:99-109
[calling.x86_64]
syscall_number = "rax"         # x86_64: số hiệu syscall ở rax (khuôn Linux x86_64)
args = ["rdi", "rsi"]          # 2 tham số đầu — đối xứng a0/a1 của riscv64
message_info = "rdx"           # MessageInfo (35 bit) — đối xứng a2
mr_registers = ["r10", "r8", "r9", "r12"]   # MR0..MR3 — đối xứng a3..a6
return_error = "rax"           # 0 = thành công, khác 0 = mã lỗi (1..19)
return_value = "rdx"
trap = "int 0x80"              # cơ chế chuyển: `int 0x80` (IDT vector 0x80, gate DPL=3)
…
clobbered = ["rax", "rdx"]     # thanh ghi syscall được phép đổi (error/value); cờ được `iretq` khôi phục
ipc_buffer = "Frame bind qua Tcb.Configure; header chứa receive window (CPTR, ≤4 cap)"
preserved = ["rsp", "rbx", "rbp", "r12..r15"]   # callee-saved của SysV AMD64
```

Lõi **đã** mang đủ ba kênh trả, tách biệt:

```text
v2/kernel/vifast-core/src/syscall.rs:120-129
pub struct SyscallReturn {
    /// `a0` — 0 = OK, 1..19 = mã lỗi ABI ([`KError::id`]).
    pub error: u64,
    /// `a1`.
    pub value: u64,
    /// `a2` — `MessageInfo` **đã đóng gói** (Recv/NBRecv/ReplyRecv/NBSendRecv/NBSendWait) hoặc 0.
    pub info: u64,
    /// `a3..a6` — MR0..MR3 (hoặc `min_compat`/`features`/`num_harts` của `DebugGetInfo` ở
    /// `mr[0..3]`).
    pub mr: [u64; 4],
}
```

Trên riscv64, kênh trả hiện chạy đúng **ba thanh ghi khác nhau** (`a0` error, `a1` value, `a2`
`MessageInfo`, `a3` MR0) — xác nhận bởi chính chú thích của dịch vụ đang chạy thật:

```text
v2/kernel/vifast-kbin-riscv64/src/net_svc.rs:21
//! resume: a0=error, a1=1, a2=MessageInfo, a3=rtt ◄──  reply_msg(REPLY, {label=0x291}, MR0=rtt)
```

x86_64 thì **không** có chỗ thứ ba: `message_info` và `return_value` cùng `rdx`; và ảnh hiện tại chỉ
viết được **hai** kênh:

```text
v2/kernel/vifast-kbin-x86_64/src/umode.rs:190-192
/// Tham số theo SysV: `nr` = `rax`, `a0`/`a1` = `rdi`/`rsi`, `mi` = `rdx`, `mr0`/`mr1` =
/// `r10`/`r8`, `cs`/`rip` = frame của CPU (để chứng minh **đã vào từ ring 3**). Trả
/// [`SyscallRet`] (`rax` = error, `rdx` = value) — stub ghi vào frame trước `iretq`.
```

```text
v2/kernel/vifast-plat-x86_64/src/asm.rs:559
"mov rcx, [rbx + {sc_rdx}]", // mi
```

`calling.rs` **đã có sẵn đề xuất** cho hai kênh còn thiếu, nhưng spec chưa dùng tới:

```text
v2/kernel/vifast-plat-x86_64/src/calling.rs (hằng, không asm)
pub const SYSCALL_RETURN_MSGINFO_REG: Reg = Reg::R13;   // đường syscall/sysret (bước sau)
pub const INT80_MR_REGS: [Reg; 4] = [Reg::Rcx, Reg::R8, Reg::R9, Reg::R10];
pub const INT80_RETURN_MSGINFO_REG: Reg = Reg::R11;     // đường int 0x80
pub const INT80_CLOBBERED: [Reg; 2] = [Reg::Rax, Reg::Rdx];
```

Các test host đang **ghim nguyên văn** hiện trạng sai này:

```text
v2/kernel/vifast-plat-x86_64/tests/host.rs:736-768
assert_eq!(SYSCALL_MSGINFO_REG, Reg::Rdx);
assert_eq!(SYSCALL_RETURN_VALUE_REG, Reg::Rdx);      // ← trùng
assert_eq!(SYSCALL_RETURN_MSGINFO_REG, Reg::R13);    // ← spec không có trường này
assert_eq!(INT80_MR_REGS, [Reg::Rcx, Reg::R8, Reg::R9, Reg::R10]);
assert_eq!(INT80_RETURN_MSGINFO_REG, Reg::R11);
assert_eq!(INT80_CLOBBERED, [Reg::Rax, Reg::Rdx]);
```

### 8.2 Vấn đề (ba lỗi, không phải một)

**(a) `return_value = "rdx"` trùng `message_info = "rdx"`.** `Recv`/`NBRecv`/`ReplyRecv`/
`NBSendRecv`/`NBSendWait` trả **đồng thời** `error`, `value`, `MessageInfo`, MR\* (`spec/abi.toml`
cột `ret` của syscall 3..7). Với x86_64 hiện tại, kênh `value` và kênh `MessageInfo` là **một**
⇒ kernel phải chọn một: hoặc mất `value`, hoặc mất `MessageInfo`. Lõi không thể "gộp" vì
`SyscallReturn` giữ ba trường riêng. **Ca thật sẽ nổ ngay khi `KT-340c` nối `vifast-core`**: syscall
`Recv` đầu tiên trả message là mất một kênh — im lặng, vì không có test register-level nào trên
x86_64.

**(b) Thiếu trường `return_message_info`.** riscv64/aarch64 không cần trường này vì thanh ghi
`message_info` (`a2`/`x2`) **khác** `return_value` (`a1`/`x1`) ⇒ có thể tái dùng chính nó làm kênh
trả `MessageInfo`. x86_64 không tái dùng được (`rdx` đã là value) ⇒ **bắt buộc** khai thanh ghi
riêng; `[calling.x86_64]` không có trường đó, `vifast-gen` cũng không sinh (danh sách trường ở
`gen.py:205-214` chỉ có 5 mục `syscall_number`/`message_info`/`return_error`/`return_value`/`trap`).

**(c) `r12` mâu thuẫn ba chiều.** `mr_registers` chứa `r12` (MR3 **vào**), `preserved` chứa
`r12..r15`, `clobbered` chỉ `["rax","rdx"]`. Nhưng MR3 là **kênh ra** của `Recv`: kernel ghi MR3
nhận được vào đúng `r12` ⇒ giá trị `r12` **đổi**; vậy `preserved` sai và `clobbered` thiếu. Ba
nguồn nói ba kiểu; `check_abi.py:345` chỉ kiểm `error != value` nên **không bắt** được.

> Vì sao chưa ai thấy: ảnh `x86_64` hiện chỉ chạy `DebugPutStr`(12)/`Exit`(14) — không có syscall
> nào trả `MessageInfo`/MR (`umode.rs:193-250`). Đây là lỗi **hợp đồng**, chưa có ca runtime.

### 8.3 Phương án

#### 7A — Đổi `return_value` sang thanh ghi trống (`r13`), **không** thêm trường

- **`spec/abi.toml`**: `return_value = "r13"`; `clobbered = ["rax","rdx","r13","r10","r8","r9","r12"]`;
  `preserved = ["rsp","rbx","rbp","r14","r15"]`; `message_info = "rdx"` giữ nguyên ⇒ kênh trả
  `MessageInfo` = `rdx`.
- **Thanh ghi trống:** sau `rax` (số hiệu + error), `rdi`/`rsi` (2 tham số), `rdx` (info),
  `r10`/`r8`/`r9`/`r12` (4 MR) thì các GPR còn lại là `rcx`, `r11`, `rbx`, `rbp`, `r13..r15`.
  `rcx`/`r11` **không** an toàn dài hạn: lệnh `syscall` clobber hai thanh ghi này (nhóm 5 sẽ dùng),
  nên chọn sẽ phải đổi lần nữa. `rbx`/`rbp`/`r13..r15` là callee-saved ⇒ dùng làm kênh trả phải
  **rời `preserved`**; chọn `r13` vì không trùng vai trò nào hiện có và không bị `int 0x80` đụng.
- **`vifast-gen`**: không cần thêm trường (chỉ đổi giá trị `return_value`); sinh lại hằng.
- **3 arch**: chỉ x86_64; riscv64/aarch64 không đổi.
- **Chi phí**: `calling.rs` (`SYSCALL_RETURN_VALUE_REG`), `host.rs:736-768`, `check_abi.py:326`,
  stub asm ghi `r13`, mọi U-mode stub đọc value ở `r13` — và **lần nữa** khi lên `syscall`/`sysret`.
- **`abi_version`**: **2**; **`min_compat`**: **2** — chương trình v1 đọc `rdx` sau syscall sẽ nhận giá trị khác nghĩa, nên kernel không thể cam kết chạy chúng.

#### 7B — Giữ `rdx` = `message_info`, trả `value` qua `rax`, **dời error**

- **`spec/abi.toml`**: `return_value = "rax"`, `return_error = "r11"` (hoặc `r13`); `message_info = "rdx"`.
  Đây là dạng **duy nhất hợp lý** của 7B: `rax` không thể vừa là error vừa là value.
- **Hệ quả:** đảo vai trò chuẩn của `rax` (Linux/SDM: `rax` = số hiệu vào, error/return ra) và phá
  **đối xứng 3 arch** (`return_error` = `a0`/`x0`/`rax`); mọi stub phải kiểm `r11` trước `rax`;
  `DebugGetInfo` trả `abi_version` ở `rax` trong khi 2 arch kia trả ở `x1`/`a1`.
- **`vifast-gen`**: sinh lại; **3 arch**: chỉ x86_64 nhưng làm lệch bảng.
- **Chi phí**: cao hơn 7C và **không** giải quyết gì thêm; **`abi_version` 2, `min_compat` 2** (kênh `rax` đổi nghĩa với chương trình v1).
- **Không khuyến nghị** — đây là "đổi cho hết trùng" nhưng đổi cả kênh quan trọng nhất.

#### 7C — Thêm `return_message_info = "r11"`, giữ `rdx` = value  ← *khuyến nghị*

- **`spec/abi.toml`**:

  ```toml
  # ĐỀ XUẤT 7C — [calling.x86_64]
  message_info = "rdx"
  return_error = "rax"
  return_value = "rdx"            # KHÔNG đổi
  return_message_info = "r11"     # MỚI: kênh trả MessageInfo của Recv/ReplyRecv
  mr_registers = ["r10", "r8", "r9", "r12"]
  clobbered = ["rax", "rdx", "r11", "r10", "r8", "r9", "r12"]
  preserved = ["rsp", "rbx", "rbp", "r13", "r14", "r15"]
  note = "… `r11` = MessageInfo trả về (int 0x80 không clobber r11); r12 = MR3 vào/ra nên KHÔNG preserved."
  ```

- Vì sao `r11` an toàn **với `int 0x80`**: `int 0x80` chỉ đẩy `SS/RSP/RFLAGS/CS/RIP` và **không**
  clobber `rcx`/`r11` (SDM, ghi trong `calling.rs` đầu tệp); `r11` không nằm trong 2 tham số, 4 MR,
  `rdx`, `rax` ⇒ trống. Khi nhóm 5 chuyển sang `syscall`/`sysret`, `syscall` **sẽ** clobber `r11`
  ⇒ lúc đó `return_message_info` đổi sang `r13` (đúng hằng `SYSCALL_RETURN_MSGINFO_REG = R13` đã
  có trong `calling.rs`) — thay đổi thuộc task K5' full, không phải v1.1.
- **Vì sao `rdx` vẫn = value:** giữ đúng hợp đồng mà ảnh hiện tại đang chạy và mọi thứ đã kiểm
  (`umode.rs`, `check_abi.py:326`, `host.rs`), chỉ **thêm** kênh mới; chương trình v1 đọc `rdx`
  sau `Recv` vẫn đọc đúng `value` như trước (trước đây nó **không thể** đọc `MessageInfo` — đó là
  lỗi, không phải tính năng).
- **Sửa mâu thuẫn (c):** `r12` ra khỏi `preserved`, vào `clobbered`; `r10/r8/r9` cũng vào
  `clobbered` (chúng là **kênh ra** MR0..MR2 của `Recv`). `r13..r15` giữ `preserved`.
- **`vifast-gen`**: thêm một mục vào danh sách sinh (`gen.py:205-214`) →
  `X86_64_RETURN_MESSAGE_INFO_REG = "r11"`; `gen_md` thêm cột "Trả MessageInfo".
- **3 arch**: riscv64/aarch64 **không** có trường này (kênh `MessageInfo` = thanh ghi `message_info`
  của chúng); `check_abi.py` phải cho phép `return_message_info` **chỉ** ở x86_64 (tập `extra` riêng).
- **Chi phí**: `calling.rs` (`INT80_CLOBBERED` + giữ `INT80_RETURN_MSGINFO_REG`), `host.rs:736-768`,
  `check_abi.py:326/338/345`, `generated.rs` sinh lại, stub asm của `KT-340c` ghi `r11` từ
  `SyscallReturn.info`. **Không** sửa lõi (`SyscallReturn` đã đủ).
- **`min_compat`**: 1; **`abi_version`**: **1** — `rdx` không đổi nghĩa; `r11` trước đây **không**
  được khai là `preserved` nên không có chương trình v1 nào có quyền dựa vào nó; `r12` chỉ bị
  "bớt hứa" đúng với thực tế MR3-ra.

### 8.4 Đánh giá

| Tiêu chí | 7A (value→r13) | 7B (value→rax, error→r11) | **7C (thêm `return_message_info`)** |
|---|---|---|---|
| `Recv` trả đủ error+value+info+MR | Có | Có | **Có** |
| `value` giữ `rdx` (không phá v1) | **Không** | Không | **Có** |
| Đối xứng 3 arch (`error`=a0/x0/rax) | Có | **Không** | **Có** |
| Đụng lõi `SyscallReturn` | Không | Không | Không |
| Số trường mới trong spec | 0 | 0 | **1** (đúng chỗ thiếu) |
| Đổi `abi_version` | **2** | **2** | **1** |
| Công sức | Trung bình | Cao | **Thấp–trung bình** |

### 8.5 Khuyến nghị

**7C.** Lý do: (a) đúng **một** lỗi gốc — thiếu kênh, không phải sai kênh; `rdx` = value đang chạy
đúng và không cần đổi; (b) `SyscallReturn` đã tách `info` khỏi `value` từ đầu, nên chỉ cần **một**
thanh ghi mới ở tầng arch — không đụng `message_layout` (D2-15) vì đây là **register trả về**, không
phải layout `MessageInfo`; (c) `r11` là thanh ghi duy nhất vừa trống vừa không bị `int 0x80` clobber,
và `calling.rs` đã chọn đúng nó (`INT80_RETURN_MSGINFO_REG = R11`) — spec đang đi sau mã, 7C chỉ
đồng bộ; (d) sửa luôn mâu thuẫn `r12` bằng cách nói đúng sự thật "MR3 là kênh ra".

Nếu Agent 0 muốn giữ `preserved` nguyên vẹn cho `r12`, chọn biến thể **7C.2**: đổi MR3 sang `rcx`
(`INT80_MR_REGS` đã đề xuất) — nhưng `syscall` clobber `rcx` nên sẽ phải đổi lại ở K5' full; **không**
khuyến nghị.

### 8.6 `.scn` nháp + kịch bản kiểm

```toml
# ĐỀ XUẤT — KT-345a chọn số/tag. `.scn` chỉ kiểm được NGỮ NGHĨA (Recv trả message + MR),
# không kiểm được thanh ghi; kiểm register-level nằm ở host test bên dưới.
version = 1
name = "31-replyrecv-info-and-mr"
tags = ["call-reply"]
caps = { u0 = "Untyped", c0 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
thread = [
  { name = "client", steps = [
    "call cap=ep0 mr=1,2 => ok",
    "recv cap=rep0 => ok",
  ] },
  { name = "server", steps = [
    "recv cap=ep1 => ok",
    "reply_recv reply=rep0 cap=ep1 mr=3 => ok",
  ] },
]
```

```text
# Kịch bản kiểm register-level (host + TAP) — KT-345a/KT-340c
# scope=v2/kernel/vifast-plat-x86_64/tests/host.rs (bảng) + v2/test/boot/run-x86_64.sh (TAP, khi KT-340c)
1. DƯƠNG: 4 kênh trả KHÁC nhau: rax (error), rdx (value), r11 (MessageInfo), r10/r8/r9/r12 (MR0..3).
2. DƯƠNG: `return_message_info` ∉ {return_value, return_error, message_info, mr_registers...}.
3. DƯƠNG: preserved ∩ {rax,rdx,r11,r10,r8,r9,r12} = ∅.
4. ÂM: đặt `return_message_info = "rdx"` trong bản sao spec -> check_abi ĐỎ (trùng kênh).
5. ÂM: đặt `preserved` chứa `r12` -> check_abi ĐỎ (vừa MR vừa preserved).
6. ĐỘT BIẾN: bỏ `r11` khỏi `clobbered` -> host test bảng ĐỎ.
7. TAP (KT-340c): Recv trả message có label=0x291 + value=7 + MR0=9 -> in cả ba, không mất kênh nào.
```

---

---

## 9. Nhóm 8 — `[debug_info].return_channel` và `[object_size]` cho 3 arch

### 9.1 Hiện trạng

**(a) `[debug_info].return_channel` — có, nhưng chỉ là tên thanh ghi riscv64.**

```text
spec/abi.toml:166-172
# DebugGetInfo = syscall 15 (D2-29); luôn có, không feature gate.
[debug_info]
number = 15
status = "frozen"
returns = ["abi_version", "min_compat", "features", "num_harts"]
# Kênh trả (D2-35): syscall không dùng MR vào; a0 = error.
return_channel = { a1 = "abi_version", a3 = "min_compat", a4 = "features", a5 = "num_harts" }
note = "Luôn có; features là bitmap tính năng runtime dùng để kiểm tương thích lúc khởi động; không trả địa chỉ kernel."
```

Lõi ghi đúng **bốn giá trị** vào `SyscallReturn` (value + `mr[0..2]`) — không có `Machine::debug_get_info`;
`DebugGetInfo` chỉ là một nhánh trong `dispatch`:

```text
v2/kernel/vifast-core/src/syscall.rs:810-816
SYS_DEBUG_GET_INFO => {
    let mut r = SyscallReturn::new(0, ABI_VERSION);
    r.mr[0] = MIN_COMPAT;
    r.mr[1] = env.features();
    r.mr[2] = env.num_harts();
    SyscallOutcome::Return(r)
}
```

Còn "kênh" là ánh xạ `value → return_value`, `mr[i] → mr_registers[i]` của từng arch — nhưng spec chỉ
khai **một** ánh xạ riscv64, và máy kiểm ghim nguyên văn:

```text
v2/test/abi/check_abi.py:235-238
rc = dbg.get("return_channel", {})
check(rc.get("a1") == "abi_version" and rc.get("a3") == "min_compat"
      and rc.get("a4") == "features" and rc.get("a5") == "num_harts",
      "DebugGetInfo return_channel phải là a1/a3/a4/a5 (D2-35)")
```

⇒ aarch64 (`x1/x3/x4/x5`) và x86_64 (`rdx/r10/r8/r9`) **không được khai** ở đâu cả. `gen.py` cũng
không sinh gì từ `[debug_info]` (chỉ `[calling.*]` → hằng `*_RETURN_VALUE_REG`…).

**(b) `[object_size]` — có `riscv64`, không có 2 arch còn lại.**

```text
spec/abi.toml:109-115
# Kích thước/căn lề (thứ đóng băng thứ 4). slot 48 B là [TK] của QD §3.9.
[object_size]
slot = { bytes = 48, align_bytes = 16, note = "QD §3.9 [TK] — sửa từ 32 B" }
default_align_bytes = 16
page_bytes = 4096
untyped_min_bytes = 4096
# Sizes dưới đây là [TK] — thiết kế §4.21 không cho con số; chờ duyệt ở KT-04d.
riscv64 = { tcb = 512, endpoint = 64, notification = 64, reply = 64, sched_context = 256, sched_control = 64, cnode_base = 64 }
```

Nhưng các số này **không** phải kích thước phần cứng riscv64: generator đọc đúng khoá `riscv64` rồi
sinh **một bộ** hằng `OBJ_*`; `vifast-core` dùng bộ đó trên **mọi** arch (`no_std`, không `cfg`):

```text
v2/tools/vifast-gen/gen.py:140
rv = osz.get("riscv64") or {}
```

```text
v2/kernel/vifast-core/src/spec.rs:11-15
pub use vifast_abi::{
    ABI_VERSION, CNODE_BASE_BYTES, CORE_OBJECTS, DEFAULT_ALIGN_BYTES, ERRORS, ERROR_COUNT,
    MIN_COMPAT, OBJ_ENDPOINT_BYTES, OBJ_NOTIFICATION_BYTES, OBJ_REPLY_BYTES,
    OBJ_SCHED_CONTEXT_BYTES, OBJ_SCHED_CONTROL_BYTES, OBJ_TCB_BYTES, PAGE_BYTES, RESERVED_OBJECTS,
    SLOT_ALIGN_BYTES, SLOT_BYTES, SYSCALL_COUNT, UNTYPED_MIN_BYTES,
};
```

**Nguồn số thật (theo yêu cầu của D2-237):**

| Nguồn | Cho biết gì | Có phải `size_of` thật? |
|---|---|---|
| `spec/abi.toml:115` | nguồn **duy nhất** của `tcb=512`, `endpoint=64`, … | Không — số `[TK]` do thiết kế §4.21 để trống |
| `v2/lib/vifast-abi/src/generated.rs:332` | `OBJ_TCB_BYTES: usize = 512` (sinh) | Không — bản sao của spec |
| `v2/lib/vifast-abi/src/lib.rs:96-113` | test khẳng định hằng sinh = 48/512/64/… | Không — kiểm **generator**, không kiểm layout |
| `v2/sim/vifast-sim/tests/kt59_core_parity.rs:2190` | `used == MAX_TCBS * OBJ_TCB_BYTES` | Không — kiểm **ngân sách arena** |
| `v2/kernel/vifast-core/src/tls_tests.rs:366` | `size_of::<TcbData>()` cho pin `.bss` | **Có** đo, nhưng đo **body nội bộ**, không phải cỡ object ABI |
| aarch64/x86_64 | **không có** `size_of` nào | 2 ảnh chưa link `vifast-core` (`KT-338b`/`KT-340c` đang chờ `KT-345a`) |

⇒ Số đúng cho `aarch64`/`x86_64` **hôm nay** = **đúng bằng** bộ số riscv64 (cùng một mô hình, cùng
`Machine`, cùng `slot` 48 B). Điền 3 khoá là **nói thật** — miễn ghi rõ đây là hằng **model**, sẽ
phải mở lại khi thêm ngữ cảnh kiến trúc (FPU/SIMD, `D2-240` trụ cột 5) làm TCB khác cỡ.

### 9.2 Vấn đề

1. **Kênh trả của `DebugGetInfo` chỉ đúng cho riscv64.** `KT-338b`/`KT-340c` sẽ cài `DebugGetInfo`
   cho 2 arch ngay sau `KT-345a`; nếu spec không khai, mỗi arch tự chọn thanh ghi ⇒ lệch "cùng một
   syscall, ba hợp đồng" và `check_abi` **không** bắt được (nó chỉ kiểm bản riscv64).
2. **`check_abi.py:238` biến một tên thanh ghi riscv64 thành luật ABI toàn cục.** Mọi đề xuất cho
   aarch64/x86_64 đều phải sửa dòng này — nếu không, checker đỏ oan; nếu sửa mà không có mô hình
   ánh xạ, checker mất răng.
3. **`[object_size]` hàm ý "riscv64 là arch duy nhất có object"**, trong khi core dùng nó cho cả ba.
   Người viết `KT-338b`/`KT-340c` sẽ không biết TCB 512 B có áp cho arch mình không; nếu họ tự đặt số
   khác, `object_layout` (thứ đóng băng #4) vỡ mà không có cổng nào bắt.
4. **Không có test canh `[object_size]` theo arch.** `check_abi.py:368` chỉ kiểm `slot.bytes == 48`;
   `lib.rs:96-113` chỉ kiểm hằng sinh. Thêm khoá mới mà không thêm kiểm ⇒ 3 khoá có thể lệch nhau
   im lặng.
5. **Ca thật:** `KT-338b` cần `DebugGetInfo` trả `features`/`num_harts` để chương trình U-mode kiểm
   tương thích lúc khởi động (D2-29); `KT-340c` cũng vậy. Cả hai đang `BLOCKED` chờ `KT-345a` — nếu
   `KT-345a` chỉ điền riscv64 thì 2 task này buộc phải đoán.

### 9.3 Phương án

#### 8A — Điền đủ cả hai **ngay**, viết tay trong spec

- **`[object_size]`**: thêm `aarch64`/`x86_64` với **cùng** 7 khoá + `note = "hằng model; bằng riscv64
  tới khi có ngữ cảnh arch (FPU/SIMD) — D2-240"`.
- **`[debug_info].return_channel`**: đổi thành bảng theo arch:

  ```toml
  [debug_info.return_channel.riscv64]
  value = "a1"
  mr = ["a3", "a4", "a5"]
  [debug_info.return_channel.aarch64]
  value = "x1"
  mr = ["x3", "x4", "x5"]
  [debug_info.return_channel.x86_64]
  value = "rdx"
  mr = ["r10", "r8", "r9"]
  ```

- **`vifast-gen`**: sinh `DEBUG_INFO_CHANNEL_<ARCH>` + bảng trong `docs/abi.md`; `[object_size]` sinh
  hằng per-arch (hoặc một bộ + assert bằng nhau).
- **`check_abi`**: kiểm 3 khoá `[object_size]` bằng nhau; kiểm `value == calling.<arch>.return_value`
  và `mr[i] == calling.<arch>.mr_registers[i]`; bỏ ghim `a1/a3/a4/a5` ở dòng 235-238.
- **3 arch**: chỉ metadata; không đổi mã. **`abi_version`/`min_compat`**: 1/1 (không đổi wire).
- **Chi phí**: thấp–trung bình; **nhược**: 3 bản số trùng nhau (dễ lệch khi sửa tay — đúng thứ D2-11
  muốn tránh).

#### 8B — Chỉ điền `[object_size]`, **hoãn** `return_channel`

- **`spec/`**: thêm 2 khoá `object_size`; `[debug_info].return_channel` giữ nguyên + thêm
  `note = "riscv64-only; aarch64/x86_64 khai ở task sau"`.
- **`vifast-gen`**: chỉ đổi phần `object_size`; **không** sinh kênh.
- **3 arch**: `KT-338b`/`KT-340c` vẫn phải tự chọn thanh ghi — đúng cái nợ cần đóng.
- **`abi_version`/`min_compat`**: 1/1 (chỉ thêm metadata cho `[object_size]`).
- **Chi phí thấp nhất**, nhưng để lại lỗi nặng hơn (kênh trả) ngay trước 2 task cần nó.

#### 8C — **Sinh từ `vifast-gen`**, một nguồn cho cả hai  ← *khuyến nghị*

- **`[object_size]`**: giữ **một** bộ số model; 3 khoá arch là **alias** có kiểm:

  ```toml
  [object_size]
  slot = { bytes = 48, align_bytes = 16 }
  default_align_bytes = 16
  page_bytes = 4096
  untyped_min_bytes = 4096
  model = { tcb = 512, endpoint = 64, notification = 64, reply = 64, sched_context = 256, sched_control = 64, cnode_base = 64 }
  riscv64 = { same_as = "model" }
  aarch64 = { same_as = "model" }
  x86_64 = { same_as = "model" }
  note = "Hằng MODEL dùng chung 3 arch; arch nào cần khác cỡ (vd TCB + ngữ cảnh FPU) phải mở ABI riêng."
  ```

- **`[debug_info].return_channel`**: khai theo **vai trò**, không theo tên thanh ghi:

  ```toml
  [debug_info]
  returns = ["abi_version", "min_compat", "features", "num_harts"]
  return_channel = { value = "abi_version", mr = ["min_compat", "features", "num_harts"] }
  # `value` → `[calling.<arch>].return_value`; `mr[i]` → `[calling.<arch>].mr_registers[i]`
  ```

- **`vifast-gen`**: đọc `[calling.*]` đã mở, sinh `DEBUG_INFO_CHANNEL_<ARCH>` (`&str` cho value +
  `[&str; 3]` cho mr) và bảng per-arch trong `docs/abi.md`; đọc `[object_size].model` + alias, sinh
  **một** bộ `OBJ_*`, và `die` nếu alias trỏ tới khoá không tồn tại/lệch.
- **`check_abi`**: (i) 3 arch alias đều phân giải về cùng bộ số; (ii) kênh trả suy ra khớp
  `[calling.*]`; (iii) bỏ vị từ `a1/a3/a4/a5`.
- **3 arch**: `KT-338b`/`KT-340c` đọc hằng sinh, không đoán. **`abi_version`/`min_compat`**: 1/1.
- **Chi phí**: trung bình (gen + checker + docs), **một lần**; hết nhân bản số.

### 9.4 Đánh giá

| Tiêu chí | 8A (điền tay) | 8B (hoãn kênh) | **8C (sinh + alias/vai trò)** |
|---|---|---|---|
| `return_channel` đúng 3 arch | Có | **Không** | **Có** |
| `object_size` đúng 3 arch | Có | Có | **Có** (một nguồn) |
| Số bản sao số phải sửa tay | 3 | 2 | **1** |
| `check_abi` thành luật suy ra, hết ghim riscv | Có (một phần) | Không | **Có** |
| `KT-338b`/`KT-340c` đọc được ngay | Có | Không (kênh) | **Có** |
| Công sức | Thấp–TB | **Thấp** | TB |
| Rủi ro lệch về sau | **Cao** | Cao | Thấp |

### 9.5 Khuyến nghị

**8C.** Lý do: (a) đúng nguyên tắc D2-11 "sinh từ `spec/`, không chép tay" — 3 arch cùng một bộ số
thì chỉ nên có **một** chỗ khai; (b) `return_channel` là **ánh xạ**, không phải dữ liệu độc lập: nó
phải suy ra từ `[calling.<arch>]`, nếu không sẽ lặp lại đúng lỗi hiện tại (ghim tên thanh ghi riscv64
vào luật toàn cục); (c) `KT-338b`/`KT-340c` cần cả hai **ngay** trong `KT-345a` — 8B để nợ đúng chỗ
đang chờ; (d) 8A đúng nội dung nhưng tạo 3 bản sao dễ lệch, không xứng với chi phí sinh hằng.

### 9.6 `.scn` nháp + kịch bản kiểm

```toml
# ĐỀ XUẤT — KT-345a chọn số/tag. Rig sim hiện chưa chạy `debug_get_info`; nếu chưa hỗ trợ,
# chuyển ca này sang host test (vẫn giữ làm .scn khi rig mở — D2-13).
version = 1
name = "32-debug-get-info-channel"
tags = ["exit"]
caps = { u0 = "Untyped", c0 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
thread = [
  { name = "t0", steps = [
    "debug_get_info => ok",
    "exit code=0 => halt",
  ] },
]
```

```text
# Kịch bản kiểm (máy kiểm + host) — KT-345a
1. DƯƠNG: [object_size].{riscv64,aarch64,x86_64} phân giải về CÙNG bộ 7 số; gen sinh đúng 1 bộ OBJ_*.
2. DƯƠNG: kênh trả suy ra: riscv64 a1/[a3,a4,a5]; aarch64 x1/[x3,x4,x5]; x86_64 rdx/[r10,r8,r9].
3. ÂM: sửa aarch64 `same_as = "riscv65"` -> vifast-gen DỪNG (alias không tồn tại).
4. ÂM: thêm khoá `tcb = 1024` vào x86_64 -> check_abi ĐỎ (3 arch phải bằng nhau).
5. ÂM: đổi `[debug_info].return_channel.mr[0]` = "min_compat" nhưng `[calling.x86_64].mr_registers[0]`
   = "r11" -> check_abi ĐỎ (kênh không khớp calling).
6. ÂM: bỏ `[calling.aarch64].mr_registers` -> gen DỪNG khi sinh kênh (không đoán).
7. TAP (KT-338b/340c): `# user getinfo: nr=15 err=0 abi_version=1 num_harts=N` trên cả 3 arch.
```

---

## 10. Nhóm 9 — Nâng trạng thái `VSpace`/`PageTable`/`Frame`

### 10.1 Hiện trạng

```text
spec/abi.toml:188-190
VSpace = { id = 9, stability = "reserved", install_at = "K2", unsupported = true, method = { 0 = { name = "GetAsid", unsupported = true, note = "debug" } }, note = "method id 1 để TRỐNG: VSpace khách là bit GUEST trong flags của Untyped.Retype (D2-32)" }
PageTable = { id = 10, stability = "reserved", install_at = "K2", unsupported = true, method = { 0 = { name = "Map", unsupported = true }, 1 = { name = "Unmap", unsupported = true } } }
Frame = { id = 11, stability = "reserved", install_at = "K2", unsupported = true, method = { 0 = { name = "Map", unsupported = true }, 1 = { name = "Unmap", unsupported = true }, 2 = { name = "GetAddress", unsupported = true }, 3 = { name = "MapIo", unsupported = true }, 4 = { name = "UnmapIo", unsupported = true } } }
```

**Ba loại này đã được dùng thật — không còn là "reserved":**

| Loại | Ca thật | Bằng chứng |
|---|---|---|
| **Frame** | `Frame.Map`/`Frame.Unmap` **đã cài** và nằm trong danh sách trắng invocation | `invoke.rs:370-371` (`match (ty, method)` → `map`/`unmap`); `invoke.rs:1078` comment "đúng ba cặp VM (Untyped.Retype, Frame.Map, Frame.Unmap)"; test `invoke.rs:1140` `retype_chi_ba_loai_vm` |
| **Frame** | Thi hành thật ở tầng object (map/gỡ PTE + `sfence.vma`), không chỉ mô hình | `mem_bridge.rs:295-330` ("`Frame.Map`/`Unmap` … qua `ObjectLayer::map/unmap` + `sfence.vma`") |
| **Frame** | IPC buffer của TCB **buộc** là cap `Frame` 4 KiB | `core_tcb.rs:208-215`; `core_tcb.rs:508` `require_type(frame, ObjType::Frame)`; `ipc.rs:222` |
| **VSpace** | `Tcb.Configure(vspace?)` kiểm **loại** `VSpace` (KT-320/KT-325) | `core_tcb.rs:308` (`require_type(slot, ObjType::VSpace)`); test `configure_tests.rs:189` `add_foreign_object(ObjType::VSpace, …)` |
| **VSpace** | `Frame.Map` lấy **cap phụ** `VSpace` (kiểm loại + quyền ghi) | `invoke.rs:915-940` (`c.ty != ObjType::VSpace ⇒ WrongObjectType`); `invoke.rs:928` |
| **VSpace** | Là đích `Untyped.Retype` hợp lệ | `invoke.rs:161-171` `retype_target`; `invoke.rs:843-853`: bit `FLAG_GUEST` chỉ dành cho **VSpace khách**, hiện trả `NotSupported` (H1 — chưa mở) |
| **PageTable** | Là đích `Untyped.Retype` hợp lệ | `invoke.rs:164-165`; test `invoke.rs:1140` `retype_supported(ObjType::PageTable)` |
| **PageTable** | Là loại object của object layer (dựng cây 3 cấp, `MissingPageTable` 12) | `mem_bridge.rs:267` (`ObjKind::PageTable => ObjType::PageTable`); `v2/sim/vifast-sim/tests/kt37_pagetable.rs:93-98` (retype PageTable + cây 3 cấp) |
| **PageTable** | Method `Map`/`Unmap` **chưa** cài (đúng: `KT-340a` mới làm W^X/VSpace `x86_64`) | `invoke.rs:22` ("root PageTable **không có cap** trong object layer"); `mem_bridge.rs:322-330` chỉ dispatch `Frame` |

Máy kiểm đang **ép** trạng thái sai này:

```text
v2/test/abi/check_reserved.py:17-26
RESERVED = {
    "VSpace": ["GetAsid"],
    "PageTable": ["Map", "Unmap"],
    "Frame": ["Map", "Unmap", "GetAddress", "MapIo", "UnmapIo"],
    "IrqControl": …, "IrqHandler": …, "IOSpace": …, "VCpu": …, "PowerControl": …,
}
```

```text
v2/test/abi/check_reserved.py:109-115
# 4) mọi entry reserved phải trả NotSupported
for name, obj in reserved.items():
    check(obj.get("unsupported") is True, f"{name} phải có unsupported = true (trả NotSupported)")
    for key, m in obj.get("method", {}).items():
        if isinstance(m, dict):
            check(m.get("unsupported") is True, …)
```

và bản sinh cũng ghim:

```text
v2/lib/vifast-abi/src/lib.rs:116-121
assert!(RESERVED_OBJECTS.iter().any(|(n, id)| *n == "VSpace" && *id == 9));
```

### 10.2 Vấn đề

1. **Spec nói ngược mã.** `Frame.Map` chạy thật (map PTE + `sfence.vma`) và là **một trong ba** cặp
   invocation được cài, trong khi spec ghi `unsupported = true`, `install_at = "K2"` (K2 đã qua).
   Theo AGENTS §1, code+harness đứng trước, nhưng `spec/` vẫn là "nguồn sự thật duy nhất cho ABI" —
   hai câu này chỉ cùng đúng nếu spec được sửa.
2. **`check_reserved.py` khoá cứng 8 object 9..16.** `KT-340a` (VSpace/W^X/shootdown `x86_64`) và
   `KT-338b`/`KT-340c` (nối core) sẽ phải gọi `Frame.Map`/`VSpace`/`PageTable` từ U-mode; nếu muốn
   thêm chữ ký/kiểm mới, họ buộc phải sửa `check_reserved.py` **giữa task** — ra ngoài cửa sổ ABI
   duy nhất (D2-234) và không có D2.
3. **`vifast-gen` sinh `RESERVED_OBJECTS` chứa 3 tên này** ⇒ `vifast-abi` (crate dùng chung 3 arch)
   đang dạy rằng `Frame`/`VSpace`/`PageTable` "chưa cài"; `docs/abi.md` cũng vậy.
4. **Rủi ro thật khi để nguyên:** `KT-340a` được giao "VSpace/W^X/shootdown" — nếu `VSpace` vẫn
   `reserved` + method `GetAsid` `unsupported`, task đó phải tự quyết định có mở ABI hay không, đúng
   thứ D2-234 cấm. Còn nếu họ lách bằng cách gọi nội bộ (không qua cap), thì `Frame.Map` (đã là ABI)
   và đường nội bộ sẽ lệch nhau — hai nguồn sự thật.
5. **Không có ca âm nào canh "reserved ⇒ NotSupported" cho 3 loại này** (vì chúng không còn reserved
   trên thực tế); `check_reserved.py` chỉ kiểm **metadata**, không chạy invocation.

### 10.3 Phương án

#### 9A — Nâng **cả ba** lên `stability = "frozen"`

- **`spec/abi.toml`**: bỏ `stability = "reserved"`/`install_at`/`unsupported` ở object; mỗi object
  `stability = "frozen"`; method nào chưa cài giữ `unsupported = true` + `install_at = "KT-340a"`
  (VSpace.GetAsid, PageTable.Map/Unmap, Frame.GetAddress/MapIo/UnmapIo).
- **`check_reserved.py`**: `RESERVED` còn 5 (12..16); thêm kiểm 3 object mới **không** được reserved.
- **`gen.py`**: `CORE_OBJECTS` 11, `RESERVED_OBJECTS` 5 (tự động theo `stability`).
- **`lib.rs:116-121`**: đổi object reserved mẫu (vd `IrqControl` id 12).
- **3 arch**: không đổi mã; **`abi_version`/`min_compat`**: 1/1.
- **Ưu:** khẳng định mạnh nhất "id/type/số method đã đóng băng". **Nhược:** đóng băng luôn **số
  method** `PageTable.Map/Unmap` trước khi `KT-340a` thiết kế xong cờ W^X/shootdown; nếu cần thêm
  method/tham số, phải mở cửa sổ ABI mới — trái mục tiêu "một cửa sổ".

#### 9B — Nâng **theo bằng chứng**: `Frame` → `frozen`; `VSpace`/`PageTable` → `core`  ← *khuyến nghị*

- **`Frame`**: `stability = "frozen"`, bỏ `unsupported` ở object; `Map`/`Unmap` đã cài (giữ nguyên
  chữ ký `[method_signature."Frame.Map"]` đã có); `GetAddress`/`MapIo`/`UnmapIo` giữ
  `unsupported = true` + `install_at = "D2/D3"` (IOMMU/MapIo ở Chu kỳ 3).
- **`VSpace`**: `stability = "core"` (như `SchedControl` — id/type đang dùng, chữ ký method còn
  `stable-draft`); bỏ `unsupported` ở object; `GetAsid` giữ `unsupported = true` + `install_at = "KT-340a"`.
- **`PageTable`**: `stability = "core"`; bỏ `unsupported` ở object; `Map`/`Unmap` giữ
  `unsupported = true` + `install_at = "KT-340a"` (root PageTable chưa có cap — `invoke.rs:22`).
- **`check_reserved.py`**: `RESERVED` còn 5 (12..16) + kiểm mới: `VSpace/PageTable/Frame` **không**
  được nằm trong `reserved`, phải có id 9/10/11.
- **`gen.py`**: tự động (`stability in {"frozen","core"}`) ⇒ `CORE_OBJECTS` 11, `RESERVED_OBJECTS` 5;
  `RESERVED_OBJECTS` vẫn sinh cho 12..16.
- **`lib.rs:116-121`**: đổi assert sang object reserved còn lại.
- **`docs/abi.md`**: `Frame` hàng "frozen", `VSpace`/`PageTable` hàng "core"; ghi rõ method nào chưa cài.
- **3 arch**: không đổi mã; **`abi_version`/`min_compat`**: 1/1; **`vifast-gen`**: không đổi logic.
- **Chi phí**: thấp (spec + 2 checker + 1 test + docs), **không** chạm `v2/kernel`.
- **Vì sao `core` chứ không `frozen` cho VSpace/PageTable:** `frozen` sẽ đóng băng **số method** trước
  khi `KT-340a` chốt thiết kế W^X/shootdown; `core` nói đúng sự thật "id + loại đã dùng, chữ ký còn
  draft" — đúng tiền lệ `SchedControl` (D2-32: "không vừa reserved vừa có chữ ký").

#### 9C — Giữ `reserved`, chỉ đổi `unsupported = false` + ghi chú

- **`spec/abi.toml`**: `stability = "reserved"` giữ; `unsupported = false`; note "đã dùng nội bộ,
  method chưa chốt".
- **Hệ quả:** `check_reserved.py:109-115` **đỏ** (reserved ⇒ phải `unsupported = true`) ⇒ phải nới
  checker, tức định nghĩa "reserved" mất nghĩa (vừa reserved vừa cài). Trái D2-32 ("object chưa cài
  giữ số reserved"); `gen.py` vẫn xếp vào `RESERVED_OBJECTS` ⇒ crate chung vẫn nói "NotSupported".
- **`abi_version`/`min_compat`**: 1/1 (không đổi wire).
- **Chi phí**: thấp nhưng **sai bản chất**; **không khuyến nghị**.

### 10.4 Đánh giá

| Tiêu chí | 9A (cả ba frozen) | **9B (theo bằng chứng)** | 9C (reserved + unsupported=false) |
|---|---|---|---|
| Spec khớp mã (`Frame.Map` đã cài) | Có | **Có** | Một phần (vẫn reserved) |
| Đóng băng số method trước `KT-340a` | **Có** (rủi ro) | Không | Không |
| `check_reserved.py` phải nới định nghĩa | Không | Không | **Có** (sai) |
| `gen.py`/`docs` phản ánh đúng | Có | **Có** | Không |
| `KT-340a`/`KT-338b`/`KT-340c` không phải tự quyết ABI | Có | **Có** | Không |
| Công sức | Thấp | **Thấp** | Thấp |
| Rủi ro phải mở cửa sổ ABI lần nữa | **Cao** | Thấp | Cao |

### 10.5 Khuyến nghị

**9B.** Lý do: (a) mỗi loại được nâng đúng mức bằng chứng — `Frame` có method **đã cài** nên
`frozen`; `VSpace`/`PageTable` mới là **kiểu object đang dùng** (Retype/Configure/Map phụ), method
chưa cài nên `core` + `unsupported` từng method; (b) không đóng băng số method trước `KT-340a`, tránh
phải mở cửa sổ ABI thứ hai; (c) `check_reserved.py` vẫn giữ nguyên tắc "reserved ⇒ NotSupported" cho
5 object 12..16 — không nới luật, chỉ cập nhật danh sách đúng thực tế; (d) gỡ đúng nợ mà D2-237 nêu,
và `KT-338b`/`KT-340c` không phải tự đoán.

### 10.6 `.scn` nháp + kịch bản kiểm

```toml
# ĐỀ XUẤT — KT-345a chọn số/tag. Rig sim cần hỗ trợ `Untyped.Retype` type=VSpace/PageTable + cap phụ
# `Frame.Map` (hiện chỉ có khuôn Frame); nếu chưa, giữ dạng text bên dưới.
version = 1
name = "33-vm-object-status"
tags = ["vm-map"]
caps = { u0 = "Untyped", c0 = "CNode", c1 = "CNode", c2 = "CNode", c3 = "CNode", ep0 = "Endpoint", ep1 = "Endpoint", ntf0 = "Notification", tcb0 = "Tcb", tcb1 = "Tcb", rep0 = "Reply", sc0 = "SchedContext" }
setup = [
  "invoke Untyped.Retype cap=u0 dst=c1 type=Frame size_bits=12 flags=0 => ok",
  "invoke Untyped.Retype cap=u0 dst=c2 type=VSpace size_bits=12 flags=0 => ok",
  "invoke Untyped.Retype cap=u0 dst=c3 type=PageTable size_bits=12 flags=0 => ok",
]
thread = [
  { name = "t0", steps = [
    "invoke Frame.Map frame=c1 vspace=c2 va=0x1000 flags=3 => ok",
    "exit code=0 => halt",
  ] },
]
```

```text
# Kịch bản kiểm (checker + host) — KT-345a
1. DƯƠNG: [object].Frame.stability = "frozen", KHÔNG còn unsupported ở object; Frame.Map đã cài.
2. DƯƠNG: VSpace/PageTable stability = "core"; GetAsid/Map/Unmap vẫn unsupported = true.
3. DƯƠNG: generated.rs CORE_OBJECTS có đủ 11 (1..11), RESERVED_OBJECTS còn 5 (12..16).
4. ÂM: để Frame unsupported = true -> check_abi ĐỎ (frozen mà method đã cài thì không được unsupported).
5. ÂM: đưa VSpace trở lại RESERVED -> check_reserved ĐỎ (id 9 không được reserved sau khi nâng).
6. ÂM: đổi Frame.id = 12 -> check_reserved/check_abi ĐỎ (trùng IrqControl; dải 9..11 đã cấp).
7. ĐỘT BIẾN: bỏ Frame.Map khỏi danh sách trắng invoke -> ca 1 ĐỎ (map trả NotSupported thay vì ok).
8. Host: `KT-340a` chỉ được thêm method mới bằng task có D2 — không tự sửa spec (D2-234).
```

---

## 11. Tổng hợp tác động lên `spec/` (nếu Agent 0 ký đúng khuyến nghị)

| Tệp | Thay đổi đề xuất | Loại | Ai làm |
|---|---|---|---|
| `spec/kernel.toml` | **Tạo mới** (nhóm 1) + `[arch.x86_64.gdt]` (nhóm 5); `[reserved]` cho phần chưa quyết | mới, không wire | KT-345a |
| `spec/abi.toml` | +`[ipc_buffer]` (2C); +`[cspace]` (3.2A); +`errors`/`note` của `Tcb.Configure` (3.1A) | **thêm mục, không đổi byte** | KT-345a |
| `spec/abi.toml` | `[calling.x86_64]`: +`return_message_info = "r11"` (7C); `clobbered` thêm `r11/r10/r8/r9/r12`; `preserved` bỏ `r12`; `trap` **giữ** `int 0x80` (5A chỉ chốt hướng) | thêm trường + sửa danh sách, **không đổi byte wire** | KT-345a |
| `spec/abi.toml` | `[debug_info].return_channel` theo **vai trò**; `[object_size]` 3 khoá alias `same_as = "model"` (8C) | metadata, không wire | KT-345a |
| `spec/abi.toml` | `[object]`: `Frame` → `frozen`; `VSpace`/`PageTable` → `core`, method chưa cài giữ `unsupported` (9B) | metadata, không wire | KT-345a |
| `spec/protocols/socket.vidl` | **Tạo mới** (6A) | mới, không wire | KT-345a |
| `spec/protocols/meminfo.vidl`, `fsstat.vidl` | **Tạo mới** (4C) | mới, không wire | KT-345a |
| `spec/v1-parity.toml` | `free`/`df`: `deferred-K7` → `active` **sau khi** chạy thật | cho phép bởi D2-234 | KT-345b |

**Bộ kiểm phải sửa (và chỉ sửa đúng các dòng này):**

| Tệp | Sửa gì | Vì sao |
|---|---|---|
| `v2/test/abi/check_abi.py` | thêm rule cho `[ipc_buffer]`, `[cspace]`, `errors` của `Tcb.Configure` | spec có mục mới (2C/3.1A/3.2A) |
| `v2/test/abi/check_abi.py` | `:235-238` kiểm `return_channel` **3 arch** suy ra từ `[calling.*]`; `:326/328/338/345` thêm `return_message_info`; `:363-364` giữ MR; `:368-370` kiểm 3 khoá `object_size` bằng nhau | 7C + 8C |
| `v2/test/abi/check_reserved.py` | `RESERVED` 8 → **5** (bỏ `VSpace`/`PageTable`/`Frame`); kiểm 3 object đã nâng **không** còn reserved + id 9/10/11 | 9B |
| `v2/test/abi/check_abi.py` | `:60-70` thêm `VSpace` 9 / `PageTable` 10 / `Frame` 11 vào `CORE_OBJECTS`/`CORE_IDS` | 9B |
| `v2/tools/vifast-gen/gen.py` | `:205-214` sinh thêm `return_message_info`; `:140` đọc 3 khoá `object_size` + assert bằng nhau; sinh kênh `[debug_info]` từ `[calling.*]`; `gen_md` thêm cột/mục | 7C + 8C |
| `v2/tools/vifast-gen/gen_idl.py` | quét thêm `spec/protocols/` (nhóm 6) | 6A/4C |
| `v2/test/kernel/check_kernel.py` | **mới**: spec↔mã (nhóm 1), gồm GDT (nhóm 5) | chống lệch 5 bản `MAX_HARTS` |
| `v2/kernel/vifast-plat-x86_64/src/calling.rs` + `tests/host.rs:736-768` | `INT80_CLOBBERED` thêm `R11` + 4 MR; `INT80_RETURN_MSGINFO_REG = R11`; assert bảng cập nhật | 7C |
| `v2/lib/vifast-abi/src/lib.rs:116-121` | đổi assert `RESERVED_OBJECTS` (VSpace id 9 → object còn reserved, vd `IrqControl` id 12) | 9B |
| `v2/test/scn/check_scn.py` | +tag mới (nếu có) + số tệp (27 → 27+n) **sau khi** thêm `.scn` thật | KT-345a thêm ca |
| `v2/bin/v2` | thêm bước `check_kernel` (ID **sau `KT-333a/b`**) | bước cổng mới |

**Không** sửa: `[syscall]` (4C **không** thêm số), `[message_info]` (2C **không** đổi), `[error]` (chỉ thêm
`errors` của `Tcb.Configure`, không đổi bảng mã). **Không** đổi `abi_version`/`min_compat` (xem §0.1).

---

## 12. Việc `KT-345a`/`KT-345b` phải làm (checklist để Agent 0 đối chiếu khi mở task)

**`KT-345a` — áp `spec/` + `vifast-gen` (tiền đề của `KT-338b`/`KT-340c`, 2 h):**

1. Áp **đúng** các mục đã duyệt vào `spec/` (`kernel.toml`; `[ipc_buffer]`, `[cspace]`, `errors` của
   `Tcb.Configure`; `[calling.x86_64]`; `[debug_info]`/`[object_size]`; `[object]`;
   `spec/protocols/{socket,meminfo,fsstat}.vidl`). **Không** đổi `abi_version`/`min_compat` nếu duyệt
   đúng khuyến nghị (§0.1).
2. Sửa bộ kiểm **đúng các dòng** ở §11: `check_abi.py:60-70/235-238/326/328/338/345/363-364/368-370`,
   `check_reserved.py:17-26`, `lib.rs:116-121`, `calling.rs` + `host.rs:736-768`.
3. `vifast-gen gen` + `vifast-idl gen`; commit tệp sinh; `--check` xanh. Thêm
   `v2/test/kernel/check_kernel.py` (+ `--selftest` âm/dương) và bước cổng `check_kernel` trong
   `v2/bin/v2` — **rebase sau `KT-333a/b`**, kiểm trùng `lane_step` **cả hai** hàm (D2-235(5)).
4. Ca âm/đột biến **bắt buộc** cho metadata: `return_message_info` trùng kênh ⇒ đỏ; `preserved` chứa
   `r12` ⇒ đỏ; `object_size` 3 arch lệch ⇒ đỏ; `Frame` còn `unsupported` ⇒ đỏ; `RESERVED_OBJECTS` còn
   `VSpace` ⇒ đỏ; `return_channel` lệch `[calling.*]` ⇒ đỏ.
5. `.scn` mới (nếu thêm) + tag + số tệp (27 → 27+n); **không** `#[ignore]`, **không** nới ngưỡng.
6. **T2** (`./v2/bin/v2 gate --fast`) PASS trên ứng viên tích hợp — chạy **sau** khi `KT-330` nhả máy đo.

**`KT-345b` — thi hành tính năng (3 h, sau `KT-345a` + `KT-342` + `KT-343`):**

7. `NetSvc` phục vụ **≥ 2 caller** (bảng phiên + badge), có **2 ca âm** + **1 đột biến ĐỎ** mỗi hành vi
   (theo IDL đã duyệt ở nhóm 6).
8. `free`/`df`/`loop` chạy thật; `spec/v1-parity.toml` lật `active`; parity **25/28 → 28/28** (D2-239: 25
   trước `KT-345b`, 28 = 100 % sau).
9. Parity `--mode serial` xanh **28/28** ở `smp=1` và `smp=4`; **T2** PASS.

**Rủi ro đã biết:**

| Rủi ro | Dấu hiệu | Cách chặn |
|---|---|---|
| `KT-333a/b` sửa `v2/bin/v2` cùng lúc | trùng `lane_step` ID ⇒ bước bị bỏ im lặng | rebase + `grep -n lane_step` **cả hai** hàm (D2-235(5)) |
| `gen.py` cứng `0..15` | `vifast-gen: syscall phải 0..15` | nếu chọn 4A: sửa **trước** khi thêm số |
| Kỳ vọng harness ghim selector/version | TAP 24, `tr=0x28`, `abi_version=1` | sửa **đúng** dòng đã liệt ở §0.1/§6.1 (không đổi ngưỡng) |
| Checker `kernel.toml` bằng regex dễ giòn | đỏ oan khi format `gdt.rs` đổi | đối chiếu **theo tên hằng**, không theo dòng; có `--selftest` âm/dương |
| IDL generator giới hạn 4 scalar/1 cap | `socket.vidl` không parse | giữ đúng giới hạn; payload lớn để `ring` (sau) |
| `KT-345a` chậm ⇒ `KT-338b`/`KT-340c` chờ | 2 task BLOCKED quá 1 vòng | đẩy `KT-344 → REVIEW` sớm (D2-237); không thêm phạm vi vào `KT-345a` |

---

## 13. Những gì **KHÔNG** đưa vào gói (ghi để khỏi hỏi lại)

1. **`Suspend(Defer)` / `Restart`** — D2-228 đã chốt, spec đã đủ (`Tcb` method 3 + `Resume` 12); không mở.
2. **`TcbInfo`/`TimerInfo`** — thiết kế:494 bỏ cùng `MemInfo`; không có nhu cầu trong Chu kỳ 2.
3. **`loop`** — D2-239 giao `KT-345b` đóng nốt `loop` (**28/28**); gói này **không** đề xuất gì cho `loop`
   vì nó dùng `ClockRead` (11) đã có, không cần ABI. Nếu parity cần, `KT-345b` tự quyết trong DoD.
4. **`spec/deps.toml`** — thiết kế:680 có nhắc, nhưng chưa task nào cần; không thuộc 9 nhóm (D2-234/D2-237).
5. **Đổi `MessageInfo` 35 bit** — chỉ xuất hiện như **phương án 2A bị khuyến nghị loại**, không phải đề xuất.

---

## 14. Đề nghị Agent 0 ký (câu chữ D2 mẫu — số D2 do Agent 0 cấp)

> **D2-XXX (ABI v1.1 — nhóm 1).** Tạo `spec/kernel.toml` **tối thiểu**: `[limits]`/`[limits.core]`/`[limits.plat]`
> (mỗi giá trị có `source` trỏ mã), `[mailbox]`, `[features]` (tên, `trace_always_on = true`),
> `[arch.x86_64.gdt]` (hình học **hiện tại**), `[reserved]` cho phần chưa quyết. Thêm
> `v2/test/kernel/check_kernel.py` đối chiếu spec↔mã; **chưa** thêm target `vifast-gen`, **chưa** đổi consumer.
> `abi_version`/`min_compat` giữ 1.

> **D2-XXX (nhóm 2).** `extra_caps` là **ABI** qua `[ipc_buffer]` (đóng băng `n` tại 560, CPTR 568..600, `dst`
> 600; `extra_caps_max = 4`). **Không** thêm bit vào `MessageInfo`; `extra_caps_unwrapped` (24..27) giữ nguyên
> nghĩa unwrap (KT-63). `abi_version = 1`.

> **D2-XXX (nhóm 3).** (a) `SyscallEnv::tcb_vspace` đổi sang **3 trạng thái**; arch không giữ được ⇒
> `Tcb.Configure` trả **`Busy` (14)** và **rollback** `TcbData.vspace`; nền tảng không U-mode = `Unsupported`
> (vẫn `ok`). (b) Thêm `[cspace]`: ô 0 = `CS_SELF`, CPTR 0 = "vắng" (`vspace?` = xoá theo KT-325); cap đặt ô 0
> **không** truyền được qua CPTR. Không đổi wire.

> **D2-XXX (nhóm 4).** `free`/`df` **không** thêm syscall/object: `df` = `OP_STATFS` trong hợp đồng `[TK]` của
> `proc_fs`; `free` = dịch vụ kernel `MemSvc` cùng khuôn `net_svc`. Protocol khai bằng IDL
> (`spec/protocols/{meminfo,fsstat}.vidl`). `spec/v1-parity.toml` lật `free`/`df` → `active` khi chạy thật.

> **D2-XXX (nhóm 5).** Chốt **thứ tự GDT đích** cho `syscall`/`sysret`: `null, kcode, kdata, **udata**,
> **ucode**, TSS` (7 entry; `STAR[63:48] = 0x10`). Trong v1.1 **không** đổi `[calling.x86_64].trap` (vẫn
> `int 0x80`) và **không** sửa mã; ghi hình học hiện tại + `planned` vào `kernel.toml`. Việc chuyển
> `syscall`/`sysret` (+`swapgs`/per-CPU) là task **K5' full** riêng, giữ cổng `int 0x80` để `min_compat = 1`.

> **D2-XXX (nhóm 6).** Thêm `spec/protocols/socket.vidl` (`open/connect/send/recv/close/poll`, `handle` do
> server cấp); netsrv U-mode giữ **bảng phiên** có biên, danh tính = **badge**, đầy ⇒ `Busy` (14);
> **kernel `NetSvc` giữ đơn caller**. `gen_idl.py` quét thêm `spec/protocols/`; `ring`/`pager` giữ nguyên tới
> KT-346. Không đụng `abi.toml`.

> **D2-XXX (nhóm 7).** `[calling.x86_64]` thêm **`return_message_info = "r11"`** (int 0x80 không clobber
> `r11`); `return_error = "rax"`, `return_value = "rdx"` **giữ nguyên**; `clobbered` =
> `["rax","rdx","r11","r10","r8","r9","r12"]`, `preserved = ["rsp","rbx","rbp","r13","r14","r15"]` (r12 là
> MR3 **vào/ra**, không preserved). Khi chuyển `syscall`/`sysret` (K5') `return_message_info` đổi sang `r13`.
> Không đổi `message_layout`; `min_compat = 1`, `abi_version = 1`.

> **D2-XXX (nhóm 8).** `[object_size]` khai **một** bộ `model` + 3 khoá `riscv64`/`aarch64`/`x86_64` =
> `{ same_as = "model" }`; `[debug_info].return_channel` theo **vai trò** (`value`/`mr[3]`), `vifast-gen` suy
> ra thanh ghi từ `[calling.<arch>]` và `die` nếu lệch; `check_abi.py` kiểm 3 arch bằng nhau + kênh khớp
> `[calling.*]`. `abi_version = 1`.

> **D2-XXX (nhóm 9).** `[object]`: `Frame` → `stability = "frozen"` (Map/Unmap **đã cài**);
> `VSpace`/`PageTable` → `stability = "core"` (id/type đang dùng: `Untyped.Retype`, `Tcb.Configure(vspace?)`,
> `Frame.Map`), method chưa cài giữ `unsupported = true` + `install_at = "KT-340a"`.
> `check_reserved.py` còn **5** object reserved (12..16); `lib.rs:116-121` đổi assert. `abi_version = 1`.

**Ký ở đâu:** một dòng `APPROVED <YYYY-MM-DD> Agent 0` ngay dưới mục `KT-344` trong `v2/agent/QUEUE.md`
(AGENTS.md §8), kèm các mục D2 ở trên trong `v2/agent/DECISIONS.md`. **Không** duyệt bằng chat.

---

## 15. Xác nhận của người soạn

- **Không** sửa `spec/`, `v2/kernel/**`, `v2/lib/**`, `v2/bin/v2`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
  `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/**`.
- Tệp này là tệp **duy nhất** được tạo bởi KT-344 (cộng báo cáo của lead).
- Chỉ đọc + `python3`/`grep`; **không** chạy QEMU, **không** `cargo build`, **không** push/merge.
- Mọi trích dẫn `spec/` là **nguyên văn** (chỗ cắt ngắn đều đánh dấu `…`), kèm `tệp:dòng`; mọi khối TOML/IDL là **đề xuất chưa áp dụng**.
- Không có số đo viết tay: tài liệu chỉ dùng **hằng số thiết kế** và **ngưỡng đã chốt trong D2**.

**Nguồn đã đọc:** `KE-HOACH-CHU-KY-2-2026-11-22.md`; `v2/agent/{DECISIONS,QUEUE}.md` (D2-228, D2-234,
D2-235, D2-04, D2-15, D2-53, D2-65, D2-72, D2-195); `spec/abi.toml`, `spec/v1-parity.toml`;
`v2/agent/JOURNAL/{kt309-captransfer,kt311-cfgfull,kt325-vspace2,kt281-netsrv,kt290-cfg,kt67a-journal,review-2026-11-12-16}.md`; `v2/agent/QUEUE.md` (KT-344/KT-345a/KT-345b), D2-236..D2-240;
`v2/kernel/vifast-core/src/{ipc,invoke,syscall,core_capxfer,core_tcb,machine,mailbox}.rs`;
`v2/kernel/vifast-plat-riscv64/src/{user_run,ipc_buffer}.rs`;
`v2/kernel/vifast-plat-x86_64/src/{gdt,umode,calling,asm,control}.rs` + `tests/{exc,host,smp}.rs`;
`v2/kernel/vifast-kbin-x86_64/src/umode.rs`; `v2/lib/vifast-abi/src/{lib.rs,generated.rs}`;
`v2/kernel/vifast-plat-common/src/{smp,obj/mod}.rs`;
`v2/kernel/vifast-kbin-riscv64/src/{net_svc,proc_fs}.rs`;
`v2/lib/vifast-idl/{idl/ring.vidl,idl/pager.vidl,src/lib.rs}`;
`v2/tools/vifast-gen/{gen.py,gen_idl.py}`; `v2/test/abi/{check_abi,check_reserved}.py`;
`v2/test/scn/check_scn.py`; `v2/test/boot/{run-x86_64.sh,ktest.sh}`;
`v2/kernel/vifast-plat-riscv64/src/mem_bridge.rs`; `v2/sim/vifast-sim/tests/{kt37_pagetable,kt59_core_parity}.rs`;
`v2/kernel/vifast-core/src/{spec.rs,tls_tests.rs,configure_tests.rs}`;
`v2/docs/{k5-x86_64-boot.md,k1c-ipc-semantics.md,k3b-core.md,d1-net.md}`;
`docs/v2/ViFastOs-v2-thiet-ke.md` (§4.6, §4.22, §6.7, §8); `AGENTS.md`.

---

## Phụ lục A — `spec/kernel.toml` nháp (ĐỀ XUẤT, **chưa** tạo)

```toml
# SPDX-License-Identifier: MIT OR Apache-2.0
# ĐỀ XUẤT KT-344 — CHƯA ÁP DỤNG. Chỉ chứa giá trị ĐÃ CÓ quyết định hoặc ĐÃ CÓ trong mã.
schema = 1
status = "REVIEW"          # Agent 0 đặt "APPROVED" khi ký; sau đó chỉ Agent 0 sửa (D2-22)
name = "vifast-kernel-config"

[limits]
max_harts = 8              # D2-72 Q4, D2-195 Q5
max_harts_hard = 64        # thiết kế §4.22 [TK] — trần cấu hình, chưa dùng
source = [
  "v2/kernel/vifast-core/src/machine.rs:64",
  "v2/kernel/vifast-plat-common/src/smp.rs:31",
  "v2/kernel/vifast-plat-x86_64/src/smp.rs:54",
  "v2/kernel/vifast-plat-aarch64/src/smp.rs:48",
  "v2/sim/vifast-sim/src/kobj.rs:150",
]

[limits.core]              # bảng tĩnh của MODEL (vifast-core)
max_objects = 256
max_caps = 512
max_tcbs = 64
max_sched_contexts = 32
max_endpoints = 16
max_replies = 32
max_notifications = 16
max_queued_messages = 24
source = "v2/kernel/vifast-core/src/machine.rs:42-62"

[limits.plat]              # bảng tĩnh của NỀN TẢNG (vifast-plat-common)
max_objects = 96
max_caps = 160
max_untypeds = 4
max_mappings = 64
max_flushes = 16
source = "v2/kernel/vifast-plat-common/src/obj/mod.rs:74-86"

[mailbox]
cap = 8                    # core + sim
max_mail_log = 32
source = ["v2/kernel/vifast-core/src/mailbox.rs:24,27", "v2/sim/vifast-sim/src/mailbox.rs:28"]

[features]
names = ["debug", "test"]  # tên feature đang dùng (Cargo) — CHƯA gán bit
trace_always_on = true     # D2-09: trace không có cờ tắt

[arch.x86_64.gdt]          # hình học HIỆN TẠI (nhóm 5); đổi cùng lúc với gdt.rs
entries = 7
sysret_ready = false
planned = "dao thu tu: user data 0x18 truoc user code 0x20 (D2 moi)"
order = ["null", "kernel_code", "kernel_data", "user_code", "user_data", "tss"]
selector_kernel_code = 0x08
selector_kernel_data = 0x10
selector_user_code = 0x18
selector_user_data = 0x20
selector_tss = 0x28
source = "v2/kernel/vifast-plat-x86_64/src/gdt.rs:33-56"

[reserved]
# Chưa quyết / chưa có mã — CẤM generator sinh, CẤM consumer đọc.
keys = [
  "trace_ring_bytes",   # §4.22, chưa có trace ring trong kernel
  "max_refills",        # §4.22, chưa có refill
  "cluster_defs",       # §4.22, chưa có cụm
  "features.bits",      # DebugGetInfo.features chưa có bit-map
  "kpti",               # tuỳ chọn bảo mật x86
]
```

## Phụ lục B — `spec/protocols/socket.vidl` nháp (ĐỀ XUẤT)

```text
# SPDX-License-Identifier: MIT OR Apache-2.0
# ĐỀ XUẤT KT-344 — giao thức socket giữa client U-mode và net server U-mode.
# Giới hạn generator (gen_idl.py): ≤ 4 scalar (MR0..MR3) + ≤ 1 cap/method; returns u32|u64|unit.
# Method id sinh tuần tự 0..N-1 (D2-102c) — không phải số trong spec/abi.toml.
#
# Đa caller: `handle` do server cấp; danh tính = badge của cap client dùng để Call.
# Bảng phiên có biên; đầy ⇒ mở mới trả Busy (14) — fail-closed.
interface socket version 1

# client -> server: mở phiên; `proto` 0 = ICMP/echo (mức hiện có), 1 = TCP (N1).
# trả `handle` (u32) do server cấp; 0 = không cấp.
method open
  param proto u32
  param flags u32
  returns u32

# client -> server: kết nối phiên `handle` tới `ip:port`.
method connect
  param handle u32
  param ip u32
  param port u32
  returns u32

# client -> server: gửi `len` byte đầu của `buf` (Frame do client cấp).
method send
  param handle u32
  param len u32
  cap buf frame
  returns u32

# client -> server: nhận tối đa `len` byte vào `buf`.
method recv
  param handle u32
  param len u32
  cap buf frame
  returns u32

# client -> server: trạng thái phiên (bit 0 = readable, bit 1 = writable, bit 2 = closed).
method poll
  param handle u32
  returns u32

# client -> server: đóng phiên; handle không còn hiệu lực.
method close
  param handle u32
  returns u32
```

## Phụ lục C — `spec/protocols/{meminfo,fsstat}.vidl` nháp (ĐỀ XUẤT, nhóm 4C)

```text
# SPDX-License-Identifier: MIT OR Apache-2.0
# meminfo.vidl — hỏi dung lượng; trả u64 để không cần payload MR (gen_idl hỗ trợ returns u64).
interface meminfo version 1

# tổng dung lượng quản lý được (byte)
method total
  returns u64

# dung lượng còn cấp được (byte)
method free
  returns u64
```

```text
# SPDX-License-Identifier: MIT OR Apache-2.0
# fsstat.vidl — số liệu volume cho `df`; thay mục deferred-K7 "STATFS".
interface fsstat version 1

# tổng số block của volume (block_size lấy từ block_size)
method blocks
  returns u64

# số block còn trống
method blocks_free
  returns u64

# cỡ block (byte)
method block_size
  returns u64
```

## Phụ lục D — các đoạn `spec/abi.toml` đề xuất (ĐỀ XUẤT, **chưa** áp dụng)

```toml
# (2C) — chèn sau [message] / trước [object_size]
[ipc_buffer]
bytes = 4096
align = 16
mr_offset = 80
mr_words = 60
extra_caps_offset = 560
extra_caps_max = 4
extra_caps_dst_offset = 600
configure_offset = 608      # vùng [TK] của KT-311/KT-320 — CHƯA đóng băng
note = "Chỉ vùng extra caps là ABI trong gói v1.1; phần còn lại giữ [TK]."
```

```toml
# (3.1A) — thay khối [method_signature."Tcb.Configure"] hiện có
[method_signature."Tcb.Configure"]
params = ["tcb", "cspace?", "vspace?", "entry?", "ipc_buffer?", "priority?"]
returns = []
errors = ["InvalidCapability", "WrongObjectType", "InsufficientRights", "InvalidArgument", "Busy"]
note = "`Busy` (14) = nền tảng không giữ được VSpace (bảng khe arch đầy) — fail-closed, rollback TcbData.vspace. Nền tảng không U-mode = Unsupported, vẫn trả ok."
```

```toml
# (3.2A) — mục mới, đặt sau [frozen]
[cspace]
slot0 = "CS_SELF"
cptr_zero = "absent"
note = "Ô 0 = cap CNode của chính CSpace, không chuyển nhượng; CPTR 0 = vắng (với `vspace?` = xoá, KT-325). Cap đặt ô 0 phải CNode.Move sang ô khác trước khi dùng làm tham số."
```

---

## Phụ lục E — `[calling.x86_64]` đề xuất (7C)

```toml
# ĐỀ XUẤT KT-344 — CHƯA ÁP DỤNG. Thay khối [calling.x86_64] hiện có.
[calling.x86_64]
syscall_number = "rax"
args = ["rdi", "rsi"]
message_info = "rdx"
mr_registers = ["r10", "r8", "r9", "r12"]
return_error = "rax"           # 0 = thành công, khác 0 = mã lỗi (1..19)
return_value = "rdx"           # KHÔNG đổi (kênh value đang chạy)
return_message_info = "r11"    # MỚI: MessageInfo trả về của Recv/NBRecv/ReplyRecv/... (int 0x80 không clobber r11)
trap = "int 0x80"
stack = "RSP user → RSP0 của TSS khi vào CPL0; `int 0x80` đẩy SS/RSP/RFLAGS/CS/RIP, `iretq` khôi phục đúng RSP user"
clobbered = ["rax", "rdx", "r11", "r10", "r8", "r9", "r12"]   # error, value, info, MR0..MR3 (MR3 là KÊNH RA của Recv)
ipc_buffer = "Frame bind qua Tcb.Configure; header chứa receive window (CPTR, ≤4 cap)"
preserved = ["rsp", "rbx", "rbp", "r13", "r14", "r15"]        # r12 RA KHỎI preserved: nó là MR3 vào/ra
note = "Đối xứng [calling.riscv64]/[calling.aarch64], trừ kênh MessageInfo: riscv64/aarch64 tái dùng thanh ghi message_info (a2/x2) làm kênh trả, x86_64 không thể vì `rdx` đã là value ⇒ khai `return_message_info = r11` (int 0x80 không clobber r11). Khi chuyển sang `syscall`/`sysret` (K5') `r11` bị lệnh clobber ⇒ đổi `return_message_info` sang `r13` (đúng hằng SYSCALL_RETURN_MSGINFO_REG)."
```

## Phụ lục F — `[debug_info]` + `[object_size]` đề xuất (8C)

```toml
# ĐỀ XUẤT KT-344 — CHƯA ÁP DỤNG. Thay khối [debug_info] hiện có.
[debug_info]
number = 15
status = "frozen"
returns = ["abi_version", "min_compat", "features", "num_harts"]
# Kênh trả theo VAI TRÒ, không theo tên thanh ghi: `value` → [calling.<arch>].return_value,
# `mr[i]` → [calling.<arch>].mr_registers[i]. vifast-gen sinh hằng per-arch + bảng docs.
return_channel = { value = "abi_version", mr = ["min_compat", "features", "num_harts"] }
note = "Luôn có; features là bitmap tính năng runtime dùng để kiểm tương thích lúc khởi động; không trả địa chỉ kernel."
```

```toml
# ĐỀ XUẤT KT-344 — CHƯA ÁP DỤNG. Thay khối [object_size] hiện có.
[object_size]
slot = { bytes = 48, align_bytes = 16, note = "QD §3.9 [TK] — sửa từ 32 B" }
default_align_bytes = 16
page_bytes = 4096
untyped_min_bytes = 4096
# Hằng MODEL dùng chung 3 arch (core no_std không cfg theo arch). Arch nào cần cỡ khác
# (vd TCB + ngữ cảnh FPU/SIMD — D2-240 trụ cột 5) phải mở ABI riêng, không sửa alias.
model = { tcb = 512, endpoint = 64, notification = 64, reply = 64, sched_context = 256, sched_control = 64, cnode_base = 64 }
riscv64 = { same_as = "model" }
aarch64 = { same_as = "model" }
x86_64 = { same_as = "model" }
```

## Phụ lục G — `[object]` trạng thái VM đề xuất (9B)

```toml
# ĐỀ XUẤT KT-344 — CHƯA ÁP DỤNG. Thay ba dòng VSpace/PageTable/Frame trong [object].
Frame = { id = 11, stability = "frozen", method = { 0 = { name = "Map" }, 1 = { name = "Unmap" }, 2 = { name = "GetAddress", unsupported = true, install_at = "D2/D3" }, 3 = { name = "MapIo", unsupported = true, install_at = "D3" }, 4 = { name = "UnmapIo", unsupported = true, install_at = "D3" } }, note = "Map/Unmap ĐÃ CÀI (KT-67a); GetAddress/MapIo/UnmapIo chưa (IOMMU ở D3)" }
VSpace = { id = 9, stability = "core", method = { 0 = { name = "GetAsid", unsupported = true, install_at = "KT-340a", note = "debug" } }, note = "id/type ĐANG DÙNG: đích Untyped.Retype, tham số vspace? của Tcb.Configure (KT-320/325), cap phụ của Frame.Map. method id 1 để TRỐNG (D2-32)" }
PageTable = { id = 10, stability = "core", method = { 0 = { name = "Map", unsupported = true, install_at = "KT-340a" }, 1 = { name = "Unmap", unsupported = true, install_at = "KT-340a" } }, note = "id/type ĐANG DÙNG (Untyped.Retype, MissingPageTable 12); method chưa cài — root PageTable chưa có cap (invoke.rs:22)" }
```
