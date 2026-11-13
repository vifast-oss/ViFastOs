<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K2 — bộ nhớ ảo model-only: hình 3 cấp, bảy hằng số `[TK]`, mã lỗi dùng lại (KT-35)

Trạng thái: **KT-35** (K2 mở theo D2-65) — task **đầu** của K2. Tài liệu này **không phải**
`spec/`, **không phải** ABI: không thêm method/chữ ký/mã lỗi nào, không đổi `spec/abi.toml`.
Nguồn ngữ nghĩa: thiết kế §4.10 (`docs/v2/ViFastOs-v2-thiet-ke.md:353-366`),
`review/agent0/K2-PLAN.md` §1 + §2 Q1–Q10 (**D2-65** chốt), `spec/abi.toml` (`[error]`,
`VSpace id=9`, `PageTable id=10`, `Frame id=11` — cả ba `stability="reserved"`,
`unsupported=true`, `install_at="K2"`), D2-17/D2-21/D2-42/D2-47.

Số đo chỉ do `./v2/bin/v2 nightly` sinh (QD §9 Q12); tài liệu này **không** chứa số đo viết tay.
Mọi hằng số dưới đây là **`[TK]`** (thiết kế), không phải `[ƯL]`/`[GT]`.

> **K2 KHÔNG phải kernel thật.** K2 là **mô hình tất định trong sim**: không PTE phần cứng,
> không MMU/TLB thật, không QEMU/không boot, không `sfence.vma`/`tlbi`/`invlpg`, không copy nửa
> cao kernel + physmap, không pager/COW thật. **K3 mới là QEMU + page table thật + TLB thật +
> pager thật.** Mọi tuyên bố "K2 xong" phải đọc kèm câu này (K2-PLAN §0, D2-65 Q10).

## 1. Phạm vi KT-35

| Hạng mục | Nội dung |
|---|---|
| Crate | `v2/lib/vifast-pte` (member workspace `v2/Cargo.toml`), lib `vifast_pte` |
| Có trong task này | hình VA 39 bit, cỡ Frame ↔ `size_bits`, `Pte(u64)` thô, `PteArray<const N>`, bảy hằng số Q1 |
| **Không** có trong task này | PageTable walker/map/unmap, quyền/attrs, W^X, ASID/generation, TLB/shootdown, VM fault, Frame/VSpace thật (KT-36..KT-41) |
| `no_std` | `#![no_std]`; test tích hợp dùng `std` của **crate test** (D2-42) |
| `unsafe` | `#![forbid(unsafe_code)]` — không `unsafe`, không con trỏ thô, không `transmute` (D2-65 Q8) |
| Dependency | **không** dependency ngoài (D2-21), kể cả `vifast-abi`/`alloc` |
| Kiến trúc | **không** `cfg` theo kiến trúc (luật R2): hình PTE là của mô hình, không của kiến trúc |
| `spec/` | **không đổi** (xem §8) |
| `spec/kernel.toml` | **chưa tồn tại, chưa tạo** (Q1, D2-65; D2-53 Q3 hoãn sang K3) |

## 2. Bảy hằng số `[TK]` (Q1)

Tất cả nằm trong crate `vifast_pte` và được chép lại **nguyên giá trị** ở đây; task sau đọc từ
crate, **không** chép tay giá trị vào code của task khác.

| Hằng số | Giá trị `[TK]` | Nghĩa | Dùng ở |
|---|---|---|---|
| `PAGE_SIZE_BITS` | `[12, 21, 30]` | `size_bits` của ba cỡ Frame: 4 KiB / 2 MiB / 1 GiB, cùng thứ tự `FrameSize::ALL` | KT-35, KT-36, KT-37 |
| `PTE_LEVELS` | `3` | số cấp của hình (gốc → giữa → lá), kiểu Sv39 | KT-35, KT-37 |
| `VA_BITS` | `39` | số bit VA; VA chia `9/9/9/12`, bit `38..12` vào ba chỉ số | KT-35, KT-37, KT-38 |
| `ASID_BITS` | `8` | 8 bit ASID ⇒ **256** ASID mỗi thế hệ | KT-38, KT-40 |
| `TLB_ENTRIES_PER_HART` | `16` | số entry TLB mỗi hart của TLB model tất định | KT-40 |
| `MAX_FRAMES_PER_VSPACE` | `4096` | biên số Frame sống trong một VSpace; luỹ thừa 2 (`2^12`) để chia hết bảng 512 entry | KT-36..KT-39 |
| `MAX_MAP_DEPTH` | `3` | độ sâu tối đa khi đi/dựng cây; **bằng** `PTE_LEVELS` (tên riêng theo Q1, dùng làm biên vòng lặp ở KT-37) | KT-37 |

