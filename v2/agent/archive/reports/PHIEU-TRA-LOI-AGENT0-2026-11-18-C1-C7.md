# PHIẾU TRẢ LỜI AGENT 0 — C1–C7 + hai câu hỏi lại

**Ngày**: 2026-11-18
**Người trình**: lead (Agent 1, dòng OS)
**HEAD**: `220b9993`, đã push, cây sạch, `v2 check` PASS.
**Task**: DONE 131 · READY 2 · BLOCKED 0.

> Tệp tự chứa. Phần 1 trả lời hai câu anh hỏi lại. Phần 2 báo kết quả C1–C7. Phần 3 là việc đã mở
> theo quyết định của anh.

---

# PHẦN 1 — HAI CÂU HỎI LẠI

## 1.1 Bổ sung Q1 — `fault_seen` là gì, ba dòng

**Dòng một — fault đó là gì.** Đó là `FaultKind::Exit{0}`, **không phải lỗi**. Theo D2-25, `Exit`
**đi qua kênh fault** chứ không đi qua Reply: một tiến trình thoát thì kernel ghi lý do thoát vào
`Tcb.last_fault` rồi báo cho người phục vụ qua endpoint fault. Vì vậy bộ đếm `svc.faults` **đếm cả
lần thoát bình thường**, không chỉ đếm lỗi.

**Dòng hai — do đâu sinh ra.** Chuỗi đầy đủ, mỗi mắt xích đều có dòng log:

1. Shell U-mode gọi `exit(0)`.
2. Kernel ghi `Tcb.last_fault = FaultKind::Exit{0}` và in `FAULT_EXIT`.
3. procsrv nhận qua endpoint fault, in `# proc: recv exit pid=… code=0`.
4. procsrv gửi `EV_FAULT` cho shell.
5. Shell nhận, chạy `svc.faults += 1`.
6. Cuối cùng kernel in dòng `# product halt: … fault_seen=1 …`.

**Dòng ba — vì sao giá trị mong đợi là một.** Mỗi lần boot có **đúng một** tiến trình shell, và nó
thoát **đúng một lần** bằng `exit(0)`. Nên `fault_seen` phải bằng một. Bằng không ⇒ shell **không**
thoát qua kênh fault, tức thiết kế D2-25 đã bị phá. Lớn hơn một ⇒ có fault thừa, tức có lỗi thật.
Vì vậy checker soak nay **fail-closed trên giá trị đúng bằng một**, không phải "khác không là được".

**Commit của đột biến làm checker đỏ**: `07b244a7` (KT-127). Đột biến ở `proc_rt.rs:219`:
`EV_FAULT => svc.faults += 1` đổi thành `+= 2`. Kết quả đo được:

```text
fail 1 lane=1 why=fault-seen=2(want=1) help=15 shell_polls=2 help_polls=0 halt_polls=1 …
```

Tức checker **đỏ đúng lý do**, không đỏ vô tội vạ. Nhật ký đầy đủ: `v2/agent/JOURNAL/kt127-journal.md`.

## 1.2 Xác nhận Q4 — đúng chín lệnh × hai ca, không phải hai ca tổng

**Xác nhận: đúng chín lệnh, mỗi lệnh có một ca đúng và một ca biên, tất cả trên ảnh sản phẩm.**

Cách làm: thêm **hai** phép kiểm vào `shell-edge.sh`, nhưng mỗi phép kiểm chạy **một lượt gửi cho
từng lệnh** và khẳng định riêng cho từng lệnh — không phải một khẳng định gộp:

- **`new-cmds-ok`** (ca đúng) — gửi lần lượt `time`, `uptime`, `sync`, `stat /README`, `ls`; mỗi lệnh
  phải in **đúng dạng** của nó, và `ls` phải liệt kê được ít nhất một mục thật.
