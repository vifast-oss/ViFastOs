# K2-PLAN — Bộ nhớ ảo (model-only): Frame / PageTable / VSpace / ASID / TLB

Trạng thái: **REVIEW** (planner K2 soạn cho Agent 0; chưa phải QUEUE, chưa duyệt)
Ngày: 2026-11-14 · Người soạn: planner K2 (`planner-k2b`) · Phạm vi: **chỉ vòng K2**
Tệp được phép ghi bởi plan này: **duy nhất** `review/agent0/K2-PLAN.md`.
Không sửa: code, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `AGENTS.md`, `spec/`, `viscript/`.

> **K2 KHÔNG phải kernel thật.** K2 là **mô hình tất định trong sim**, không có PTE thật, không có
> MMU/TLB phần cứng, không chạy QEMU, không boot, không copy nửa cao kernel. **K3 mới là QEMU +
> page table thật + TLB thật + pager thật.** Mọi tuyên bố "K2 xong" phải đọc kèm câu này.

## 0. Nguồn đã đọc (đúng giới hạn, không đọc thêm)

| Nguồn | Phần đọc | Dùng cho |
|---|---|---|
| `docs/v2/ViFastOs-v2-thiet-ke.md` | `sed -n '353,375p'` (§4.10 Bộ nhớ ảo) | object, rights/attrs, W^X, ASID, shootdown, ghi chú COW/pager |
| cùng tệp | `sed -n '468,500p'` | bảng Object×Method, `install_at = K2`, reserved/`NotSupported` |
| cùng tệp | dòng 876 | dòng DoD của K2: "0 vi phạm TLB-safety 100.000 seed; đột biến AP→BSP bị bắt" |
| `docs/QUYET-DINH-sau-review.md` | grep `Frame\|VSpace\|PageTable\|TLB\|ASID\|W^X` → dòng 142, 164, 168, 221 | EXEC không là bit cap; 19 mã lỗi có `WxViolation`, `MissingPageTable`, `HasChildren`; reserved `VSpace` khách; đột biến "bỏ xử lý hộp thư khi chờ ack" |
| `v2/agent/DECISIONS.md` | D2-47..D2-64 (dòng 83–100) | Miri ngoài `gate --fast`; hằng số model ở `v2/docs`, **chưa** có `spec/kernel.toml`; `FaultKind` VM model-only; `TlbFlush`/`VmKick` trong backlog K3; ack thế hệ của `vifast-sync`; kỷ luật cổng/không nới ngưỡng |
| `v2/sim/vifast-sim/src/kobj.rs` | grep 7 ký hiệu, 60 dòng đầu | `ObjType::{VSpace,PageTable,Frame}` = reserved 9/10/11, `is_core()==false`, `KError::{MissingPageTable,WxViolation,Alignment,HasChildren,AlreadyMapped,NotMapped}` đã có; `FaultKind` đã tồn tại (Exit); `HartMask` ≤64 bit; `waitaddr_bucket` đang khoá bằng `ObjId` thô vì **chưa có Frame** |
| `spec/abi.toml` | grep 3 tên | `VSpace id=9`, `PageTable id=10`, `Frame id=11`, đều `stability="reserved"`, `install_at="K2"`, `unsupported=true`; method id đã có sẵn (Frame 0..4) |
| `v2/lib/vifast-objmem/src/obj.rs` | chỉ kind/constructor | `ObjHeader.kind: u8`, `BODY_OFFSET=16`, `kind → constructor` (D2-48); object body là vùng có cỡ, **không** có allocator động bên trong |
| **Ngoài nguồn** | `ls` thư mục (không đọc nội dung) | xác nhận `spec/` **chỉ có `abi.toml`** (⇒ **không có `spec/kernel.toml`**), `v2/docs/` có lối đặt tài liệu `k1b-*`/`k1c-*`, `v2/lib/` **chưa có** `vifast-pte`, `v2/test/scn/` đã dùng tới `20-*` |

Nhãn: `[TK]` = hằng số/quyết định thiết kế của plan này; `[GT]` = giả thuyết phải xác minh hoặc
Agent 0 phải chốt; mọi số **đo** phải do `./v2/bin/v2 nightly` sinh, plan này **không** chứa số đo.

## 1. Phạm vi K2 model

### 1.1 Làm trong K2

1. **Frame** 3 cỡ: 4 KiB (`size_bits=12`), 2 MiB (`21`), 1 GiB (`30`) — metadata thuần (pa/extent,
   cỡ, quyền đang map, bộ đếm W^X, trạng thái sống), **không** cấp phát 1 GiB thật.