Ghi chú:

- Không có hằng số nào ở đây là **ngưỡng cổng** để nới: đổi giá trị phải có D2 mới của Agent 0.
- `PAGE_SIZE_BITS` là **mảng** theo cấp lá (phần tử thứ `i` = cấp `i`), không phải "một cỡ duy
  nhất"; `FrameSize::size_bits()` trả cùng giá trị nên hai nguồn không thể lệch nhau (test
  `hang_so_q1_dung_gia_tri_chot` + `frame_size_bits_va_byte_roundtrip`).
- `MAX_MAP_DEPTH == PTE_LEVELS` là **cố ý**: Q1 yêu cầu tên riêng cho biên vòng lặp của
  walker/map, nhưng K2 **không** được nới hình 3 cấp.
- Q1 chỉ **nêu tên** hai hằng số `MAX_FRAMES_PER_VSPACE` và `MAX_MAP_DEPTH`, không cho số;
  giá trị `4096` / `3` là `[TK]` do KT-35 chọn và đã được `reviewer-kt35` chấp nhận (Agent 0 xác
  nhận). Đổi giá trị phải có D2 mới của Agent 0, không tự nới trong task sau.

## 3. Hình ba cấp (Q2)

### 3.1 VA 39 bit: `9 / 9 / 9 / 12`

```
 VA 39 bit:   [38:30]   [29:21]   [20:12]   [11:0]
              9 bit      9 bit      9 bit     12 bit
                │          │          │          └── offset trong trang (4 KiB)
                │          │          └───────────── chỉ số cấp 0 (bảng lá 4 KiB)
                │          └──────────────────────── chỉ số cấp 1 (2 MiB)
                └─────────────────────────────────── chỉ số cấp 2 = gốc (1 GiB)

 root (cấp 2): 512 PTE × 8 B = 4096 B ──► bảng cấp 1
 bảng cấp 1  : 512 PTE             ──► bảng cấp 0
 bảng cấp 0  : 512 PTE             ──► trang vật lý 4 KiB
```

- Mỗi bảng có `2^9 = 512` entry; mỗi entry 8 B ⇒ `PteArray<512>` **đúng** 4096 B = một trang.
- Cấp **2** là gốc; cấp **0** là bảng lá cho trang 4 KiB. Thứ tự ba chỉ số trong
  `WalkShape::indices()` là `[cấp 2, cấp 1, cấp 0]` (`level2`/`level1`/`level0`).

### 3.2 Lá theo cỡ Frame (Q2: 1 GiB / 2 MiB / 4 KiB)

| `FrameSize` | `size_bits` | Cỡ | Cấp lá | Một entry phủ |
|---|---|---|---|---|
| `FrameSize::K4` | 12 | 4 KiB | 0 (lá) | đúng 1 trang 4 KiB |
| `FrameSize::M2` | 21 | 2 MiB | 1 (lá) | **một bảng cấp 0** (512 entry) = `2^9` trang 4 KiB; lá ở cấp 1 |
| `FrameSize::G1` | 30 | 1 GiB | 2 (gốc, lá) | **một bảng cấp 1** (512 entry) = `512 × 512 = 2^18` trang 4 KiB; lá ở cấp 2 |

- Kernel (model) **chọn cấp lá theo cỡ Frame** (design §4.10); 2 MiB/1 GiB phải phủ **đúng một
  entry lá**, **không** cắt ngang entry và không trộn cỡ trong cùng một nhánh (KT-37 kiểm).
- `walk_shape` **không** chọn lá, không đọc PTE; nó chỉ tách hình. Việc "entry này là lá hay
  bảng con" là việc của KT-37 (cần bit/loại entry).

### 3.3 Căn lề theo cỡ

- `pa % frame_size == 0` và `va % frame_size == 0`; sai ⇒ mã `Alignment` (số 6, §4).
  `frame_size = 1 << size_bits`; với 2 MiB/1 GiB nghĩa là **toàn bộ bit offset dưới cấp lá bằng 0**.
- Frame 4 KiB căn 4 KiB; 2 MiB căn 2 MiB (bit `20..0` của pa/va bằng 0); 1 GiB căn 1 GiB (bit
  `29..0` bằng 0). `FrameSize::size_bytes()` trả `1 << size_bits` để task sau dùng, không chép số.
- KT-35 **không** kiểm căn lề: crate chỉ có hình/layout. KT-36 kiểm `pa` khi tạo/ownership Frame;
  KT-37 kiểm `va` khi map/unmap (`Alignment`).

### 3.4 `walk_shape(va)` — ví dụ biên

