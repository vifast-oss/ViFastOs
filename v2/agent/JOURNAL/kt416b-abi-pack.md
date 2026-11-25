# KT-416b — Gói bổ sung RFC Hợp nhất Kiến trúc & ABI Sạch (bảng `EP_*` ↔ `BootCapLayout`, khuyến nghị A/B, kịch bản kiểm)

- Nhánh: `task/kt416b-abi-pack` · worktree `/private/tmp/vifast-wt/1` · base `main` @ `9eb0e589`
- Tính chất: **task TÀI LIỆU** — 0 dòng mã sản phẩm, **0 byte `spec/`**, 0 sửa
  `v2/agent/{STATE,QUEUE,DECISIONS,BLOCKED}.md` / `reports/STATUS.md`.
- Tệp được sửa/tạo: `review/agent0/GOI-ABI-KIEN-TRUC.md` (+ PHẦN B), `review/agent0/GOI-ABI-KIEN-TRUC.json`
  (thêm `abi_version_decision`, `corrections`, `recommendation` + `test_plan` cho 8 nhóm, `tool_debt`),
  và **tệp này**.
- Việc còn lại của lead: commit (sub-agent không `git add/commit/push`), rồi trình Agent 0 chọn A/B.

## 0. Ràng buộc đọc được từ mã nguồn (định hình toàn bộ gói bổ sung)

1. **RFC gốc soạn tại `fc998aa8`; cây nay đã đi qua `KT-409`…`KT-418b`** ⇒ 8 khẳng định của PHẦN A
   không còn khớp mã. Đã lập bảng đính chính §B.1 (kèm `đường-dẫn:dòng`), **không** xoá văn cũ.
2. **Nâng `abi_version` là quyết định có giá**: `v2/test/abi/check_abi.py:243-244` ghim cứng
   `abi_version == 1` / `min_compat == 1`; `docs/abi.md` là golden sinh từ spec (`spec/abi.toml:18`).
   Luật đã có: chữ ký method **không** thuộc 5 thứ đóng băng (D2-33, `v2/agent/DECISIONS.md:70`), và
   tiền lệ D2-242 duyệt cả gói ABI v1.1 với `abi_version = 1` (`v2/agent/DECISIONS.md:462`).
3. **`VSpace` không được có method** (D2-32, `v2/agent/DECISIONS.md:63`; D2-98 nhắc lại ở `:167`) ⇒
   nhóm 4 của PHẦN A phải **bỏ** phần `VSpace.Map/Unmap`; `Tcb.Exec` là method **mới** số 16 vì
   16 method `Tcb` hiện là 0..15 (`spec/abi.toml:230`).
4. **Nhóm 5 gần như đã xong từ trước**: `[arch.x86_64.gdt.planned]` **đã có** (`spec/kernel.toml:92-106`)
   và máy kiểm đã canh (`v2/test/kernel/check_kernel.py:400-424`). Việc còn lại chỉ là lật
   `sysret_ready` (`spec/kernel.toml:87`), đang bị `check_kernel.py:395-396` buộc `false`.
5. **Nhóm 1 là bài toán "máy kiểm buộc đường dẫn"**: `check_kernel.py:277` đòi `[limits.plat]`, 5 lá
   trỏ `v2/kernel/vifast-plat-common/src/obj/mod.rs#MAX_*` (`spec/kernel.toml:43-47`); máy kiểm so
   theo **tên hằng trong tệp** (`check_kernel.py:318-343`) nên bước 1 chỉ cần đổi chuỗi `source`.
   Bẫy: `MAX_OBJECTS/MAX_CAPS/MAX_UNTYPEDS` **trùng tên** giữa `v2/kernel/vifast-core/src/machine.rs:35-39`
   (256/512/16) và `obj/mod.rs:76-80` (96/160/4).
6. **Nhóm 8 là nợ "không có máy kiểm"**: `proc_abi.rs:13-15` dẫn `v2/agent/JOURNAL/kt77b-harness-scan.sh`
   §slot, nhưng `grep -n slot` trên tệp đó = **0 dòng** và `grep -rln proc_abi v2/test v2/bin v2/tools`
   = **0** tệp. Phía U-mode giữ **17** `const EP_*` trong 4 tệp và đồng bộ với kernel **bằng comment**
   (`v2/user/programs/shell.rs:69-94`). ⇒ `BootCapLayout` chỉ có giá trị nếu đi kèm ca kiểm mới.