- **`new-cmds-edge`** (ca biên) — gửi `stat` không tham số, `stat /khongco`, `ls /khongco`,
  `truncate /README abc`, `truncate /README 4294967296`, `sync` hai lần, `time` hai lần; mỗi lệnh
  phải báo **đúng lỗi** của nó, kích thước ≥ 2^32 phải bị từ chối, và đồng hồ phải đơn điệu.

Cộng với bốn lệnh đã có ca từ trước (`truncate`/`jobs` trong `truncate-jobs`, `stat` trong cùng ca),
tổng là **chín lệnh**, mỗi lệnh có **ít nhất một ca đúng và một ca biên** trên ảnh sản phẩm.

Bằng chứng chạy thật trên ảnh `6b8f4870…`:

```text
# new-cmds-ok: PASS (# shell: stat path=/README kind=1 size=79 ino=2)
# new-cmds-edge: PASS (# shell: kich thuoc qua lon (toi da 4294967295))
# truncate-jobs: PASS (# shell: stat path=/README kind=1 size=5 ino=2)
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge PASS
```

Và bộ tự kiểm của chính `shell-edge` (27 phép kiểm, trong đó 19 ca âm) cũng xanh — nên "PASS" ở trên
không phải PASS rỗng.

---

# PHẦN 2 — KẾT QUẢ C1–C7

## 2.1 C1 = (A) — đã làm xong việc kiểm tra kèm theo

**Tệp đã làm sha ảnh đổi**: đúng **một** tệp — `v2/test/boot/build-rev.sh`, do KT-128 sửa (thêm ca
hồi quy âm cho cổng build-inputs). KT-127 chỉ sửa script trong `v2/agent/JOURNAL/`, thư mục này vốn
đã bị loại khỏi dấu vân tay nên không ảnh hưởng.

**Nó có trong bao đóng cargo của ảnh không**: **không**. Nó là script shell, không được biên dịch vào
ảnh.

**Đã làm**: loại **ba** tệp khỏi danh sách ghim, không chỉ một — vì cùng một lý do:

| Tệp bị loại | Vì sao không phải đầu vào của ảnh |
|---|---|
| `v2/test/boot/build-rev.sh` | chính là script **định nghĩa** dấu vân tay, không tham gia build |
| `v2/tools/pre-measure.sh` | kiểm **trước khi đo**, không tham gia build |
| `v2/tools/task-target.sh` | chỉ đặt thư mục đích của cargo, không đổi byte ảnh |

**Giữ lại hai tệp** vì chúng **thật sự** đổi byte ảnh: `v2/tools/repro-build.sh` (đặt `RUSTFLAGS`
remap + strip) và `v2/tools/rustc-canon.rs` (wrapper rustc canonical). Đây là "ảnh thật sự phụ thuộc
vào nó" theo đúng câu anh dặn.

**Ca kiểm mới (h)**: sửa một tệp test/checker ⇒ dấu vân tay **KHÔNG** đổi. Đã chứng minh ca này
**đỏ thật** khi đột biến: thêm lại `build-rev.sh` vào danh sách ghim thì ca (h) đỏ với
`sửa tệp test/checker mà dấu vân tay ĐỔI (e0657f96… -> 2b18cf77…)`. Bản đầu của ca (h) tôi viết có
`|| true` nên đột biến vẫn xanh — đúng loại "xanh giả", đã sửa thành fail-closed.

**Một chi tiết tôi tự thêm, xin anh biết**: để bịt khe "đổi **thuật toán** dấu vân tay mà dấu vân tay
không đổi", tôi đưa một hằng số phiên bản thuật toán (`VBREV_ALGO`) vào **chuỗi băm**. Đổi thuật toán
mà quên bump hằng số thì dấu vân tay vẫn đổi; đổi tệp công cụ thì không đổi. Đúng ranh giới anh đặt.

**Không làm lại baseline K6** theo đúng lệnh. Ảnh mới nhất hiện tại là `d2925f7e…` (build từ HEAD
`be0e5ab8`), khác `a0287e4b` — đúng như dự đoán, và văn bản đóng K6 vẫn gắn `a0287e4b`.

