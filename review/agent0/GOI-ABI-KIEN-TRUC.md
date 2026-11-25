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

> **→ Đã có khuyến nghị đo được ở §B.0 (PHẦN B, KT-416b): KHÔNG nâng bậc** — giữ
> `abi_version = 1` / `min_compat = 1` cho cả 8 nhóm. Lý do: thêm method
> **không** thuộc 5 thứ đóng băng (`v2/agent/DECISIONS.md:70`), tiền lệ `D2-242` duyệt cả gói v1.1
> với `abi_version = 1` (`v2/agent/DECISIONS.md:462`), và nâng bậc bắt buộc sửa máy kiểm
> (`v2/test/abi/check_abi.py:243-244` ghim cứng `1`/`1`) + sinh lại golden `docs/abi.md`.
> **PHẦN B cũng đính chính 8 chỗ của PHẦN A đã lệch cây tại `9eb0e589`** (§B.1) — đọc §B.1 trước khi
> đọc tiếp §1..§8.

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

---

# PHẦN B — BỔ SUNG **KT-416b**: đối chiếu `EP_*`, khuyến nghị A/B, kịch bản kiểm

**Ngày soạn:** 2026-11-26 · **Nhánh:** `task/kt416b-abi-pack` · **Base:** `main` @ `9eb0e589`+ ·
**Người soạn:** sub-agent `kt416b` — task **TÀI LIỆU**: **0 dòng mã sản phẩm**, **0 byte `spec/`**.

**Vì sao có PHẦN B.** PHẦN A soạn tại `fc998aa8`; cây nay đã đi qua `KT-409`…`KT-418b`, nên 8 chỗ
trong PHẦN A **không còn khớp mã**. PHẦN B (i) đính chính 8 chỗ đó kèm `đường-dẫn:dòng` (§B.1),
(ii) làm đúng ba việc được giao: bảng 17 `EP_*` ↔ `BootCapLayout` (§B.2), 8 khuyến nghị A/B (§B.3),
kịch bản kiểm T0/T1/T2 + ca phải ĐỎ (§B.4), và (iii) trả lời câu hỏi chặn `abi_version` (§B.0).

**Quy ước số.** Mọi con số dưới đây là **đếm tĩnh** trên tệp nguồn tại base (lệnh tái lập ghi ngay
cạnh), **không** phải số đo hiệu năng/thời gian — QD §2.2. Số hiệu ảnh/dung lượng chỉ được **trích
log có sẵn** (duy nhất một chỗ: §B.1 mục 7, trích `v2/agent/JOURNAL/kt410-general-store.md`).
Chỗ nào PHẦN B nói khác PHẦN A thì **PHẦN B thắng**, vì nó đo tại `9eb0e589`.

---

## B.0 Câu hỏi chặn: `abi_version` có cần nâng lên 2?

**Khuyến nghị A — KHÔNG nâng:** giữ `abi_version = 1`, `min_compat = 1` cho **cả 8** nhóm.

Lý do đo được:

1. **5 thứ đóng băng không gồm "bảng method"** — `spec/abi.toml:22-27` liệt đúng: (1) số syscall
   0..15 + quy ước thanh ghi, (2) layout `MessageInfo`, (3) bảng 19 mã lỗi, (4) **kích thước/căn lề
   object** theo arch, (5) ID không tái dùng. Nhóm 2/3/4/6 **thêm method** ⇒ không chạm mục nào.
2. **Luật đã có**: `v2/agent/DECISIONS.md:70` (D2-33) — "Chữ ký method **không** thuộc 5 thứ đóng
   băng của D2-15 … đổi chữ ký ở K1 vẫn qua Agent 0 nhưng **không cần tăng `abi_version` major**".
3. **Tiền lệ**: `v2/agent/DECISIONS.md:462` (D2-242) duyệt **cả gói ABI v1.1** (9 nhóm) mà "**Giữ
   nguyên `abi_version = 1`, `min_compat = 1`** (0 byte wire đổi)".
4. **Nâng bậc có giá**: `v2/test/abi/check_abi.py:243-244` **ghim cứng** `abi_version == 1` và
   `min_compat == 1`; `docs/abi.md` là golden sinh từ spec (`spec/abi.toml:18`). Nâng bậc ⇒ phải sửa
   máy kiểm + sinh lại golden + cập nhật `[debug_info]` trong **cùng** cửa sổ duyệt — thêm một bề
   mặt duyệt cho 0 lợi ích wire.
5. **Không nhóm nào đổi byte của một lời gọi đang có**: `MapIo`/`UnmapIo` đã có số 11.3/11.4 và đang
   `unsupported` (`spec/abi.toml:241`); `Mutate`/`Rotate` đã có số 2.3/2.6 (`spec/abi.toml:229`);
   `Tcb.Exec` là số **mới** sau 15 (`spec/abi.toml:230`); `BootCapLayout` là hợp đồng **lúc boot**,
   không nằm trên đường syscall.

**Phương án thay thế B — nâng `abi_version = 2`.** Chỉ hợp lý nếu Agent 0 phán quyết rằng (a) thêm
method **làm đổi** byte wire (ngược D2-33), **hoặc** (b) muốn dùng chính `abi_version` để đánh dấu
hợp đồng boot mới. Khi đó bắt buộc trong cùng cửa sổ: sửa `v2/test/abi/check_abi.py:243-244`, sinh
lại `docs/abi.md`, ghi một D2 mới. **Nếu chỉ cần đánh dấu hợp đồng boot**, dùng khoá **riêng**
`boot_abi = 1` trong `[boot_caps]` (nhóm 8) — 0 đụng `abi_version`.

---

## B.1 Đính chính PHẦN A theo cây tại `9eb0e589` (8 chỗ)

