# Trả lời Agent 0 — 3 câu kỹ thuật (KT-417d / KT-420b / 17 `EP_*`)

**Tác giả**: sub-agent `prep-tech-a` (read-only) · **Cây**: `/Users/mps/dvchd/ViFastOs` `main` @ **`82502f7d`** ·
**Ngày**: 2026-11-28.
**Ràng buộc**: máy đang `mutants --all` ⇒ **0** build / **0** test / **0** QEMU / **0** `v2 gate` / **0** ghi git.
Không sửa tệp repo; chỉ ghi tệp này.
**Nhãn số** (QD §2.2): `[V1:ĐO]` = số đo có log trong repo; `[V1:CODE]` = đếm tĩnh trên mã;
`[ƯL]` = **tính tay theo luật layout `repr(Rust)`**, chưa đo ⇒ mục §4 ghi cách đo.

**Số nền duy nhất được đo**: `size_of::<Machine<StaticStore, TestObserver>>()` = **172.424 B** `[V1:ĐO]`
(`v2/test/boot/lib/budget-common.sh:54`, `v2/kernel/vifast-core/src/kt408_tests.rs:105`,
log `v2/agent/JOURNAL/kt417b-cap-vm/footprint.log:2`). Trần **184.832** (KT-312) / **196.608** (D2-84) **không nới**.

---

## Câu 1 — `VmObjMeta`: bảng riêng hay gộp vào `Meta`?

### 1.1 Hiện trạng (`path:line`)

| Việc | Bằng chứng |
|---|---|
| **Chưa có `VmObjMeta` trong mã** — chỉ là tên trong kế hoạch | `grep -rn VmObjMeta --include=*.rs v2/` = **0**; chỉ có ở `v2/agent/QUEUE.md:479`, `v2/agent/DECISIONS.md:657`, `JOURNAL/kt417d-handoff-con-ho-lai.md:36`, `JOURNAL/kt000-thap-diem-2026-11-26.md:66` |
| Bảng thứ hai hiện tại (phải xoá) | `v2/kernel/vifast-core/src/vm/mod.rs:643` `pub struct Machine<M: PhysMem>`; các bảng ở `:645-650`: `untypeds[4]`, `objects[96]`, `caps[160]`, `mappings[64]`, `flushes[16]` |
| Biên của bảng thứ hai | `vm/mod.rs:85` `MAX_OBJECTS = 96`, `:87` `MAX_CAPS = 160`, `:89` `MAX_UNTYPEDS = 4`, `:91` `MAX_MAPPINGS = 64`, `:96` `MAX_FLUSHES = 16` |
| Kiểu bản ghi | `vm/mod.rs:497` `Object` (16 trường) · `:539` `Cap` · `:560` `Mapping` · `:626` `Flush` — đều `Clone+Copy`, `repr(Rust)` |
| Sổ cái core (đích hợp nhất) | `v2/kernel/vifast-core/src/objmeta.rs:65` `pub struct Meta` (10 trường, **48 B**, `footprint.rs:270` + `kt408_tests.rs:97` + `footprint.rs:181` ghim) |
| `KernelStore` + 2 Store | `store.rs:168` trait (23 kiểu liên kết), `:271` `StaticStore`, `:422` `UntypedStore`, `:433` `type Objects = Table<Meta, GENERAL_MAX_OBJECTS>` (`:381` = 1024) |
| **Trần 96/160/4/64/16 vừa MẤT CHỦ** | `[limits.plat]` (nguồn duy nhất khai 5 số đó, `source = …vm/mod.rs#MAX_*`) **đã bị xoá** ở `2fc15e0e`; `check_kernel.py` chỉ còn `[limits.core]` (`spec/kernel.toml:29-38`) trỏ `machine.rs#MAX_OBJECTS = 256` (`machine.rs:35`); ratchet `E_LIMITS_PLAT` **cấm** section quay lại |
| Thiết kế đã chốt cho lượt kế | `JOURNAL/kt417d-handoff-con-ho-lai.md:36-39` (4 associated type `Vm*`, `StaticStore::Vm = VmTables<…>`), `:47-53` ("`Meta` của core nhận thêm trường VM") ⇒ **hai câu văn mâu thuẫn nhau**, đây chính là chỗ cần chốt |

### 1.2 Phương án (byte, `[ƯL]`)

