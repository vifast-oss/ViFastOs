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

## 10. KT-36 — `Frame` 3 cỡ, ownership/Untyped, `GetAddress`, bộ đếm W^X

Mục này là **ngữ nghĩa model-only `[TK]`** của Frame trong K2 (K2-PLAN §1.1 mục 1 + §3 KT-36,
D2-65 Q2/Q3/Q7). Không thêm method/chữ ký/mã lỗi vào `spec/`: `Frame id=11` vẫn `reserved`,
`unsupported=true`, `install_at="K2"`; K2 cài ngữ nghĩa **sau model API** của sim.

### 10.1 `FrameMeta` — 6 trường DoD

| Trường | Kiểu | Nghĩa |
|---|---|---|
| `pa` | `u64` | địa chỉ vật lý **của mô hình** = `base` (cửa sổ PA của Untyped) + watermark; **không** phải vùng nhớ thật |
| `size_bits` | `u8` | `12` / `21` / `30` — cùng nguồn `vifast_pte::FrameSize::{K4,M2,G1}` (không chép tay) |
| `live` | `bool` | Frame còn sống; `false` sau khi Untyped sở hữu bị `CNode.Revoke` |
| `maps` | `u32` | tổng số mapping **sống** của Frame này (mọi VSpace; KT-37 mới map thật) |
| `mapped_w` | `u32` | số mapping sống có quyền `WRITE` |
| `mapped_x` | `u32` | số mapping sống có thuộc tính `EXEC` |

