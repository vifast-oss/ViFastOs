# PHIẾU GỬI AGENT 0 — 2026-11-19 02:40 JST

**Từ**: Agent 1 (lead), theo uỷ quyền của người dùng 2026-11-18 21:55 JST
(*"tự quyết định thay Agent0"*).
**Việc đã làm**: đóng **K7 "Đóng lỗi v1"** trong một đêm, bằng **chín sub-agent** (tối đa bốn
cùng lúc), mỗi nhánh một worktree, chỉ lead merge, `gate --fast` sau **mỗi** merge.
**Báo cáo đầy đủ**: [`BAO-CAO-QUA-DEM-2026-11-19.md`](BAO-CAO-QUA-DEM-2026-11-19.md).

---

## 1. Kết quả một dòng

**K7 ĐẠT** — bốn lỗi v1 **N1/N2/N3/N7** đóng, **B7** đóng, **KT-137** đóng, **sáu tiêu chí K6**
đo lại đầy đủ trên **ảnh cuối** `e7149448…` (cùng dấu vân tay `0b8016e6cb6038d5` với cây).
Kèm **bốn hạn chế** và **bốn nợ** đã ghi tên — không có mục nào bị giấu.

---

## 2. Bốn việc cần Agent 0 chốt

### Q1 — Hai ngữ nghĩa `wake()` và `wake_from` (N7)

Hiện có **hai** ngữ nghĩa chéo hart, cả hai **có chủ ý**:

- `wake()` — **post-only**: TCB có `home` khác hart hiện tại thì chỉ gửi `MailboxMsg::Wake` về
  `home`, **không** đặt `Ready` ngay. Đây là ngữ nghĩa test KT-151 canh.
- `wake_from()` — **post + Ready ngay**: dùng cho bốn đường finalise/resume **không thể hoãn**
  (chúng phải hoàn tất ngay trong lượt gọi).

**Câu hỏi**: có thống nhất về **một** ngữ nghĩa không? Nếu có, phải **đổi test KT-151** — việc của
Agent 0, không phải Agent 1. Nếu không, ghi rõ hai ngữ nghĩa là thiết kế chấp nhận.

### Q2 — 4 client **mức kernel** cho pager (N3)

Đã có 4 client **mức host** (`PagerCore`). 4 client **mức kernel** đòi đổi VA base / CSpace / bảng
trang của ảnh `ktest-user` — D2-130 cấm Agent 1 tự làm. **Câu hỏi**: làm trong K7 hay chuyển v2.1?

### Q3 — Seed **thư mục** vào FS sản phẩm (KT-137)

FS sản phẩm hiện **chỉ có gốc `/`** (`proc_fs::seed` chỉ tạo `/README`). Vì vậy `cd` vào **thư mục
con** chưa kiểm được, và đột biến "`pwd` in hằng `/`" **không phân biệt được** — đã ghi là **chưa
chứng minh**, không tuyên bố. Thêm `/bin` sẽ làm `cd /bin` giống kịch bản v1, nhưng **đổi nội dung
FS sản phẩm** ⇒ phải chạy lại F2 (parity) và có thể cả `persist.sh`. **Câu hỏi**: làm hay để v2.1?

### Q4 — Vế 1 của N2/N3/N1 **không phải ảnh**

Luật 4 đòi vế "đỏ trên mã cũ" chạy trên **ảnh build từ commit cũ**. Đêm nay:

- **N2**: ảnh v1 build từ **cha của commit vá** (`5fc7b3ff`) — **đúng luật**, có sha ảnh + log.
  Nhưng tag `v1-archive` **đã chứa bản vá r296b** nên 50 boot trên tag cho **0/50**; phép đo đó
  được giữ làm **đối chứng dương**.
- **N3**: tag đã vá nhãn (r309) nên **không tái hiện** trong 3 boot ⇒ vế 1 là **test host trên mã
  trước r309**, ghi rõ **"không phải ảnh"**.
- **N1**: v2 không có x86_64 ⇒ vế 1 là **trích dẫn** `[V1:ĐO]`, ghi rõ **"chưa đo lại"**.