| # | PHẦN A nói | Đếm/đọc tại base | Hệ quả cho Agent 0 |
|---|---|---|---|
| 1 | §1: cây cap thứ hai gồm `vifast-core/src/{capalloc.rs,mdb.rs,core_capxfer.rs}`, `obj` "1.9k LOC" | `capalloc.rs` **không tồn tại**; có `captable.rs` (60 dòng), `mdb.rs` (94), `core_capxfer.rs` (263). `obj/` = **4 tệp, 2.841 dòng**: `mod.rs` 1259, `check.rs` 445, `vspace.rs` 450, `arena.rs` 270 (`wc -l`) | Khối phải dời **lớn hơn** PHẦN A ghi ⇒ càng phải hai bước (§B.3 nhóm 1) |
| 2 | §2: 13/14 "chỉ model API, không nhánh dispatch" | **Đúng**: dispatch chỉ có 4 cặp `Tcb` (`v2/kernel/vifast-core/src/invoke.rs:386-394`), whitelist 12 cặp (`invoke.rs:1040-1054`). **Thiếu**: object `IrqControl` 12 / `IrqHandler` 13 **đã có** trong spec (`spec/abi.toml:244-245`) | Nhóm 2 **không** cần ID object mới; chỉ cần `[method_signature]` + dispatch |
| 3 | §3: `MapIo`/`IOSpace` "**0 dòng** method" | Đúng 0 dòng mã, **nhưng spec đã khai đủ**: `Frame.MapIo`/`UnmapIo` = `unsupported = true, install_at = "Chu-ky-3"` (`spec/abi.toml:241`), `IOSpace` id 14 `install_at = "D3"` (`spec/abi.toml:246`) | "Hoãn" (phương án A) **không cần sửa spec** — hiện trạng đã đúng |
| 4 | §4: "`Tcb.Exec` **có số** trong spec" + "cài `VSpace.Map/Unmap`" | **Sai cả hai**: 16 method `Tcb` là 0..15, **không có `Exec`** (`spec/abi.toml:230`); 16 `[method_signature]` không có `Tcb.Exec` (`grep -c '^\[method_signature\."'` = 16). `VSpace.Map` **bị cấm**: D2-32 "VSpace khách không phải method … method id 1 của VSpace để trống" (`v2/agent/DECISIONS.md:63`) và D2-98 "`VSpace.Map` **không tồn tại** — dùng `Frame.Map`" (`v2/agent/DECISIONS.md:167`) | Nhóm 4 phải **bỏ** phần `VSpace`; `Tcb.Exec` là **method mới số 16** |
| 5 | §5: "Ghi hình học GDT **đích** vào `kernel.toml`" | **Đã có**: `[arch.x86_64.gdt.planned]` (`spec/kernel.toml:92-106`) với `order`, selector 8/16/24/32/40, `star_63_48 = 16`, `trap = "int 0x80"`, `min_compat = 1`; máy kiểm canh khối này (`v2/test/kernel/check_kernel.py:400-424`) | Câu hỏi thật của nhóm 5 là **lật `sysret_ready`** (`spec/kernel.toml:87`), không phải "ghi" |
| 6 | §6: "`CNode` có `Insert`/`Move`/`Revoke`; `Mutate`/`Rotate` **chưa có**" | Nửa đúng: **đã có số** — `Mutate` 3, `Rotate` 6 (`spec/abi.toml:229`), sinh tự động thành `CNODE_METHODS` (`invoke.rs:1144-1158`); thiếu `[method_signature]` và `is_whitelisted` đang **cố tình** `false` (`invoke.rs:1128-1129`) | Nhóm 6 = chữ ký + cài + **lật 2 dòng test**, không phải cấp số |
| 7 | §7: "`asm/fpu.rs` chỉ **62 dòng**; `TcbCore` không có trường FPU"; BootArena "**617.208 B**" | `fpu.rs` nay **368 dòng**; vùng FPU **đã có** ở `vifast-plat-common`: `FPU_AREA_BYTES = 528`, `FPU_SLOTS = 65` (`v2/kernel/vifast-plat-common/src/fpu.rs:52,56`), `FpuArea` `#[repr(C, align(16))]` (`fpu.rs:62`) — **KT-418 DONE** (`v2/agent/QUEUE.md:472`). Số `617208` chỉ có trong **log** `v2/agent/JOURNAL/kt410-general-store.md:119,207`; hằng có tên là `GENERAL_MACHINE_BYTES` (`untyped_store.rs:44`) | Nhóm 7 còn **phần spec**; "617.208" **không** được chép vào spec (QD §2.2) |
| 8 | §8: "8 `SLOT_*`/`LOOP_*`" | Đúng **8** hằng có tiền tố đó (5 `SLOT_*` + 3 `LOOP_*`), **nhưng** còn **2 mảng** `TCB_SLOTS` (`proc_abi.rs:49`) và `SC_SLOTS` (`:51`) ⇒ **10** khai báo bố cục | Bảng §B.2 phải phủ **10**, không phải 8 |

**Đo thêm (dùng cho §B.3 nhóm 8).** `proc_abi.rs:13-15` nói ràng buộc "khớp bảng" được kiểm bằng
`v2/agent/JOURNAL/kt77b-harness-scan.sh` §slot — **không có thật**: `grep -n slot` trên tệp đó = **0**
dòng, và `grep -rln proc_abi v2/test v2/bin v2/tools` = **0** tệp. Phía U-mode giữ **17** `const EP_*`
trong 4 tệp (`v2/user/programs/{shell,procsrv,shell_cmds,echo}.rs`; `shell.rs:69-94`), đồng bộ với
kernel **bằng comment** ("khớp `proc_abi::EP_FS`"). Tức hiện nay **không có máy kiểm nào** canh
kernel ↔ U-mode.

---