| VA | `indices()` `[cấp2, cấp1, cấp0]` | Ghi chú |
|---|---|---|
| `0` | `[0, 0, 0]` | |
| `0x1000` | `[0, 0, 1]` | offset 0, sang trang kế |
| `1 << 21` | `[0, 1, 0]` | |
| `1 << 30` | `[1, 0, 0]` | |
| **`1 << 38`** | **`[256, 0, 0]`** | **cao nhất của VA 39 bit; cắt đúng 3 chỉ số, không panic** |
| `(511<<30) \| (511<<21) \| (511<<12)` | `[511, 511, 511]` | ba chỉ số lớn nhất |
| `u64::MAX` | `[511, 511, 511]` | bit trên `VA_BITS` bị **cắt**, không panic |

`walk_shape` là `const fn` nhận **mọi** `u64`; nó **không** kiểm canonical/`RangeError` (việc của
KT-37) và **không** phải walker: không đọc bộ nhớ, không dereference, không panic. Test
`walk_shape_property_10k_va` kiểm 10.000 VA tất định (LCG): ba chỉ số khớp phép dịch trực tiếp,
mỗi chỉ số `< 512`, dựng lại bit `12..38` khớp VA đã cắt.

## 4. Bảng mã lỗi **dùng lại** (7 mã) — không thêm mã ABI

Bảy mã dưới đây **đã có** trong `spec/abi.toml` `[error]`; K2 chỉ dùng lại **tên**, không đổi số,
không thêm mã. Crate không phụ thuộc `vifast-abi` nên các mã này sẽ được task cài đặt map sang
`KError` của model ở KT-36..KT-39; KT-35 chỉ cần `InvalidArgument` (mã chung số 5) cho
`size_bits` lạ.

| Số ABI | Tên | Dùng ở K2 | Khi nào |
|---|---|---|---|
| 4 | `WxViolation` | KT-39 | map `WRITE` khi Frame đang `mapped_x > 0`, hoặc map `EXEC` khi `mapped_w > 0`; **atomic**, không đổi gì |
| 6 | `Alignment` | KT-36, KT-37 | `pa`/`va` không căn theo `frame_size` (§3.3) |
| 10 | `AlreadyMapped` | KT-37 | map vào VA đã có mapping sống |
| 11 | `NotMapped` | KT-37 | unmap/protect VA chưa map |
| 12 | `MissingPageTable` | KT-37 | client chưa map PageTable cần thiết — "kernel không tự cấp page table" (design §4.10) |
| 13 | `HasChildren` | KT-37 | unmap gốc bảng khi còn bảng con sống |
| 19 | `RangeError` | KT-37 | VA ngoài `VA_BITS`/ngoài khoảng của VSpace |

Ngoài bảng: `InvalidArgument` (số 5) — `FrameSize::from_size_bits` từ chối `size_bits` lạ (§5).
Đây là mã **chung**, không nằm trong 7 mã VM ở trên; không thêm mã, không đổi `spec/`.

## 5. API crate `vifast-pte`

| Kiểu/hàm | Nghĩa | Ràng buộc |
|---|---|---|
| `FrameSize {K4, M2, G1}` | ba cỡ Frame; `ALL = [K4, M2, G1]` | tên biến thể `<đơn vị><số>` vì Rust không cho định danh bắt đầu bằng số |
| `FrameSize::size_bits()`, `size_bytes()` | `12/21/30`; `4096 / 2^21 / 2^30` | `const fn` |
| `FrameSize::from_size_bits(u8)`, `TryFrom<u8>` | `Ok` chỉ với 12/21/30; còn lại `Err(PteError::InvalidArgument)` | **không** panic; lỗi model/local, không đổi ABI |
| `Pte(u64)` | newtype `u64` **thô** | KT-35 **không** gán nghĩa bit (rights/attrs/PPN là KT-37); `Pte::EMPTY == 0` |
| `PteArray<const N: usize>` | mảng PTE cố định, `N × 8` B, `PteArray<512>` = 4096 B | `new`/`len`/`is_empty`/`byte_size`/`as_array`/`get`/`set`/`iter`; `get`/`set` dùng `Option`, ngoài biên **không** panic, **không** ghi |
| `walk_shape(u64) -> WalkShape` | tách VA thành `[cấp2, cấp1, cấp0]` | `const fn`, mọi `u64`, không panic, không kiểm canonical |
| `PteError` | lỗi cục bộ, hiện chỉ `InvalidArgument` | không phải bảng mã ABI; `code_name()` trả tên đúng `spec/abi.toml` |