2. **VSpace root**: object gốc giữ cây page table + `asid` + `generation` + `active_on: HartMask`.
3. **PageTable** map/unmap/protect: 3 cấp, lá chọn theo cỡ Frame; lỗi `MissingPageTable`,
   `AlreadyMapped`, `NotMapped`, `Alignment`, `RangeError`, `HasChildren`.
4. **Rights** `READ`/`WRITE` (không có `EXEC` trong cap — QD:142) + **attrs** `WB/WC/UC/Device/NX/EXEC`;
   `EXEC` là thuộc tính lúc map.
5. **W^X**: bộ đếm `mapped_w`/`mapped_x` **theo Frame vật lý, toàn cục mọi VSpace**; vi phạm ⇒ `WxViolation`.
6. **ASID + generation**: cấp phát/trả ASID, hết ASID ⇒ `generation += 1` + flush toàn bộ.
7. **TLB model**: mỗi hart một TLB nhỏ tất định, entry `(asid, gen, va_page, pa_page, size, rights)`.
8. **Shootdown** `TlbFlush{asid, va, gen, seq}`: `active_on` → hộp thư các hart → IPI model → **ack thế hệ**;
   luật chống deadlock: hart chờ ack **phải** xử lý hộp thư của mình.
9. **VM fault**: `FaultKind::Vm` **model-only**, giao qua đường fault sẵn có tới `fault_ep`.
10. **Shared frame**: một Frame map vào ≥2 VSpace (read-only) — chỉ cần bộ đếm toàn cục đúng.

### 1.2 KHÔNG làm trong K2 (chống trượt phạm vi)

- **K3**: QEMU/OpenSBI, PTE thật, `sfence.vma`/`tlbi`/`invlpg` thật, IPI thật, Sv39/Sv48 thật, PBMT.
- **K3**: nửa cao kernel + physmap + copy root entry (design §4.10) — K2 chỉ ghi chú, không cài.
- **K3/khách**: `VSpace` khách (`GUEST` flag) — giữ `NotSupported` (D2-32).
- **D3**: `Frame.MapIo`/`UnmapIo`, Device Frame thật — K2 chỉ giữ nhánh `Device` trong attrs.
- **K3**: pager/COW thật, cấp phát lười, file ánh xạ (cần page fault thật + VFS).
- **Không** sửa `spec/abi.toml`: `VSpace`/`PageTable`/`Frame` **vẫn `reserved`, `unsupported=true`**;
  K2 cài ngữ nghĩa **sau model API** trong sim, conformance "reserved ⇒ `NotSupported`" vẫn xanh
  (bảng Object×Method: "Object chưa cài chỉ giữ số reserved"; D2-56(a) hoãn đưa `FaultKind` VM vào spec).
- **Không** tạo `spec/kernel.toml` (chưa tồn tại; D2-53 Q3: K3 mới bàn).

## 2. Q1..Q10 Agent 0 phải chốt (kèm đề xuất mặc định)

Mỗi câu: **đề xuất mặc định** (nếu Agent 0 im lặng, plan chạy theo mục này) · **chặn task** · **nếu chọn khác**.

### Q1 — Hằng số K2 để đâu; có mở `spec/kernel.toml` không?
- **Đề xuất**: `v2/docs/k2-vm.md` `[TK]`, **không** mở spec. `spec/` hiện **chỉ có `abi.toml`** ⇒ tạo
  `kernel.toml` là quyết định ABI mới, phải qua Agent 0 + D2 riêng, và D2-53 Q3 đã hoãn sang K3.
  Tệp `k2-vm.md` ghi: `PAGE_SIZE_BITS = {12,21,30}`, `PTE_LEVELS = 3`, `VA_BITS = 39` (hình Sv39),
  `ASID_BITS = 8`, `TLB_ENTRIES_PER_HART = 16`, `MAX_MAP_DEPTH`/`MAX_FRAMES_PER_VSPACE`, bảng mã lỗi dùng lại.
- **Chặn**: KT-35 (tạo tài liệu), mọi task sau đọc hằng số từ đây.
- **Nếu khác**: mở `spec/kernel.toml` ⇒ phải có D2 + task ghi spec riêng, **không** gộp vào KT-35.

### Q2 — Page table 3 cấp hay phẳng; cỡ/căn lề Frame?
- **Đề xuất**: **3 cấp hình Sv39**, lá tại cấp 2 = 1 GiB, cấp 1 = 2 MiB, cấp 0 = 4 KiB. Lý do: cần
  `MissingPageTable` (client phải map PageTable trước — "kernel không tự cấp page table") và `HasChildren`
  (unmap gốc còn con) — bảng phẳng **không** biểu diễn được hai lỗi này. Căn lề: `pa % frame_size == 0`
  và `va % frame_size == 0`, sai ⇒ `Alignment`; 2 MiB/1 GiB phải phủ đúng một entry lá, không cắt ngang.