**Câu hỏi**: Agent 0 có chấp nhận ba dạng vế 1 này không, hay muốn **luôn** là ảnh (khi đó N3 phải
build v1 tại commit trước r309 — làm được, tốn thêm thời gian; N1 thì **không thể** cho tới K5')?

---

## 3. Những gì đã tự quyết (D2-128 … D2-143)

Ghi đầy đủ trong [`v2/agent/DECISIONS.md`](../../DECISIONS.md). Tóm tắt:

| Mã | Nội dung | Có gì đáng chú ý |
|---|---|---|
| D2-128 | B7: sửa **test + chú thích**, không sửa mã; thêm L0 workspace kernel vào cổng | kỳ vọng cũ **sai** so với thiết kế, hai chỗ cùng cũ |
| D2-129 | N2: định nghĩa lại thành **bốn điều** | tiêu chí gốc `MAX_IO=256` **không đo được** trên v2 (thiết bị 512) |
| D2-130 | N3: checker + 4 client + đột biến; **cấm** đổi ABI | — |
| D2-131 | N7: đòi **mọi** đường wake qua một cơ chế | **tiêu chí này bị kết quả đo bác bỏ** — xem D2-137 |
| D2-132 | N1: kịch bản sim + conformance; **không đột biến nào làm đỏ ⇒ không đóng** | — |
| D2-133 | KT-137 mở ngay (B4 đã đóng bởi KT-132) | — |
| D2-134 | Giao thức đêm: bốn sub-agent, chỉ lead merge, gate sau mỗi merge | — |
| D2-135 | N2 vế 1 dùng **cha của commit vá** | cùng nguyên tắc A3 của phiếu lượt năm |
| D2-136 | N3 đóng **có hạn chế** | vế 1 là test host, không phải ảnh |
| D2-137 | **N7 KHÔNG đóng** ở lượt đầu | reviewer phá được cổng ⇒ mở KT-154 |
| D2-138 | KT-137 đóng **có hạn chế** | FS sản phẩm chỉ có `/` |
| D2-139 | T-INFRA: QEMU mồ côi làm cổng đỏ oan | đã dọn, chạy lại PASS |
| D2-140 | **N7 ĐÓNG** sau KT-154 + KT-155 | đột biến mà reviewer phá được **nay bị bắt** |
| D2-141 | T-INFRA: cổng **không chạy song song** sub-agent | số worktree đổi làm ca git-guard đỏ |
| D2-142 | T-INFRA: bash 3.2 hỏng heap; workaround subshell | **đã kiểm** không nuốt lỗi (`rules` chỉ dùng `local`) |
| D2-143 | **K7 ĐẠT** | bốn hạn chế + bốn nợ ghi rõ |

**Hai ngoại lệ `spec/` đã dùng** (đều hẹp): `status` của `cd`/`pwd` → `active`; `expect` của đúng
hai mục đó → đầu ra thật của v2. **Không** đụng `spec/abi.toml`, `spec/kernel.toml`, không đổi ABI.

---

## 4. Điều đáng chú ý nhất của đêm

**N7 suýt được tuyên bố đóng sai.** Máy kiểm `check-wake-paths.py` nằm trong cổng, test chạy xanh,
đột biến của tác giả làm đỏ — nhưng khi reviewer độc lập thêm **một đường wake thứ hai đi vòng qua
hộp thư**, cổng vẫn `errors=0` và test vẫn `5 passed`. Chỉ `--strict` (khi đó **không** nằm trong
cổng) mới đỏ.

Đã vá trong đêm: thay **heuristic** bằng **allowlist tường minh**, đưa `--strict` vào cổng, và
**chứng minh lại bằng đúng đột biến đó** — nay nó bị bắt (`E_STRICT_TRANSITION … errors=2 EXIT=1`).

Đây là lần thứ mười mấy của lớp lỗi "test xanh nhưng giả" trong repo này, và là lần đầu nó bị bắt
**trước khi** báo cáo được gửi đi. Lý do bắt được: một sub-agent **chỉ** làm việc phá bằng chứng,
không làm tính năng.

---

## 5. Trạng thái cuối

```text
$ ./v2/bin/v2 status
HEAD        5fddeca106cc (main)  chưa push: 0
build-inputs 213 mục  dấu vân tay: 0b8016e6cb6038d5  dirty: 0
ảnh mới nhất e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin
             sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536
             # build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
task        DONE=145 READY=0 BLOCKED=0 REVIEW=0
```

`gate --fast` PASS sau **mỗi** merge trên cây sạch. `v2 check` PASS. Không QEMU mồ côi.
