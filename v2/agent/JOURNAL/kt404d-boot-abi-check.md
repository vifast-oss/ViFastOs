# KT-404d — máy kiểm khớp boot ABI kernel ↔ userland

Base: `main` @ `7f4d217b` (+ commit QUEUE `12a2011b`), nhánh `task/kt404d-boot-abi-check`.
Nguồn: phát hiện của `KT-416b` (`review/agent0/GOI-ABI-KIEN-TRUC.md` §B.1 #8, §B.2) —
`proc_abi.rs:13-15` dẫn một máy kiểm **không tồn tại**; 17 `const EP_*` phía U-mode đồng bộ với
kernel **bằng comment**. Không cổng nào canh kernel ↔ userland.

## Kế hoạch (đỏ-trước)

1. `v2/tools/check-boot-abi.py` mới, có `--selftest`:
   - đọc `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs`: **17** `EP_*`, **8** `SLOT_*`/`LOOP_*`,
     **2** mảng `TCB_SLOTS`/`SC_SLOTS` (parse fail-closed);
   - đọc mọi `.rs` dưới `v2/user/`: **17** `const EP_*: u64` (13 tên, 4 tệp);
   - ĐỎ khi lệch **tên** (bản sao lạ / bản sao bị đổi tên), lệch **giá trị**, **thiếu bản sao**,
     hoặc parse fail; cộng bất biến bố cục cspace (không trùng slot, < 512, 2 mảng cùng độ dài).
2. **Đỏ-trước**: dựng selftest + `run_check` **rỗng** (luôn rc 0) ⇒ chạy `--selftest` phải ĐỎ
   đúng lý do ("rc=0, kỳ vọng 1"); dán log. Sau đó cài thân checker ⇒ selftest xanh.
3. Chạy trên cây thật ⇒ phải XANH (nếu ĐỎ là phát hiện thật, ghi lại, **không** sửa
   `proc_abi.rs`/`v2/user/` để né).
4. Nối **một** bước `v2 check` (`lane_step` id 85, làn E chỉ-đọc) + `CHECK_STEPS_EXPECTED` 49→50,
   cập nhật comment `49 + 31 = 80` → `50 + 31 = 81`; thêm id 85 vào phép đếm `lane_failed`.
5. T0: checker + `--selftest` + `v2 check`; T1: `gate --dev` qua khoá `/tmp/vifast-t1-lock.sh`.
6. Không sửa `spec/`, `expected.txt`/TAP, byte ảnh; không `git add/commit`.

## Nhật ký thực hiện

(xem các mục dưới, cập nhật theo thứ tự)

### 1. Đỏ-trước (chạy selftest trên thân stub, **trước khi cài** phép kiểm)

Bước "cài checker" được tách khỏi bước "viết selftest": thân `run_check` để **stub** (`rc 0`,
không kiểm gì), chạy `--selftest` ⇒ **12/13 ca ĐỎ đúng lý do "rc=0 (kỳ vọng 1)"**; ca xanh duy
nhất là ca "cây khớp" (stub trả 0 nên tình cờ xanh — đúng như kỳ vọng của một stub). Log
(`/tmp/kt404d-red-selftest.log`, trích):

```
ok 1 - cây khớp (16 bản sao + EP_ECHO ngoài phạm vi) ⇒ XANH: rc=0 (kỳ vọng 0)
not ok 2 - lệch giá trị (EP_FAULT 1→2) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | stdout='check-boot-abi: STUB — chưa kiểm gì (rc 0)'
not ok 3 - lệch giá trị (EP_FS 7→8) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 4 - lệch tên (EP_MEM→EP_MEMORY) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 5 - thiếu bản sao (EP_NET_CLIENT) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 6 - kernel thiếu hằng (EP_UART) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 7 - kernel khai báo trùng ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 8 - kernel parse fail ⇒ ĐỎ (fail-closed): rc=0 (kỳ vọng 1) | ...
not ok 9 - userland parse fail ⇒ ĐỎ (fail-closed): rc=0 (kỳ vọng 1) | ...
not ok 10 - bản sao tên lạ (EP_BOGUS) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 11 - slot trùng (LOOP_ARENA=21↔EP_MEM) ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 12 - 2 mảng khác độ dài ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
not ok 13 - thiếu hẳn tệp bản sao ⇒ ĐỎ: rc=0 (kỳ vọng 1) | ...
check-boot-abi: selftest 1/13 ok
```

### 2. Selftest với thân thật ⇒ 13/13 XANH

Mỗi ca ĐỎ trả `rc=1` **kèm thông điệp đúng đối tượng**, ví dụ:

```
ok 2 - lệch giá trị (EP_FAULT 1→2) ⇒ ĐỎ: ... lệch giá trị `EP_FAULT`: kernel .../proc_abi.rs = 1, v2/user/programs/procsrv.rs:3 = 2
ok 5 - thiếu bản sao (EP_NET_CLIENT) ⇒ ĐỎ: ... thiếu bản sao `EP_NET_CLIENT` trong v2/user/programs/procsrv.rs (kernel = 26)
ok 11 - slot trùng (LOOP_ARENA=21↔EP_MEM) ⇒ ĐỎ: ... slot trùng: `EP_MEM` và `LOOP_ARENA` cùng = 21
ok 13 - thiếu hẳn tệp bản sao ⇒ ĐỎ: ... thiếu bản sao `EP_MEM` trong v2/user/programs/shell_cmds.rs (kernel = 21)
check-boot-abi: selftest 13/13 ok
```

### 3. Cây thật ⇒ XANH (`rc=0`, 0 lệch)

```
check-boot-abi: miễn trừ `EP_ECHO` — cspace ảnh ktest aarch64 (`k5user::SLOT_EP_ECHO`), không thuộc proc_abi riscv64 (giá trị 9 không đối chiếu)
check-boot-abi: kernel 17 EP_* + 8 SLOT_*/LOOP_* + 2 mảng; userland 17 const EP_* trong 4 tệp; 16 bản sao khớp tên+giá trị; 0 lệch
```

13 tên có bản sao (16 khai báo: procsrv 6 + shell 8 + shell_cmds 2); 4 hằng **chỉ kernel** đúng
thiết kế: `EP_UART` (KT-73 bỏ đường dây, giữ slot), `EP_CTL` (cap `SchedControl`, không phải
endpoint), `EP_NET_KERN`/`EP_NET_KERN_REPLY` (kernel nhận `Call`; netsrv dùng cspace riêng
`SLOT_EP_KERN = 3`). `EP_ECHO` (echo.rs, slot 9) thuộc cspace ảnh **ktest aarch64**
(`k5user::SLOT_EP_ECHO = 9`) — ngoài phạm vi `proc_abi.rs`, miễn trừ **có lý do ghi trong mã**.

### 4. Nối cổng (một bước)

- `v2/bin/lib/steps_check.sh`: `step_boot_abi()` = `--selftest` rồi `--root "$REPO_ROOT"`
  (một hàm, một `lane_step`; `check-selftest-wiring.py` đòi cờ tự kiểm xuất hiện trên dòng cổng).
- `v2/bin/lib/check.sh`: `lane_step E 85 "khớp boot ABI kernel ↔ userland (KT-404d)"` (làn E
  chỉ-đọc) + `lane_failed 85` vào tổng đếm.
- `v2/bin/lib/consts.sh`: `CHECK_STEPS_EXPECTED` **49→50**, T2 80→81; comment `49 + 31 = 80`
  → `50 + 31 = 81`; `gate.sh` comment tổng bước cập nhật (đã cũ 47/78 → 50/81).

### 5. T0 — `./v2/bin/v2 check` PASS

```
v2 check: PASS (L1 chạy trong gate --fast)
--- khớp boot ABI kernel ↔ userland (KT-404d): OK
# step-time khớp boot ABI kernel ↔ userland (KT-404d) 482 ms
# gate-steps registered=50 ran=50 skipped=0 failed=0
# gate-wall 91133 ms
```

`check-selftest-wiring.py`: `PASS (45 script tự kiểm: 43 có cổng, 2 ngoại lệ, 0 mồ côi)` — dòng
`| v2/tools/check-boot-abi.py | CÓ | v2/bin/lib/steps_check.sh:117 |`.
`check-filenames.py`: `PASS (1839 tên tracked)`. `cargo fmt --all` cả 2 workspace: không đổi tệp Rust
(0 tệp `.rs` bị sửa bởi task này).

### 6. T1 — `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` PASS

```
# t1-lock: giữ khoá (pid …) — chạy: ./v2/bin/v2 gate --dev
--- khớp boot ABI kernel ↔ userland (KT-404d): OK
# step-time khớp boot ABI kernel ↔ userland (KT-404d) 247 ms
# gate-steps registered=82 ran=68 skipped=14 failed=0
# gate-wall 649901 ms (649.9 s)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
# t1-lock: nhả khoá (rc=0)
```

`registered=82` = `CHECK_STEPS_EXPECTED` 50 + `GATE_FAST_EXTRA_EXPECTED` 31 + QEMU smoke T1 (id 70);
14 bước T2-only bị bỏ **có in nhãn** (đúng cơ chế KT-242), 0 đỏ.

### 7. Việc còn lại (ghi nợ, không làm trong 55 phút)

1. **`SLOT_EP_*` của netsrv chưa được canh**: `v2/user/programs/netsrv_prod.rs` có 5 bản sao
   (`SLOT_EP_NET=1`, `..._REPLY=2`, `..._KERN=3`, `..._CLIENT=5`, `..._CLIENT_REPLY=6`) khớp với
   **bảng copy cap** `proc_net.rs:180-186` (CNode thứ cấp), không phải `proc_abi::EP_*`. Cùng lớp
   trôi nhưng khác nguồn; đề nghị mở rộng checker (hoặc gộp vào `BootCapLayout`/`KT-420`).
2. **4 hằng chỉ-kernel** (`EP_UART`, `EP_CTL`, `EP_NET_KERN`, `EP_NET_KERN_REPLY`) chưa được ghim
   **giá trị** — chỉ được bảo vệ gián tiếp bằng bất biến "không trùng slot + < 512". Cố ý: ghim
   giá trị trong tool sẽ chép lại bảng mà `KT-420` sẽ xoá; nếu Agent 0 muốn ghim thì thêm bảng
   vàng 4 dòng.
3. **`EP_ECHO`** được miễn trừ **có lý do** (`k5user::SLOT_EP_ECHO`, cspace ảnh ktest aarch64); tool
   **không** đối chiếu giá trị với `k5user.rs` (ngoài phạm vi `proc_abi.rs`).
4. **Lead**: commit + chạy **T2** `gate --fast` trên ứng viên tích hợp (task này không chạy T2 theo
   luật). Kỳ vọng `registered=81` (50 + 31), bước 85 xanh.
5. Khi `KT-417`/`KT-420` thay 17 `EP_*` bằng `BootCapLayout` sinh từ `spec/`, **bảng
   `EXPECTED_COPIES` phải viết lại** (nó ghim cả tên tệp) — đây là chủ ý fail-closed, không phải
   nợ ẩn.