- **Chặn**: KT-37, KT-38; ảnh hưởng KT-36 (ràng buộc pa).
- **Nếu khác** (phẳng): bỏ `HasChildren`, `MissingPageTable` chỉ còn nghĩa "thiếu root"; phải ghi lý do
  vào `k2-vm.md` và hạ DoD tương ứng — **không** được im lặng bỏ lỗi.

### Q3 — W^X: đếm thế nào; map W sau X thì fail hay unmap?
- **Đề xuất**: đếm **theo Frame vật lý, toàn cục**, tăng khi tạo mapping có quyền tương ứng, giảm khi
  unmap/protect hạ quyền; `map(WRITE)` khi `mapped_x > 0` ⇒ **`WxViolation`, không thay đổi gì** (atomic);
  `map(EXEC)` khi `mapped_w > 0` ⇒ `WxViolation`. **Không** tự unmap/không tự hạ quyền: JIT phải
  `unmap` W rồi `map` X (design §4.10). Bất biến kiểm sau **mọi** op: `!(w>0 && x>0)`, `w,x ≥ 0`,
  `w+x ≤ số mapping sống`, và ở cổng: đếm lại từ cây page table phải khớp bộ đếm.
- **Chặn**: KT-36 (trường bộ đếm), KT-39 (toàn bộ luật), KT-40 (unmap trong shootdown), KT-43.
- **Nếu khác** (tự unmap): phải có op trả "đã hạ quyền" và test riêng; rủi ro mất mapping của VSpace khác.

### Q4 — ASID + generation; TLB invalidate theo VA hay theo ASID?
- **Đề xuất**: `ASID_BITS = 8` ⇒ 256 ASID/thế hệ, cấp từ free-list, **không tái dùng trong cùng thế hệ**;
  cạn ⇒ `generation += 1`, mọi entry TLB có `gen` cũ thành vô hiệu, flush toàn bộ `active_on` **một lần**
  theo `gen` (không flush từng ASID). Invalidate: `(asid, va)` cho unmap/protect một trang (phải phủ cả
  entry lớn chứa `va`), `asid` cho teardown VSpace, `gen` cho cạn ASID. `GetAsid` chỉ là debug (spec).
- **Chặn**: KT-38, KT-40; KT-42 (kịch bản cạn ASID).
- **Nếu khác** (16 bit): đổi hằng số trong `k2-vm.md`, test cạn ASID phải dựng 65 536 vspace ⇒ tăng thời gian L1, cân nhắc.

### Q5 — Shootdown: `TlbFlush` vào hộp thư ngay ở K2? Ack thế hệ tính sao?
- **Đề xuất**: K2 **thêm đúng một** kind `TlbFlush` vào hộp thư model (`VmKick`/`PowerReq` vẫn để K3);
  đây là **nới D2-59** ("thêm `TlbFlush`… vào hộp thư" đang nằm trong backlog K3), nên cần **một dòng D2 mới**
  của Agent 0. Ack dùng lại ngữ nghĩa `vifast-sync` (D2-60): `seq = số send Ok`, `ack(seq) = fetch_max`,
  `acked ≠ delivered`; unmap/protect **chỉ trả `Ok` khi** `ack_mask == active_on` — đó là bất biến
  `unmap_ok ⇒ TLB mọi hart trong mask nhất quán với page table`. Hart chờ ack **phải** rút hộp thư của mình
  (luật chống deadlock, design §4.10.3) — chính là chỗ đột biến AP→BSP phải bị bắt.
- **Chặn**: KT-40, KT-43 (mutation "bỏ xử lý hộp thư khi chờ ack").
- **Nếu khác** (không đụng hộp thư, dùng hàng phụ `tlb_queue`): **được**, nhưng phải nói rõ trong D2 và
  đột biến phải nhắm hàng phụ; rủi ro K3 phải viết lại đường IPI. K2 **không** được vừa "không có kênh"
  vừa tuyên bố "đã mô hình shootdown".

### Q6 — VM fault: `FaultKind::Vm` model-only; ai xử lý ở K2?
- **Đề xuất**: thêm biến thể `FaultKind::Vm { vspace, va, access, reason }` (`reason ∈ NotMapped |
  Permission | Wx`), **chỉ trong model**, **không** vào `spec/abi.toml` (D2-56(a) hoãn K3). K2 **không** có
  memory server/pager: fault đi qua đường fault sẵn có (`inject_fault`/giao fault) tới `fault_ep` của TCB,
  TCB `BlockedOnSend` mang fault; `procsrv_model` chỉ **nhận và ghi log** — không cấp trang. Không có
  `fault_ep` ⇒ hành vi K1c (kill/`InvalidArgument`) **giữ nguyên**, ghi rõ trong `k2-vm.md`.
