# ViFastOs là gì, đã làm được gì, và còn phải làm gì

> Viết cho **sinh viên năm 1** — không cần biết gì trước về hệ điều hành.
> Số liệu lấy từ chính máy đo của repo, cập nhật **2026-11-23 08:20 JST** (commit `7494aaeb`).
> Tài liệu này **chỉ giải thích**; nguồn sự thật vẫn là code + `spec/` + `v2/agent/DECISIONS.md`.

---

## 1. Một câu cho dễ hình dung

**ViFastOs là một hệ điều hành viết từ đầu bằng Rust** — cùng loại phần mềm với Windows, macOS hay
Linux, nhưng do nhóm này tự viết. Nó boot được thật trên máy ảo (QEMU), chạy được chương trình
người dùng, có hệ tệp, có mạng, và — điều đáng tự hào nhất — **cùng một lõi chạy được trên ba loại
CPU khác nhau**: `riscv64`, `aarch64` (ARM 64-bit, tức chip điện thoại/Apple Silicon) và `x86_64`
(PC thường).

Nếu Windows là "một toà nhà đã xây xong", ViFastOs đang ở giai đoạn: **đã cất xong móng + 3 tầng
chính của ba toà nhà giống nhau, mỗi toà đã có điện nước, thang máy chạy được, có bảo vệ và một
căn hộ mẫu để ở thử**.

---

## 2. Những khái niệm cơ bản (giải thích bằng đời thường)

### 2.1 Hệ điều hành làm gì?

Máy tính có phần cứng (CPU, RAM, đĩa, màn hình). Nếu để mỗi chương trình tự điều khiển phần cứng thì
loạn: hai chương trình cùng ghi vào một chỗ trong RAM, một chương trình đọc trộm dữ liệu của chương
trình khác, một chương trình lỗi làm sập cả máy.

Hệ điều hành (HĐH) là **lớp trung gian có thẩm quyền**:

- **Chia** CPU, RAM, đĩa cho từng chương trình một cách công bằng và an toàn.
- **Cấm** chương trình A đọc bộ nhớ của chương trình B.
- **Cung cấp** dịch vụ chung: đọc/ghi tệp, gửi/nhận mạng, hiển thị chữ.
- **Không sập** khi một chương trình sai: chỉ chương trình đó chết.

### 2.2 Hai chế độ: "chế độ quản lý" và "chế độ người dùng"

CPU hiện đại có ít nhất hai mức quyền:

| Chế độ | Gọi trên từng kiến trúc | Quyền |
|---|---|---|
| **Kernel mode** (chế độ quản lý) | riscv64: *S-mode* · aarch64: *EL1* · x86_64: *CPL0* | Làm được mọi thứ: sửa bảng trang, tắt ngắt, chạm mọi thanh ghi |
| **User mode** (chế độ người dùng) | riscv64: *U-mode* · aarch64: *EL0* · x86_64: *CPL3* | Chỉ chạm được vùng nhớ của mình, muốn gì phải **xin** kernel |

Đây là nền của mọi thứ an toàn: chương trình người dùng (shell, trình duyệt, game) chạy ở User mode.
Muốn đọc tệp, nó phải gọi **syscall** — "quầy hành chính" của kernel.

> **Analogy**: kernel là toà nhà hành chính có khoá; bạn (chương trình) không được vào kho, chỉ được
> nộp đơn ở quầy. Quầy đó gọi là **syscall**.

### 2.3 ViFastOs là *microkernel*: chia việc cho nhiều "phòng ban nhỏ"

Có hai trường phái:

- **Monolithic** (như Linux): mọi thứ — hệ tệp, mạng, driver — nằm trong kernel. Nhanh, nhưng một
  lỗi trong driver mạng là sập cả máy.
- **Microkernel** (ViFastOs chọn hướng này): kernel chỉ giữ **cơ chế tối thiểu** (chuyển CPU giữa
  các task, gửi thông điệp, cấp phát bộ nhớ, phân quyền). Hệ tệp, mạng, driver… chạy thành **tiến
  trình người dùng riêng**. Chúng "nói chuyện" với nhau bằng **thông điệp (IPC)**.