## B.2 Bảng đối chiếu 17 `EP_*` (+ 8 `SLOT_*`/`LOOP_*` + 2 mảng) ↔ trường `BootCapLayout` đề xuất

**Ba quyết định kiểu (dùng cho cả ba bảng):**

- **Slot cspace dùng `u16`**: trần cap của model là `max_caps = 512` (`spec/kernel.toml:31`) ⇒ slot
  hợp lệ ≤ 511; `u16` đủ và **không** phụ thuộc độ rộng word của arch (lý do `proc_abi.rs` hiện dùng
  `usize`: nó là hằng Rust nội bộ, **không** dùng được làm ABI).
- **Ô chương trình dùng `u8`**: trần `user::MAX_PROCS = 6` (`v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs:160`).
- **Sentinel "vắng"**: `0xFFFF` (cspace) / `0xFF` (proc). **Không** dùng `0` — vì `0` là giá trị **hợp
  lệ** ở không gian ô chương trình (`SLOT_PROCSRV = 0`, `proc_abi.rs:24`).

### B.2.1 Không gian **cspace** — 17 `EP_*`

| # | Hằng (`v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs`) | Giá trị | Trường `BootCapLayout` | Kiểu | Ghi chú |
|---|---|---:|---|---|---|
| 1 | `EP_FAULT` :31 | 1 | `fault` | `u16` | core ghi `FaultKind` (D2-25/QD §1 Q-A) |
| 2 | `EP_PROC` :33 | 2 | `proc` | `u16` | yêu cầu `wait`/`kill` → procsrv |
| 3 | `EP_UART` :35 | 3 | `uart` | `u16` | **deprecated**: KT-73 đã bỏ đường dây, "giữ slot để bố cục không đổi" (:34) ⇒ profile mới đặt `NONE` |
| 4 | `EP_REPLY` :37 | 4 | `reply` | `u16` | reply của procsrv |
| 5 | `EP_PS` :39 | 5 | `ps` | `u16` | `PS_REQ` → kernel |
| 6 | `EP_CTL` :41 | 6 | `sched_control` | `u16` | **KHÔNG phải endpoint** — là cap `SchedControl` (:40) ⇒ tên trường **không** mang hậu tố `_ep` |
| 7 | `EP_FS` :81 | 7 | `fs` | `u16` | `FS_REQ` (KT-77c) → sau này là `.vidl` `fs` (KT-420) |
| 8 | `EP_EVENTS` :43 | 8 | `events` | `u16` | kênh `EV_*` |
| 9 | `EP_NET` :141 | 9 | `net` | `u16` | shell → netsrv |
| 10 | `EP_NET_REPLY` :143 | 15 | `net_reply` | `u16` | netsrv → shell |
| 11 | `EP_NET_KERN` :145 | 16 | `net_kern` | `u16` | netsrv → kernel (`Call`) |
| 12 | `EP_NET_KERN_REPLY` :147 | 17 | `net_kern_reply` | `u16` | cap `Reply` của kernel |
| 13 | `EP_EXEC` :149 | 18 | `exec` | `u16` | `run <path>` (kernel-mediated) |
| 14 | `EP_MEM` :172 | 21 | `mem` | `u16` | `MemSvc` (`meminfo`) |
| 15 | `EP_LOOP` :175 | 22 | `loop_ep` | `u16` | `loop <n>` |
| 16 | `EP_NET_CLIENT` :179 | 26 | `net_client` | `u16` | caller thứ hai (6A) |
| 17 | `EP_NET_CLIENT_REPLY` :181 | 27 | `net_client_reply` | `u16` | trả lời caller thứ hai |

**Dải giá trị** 1..27 có **7 lỗ trống** (0, 10–14, 19–20, 23–25). Bốn trong số đó **có chủ** bởi bố
cục khác (xem §B.2.2), nên `BootCapLayout` **không** được suy ra bằng "slot = chỉ số mảng".

### B.2.2 Cùng không gian **cspace** — 8 `SLOT_*`/`LOOP_*` + 2 mảng

| # | Hằng | Giá trị | Trường | Kiểu | Ghi chú |
|---|---|---:|---|---|---|
| 18 | `SLOT_CSPACE` :46 | 20 | `self_cspace` | `u16` | cap `CNode` của **chính** cspace — **khác** ô 0 = `CS_SELF` (`spec/abi.toml:35-36`); đừng gộp |
| 19 | `TCB_SLOTS` :49 | `[10,11,12,13,14]` | `tcb` | `[u16; 5]` | theo thứ tự ảnh (procsrv, shell, …) |
| 20 | `SC_SLOTS` :51 | `[30,31,32,33,34]` | `sc` | `[u16; 5]` | cặp 1:1 với `tcb` — 2 mảng phải **cùng độ dài** (ca kiểm §B.4) |
| 21 | `LOOP_ARENA` :177 | 23 | `loop_arena` | `u16` | `Untyped` 4 KiB riêng cho `loop` |
| 22 | `LOOP_TCB_SLOT` :183 | 24 | `loop_tcb` | `u16` | ô **nháp**, xoá sau mỗi vòng |
| 23 | `LOOP_SC_SLOT` :185 | 25 | `loop_sc` | `u16` | ô **nháp**, xoá sau mỗi vòng |
| 24 | `SLOT_PROCSRV` :24 | 0 | `procsrv` | `u8` | → §B.2.3 |
| 25 | `SLOT_SHELL` :26 | 1 | `shell` | `u8` | → §B.2.3 |
| — | `SLOT_CHILD` :28 | 2 | `child` | `u8` | → §B.2.3 |
| — | `SLOT_NETSRV` :162 | 2 | `netsrv` | `u8` | → §B.2.3 |

### B.2.3 Không gian **ô chương trình** (`user::MAX_PROCS`) — 4 `SLOT_*`