- **Chặn**: KT-41; KT-42 (kịch bản fault).
- **Nếu khác** (làm pager model): vượt 3 h/task ⇒ phải tách và dời sang K3; **không** nhồi vào KT-41.

### Q7 — Shared frame/COW có làm ở K2 không?
- **Đề xuất**: **shared read-only frame: CÓ** (cùng một Frame map vào nhiều VSpace, quyền `READ`, bộ đếm
  toàn cục là lưới an toàn) — chi phí thấp, chứng minh bộ đếm W^X đúng. **COW: KHÔNG** — COW cần
  write-fault + cấp trang + copy, tức pager thật (K3); làm sớm sẽ tạo "COW giả" không kiểm chứng được.
- **Chặn**: KT-42 (test shared read-only), không chặn task nào khác.
- **Nếu khác** (làm COW): cần task riêng 3 h + tiêu chí "không rò frame khi copy lỗi"; chấp nhận K2 dài thêm.

### Q8 — Miri/unsafe: PTE array dùng crate nào?
- **Đề xuất**: crate mới `v2/lib/vifast-pte`, `#![forbid(unsafe_code)]`, `Vec<Pte>` + const-generic
  `PteArray<const N: usize>`, `Pte(u64)` newtype, không phụ thuộc ngoài, không `transmute`. Lý do: design
  §4.10 đặt logic `PageTableFormat` "dùng chung" ở `vifast-pte`; `vifast-objmem` giữ `unsafe` đóng (D2-17),
  K2 **không** thêm `unsafe` mới. Miri: `v2 miri` + `nightly --only-miri` **ngoài** `gate --fast` (D2-47(a)).
- **Chặn**: KT-35 (tạo crate), KT-37; KT-43 (Miri là điều kiện cổng).
- **Nếu khác** (nhét vào `vifast-sim`): chấp nhận được cho K2 nhưng K3 sẽ phải bê sang crate khác — ghi nợ.

### Q9 — Thứ tự/song song task nào?
- **Đề xuất** 3 luồng, 2 điểm đồng bộ:
  - **A** KT-35 → KT-36 → KT-37 (nền: crate + Frame + PageTable).
  - **B** sau KT-37: KT-38 (VSpace/ASID) → KT-40 (TLB/shootdown); **C** song song với B: KT-39 (W^X),
    vì KT-39 chỉ cần `FrameMeta` + hook map/unmap đã chốt ở KT-36/37.
  - KT-41 (fault) chạy sau KT-38; KT-42 (kịch bản + bất biến) sau KT-39/40/41; KT-43 cuối, **một mình**
    (nightly/soak không chạy song song — AGENTS §6).
- **Chặn**: toàn bộ bảng task; KT-43 không được bắt đầu khi còn task khác đang giữ cổng.
- **Nếu khác**: nếu chỉ 1 người thi hành, giữ nguyên thứ tự A→B→C→41→42→43.

### Q10 — Cổng K2 gồm gì; ranh giới K3 nói ở đâu?
- **Đề xuất**: `check` + `gate --fast` (L1 1.000 seed) mỗi task; **cổng K2** = `nightly --l1` 100.000 seed
  × smp 1/2/4/8 + mutation (12 cũ + 4 mới) + Miri + `.scn` + bảng DoD; tiêu chí đúng nguyên văn dòng 876:
  **0 vi phạm TLB-safety**, **đột biến AP→BSP bị bắt**. Không nới ngưỡng, không retry để lấy xanh (D2-07).
  Ranh giới K3 ghi thành mục "K2 không phải kernel thật" trong `k2-vm.md` + báo cáo cổng.
- **Chặn**: KT-43; mọi task (điều kiện dừng).
- **Nếu khác**: hạ seed/ngưỡng = **cấm**; nếu wall >60 s thì giảm seed **của `--fast`**, giữ nguyên bất biến (QD §5.2).

## 3. Bảng task KT-35..KT-43

| Task | Tên | Giờ | Phụ thuộc | Song song với | Cổng riêng |
|---|---|---|---|---|---|
| KT-35 | `vifast-pte` + `v2/docs/k2-vm.md` (hằng số, hình 3 cấp) | 2.5 | — | — | `check` + `gate --fast` |
| KT-36 | Frame 3 cỡ + ownership/Untyped + `GetAddress` | 3 | KT-35 | — | `gate --fast` |
| KT-37 | PageTable 3 cấp map/unmap + rights/attrs + mã lỗi | 3 | KT-35, KT-36 | — | `gate --fast` |
| KT-38 | VSpace + ASID + generation + `active_on` | 2.5 | KT-37 | KT-39 | `gate --fast` |
| KT-39 | W^X counters + protect/unmap + bất biến | 2.5 | KT-36, KT-37 | KT-38 | `gate --fast` |
| KT-40 | TLB model + shootdown `TlbFlush` + ack + chống deadlock | 3 | KT-38, KT-39 | — | `gate --fast` + test đỏ mutation |
| KT-41 | VM fault + `FaultKind::Vm` (model-only) | 2 | KT-38 | KT-40 | `gate --fast` |
| KT-42 | `.scn` + `check_vm` bất biến + shared read-only frame | 2.5 | KT-39, KT-40, KT-41 | — | `gate --fast` |
| KT-43 | **Cổng cuối K2** 100k×4 + mutation + Miri + DoD | 3 | tất cả | — (chạy một mình) | `nightly --l1`, mutation, Miri |

