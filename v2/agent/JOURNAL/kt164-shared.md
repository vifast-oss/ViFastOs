# KT-164 — K4' bước 1b: đưa trait `PageTableFormat`/`Mmu` lên crate **dùng chung**

**Ngày**: 2026-11-19 · **Agent 2 (L7)** · nhánh `night2/L7-shared` · HEAD trước task: `9a8b8acb`
**Hàng QUEUE**: `KT-164` · **Nợ đóng**: N1 của KT-160 (trait định nghĩa cục bộ trong
`vifast-plat-aarch64`)
**Phạm vi**: chỉ trait + kiểu đi kèm + impl; **không** boot, **không** QEMU, **không** tuyên bố
K4' đạt.

## 0. Việc được giao (nguyên văn, rút gọn)

> **KT-164 — K4' bước 1b: đưa trait `PageTableFormat`/`Mmu` lên crate DÙNG CHUNG.** KT-160 vừa
> thêm `v2/kernel/vifast-plat-aarch64` với trait định nghĩa **cục bộ** (vì `vifast-plat-common`
> không có trait). Thiết kế §1101 nói `PageTableFormat`/`Mmu` **dùng chung từ K2**. Nợ N1 của
> KT-160: trait phải nằm ở crate dùng chung để riscv64 và aarch64 thật sự thay thế được cho nhau.
> Chuyển định nghĩa trait + kiểu đi kèm sang crate dùng chung (ưu tiên `v2/lib/vifast-pte`);
> `vifast-plat-aarch64` **implement** trait chung (không định nghĩa lại); nếu làm được trong ngân
> sách thì cho `PageTableBuilder` (Sv39) implement **cùng** trait và viết **một** test generic
> `fn kiem_tung_dinh_dang<F: PageTableFormat>()` chạy cho **cả hai** định dạng.

## 1. Kết luận trước (đọc nhanh)

- **ĐẠT bước 2–3**: trait `PageTableFormat` + `Mmu` và **toàn bộ** kiểu đi kèm (`Perms`,
  `MemAttr`, `Shareability`, `Descriptor`, `Translation`, `MapError`, `is_canonical_bits`) nằm ở
  `v2/lib/vifast-pte/src/mmu.rs` (crate dùng chung, `no_std`, 0 dependency ngoài,
  `#![forbid(unsafe_code)]`). `vifast-plat-aarch64` **chỉ còn impl** — không còn bản trait thứ
  hai (kiểm bằng `grep -rn "trait PageTableFormat" v2/` ⇒ đúng **một** dòng).
- **ĐẠT bước 4**: `PageTableBuilder` (Sv39, arena) cài **cùng** trait `Mmu`
  (`v2/kernel/vifast-plat-common/src/mmu.rs:559`), và **một** thân test generic
  `kiem_tung_dinh_dang<F: PageTableFormat>()`
  (`v2/kernel/vifast-plat-common/tests/mmu_shared.rs:156`) chạy cho **cả hai** định dạng thật:
  `Aarch64` (4 mức) và `Sv39` (3 mức) — map/translate/quyền user/ca âm/unmap/không rò rỉ frame.
- **ĐẠT đột biến**: (a) đảo `AP[2]` của descriptor AArch64 ⇒ `dinh_dang_aarch64` **ĐỎ**;
  (b) bỏ `PTE_VALID` khỏi `Sv39::encode_leaf` ⇒ `dinh_dang_sv39` **ĐỎ** (§4.2–§4.3). Khôi phục
  ⇒ xanh lại (§4.4).
- **ĐẠT cổng**: `./v2/bin/v2 check` **PASS**, `EXIT=0` (§4.5).
- **ĐẠT L0**: `cargo test -p vifast-plat-common` **118 passed, 0 failed** (115 cũ + 3 test mới).
- **ĐẠT aarch64**: `cargo test -p vifast-plat-aarch64` **22 passed, 0 failed** (không đổi số test,
  chỉ đổi đường dẫn import `mair_index`/`mair_byte`).