Thành phần bảng VM bắt buộc phải vào `Machine` (`vm/mod.rs:645-655`): `Mapping` 16 B × 64 = **1.024**,
`Flush` 16 B × 16 = **256**, `Untyped` 248 B × 4 = **992** (`arena.rs:22-33`: `FrameArena` = 128 B
(`v2/lib/vifast-alloc/src/lib.rs:102-111`, `Hole` 8 B × `MAX_HOLES=12`) + 5 trường + `holes[12]`),
bộ đếm `n_untypeds`/`n_flushes`/`next_id`/`next_asid` ≈ **12** ⇒ **≈ 2.284 B** cho mọi phương án.

Chỉ khác nhau ở chỗ đặt **metadata riêng của object VM**. `Object` (36 B, 16 trường) có **6 trường
trùng** với `Meta` (`live`↔`alive`, `kind`↔`ty`, `id`↔`ObjId`, `untyped`, `off`↔`offset`, `len`) ⇒ phần
**thật sự mới** chỉ **20 B**: `pa_off:u32` 4 · `cap:u16` 2 · `root:u16` 2 · `owner:u8` 1 · `size_bits:u8` 1
· `maps/mapped_w/mapped_x/mapped_device/asid:u16` ×5 = 10 → **align 4 ⇒ 20 B** (bỏ `cap` thì 18 B).

| # | Phương án | Thêm | Tổng `Machine` | Headroom vs 184.832 | Đánh đổi |
|---|---|---:|---:|---:|---|
| **A1** | bảng rời **`Object` 36 B × 96** | 3.456 | **178.164** | 6.668 | giữ `Meta == 48`, **nhưng** chỉ 96/256 `ObjId` được là `VSpace/PageTable/Frame`; biên 96 **không còn chủ** |
| **A2** | bảng rời `Object` 36 B × 256 | 9.216 | **183.924** | **908** | đủ 256 object nhưng sát trần, +36.864 B cho `UntypedStore`, 0 lợi ích |
| **A3** | bảng rời **hẹp 20 B × 256** (chỉ trường VM-only, `cap` nằm đây) | 5.120 | **179.828** | 5.004 | đủ 256 object, giữ `Meta == 48`, `Object` **xoá được** (không nhân bản trường) |
| **B1** | gộp vào `Meta`, **flatten** ≤ 18 B ⇒ `Meta` 48→**64** | 4.096 | **178.804** | 6.028 | 1 sổ cái, rẻ nhất, **nhưng** phải bỏ `Object.cap` (quét bảng 512 cap khi object chết) + sửa 3 pin `Meta == 48` + viết lại lý lẽ KT-312 |
| **B2** | gộp, giữ `cap:u16` (hoặc nested struct) ⇒ `Meta` 48→**72** | 6.144 | **180.852** | 3.980 | như B1, đắt hơn A3 1.024 B |

*(5 phương án chênh nhau **5.044 B = 2,7 %** trần ⇒ **byte không phải tiêu chí quyết định**.)*

### 1.3 Khuyến nghị: **A3 — bảng riêng, hẹp, đủ rộng `VmObjMeta[256]` 20 B = 5.120 B**

1. **Loại A1 (96)**: 96 là con số **duy nhất** trong 5 số của `[limits.plat]` không thể tái neo —
   section đã xoá (`2fc15e0e`) và `E_LIMITS_PLAT` cấm thêm lại, `[limits.core]` chỉ biết
   `MAX_OBJECTS = 256`. Giữ bảng 96 ⇒ **biên không chủ, không máy kiểm**: `Meta` cho phép 256 object
   nhưng VM chỉ 96 ⇒ chứng minh `CNode.Revoke` 1 `Untyped` thu hồi 4 loại (mục 5 của `QUEUE.md:479`,
   `KT-417g`) sẽ **chết ở object 97** mà không có bất biến nào báo.
2. **Loại A2**: `Object` 36 B nhân bản 6 trường đã có trong `Meta` (chỉ 20/36 B là thật); 908 B headroom là rủi ro trắng
   cho một lần `Meta`/`Cap` phình sau này.
3. **A3 giữ được lời hứa của `KT-417e`** ("0 chạm `store.rs`/`machine.rs`/`footprint.rs`/`objmeta.rs`",
   `/tmp/briefs/agent0-split-lines.md:93`): 3 pin `Meta == 48` (`footprint.rs:181`, `:270`,
   `kt408_tests.rs:97`) **không đụng**; chỉ 2 pin **tổng** phải tái neo (`footprint.rs:335`,
   `kt408_tests.rs:105`) — mà **mọi** phương án đều phải tái neo 2 pin này.