Tổng ngân sách ≈ 24 h; 3 luồng ⇒ ~10–11 h tường. Mỗi task ≤ 3 h (QD §5.1); task nào vượt phải tách.

### KT-35 — `vifast-pte` + tài liệu hằng số
- **Việc**: tạo crate safe (`#![forbid(unsafe_code)]`, SPDX `//`), `FrameSize{4K,2M,1G}` ↔ `size_bits{12,21,30}`,
  `Pte(u64)`, `PteArray<N>`, `walk_shape()` (3 cấp: chỉ số bit của VA 39 bit), `v2/docs/k2-vm.md` ghi **toàn bộ**
  hằng số `[TK]`, mã lỗi dùng lại, mục "K2 không phải kernel thật".
- **Test đỏ trước**: `size_bits` lạ ⇒ `InvalidArgument`; `va=1<<38` cắt đúng ba chỉ số; `PteArray<512>` cỡ/không panic.
- **DoD đo được**: `cargo test -p vifast-pte` xanh; `k2-vm.md` có đủ 7 hằng số ở Q1 + 1 bảng lỗi; `cargo fmt`/`clippy -D warnings`.
- **Bằng chứng**: log `gate --fast`, danh sách test; **Miri**: `v2 miri` trên crate mới phải sạch (không unsafe).
- **Rủi ro**: đặt sai thứ tự cấp (bit index) ⇒ mọi test sau sai hệ thống — chốt bằng test biên.

### KT-36 — Frame 3 cỡ + ownership
- **Việc**: `FrameMeta { pa, size_bits, live, mapped_w, mapped_x, maps }`; `Retype(Untyped, Frame, size_bits)`
  chỉ nhận `{12,21,30}`, kiểm `pa` căn lề + nằm trong vùng Untyped; `Frame.GetAddress`; frame không cấp byte thật;
  khi Frame chết: không còn mapping, bộ đếm = 0 (nếu không ⇒ lỗi bất biến, không im lặng).
- **Test đỏ trước**: retype `size_bits=13` ⇒ `InvalidArgument`; pa lệch 1 ⇒ `Alignment`; 1 GiB không ăn RSS.
- **DoD**: 3 cỡ × (map được / huỷ được); `maps==0 && mapped_w==0 && mapped_x==0` sau khi xoá frame;
  `check_vm` không phát hiện frame mồ côi trên 10k seed.
- **Bằng chứng**: `gate --fast`; **Miri** trên test frame; mutation "bỏ kiểm liveness" phải bị bắt ở KT-43.
- **Rủi ro**: 1 GiB "ảo" dễ sinh test giả — cấm mọi test dựa trên `Vec` 1 GiB.

### KT-37 — PageTable 3 cấp map/unmap
- **Việc**: `map(vspace, page_table, va, frame, rights, attrs)`; phải có PageTable đã map ở cấp trên, thiếu ⇒
  `MissingPageTable`; trùng ⇒ `AlreadyMapped`; chọn lá theo cỡ Frame; `unmap(va)`; mã lỗi `NotMapped`,
  `RangeError`, `Alignment`, `HasChildren`; lưu **rights/attrs của từng mapping** (không suy từ cap).
- **Test đỏ trước**: map 4 KiB vào VA 2 MiB-căn nhưng chưa có PageTable cấp 1 ⇒ `MissingPageTable`;
  unmap gốc còn con ⇒ `HasChildren`; map 2 MiB cắt ngang entry ⇒ `Alignment`.
- **DoD**: bảng chân trị 7 mã lỗi × 3 cỡ có test; walk ngược (VA→PA) khớp 100% sau 10k op ngẫu nhiên;
  không rò PageTable khi unmap lá.
- **Bằng chứng**: `gate --fast`; **Miri** trên test walk; **L1** 1k seed.
- **Rủi ro**: lẫn "att minh hoạ" với "leaf" ⇒ walk sai; test walk ngược là chốt.