Bất biến KT-36: `mapped_w ≤ maps`, `mapped_x ≤ maps`; Frame **đã chết** ⇒ cả ba bằng **0**
("frame chết ⇒ không còn mapping"). Bất biến chặt hơn `mapped_w + mapped_x ≤ maps` và
`!(w > 0 && x > 0)` chỉ đúng **sau khi** KT-39 áp luật W^X — KT-36 **cố ý chưa** áp (Q3: "KT-39
toàn bộ luật"). Bộ đếm chỉ đổi qua hai cửa `frame_note_map`/`frame_note_unmap` (model API);
KT-37/KT-39 gọi từ `map`/`unmap` thật.

### 10.2 Cửa sổ PA của Untyped (vì sao `pa` là ảo) — `UNTYPED_PA_WINDOW`

**K2 không cấp byte thật cho Frame.** 1 GiB Frame phải retype được mà **không** ăn RSS (KT-36
rủi ro (a): cấm mọi test dựa trên `Vec` 1 GiB), nên Frame không chứa trong arena host của Untyped
mà cấp từ **cửa sổ PA ảo** của nó:

| Hằng số `[TK]` | Giá trị | Nghĩa |
|---|---|---|
| `UNTYPED_PA_WINDOW` | `1 << 30` (1 GiB) | số byte PA ảo mà **mỗi** Untyped cấp cho Frame; đúng bằng cỡ Frame lớn nhất |

- **Đáy cửa sổ** = tham số `base` của `Machine::add_untyped(base, size)` (trước KT-36 tham số này
  bị bỏ qua; nay là đáy cửa sổ PA). Untyped con (retype `Untyped`) thừa hưởng
  `base_cha + offset_con` — quan hệ **logic** của mô hình, không phải containment.
- **Watermark** cửa sổ tiến theo từng Frame đã cấp; `Revoke` Untyped (hoặc cha của nó) đưa
  watermark về 0 **song song** với watermark arena (KT-09).
- Arena host của Untyped **không đổi** khi cấp Frame: `untyped_used`/`check_bytes` giữ nguyên
  (Frame không tiêu byte nào). Frame chỉ tốn vài chục byte metadata (`FrameMeta` + body mốc
  `Kind::Frame` trong arena kernel).
- Vì `pa` là nhãn ảo, bất biến **không phải** "hai Untyped không được chồng cửa sổ" mà là
  "**Frame sống** không được chồng `pa`" (D2-66): Frame mới chồng `[pa, pa + cỡ)` với Frame
  **sống** khác — kể cả của Untyped khác — ⇒ `AlreadyMapped`; `check_frames` kiểm lại toàn cục.
  Hệ quả thực dụng: Untyped dùng để retype Frame nên khai `base` căn theo cỡ Frame và **rời nhau**
  (hai cửa sổ chồng nhau vẫn hợp lệ nếu không Frame sống nào chồng `pa`). `add_untyped` từ chối
  ngay khi `base + 1 GiB` tràn `u64`.

### 10.3 `Untyped.Retype(Untyped, Frame, size_bits)` — thứ tự kiểm (mọi lỗi **atomic**)

1. `size_bits ∉ {12, 21, 30}` ⇒ **`InvalidArgument`** (không còn `NotSupported` như thời reserved).
2. `pa = base + watermark` **chưa** căn đúng `1 << size_bits` ⇒ **`Alignment`**. Mô hình
   **không** căn lề ngầm: đệm ngầm sẽ làm `pa` phụ thuộc thứ tự cấp một cách khó kiểm, và DoD
   KT-36 yêu cầu đúng ca "pa lệch 1 ⇒ Alignment". Muốn trộn cỡ trong một cửa sổ thì cấp theo thứ
   tự **giảm dần** (1 GiB → 2 MiB → 4 KiB luôn đúng căn; tăng dần thì không).
3. `pa + cỡ` ra ngoài cửa sổ 1 GiB ⇒ **`NotEnoughMemory`** (vd một Frame 1 GiB chiếm trọn cửa sổ).
4. `[pa, pa + cỡ)` chồng Frame **sống** của Untyped khác ⇒ **`AlreadyMapped`**.
5. Thiếu byte metadata trong arena kernel ⇒ `NotEnoughMemory` (không phải "hết bộ nhớ Frame").

Frame do retype tạo là **con của cap Untyped nguồn** (kế thừa quyền cap nguồn, không nới ngược)
và **thuộc Untyped** đó: `Revoke` Untyped finalise mọi Frame trong cây, kể cả Untyped con lồng
nhau (KT-09).

### 10.4 `Frame.GetAddress` (method 2)

- Qua cap: `Machine::frame_get_address(cap)` — cap phải **sống** + trỏ `Frame`
  (`WrongObjectType`); Frame đã chết ⇒ `InvalidCapability`; slot rỗng ⇒ `SlotEmpty`; ngoài dải ⇒
  `RangeError`. Trả `pa`.
- Theo `ObjId` (model API cho KT-37/KT-39): `Machine::frame_address(frame)`.

### 10.5 Finalise Frame — "frame chết ⇒ không còn mapping"

- `Revoke` Untyped có Frame đang `maps > 0` ⇒ **`HasChildren`** và **không đổi gì** (atomic):
  mô hình **không** im lặng bỏ mapping, cũng không tự unmap (unmap thật là KT-39). Phải
  `frame_note_unmap` hết rồi mới revoke được.
- Frame chết: `live = false`, ba bộ đếm **đã** bằng 0 từ trước; `frame_meta` vẫn đọc được sau
  chết (DoD "frame chết sạch").
- `check_frames()` (mở ở KT-36, KT-42 gộp vào `check_vm`) kiểm **bảy nhóm** (khớp D2-66):
  1. **kiểu** — `ty == Frame` ⇔ có `FrameMeta`; 2. **trạng thái** — `FrameMeta.live == Meta.alive`;
  3. **cỡ** — `size_bits ∈ {12,21,30}`, `len == 1 << size_bits`; 4. **cửa sổ PA + chủ sở hữu** —
  `pa == base + offset`, `pa` căn cỡ, `[pa, pa + cỡ)` trong cửa sổ, Frame sống ⇒ Untyped sở hữu
  sống (không mồ côi); 5. **bộ đếm cấu trúc** — `mapped_w ≤ maps`, `mapped_x ≤ maps`; 6. **frame
  chết sạch** — đã chết ⇒ cả ba bộ đếm 0; 7. **toàn cục** — không hai Frame sống chồng `pa` và
  watermark cửa sổ = `max(offset + len)` của Frame sống. Chạy **sau mỗi public op** (qua
  `sched_after_op`, có cửa sổ nhanh `frame_objects == 0` nên mô hình chưa từng có Frame không tốn
  chi phí).

### 10.6 Không làm ở KT-36 (ranh giới)

- **Không** map/unmap Frame vào PageTable/VSpace, không rights/attrs khi map, không
  `MissingPageTable`/`NotMapped`/`HasChildren` của lá — **KT-37**; không luật W^X
  (`WxViolation`) — **KT-39**; không PageTable/VSpace/ASID/TLB/shootdown — KT-37..KT-41.
- **Không** Device Frame/`MapIo`/`MapUnmapIo` (D3), không pager/COW (K3), không PTE/MMU thật.
- **Không** đổi `spec/`: `Frame` vẫn `reserved` + `unsupported=true`; ngữ nghĩa trên là model API
  của sim, conformance "reserved ⇒ `NotSupported`" của `vifast-model`/`.scn` không đổi.

## 11. KT-37 — `PageTable` 3 cấp: map/unmap Frame, rights/attrs, 7 mã lỗi

Mục này là **ngữ nghĩa model-only `[TK]`** của PageTable trong K2 (K2-PLAN §3 KT-37, D2-65
Q2/Q3/Q7, thiết kế §4.10 `:353-366`). Không thêm method/chữ ký/mã lỗi vào `spec/`: `PageTable
id=10` vẫn `reserved`, `unsupported=true`, `install_at="K2"`; K2 cài ngữ nghĩa **sau model API**
của sim. **K2 không phải kernel thật**: không PTE phần cứng/MMU/TLB; **chưa** VSpace/ASID/TLB
(KT-38/40), **chưa** W^X (KT-39), **chưa** VM fault (KT-41).

### 11.1 Object `PageTable` và cây 3 cấp

| Hạng mục | Nội dung |
|---|---|
| Tạo | `Untyped.Retype(Untyped, PageTable, size_bits)`; `size_bits ∈ {0, 12}` (4096 B), cỡ khác ⇒ `InvalidArgument`; tiêu **4096 B arena** của Untyped (khác Frame — bảng thật là một trang PTE) |
| Dữ liệu | 512 PTE (`vifast_pte::Pte`, layout §11.2) + `level` + `attached: Option<(cha, chỉ số)>` + `mapped` (số lá trong cây con) — đặt ở `Machine`, body objmem chỉ là mốc `Kind::PageTable` |
| Bảng mới | `level = 2` (gốc), chưa attach, rỗng; dùng làm **root** cho `map`/`unmap`/`walk` |
| Chọn lá | `FrameSize::K4` → cấp 0; `M2` → cấp 1; `G1` → cấp 2 (bảng `§3.2`); `size_bits - 12) / 9` |
| Bảng phủ | cấp 0 phủ 2 MiB, cấp 1 phủ 1 GiB, cấp 2 phủ toàn bộ VA 39 bit (`table_span`) |
| Attach | `page_table_attach(parent, va, child)`: cha cấp 2 ⇒ con cấp 1, cha cấp 1 ⇒ con cấp 0; `va` phải căn **đúng vùng con phủ**, ô đích rỗng; con phải chưa attach + rỗng; cha cấp 0 không có cấp dưới |
| Detach | `page_table_detach(parent, va)`: `va` phải đúng **gốc vùng** con phủ (`Alignment`), trong VA 39 bit (`RangeError`); con còn **bất kỳ** entry/lá nào ⇒ `HasChildren` (atomic); con rỗng ⇒ gỡ, trả thành bảng gốc cấp 2 dùng lại được |

Thứ tự chỉ số lấy từ `vifast_pte::walk_shape` (`[cấp 2, cấp 1, cấp 0]`, VA `9/9/9/12`); biên
vòng lặp `vifast_pte::MAX_MAP_DEPTH = 3`; một bảng `PteArray<512>` = 4096 B.

### 11.2 Layout PTE của **mô hình** (KT-37 chốt)

`vifast_pte::Pte` là `u64` thô; KT-37 gán nghĩa bit (K3 sẽ có PTE thật, không đọc layout này):

```text
bit 0     : valid
bit 1     : 1 = bảng con (Table), 0 = lá (Leaf)
bit 2..=3 : rights lá: bit 2 = READ, bit 3 = WRITE
bit 4..=9 : attrs lá: WB, WC, UC, Device, NX, EXEC (mỗi thuộc tính một bit)
bit 10..41: ObjId (u32) của Frame (lá) hoặc PageTable con (bảng)
```

Vì là **mô hình**, PTE **không** mang `pa`: `pa` đọc từ `FrameMeta` lúc walk (PTE thật của K3
mang PPN). `map` **lưu** rights/attrs của **từng mapping** do tham số quyết định, **không** suy
từ quyền của cap Frame. KT-37 **không** áp luật W^X, không kiểm "WB/WC/UC/Device đúng một giá
trị", không từ chối `Device + EXEC` — KT-39.

### 11.3 API model (không đổi `spec/`)

| Hàm | Nghĩa |
|---|---|
| `page_table_map(root, va, frame_cap, rights, attrs)` | `PageTable.Map` qua **cap Frame** (method 0, vẫn reserved) |
| `page_table_map_obj(root, va, frame, rights, attrs)` | model API theo `ObjId` (property/oracle) |
| `page_table_unmap(root, va)` | `PageTable.Unmap` (method 1): gỡ lá phủ `va`, trả `Mapping` đã gỡ |
| `page_table_walk(root, va)` | walk ngược VA→PA, trả `Mapping { va, pa, size_bits, frame, rights, attrs }` |
| `page_table_attach` / `page_table_detach` | dựng/gỡ bảng cấp dưới (KT-37 thêm vì chưa có VSpace) |
| `page_table_count` / `page_table_info` / `page_table_pte_raw` | nguồn test/harness (không phải ABI) |
| `check_pages()` | bất biến cây + bộ đếm, chạy **sau mỗi public op** qua `sched_after_op` (cửa sổ nhanh `page_table_objects == 0`) |

`map`/`unmap`/`walk` **chỉ** nhận PageTable **gốc** (chưa attach, cấp 2). Truyền bảng cấp 1/0
(đã attach) ⇒ `MissingPageTable`: bảng gốc cấp trên không nằm trong lời gọi. Đây là cách KT-37
có ca `MissingPageTable` cho **cả 1 GiB** (lá 1 GiB nằm ngay root nên không có bảng giữa).

### 11.4 Thứ tự kiểm và 7 mã lỗi (mọi lỗi **atomic**)

`map`:

1. `root` phải là PageTable gốc ⇒ nếu không: `MissingPageTable` (sai loại/chết: `WrongObjectType`/`InvalidCapability`);
2. Frame sống + `size_bits ∈ {12,21,30}`; cỡ lạ ⇒ `InvalidArgument`;
3. `va >= 1 << 39` hoặc `va + cỡ > 1 << 39` ⇒ `RangeError`;
4. `va`/`pa` lệch căn cỡ Frame ⇒ `Alignment` (2 MiB/1 GiB phủ **đúng một** entry lá);
5. thiếu bảng cấp trên trên đường đi ⇒ `MissingPageTable`;
6. entry lá đã có (kể cả lá lớn 2 MiB/1 GiB phủ VA) ⇒ `AlreadyMapped`;
7. `root.mapped >= MAX_FRAMES_PER_VSPACE` (4096) ⇒ `NotEnoughMemory`;
8. cộng `maps/mapped_w/mapped_x` qua `frame_note_map` **rồi** mới ghi PTE.

`unmap`: `RangeError` ngoài VA 39 bit; `NotMapped` nếu không có lá phủ `va`; **chỉ** nhận `va`
đúng **gốc** mapping — lệch ⇒ `Alignment` (không xoá ngầm trang lớn); trừ bộ đếm Frame trước khi
xoá PTE (lỗi ⇒ cây còn nguyên).

| Mã (ABI) | Ca KT-37 |
|---|---|
| `AlreadyMapped` (10) | map trùng VA; map 4K/2M vào vùng đã bị lá lớn phủ; attach vào ô đã có bảng/con đã attach |
| `NotMapped` (11) | unmap VA chưa map; detach ô rỗng/ô chứa lá |
| `MissingPageTable` (12) | thiếu bảng cấp 1 (map 4K/2M) hoặc cấp 0 (map 4K); truyền bảng cấp 1/0 làm root |
| `HasChildren` (13) | detach bảng còn entry/lá; `Revoke` Untyped có PageTable còn entry/đang attach |
| `Alignment` (6) | `va`/`pa` lệch căn; attach/detach `va` lệch vùng con phủ; unmap giữa trang lớn |
| `RangeError` (19) | `va`/`va + cỡ` ngoài VA 39 bit (map/attach/unmap/walk) |
| `InvalidArgument` (5) | `PageTable` retype `size_bits` lạ; attach cha cấp 0/tự attach/con không rỗng; Frame cỡ lạ |

### 11.5 Bất biến `check_pages` (5 nhóm) và rò rỉ

1. **kiểu/dữ liệu**: `ty == PageTable` ⇔ có `Meta.pt` + dữ liệu; object chết ⇒ dữ liệu `None`;
   object sống ⇒ dữ liệu `Some`, `len == 4096`;
2. **cấp/attach**: root ⇒ chưa attach + cấp 2; con ⇒ cấp = cha − 1 và PTE của cha trỏ **đúng**
   con tại **đúng** chỉ số;
3. **cây**: mọi bảng con được trỏ **đúng một lần**, không bảng mồ côi; mỗi lá cấp đúng cỡ Frame,
   Frame sống, `pa` căn cỡ;
4. **bộ đếm**: `PageTableData.mapped` = số lá cây con; với **mọi** Frame, `maps/mapped_w/mapped_x`
   = số lá trỏ tới Frame + quyền `WRITE`/attr `EXEC` tương ứng (toàn cục, mọi root);
5. **biên**: `mapped ≤ MAX_FRAMES_PER_VSPACE` mỗi cây.

"Không rò PageTable/Frame": `Revoke` Untyped chỉ finalise PageTable **tinh khiết** (chưa attach,
không PTE, `mapped = 0`); ngược lại ⇒ `HasChildren` atomic (cùng tinh thần `finalise_frame` của
KT-36). Bảng bị finalise chuyển tombstone `None` (không giữ 4 KiB PTE); Frame chết vẫn phải có
ba bộ đếm 0. `check_pages` là lưới an toàn thứ hai sau mỗi op.

### 11.6 Không làm ở KT-37 (ranh giới)

- **Không** VSpace/ASID/generation/`active_on` (KT-38), **không** TLB/shootdown (KT-40).
- **Không** luật W^X (`WxViolation`), `protect`, ràng buộc kiểu bộ nhớ/Device (KT-39).
- **Không** VM fault/`FaultKind::Vm` (KT-41), **không** `.scn`/`check_vm`/shared read-only chính
  thức (KT-42; KT-37 có test map cùng Frame vào 2 root như bằng chứng bộ đếm toàn cục).
- **Không** PTE/MMU/TLB thật, không QEMU — K3.

## 12. KT-38 — `VSpace`, ASID (8 bit = 256/thế hệ), `generation`, `active_on: HartMask`

Nguồn ngữ nghĩa: `review/agent0/K2-PLAN.md` §3 KT-38 + §2 Q4/Q9; **D2-65** (Q4/Q9); thiết kế
§4.10 (`:353-366`: "Object: VSpace, PageTable, Frame"; "ASID có generation (hết ASID thì tăng
generation và flush toàn bộ)"; "VSpace giữ `active_on: HartMask`"). Hằng số ASID **đọc từ crate**
`vifast_pte` (`ASID_BITS`), **không** chép tay `256`.

`VSpace` **vẫn `reserved`** trong `spec/abi.toml` (`id = 9`, `stability="reserved"`,
`unsupported = true`, method 0 `GetAsid` chỉ là debug): KT-38 cài ngữ nghĩa **sau model API**
trong sim, **không** đổi ABI/conformance "reserved ⇒ `NotSupported`". VSpace + `GUEST` vẫn
`NotSupported` (D2-32).

### 12.1 Object `VSpace` + **một** root PageTable

**Tạo bằng model API `vspace_create(src)`**, không qua ABI: `Untyped.Retype(VSpace)` **vẫn**
`NotSupported` (spec giữ `reserved`/`unsupported`; conformance + test cũ `kt09_untyped` giữ
nguyên; K2-PLAN §3 KT-38 ghi rõ "tạo VSpace root (model API)"). `vspace_create` nhận cap Untyped
nguồn (cần `WRITE`, như `Retype`) và trả `CapId` cho harness đặt vào CNode (quy ước D2-35); cap
là **con của cap Untyped** nên `Revoke` Untyped finalise VSpace + root theo đúng cây. Nó tạo ra:

| Phần | Vị trí | Tiêu arena | Ghi chú |
|---|---|---|---|
| metadata `VSpaceData` | `Machine::vspaces` (chỉ số `Meta.space`) | **0 byte** | `{ root, asid, generation, active_on }` |
| body `Kind::VSpace` | arena kernel | 0 byte | chỉ là mốc cho objmem, như `Kind::Frame`/`Kind::PageTable` |
| root PageTable | arena của Untyped nguồn | **4096 B** | `PageTableData` rỗng, cấp 2, chưa attach; object **nội bộ, không cap** |

Mọi lỗi (Untyped sai/không `WRITE`, hết arena) **atomic**: không đốt byte, **không cấp ASID**,
không tăng thế hệ (arena được cấp **trước** ASID).

Root là bảng gốc **thật** của hình 3 cấp: `PageTable.Map/Unmap` (KT-37) làm việc trên nó như mọi
bảng gốc; `page_table_walk(root, va)` là **oracle** của `vspace_walk(vs, va)` (một đường walk
duy nhất, không có bảng riêng để hai đường lệch nhau). Root **không được** `page_table_attach`
làm bảng con (`IllegalOperation`) — giữ bất biến "root = bảng gốc"; muốn dựng cây 4 KiB/2 MiB thì
attach các PageTable cấp dưới **vào** root như KT-37.

API model (không đổi `spec/`):

| Hàm | Nghĩa |
|---|---|
| `vspace_create(src)` | **model API** tạo VSpace + root 4 KiB + ASID (không mở ABI) |
| `vspace_count()` | số VSpace sống |
| `vspace_info(vs)` / `vspace_root(vs)` / `vspace_get_asid(vs)` / `vspace_active_on(vs)` | ảnh chụp / root / `GetAsid` (debug) / `active_on` |
| `vspace_walk(vs, va)` | `page_table_walk(root, va)` — oracle |
| `vspace_map(vs, va, cap_frame, rights, attrs)` / `vspace_unmap(vs, va)` | `Frame.Map`/`Frame.Unmap` model API (§4.10), uỷ quyền nguyên KT-37 (**không** áp W^X — KT-39) |
| `vspace_activate(vs, hart)` / `vspace_deactivate(vs, hart)` | hart **chạy**/**nghỉ** trên VSpace (`active_on`) |
| `set_hart_online(hart, online)` / `hart_online()` | mask hart online của mô hình |
| `asid_state()` | `{ generation, free, issued, exhaustions }` |
| `flush_log()` / `flush_count()` / `last_flush()` | log **lệnh flush** cho KT-40 |
| `check_vspace()` / `check_asid()` | bất biến, chạy sau **mỗi** public op |

### 12.2 ASID: 8 bit, 256/thế hệ, free-list, **không tái dùng trong cùng thế hệ**

- `ASID_COUNT = 1 << ASID_BITS` = **256** (`vifast_pte::ASID_BITS = 8`, Q4).
- Free-list của thế hệ hiện tại là dãy **giảm dần** `255, 254, …, 0`; cấp = `pop()` ⇒ ASID thấp
  nhất trước ⇒ chuỗi cấp trong một thế hệ là `0, 1, 2, …` (tất định, dễ đọc trong log/test).
- **ASID đã cấp thì "về hưu" cho hết thế hệ**: teardown VSpace **không** trả ASID về free-list
  hiện tại. Lý do (K2-PLAN §3 KT-38 "rủi ro: tái dùng ASID sớm ⇒ lỗi stale TLB giả"): hart đã
  rời VSpace vẫn có thể còn entry TLB mang `(gen, asid)`; nếu ASID được phát lại trong cùng thế
  hệ thì entry đó sẽ khớp nhầm VSpace mới. Vì vậy teardown = **thôi sở hữu** + flush theo `asid`
  + **về hưu**, không phải "trả về free-list".
- **Cạn** (free-list rỗng) ⇒ `generation += 1` **đúng một lần**, free-list nạp lại đủ 256, phát
  **đúng một** lệnh `FlushScope::Generation` cho **mọi hart online** (Q4: "flush toàn bộ `active_on`
  một lần theo `gen`", **không** flush từng ASID). VSpace cũ giữ nguyên `(asid, generation)` cũ;
  ASID của thế hệ mới có thể **trùng số** với thế hệ cũ — chỉ `(gen, asid)` là danh tính.
- Bất biến: `free-list + issued == 256`; free-list giảm dần nghiêm ngặt; **không** hai VSpace sống
  cùng `(generation, asid)`; VSpace thuộc thế hệ hiện tại giữ ASID **không** nằm trong free-list;
  số VSpace sống của thế hệ hiện tại `≤ issued`.

### 12.3 `active_on: HartMask`

`HartMask` (kiểu riêng của mô hình, **≤ 64 bit**, `D2-59` tinh thần `HartMask`): `contains/insert/
remove/union/intersection/is_subset_of/for_harts(harts)`. `hart_online` khởi tạo =
`for_harts(hart_count)`.

- `vspace_activate(vs, hart)`: bật bit; hart đang chạy VSpace **khác** ⇒ bỏ bit ở VSpace cũ trước
  (một hart chỉ nạp **một** ASID hiện hành, như thanh ghi ASID của MMU); gọi lại trên đúng VSpace
  ⇒ no-op. Hart **offline**/không có thật ⇒ `InvalidArgument`.
- `vspace_deactivate(vs, hart)`: tắt bit; hart không chạy đúng VSpace ⇒ `IllegalOperation`.
- `set_hart_online(hart, false)`: bỏ hart khỏi **mọi** `active_on` + khỏi `hart_vspace`; bật lại
  **không** tự gắn vào VSpace nào.
- Bất biến: `active_on ⊆ hart_online`; `hart_vspace[h] == Some(vs)` ⇔ bit `h` bật trong `active_on`
  của `vs`; mỗi hart tối đa **một** VSpace; `hart_vspace` chỉ trỏ VSpace sống.

### 12.4 **Lệnh flush** (mô hình — KT-40 tiêu thụ)

KT-38 **không** có TLB/shootdown thật; nó ghi **lệnh** vào `Machine::flush_log`:

| Trường | Nghĩa |
|---|---|
| `seq` | 1-based, tăng đơn điệu (= `flush_log.len()`) |
| `scope` | `Asid` (teardown VSpace) hoặc `Generation` (cạn ASID) |
| `generation` | thế hệ áp dụng: của VSpace bị teardown, hoặc thế hệ **mới** khi cạn |
| `asid` | `Some(asid)` với `Asid`, `None` với `Generation` |
| `harts` | `HartMask` mục tiêu — **đã lọc hart online** (hart offline không bao giờ nhận lệnh) |

- Teardown VSpace ⇒ **một** `Asid` với `harts = active_on` tại thời điểm teardown (có thể rỗng:
  chưa hart nào chạy ⇒ không shootdown hart nào).
- Cạn ASID ⇒ **một** `Generation` với `harts = hart_online` (mọi hart online — kể cả hart đã rời
  mọi VSpace, vì entry TLB cũ của thế hệ cũ có thể còn).
- Kind `(asid, va)` cho unmap/protect một trang (Q4) là **KT-40**; KT-38 cố ý **không** phát nó.

### 12.5 `check_vspace` (5 nhóm) và `check_asid` (3 nhóm)

`check_vspace`: (1) `ty == VSpace` ⇔ `Meta.space` + dữ liệu, chết ⇒ tombstone, số dữ liệu = số
VSpace sống; (2) root là PageTable **sống**, **bảng gốc** (cấp 2, chưa attach), không VSpace nào
dùng chung; (3) `asid < 256`, `generation ≤ asid_generation`; (4) `active_on ⊆ hart_online` và đối
xứng với `hart_vspace` từng hart; (5) `hart_vspace` chỉ trỏ VSpace sống.

`check_asid`: (1) `free + issued == 256`, phần tử `< 256`, giảm dần nghiêm ngặt; (2) không hai
VSpace sống cùng `(generation, asid)`; (3) VSpace của thế hệ hiện tại không giữ ASID còn trong
free-list, và số VSpace sống của thế hệ hiện tại `≤ issued`.

Cả hai chạy trong `sched_after_op` sau **mỗi** public op, có **cửa sổ nhanh** (chưa từng có
VSpace / trạng thái ASID nguyên thủy) để L1/test K1a..K1c giữ nguyên chi phí. Unit test âm chứng
minh `check_vspace`/`check_asid` **bắt** được: `active_on` ngoài hart online, lệch
`active_on`/`hart_vspace`, VSpace chết còn dữ liệu, root bị attach, free-list trùng, ASID trùng
trong thế hệ, ASID còn trong free-list.

### 12.6 Teardown: `HasChildren` khi còn mapping; thứ tự finalise

`Revoke` Untyped chứa VSpace ⇒ kiểm **trước** mọi thay đổi (atomic): root còn PTE/bảng con/đang
attach ⇒ `HasChildren`; muốn xoá: unmap lá → detach bảng con → revoke (như KT-37). Thứ tự finalise
trong `revoke_children`: Frame → **VSpace** → PageTable (root tombstone sau VSpace để bước kiểm
"root tinh khiết" còn dữ liệu). VSpace chết: phát lệnh `Asid`, gỡ `hart_vspace`, tombstone dữ liệu
⇒ `check_vspace`/`check_asid` thấy "VSpace chết không còn asid/active_on".

### 12.7 Không làm ở KT-38 (ranh giới)

- **Không** TLB model/`TlbFlush` vào hộp thư/IPI/ack thế hệ/chống deadlock (KT-40) — KT-38 chỉ
  có **lệnh flush**; DoD "flush theo `gen` làm mọi entry TLB cũ mất hiệu lực" được kiểm bằng
  oracle của KT-40.
- **Không** W^X/`protect`/`WxViolation` (KT-39); **không** VM fault/`FaultKind::Vm` (KT-41);
  **không** `.scn`/`check_vm`/shared read-only (KT-42).
- **Không** đổi `spec/` (VSpace vẫn `reserved`/`unsupported`), **không** tạo `spec/kernel.toml`.
- **Không** PTE/MMU/TLB/`sfence.vma` thật, không QEMU — **K3**.

## 13. KT-39 — luật W^X toàn cục, `protect`, shared read-only (D2-67)

**W^X là luật của toàn bộ mô hình, không phải của riêng VSpace.** Bộ đếm `mapped_w`/`mapped_x`
nằm trên **Frame vật lý** và là **toàn cục**: một Frame ở trong hai VSpace hay hai PageTable vẫn
chịu **cùng** một luật. `PageTable.Map` của `spec/abi.toml` vẫn `reserved`; KT-39 chỉ áp luật cho
các **model API** đã có (KT-37/38).

### 13.1 Luật `attrs` — `pagetable::validate_attrs` (trước mọi thay đổi)

| Kiểm | Lỗi |
|---|---|
| `WB/WC/UC/Device` phải **đúng một** giá trị | `InvalidArgument` |
| `NX` và `EXEC` cùng lúc | `InvalidArgument` |
| `Device` + `EXEC` | **`WxViolation`** (D2-67: dùng chung mã W^X cho "thiết bị không bao giờ thực thi") |

`attrs` rỗng vẫn phải chọn một kiểu bộ nhớ: caller truyền `MapAttrs::WB` (mặc định "không thực
thi") như mọi ca KT-37/38. Không kiểm `READ/WRITE` ở đây — luật W^X theo **bộ đếm** do
`frame_note_map` áp.

### 13.2 Áp luật trong `frame_note_map` (cửa duy nhất **của đường `map`**)

`map` (`page_table_map*`, `vspace_map*`) đi qua **`frame_note_map(frame, write, exec)`**; `protect`
(`page_table_protect`, `vspace_protect`) đi qua **cửa delta** riêng trong `frame.rs` vì nó **đổi**
một mapping đã đếm chứ không thêm mapping. Hai cửa, **cùng một bất biến** (§13.5); cả hai kiểm
**trước** khi ghi bất cứ thứ gì:

- mapping mới là **W** mà Frame đang có `mapped_x > 0` ⇒ `WxViolation`;
- mapping mới là **X** mà Frame đang có `mapped_w > 0` ⇒ `WxViolation`;
- mapping mới **vừa W vừa X** (một lời gọi) ⇒ `WxViolation`.

Không tự unmap, không tự hạ quyền: JIT phải `unmap` W rồi `map` X (design §4.10). Bộ đếm tràn
`u32` ⇒ `InvalidArgument`; Frame chết ⇒ `InvalidCapability`; không phải Frame ⇒ `WrongObjectType`.

`page_table_map` gọi `validate_attrs` **sau** các kiểm cấu trúc (`MissingPageTable`/`Alignment`/
`AlreadyMapped`/`NotEnoughMemory`) và **trước** `frame_note_map` + ghi PTE — lỗi không đổi cây,
không đổi bộ đếm (atomic).

### 13.3 `PageTable.Protect` / `VSpace.Protect` (model API)

`page_table_protect(root, va, rights, attrs)` và `vspace_protect(vs, …)` đổi `rights`/`attrs` của
**một** lá đang sống:

- `va` phải đúng **gốc** mapping (lệch trong trang lớn ⇒ `Alignment`; chưa map ⇒ `NotMapped`);
- `validate_attrs` như 13.1;
- tính **bộ đếm kết quả**; nếu `w > 0 && x > 0` ⇒ `WxViolation` **atomic** (không đổi PTE/bộ đếm);
- thành công: `maps` giữ nguyên, `mapped_w/mapped_x` đổi đúng hiệu, PTE đổi, trả `Mapping` mới.

Khác `map`: `protect` được phép **W→X hoặc X→W** khi kết quả thoả bất biến, vì đó là yêu cầu
tường minh của người gọi (không phải tự hạ quyền mapping cũ).

### 13.4 Shared read-only

Cùng một Frame map `READ` (không W, không X) vào **hai** VSpace: `maps` = 2, `mapped_w` =
`mapped_x` = 0, **không** cấp bản sao, `pa` giống nhau. Đây là lưới an toàn của bộ đếm toàn cục
(Q7) và là tiền đề cho shared frame của KT-42.

### 13.5 Bất biến và hai lưới đếm

`check_wx` (rẻ, sau **mỗi** public op, cửa sổ nhanh `frame_objects == 0`):

1. **không** Frame nào `mapped_w > 0 && mapped_x > 0`;
2. `mapped_w + mapped_x ≤ maps` (suy ra từng bộ đếm `≤ maps`; `u32` nên không âm).

`wx_recount` (**cổng**, đếm lại **độc lập** từ cây PageTable của **mọi** root — kể cả PageTable
thuần, không chỉ root VSpace) và so với `FrameMeta`; bắt: bảng bị trỏ nhiều lần/mồ côi, lá trỏ
Frame chết, bộ đếm khác số lá, Frame vừa W vừa X. `k2-vm.md` **§10.1** là **hợp đồng lịch sử của
KT-36** (`w ≤ maps && x ≤ maps`, "W^X để KT-39"); **KT-39 nâng lên** `!(w>0 ∧ x>0)` + `w + x ≤ maps`
cho mọi đường — §10 giữ nguyên làm mốc lịch sử, §13 là luật hiện hành.

### 13.6 Cập nhật test cũ (D2-67 — làm chặt, không nới)

Luật mới làm **3 ca KT-36** và **2 ca KT-37** trước đây cố ý map W+X cùng Frame trở nên sai. Đã
cập nhật **kỳ vọng**, giữ nguyên mục đích kiểm và mọi assertion khác:

- KT-36 `ba_co_retype_get_address_va_meta` và `bat_bien_bo_dem_map`: thêm assert `WxViolation` khi
  cố map X trên Frame đang W; `bat_bien_bo_dem_map` kiểm thêm "bị từ chối ⇒ không đổi bộ đếm".
- KT-36 `revoke_bi_tu_choi_khi_con_mapping`: chuyển sang **một** mapping W và thêm assert
  `WxViolation` cho lần map X — mục đích "revoke bị từ chối khi còn mapping" giữ nguyên.
- KT-37 `rights_attrs_theo_mapping_va_chia_se_frame` và property 10k: attrs hợp lệ (một kiểu bộ
  nhớ, `NX`⊥`EXEC`) và kỳ vọng `WxViolation` khi W/X xung đột.

### 13.7 Không làm ở KT-39 (ranh giới)

- **Không** TLB/`TlbFlush`/IPI/ack thế hệ (KT-40) — KT-39 không phát lệnh flush nào.
- **Không** VM fault/`FaultKind::Vm` (KT-41); **không** `.scn`/`check_vm` gộp (KT-42); **không**
  COW (Q7: COW cần pager thật = K3).
- **Không** đổi `spec/` (VSpace/PageTable/Frame vẫn `reserved`), **không** tạo `spec/kernel.toml`.
- **Không** PTE/MMU/TLB/`sfence.vma` thật, không QEMU — **K3**.

## 14. KT-40 — TLB 16 entry/hart, shootdown `TlbFlush`, ack thế hệ, chống deadlock

Nguồn ngữ nghĩa: `review/agent0/K2-PLAN.md` §3 KT-40 + §2 **Q4/Q5**; **D2-65** (Q4/Q5 — nới
D2-59: K2 **thêm đúng một** kind `TlbFlush` vào hộp thư model, `VmKick`/`PowerReq` vẫn K3);
**D2-60** (ack `fetch_max` của `vifast-sync`); thiết kế §4.10 `:353-366` ("mô hình TLB theo
hart": cache `(asid, va) -> (pa, rights)`; shootdown: "sửa PTE, flush cục bộ, gửi
`TlbFlush{asid, va, gen}` vào hộp thư các hart trong mask, IPI, chờ ack"; luật chống deadlock
§4.10.3) và §9.5 `:799`, dòng DoD K2 `:876`.

**K2 không phải kernel thật**: đây là **mô hình** TLB trong sim — không `sfence.vma`, không
IPI/TLB phần cứng, không MMU. "IPI" là một [`MailboxMsg::TlbFlush`] trong hộp thư mô hình;
"ack" là bộ đếm thế hệ trong mô hình. `spec/` **không** đổi; VSpace/PageTable/Frame vẫn
`reserved`.

### 14.1 Hình TLB (`v2/sim/vifast-sim/src/tlb.rs`)

| Thành phần | Giá trị | Nguồn |
|---|---|---|
| `TLB_ENTRIES_PER_HART` | **16** `[TK]` | K2-PLAN §3 KT-40; đổi là đổi ngưỡng mô hình ⇒ cần D2 mới |
| Khoá entry | `(generation, asid, va)` | Q4: `(asid, va)` là danh tính, `gen` để vô hiệu toàn bộ |
| `va` của khoá | **VA gốc** của mapping (`Mapping::va`) | entry 2 MiB phủ 2 MiB; tra cứu/`flush` theo VA **trong** khoảng đều khớp |
| Thay trang | **LRU tất định theo chỉ số** | K2-PLAN KT-40; `used` nhỏ nhất, hoà ⇒ chỉ số nhỏ nhất |
| Bộ đếm | `hits/misses/fills/evictions/flushes/invalidated` + `delivered/acked` | `TlbHartInfo` |

- `hit` ⇔ có entry **cùng `(generation, asid)`** và `va` **trong** `[key.va, key.va + 2^size_bits)`;
  khác danh tính ⇒ **miss** (không dùng nhầm translation của thế hệ/ASID khác).
- `translate(vspace, va)`: hit ⇒ trả ngay (`from_tlb = true`); miss ⇒ walk PageTable (KT-37) →
  **fill** entry (`from_tlb = false`); walk lỗi (chưa map) ⇒ trả nguyên lỗi, **không** nạp entry
  rác. Hart hiện tại phải **đang chạy** VSpace (`active_on`) — Touch chỉ có nghĩa khi hart nạp
  ASID — nếu không ⇒ `IllegalOperation`.
- `tlb_lookup` là tra **không** nạp/không đổi LRU (test/oracle).

### 14.2 Ba phạm vi flush (Q4) — một hàm, ba mức

`tlb_flush_local(hart, vs, scope)` và `HartTlb::flush` xử lý:

| Scope | Xoá entry | Dùng ở |
|---|---|---|
| `Page { va }` | `(gen, asid)` khớp **và** entry **phủ** `va` (kể cả entry lớn chứa `va`) | `unmap`/`protect` (qua shootdown) |
| `Asid` | mọi entry `(gen, asid)` | teardown VSpace; hart rời VSpace (`deactivate`/`activate` sang VSpace khác — flush cục bộ trên chính hart đó) |
| `Generation` | mọi entry có `generation` **cũ hơn** mốc | cạn ASID: `generation += 1` ⇒ **một lần** flush mọi hart (Q4) |

- Teardown VSpace (`finalise_vspace`) và cạn ASID là **broadcast của mô hình**: xoá thẳng TLB
  của **mọi** hart (không qua hộp thư/ack), vì (a) chúng xảy ra bên trong `Revoke`/tạo VSpace,
  không được phép chờ message; (b) teardown là lúc VSpace **không còn mapping** nào (root tinh
  khiết) nên chỉ còn entry "kỷ niệm" — xoá đồng bộ là an toàn và mạnh hơn yêu cầu.
- Hart **offline** (`set_hart_online(h, false)`) bị `clear()` toàn bộ TLB: hart đã tắt không
  thể dịch, và không được nhận shootdown (đã bị bỏ khỏi mọi `active_on`).
- `hart` rời VSpace (deactivate/chuyển VSpace) luôn flush **cục bộ trên hart đó** — mô hình
  "thanh ghi ASID": một hart chỉ nạp một ASID hiện hành. Nhờ vậy `active_on` là **đúng** tập
  hart có thể còn entry của VSpace; oracle ở §14.5 không thấy entry mồ côi.

### 14.3 Shootdown + ack thế hệ (`unmap`/`protect`)

`vspace_unmap` và `vspace_protect` đi theo đúng thứ tự thiết kế §4.10:

1. `active_on` rỗng ⇒ **không gửi gì** (DoD); flush cục bộ + sửa PTE, trả như KT-38/KT-39.
2. `active_on ≠ ∅`:
   - **prepare**: đăng ký pending + **flush cục bộ** hart hiện tại (trước khi sửa PTE để
     `check_tlb` cho phép trạng thái chuyển tiếp trên chính hart đó);
   - sửa PTE (`page_table_unmap`/`page_table_protect`) — lỗi ⇒ xoá pending, trả nguyên lỗi;
   - **post**: giao `TlbFlush { seq, ack, generation, asid, va }` cho **mọi hart khác** trong
     `active_on`, theo hart **tăng dần** (tất định). **Không** IPI cho hart **gọi** — nó đã
     flush cục bộ đồng bộ ở bước prepare và **ack ngầm** (D2-68). `ack` do **kernel** gán trong
     `push_mail`: `ack = delivered` của hart đích sau khi tăng — đúng Q5 "`seq` = số **send Ok**";
   - **wait**: chờ tới `acked_mask == targets`; chỉ khi đó mới trả `Ok`. Quá số vòng an toàn ⇒
     `Busy` (không bao giờ treo) và `check_tlb` sau op bắt entry stale nếu có.

**Deviation D2-68** (nhỏ hơn self-target, để local flush là đường tải **thật**): message
`TlbFlush` **không** gửi cho hart gọi; `acked_mask` để so bất biến được định nghĩa
`acked_mask = {hart gọi nếu hart gọi ∈ active_on} ∪ {hart đích đã ack message}`. Vì vậy bất biến
Q5 `ack_mask == active_on` vẫn giữ **nguyên vẹn**, trong khi hart gọi không tự IPI mình. Đột biến
"bỏ flush cục bộ" (M2, §14.6) vì thế **bị bắt** — không còn đường phụ nào khác xoá entry của
hart gọi.

Bất biến (Q5, dòng DoD "0 vi phạm TLB-safety"): **`unmap`/`protect` trả `Ok` ⇒ TLB của mọi hart
trong `active_on` nhất quán với page table**. `last_tlb_shootdown()` trả ảnh chụp
(`targets`, `acked`, `messages`, `rounds`, `completed`) để test khẳng định; `messages` = số
`TlbFlush` đã giao (**trừ** hart gọi).

Ack dùng đúng ngữ nghĩa **D2-60** (`vifast-sync`), không phải "giao = xong":

- `delivered` = số `TlbFlush` đã giao **thành công** vào hộp thư hart (`= số send Ok`);
- `acked` = `fetch_max` của các `ack` đã xử lý — **đơn điệu, không lùi** (`ack(3)` rồi `ack(1)`
  vẫn 3); mô hình một luồng nên là `if seq > acked { acked = seq }`, **không** atomic/unsafe;
- `acked ≠ delivered`: `delivered - acked` **phải bằng** số `TlbFlush` còn trong hộp thư
  (`check_mailboxes` kiểm đẳng thức này). Hàng FIFO + `ack` cấp tăng dần ⇒ `acked` là **tiền tố
  liên tục**: ack một message ngụ ý mọi message trước nó đã xử lý.

### 14.4 Chống deadlock — hart chờ ack **phải rút hộp thư của mình**

Luật thiết kế §4.10.3: "hart quay chờ khoá hoặc chờ ack phải xử lý hộp thư của mình" — v1 vi
phạm đúng luật này ở lỗ **AP→BSP**. Vòng chờ ack của mô hình (mỗi vòng, `tlb_wait_round`):

1. **rút hộp thư của hart đang chờ** (`drain_mail_fifo(hart)`) — đây là chỗ ack của hart khác
   (và `TlbFlush` của hart khác gửi cho mình) được xử lý;
2. `tlb_shootdown_wait` cho các hart đích **còn thiếu ack** "chạy" handler của họ (mô phỏng IPI
   được phục vụ); `tlb_shootdown_poll` **không** làm hộ — đúng nghĩa "hart đang chờ tự phục vụ";
3. kiểm `acked_mask == targets`; chưa đủ ⇒ vòng kế; quá `MAILBOX_CAP + 2` vòng ⇒ `Busy`.

Hai ca **dương tính**:

- `deadlock_ack_tu_hart_dang_cho` — hart 1 **đang chờ** ack của shootdown riêng; hart 0 `unmap`
  buộc phải vừa trả ack cho hart 1 (rút hộp thư **của mình**) vừa nhận ack từ hart 1 (cho hart 1
  chạy handler), cả hai hoàn tất, không treo.
- `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` — **2 hart chờ nhau** (không self-target: hart 0
  chờ ack hart 1, hart 1 chờ ack hart 0); mỗi hart chỉ tiến khi **tự** rút hộp thư của mình
  (`tlb_shootdown_poll`), không hart nào chạy hộ **cho** hart kia. Vòng 1: hart 1 vẫn chờ; vòng
  2: hart 0 xong; vòng 3: hart 1 xong. Thiếu bước tự phục vụ ⇒ cả hai không bao giờ xong.

**Đột biến** "bỏ xử lý hộp thư khi chờ ack" (xoá `drain_mail_fifo` trong `tlb_wait_round`) bị
bắt bởi `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu`: ack chéo không bao giờ xuất hiện vì mỗi
hart không tự rút hộp thư của mình (không dựa self-target); log
`v2/agent/JOURNAL/kt40-mutation-bo-rut-hop-thu.log`.

### 14.5 Oracle TLB-safety — `check_tlb` / `tlb_violations`

Sau **mỗi** public op (`sched_after_op`, cửa sổ nhanh `tlb_fill_total == 0 && tlb_msg_total == 0`):

1. bất biến rẻ: `entries ≤ 16`, `acked ≤ delivered`;
2. mọi entry còn sống phải khớp **cây PageTable hiện tại**: tồn tại VSpace **sống** cùng
   `(generation, asid)`, `va` còn map, và `pa`/`size_bits`/`frame`/`rights`/`attrs` **bằng**
   mapping hiện tại; entry của VSpace đã chết hoặc VA đã hết map ⇒ vi phạm;
3. **entry chỉ được nằm trên hart thuộc `active_on`** của VSpace đó — hart rời VSpace
   (`deactivate`/đổi VSpace/offline) hoặc VSpace bị teardown phải xoá entry. Entry trên hart
   **ngoài** `active_on` ⇒ vi phạm (đột biến "bỏ flush khi `deactivate`" bị bắt ở đây);
4. entry nằm dưới một shootdown **chưa xong** (pending của bất kỳ hart) được **bỏ qua** — trạng
   thái chuyển tiếp có chủ đích "PTE đã đổi, `TlbFlush` chưa xử lý"; pending xong thì oracle
   **chặt** trở lại.

`tlb_violations()` đếm số entry vi phạm (0 ⇔ TLB-safety đúng). Ca **dương tính** chứng minh
oracle không rỗng: unit test `kobj::tests::check_tlb_bat_entry_mo_coi_va_entry_khong_con_map`
gieo entry của `(gen, asid)` không còn VSpace và entry của VA đã hết map — cả hai bị bắt.

### 14.6 Test, mutation, cổng

`v2/sim/vifast-sim/tests/kt40_tlb.rs` (**10 test**):

| Test | Kiểm |
|---|---|
| `tlb_16_entry_moi_hart_lru_tat_dinh_hit_miss` | 16 entry/hart, hit/miss/fill/evict, LRU tất định, TLB **theo hart** |
| `shootdown_1_2_4_8_hart_ack_du` | shootdown 1/2/4/8 hart: `Ok ⇒ acked == active_on`; entry trang bị unmap mất trên **mọi** hart; `delivered == acked`; dùng entry cũ ⇒ miss/`NotMapped` |
| `ack_thieu_ok_chua_tra` | `begin` chỉ giao message (acked ≠ targets, message còn trong hộp thư); `wait` đủ ack mới `Ok` |
| `flush_theo_trang_asid_va_gen` | flush theo `(asid, va)`, `asid` (kể cả `deactivate`), `gen` (cạn 256 ASID ⇒ thế hệ mới xoá **mọi** entry cũ) |
| `protect_cap_nhat_quyen_tren_moi_hart` | entry quyền **cũ** bị vô hiệu trên mọi hart; dịch lại ra quyền mới |
| `deadlock_ack_tu_hart_dang_cho` | hart đang chờ vẫn trả ack cho hart khác, không treo |
| `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` | 2 hart chờ nhau, **không** self-target: chỉ drain hộp thư của mình mới tiến (AP→BSP) |
| `hart_goi_ngoai_active_on_van_shootdown_dung` | unmap từ hart **ngoài** `active_on`: local flush + chỉ gửi cho hart trong mask, ack đủ |
| `loi_bien_hart_va_vspace` | hart không chạy VSpace ⇒ `IllegalOperation`; hart/VSpace không có thật; `active_on` rỗng ⇒ **không gửi**; begin thứ hai khi đang chờ ⇒ `Busy`; VSpace đã chết ⇒ `InvalidCapability` |
| `property_10k_tlb_shootdown` | 10.000 vòng 4 hart ngẫu nhiên tất định: `check_tlb`/`tlb_violations() == 0` sau mỗi vòng, `acked ≤ delivered`, không pending/message tồn dư |

Unit test trong `src/tlb.rs` (5) + `src/kobj.rs` (1 ca dương tính oracle). Miri: chạy
`--test kt40_tlb` (property tự giảm còn 2 hart × 150 vòng, ca cạn 256 ASID tự bỏ qua bằng
`cfg(miri)` — không `#[ignore]`); `kt40_tlb` nằm trong danh sách `v2 miri`/`nightly --only-miri`.

**Năm đột biến, mỗi ca một chỗ, log thật** (khôi phục bằng bản sao + hash khớp; K2-PLAN KT-40
+ Q5 chặn KT-43):

| Mã | Đột biến | Ca đỏ kỳ vọng | Log |
|---|---|---|---|
| M1 | bỏ `tlb_shootdown_wait` trong `vspace_unmap` (trả `Ok` khi thiếu ack) | `shootdown_1_2_4_8_hart_ack_du`, `hart_goi_ngoai_active_on_van_shootdown_dung`, `deadlock_ack_tu_hart_dang_cho`, `property_10k_tlb_shootdown` | `v2/agent/JOURNAL/kt40-mutation-bo-wait.log` |
| M2 | bỏ **flush cục bộ** trong `tlb_shootdown_prepare` | `protect_cap_nhat_quyen_tren_moi_hart`, `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu`, `shootdown_1_2_4_8_hart_ack_du`, `property_10k_tlb_shootdown` | `v2/agent/JOURNAL/kt40-mutation-bo-flush-cuc-bo.log` |
| M3 | ack **sai seq** (+1 khi ghi `acks` trong `tlb_shootdown_post`) | `ack_thieu_ok_chua_tra`, `shootdown_1_2_4_8_hart_ack_du`, `hart_goi_ngoai_active_on_van_shootdown_dung`, `protect_...`, `deadlock_*`, `property_10k_tlb_shootdown` | `v2/agent/JOURNAL/kt40-mutation-ack-sai-seq.log` |
| M4 | **bỏ rút hộp thư** trong `tlb_wait_round` | `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` (AP→BSP, không self-target) | `v2/agent/JOURNAL/kt40-mutation-bo-rut-hop-thu.log` |
| M5 | bỏ flush khi `vspace_deactivate` | `flush_theo_trang_asid_va_gen`, `property_10k_tlb_shootdown` | `v2/agent/JOURNAL/kt40-mutation-bo-flush-deactivate.log` |

Mỗi ca: mutant **một chỗ**, chạy `cargo test -p vifast-sim --test kt40_tlb`, log đỏ đúng tên ca
ở trên, khôi phục bằng `cp` bản sao + `shasum -a 256` **khớp** trước/sau, chạy lại 10/10 xanh
(xem từng log).

### 14.7 Không làm ở KT-40 (ranh giới)

- **Không** `sfence.vma`/TLB phần cứng/IPI thật/shot IPI; không MMU — **K3**.
- **Không** thêm kind nào khác vào hộp thư: `VmKick`/`PowerReq` vẫn **K3** (D2-65 Q5 chỉ nới
  D2-59 cho đúng `TlbFlush`).
- **Không** VM fault/`FaultKind::Vm` (KT-41); **không** `.scn`/`check_vm` gộp/shared read-only
  (KT-42); **không** 4 đột biến còn lại + 100k×4 (KT-43).
- **Không** đổi `spec/`; `abi.toml` không có kind/mã mới.
- Khoá `usize`→hart, `Vec`/`u64` thường: **không** `unsafe`, **không** atomic/thread/loom —
  mô hình một luồng tất định (D2-49(f)); K3 thay "hộp thư + ack bộ đếm" bằng IPI/SBI thật nhưng
  **giao thức không đổi** (thiết kế §4.10 bước 4).