4. **B (gộp) rẻ hơn A3 1.024 B nhưng đắt hơn ở chỗ khác**: `Meta` là `Table<Meta, 256>` **trong**
   `Machine`, nên gộp **làm `size_of` đổi ngay trong `KT-417e`** ⇒ phá DoD "`size_of::<Machine>()`
   **vẫn 172.424**" của `e` và kéo việc pin sang `e` (đúng thứ `e` được tách ra để tránh). Nếu Agent 0
   vẫn muốn 1 sổ cái thì **B1 phải nằm ở `f`**, không phải `e` — và ghi rõ là `f` sửa 3 pin `Meta` + lý lẽ KT-312.
5. `Object.cap` (dùng ở `vm/mod.rs:1007,1059,1095,1114` để `kill_cap` khi object chết) **nên giữ** ⇒
   nó nằm trong 20 B của A3, không phải trong `Meta`.

### 1.4 Test / pin nào sẽ **đo**

| # | Nơi đo | Nội dung |
|---|---|---|
| 1 | `v2/kernel/vifast-core/src/footprint.rs:32` `FOOTPRINT_PARTS` **26 → 30** + `in_bang_phan_bo_va_pin_d2_84` (`:278-295`) | thêm 4 dòng `vm_mappings[64]` / `vm_flushes[16]` / `vm_untypeds[4]` / `vm_obj_meta[256]`; bất biến `sum + scalars + padding == total` và **`padding ≤ 8`** vẫn phải giữ |
| 2 | **`const _: () = assert!(…)` mới**, khuôn `store.rs:347-356` | `size_of::<VmObjMeta>() == 20`, `size_of::<[VmObjMeta; 256]>() == 5_120`, `size_of::<VmTables<64,16,4,256>>() == 7_404` — const-assert chạy **cả khi build ảnh bare-metal**, không chỉ `cargo test` |
| 3 | `footprint.rs:335` | `total ≤ 184_832 − 12_288` (172.544) → **`total ≤ 184_832`**; giữ nguyên chứng minh thu hồi 12.288 B ở `assert_eq!(size_of::<[Cap;512]>(), 32*512)` |
| 4 | `kt408_tests.rs:105` | `assert_eq!(test, 172_424)` → **số mới đo** (tiền lệ `KT-417b` đã đổi 184.712 → 172.424) |
| 5 | Harness (5 tệp, 8 dòng) | `v2/test/boot/lib/budget-common.sh:54` (+ cặp nhận/từ chối `:303` "≥172.424 nhận; 172.423/0 từ chối"), `v2/test/boot/ktest.sh:608` (`parts=26 sum=172424`), `:621`, `:654`, `:839`, `v2/test/boot/run-aarch64.sh:2447`, `v2/test/boot/run-x86_64.sh:1990` |
| 6 | `v2/kernel/vifast-plat-common/src/untyped_store.rs:44` | `GENERAL_MACHINE_BYTES = size_of::<GeneralMachine>()` **là kiểu dẫn xuất** ⇒ tự theo, không có số tay phải sửa; chỉ cần xác nhận `:187` (`machine_pa + bytes ≤ 0x8000_0000 + 128 MiB`) vẫn đúng (A3 thêm +20.480 B) |

### 1.5 Hệ quả task

- **`KT-417d` (`QUEUE.md:479`, mục 1)**: đổi danh sách kiểu liên kết `VmMappings/VmFlushes/VmUntypeds/VmObjMeta`
  → **`VmTables<MAP,FLUSH,UNT,NT>`** (một kiểu gói) + ghi rõ `VmObjMeta` **hẹp 20 B, `NT = MAX_OBJECTS`**
  (`256`/`1024`), **cấm 96**; thêm vào DoD: "`Meta == 48 B` giữ nguyên; 3 pin `Meta` không đổi".
- **`KT-417e`** (`/tmp/briefs/agent0-split-lines.md:93`): giữ nguyên phạm vi (0 chạm 4 tệp pin) — chỉ cần
  `VmTables` **có tham số `NT`** ngay từ `e` (đừng hard-code 96 rồi `f` phải sửa lại).
- **`KT-417f`**: nơi **duy nhất** tái neo 2 pin tổng + `parts=26` + 8 dòng harness; 0 sửa `Meta`.
- **`KT-417g`** (revoke 4 loại): chứng minh được ở **256** object; nếu chọn A1 thì phải hạ kỳ vọng test
  xuống 96 và **khai** biên đó ở đâu đó — không còn chỗ nào để khai.
- **`KT-420b`**: không ảnh hưởng (khác `spec/`, khác `store.rs`).