Lợi ích: driver mạng hỏng thì chỉ tiến trình mạng chết, kernel vẫn sống. Giá phải trả: mọi thứ phải
đi qua thông điệp nên khó tối ưu hơn.

### 2.4 Bốn khái niệm cốt lõi bạn sẽ gặp trong code

| Khái niệm | Trong code | Nghĩa đời thường |
|---|---|---|
| **Object** (đối tượng) | `ObjId`, `vifast-core` | Mọi thứ kernel quản lý đều là "đồ vật có số hiệu": một task, một hộp thư, một vùng nhớ, một cái cổng. |
| **Capability** (quyền) | `Cap`, `CNode`, `SlotRef` | Một **tấm vé** ghi rõ "được làm gì với đồ vật nào". Không có vé thì không chạm được — kể cả khi biết số hiệu. Đây là điểm mạnh nhất của thiết kế. |
| **TCB** (Task Control Block) | `vifast-core/src/core_tcb.rs` | **Hồ sơ công dân** của một luồng chạy: thanh ghi, ngăn xếp, bảng trang riêng, trạng thái (`Ready`/`Running`/`Blocked`/`Inactive`), danh sách vé. |
| **Endpoint** (điểm cuối) | `Endpoint`, `ipc.rs` | **Hộp thư**. Muốn nhờ tiến trình khác làm gì, bạn gửi thư vào hộp của nó; nó `Recv` để đọc. |

### 2.5 Bộ nhớ ảo: "mỗi người một tấm bản đồ riêng"

RAM thật chỉ có một. Nhưng mỗi chương trình lại được trao một **bản đồ địa chỉ riêng** (gọi là
**VSpace** / không gian địa chỉ), do **MMU** (bộ quản lý bộ nhớ trong CPU) dịch địa chỉ ảo →
địa chỉ thật. Hai chương trình có thể cùng "thấy" địa chỉ `0x40000000` mà thực ra là hai vùng RAM
khác nhau. Bản đồ đó là các **bảng trang (page table)**.

Nhờ vậy kernel cấm được: chương trình A không có "trang" nào trỏ tới RAM của B ⇒ A có muốn đọc trộm
cũng không có đường.

### 2.6 Driver, virtio, DHCP/ARP/DNS

- **Driver**: đoạn mã biết nói chuyện với một thiết bị cụ thể (card mạng, ổ đĩa).
- **virtio**: chuẩn thiết bị **ảo** của QEMU. Máy ảo giả làm "card mạng virtio", kernel viết driver
  một lần là chạy trên mọi máy ảo.
- **DHCP**: xin địa chỉ IP từ router ("nhà em ở số mấy?").
- **ARP**: đổi "địa chỉ IP" thành "địa chỉ phần cứng MAC" trong mạng nội bộ ("số nhà → tên người").
- **DNS**: đổi tên miền thành IP ("vifast.org → 1.2.3.4").
- **ping**: gửi một gói tin nhỏ, đo thời gian đi–về.

Ba thứ DHCP/ARP/DNS chính là ba bước để "ra được Internet" — và đó là lý do một lỗi DNS nhỏ có thể
làm cả bài soak đỏ (đã từng xảy ra, xem §5).

---