| # | Hằng | Giá trị | Trường | Kiểu | Ghi chú |
|---|---|---:|---|---|---|
| 26 | `SLOT_PROCSRV` :24 | 0 | `procsrv` | `u8` | ảnh nào cũng có |
| 27 | `SLOT_SHELL` :26 | 1 | `shell` | `u8` | ảnh nào cũng có |
| 28 | `SLOT_CHILD` :28 | 2 | `child` | `u8` | **chỉ** ảnh test `ktest-proc` |
| 29 | `SLOT_NETSRV` :162 | 2 | `netsrv` | `u8` | **chỉ** ảnh sản phẩm — **cùng số 2** với `child`, khác ảnh (:160) |

**Hệ quả bắt buộc:** `child` và `netsrv` tranh nhau ô 2 ⇒ hợp đồng phải là **hai giá trị**
(`BOOT_CAPS_TEST` / `BOOT_CAPS_PRODUCT`), **không** phải một bảng gộp. Đây là lý do kỹ thuật để tách
bảng theo **không gian chỉ số**, không tách theo "to cho dễ đọc".

### B.2.4 Hình dạng đề xuất (khuyến nghị cho câu hỏi §8)

```toml
# ĐỀ XUẤT — CHƯA ÁP DỤNG. Mọi khoá là [TK]; giá trị sẽ do KT-417 sinh từ mã đang chạy.
[boot_caps]
boot_abi = 1                        # đánh dấu hợp đồng boot, ĐỘC LẬP với abi_version (§B.0)
none_cspace = 0xFFFF                # sentinel "vắng" cho slot cspace
none_proc = 0xFF                    # sentinel "vắng" cho ô chương trình
source = "review/agent0/GOI-ABI-KIEN-TRUC.md#text:Phần B.2"

[boot_caps.cspace]                  # 23 trường: 17 EP_* + self_cspace + tcb + sc + 3 loop_*
fault = { value = 1, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#EP_FAULT" }
# … 16 dòng còn lại theo §B.2.1 …
self_cspace = { value = 20, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#SLOT_CSPACE" }

[boot_caps.proc]                    # 4 trường — KHÔNG cùng không gian chỉ số với [boot_caps.cspace]
procsrv = { value = 0, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#SLOT_PROCSRV" }
shell   = { value = 1, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#SLOT_SHELL" }
```

Sinh ra **một struct phẳng** cho mỗi profile:

```rust
// ĐỀ XUẤT — CHƯA ÁP DỤNG (KT-417/KT-420 sinh từ spec, không chép tay).
#[repr(C)]
pub struct BootCapLayout {
    // cspace (u16, NONE_CSPACE = 0xFFFF)
    pub fault: u16, pub proc: u16, pub uart: u16, pub reply: u16, pub ps: u16,
    pub sched_control: u16, pub fs: u16, pub events: u16, pub net: u16,
    pub net_reply: u16, pub net_kern: u16, pub net_kern_reply: u16, pub exec: u16,
    pub mem: u16, pub loop_ep: u16, pub net_client: u16, pub net_client_reply: u16,
    pub self_cspace: u16, pub tcb: [u16; 5], pub sc: [u16; 5],
    pub loop_arena: u16, pub loop_tcb: u16, pub loop_sc: u16,
    // ô chương trình (u8, NONE_PROC = 0xFF)
    pub procsrv: u8, pub shell: u8, pub child_or_netsrv: u8,
}
pub const BOOT_CAPS_TEST: BootCapLayout = /* child = 2, netsrv = NONE_PROC */;
pub const BOOT_CAPS_PRODUCT: BootCapLayout = /* netsrv = 2, child = NONE_PROC */;
```

**Thay thế (phương án B cho §8):** một bảng TOML phẳng 25–27 khoá. Gọn cho `vifast-gen` nhưng trộn
**hai không gian chỉ số**: `SLOT_PROCSRV = 0` sẽ bị đọc thành cspace slot 0 = `CS_SELF`
(`spec/abi.toml:35-36`) — lỗi **im lặng**, không máy kiểm nào bắt. Không khuyến nghị.

---

## B.3 Tám khuyến nghị A/B cho Agent 0 (mỗi nhóm 1 dòng + lý do đo + thay thế)

### Nhóm 1 — Hợp nhất 2 Object/Cap Layer

**Khuyến nghị A — dời hai bước:** (b1) dời `obj` vào `vifast-core` (đường dẫn mới), chỉ **đổi chuỗi
`source`** trong `spec/kernel.toml`; (b2) xoá `[limits.plat]` + sửa `check_kernel.py:277`.

Lý do đo được:

- `v2/test/kernel/check_kernel.py:277` liệt `limits.plat` vào danh sách `required`, và 5 lá của mục
  này trỏ đúng `v2/kernel/vifast-plat-common/src/obj/mod.rs#MAX_*` (`spec/kernel.toml:43-47`). Xoá
  tệp trước khi sửa máy kiểm ⇒ `E_SOURCE_FILE`/`E_SECTION` đỏ vì lý do **không liên quan** boot.
- Bước 1 giữ `check_kernel` **xanh chỉ bằng đổi chuỗi `source`**: máy kiểm so theo **tên hằng trong
  tệp nguồn** (`check_kernel.py:318-343`), không theo số dòng.
- Khối phải dời là **2.841 dòng / 4 tệp** (§B.1 mục 1) nằm trên **đường boot**; gộp một commit thì
  ca đỏ mơ hồ (D2-05: lỗi logic phải có `.scn` đỏ trước; D2-07: cấm retry để lấy xanh).