---

## Câu 2 — `BootCapLayout` v2: **3** hay **4** trường?

### 2.1 Hiện trạng (`path:line`)

| Việc | Bằng chứng |
|---|---|
| Hợp đồng hiện tại (`boot_abi = 1`, **66 B**) | `spec/abi.toml:504` `[boot_caps]` (`:505 boot_abi`, `:506 none_cspace = 0xFFFF`, `:507 none_proc = 0xFF`); `[boot_caps.cspace]` `:511-534` = **23 leaf** (17 `EP_*` + `self_cspace` + `tcb[5]` + `sc[5]` + 3 `loop_*`); `[boot_caps.proc]` `:536-544` (procsrv/shell + `test.child`/`product.netsrv`) |
| Sinh tự động | `v2/tools/vifast-gen/gen.py:347-353` (3 danh sách thứ tự `*_ORDER`), `:355-364` (tập khoá hợp lệ), `:399` `gen_boot_caps`, **`:406` `die` nếu `boot_abi != 1`**, `:445-458` phát struct, `:461+` phát `BOOT_CAPS_TEST/PRODUCT` |
| Artifact | `v2/lib/vifast-abi/src/generated.rs:484-516` struct **27 trường** (`17+1+5+5+3` u16 = 62 B + 4 u8 = 66 B), `:518` profile |
| Pin bố cục | `v2/lib/vifast-abi/tests/boot_caps_layout.rs:26` `BOOT_ABI == 1`, **`:35` `size_of == 66`**, `:36` `align == 2`, `:42-66` `offset_of!` từng trường, `:70+` 2 profile; tệp tự ghi luật **"đổi bố cục ⇒ BẮT BUỘC bump `boot_abi`"** (`:12-15`) |
| Máy kiểm 1 | `v2/test/abi/check_abi.py:109-131` `BOOT_CAPS_CSPACE` (**21** leaf: 17 `EP_*` + `self_cspace` + 3 `loop_*`), `:133-140` `ARRAYS`(2)+`PROC`(2); `:165` `boot_abi == 1`; `:180-205` **đối chiếu từng leaf ↔ `proc_abi.rs`**; `:211-217` `child`/`netsrv` cùng ô 2; `:223-228` golden needle; `:319-322` ca âm "`boot_abi 1→2` ⇒ đỏ" |
| Máy kiểm 2 | `v2/tools/check-boot-abi.py:29-30` ghi "việc gộp 17 hằng vào `BootCapLayout` là `KT-420`" ⇒ **sẽ sai** sau `KT-420b` |
| Bước cổng | `v2/bin/lib/steps_check.sh:198` `step_gen` = `vifast-gen --check` (golden `generated.rs` + `docs/abi.md`) |
| **Không có người dùng production** | `grep -rn "BOOT_CAPS_\|BootCapLayout" v2/kernel v2/user` ⇒ **chỉ** `v2/kernel/vifast-core/src/invoke417_tests.rs:140-154`. Kernel dựng cspace bằng `proc_abi.rs` (**140** tham chiếu / **22** tệp) ⇒ đổi layout **không** đổi hành vi boot |
| Lý do tồn tại 2 profile | `review/agent0/GOI-ABI-KIEN-TRUC.md:341-346` (§B.2.3): **`SLOT_CHILD` và `SLOT_NETSRV` cùng số 2 nhưng khác ảnh** ⇒ buộc 2 giá trị profile |

### 2.2 Phương án

- **3 trường** (`SELF_TCB`, `SELF_CSPACE`, `NS_EP`): `#[repr(C)]` = 3 × `u16` = **6 B**, `align 2`.
  Mỗi tiến trình nhận cap **của chính nó** (TCB/CNode self) + **một** endpoint tra cứu ⇒ bố cục
  **độc lập ảnh** ⇒ `[boot_caps.proc.test]`/`[boot_caps.proc.product]` và cả `[boot_caps.proc]`
  **biến mất**, cùng với lý do duy nhất khiến phải có 2 profile. `none_proc` (`0xFF`) thành **chết**
  (không còn không gian ô chương trình trong ABI) ⇒ xoá luôn.
- **4 trường** (+1 × `u16` = **8 B**): **2 B chênh lệch ⇒ byte không phải tiêu chí**; quyết định thuần ngữ nghĩa.

### 2.3 Trường thứ 4 **không** hoạt động như "cap trang" — đây là lý do chính để chọn 3