## 2.2 C2 = (A) — đã làm, có ca kiểm và có đột biến

`soak.sh` nay có cờ `--features`. Ba điểm đáng nói:

**Một: build bằng pipeline canonical, không phải `cargo build` trần.** Đây là chỗ tôi phải sửa thêm
một thứ ngoài yêu cầu: nếu build trần thì sha **khác** ảnh phát hành (thiếu remap + strip), và như
thế "soak ảnh phát hành" hoá ra là soak một ảnh khác. Nay khi có `--features`, soak gọi
`repro-build.sh` ⇒ sha trùng pipeline canonical, và in dòng neo `# neo ảnh: sha=… TRÙNG ảnh đã ghim`.

**Hai: mỗi vòng đòi đủ ba dấu hiệu**, đúng như anh dặn — thiếu một là đỏ:

- `shell>` — U-mode shell đã lên;
- `# shell: help cmds=<n>` — shell **nhận và xử lý** lệnh thật (không chỉ in prompt);
- `# shell: exit code=0` + `# product halt:` + QEMU **tự thoát** mã 0.

**Ba: đã chứng minh đỏ thật.** Đổi mẫu `help` thành một mẫu không bao giờ khớp ⇒ cả hai vòng đều đỏ
với lý do `no-help-response`, không phải đỏ vô tội vạ.

**Mặc định giữ nguyên**: không có `--features` thì vẫn build ảnh **không shell** như cũ, nên bằng
chứng soak một nghìn vòng cũ không mất giá trị.

**Chạy thử**: `--features product-shell,product-fs --smoke 3 1` ⇒ `3/3 ok`, mỗi vòng đủ ba dấu hiệu.

## 2.3 C7 = (A) có điều kiện — đã có kiểm máy

Đã thêm `v2/tools/check-scope-table.py`, nối vào `v2 check`. Nó làm đúng ba việc:

1. **Cấu trúc** — mỗi dòng bảng phải trỏ tới một khối `scope=`/`scope-red=` **có thật** trong chính
   tài liệu đó, và trường đo phải có **giá trị số** trong khối ấy.
2. **Ngữ nghĩa** — bảng nay có thêm cột **"Giá trị (chữ)"**; cột này phải khớp số trong dòng log.
   Cộng thêm: nếu văn xuôi nói "`trường` bằng <chữ số>" thì chữ số đó cũng phải khớp — trừ những câu
   nói về **tình huống khác** (nhánh âm, ca đỏ, giả thuyết), vì ba tài liệu này có nhiều câu như thế.
3. **Đột biến** — 10 ca tự kiểm, trong đó 8 ca âm, mỗi ca đỏ **kèm đúng mã lý do**.

**Đột biến trên tài liệu thật đã chạy**: đổi `durable=1` thành `durable=0` trong khối log của
`k6-persist.md` ⇒ checker đỏ với `E_VALUE_MISMATCH: durable — văn xuôi nói 1, khối ghi 0`. Khôi phục
⇒ xanh. Vậy "sửa một số trong log" đúng là bị bắt.

**Một chỗ tôi phải nói thẳng**: bản đầu của checker tôi viết chỉ so văn xuôi, và nó báo **bảy lỗi
oan** trên chính ba tài liệu của mình — vì văn xuôi có nhiều câu nói về nhánh âm ("nhánh `durable`
bằng không chưa có ca"). Tôi sửa bằng cách thêm cột giá trị vào bảng (vế chính, không phụ thuộc hành
văn) và chỉ so văn xuôi ở những câu không có từ khoá tình huống khác. Nếu anh thấy cách này lỏng hơn
ý anh, nói để tôi siết.

## 2.4 Phần 3 — rà selftest: **chín** cái không cổng nào gọi

Đây là kết quả rà, đúng như anh dặn. Trước khi rà, chỉ **hai** selftest được cổng gọi; còn **chín**
cái tồn tại, đều xanh khi chạy tay, nhưng **không cổng nào gọi** — cùng đúng lớp lỗi đã làm
`build-rev.sh --selftest` đỏ âm thầm:

| Selftest | Số ca | Trước | Nay |
|---|---|---|---|
| `test/boot/selftest.sh` (kiểm runner ktest) | 18 | không ai gọi | `gate --fast` |
| `test/boot/guard_selftest.sh` (kiểm qemu guard) | 10 | không ai gọi | `gate --fast` |
| `test/boot/shell-edge.sh --selftest` | 27 | không ai gọi | `v2 check` |
| `test/boot/budget.sh --selftest` | — | không ai gọi | `v2 check` |
| `test/rules/counter-evidence.sh --selftest` | — | không ai gọi | `v2 check` |
| `tools/pre-measure.sh --selftest` | 6 | không ai gọi | `gate --fast` |
| `tools/pin-artifact.sh --selftest` | 6 | không ai gọi | `gate --fast` |
| `tools/task-target.sh --selftest` | 6 | không ai gọi | `v2 check` |
| `test/boot/persist.sh --red-selftest` | — | không ai gọi | **chưa** — cần QEMU, xem dưới |

Chia theo thời gian chạy: nhóm nhanh (dưới một giây mỗi cái) vào `v2 check`; nhóm chậm (18 giây tới
55 giây) vào `gate --fast`, để `check` vẫn nhanh.

**Một cái còn lại**: `persist.sh --red-selftest` cần QEMU nên tôi **chưa** đưa vào cổng — nó sẽ làm
`gate --fast` chậm thêm khoảng một phút. Đề nghị: để nó trong `nightly` khi nightly đầy đủ được làm
(hiện nightly đầy đủ **chưa có**, chỉ có `--only-miri` và `--l1`). Nếu anh muốn nó vào `gate --fast`
luôn thì nói, tôi thêm.

---

# PHẦN 3 — ĐÃ MỞ THEO QUYẾT ĐỊNH CỦA ANH

| Anh chốt | Task | Trạng thái |
|---|---|---|
| **C3 = (A), cuối K7** | **KT-137** — `cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối, kèm đúng bộ ca anh liệt kê, và **mọi** lệnh có tham số đường dẫn phải chạy lại ca cũ | READY |
| **C4 = (B)** | K7 đóng khi bốn lỗi đủ ba vế **và** chạy lại sáu tiêu chí K6 không hồi quy | đã ghi vào `K6-CLOSURE.md` §8 |
| **C5 = (A)** | **KT-136** — soak đĩa thật 100 boot, mỗi vòng ghi → reboot → đọc lại → fsck sạch, cộng ≥ 1 ca kill QEMU giữa lúc ghi | READY |
| **C6** | Thứ tự: **KT-131 (xong) → KT-136 → KT-81 → K7** | đã ghi trong QUEUE |
| **C7 = (A)** | Xong, có kiểm máy + đột biến | DONE |
| **Phần 3** | Xác nhận cả năm việc; rà selftest xong, báo ở §2.4 | DONE |

**Việc kế tiếp tôi sẽ làm**: **KT-136** (soak đĩa thật), vì nó gỡ hạn chế thứ hai của K6 — hạn chế
duy nhất trong ba hạn chế còn đóng được bằng công sức.

---

# PHẦN 4 — MỘT ĐIỀU TÔI MUỐN ANH BIẾT

Trong lượt này tôi viết **hai** ca kiểm mà bản đầu **xanh giả**, và cả hai đều do **đột biến** phát
hiện, không do đọc code:

- Ca (h) của cổng build-inputs: bản đầu có `|| true` nên khi đột biến nó vẫn xanh.
- Checker bảng đối chiếu: bản đầu so văn xuôi quá thô, báo bảy lỗi oan trên chính tài liệu của mình.

Tôi ghi ra vì đây đúng là cái anh cảnh báo nhiều lần: **một ca kiểm chưa từng thấy đỏ thì chưa chứng
minh gì**. Cả hai nay đều đã thấy đỏ thật rồi mới xanh.