**Bẫy phải chặn:** `MAX_OBJECTS`/`MAX_CAPS`/`MAX_UNTYPEDS` **trùng tên** giữa
`vifast-core/src/machine.rs:35-39` (256/512/16) và `obj/mod.rs:76-80` (96/160/4). Nếu trộn hai bộ
vào **một** tệp, `check_kernel.py` đọc `const <TÊN>` **đầu tiên** ⇒ so sai giá trị. Bước 1 phải đặt
tệp mới ở đường dẫn riêng (vd `vifast-core/src/vm/mod.rs`) **hoặc** đổi tên `VM_MAX_*` trước khi spec
trỏ vào.

**Thay thế B (một commit):** chấp nhận được **chỉ khi** lead chấp nhận đúng 1 lần đỏ `E_SECTION` **có
chủ đích** trong commit đó và ghi vào journal; nếu không, chọn A.

### Nhóm 2 — `IrqControl`/`IrqHandler` + `Notification`

**Khuyến nghị A — dùng số đã cấp:** `IrqHandler.SetNotification = 0` (`spec/abi.toml:245`), thêm
`[method_signature]`, nối dispatch 13/14 lên model API đã có; **không** tái dùng `Tcb.BindNotification`.

Lý do đo được:

- `IrqHandler` id 13 và 4 method 0..3 **đã có** trong spec (`spec/abi.toml:245`) ⇒ **0** ID mới,
  không chạm `[object_reserved]` (`spec/abi.toml:250-253`).
- Invocation chạy trên **cap được gọi** (D2-99(a), `v2/agent/DECISIONS.md:169`). Tái dùng
  `Tcb.BindNotification` buộc driver U-mode giữ **cap TCB** — mà cap TCB mở `Suspend`/`WriteRegs`
  (`spec/abi.toml:230` method 3/10) ⇒ nới quyền trái D2-99(e).
- Model API đã có **và đã kiểm 1:1**: `core_sync.rs:176-206` trả `Busy` nếu Notification đã gắn TCB
  khác; `core_sync.rs:208` (`UnbindNotification`) idempotent ⇒ nhánh dispatch chỉ là lớp mỏng.

**Thay thế B:** cấp `SetNotification` một số **mới** (vd 4). Vô ích: số 0 đã cấp, và `vifast-gen`
đòi method **liền mạch 0..N** (`v2/tools/vifast-gen/gen.py:109-111`) nên để trống 0 sẽ **die**.

### Nhóm 3 — `Frame.MapIo`/`UnmapIo` + `IOSpace`

**Khuyến nghị A — hoãn:** 0 sửa `spec/` trong đợt này; giao trọn cho `KT-419`.

Lý do đo được:

- Hiện trạng spec **đã đúng** điều PHẦN A định ghi: `MapIo`/`UnmapIo` = `unsupported = true,
  install_at = "Chu-ky-3"` (`spec/abi.toml:241`), `IOSpace` = id 14 `install_at = "D3"`
  (`spec/abi.toml:246`) ⇒ "hoãn" **không cần** byte nào.
- `vifast-virtio` **không** đàm phán `VIRTIO_F_ACCESS_PLATFORM` vì **không có IOMMU**:
  `v2/lib/vifast-virtio/src/features.rs:21-22`.
- `KT-419` (`v2/agent/QUEUE.md:474`) đã giao đúng việc, kèm **2 ca âm bắt buộc**; ca "thiết bị DMA
  ghi ngoài bảng trang `IOSpace` ⇒ bị chặn" **không thể** đạt nếu map PA thật.

**Thay thế B (map PA thật ngay):** chỉ nếu Agent 0 muốn driver U-mode DMA **trước** IOMMU. Khi đó
spec phải ghi cờ `no_iommu = true` + nợ bảo mật có tên, và ca âm của `KT-419` phải hoãn. Không khuyến
nghị.

### Nhóm 4 — `Tcb.Exec` + method `VSpace`/`PageTable`

**Khuyến nghị A — chỉ `Tcb.Exec` (method mới số 16) + chữ ký `PageTable.Map/Unmap`; BỎ phần
`VSpace.Map/Unmap`.**

Lý do đo được:

- `Tcb.Exec` **không tồn tại** trong spec: 16 method `Tcb` là 0..15 (`spec/abi.toml:230`), và
  `grep -c '^\[method_signature\."'` = 16, không có `Tcb.Exec` ⇒ đây là method **mới**; số 16 **liền
  mạch** nên `gen.py:109-111` chấp nhận.
- `VSpace.Map` **bị cấm bởi D2-32**: "VSpace khách không phải method … Method id 1 của VSpace để
  trống" (`v2/agent/DECISIONS.md:63`); D2-98 nhắc lại "`VSpace.Map` **không tồn tại** — dùng
  `Frame.Map`" (`v2/agent/DECISIONS.md:167`). Đường map **đã có và đã cài**: `Frame.Map` 11.0 với cap
  VSpace ở MR0 (`spec/abi.toml:289-293`, D2-99(d)).
- `PageTable` id 10, method 0/1 đã khai `unsupported` (`spec/abi.toml:243`) ⇒ chỉ cần chữ ký + cài.

**Tham số `Tcb.Exec` — khuyến nghị A: `Frame` (MR0 = cap `Frame` chứa ảnh ELF), không `Untyped+offset`,
không `path`.**

- `KT-422` sẽ **gỡ `proc_fs.rs` + `vifast-fs`/`vifast-vfs` khỏi kernel** (`v2/agent/QUEUE.md:477`) ⇒
  `Exec(path)` phải viết lại trong 2 mốc.
- Cap đi qua IPC **đã có sẵn**: `[message] max_caps = 4` (`spec/abi.toml:45`) + receive window (KT-63)
  ⇒ 0 cơ chế mới.
- `Untyped + offset` buộc kernel đọc/ghi vùng nhớ **không** có cap `Frame` mô tả ⇒ không kiểm được
  quyền, trái tinh thần D2-99(f) ("sai ⇒ lỗi, **không** im lặng", `v2/agent/DECISIONS.md:169`).