`JOURNAL/kt000-thap-diem-2026-11-26.md:67` nêu ứng viên `SELF_UNTYPED`/`SELF_FRAME`. Nhưng:

1. `Frame.Map` đòi **cap `VSpace` ở MR0 có `RIGHT_WRITE`** (`spec/abi.toml:290-294`). Một cap trang
   duy nhất **không** đủ: `Frame` là lá, **không** retype được; `Untyped` retype được `Frame` nhưng
   tiến trình **không có cap `VSpace` của chính nó** để map ⇒ cấp trang xong vẫn vô dụng.
2. Muốn `PageSource` chạy thật ở U-mode phải thêm **2** cap (`SELF_UNTYPED` + `SELF_VSPACE`) ⇒ không
   còn là "3 ± 1" nữa, và mỗi cap thêm là một dòng `[boot_caps]` + `offset_of` + leaf checker.
3. Đường đã có sẵn trong ABI **không cần** cap trang lúc boot: `Tcb.Exec(tcb, elf_frame, vspace, entry_arg?)`
   (`spec/abi.toml:230`, `QUEUE.md:487`) cho **cha** sở hữu không gian địa chỉ của con; cha cài cap
   bằng `CNode.Insert`. Con **không** cần cap trang lúc boot.
4. `SELF_FRAME` còn tệ hơn: một cap `Frame` **chốt cỡ/chỗ** một trang vào hợp đồng boot ⇒ sinh lại
   đúng defect "profile phụ thuộc ảnh" mà `KT-420b` đang xoá.

### 2.4 Khuyến nghị: **3 trường, `boot_abi = 2`, 1 profile, 6 B**

```toml
[boot_caps]
boot_abi = 2
none_cspace = 0xFFFF                     # giữ: sentinel "vắng" cho slot cap
source = "review/agent0/GOI-ABI-KIEN-TRUC.md#text:Phần B.2"
[boot_caps.self]                          # 3 leaf u16 — KHÔNG còn [boot_caps.cspace]/[boot_caps.proc]
self_tcb     = { value = …, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#TCB_SLOT_SELF" }   # [TK] tên hằng do KT-420b đặt
self_cspace  = { value = …, source = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs#SLOT_CSPACE" }
ns_ep        = { value = …, source = "v2/user/namesrv/lib.rs#NS_EP" }                                    # [TK]
```

- **Không** thêm trường thứ 4. Thay vào đó ghi **tripwire**: khi `vifast-rt` cần cap trang lúc boot
  thật, đó là **bump `boot_abi` 2 → 3** (đúng luật của chính `boot_caps_layout.rs:12-15`) + lý do ghi
  trong test — không nhét trước một trường "để dành".
- Nếu Agent 0 vẫn muốn 4: **chỉ** `SELF_UNTYPED` (+ leaf ngân sách byte), **không bao giờ** `SELF_FRAME`;
  và phải ghi rõ nó chưa dùng được cho tới khi có `SELF_VSPACE` ⇒ khuyến nghị dứt khoát là **không**.

**Việc `KT-420b` phải sửa (đều trong cửa sổ `spec/abi.toml` của chính nó)**: `spec/abi.toml:504-544`;
`gen.py` (`:347-353` gộp 3 danh sách → 1, `:355-364` tập khoá, **`:406`** nhận `2`, `:445-458`, `:461+`
→ **một** const profile); sinh lại `generated.rs` + `docs/abi.md` (`step_gen`, `steps_check.sh:198`);
`boot_caps_layout.rs` (`:26` `BOOT_ABI == 2`, **`:35` 66 → 6**, `:36` align 2, `:42-66` offset `0/2/4`,
`:70+` → 1 profile); `check_abi.py` (`:109-140` bảng mới, `:165` `== 2`, bỏ `:180-205` + `:211-217`,
`:223-228` needle, **`:319-322` ca âm `2→3`**, đếm selftest trong `--selftest`); `check-boot-abi.py:29-30`
(đoạn văn "gộp 17 hằng vào `BootCapLayout`"); `invoke417_tests.rs:140-154` viết lại theo 3 trường.

### 2.5 Hệ quả task

- **`KT-420b` mục (1)** trở nên **đo được**: DoD = `size_of::<BootCapLayout>() == 6`, `BOOT_ABI == 2`,
  **1** profile, `[boot_caps]` **0** `EP_*`; và **không** kéo theo `v2/user/` (xem Câu 3).