## 1. Đã bổ sung gì (đúng 3 việc được giao + 2 mục bắt buộc kèm theo)

1. **§B.2 — bảng đối chiếu**: 17 `EP_*` (giá trị 1..27) + 8 `SLOT_*`/`LOOP_*` + **2 mảng**
   `TCB_SLOTS`/`SC_SLOTS` (= 29 dòng bảng) ↔ trường `BootCapLayout` đề xuất, kèm **kiểu** (`u16` cho
   cspace vì `max_caps = 512` — `spec/kernel.toml:31`; `u8` cho ô chương trình vì `MAX_PROCS = 6` —
   `proc_abi.rs:160`) và **sentinel** `0xFFFF`/`0xFF` (không dùng `0`: `SLOT_PROCSRV = 0` là giá trị
   hợp lệ). Nêu rõ 2 chỗ **không gộp được**: `SLOT_CHILD = SLOT_NETSRV = 2` (khác ảnh) và
   `SLOT_PROCSRV = 0` vs cspace slot 0 = `CS_SELF` (`spec/abi.toml:35-36`).
2. **§B.3 — 8 khuyến nghị A/B**, mỗi nhóm: khuyến nghị + lý do **có `đường-dẫn:dòng`** + phương án
   thay thế. Tóm tắt 1 dòng/nhóm ở báo cáo cuối.
3. **§B.4 — kịch bản kiểm** theo 3 tầng: lệnh T0 (`./v2/bin/v2 t0 <crate…>`, `t0.sh:13-14`), bước T1
   phải xanh (`gate --dev`; số bước: 3/26/28/36/77/84 — `check.sh:22-86`, `gate.sh:151`), bước T2
   (`gate --fast`; 43/59/70 — `gate.sh:24,123,139`), và **ca phải ĐỎ** (đột biến có thật trong
   `v2/test/mutants.toml`, không bịa id).
4. **§B.0 — câu hỏi chặn `abi_version`** (thay câu chốt §0 của PHẦN A): khuyến nghị **giữ 1/1**.
5. **§B.1 — đính chính 8 chỗ** PHẦN A lệch cây tại base (bảng, có dòng dẫn).

## 2. Lệnh đã dùng để lấy bằng chứng (tái lập được, không có số đo hiệu năng)

```bash
wc -l v2/kernel/vifast-plat-common/src/obj/*.rs            # 1259+445+450+270
wc -l v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs        # 201
grep -c '^pub const EP_' v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs      # 17
grep -c '^\[method_signature\."' spec/abi.toml             # 16 (không có Tcb.Exec)
grep -n '^\[\[mutant\]\]\|^id = ' v2/test/mutants.toml     # danh sách đột biến có thật
grep -n "slot\|proc_abi" v2/agent/JOURNAL/kt77b-harness-scan.sh              # 0 dòng
grep -rln proc_abi v2/test v2/bin v2/tools                 # 0 tệp
grep -rn "^const EP_" v2/user/programs/*.rs | wc -l        # 17 (bản sao phía U-mode)
./v2/bin/v2 check                                          # 1 lần: luật cấu trúc + fmt + clippy
```

## 3. Ghi chú về QD §2.2 (không có số đo viết tay)

- Mọi con số trong PHẦN B là **đếm tĩnh trên tệp nguồn** (dòng, số hằng, số mục) — tái lập bằng lệnh
  ở §2, **không** phải số đo hiệu năng/thời gian.
- Con số duy nhất thuộc loại "đo" là `617208` (arena `Machine<UntypedStore>`): **không** chép vào spec,
  chỉ **trích log có sẵn** `v2/agent/JOURNAL/kt410-general-store.md:119,207`, và khuyến nghị nhóm 7
  thay bằng `source = ".../untyped_store.rs#text:..."` (luật ở `untyped_store.rs:71`).

## 4. Không làm (đúng luật task)

- Không sửa `spec/`, không sửa mã sản phẩm, không sửa `v2/agent/{STATE,QUEUE,DECISIONS,BLOCKED}.md`,
  `reports/STATUS.md`.
- Không `git add/commit/push/merge/stash` (lead commit).
- Không chạy `gate --dev`/`gate --fast`/`nightly`/`soak`/`l1-tier`/`mutants`; chỉ **đọc** tệp + `grep`
  và chạy `./v2/bin/v2 check` **một lần**.