### KT-38 — VSpace + ASID + `active_on`
- **Việc**: tạo VSpace root (model API; spec vẫn `reserved`), cấp ASID free-list + `generation`, `GetAsid` debug,
  `active_on` cập nhật khi hart chạy/nghỉ trên VSpace, teardown giải phóng ASID + flush theo asid, cạn ASID ⇒
  tăng `generation` + flush toàn bộ; kiểm mask chỉ hart có thật (D2-59 tinh thần `HartMask`).
- **Test đỏ trước**: cấp 257 ASID ⇒ `generation` tăng đúng 1, ASID cũ không tái dùng trong thế hệ cũ;
  `active_on` bỏ hart offline ⇒ không gửi shootdown cho hart đó.
- **DoD**: ASID không trùng trong một thế hệ; `active_on ⊆ hart online`; 10k seed 0 panic;
  flush theo `gen` làm **mọi** entry TLB cũ mất hiệu lực (kiểm bằng oracle của KT-40).
- **Bằng chứng**: `gate --fast`; log ASID cạn; `k2-vm.md` cập nhật nếu đổi `ASID_BITS`.
- **Rủi ro**: tái dùng ASID sớm ⇒ lỗi "stale TLB" **giả** ở K3; test phải khẳng định không tái dùng.

### KT-39 — W^X counters + protect
- **Việc**: tăng/giảm `mapped_w`/`mapped_x` ở **mọi** đường map/unmap/protect/teardown; `protect(va, rights, attrs)`
  tính lại hai bộ đếm; `WxViolation` **trước** mọi thay đổi (rollback sạch); `Device ⇒ EXEC` bị từ chối;
  `NX` và `EXEC` xung khắc ⇒ `InvalidArgument`; `WB/WC/UC/Device` đúng **một** giá trị.
- **Test đỏ trước**: X rồi W ⇒ `WxViolation` và frame **vẫn** X (không bị hạ ngầm); W rồi X ⇒ `WxViolation`;
  unmap một trong hai VSpace chia sẻ ⇒ bộ đếm còn đúng phần còn lại.
- **DoD**: bất biến `!(w>0 && x>0)` sau **mỗi** op trên 10k seed; đếm lại từ cây = bộ đếm ở mọi lần `check_vm`;
  0 frame rò khi 1000 vòng tạo/huỷ.
- **Bằng chứng**: `gate --fast`; mutation "bỏ giảm bộ đếm" và "cho W sau X" **phải đỏ**; Miri sạch.
- **Rủi ro**: bộ đếm toàn cục dễ lệch khi unmap nửa chừng ⇒ rollback + recount ở cổng.

### KT-40 — TLB model + shootdown + ack
- **Việc**: `Tlb` mỗi hart (16 entry, LRU tất định theo chỉ số); `translate(vspace, va)` dùng entry nếu
  `(asid, gen)` khớp, ngược lại walk; `unmap/protect` khi `active_on` khác rỗng ⇒ flush cục bộ + gửi
  `TlbFlush{asid, va, gen, seq}` cho các hart còn lại + chờ **đủ** ack; vòng chờ **rút hộp thư của mình**
  (chống deadlock); oracle vi phạm: `unmap/protect` trả `Ok` ⇒ mọi hart trong mask phải nhất quán.
- **Test đỏ trước**: chạy 2 hart, hart B giữ entry, unmap trên A ⇒ **A chưa được trả `Ok`** khi B chưa ack;
  hart B chờ ack mà không rút hộp thư ⇒ phát hiện deadlock/vô hạn ⇒ **test đỏ** (đúng kỳ vọng).
- **DoD**: `tlb_violations == 0` trên 100k seed × smp 1/2/4/8 (điều kiện dòng 876); ack `seq` đơn điệu
  (`fetch_max`), không lùi; `active_on` rỗng ⇒ không gửi gì; huỷ VSpace giữa lúc chờ ack ⇒ `Cancelled`/`Busy`
  theo D2, không treo.
- **Bằng chứng**: `nightly --l1`, mutation 4 ca (bỏ rút hộp thư / bỏ chờ ack / bỏ flush cục bộ / ack sai seq)
  phải **bị bắt**; Miri trên test 2 hart.
- **Rủi ro**: oracle quá dễ dãi ⇒ "0 vi phạm" vô nghĩa; phải có **ca dương tính** chứng minh oracle bắt được lỗi.

### KT-41 — VM fault (`FaultKind::Vm`, model-only)
- **Việc**: thêm biến thể `Vm{vspace, va, access, reason}`; op model `vm_touch(vspace, va, access)`:
  walk → thiếu/không quyền ⇒ giao fault qua `fault_ep` (đường sẵn có), TCB `BlockedOnSend` mang fault;
  `procsrv_model` nhận + log, **không** cấp trang; không `fault_ep` ⇒ hành vi K1c; **không** sửa spec.