- **`KT-419b` (`IOSpace`)**: `JOURNAL/…/w8c-kt419b.md:208` cảnh báo "nếu `IOSpace` cần cap lúc boot thì
  tối giản 3 trường sai". Đọc `spec/abi.toml:246` (`IOSpace install_at = "D3"`) + §3 RFC: `IOSpace` do
  **driver tạo lúc chạy** bằng `Untyped.Retype`, không phải cap boot ⇒ **3 trường vẫn đúng**; nhưng
  brief `KT-419b` phải **chốt** câu này bằng một dòng, đừng để nó quay lại `KT-420b`.
- **`KT-421`**: không liên quan layout boot.

---

## Câu 3 — 17 `EP_*`: phương án **(a)** hay **(b)**?

### 3.1 Hiện trạng (`path:line`) — quy mô thật

| Việc | Bằng chứng `[V1:CODE]` |
|---|---|
| Nguồn kernel | `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs:31,33,35,37,39,41,43,81,141,143,145,147,149,172,175,179,181` (17 `EP_*`) + 8 `SLOT_*`/`LOOP_*` + 2 mảng `:49,51` |
| **Lan toả kernel** | **140** tham chiếu `EP_*` trong **22** tệp `.rs`: `proc_net.rs` 34, `proc_abi.rs` 20, `proc_rt.rs` 15, `ktest_netsrv.rs` 13, `proc_exec.rs` 11, `loop_svc.rs` 10, `mem_svc.rs` 7, `kbin-aarch64/src/k5user.rs` 7, `proc_demo.rs` 4, `netcli_user.rs` 3, `shellpath.rs`(x64) 2, `net_svc.rs` 2, `netsrv_user.rs` 2, `pshell.rs` 2, `syscall_env.rs`(x64) 1, `k5cap.rs`(a64) 1, `build.rs`(x64) 1, `proc_fs.rs` 1, `ktest_user.rs` 1, `capa_client_user.rs` 1, `nb_queue_tests.rs` 1, `core_tcb.rs` 1 |
| Bản sao U-mode | **17** `const EP_*` trong **4** tệp: `v2/user/programs/procsrv.rs:27-47` (6), `shell.rs:69-94` (8), `shell_cmds.rs:255,257` (2), `echo.rs:23` (1); **67** tham chiếu |
| Cspace thứ hai | `proc_net.rs` 5 `SLOT_EP_*` → `v2/user/programs/netsrv_prod.rs` (rule 7 của checker) |
| Sim | `v2/sim/vifast-sim/src/procsrv_model.rs:69,82` (+ `tests/kt23_k1b.rs`, `tests/kt65_procsrv.rs`) |
| Máy kiểm hiện có | `v2/tools/check-boot-abi.py` (`EP_NAMES` 17 `:87-105`, `SLOT_LOOP_NAMES` 8, `ARRAY_NAMES` 2, `CSPACE_SLOTS` `:126`, `EXPECTED_COPIES` 13/16 bản sao `:132-146`, `USER_ONLY_KNOWN` `:152`, bất biến slot `< MAX_CAPS = 512` `:83`+`:446-452`, **18 ca selftest**); `v2/test/abi/check_abi.py:109-131` đối chiếu **spec ↔ `proc_abi.rs`** |
| Ghi chú sai trong mã | `proc_abi.rs:13-15` nói ràng buộc được kiểm bằng `kt77b-harness-scan.sh` §slot — **tệp đó không có mục `slot`** (§B.1 #8, `GOI-ABI-KIEN-TRUC.md:282`); máy kiểm thật là `check-boot-abi.py` (`KT-404c`, `DONE bdf5c61`) |
| 6/17 hằng sẽ chết ở `KT-421` | `QUEUE.md:488` xoá `net_svc.rs`, `proc_net.rs`, `virtio_net.rs`, `tcp.rs` ⇒ `EP_NET`(9), `EP_NET_REPLY`(15), `EP_NET_KERN`(16), `EP_NET_KERN_REPLY`(17), `EP_NET_CLIENT`(26), `EP_NET_CLIENT_REPLY`(27) + 5 `SLOT_EP_*` |
| Khuyến nghị A/B cũ | `GOI-ABI-KIEN-TRUC.md:548-571` (§B.3 nhóm 8) chọn **A** cho `KT-417`: "2 bảng con TOML → 1 struct phẳng" — **không** nói gì tới việc xoá 17 hằng; `KT-420b` là chỗ đặt câu hỏi đó |

### 3.2 Hai phương án

- **(a)** `[boot_caps]` v2 **không** mang 17 `EP_*`; `proc_abi.rs` **giữ** chúng như **legacy nội bộ kernel**,
  vẫn được `check-boot-abi.py` canh (kernel ↔ U-mode), **kèm ratchet đi xuống**; xoá dần theo đúng
  service rời kernel (`KT-421`: 6 hằng `EP_NET*`; `KT-422`: `EP_FS`; `KT-423`: `EP_MEM`/`EP_LOOP`/`EP_CTL`…)
  và dọn sạch ở `KT-424`.
- **(b)** Xoá **thật** cả 17 ngay trong `KT-420b` (kèm task mới), đúng chữ "gỡ sạch 17 hằng `EP_*`
  hardcode rải rác giữa kernel & userland" của `QUEUE.md:487`.

### 3.3 Khuyến nghị: **(a)** — và ghi lý do vào `QUEUE.md`/`D2`

1. **Chưa có cơ chế thay thế ở `KT-420b`.** `namesrv` mới chỉ vừa có `NS_EP` + vòng `Recv/Reply`
   (`v2/user/namesrv/lib.rs:9-24`; `KT-420b` phụ thuộc `KT-420` — `QUEUE.md:487`). Thay 140 điểm gọi
   bằng tra cứu tên **chính là thân Chặng 3** (`KT-421..423`), không phải việc 3 h. Làm (b) → hoặc ảnh
   sản phẩm gãy, hoặc "xoá hằng" nhưng caller vẫn trỏ số cứng ⇒ **spec nói một đằng, mã làm một nẻo** —
   đúng thứ `AGENTS.md` §1 cấm.
2. **Chúng chết cùng chủ.** (b) làm `KT-421` phải sửa lại đúng những dòng vừa sửa (nó **xoá cả**
   `proc_net.rs`), tức trả tiền hai lần cho cùng một diff.
3. **(a) giữ được máy kiểm; (b) tự tay tháo máy kiểm.** `check-boot-abi.py` (18 ca) tồn tại **chỉ** để
   canh 17 hằng + 16 bản sao. (b) ⇒ xoá chủ thể và máy kiểm trong cùng commit ⇒ từ `KT-420b` tới
   `KT-423` **không còn gì canh** kernel ↔ U-mode (bước 26 "test có răng" mất đối tượng).
4. **Hợp đồng boot nói về cái boot CẤP, không nói về cái kernel còn tham chiếu.** 3 trường + legacy
   nội bộ là nhất quán: ABI = 3 cap; `proc_abi` = chi tiết cài đặt sẽ biến mất cùng service.
5. Nếu Agent 0 chọn (b) thì **phải** tách task mới (≈10 h `[ƯL]`: 22 tệp kernel + 4 tệp U-mode + sim +
   2 máy kiểm), và `KT-420b` mất tính "3 h" ⇒ khuyến nghị dứt khoát: **không**.

### 3.4 Việc cụ thể của (a) — gói trong `KT-420b`, ≈45 phút

| Tệp | Sửa |
|---|---|
| `spec/abi.toml` | bảng v2 (Câu 2) — `source` **không** còn trỏ `proc_abi.rs#EP_*` |
| `v2/test/abi/check_abi.py` | thu `BOOT_CAPS_CSPACE`/`ARRAYS`/`PROC` (`:109-140`) về đúng bảng v2; bỏ vòng đối chiếu 17 hằng (`:180-205`) + `:211-217`; ca âm `:319-322` đổi `boot_abi 2→3` |
| `v2/tools/check-boot-abi.py` | sửa đoạn đầu (`:8-36`, nhất là `:29-30`) thành: "17 `EP_*` là **legacy nội bộ kernel**, **không** thuộc `[boot_caps]`; máy kiểm này canh **bản sao kernel ↔ U-mode**"; thêm **ratchet sunset**: `len(EP_NAMES) ≤ 17`, tập hiện tại là **baseline**, `KT-421/422/423` **bắt buộc** hạ số và ghi trong commit xoá |
| `proc_abi.rs` | khối 17 hằng thêm dòng `// LEGACY (KT-420b): không còn trong [boot_caps]; xoá theo KT-421..423` — **0 đổi giá trị** ⇒ **0 đổi TAP**, 0 đổi `expected.txt` |
| `v2/user/**` | **0 sửa** ⇒ `EXPECTED_COPIES`/`USER_ONLY_KNOWN` giữ nguyên (nợ `KT-404c` "cập nhật khi `KT-420b` thay 17 `EP_*`" **đóng bằng 0 thay đổi**, ghi rõ trong journal) |

### 3.5 Hệ quả task

- **`KT-420b`**: mục (1) còn `spec/abi.toml` + `gen.py` + `generated.rs`/`docs/abi.md` + `boot_caps_layout.rs`
  + 2 máy kiểm + `invoke417_tests.rs` ⇒ vừa 3 h. **0 chạm** `v2/user/`, `v2/sim/`, `proc_rt/proc_exec/loop_svc/mem_svc`.
- **`KT-421`**: DoD phải nói thêm "xoá 6 `EP_NET*` + 5 `SLOT_EP_*` + hạ ratchet `EP_NAMES` 17 → 11"
  (hiện `QUEUE.md:488` **im lặng** về hằng số ⇒ dễ để rơi).
- **`KT-422`**: `EP_FS` (7) → 10. **`KT-423`**: `EP_MEM`(21)/`EP_LOOP`(22) → 8.
- **`KT-424`** (kiểm toán chốt chặng): điều kiện "0 hằng `EP_*` legacy" + xoá hẳn khối trong `proc_abi.rs`
  và `EP_NAMES`/`EXPECTED_COPIES` của `check-boot-abi.py` ⇒ đóng nợ `KT-404c`.
- Nếu (b): `KT-420b` **không** còn 3 h; và `KT-421` phải chạy lại phần vừa xoá.

---

## 4. Chưa kiểm được vì read-only (bắt buộc đọc trước khi dùng số)

1. **Mọi cỡ `size_of` trong tài liệu này là `[ƯL]` tính tay** theo luật layout `repr(Rust)` (thứ tự
   trường được phép hoán vị, `align` = max align trường): `Meta` 64/72 · `VmObjMeta` 20 · `Object` 36 ·
   `Mapping`/`Flush` 16 · `Untyped` 248 · `FrameArena` 128 · `Hole` 8. **Không** chạy
   `cargo test -p vifast-core footprint -- --nocapture` (lệnh này in bảng 26 dòng + `scalars`/`padding`)
   vì máy đang `mutants --all`. **Chỉ 172.424 B là `[V1:ĐO]`** (log `JOURNAL/kt417b-cap-vm/footprint.log`).
   → Cách đo: `cargo test -p vifast-core footprint -- --nocapture` (hoặc `./v2/bin/v2 t0 vifast-core`),
   đọc dòng `# kt405 abi: … Meta=… Machine=…` (`footprint.rs:245-260`) và bảng `in_bang_phan_bo_va_pin_d2_84`;
   thêm `println!` cho `VmObjMeta`/`VmTables` **trước** khi chốt pin.
2. **Chưa chạy** `vifast-gen --check`, `check_abi.py --selftest`, `check-boot-abi.py --selftest` ⇒
   **chưa chứng minh** bản sửa spec/checker ở §2.4/§3.4 sẽ xanh. Con số "18 ca" đọc từ
   `check-boot-abi.py:41,872` và "19/19" từ `QUEUE.md:477` là **đọc mã**, không phải chạy.
3. **Chưa kiểm `padding ≤ 8`** của bảng footprint 30 dòng sau khi thêm 4 dòng VM (bất biến
   `footprint.rs:292-295`) — chỉ đo được bằng test.
4. **Chưa xác nhận `Untyped.Retype` chạy thật từ U-mode** (chỉ đọc whitelist `invoke.rs:1331-1352` +
   `spec/abi.toml:228`); lập luận "cần thêm 2 cap" ở §2.3 dựa trên **chữ ký** `Frame.Map`
   (`spec/abi.toml:290-294`), chưa dựng ca chạy.
5. **Chưa xác nhận** `Machine<UntypedStore>` vẫn vừa `BootArena` sau +20.480 B (A3): hằng
   `GENERAL_MACHINE_BYTES` là kiểu dẫn xuất (`untyped_store.rs:44`) nên **không** có pin tay phải sửa,
   nhưng điều kiện `:187` (`≤ 0x8000_0000 + 128 MiB`) chỉ kiểm được khi chạy test `vifast-plat-common`.
6. **Chưa đo thời gian** của 4 hạng mục `KT-417e/f/g/h`; con số 8–11 h / 10,5 h là **`[ƯL]` từ brief cũ**
   (`JOURNAL/kt417d-vm-store.md:126`, `/tmp/briefs/approval-packet-kt417e.md`), không phải số đo.
7. **Không** chạy `git`/`v2 gate`/QEMU/build; **không** sửa tệp nào trong repo. Tệp duy nhất được ghi:
   `/tmp/briefs/agent0-ky-thuat-answers.md` (tệp này).
