# KT-37 — `PageTable` 3 cấp: map/unmap + rights/attrs + 7 mã lỗi (K2)

**Vào**: READY (`agentctl next --project v2` trả KT-37) · HEAD `eaf30c1` ([KT-00] lật KT-36 DONE,
494 test, Miri 12+5) · `git rev-list --count HEAD..origin/main` = **0** · working tree có
`viscript/` của tab khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 494
test/66 suite**, L1 1.000 seed **985 ms**
([`kt37-baseline-gate-fast.log`](kt37-baseline-gate-fast.log)).

**Nguồn ngữ nghĩa**: `review/agent0/K2-PLAN.md` §1.1 mục 3 + §2 Q2/Q3/Q7 + §3 KT-37; **D2-65**
(chốt Q1–Q10), **D2-66** (Frame/PA window); `v2/docs/k2-vm.md` §2/§3/§4/§10 (hằng số + Frame);
crate `vifast-pte` (`FrameSize`, `Pte`, `PteArray`, `walk_shape`, `VA_BITS`, `MAX_MAP_DEPTH`,
`MAX_FRAMES_PER_VSPACE` — **đọc từ crate, không chép giá trị**); `v2/sim/vifast-sim/src/kobj.rs`
(`ObjType::PageTable` reserved 10, `KError` 19 mã, retype/revoke/finalise KT-09..KT-36);
`spec/abi.toml` `PageTable id=10` `reserved`/`unsupported`, method 0 `Map`, 1 `Unmap`; v1
`crates/vifast-kernel/src/lib.rs:3102` (`map_frame`: "intermediate page tables must already
exist; otherwise `MissingPageTable`") + `:2924` (`map_page_table`).

**Không** làm (đúng brief): không VSpace/ASID/TLB (KT-38/40), không W^X/protect (KT-39), không
VM fault (KT-41), không `.scn`/`check_vm` (KT-42), không `spec/`, `vifast-model`, `.scn`,
QUEUE/DECISIONS/AGENTS, `viscript/`; không `unsafe`, không dependency ngoài workspace, không
`#[ignore]`/`#[should_panic]`, không sửa test cũ.

## 0. Kế hoạch nhỏ (8 bước, ghi vào STATE)

1. Baseline `gate --fast` + đọc nguồn. 2. Test đỏ **ngữ nghĩa** (API cũ). 3. Test đỏ **biên dịch**
(API mới). 4. Code `pagetable.rs` + `kobj.rs`. 5. Test xanh + unit test âm `check_pages`.
6. `check` → `gate --fast`. 7. Miri `kt37_pagetable` + lib. 8. Review độc lập → commit
`[KT-37]` → push → STATE/JOURNAL.

## 1. Test đỏ trước (D2-05)

- [`kt37-red-semantics.log`](kt37-red-semantics.log) — test tạm chỉ dùng API **cũ**:
  `Retype(Untyped, PageTable)` trả `Err(NotSupported)` (kỳ vọng `Ok`) ⇒ **0 passed / 1 failed**;
  test tạm đã xoá sau khi ghi log.
- [`kt37-red-compile.log`](kt37-red-compile.log) — `tests/kt37_pagetable.rs` (10 test + property)
  viết trước khi có code: **E0432** `no MapAttrs/MapRights/Mapping in kobj` + **E0599** cho
  `check_pages`, `page_table_attach/detach/map/map_obj/unmap/walk/info/count/pte_raw` ⇒
  `could not compile`, exit 101.

## 2. Thay đổi (chỉ `v2/`)

| Tệp | Nội dung |
|---|---|
| `v2/sim/vifast-sim/src/pagetable.rs` **(mới)** | `MapRights` (READ/WRITE), `MapAttrs` (WB/WC/UC/Device/NX/EXEC), `Mapping`, `PageTableInfo`, `PageTableData` (level/attached/`PteArray<512>`/mapped), `level_for_size_bits`, `size_bits_for_level`, `table_span`, `index_in_level`, layout PTE mô hình (`leaf_pte`/`table_pte`/`decode_pte`); 4 unit test (hằng số/hình, roundtrip mọi tổ hợp rights×attrs×4 ObjId, bit/contains, dữ liệu mới) |
| `v2/sim/vifast-sim/src/kobj.rs` | `Kind::PageTable` (mốc body), `Meta.pt`/`ChoDat.pt`, `page_tables: Vec<Option<PageTableData>>` + `page_table_objects` (cửa sổ nhanh); `retype` rẽ nhánh `PageTable` → `retype_page_table` (4096 B arena, `size_bits ∈ {0,12}`, atomic); `page_table_map` (qua cap), `page_table_map_obj`, `page_table_unmap`, `page_table_walk`, `page_table_attach`, `page_table_detach`, `page_table_count`, `page_table_info`, `page_table_pte_raw`; `check_pages` (**5 nhóm** bất biến) chạy trong `sched_after_op`; `finalise_page_table` (tinh khiết, nếu không ⇒ `HasChildren`); `revoke_children` kiểm PageTable trước mọi thay đổi |
| `v2/sim/vifast-sim/tests/kt37_pagetable.rs` **(mới)** | 10 test: retype/size_bits; map/walk/unmap 3 cỡ; **bảng chân trị 7 mã lỗi × 3 cỡ**; attach/detach + `HasChildren`; rights/attrs lưu theo mapping + cùng Frame 2 root; unmap đúng gốc/`Alignment`; **property 10.000 op** (walk ngược 100% + bộ đếm + `check_pages` sau mỗi op); không rò PageTable/Frame (100×10 vòng); lỗi cap; layout PTE |
| `v2/sim/vifast-sim/src/kobj.rs` (unit test) | 4 test **âm**: `check_pages` bắt `mapped` sai, bộ đếm Frame sai, `attached` sai, PageTable chết còn dữ liệu, PTE trỏ bảng chưa attach; map với `pa` lệch 1 ⇒ `Alignment` |
| `v2/sim/vifast-sim/src/lib.rs` | `pub mod pagetable;` |
| `v2/docs/k2-vm.md` | **§11** (mới): object/cây 3 cấp, layout PTE mô hình, API, thứ tự kiểm + bảng 7 mã lỗi, 5 nhóm bất biến `check_pages`, ranh giới không làm |
| `v2/agent/JOURNAL/kt37-*`, `STATE.md` | log đỏ/baseline/check/gate/miri + trạng thái |

## 3. Quyết định có thể gây tranh luận (ghi rõ thay vì ẩn)

1. **`map`/`unmap`/`walk` chỉ nhận bảng gốc.** Một PageTable mới tạo có `level = 2`, chưa
   attach ⇒ là root; bảng đã attach (cấp 1/0) truyền vào `map` ⇒ `MissingPageTable` ("bảng gốc
   cấp trên không nằm trong lời gọi"). Nhờ vậy **cả 3 cỡ** đều có ca `MissingPageTable`: 4 KiB
   thiếu bảng cấp 0/1, 2 MiB thiếu bảng cấp 1, 1 GiB truyền bảng cấp 1 làm root. Không có cách
   nào khác để 1 GiB "thiếu bảng cấp trên" vì lá 1 GiB nằm ngay root — ghi rõ trong `k2-vm.md`
   §11.3.
2. **PageTable tiêu 4096 B arena, Frame thì không.** Bảng thật là một trang PTE; PageTable của
   mô hình cấp 4 KiB từ arena Untyped (khác Frame — `pa` ảo không tiêu byte). Nhờ vậy
   `check_bytes` vẫn có nghĩa và `NotEnoughMemory` xảy ra thật; `check_pages` kiểm
   `len == 4096`.
3. **PTE mô hình trỏ **ObjId**, không mang `pa`.** `vifast_pte::Pte` cố ý thô (KT-35); KT-37
   chốt layout bit (valid/table/rights/attrs/ObjId). `pa` đọc từ `FrameMeta` lúc walk. PTE thật
   (mang PPN) là K3; ghi rõ trong `k2-vm.md` §11.2 để K3 không đọc nhầm.
4. **Rights/attrs là tham số, không suy từ cap.** `map` qua cap chỉ kiểm cap sống + đúng
   `ObjType::Frame`; rights/attrs lấy đúng tham số (test: cap `RIGHTS_ALL`, mapping `WRITE` +
   `UC|EXEC` vẫn giữ `WRITE`/`UC|EXEC`). KT-37 **không** áp W^X/kiểu bộ nhớ — KT-39.
5. **`unmap` chỉ nhận `va` đúng gốc mapping**, lệch ⇒ `Alignment`; không xoá ngầm trang lớn
   (2 MiB/1 GiB) khi người gọi chỉ định một địa chỉ bên trong. Walk thì vẫn trả mapping phủ `va`
   (đúng ngữ nghĩa MMU) — hai hành vi khác nhau có chủ đích.
6. **`HasChildren` = "không im lặng bỏ cây/mapping".** `page_table_detach` bảng còn entry/lá ⇒
   `HasChildren`; `Revoke` Untyped có PageTable chưa tinh khiết (còn attach/PTE/`mapped`) ⇒
   `HasChildren`, **trước** mọi thay đổi (atomic). Muốn xoá: unmap lá → detach con → revoke.
7. **`check_pages` chạy sau mỗi public op** qua `sched_after_op`, có cửa sổ nhanh
   `page_table_objects == 0` (giống `frame_objects` của KT-36) để L1/test K1a..K1c không hồi quy.
   Nó không chỉ đếm: unit test âm chứng minh bắt được `mapped` sai, bộ đếm Frame sai,
   `attached` sai, bảng chết còn dữ liệu, PTE trỏ bảng chưa attach, `pa` lệch căn.

## 4. Cổng và bằng chứng

- [`kt37-check.log`](kt37-check.log) — `./v2/bin/v2 check` PASS (fmt/clippy `-D warnings`, luật
  R1..R5, ABI/scn/gen/golden/report).
- [`kt37-gate-fast.log`](kt37-gate-fast.log) — `./v2/bin/v2 gate --fast` **PASS 512 test/67 suite**
  (baseline 494/66 + 10 `kt37_pagetable` + 4 unit `pagetable` + 4 unit KT-37), L1 1.000 seed
  **726 ms**, không retry (D2-07).
- [`kt37-miri.log`](kt37-miri.log) — Miri **PASS**: `--test kt37_pagetable` **10/10** (171,54 s,
  exit 0) + `--lib pagetable` **4/4** (1,09 s) + `--lib 'kobj::tests::check_pages'` **3/3**
  (12,86 s); dưới `cfg(miri)` giảm property 10.000 → **20** op, vòng rò 100 → **1×2** (stable giữ
  nguyên 10.000/100 — D2-47(a)); 2 warning `integer-to-pointer cast` là của `vifast-objmem`
  (`obj.rs:166/175`, D2-17), **không** phải KT-37; `v2/sim/**` **0 unsafe**.
- Test xanh: `cargo test -p vifast-sim --test kt37_pagetable` **10/10**; lib **47/47** (39 cũ +
  **4** unit `pagetable` + **4** unit KT-37: 3 âm `check_pages` + 1 `pa` lệch căn ⇒ `Alignment`).

## 5. Không đổi

`spec/` (`PageTable` vẫn `reserved`/`unsupported`), `v2/lib/vifast-model` (oracle), `.scn`,
QUEUE/DECISIONS/AGENTS, `viscript/`, `v2/bin/v2`; không `unsafe`, không dependency ngoài
workspace, không `#[ignore]`/`#[should_panic]`, không nới ngưỡng/seed nào của cổng.