**Thay thế B:** `Exec(path)` đúng như chữ trong `KT-420` (`v2/agent/QUEUE.md:475`) — chấp nhận được
nếu ghi `install_at` + điều kiện gỡ khi `KT-422` xong.

### Nhóm 5 — GDT `planned` cho `syscall`/`sysret` `x86_64` (giữ `7C-r13`)

**Khuyến nghị A — 0 việc spec trong đợt này:** chỉ lật `sysret_ready` trong `KT-417` và **giữ cổng
`int 0x80`**.

Lý do đo được:

- Khối đích **đã có**: `[arch.x86_64.gdt.planned]` (`spec/kernel.toml:92-106`) — `order`, selector
  8/16/24/32/40, `star_63_48 = 16`, `trap = "int 0x80"`, `min_compat = 1`.
- Máy kiểm **đã canh**: `check_kernel.py:400-424` (`E_GDT_PLANNED_ORDER/ENTRIES/STAR/TRAP/COMPAT`);
  hình học đang chạy cũng được kiểm tự nhất quán (`check_kernel.py:361-397`).
- `sysret_ready` đang `false` (`spec/kernel.toml:87`) và `check_kernel.py:395-396` **buộc** `false` ở
  v1.1 ⇒ lật `true` là **sửa máy kiểm**, không phải sửa dữ liệu.
- Giữ `int 0x80` chính là điều kiện của `min_compat = 1` (`spec/kernel.toml:91` ghi thẳng) ⇒ bỏ cổng
  cũ kéo theo nâng `min_compat` ⇒ quay lại §B.0.

**Thay thế B:** đổi `trap` sang `syscall` và **bỏ** `int 0x80` — chỉ khi Agent 0 chấp nhận nâng
`min_compat`, sửa `check_kernel.py:417`, và sửa `[calling.x86_64].trap` (`spec/abi.toml:138`).
Đắt hơn giá trị (một đường syscall) trong cùng chu kỳ.

### Nhóm 6 — `CNode.Mutate`/`Rotate` + MCS refill đa đoạn

**Khuyến nghị A — `Mutate` nhận `CapRights` đầy đủ (không phải mask):** cài `Rotate`; **giữ refill
MCS một đoạn** ở đợt này.

Lý do đo được:

- Chữ ký hiện có biểu diễn quyền bằng **tập tên đầy đủ** `rights?` (`CNode.Copy`/`CNode.Mint`,
  `spec/abi.toml:269-276`) chứ không phải mask ⇒ `Mutate` theo `rights` giữ **một** quy ước.
- Quyền trong kernel là bitmask `u8`: `pub rights: u8` (`v2/kernel/vifast-core/src/error.rs:242`),
  `RIGHTS_ALL = RIGHT_READ|RIGHT_WRITE|RIGHT_GRANT|RIGHT_GRANT_REPLY`
  (`v2/kernel/vifast-core/src/spec.rs:32`, các bit ở :24-30). Mask chỉ **tương đương** nếu thêm một
  kiểm `mask ⊆ RIGHTS_ALL` — một kiểm mới cho 0 lợi ích.
- `is_whitelisted` đang **cố tình** từ chối 3/6 và có test khẳng định (`invoke.rs:1128-1129`) ⇒ đợt
  cài phải **lật đúng 2 dòng**, **giữ** ca âm `!is_whitelisted(ObjType::CNode, 7)`.
- Refill đa đoạn **trái** `D2-16` (`v2/agent/DECISIONS.md:45`: "thuật toán refill **sau K6**") và
  `D2-49(e)` (`v2/agent/DECISIONS.md:85`: "cửa sổ trượt/sporadic refill hoãn sau K6").

**Thay thế B:** `Mutate` nhận **mask** — 0 kiểm mới nhưng tạo quy ước quyền **thứ hai**; nếu chọn,
phải ghi rõ `mask` trong `[method_signature]` + định nghĩa `RIGHTS_ALL` là trần cứng.

### Nhóm 7 — `[profiles]` `mission`/`general` + BootArena + vùng FPU/SIMD

**Khuyến nghị A — `[profiles]` là mục con của `spec/kernel.toml`:** không tệp riêng; BootArena ghi
**luật** bằng `#text:`; FPU pin **hằng đã có**.

Lý do đo được:

- Mọi giá trị **đã có hằng có tên**: `GENERAL_MACHINE_BYTES` (`v2/kernel/vifast-plat-common/src/untyped_store.rs:44`),
  `GENERAL_MAX_TCBS/CAPS/OBJECTS` (`untyped_store.rs:33`), `FPU_AREA_BYTES = 528`, `FPU_SLOTS = 65`
  (`v2/kernel/vifast-plat-common/src/fpu.rs:52,56`) ⇒ `[profiles]` chỉ là **ánh xạ tên**;
  `check_kernel.py` kiểm được **ngay, 0 sửa máy kiểm**.