## 3. Kiến trúc hiện tại (sơ đồ)

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  NGƯỜI DÙNG / ỨNG DỤNG                                                       │
│  shell (help/echo/ps/ls/cat/write/ping/run/… 25 lệnh)   procsrv   echo        │
│  ↑ cùng MỘT nguồn Rust: v2/kernel/vifast-plat-user/programs/*.rs             │
├──────────────────────────────────────────────────────────────────────────────┤
│  THƯ VIỆN NGƯỜI DÙNG  vifast-plat-user (façade: in, cấp phát, gọi syscall)     │
│  mỗi kiến trúc có "cửa" riêng: riscv64 `ecall` · aarch64 `svc #0` · x64 `int 0x80` │
├──────────────────────────────────────────────────────────────────────────────┤
│  KERNEL LÕI  vifast-core  (DÙNG CHUNG cho cả 3 kiến trúc)                     │
│  Machine · TCB · Endpoint/IPC · CNode/Cap · VSpace · Untyped · Scheduler      │
│  "một bộ não, ba cơ thể"                                                      │
├──────────────────────────────────────────────────────────────────────────────┤
│  TẦNG NỀN TẢNG THEO KIẾN TRÚC                                                 │
│  vifast-plat-riscv64   Sv39, PLIC, OpenSBI, virtio-blk/net/gpu/input          │
│  vifast-plat-aarch64   MMU 4K, GIC, PL011 UART, virtio                         │
│  vifast-plat-x86_64    long mode, IDT/APIC, 8259 PIC, COM1, virtio             │
├──────────────────────────────────────────────────────────────────────────────┤
│  ẢNH BOOT  vifast-kbin-{riscv64,aarch64,x86_64}                              │
│  `ktest` (bộ kiểm, TAP 71/46/47) · `product` (< 64 KiB) · `product-shell` (≤ 256 KiB) │
├──────────────────────────────────────────────────────────────────────────────┤
│  PHẦN CỨNG / MÁY ẢO QEMU  ── CPU · RAM · virtio-blk (đĩa) · virtio-net (mạng) │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Ba "loại ảnh" kernel — vì sao cần phân biệt

Một hệ điều hành cần nhiều **bản dựng** khác nhau cho các mục đích khác nhau. ViFastOs có ba:

| Loại ảnh | Dùng để | Ngân sách `.data+.bss` |
|---|---|---|
| `ktest` | Ảnh **kiểm thử**: in ra hàng trăm dòng `ok N - …` (chuẩn TAP) để máy chấm điểm | ≤ 256 KiB |
| `*-product` | Ảnh **sản phẩm tối giản**: boot, in vài dòng, tắt. Dùng để chứng minh "nhỏ thật" | **< 64 KiB** |
| `*-product-shell` | Ảnh **sản phẩm có shell**: có shell + `Machine` + `procsrv` cho người dùng gõ lệnh | ≤ 256 KiB |

Đây là chỗ dễ hiểu sai nhất, và nhóm đã sai một lần rồi sửa: xem **§5.1**.

### Các crate chính (bạn sẽ đọc cái nào?)

| Thư mục | Nội dung | Khi nào đọc |
|---|---|---|
| `v2/kernel/vifast-core/` | Lõi HĐH thật: `machine.rs` (1430 dòng), `core_ipc.rs` (1498), `invoke.rs` (1207), `core_tcb.rs`, `ipc.rs` | Muốn hiểu **cách HĐH hoạt động** — đọc cái này trước |
| `v2/kernel/vifast-plat-{riscv64,aarch64,x86_64}/` | Chạm phần cứng: bảng trang, ngắt, UART, driver virtio | Muốn hiểu **máy tính chạy thật thế nào** |
| `v2/kernel/vifast-kbin-*/` | Ghép tất cả lại thành một ảnh boot được (`main.rs`, `bootasm.rs`, `trap.rs`) | Muốn hiểu **boot** |
| `v2/kernel/vifast-plat-user/` | Thư viện + chương trình người dùng (`shell.rs` 1400 dòng, `procsrv.rs`, `shell_cmds.rs`) | Muốn hiểu **người dùng thấy gì** |
| `v2/lib/vifast-fs/` | Hệ tệp (đọc/ghi/`mkdir`/`rm`/`rename`) | Muốn hiểu **lưu dữ liệu** |
| `v2/lib/vifast-net/` | DHCP, ARP, DNS, TCP/UDP client | Muốn hiểu **mạng** |
| `v2/sim/vifast-sim/` | **Mô phỏng** kernel trên máy thật, chạy hàng nghìn "seed" ngẫu nhiên để tìm lỗi tranh chấp | Muốn hiểu **kiểm thử tự động** |
| `v2/bin/v2`, `v2/test/boot/*.sh` | "Cỗ máy chấm điểm": 75 bước kiểm tra | Muốn hiểu **quy trình làm việc** |

---

## 4. Đã làm được gì (số liệu thật, lấy từ máy đo)

### 4.1 "Một bộ não, ba cơ thể" — đã xong

| Hạng mục | riscv64 | aarch64 | x86_64 |
|---|---|---|---|
| Boot được trong QEMU | ✅ | ✅ | ✅ |
| Cùng lõi `vifast-core::Machine` | ✅ | ✅ | ✅ |
| Shell ở User mode (`help`/`echo`/`ps`/`wait`/`kill`) | ✅ (25 lệnh) | ✅ (6 lệnh) | ✅ (6 lệnh) |
| Hệ tệp (đọc/ghi/`mkdir`/`rm`/`mv`/`ln`/`fsck`) | ✅ | — | — |
| Mạng (DHCP + ARP + DNS + ping) | ✅ | ✅ (ping) | ✅ |
| Chạy chương trình con (`run`/`exec`) | ✅ | — | — |
| **Số ca TAP tự kiểm** | **27/27 ảnh boot sạch** | **46** | **47** (+ GPU: 48) |

Con số "**TAP 46**" nghĩa là: ảnh kernel tự in ra 46 dòng `ok N - <tên ca>` và bộ chấm điểm kiểm đủ
46 dòng, không có `not ok` nào. Nó giống bài kiểm tra trắc nghiệm mà **bài thi tự chấm**.

### 4.2 Ảnh sản phẩm đã "gầy" tới mức đáng kinh ngạc

| Ảnh | `.data+.bss` (biến toàn cục + biến chưa khởi tạo) | Trần |
|---|---:|---:|
| `x86_64-product` | **43.072 B** (≈ 42 KiB) | < 65.536 B |
| `x86_64-product-shell` | 51.176 B | ≤ 262.144 B |
| `aarch64-product-shell` | **696 B** (!!) | ≤ 262.144 B |
| `riscv64-product-shell` | 224.472 B | ≤ 262.144 B |

Vì sao phải nhỏ? Vì ảnh sản phẩm là thứ chạy trên máy thật/thiết bị nhúng; "nhỏ" là một **tính năng**
được **đo** chứ không phải khẩu hiệu. Kỹ thuật chính: đưa các cấu trúc lớn (như `Machine` 184.792 B)
ra **vùng nhớ không nằm trong ảnh** (NOLOAD) và khởi tạo lúc chạy.

### 4.3 Shell "cùng một nguồn" cho ba kiến trúc

Trước đây mỗi kiến trúc có một bản shell riêng (viết 3 lần, sửa 3 lần). Nay **một** tệp
`vifast-plat-user/programs/shell.rs` được biên dịch cho cả ba, chỉ khác "cửa vào kernel"
(`ecall` / `svc #0` / `int 0x80`).

### 4.4 Tương thích với "v1" đo được: **25/28**

Repo có một danh sách 28 lệnh mà phiên bản cũ (v1) đã có. Hiện **25 lệnh đã chạy được** với hợp đồng
được kiểm tự động (`spec/v1-parity.toml` + bộ chạy parity), **3 lệnh còn thiếu**: `free`, `df`, `loop`.

### 4.5 Đã sửa một lỗi rất khó — và tìm ra **gốc rễ**

Triệu chứng: shell `ping` xong không nhận được trả lời, dù phía mạng đã trả lời. Ai cũng tưởng lỗi ở
mạng. Thực ra lỗi ở **kernel**: khi một tiến trình gửi thông điệp bằng `NBSend` (gửi không chờ) rồi
**kết thúc ngay**, kernel dọn "hộp thư" của nó và **xoá luôn thông điệp đã gửi thành công** — thông
điệp đã thuộc về người nhận nhưng vẫn bị rút lại.

Sửa: chỉ rút những thông điệp **còn đang chặn**, không rút thông điệp đã trả `Ok`
(`unlink_tcb_queues_keep_committed`). Kèm 4 bài test viết **trước** khi sửa (đỏ → xanh) và 2 phép
"đột biến" để chứng minh bài test có răng.

---

## 5. Cỗ máy chấm điểm: vì sao dự án này "khó chết"

Đây là phần hay nhất và cũng lạ nhất với sinh viên năm 1: **dự án không tin vào lời hứa, chỉ tin vào
số đo**.

### 5.1 Ba tầng kiểm

| Tầng | Ai chạy | Nội dung | Thời gian |
|---|---|---|---|
| **T0** | Agent đang làm | `cargo test`, bài test viết trước | vài giây–phút |
| **T1** | Agent đang làm | `./v2/bin/v2 gate --dev` — kiểm nhanh, bỏ 14 bước nặng | ~10 phút |
| **T2** | **Chỉ lead** | `./v2/bin/v2 gate --fast` — **75 bước**, gồm QEMU thật, L1 1000 seed, soak | ~11 phút |

### 5.2 T2 gồm những gì (75 bước)

- `cargo fmt` + `clippy` (không warning) + luật cấu trúc (mỗi tệp ≤ 1.500 dòng…).
- **Mô phỏng L1**: chạy kernel giả lập với **1.000 seed ngẫu nhiên**; bản "full" là **100.000 seed ×
  4 cấu hình = 400.000 lượt** (mất ~24 phút).
- **QEMU thật** trên cả 3 kiến trúc: boot, TAP, ca âm (cố tình làm sai ⇒ **phải** đỏ).
- **Soak**: boot đi boot lại hàng trăm/hàng nghìn lần để bắt lỗi hiếm.
- **Đột biến (mutation)**: cố tình chèn lỗi vào code ⇒ bộ test **phải** phát hiện. Nếu chèn lỗi mà
  test vẫn xanh ⇒ test vô dụng, bị đánh dấu.
- **Ngân sách ảnh**: `.data+.bss` vượt trần ⇒ đỏ.
- **Quét marker**: ảnh sản phẩm không được chứa chữ của bộ test (`ktest`, `not ok`, `# harness`…).

### 5.3 Vì sao phải khắt khe đến vậy

Vì có những lỗi **chỉ xuất hiện 1/600 lần** — và đúng một lỗi như thế đã bị bắt: soak x86_64 chạy
**599/600** rồi đỏ; hoá ra là DNS trả về khung hỏng mà driver không thử lại. Nếu chỉ chạy 10 lần thì
không bao giờ thấy. Đó là lý do tồn tại của soak và mutation.

### 5.4 Ba bài học "người lớn" đã học được trong đêm 2026-11-23

1. **Trần ngân sách không đủ bắt lỗi cấu trúc.** `Machine` 184.792 B nhét vào `.bss` vẫn **lọt** trần
   262.144 B (`696 + 184.792 = 185.488`). Vì vậy phải kiểm thêm **bất biến cấu trúc**: "`Machine`
   phải nằm ở vùng NOLOAD ≥ 184.792 B".
2. **"Không có thiết bị" khác "thiết bị hỏng".** Một HĐH **phải boot được trên máy không có card
   mạng**. Gộp hai trường hợp làm kernel `PANIC` (và bộ tự kiểm guard đỏ). Nay: không có NIC ⇒ in
   `# product net: no device` rồi chạy tiếp; có NIC mà hỏng ⇒ vẫn dừng (fail-closed).
3. **Đo sai thời điểm = kết luận sai.** Bộ tự kiểm đếm số QEMU **ngay** sau khi lệnh `timeout` trả về
   ⇒ thấy "còn 1 QEMU" trong khi QEMU đã chết nhưng chưa kịp biến khỏi bảng tiến trình. Sửa: chờ tối
   đa 1 giây cho số đó hạ về 0 — **không** nới ngưỡng, chỉ bỏ **race** đo.

---

## 6. Còn phải làm gì

### 6.1 Chặng cuối của `v2.0` (4 việc, theo đúng thứ tự)

| Task | Việc | Vì sao cần |
|---|---|---|
| **KT-345b** | `free`/`df`/`loop` ⇒ **28/28** parity + `netsrv` phục vụ nhiều người gọi | Đóng nốt 3 lệnh còn thiếu so với v1 |
| **KT-346** | Tách `v2/user/`; `VBREV_ALGO kt134-v5`; đóng nợ **P20** | Hiện "dấu vân tay bản dựng" phụ thuộc **thư mục build** ⇒ hai máy build ra hai mã khác nhau dù cùng mã nguồn |
| **KT-347** | PoC `U1a` (`no_std+alloc` runtime `vifast-rt`) + tài liệu sẵn sàng UEFI | Chuẩn bị cho ứng dụng Rust thật |
| **KT-347b** | Gộp `v2/test/boot/lib/` (harness/budget/soak dùng chung) + `nightly --soak --profile` | Để bộ kiểm không phình thành 20 bản sao |
| **KT-348** | **Nghiệm thu**: T2 75/75 + `nightly --all` + soak 3 kiến trúc + xuất `R2-v2.{md,json}` | Đây là "lễ tốt nghiệp" của `v2.0` |

### 6.2 Chu kỳ 3 (`v2.1`, đã thiết kế xong, **chưa mở**): 17 task, 4 đợt

| Đợt | Nội dung | Ví dụ task |
|---|---|---|
| **C3-1** | **SMP 4 nhân chạy thật** + hợp nhất lõi + công cụ quan sát | `KT-350` (SMP preemptive riscv64/aarch64), `KT-351` (x86_64 4 hart), `KT-354` (mutants + trace nhị phân) |
| **C3-2** | Cấp phát động từ `Untyped`, an toàn FPU/SIMD, `DTB`/`ACPI` thật, RFC ABI v1.2 | `KT-355`, `KT-356`, `KT-357`, `KT-358` |
| **C3-3** | ABI v1.2, driver chạy bằng **ngắt** (thay vì hỏi liên tục), `U1a`/`VSH2`, UEFI | `KT-360`…`KT-364` |
| **C3-4** | Chuẩn bị phần cứng thật (PCIe MSI-X, NVMe, IOMMU) + nghiệm thu `v2.1` | `KT-365`, `KT-366` |

**Điều kiện mở Chu kỳ 3**: `KT-348` phải ĐẠT, Agent 0 duyệt báo cáo `R2-v2`, gắn tag `v2.0`.

### 6.3 Những "nợ" đã biết mà chưa trả (nói thẳng)

- **SMP chưa thật**: `Machine` hiện chạy trên **1 nhân** ở ảnh sản phẩm (đa nhân là việc của `KT-350`,
  `KT-351`, `KT-352`).
- **W^X cho vùng `Machine`** (chống ghi-và-chạy đồng thời) chưa bật.
- **Driver vẫn "hỏi vòng" (polling)** thay vì chờ ngắt ⇒ tốn CPU; `KT-361` sẽ đổi.
- **Mới 1 kiến trúc có hệ tệp** (riscv64); aarch64/x86_64 có shell nhưng chưa có dịch vụ tệp.
- **`history`/`!!`** mới giữ 1 dòng; **`runfs`/`exec`** chưa truyền tham số.

---

## 7. Nếu bạn là sinh viên năm 1, đọc code theo thứ tự này

1. **`v2/kernel/vifast-core/src/ipc.rs`** (nhỏ, ~300 dòng) — hiểu "hộp thư" và 3 thao tác
   `Send`/`Recv`/`NBRecv`. Đây là trái tim của microkernel.
2. **`v2/kernel/vifast-core/src/core_tcb.rs`** — hiểu "hồ sơ công dân": tạo, chạy, chặn, kết thúc.
3. **`v2/kernel/vifast-core/src/invoke.rs`** — "quầy hành chính": mọi syscall đi qua **một** hàm
   `dispatch`. Đọc xem `DebugPutStr` và `Exit` được phục vụ thế nào.
4. **`v2/kernel/vifast-plat-user/programs/shell.rs`** — phía người dùng: `help`/`echo` gọi gì.
5. **`v2/kernel/vifast-kbin-riscv64/src/main.rs`** — điểm khởi đầu sau khi bootloader trao quyền.
6. **`v2/test/boot/ktest.sh`** — cách máy chấm điểm một ảnh kernel (đọc để biết "thế nào là đủ bằng
   chứng").

**Bài tập tự kiểm**: thêm một lệnh mới vào `shell.rs` (ví dụ `hi` in "xin chào"), rồi chạy
`bash v2/test/boot/product-riscv64.sh` — bạn sẽ thấy dòng `# shell: hi …` xuất hiện trong log. Đó
chính là vòng lặp mà cả dự án đang chạy: **sửa → đo → ghi bằng chứng → mới tin**.

---

## 8. Từ điển nhanh

| Từ | Nghĩa ngắn |
|---|---|
| **Kernel** | Lõi HĐH, chạy ở chế độ đặc quyền |
| **Syscall** | Lời gọi từ chương trình người dùng vào kernel |
| **TAP** | Chuẩn in kết quả test: `ok 1 - tên`, `not ok 2 - tên` |
| **TCB** | Hồ sơ một luồng chạy (thanh ghi, ngăn xếp, quyền…) |
| **Capability** | Tấm vé cho phép làm gì với đối tượng nào |
| **IPC / Endpoint** | Gửi thông điệp / hộp thư |
| **VSpace / page table** | Bản đồ địa chỉ ảo → thật của một tiến trình |
| **MMU** | Bộ dịch địa chỉ trong CPU |
| **virtio** | Thiết bị ảo chuẩn của QEMU (đĩa, mạng, GPU) |
| **NOLOAD** | Vùng nhớ **không** nằm trong tệp ảnh; kernel tự khởi tạo lúc chạy |
| **Soak** | Chạy đi chạy lại rất nhiều lần để bắt lỗi hiếm |
| **Mutation test** | Cố tình chèn lỗi để xem bộ test có phát hiện không |
| **Seed** | Con số khởi tạo cho bộ sinh ngẫu nhiên (để tái lập được) |
| **Green / Đỏ** | Cổng chạy đúng hết / có bước hỏng |
| **T0 / T1 / T2** | Ba tầng kiểm: nhanh (agent) / vừa (agent) / đầy đủ (chỉ lead) |

---

## 9. Tóm lại trong 5 dòng

1. ViFastOs là một **hệ điều hành microkernel viết bằng Rust**, boot thật trên QEMU.
2. **Một lõi `vifast-core` chạy trên ba kiến trúc** (riscv64, aarch64, x86_64); ảnh sản phẩm đã có
   shell ở User mode, hệ tệp, mạng, chạy chương trình con.
3. **Mọi kết luận đều kèm số đo**: T2 **75 bước PASS 75/75**, TAP 27/46/47, ngân sách ảnh
   `< 64 KiB` / `≤ 256 KiB`, parity **25/28**, ~333 task DONE.
4. Còn **4 việc** để chốt `v2.0` (`KT-345b`, `KT-346`, `KT-347`/`b`, **`KT-348`**), rồi **Chu kỳ 3**
   (17 task) sẽ làm **SMP 4 nhân thật**, cấp phát động, driver theo ngắt, UEFI.
5. Điều đáng học nhất không phải code mà là **quy trình**: viết test trước, đo bằng máy, ghi bằng
   chứng, và **không bao giờ nới ngưỡng để cho qua**.