- **Test đỏ trước**: touch VA chưa map ⇒ fault `NotMapped` tới đúng `fault_ep`, không phải panic;
  touch với `READ` trên mapping `WRITE`-only ⇒ `Permission`; `FaultKind::Exit` cũ vẫn chạy (không hồi quy).
- **DoD**: 3 `reason` có test; fault không làm rò Frame/PageTable; `.scn` fault đỏ trước rồi xanh (D2-05);
  `abi.toml` **không đổi** (`git diff --stat spec/` rỗng).
- **Bằng chứng**: `gate --fast`; Miri; đối chiếu `git diff spec/` rỗng.
- **Rủi ro**: thêm biến thể vào enum dùng chung dễ vỡ `match` ở nơi khác ⇒ chạy full test, không `_ =>`.

### KT-42 — `.scn` + bất biến `check_vm` + shared read-only
- **Việc**: 4 kịch bản `.scn` (đặt số trống đầu tiên ≥ 30, kiểm `ls v2/test/scn` tại thời điểm làm `[GT]`):
  retype 3 cỡ; map W sau X bị từ chối; shootdown 2 hart rồi đọc lại; fault→procsrv. Thêm `check_vm` chạy
  sau mỗi op ở mức L1 (rẻ) và bản đầy đủ (đếm lại cây, kiểm bộ đếm, ASID, frame mồ côi) ở mức cổng.
  Test shared read-only: 1 Frame trong 2 VSpace, W^X toàn cục vẫn đúng.
- **Test đỏ trước**: mỗi `.scn` phải đỏ trước khi cài (D2-05), ghi log đỏ vào journal.
- **DoD**: 4 `.scn` xanh + 4 log đỏ trước; `check_vm` bắt được ít nhất 2 lỗi cố ý tiêm vào (self-test);
  shared read-only 0 vi phạm.
- **Bằng chứng**: `gate --fast` (có L1 1k), log `.scn`, self-test của `check_vm`.
- **Rủi ro**: `.scn` chỉ kiểm đường vàng ⇒ phải có self-test chứng minh `check_vm` không rỗng.

### KT-43 — Cổng cuối K2 (chạy một mình)
- **Việc**: chạy `check`; `gate --fast`; `nightly --l1` **100.000 seed × smp 1/2/4/8**; mutation toàn bộ
  (12 ca §7.2 cũ + 4 ca VM mới, gồm **AP→BSP** và **"bỏ xử lý hộp thư khi chờ ack"**); Miri toàn bộ crate
  liên quan; bảng DoD; ghi "K2 không phải kernel thật".
- **DoD**: **0 vi phạm TLB-safety** trên 400.000 lượt; **12+4/16 mutation bị giết** (không ca nào sống);
  Miri sạch; mọi `.scn` xanh; DoD K2 đạt **hoặc** liệt kê nợ có chủ (không được "coi như đạt").
- **Bằng chứng**: log cổng + báo cáo; nếu wall >60 s ⇒ **T-INFRA**, không nới ngưỡng.
- **Rủi ro**: chạy song song với build/cổng khác làm sai số (AGENTS §6) ⇒ chạy một mình, ghi giờ máy.

## 4. Cổng K2 và bảng DoD

### 4.1 Lệnh

```bash
./v2/bin/v2 check                 # fmt + clippy + luật cấu trúc
./v2/bin/v2 gate --fast           # cuối MỖI task; có L1 1.000 seed
./v2/bin/v2 nightly --l1          # 100.000 seed × smp 1/2/4/8 — chỉ ở KT-43
./v2/bin/v2 miri                  # Miri rời (D2-47(a): KHÔNG nằm trong gate --fast)
./v2/bin/v2 nightly --only-miri   # Miri qua nightly
```

### 4.2 Bảng DoD K2 (đo được, không nới)

| # | Tiêu chí | Cách đo | Ngưỡng |
|---|---|---|---|
| D1 | TLB-safety | `tlb_violations` trên `nightly --l1` 100k × 4 smp | **0** |
| D2 | Mutation AP→BSP | bộ mutation | **bị giết** |
| D3 | Mutation "bỏ xử lý hộp thư khi chờ ack" | bộ mutation | **bị giết** |
| D4 | Tổng mutation | 12 ca cũ + 4 ca VM | 16/16 bị giết |
| D5 | W^X | bất biến `!(w>0&&x>0)` sau mỗi op | 0 vi phạm / 400k lượt |
| D6 | Frame leak | `check_vm` cuối mỗi seed | 0 frame mồ côi |
| D7 | ASID | không trùng trong thế hệ; cạn ⇒ +1 gen | đúng 100% ca test |
| D8 | Miri | `v2 miri` + `--only-miri` | 0 lỗi |
| D9 | ABI | `git diff --stat spec/` | **rỗng** |
| D10 | `.scn` | 4 kịch bản mới + toàn bộ cũ | xanh, có log đỏ trước |
| D11 | `gate --fast` wall | log cổng | ≤ 60 s |

