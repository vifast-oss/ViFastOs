<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Gói thẩm định **Hợp nhất Kiến trúc & ABI Sạch** — RFC (KT-416, **8 nhóm**)

**Ngày:** 2026-11-26 · **Nhánh:** `task/kt416-rfc-abi` · **Base:** `main` @ `fc998aa8`
**Người soạn:** sub-agent `kt416` (khảo sát) + Lead (hoàn tất sau khi agent hết ngân sách) · **Người ký:** **Agent 0**

> **Đây KHÔNG phải duyệt.** Duyệt chỉ có giá trị bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` ghi trong
> [`v2/agent/QUEUE.md`](../../v2/agent/QUEUE.md) (AGENTS.md §8), kèm mục D2 mới nếu quyết định đổi nội dung ABI.
>
> **KHÔNG có byte nào của `spec/` bị sửa bởi KT-416** (D2-04, D2-22). Mọi khối TOML/IDL dưới đây là
> **ĐỀ XUẤT — CHƯA ÁP DỤNG**. `KT-417` chỉ mở sau `APPROVED` riêng của Agent 0.

**Nguồn yêu cầu:** `v2/agent/QUEUE.md` dòng `KT-416`; `KE-HOACH-KIEN-TRUC.md` dòng `KT-416`;
`v2/agent/DECISIONS.md` **D2-259**, **D2-260**, **D2-16**, **D2-17**, **D2-21**, **D2-22**.

---

## 0. TL;DR — 8 nhóm, một bảng

| # | Nhóm nợ | Hiện trạng đo được | Đề xuất | Tệp `spec/` đề nghị chạm | Đổi **byte wire**? | `abi_version` |
|---|---|---|---|---|---|---|
| 1 | Hợp nhất 2 Object/Cap Layer | `plat-common::obj` (1.9k LOC) + `vifast-core` (`capalloc`/`mdb`/`core_capxfer`) cùng giữ cây cap; `spec/kernel.toml` có `[limits.plat]` | Dời `obj` → `vifast-core`; xoá `[limits.plat]`; **sửa `check_kernel.py:277`** (đang đòi mục này) | `spec/kernel.toml` | Không | 1 |
| 2 | `IrqControl`/`IrqHandler` + `Notification` | `Tcb.BindNotification`(13)/`UnbindNotification`(14) có số trong spec nhưng **chỉ model API** (`core_sync.rs:176,208`), **không** nhánh dispatch trong `invoke.rs` | Nối dispatch 13/14; `IrqHandler.SetNotification` đi qua cùng đường | `spec/abi.toml` (`[object]`, `[method_signature]`) | **Có** (thêm nhánh) | **2** |
| 3 | `Frame.MapIo`/`UnmapIo` + `IOSpace` | Chỉ enum `ObjType::IOSpace` (`error.rs:63`) + ghi chú `vifast-virtio/src/features.rs:22` ("không IOMMU: DMA dùng PA thật"); **0 dòng** method | Thêm `IOSpace` + 2 method; hoặc **hoãn** và ghi `unsupported` có lý do | `spec/abi.toml` (`[object]`) | **Có** (method mới) | **2** |
| 4 | `Tcb.Exec` + method `VSpace`/`PageTable` | `Tcb.Exec` có số; `VSpace`/`PageTable` là đích `Untyped.Retype` nhưng method `unsupported` | Cài `Tcb.Exec` (nạp ELF U-mode) + method `VSpace.Map/Unmap` | `spec/abi.toml` | **Có** | **2** |
| 5 | GDT `planned` cho `syscall`/`sysret` `x86_64` giữ `7C-r13` | GDT KT-261 đặt user data **sau** user code; `sysret` đòi `CS−8`; `7C-r13` là ràng buộc đã ghim | Ghi **hình học GDT đích** vào `kernel.toml` (`[arch.x86_64.gdt]`), **chưa** đổi trap | `spec/kernel.toml` | Không | 1 |
| 6 | `CNode.Mutate`/`Rotate` + MCS refill đa đoạn | `CNode` có `Insert`/`Move`/`Revoke`; `Mutate`/`Rotate` **chưa có**; refill MCS một đoạn (`D2-16`) | Thêm 2 method + refill đa đoạn (giữ `D2-16`) | `spec/abi.toml` | **Có** | **2** |
| 7 | `[profiles]` `mission`/`general` + BootArena + vùng FPU/SIMD | `StaticStore`/`UntypedStore` đã có (`store.rs:271,408`); **chưa có tên profile trong spec**; `asm/fpu.rs` chỉ 62 dòng CR0/CR4, `TcbCore` **không** có trường FPU | Thêm `[profiles]` + `BootArena` (617.208 B đo được) + vùng lưu FPU ngoài `TcbCore` | `spec/kernel.toml` | Không | 1 |
| 8 | Hợp đồng `BootCapLayout` thay 16 hằng `EP_*` | `proc_abi.rs` khai **17** `EP_*` + 8 `SLOT_*`/`LOOP_*` (kế hoạch ghi 16 — **đo được 17**) | `BootCapLayout` phủ **17** `EP_*` + nhóm `SLOT_*`; bỏ hardcode | `spec/abi.toml` (`[boot_caps]`) | **Có** (layout mới) | **2** |

**Câu chốt:** 3 nhóm (2, 3, 4) **đổi byte wire** ⇒ `abi_version` 1→2; 2 nhóm (1, 7) chỉ đổi
`spec/kernel.toml`; 1 nhóm (5) chỉ ghi `planned`; 1 nhóm (6) đổi wire; 1 nhóm (8) đổi layout boot.
**Đây là câu hỏi lớn nhất cho Agent 0**: có chấp nhận `abi_version = 2` trong một lần không, hay
tách 3 nhóm wire thành đợt sau?

---

## 1. Nhóm 1 — Hợp nhất 2 Object/Cap Layer

**Hiện trạng đo được**
- `v2/kernel/vifast-plat-common/src/obj.rs` — object layer theo `CapId`/Untyped, dùng bởi `riscv64`.
- `v2/kernel/vifast-core/src/{capalloc.rs,mdb.rs,core_capxfer.rs}` — cây cap/MDB **thứ hai**.
- `spec/kernel.toml` có `[limits.plat]`; `v2/test/kernel/check_kernel.py:277` **đòi** mục này tồn tại
  (`required` list) ⇒ xoá nó **bắt buộc** sửa cả máy kiểm, nếu không `E_SECTION`.

**Đề xuất**
1. Dời `plat-common::obj` vào `vifast-core` (một cây cap duy nhất); `plat-common` chỉ còn trait.
2. Xoá `[limits.plat]` khỏi `spec/kernel.toml` **và** sửa `check_kernel.py` `required` list trong cùng commit.
3. Giữ `D2-83` (một chiều `plat → core`).

**Rủi ro + cách kiểm**
- Rủi ro: `riscv64` đang dùng `obj` cho `Untyped.Retype`/`Frame.Map`; dời sai ⇒ boot đỏ.
- Kiểm: T0 `cargo test -p vifast-core -p vifast-plat-common`; T1 `gate --dev`; T2 `gate --fast` (bắt buộc —
  chạm allocator/revoke); ca âm `mut-obj-*` hiện có phải còn ĐỎ.

**Câu hỏi cho Agent 0:** dời `obj` trong **một** commit hay hai bước (thêm bản sao → chuyển caller → xoá)?

---

## 2. Nhóm 2 — `IrqControl`/`IrqHandler` + `Notification`

**Hiện trạng đo được**
- `Tcb.BindNotification` (13) / `UnbindNotification` (14) **có số** trong `spec/abi.toml` nhưng chỉ có
  model API (`v2/kernel/vifast-core/src/core_sync.rs:176,208`); **không** nhánh dispatch trong `invoke.rs`.
- `IrqHandler.SetNotification` chưa có đường ABI.

**Đề xuất**
1. Nối dispatch 13/14 trong `invoke.rs` (gọi model API đã có).
2. `IrqHandler.SetNotification` dùng cùng đường `Notification` (không thêm object mới).

**Rủi ro + cách kiểm**
- Rủi ro: đổi `invoke` ⇒ ảnh hưởng mọi syscall; phải giữ `MessageInfo` layout.
- Kiểm: T0 `cargo test -p vifast-core`; ca host `invoke` 13/14; T1/T2; ca âm `mut-notify-*`.

**Câu hỏi cho Agent 0:** `IrqHandler.SetNotification` là method **mới** (số mới) hay tái dùng `Tcb.BindNotification`?

---

## 3. Nhóm 3 — `Frame.MapIo`/`UnmapIo` + `IOSpace`

**Hiện trạng đo được**
- Chỉ enum `ObjType::IOSpace` (`v2/kernel/vifast-core/src/error.rs:63`) + ghi chú
  `v2/lib/vifast-virtio/src/features.rs:22` ("**không** đàm phán `VIRTIO_F_ACCESS_PLATFORM` — không có IOMMU:
  DMA dùng PA thật"). **0 dòng** method.

**Đề xuất**
- **Phương án A (khuyến nghị):** hoãn — ghi `unsupported` **có lý do** trong spec (không IOMMU), giữ
  `IOSpace` là `reserved`; mở task riêng khi có IOMMU.
- **Phương án B:** thêm `Frame.MapIo`/`UnmapIo` + `IOSpace` (map PA thật, không IOMMU) — đổi wire.

**Rủi ro + cách kiểm**
- Rủi ro B: driver DMA tin PA thật ⇒ lỗ hổng nếu sau này có IOMMU; phải ghi rõ "không IOMMU".
- Kiểm: T0 host; T2 (chạm page table/quyền).

**Câu hỏi cho Agent 0:** chọn A (hoãn) hay B (map PA thật)?

---

## 4. Nhóm 4 — `Tcb.Exec` + method `VSpace`/`PageTable`

**Hiện trạng đo được**
- `Tcb.Exec` có số trong spec; `VSpace`/`PageTable` là đích `Untyped.Retype` nhưng method `unsupported`.
- `KT-404` đã có `v2/user/` workspace thật (ELF U-mode) ⇒ `Tcb.Exec` có thể cài.

**Đề xuất**
1. Cài `Tcb.Exec` (nạp ELF U-mode từ `v2/user/`).
2. Cài `VSpace.Map/Unmap` (method) — dùng `vifast-pte::sv39` (KT-409b) + `VSpace::free_frame` (KT-415).

**Rủi ro + cách kiểm**
- Rủi ro: `Exec` chạm trap/U-mode; `VSpace` chạm page table.
- Kiểm: T0 host; T1; T2 (bắt buộc — page table/quyền); ca âm `mut-exec-*`/`mut-vspace-*`.

**Câu hỏi cho Agent 0:** `Tcb.Exec` nhận `Frame` (ELF image) hay `Untyped` + offset?

---

## 5. Nhóm 5 — GDT `planned` cho `syscall`/`sysret` `x86_64` giữ `7C-r13`

**Hiện trạng đo được**
- GDT KT-261 đặt user data **sau** user code; `sysret` đòi user data ở `CS−8` ⇒ **ngược**.
- `7C-r13` là ràng buộc đã ghim (không đổi).

**Đề xuất**
- Ghi **hình học GDT đích** vào `spec/kernel.toml` (`[arch.x86_64.gdt]`, `planned = true`), **chưa** đổi
  trap trong đợt này; task riêng đổi thứ tự + cập nhật TAP/đột biến đang ghim selector.

**Rủi ro + cách kiểm**
- Rủi ro: đổi thứ tự GDT làm đỏ TAP/đột biến đang ghim selector.
- Kiểm: T0 host; T2 (chạm trap).

**Câu hỏi cho Agent 0:** chấp nhận `planned` (chưa đổi) trong đợt này?

---

## 6. Nhóm 6 — `CNode.Mutate`/`Rotate` + MCS refill đa đoạn

**Hiện trạng đo được**
- `CNode` có `Insert`/`Move`/`Revoke`; `Mutate`/`Rotate` **chưa có**.
- Refill MCS **một đoạn** (`D2-16`).

**Đề xuất**
1. Thêm `CNode.Mutate`/`Rotate` (đổi quyền/hoán vị slot).
2. Refill MCS **đa đoạn** (giữ `D2-16`).

**Rủi ro + cách kiểm**
- Rủi ro: `Mutate` đổi quyền ⇒ phải giữ bất biến MDB; `Rotate` chạm slot.
- Kiểm: T0 host; ca âm `mut-cnode-*`; T2.

**Câu hỏi cho Agent 0:** `Mutate` nhận mask quyền hay `CapRights` đầy đủ?

---

## 7. Nhóm 7 — `[profiles]` `mission`/`general` + BootArena + vùng FPU/SIMD

**Hiện trạng đo được**
- `StaticStore`/`UntypedStore` đã có (`v2/kernel/vifast-core/src/store.rs:271,408`); **chưa có tên profile
  trong spec**.
- `BootArena` đo được **617.208 B** (KT-410).
- `v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs` chỉ **62 dòng** đọc/ghi `CR0`/`CR4`; `TcbCore`
  (`v2/kernel/vifast-core/src/tcbdata.rs:24`) **không** có trường FPU.

**Đề xuất**
1. Thêm `[profiles]` `mission`/`general` vào `spec/kernel.toml` (trỏ `StaticStore`/`UntypedStore`).
2. Ghi `BootArena` (617.208 B) là hằng **đo được** (không viết tay — sinh từ `v2 nightly`).
3. Vùng lưu FPU/SIMD **ngoài** `TcbCore` (KT-418 đang làm) — spec chỉ ghi kích thước.

**Rủi ro + cách kiểm**
- Rủi ro: `[profiles]` chạm `check_kernel.py`; FPU area chạm `TcbCore` (pin `size_of`).
- Kiểm: T0 host; T2 (chạm layout).

**Câu hỏi cho Agent 0:** `[profiles]` là mục **mới** trong `kernel.toml` hay tệp riêng?

---

## 8. Nhóm 8 — Hợp đồng `BootCapLayout` thay 16 hằng `EP_*`

**Hiện trạng đo được**
- `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs` khai **17** `pub const EP_*` (`EP_FAULT` 1, `EP_PROC` 2,
  `EP_UART` 3, `EP_REPLY` 4, `EP_PS` 5, `EP_CTL` 6, `EP_FS` 7, `EP_EVENTS` 8, `EP_NET` 9,
  `EP_NET_REPLY` 15, `EP_NET_KERN` 16, `EP_NET_KERN_REPLY` 17, `EP_EXEC` 18, `EP_MEM` 21, `EP_LOOP` 22,
  `EP_NET_CLIENT` 26, `EP_NET_CLIENT_REPLY` 27) + 8 `SLOT_*`/`LOOP_*` cùng tệp.
- Kế hoạch ghi "16 hằng" — **đo được 17** ⇒ hợp đồng phải phủ **17** + nhóm `SLOT_*`.

**Đề xuất**
- `BootCapLayout` (struct trong `spec/abi.toml` `[boot_caps]`) phủ **17** `EP_*` + `SLOT_*`; bỏ hardcode
  trong `proc_abi.rs`; sinh từ spec (KT-05).

**Rủi ro + cách kiểm**
- Rủi ro: đổi layout boot ⇒ mọi ảnh U-mode phải biên dịch lại; `KT-420` phụ thuộc.
- Kiểm: T0 host; T2 (chạm boot/layout).

**Câu hỏi cho Agent 0:** `BootCapLayout` là **một** struct phẳng hay nhóm `[boot_caps.ep]`/`[boot_caps.slot]`?

---

## 9. Việc còn lại / rủi ro

- RFC **chưa được duyệt**; `KT-417` vẫn `BLOCKED`.
- 3 nhóm (2, 3, 4) **đổi `abi_version` 1→2**; 2 nhóm (1, 7) chỉ đổi `spec/kernel.toml`; 1 nhóm (5) chỉ
  ghi `planned`; 1 nhóm (6) đổi wire; 1 nhóm (8) đổi layout boot — xem bảng §0.
- **Không có thay đổi mã nào** trong KT-416.
- Nợ công cụ: `check_kernel.py:277` đòi `[limits.plat]` — nhóm 1 phải sửa **cả** máy kiểm.