- **KHÔNG tuyên bố K4' đạt**: vẫn chưa có `TTBR*`/`MAIR`/`TCR`/`TLBI`/asm/boot (N3 của KT-160).

## 2. Chuyển cái gì, đi đâu

| Trước KT-164 | Sau KT-164 |
|---|---|
| `vifast-plat-aarch64/src/format.rs`: `trait PageTableFormat`, `Perms`, `MemAttr`, `Shareability`, `Descriptor`, `is_canonical_bits` | `v2/lib/vifast-pte/src/mmu.rs` (dùng chung); `format.rs` **re-export** để đường dẫn cũ không đổi |
| `vifast-plat-aarch64/src/mmu.rs`: `trait Mmu`, `MapError`, `Translation` | `v2/lib/vifast-pte/src/mmu.rs`; `mmu.rs` **re-export** |
| `vifast-plat-common/src/mmu.rs`: `enum MapError` (bản riêng) | `pub use vifast_pte::MapError` — **một** kiểu lỗi cho cả hai định dạng |
| — (chưa có) | `vifast-plat-common/src/mmu.rs`: `struct Sv39` + `impl PageTableFormat for Sv39` |
| — (chưa có) | `vifast-plat-common/src/mmu.rs`: `impl Mmu for PageTableBuilder<'_>` |

**Vì sao `vifast-pte` mà không `vifast-plat-common`** (đề bài cho phép chọn): `vifast-pte` đã là
nhà của hình học PTE dùng chung (`PTE_LEVELS`, `VA_BITS`, `PAGE_SIZE_BITS`, `walk_shape`), là
`no_std` **không dependency**, và thiết kế §358/§1101 chỉ định đúng nó. `vifast-plat-common` phụ
thuộc `vifast-core`/`vifast-alloc` (tầng object) — kéo trait xuống đó buộc crate arch kéo cả tầng
object.

**Vì sao `Sv39` ở `vifast-plat-common` mà không `vifast-pte`**: bit PTE Sv39 (`PTE_VALID`,
`PTE_READ`, `PTE_PPN_SHIFT`, `PteFlags`…) đã ở `vifast-plat-common::mmu` (KT-46); đặt `Sv39` cạnh
chúng để **không** chép tay bảng bit lần thứ hai. `vifast-pte` giữ phần *hợp đồng* (trait), crate
arch giữ phần *mã hoá* (impl).

## 3. Thay đổi API đáng chú ý (có lý do, không nới test)

1. **`Perms::is_valid` đổi nghĩa**: trước là "AArch64: `read` bắt buộc"; nay là điều kiện
   **chung** "có ít nhất một quyền R/W/X". Ràng buộc riêng của từng format chuyển vào method mới
   của trait: `PageTableFormat::perms_valid` (mặc định = `Perms::is_valid`).
   - AArch64 override: `perms.read` (không có lá "không đọc") — hành vi **y hệt** trước
     (`map_invalid_perms_rejected` vẫn xanh).
   - Sv39 override: `is_valid() && (!write || read)` (cấm `W=1, R=0`).
   - `PageTable::map_with` gọi `F::perms_valid(perms)` thay vì `perms.is_valid()`.
2. **`MapError` gộp thành hợp**: 12 biến thể = hợp của AArch64 (`OutOfFrames`, `PaOutOfRange`,
   `InvalidPerms`, `NotMapped`, `BlockUnsupported`) và Sv39 (`OutOfArena`, `LevelOutOfRange`,
   `InvalidFlags`). Mỗi format dùng tập con, tài liệu enum nói rõ. Nhờ vậy tầng trên và test
   generic so lỗi mà không phải biết định dạng. Không biến thể nào bị đổi tên ⇒ test cũ của
   `vifast-plat-common` không phải sửa một dòng nào.
3. **`MemAttr::mair_index`/`mair_byte` → hàm tự do** `mair_index(attr)`/`mair_byte(attr)` ở
   `vifast-plat-aarch64::format` (tên `mair_*` là cách AArch64 gọi chỉ số thuộc tính chung; crate
   dùng chung không nên mang tên arch). Test aarch64 đổi 5 chỗ gọi, **không** đổi kỳ vọng.