## 5. Rủi ro

1. **Model-only bị đọc thành "đã có bộ nhớ ảo thật"** — nguy cơ cao nhất. Giảm: câu "K2 không phải kernel
   thật" ở đầu plan, trong `k2-vm.md`, trong báo cáo cổng; mọi DoD ghi "model"; K3 là nơi duy nhất nói QEMU.
2. **Spec thiếu**: `spec/kernel.toml` **không tồn tại**, `VSpace`/`PageTable`/`Frame` vẫn `reserved` ⇒ dễ có
   người sửa `abi.toml` để "cho khớp". Giảm: Q1/Q10 chốt **không** sửa spec; D9 kiểm `git diff spec/` rỗng.
3. **Page table consistency**: bộ đếm W^X toàn cục + unmap một phần + `HasChildren` dễ lệch sổ. Giảm: rollback
   trước mọi lỗi, `check_vm` đếm lại từ cây ở mức cổng, self-test tiêm lỗi.
4. **TLB oracle quá dễ dãi** ⇒ "0 vi phạm" vô nghĩa. Giảm: bắt buộc **ca dương tính** (cố ý dùng entry cũ phải
   bị đếm) và 4 mutation nhắm đúng đường flush/ack.
5. **Rò frame/ASID** khi task lỗi giữa chừng. Giảm: teardown có bất biến, test 1000 vòng tạo/huỷ, `check_vm` cuối seed.
6. **Miri/unsafe**: `vifast-objmem` có `unsafe` đóng (fat pointer `*mut dyn KObj`); thêm crate mới mà vô tình
   `unsafe`/`transmute` sẽ làm Miri đỏ. Giảm: `#![forbid(unsafe_code)]` ở `vifast-pte`, không thêm `unsafe` mới,
   Miri chạy rời ở KT-43.
7. **Lệch K3 conformance**: mô hình 3 cấp hình Sv39 nhưng K3 có thể chọn cấu trúc khác (AArch64/x86_64) ⇒
   ngữ nghĩa lỗi (`MissingPageTable`, `HasChildren`) phải **arch-neutral** trong model. Giảm: tên cấp theo
   "cấp lá/cấp giữa/root", không nhúng tên thanh ghi/arch; ghi nợ K3.

## 6. Điều plan này KHÔNG làm

- Không sửa `spec/abi.toml` (D9), không tạo `spec/kernel.toml`.
- Không đụng `v2/sim/vifast-sim/src/kobj.rs` hay bất kỳ code nào **trong bước soạn plan này**.
- Không đụng `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `AGENTS.md`, `viscript/`.
- Không tuyên bố K2 DONE; plan chỉ là đề xuất cho Agent 0 duyệt.
- Không đưa số đo viết tay vào tài liệu; mọi ngưỡng ở §4.2 do `nightly` sinh.

## 7. Danh sách `[GT]` cần Agent 0 chốt trước KT-35

| # | Giả thuyết | Ảnh hưởng |
|---|---|---|
| G1 | `FaultKind` đặt ở `vifast-model` hay module sim (grep chỉ thấy tái xuất tại `kobj.rs:335`) | KT-41 (chọn chỗ thêm biến thể) |
| G2 | `TlbFlush` vào hộp thư model ở K2 (nới D2-59) — cần 1 dòng D2 mới | KT-40, KT-43 |
| G3 | `ASID_BITS=8` / `TLB_ENTRIES_PER_HART=16` / `VA_BITS=39` là hằng số `[TK]` phù hợp | KT-35, KT-38, KT-40 |
| G4 | Nhánh `Device ⇒ EXEC` bị từ chối (design không nói rõ) | KT-39 |
| G5 | Số hiệu `.scn` trống đầu tiên ≥ 30 (hiện đã dùng tới `20-*`) | KT-42 |
| G6 | `vifast-pte` là crate mới, không gộp vào `vifast-sim` | KT-35, KT-37 |
| G7 | Hành vi VM fault **không** `fault_ep` giữ nguyên K1c (D2-56(c) vẫn treo) | KT-41 |

---
**Kết luận**: K2 = mô hình bộ nhớ ảo tất định trong sim (Frame 3 cỡ, PageTable 3 cấp, VSpace/ASID,
W^X toàn cục, TLB + shootdown ack thế hệ, VM fault model-only). **K2 chưa phải kernel thật**;
QEMU, PTE/TLB/IPI thật, pager/COW và nửa cao kernel thuộc **K3**.