- Tệp riêng ⇒ cần checker + bước cổng mới = **nợ công cụ mới**, đúng loại nợ `[limits.plat]` đang trả.
- "BootArena **617.208 B**" **không** phải hằng có tên trong mã; số đó chỉ có trong **log**
  `v2/agent/JOURNAL/kt410-general-store.md:119,207`. QD §2.2 cấm chép số đo tay ⇒ ghi **luật** bằng
  `source = ".../untyped_store.rs#text:..."` (nguyên văn luật ở `untyped_store.rs:71`: "Cắt arena lớn
  nhất có thể từ vùng RAM usable **lớn nhất** của `bi`") — `check_kernel.py:306-309` so **chuỗi con**,
  nên `#text:` là hợp lệ.
- FPU **đã xong** ở `KT-418` (DONE, `v2/agent/QUEUE.md:472`): `fpu.rs` 368 dòng, `FpuArea`
  `#[repr(C, align(16))]` (`fpu.rs:62`) + `FpuTracker` (`fpu.rs:107`) ⇒ nhóm 7 chỉ còn **phần spec**.

**Thay thế B:** tệp riêng `spec/profiles.toml` — chỉ hợp lý nếu Agent 0 muốn `kernel.toml` đóng băng
vĩnh viễn; khi đó phải thêm bước cổng + checker và ghi rõ trong một D2 mới.

### Nhóm 8 — Hợp đồng `BootCapLayout` thay hằng `EP_*`

**Khuyến nghị A — 2 bảng con TOML → 1 struct phẳng:** `[boot_caps.cspace]` + `[boot_caps.proc]` sinh
ra `BootCapLayout` (phẳng), 2 hằng `BOOT_CAPS_TEST`/`BOOT_CAPS_PRODUCT`, `boot_abi = 1` riêng; phủ
**17 `EP_*` + 8 `SLOT_*`/`LOOP_*` + 2 mảng**.

Lý do đo được:

- Hai **không gian chỉ số khác nhau**: cspace slot (17 `EP_*` + `self_cspace` + 3 `LOOP_*`) vs ô
  chương trình `MAX_PROCS` (`SLOT_PROCSRV = 0`, `proc_abi.rs:24`; `MAX_PROCS = 6`, `:160`). Gộp một
  bảng phẳng ⇒ `SLOT_PROCSRV = 0` bị đọc thành cspace slot 0 = `CS_SELF` (`spec/abi.toml:35-36`).
- `SLOT_CHILD` và `SLOT_NETSRV` **cùng số 2 nhưng khác ảnh** (`proc_abi.rs:28,162`) ⇒ phải là **hai**
  giá trị profile, không phải một.
- Hôm nay **0 máy kiểm** canh khớp: `proc_abi.rs:13-15` dẫn `kt77b-harness-scan.sh` §slot nhưng tệp
  đó **không** có mục nào khớp `slot`, và **0** tệp dưới `v2/test`, `v2/bin`, `v2/tools` nhắc
  `proc_abi`; phía U-mode là **17** `const EP_*` trong 4 tệp (`shell.rs:69-94` …) đồng bộ **bằng
  comment** ⇒ `BootCapLayout` chỉ có giá trị nếu đi kèm **máy kiểm** mới (§B.4 nhóm 8).
- `[boot_caps]` **chưa có consumer**: `grep boot_caps v2/tools/vifast-gen/gen.py` = **0** ⇒ `KT-417`
  phải mở rộng generator + `docs/abi.md`, nếu không bước 36 `gen-check` **không** canh mục mới.

**Thay thế B:** một bảng phẳng 25–27 khoá — gọn cho generator nhưng trộn hai không gian chỉ số
(xem trên). Không khuyến nghị.

---

## B.4 Kịch bản kiểm cụ thể theo nhóm (T0 → T1 → T2 → ca phải ĐỎ)

**Quy ước lệnh** (đúng ba tầng, `AGENTS.md` §6.1):

- **T0** = `./v2/bin/v2 t0 <crate…>` (fmt + clippy + `cargo test -p`, `v2/bin/lib/t0.sh:13-14`) —
  **KHÔNG** phải cổng, không tạo bằng chứng nghiệm thu.
- **T1** = `./v2/bin/v2 gate --dev`
- **T2** = `./v2/bin/v2 gate --fast`

**Bước cổng liên quan** (số bước là cố định trong `v2/bin/lib/check.sh`/`gate.sh`): 2 `clippy`
(`check.sh:22`), 3 `luật cấu trúc R1..R5` (`:23`), 26 `test có răng` (`:52`), **28 `abi spec`**
(`:54`), **36 `gen-check`** (`:62`), 43 `L1 (sim 1.000 seed)` (`gate.sh:123`), 70 `QEMU smoke`
(`gate.sh:24`), **77 `spec/kernel.toml ↔ mã`** (`gate.sh:151`), 84 `clippy workspace kernel`
(`check.sh:86`).

| # | T0 (trong lúc sửa) | T1 `gate --dev` — bước phải xanh | T2 `gate --fast` — thêm | Ca **phải ĐỎ** (nếu xanh ⇒ bộ kiểm không có răng) |
|---|---|---|---|---|
| 1 | `./v2/bin/v2 t0 vifast-core vifast-plat-common`; `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core footprint` — pin `size_of::<Machine>() ≤ 184_832` (`v2/kernel/vifast-core/src/footprint.rs:187`) sẽ **bắt** mọi phình do dời `obj` | 77 (mục `[limits.plat]` trỏ **đường dẫn mới**), 28, 3, 84 | 70 (QEMU smoke 7 ảnh) + 43 (L1 1.000 seed) — **bắt buộc** vì chạm allocator/revoke | (a) `mut-state-before-cap` (suite `core-host`, `expect = "red"`, `v2/test/mutants.toml:392-400`) còn ĐỎ; (b) xoá `[limits.plat]` mà **chưa** sửa `check_kernel.py:277` ⇒ **phải** thấy `E_SECTION` — tự kiểm bằng `python3 v2/test/kernel/check_kernel.py --selftest` |
| 2 | `./v2/bin/v2 t0 vifast-core`; ca host **đỏ-trước** cho `invoke` 13/14 (đỏ vì `NotSupported` 18) rồi xanh sau khi nối dispatch | 26 (test có răng), 28 (chữ ký `IrqHandler.SetNotification`), 84 | 70 + 43 — **bắt buộc** vì chạm `invoke` (mọi syscall `Call`) | (a) ca `Busy` khi 2 TCB cùng bind 1 Notification (`core_sync.rs:190-200`) — sửa để trả `Ok` ⇒ phải ĐỎ; (b) `mut-no-irq` và `mut-no-irq-restore` (`v2/test/mutants.toml:269-292`) còn ĐỎ |
| 3 | Không có gì chạy (0 dòng mã, 0 byte spec). Nếu chọn B: `./v2/bin/v2 t0 vifast-core vifast-plat-riscv64` | 28 (kiểm `unsupported`/`install_at` **không đổi**) | Chỉ khi chọn B | Nếu chọn B: ca âm `KT-419` "DMA ghi ngoài bảng `IOSpace` ⇒ bị chặn" (QUEUE.md:474) **phải** ĐỎ; nó xanh ⇒ B sai |
| 4 | `./v2/bin/v2 t0 vifast-core vifast-kbin-riscv64`; ca `Tcb.Exec` (16) + `PageTable.Map/Unmap` đỏ-trước | 26, 28, 84 | 70 + 43 — **bắt buộc** (page table + trap/U-mode) | (a) `!is_whitelisted(ObjType::Tcb, 16)` **trước** khi cài (đỏ-trước) rồi `true`; (b) `!is_whitelisted(ObjType::VSpace, 1)` phải **giữ** `false` (chống tái phát `VSpace.Map`, D2-32) |
| 5 | `./v2/bin/v2 t0 vifast-plat-x86_64`; `python3 v2/test/kernel/check_kernel.py --selftest` | 77 (khối `planned` + `sysret_ready`) | 70 + 59 `QEMU x86_64` (`gate.sh:139`) — **bắt buộc** (GDT/trap) | (a) lật `sysret_ready = true` **mà không** sửa `check_kernel.py:395-396` ⇒ `E_GDT_SYSRET` **phải** ĐỎ (đây là ca âm của chính máy kiểm); (b) `mut-no-int80` (`mutants.toml:245-256`) còn ĐỎ khi vẫn còn `int 0x80` |
| 6 | `./v2/bin/v2 t0 vifast-core`; lật **đúng** `invoke.rs:1128-1129` thành `assert!(is_whitelisted(...))` **và giữ** `assert!(!is_whitelisted(ObjType::CNode, 7))` | 26, 28 (chữ ký `CNode.Mutate`/`Rotate`), 84 | 70 + 43 — **bắt buộc** (quyền + bất biến MDB) | (a) `Mutate` cấp quyền **ngoài** `RIGHTS_ALL` (`spec.rs:32`) phải trả lỗi, không im lặng; (b) bỏ kiểm `RIGHT_WRITE` trên cap `CNode` ⇒ ca `InsufficientRights` 3 phải ĐỎ |
| 7 | `./v2/bin/v2 t0 vifast-core vifast-plat-common` | 77 (`[profiles]` có `source`/`#text:` khớp), 28 | Chỉ khi chạm `TcbCore`/layout — KT-418 đã xong nên đợt này **không** cần | (a) `[profiles]` thiếu `source` ⇒ `E_SOURCE_THIEU`/`E_LEAF_KHONG_SOURCE`; (b) `#text:` không khớp ⇒ `E_SOURCE_TEXT` (cả hai ở `check_kernel.py:291-358`); (c) `footprint.rs:187` nếu `[profiles]` kéo theo đổi `Machine` |
| 8 | **Chưa có ca nào kiểm khớp** (§B.1) ⇒ phải **thêm** ca: (i) so từng trường `BootCapLayout` sinh từ spec với `proc_abi.rs`; (ii) `cargo test -p vifast-abi` cho hằng sinh; (iii) `./v2/bin/vifast-gen --check`; (iv) `python3 v2/test/abi/check_abi.py --selftest` | 28, **36** `gen-check` (`docs/abi.md` phải sinh lại), 26 | 70 + 43 — **bắt buộc** (boot/layout ảnh) | (a) sửa 1 giá trị trong `[boot_caps]` mà **không** sinh lại ⇒ bước 36 ĐỎ; (b) sửa `proc_abi.rs` lệch spec ⇒ ca (i) ĐỎ — nếu xanh thì ca **vô dụng** (luật "test có răng", bước 26) |

**Điều kiện "T2 bắt buộc ngay"** (AGENTS.md §6.1) áp cho nhóm **1, 2, 4, 6, 8** (allocator/revoke,
`invoke` mọi syscall, page table/trap, quyền/MDB, boot/layout) và nhóm **5** nếu chạm GDT/trap.

**Không chạy trong vòng KT-416b:** `gate --fast`, `nightly`, `soak`, `l1-tier`, `mutants` — theo luật
task. Mọi lệnh T2 ở trên chỉ là **kịch bản cho `KT-417`/lead**, không phải việc của tài liệu này.

---

## B.5 Việc còn lại sau KT-416b

1. **Agent 0 chọn A/B** ở §B.0 (1 câu) + §B.3 (8 câu) — mỗi câu đã có khuyến nghị A + lý do đo +
   thay thế B, nên chỉ cần ghi `A`/`B` + dòng `APPROVED <YYYY-MM-DD> Agent 0` vào
   `v2/agent/QUEUE.md` (AGENTS.md §8).
2. **Bốn nợ công cụ phải quyết trong cùng cửa sổ `KT-417`**: `check_kernel.py:277` (`[limits.plat]`),
   `check_kernel.py:395-396` (`sysret_ready`), `check_abi.py:243-244` (**chỉ** nếu nâng bậc), và
   **mới**: `vifast-gen/gen.py` chưa đọc `[boot_caps]`.
3. **KT-416b không sửa** `spec/`, không sửa mã sản phẩm, không sửa `v2/agent/{STATE,QUEUE,DECISIONS,BLOCKED}.md`
   hay `reports/STATUS.md`; bằng chứng đọc-only ở `v2/agent/JOURNAL/kt416b-abi-pack.md`.
4. `KT-417` vẫn `BLOCKED` cho tới khi có `APPROVED` của Agent 0 cho gói này.