4. **`vifast-plat-aarch64` thêm dependency nội bộ `vifast-pte`** (path, `no_std`, 0 dependency
   ngoài) — không vi phạm D2-21 (D2-21 nói về crate ngoài); comment Cargo.toml đã cập nhật.

## 4. Bằng chứng (log nguyên văn)

### 4.1. Hai lệnh test bắt buộc

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
running 22 tests
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 ignored; 0 filtered out; finished in 0.00s
```

Tổng L0: **118 passed, 0 failed** (115 trước KT-164 + 3 test mới của `tests/mmu_shared.rs`).

### 4.2. Test generic chạy cho **cả hai** định dạng

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test mmu_shared
running 3 tests
test builder_sv39_implements_mmu ... ok
test dinh_dang_sv39 ... ok
test dinh_dang_aarch64 ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`dinh_dang_aarch64` và `dinh_dang_sv39` gọi **cùng** `kiem_tung_dinh_dang::<F>()`; thân test chỉ
dùng `F::LEVELS`/`F::PAGE_BYTES` của trait, không nhắc AArch64/Sv39. Số frame sau khi map 4 trang
= `F::LEVELS` (AArch64: 4; Sv39: 3) — đúng hình học từng format, không hardcode.

### 4.3. Đột biến (a) — sai một bit descriptor AArch64 ⇒ test generic **ĐỎ**

Sửa `Aarch64::encode_leaf`: `if !perms.write { d |= DESC_AP_RO; }` → `if perms.write { … }`
(đảo `AP[2]`, quyền ghi/đọc ngược):

```text
test builder_sv39_implements_mmu ... ok
test dinh_dang_sv39 ... ok
test dinh_dang_aarch64 ... FAILED
thread 'dinh_dang_aarch64' panicked at vifast-plat-common/tests/mmu_shared.rs:173:9:
assertion `left == right` failed: quyền round-trip
  left: Perms { read: true, write: false, exec: false, user: false }
 right: Perms { read: true, write: true, exec: false, user: false }
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 4.4. Đột biến (b) — sai `PTE_VALID` của Sv39 ⇒ test generic **ĐỎ**

Sửa `Sv39::encode_leaf`: `| PteFlags::leaf(…).bits()` → `| (PteFlags::leaf(…).bits() & !PTE_VALID)`:

```text
test builder_sv39_implements_mmu ... ok
test dinh_dang_sv39 ... FAILED
test dinh_dang_aarch64 ... ok

---- dinh_dang_sv39 stdout ----
thread 'dinh_dang_sv39' (79239876) panicked at vifast-plat-common/tests/mmu_shared.rs:171:48:
translate: NotMapped
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Khôi phục cả hai ⇒ `test result: ok. 3 passed; 0 failed; …` (chạy lại, §4.2).

### 4.5. Cổng nhánh

```text
$ ./v2/bin/v2 check
--- fmt: OK
--- clippy: OK
--- luật cấu trúc: OK
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

**Sự cố đã sửa trong lúc làm**: lần `check` đầu **ĐỎ** ở "luật cấu trúc R1..R5" vì doc comment
của `vifast-pte/src/mmu.rs` và `src/lib.rs` chứa **chuỗi** `` `cfg(target_arch)` `` — luật R2
(`v2/bin/v2:164`) grep thô chuỗi này trong `lib/`. Đã đổi thành "`cfg` theo kiến trúc (luật R2)"
như cách `vifast-pte/src/lib.rs` vốn viết; không sửa luật, không nới ngưỡng.