Ghi chú thiết kế: `Pte` cố ý **thô** để KT-37 tự chốt layout quyền/attrs mà không phải phá API
KT-35; `PteArray` cố ý **không** `Copy` để tránh copy ngầm bảng 4 KiB.

## 6. Q1–Q8 chốt theo D2-65 (tóm tắt để đọc nhanh)

| Câu | Chốt (D2-65) | Hệ quả ở KT-35 |
|---|---|---|
| Q1 | Hằng số `[TK]` ở `v2/docs/k2-vm.md` + crate; **không** mở `spec/kernel.toml` | §2; crate là nguồn giá trị, docs là bản chép |
| Q2 | Page table **3 cấp Sv39** (1G/2M/4K), căn lề theo cỡ | §3; hình/layout của crate |
| Q3 | W^X đếm **toàn cục theo Frame**; map W sau X ⇒ `WxViolation` **atomic**, không tự unmap | KT-39; KT-35 chỉ giữ hằng số/bảng lỗi |
| Q4 | `ASID_BITS = 8` (256/thế hệ) + generation; invalidate `(asid,va)`/`asid`/`gen` | `ASID_BITS`, `TLB_ENTRIES_PER_HART` trong crate |
| Q5 | `TlbFlush` vào hộp thư model ở K2 (nới D2-59); ack `fetch_max`; `unmap` chỉ `Ok` khi đủ ack; hart chờ ack **phải** rút hộp thư | KT-40; KT-35 không cài |
| Q6 | `FaultKind::Vm` **model-only**, không vào `spec/`; không `fault_ep` giữ hành vi K1c | KT-41; KT-35 không cài |
| Q7 | Shared read-only Frame: **có**; COW: **không** (pager là K3) | KT-42; KT-35 không cài |
| Q8 | Crate `vifast-pte` `forbid(unsafe_code)`, `Pte(u64)` + `PteArray<N>`, không dependency ngoài, Miri ngoài `gate --fast` | Chính crate này |

Q9 (thứ tự A→B∥C→41→42→43) và Q10 (cổng K2 100k×4 + 16 mutation + Miri + `.scn`, 0 vi phạm
TLB-safety) là cổng của KT-43; KT-35 không tự thêm ngưỡng nào.

## 7. Ranh giới "K2 không phải kernel thật"

- **Không** PTE phần cứng/MMU/TLB thật; không `sfence.vma`/`tlbi`/`invlpg`; không PBMT/Sv48.
- **Không** QEMU/OpenSBI/boot; không chạm nhân thật; mô hình chạy trong sim tất định.
- **Không** nửa cao kernel + physmap + copy entry gốc (design §4.10) — chỉ ghi chú, K3.
- **Không** pager/COW/cấp phát lười/file ánh xạ; **không** Device Frame/`MapIo`/IOMMU; **không**
  VSpace khách (`GUEST` vẫn `NotSupported`, D2-32).
- **Không** tuyên bố "đã mô hình shootdown" khi chưa có kênh `TlbFlush` (K2-PLAN Q5) — KT-40.

## 8. `spec/` không đổi, `kernel.toml` chưa tạo

- `spec/abi.toml` **không đổi**: `VSpace id=9`, `PageTable id=10`, `Frame id=11` vẫn
  `stability="reserved"`, `unsupported=true`, `install_at="K2"`; method id giữ nguyên; K2 cài
  ngữ nghĩa **sau model API** trong sim (K2-PLAN §1.2).
- **Không** tạo `spec/kernel.toml`: hiện `spec/` chỉ có `abi.toml`; tạo file mới là quyết định ABI
  mới, phải qua Agent 0 + D2 riêng và D2-53 Q3 đã hoãn sang K3. KT-35 **không** gộp việc đó.
- Crate `vifast-pte` không phụ thuộc `vifast-abi`; tên lỗi được chép **đúng tên** để không sinh
  từ vựng thứ hai, nhưng không đổi số/không thêm mã.

## 9. Cổng và bằng chứng của KT-35

- `./v2/bin/v2 gate --fast` — PASS (log `v2/agent/JOURNAL/kt35-gate-fast.log`).
- `./v2/bin/v2 miri -p vifast-pte` — PASS (log `v2/agent/JOURNAL/kt35-miri.log`).
- Test đỏ trước: `cargo test ... -p vifast-pte --test kt35_pte` ⇒ `E0432` unresolved imports khi
  crate mới có khung rỗng (log `v2/agent/JOURNAL/kt35-red-compile.log`).
- Không `#[ignore]`, không `#[should_panic]`, không `unsafe`, không dependency, không sửa
  `spec/`, `.scn`, `vifast-model`, QUEUE/DECISIONS/AGENTS, `viscript/`.