### 4.6. Kiểm thêm (không bắt buộc, để không phá chỗ khác)

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-pte
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo clippy --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common -p vifast-plat-aarch64 --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.02s
```

`cargo clippy --workspace` trên **toàn** workspace kernel vẫn đỏ ở `vifast-plat-riscv64` (30 lỗi
`invalid register a0/a1/…`) — **có sẵn từ trước**, do crate đó chứa `asm!` RISC-V không biên dịch
được trên host aarch64; cổng không chạy clippy kernel (chỉ `cargo test -p vifast-plat-common`).
Không liên quan KT-164.

## 5. Bất biến của KT-164 (để người sau không phá)

1. **Một** định nghĩa `trait PageTableFormat`/`trait Mmu` trong `v2/` — ở `vifast-pte`. Thêm bản
   thứ hai ở crate arch là phá đúng thứ task này đóng.
2. `vifast-plat-aarch64` **chỉ impl**; `vifast-plat-common` **impl** cho `Sv39`/`PageTableBuilder`.
3. Test generic phải chạy được cho **cả hai** format; nếu ai thêm `cfg`/`if` theo format trong
   thân `kiem_tung_dinh_dang` thì bằng chứng mất giá trị.

## 6. Còn thiếu / nợ có tên

- **N1 (KT-164) — `PageTable<F, M>`/`TableMemory`/`Walk` vẫn ở `vifast-plat-aarch64`**
  (`v2/kernel/vifast-plat-aarch64/src/mmu.rs:42`, `:101`, `:131`). Đây là MMU dữ liệu **generic**
  (đã chạy được với `Sv39` trong test), nhưng chưa chuyển vào `vifast-pte`. Không bắt buộc cho
  KT-164 (trait đã dùng chung + có 2 impl thật ở 2 crate), nhưng nếu K5'/K6 muốn dùng chung thì
  đây là chỗ chuyển — kèm `Walk`/`WalkStep`/`WalkOutcome`/`MAX_LEVELS`.
- **N2 (KT-160) vẫn mở — test crate aarch64 chưa nằm trong cổng**: L0 chỉ chạy
  `cargo test -p vifast-plat-common` (`v2/bin/v2:416`). Từ KT-164, crate aarch64 **được biên dịch**
  trong L0 (dev-dependency) và test generic chạy cho `Aarch64`, nhưng **22 test riêng** của crate
  vẫn không được cổng gọi. Đề xuất lead nối `-p vifast-plat-aarch64` vào L0 (một dòng) — L7
  **không tự sửa cổng**.
- **N3 (KT-160) — chưa có phần cứng**: không `TTBR0_EL1`/`TTBR1_EL1`, không `MAIR_EL1`/`TCR_EL1`,
  không `TLBI`/`dsb`/`isb`, không `SCTLR_EL1.M`, không boot, không QEMU. Bước 2 của K4'.
- **N4 (KT-164) — `PageTableBuilder::unmap` không thu hồi frame**: arena là bump allocator nên
  `unmap` chỉ xoá lá (`v2/kernel/vifast-plat-common/src/mmu.rs:559`); `translate` từ chối
  superpage bằng `BlockUnsupported`. Khác `PageTable` (nguồn frame động) — có chủ đích, đã ghi
  trong doc của impl.
- **N5 (KT-164) — `MapError` là hợp 12 biến thể**: mỗi format dùng tập con; nếu sau này muốn tách
  lại thì phải đổi `Mmu` sang associated `Error`, phá "một kiểu lỗi cho tầng trên" — cân nhắc
  trước khi làm.
- **N6 (KT-164) — dev-dependency `vifast-plat-common` → `vifast-plat-aarch64`**: chỉ để test
  generic thấy cả hai định dạng; ảnh sản phẩm không kéo crate aarch64. Nếu kiến trúc muốn
  "riscv64 không biết aarch64 kể cả trong test" thì chuyển test sang crate thứ ba.

## 7. Việc **không** làm (đúng luật)

- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Không thêm `cfg(target_arch)` vào `vifast-core`/runtime (AGENTS §4.8) — trait là dữ liệu, không
  `cfg`.
- Không nới ngưỡng, không `#[ignore]`/`#[should_panic]`, không retry để lấy xanh.
- Không `pkill -f qemu-system`; task không cần QEMU.
- Không merge, không push, không chạy `gate --fast`, không tạo worktree mới.
- Không sửa `v2/bin/v2` (cổng) dù phát hiện N2 — chỉ ghi đề xuất.

## 8. Lệnh tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test mmu_shared
./v2/bin/v2 check
```
