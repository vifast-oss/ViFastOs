# PHIẾU QUYẾT ĐỊNH · ViFastOs dòng OS · 18/11/2026

**Gửi**: Agent 0
**Từ**: lead (Agent 1, dòng OS)
**Trạng thái**: **8 việc chờ quyết định** · **3 task bị chặn** · **0 task sẵn sàng làm**

> **Tệp này tự chứa.** Mọi dữ kiện cần để quyết đều nằm trong đây, viết bằng lời. Anh không cần mở
> repo, không cần đọc mã, không cần đọc tài liệu nào khác. Chỗ nào có con số tôi ghi kèm nguồn để
> sau này đối chiếu, nhưng **quyết định không phụ thuộc việc anh đọc nguồn đó**.
>
> Cách trả lời gọn nhất: ghi `Q1 = B`, `Q3 = A`, … vào cuối tệp này hoặc gửi lại bằng lời.

---

## PHẦN 1 — DỰ ÁN ĐANG Ở ĐÂU

### 1.1 Tóm tắt một đoạn

Dòng OS đã đi hết lộ trình K0 → K6 và **đang đứng ở cửa K6**. Mọi thứ đo được đã đo xong và xanh.
Nhưng **chưa ai được tuyên bố K6 đạt** — việc đó thuộc anh. Ba task còn lại đều bị chặn chờ quyết
định của anh; **không có task nào sẵn sàng làm**. Nói cách khác: dự án dừng **đúng chỗ cần anh**,
không phải dừng vì kẹt kỹ thuật.

### 1.2 Số liệu hiện tại (đo lúc 10:2x ngày 18/11)

| Hạng mục | Giá trị |
|---|---|
| Nhánh | `main`, HEAD `f4d1dd5f`, đã push, cây sạch |
| Task | **120 xong** · **0 sẵn sàng** · **3 bị chặn** · 0 chờ duyệt |
| Cổng nhanh `v2 check` | **PASS** (vừa chạy lại) |
| Ảnh phát hành mới nhất | `a0287e4b…`, 205.440 byte, cây sạch |
| Ma trận ảnh test | 21/21 boot sạch |
| Parity 6 lệnh | ok, không lệch |
| Ca biên shell | 8/8 |
| Persist 7 pha qua reboot thật | PASS |
| Soak 1.000 boot trên ảnh phát hành | **1000/1000, 0 lỗi** (vừa chạy lại trên ảnh mới nhất — chi tiết §1.5) |

### 1.3 Đã làm được gì

**Kernel riscv64 thật, chạy trên QEMU:**
- K0 → K3.5: trap, phân trang Sv39, timer, ngắt PLIC/UART, SMP 1/2/4/8, IPI, ASID + shootdown,
  tầng object, U-mode, 16 syscall, procsrv + shell.
- K6: memsrv, capalloc, IDL + ring chia sẻ, VFS (pager + page cache + journal + fsck), virtio-blk,
  virtio-console, SetTls + exec, persist 7 pha qua reboot thật.

**Ảnh sản phẩm giờ là sản phẩm thật:** boot tới `shell>`, 15 lệnh, đọc/ghi tệp trên đĩa thật, đọc
lại sau reboot. Đây chính là chỗ K6 từng **không đạt** ngày 17/11 (ảnh sản phẩm halt sau boot, shell
chỉ có trong ảnh test) — nay đã đóng.

**Hạ tầng đo đáng tin** (phần tốn công nhất và ít thấy nhất):
- Ảnh **tự khai nguồn**: in dấu vân tay của toàn bộ đầu vào build + cờ "cây bẩn". Harness **từ chối
  đo** nếu ảnh không khớp cây hiện tại.
- Ảnh ghim theo sha; công cụ kiểm trước khi đo; 5 luật máy chạy trong cổng; hook git chặn commit
  sai vai.

### 1.4 Điều đáng lo nhất — nói thẳng

Trong sáu ngày, **hơn mười lần** phát hiện "test xanh nhưng giả": test và mã dùng **chung một công
thức sai**, nên chúng khớp nhau mà cả hai đều sai. Ví dụ nặng nhất: `ps` in `state=Unknown` cho mọi
tiến trình, và test lại dựng kỳ vọng bằng đúng công thức sai đó — chỉ khi sửa shell thì ma trận mới
đỏ 18/21 và lỗi mới lộ.

Đã vá từng cái, và đã thêm luật "mỗi bất biến phải có đột biến phía mã làm nó đỏ". Nhưng đây là
**rủi ro hệ thống**, không phải sự cố lẻ: các con số "21/21", "8/8" phải đọc kèm hiểu biết này. Tôi
không muốn anh đọc bảng xanh rồi tưởng mọi thứ đã được kiểm.

### 1.5 Soak trên ảnh mới nhất — vừa xong

Tôi vừa chạy lại soak 1.000 boot trên **đúng ảnh phát hành mới nhất** `a0287e4b` (ảnh có shell +
FS, 15 lệnh):

- **1000/1000 vòng xanh, 0 lỗi**, wall 370 giây.
- Mỗi vòng đều có `shell>` → `help` trả lời `cmds=15` → `exit code=0` → `# product halt:` sạch.
- **Ảnh không đổi** trong suốt phép đo (`changed=0`), sha khớp đầu-cuối.
- Không sót tiến trình QEMU (`qemu_con_sot 0/0`), không PANIC, không timeout.

Nghĩa là: ảnh mới nhất **đã có đủ** ma trận 21/21, parity, ca biên shell 8/8, persist 7 pha **và**
soak 1.000. Không còn phép đo nào thiếu cho ảnh này.

*(Log: `v2/agent/JOURNAL/kt90-logs/run-soak-20261118-102508-72680`.)*

---

## PHẦN 2 — TÁM VIỆC CẦN QUYẾT

### Q1 — K6: ĐẠT hay KHÔNG ĐẠT? *(chặn 3 task)*

**Vì sao phải anh quyết**: tuyên bố đạt một mốc là quyền của anh, không phải của tôi. Tôi chỉ được
trình bằng chứng.

**K6 nghĩa là gì**: mốc "tương đương v1" — bản v2 phải làm được những gì bản v1 đã làm, đo được, có
bằng chứng. Định nghĩa có ba phần: (i) tám hạng mục kỹ thuật của thiết kế; (ii) bảy lỗi của v1 phải
có văn bản đóng/hoãn/mở; (iii) **ở mức người dùng**: ảnh sản phẩm phải có shell tương tác như v1.

**Chuyện đã xảy ra**:
- 17/11: tôi kết luận **K6 KHÔNG ĐẠT**, vì phần (iii) — ảnh sản phẩm halt sau boot, shell chỉ có
  trong ảnh test. Tôi **không** tự hạ phạm vi; tôi trình anh hai cách (đưa shell vào ảnh sản phẩm,
  hoặc hạ phạm vi có văn bản).
- Anh chọn cách một (D2-119 / OD-K6-2). Hai task sau đó (KT-77b, KT-77c) đưa U-mode + shell + FS
  vào **ảnh sản phẩm**.
- Nay phần (iii) đã có bằng chứng: ảnh sản phẩm boot tới `shell>`, chạy lệnh, đọc/ghi tệp trên đĩa
  thật, đọc lại sau reboot.

**Sáu tiêu chí của cổng K6, đo trên ảnh phát hành tại HEAD sạch**: cả sáu đạt. Kèm: ma trận 21/21,
parity 6/6, persist 7/7, soak 1.000/1.000 trên **đúng ảnh phát hành** (mỗi vòng có `shell>` + `help`
+ `exit` sạch).

**Ba hạn chế phải ghi kèm nếu tuyên bố đạt** (tôi không giấu):

1. **Soak chính thức của harness build ảnh không có shell/FS.** Con số 1.000/1.000 trên ảnh phát
   hành đến từ một script riêng trong thư mục nhật ký, **không** phải harness chính thức. Muốn thành
   cổng thường trực thì phải sửa harness — và sửa cách đo là quyền của anh.
2. **Volume FS trong soak là RAM, không phải đĩa thật.** Đường đĩa thật mới có 2 boot + persist 7
   pha, không phải 1.000 vòng.
3. **Ma trận 21/21 là ảnh test, không phải ảnh phát hành** — đúng thiết kế, nhưng phải nói rõ.

**Lựa chọn**:

| | Nội dung | Hệ quả |
|---|---|---|
| **(A)** | Tuyên bố **K6 ĐẠT**, gắn với ảnh `291502d2` (ảnh đã đo đủ sáu tiêu chí + soak 1.000), ghi nguyên ba hạn chế | Đóng được ngay. Nhưng ảnh đã đổi **mười hai lần** kể từ đó (thêm 6 lệnh shell) ⇒ câu "K6 đạt" gắn với một ảnh không còn là ảnh mới nhất |
| **(B)** | Tuyên bố **K6 ĐẠT**, gắn với ảnh **mới nhất** `a0287e4b` — ảnh này **vừa soak xong 1.000/1.000** (§1.5), nên đã có đủ mọi phép đo; ghi nguyên ba hạn chế | Câu "K6 đạt" gắn đúng ảnh đang có, và **không phải chờ gì thêm** |
| **(C)** | **Chưa tuyên bố**, giữ K6 mở, chờ thêm bằng chứng | Không có việc gì để chờ — mọi thứ đo được đã đo. Dự án sẽ đứng yên |

**Đề xuất: (B).** Lý do: (A) đúng về nội dung nhưng gắn với ảnh cũ, và đây đúng là loại mơ hồ đã
gây ra năm lần lỗi "bằng chứng không khớp ảnh" trong dự án này. (B) **không còn tốn gì thêm** — soak
đã chạy xong trên chính ảnh đó.

**Kèm theo, nếu chọn (A) hoặc (B)**: mở một task đưa năng lực "soak ảnh phát hành" vào harness chính
thức (hết hạn chế 1).

---

### Q2 — Nợ P20: tái lập byte của ảnh phát hành

**Dữ kiện**:
- "Tái lập byte" = build hai lần ở hai máy / hai đường dẫn khác nhau ra **cùng một sha256**. Đây là
  tiêu chí OD-K6-3.
- Đã đạt trên **pipeline canonical** (script build chuẩn): hai máy × hai đường dẫn ⇒ cùng sha.
- **Chưa** đạt cho **ảnh phát hành**: phần chênh là **bảng symbol** (tên hàm), **không** phải mã
  chạy. Nghĩa là mã giống hệt, chỉ phần tên bị strip khác nhau.
- Anh đã quyết (D2-122.2) ảnh phát hành = đầu ra pipeline canonical, xuất 2 tệp cùng lần build (ảnh
  strip + ELF đủ symbol). Nhưng "ảnh phát hành = ảnh canonical" theo nghĩa **byte** thì chưa.

**Lựa chọn**:
- **(A)** Chấp nhận: ghi caveat "tái lập byte đạt trên pipeline canonical, **chưa** cho ảnh phát
  hành", chuyển nợ sang v2.1 với tiêu chí "ảnh phát hành = ảnh canonical".
- **(B)** Chặn K6 cho tới khi ảnh phát hành tái lập byte được.

**Đề xuất: (A).** Lý do: phần chênh là bảng symbol, không phải mã chạy; chặn K6 vì nó là đánh đổi
không cân xứng. Nhưng caveat phải nằm **trong văn bản đóng**, không để trong nhật ký.

---

### Q3 — B6: dấu vân tay build-inputs có tính thư mục nhật ký `v2/agent/**` không?

**Dữ kiện**:
- Cơ chế: ảnh in ra một dấu vân tay của **toàn bộ đầu vào build**. Harness so dấu vân tay trong ảnh
  với dấu vân tay tính lại từ cây hiện tại; lệch ⇒ từ chối đo. Đây là cách chống "đo ảnh cũ bằng cây
  mới".
- Anh định nghĩa (D2-127) build-inputs = "toàn bộ `v2/`".
- **Vấn đề**: harness **ghi log trong lúc đo**, log nằm trong `v2/agent/JOURNAL/`. Nếu tính vào
  build-inputs thì dòng log đầu tiên làm dấu vân tay đổi ⇒ lần đo kế tiếp bị từ chối ⇒ **mọi phép đo
  tự vô hiệu chính nó**.
- Tôi đã tạm loại trừ `v2/agent/**`, `v2/docs/**`, `v2/target*/**`, `v2/artifacts/**` (A1-188), kèm
  ca hồi quy dương: chỉ sửa `v2/docs` hoặc `v2/agent` ⇒ build hai lần ra **cùng sha**.

**Lựa chọn**:
- **(A)** Duyệt cách tạm của tôi: build-inputs = (bao đóng phụ thuộc theo `cargo metadata`) ∪ (tệp
  trong `v2/` mà ảnh phụ thuộc), **trừ** `v2/agent/**`, `v2/docs/**`, `v2/target*/**`,
  `v2/artifacts/**`.
- **(B)** Giữ nguyên "toàn bộ `v2/`" ⇒ phải chuyển log ra ngoài `v2/` (thay đổi cấu trúc repo).
- **(C)** Cách khác anh muốn.

**Đề xuất: (A).** Lý do: `v2/agent/**` và `v2/docs/**` không nằm trong `cargo metadata` của bất kỳ
crate nào ⇒ không phải đầu vào build; và ý định của anh là so **nội dung đầu vào**, không so lịch sử
git.

---

### Q4 — `spec/v1-parity.toml` lạc hậu: 6 lệnh đã có nhưng spec vẫn ghi "chưa có"

**Dữ kiện**:
- `spec/` là **vùng cấm sửa** với tôi (AGENTS §4): chỉ task được anh ghi rõ mới được chạm. Đây là
  hàng rào anh đặt để bảo vệ ABI.
- Trong đêm, tôi thêm 6 lệnh shell vào **ảnh sản phẩm**: `time`, `uptime`, `sync`, `stat`,
  `truncate`, `jobs`. Tất cả dùng syscall **đã có** — không mở ABI mới.
- Nhưng `spec/v1-parity.toml` vẫn ghi 6 lệnh đó là `deferred-K7` với lý do "shell v2 chưa có lệnh
  này". **Spec giờ nói sai sự thật.**
- Đây là chỗ nguy hiểm: spec là nguồn sự thật thứ hai; spec sai thì mọi thứ dẫn từ spec sai.

**Lựa chọn**:
- **(A)** Anh cho phép tôi sửa **một lần**, chỉ 6 mục đó: đổi `status` từ `deferred-K7` sang
  `active`, cập nhật `reason`, giữ nguyên mọi thứ khác.
- **(B)** Anh tự sửa.
- **(C)** Để nguyên, ghi nợ.

**Đề xuất: (A)**, kèm điều kiện: chỉ 6 mục, không đổi ABI, có ca kiểm chứng minh `check_scn` / gen
vẫn xanh.

**Kèm câu hỏi nhỏ**: ảnh sản phẩm giờ có **15 lệnh**, ảnh test vẫn **6 lệnh** — vì cả 9 lệnh thêm
sau (3 lệnh FS ở KT-77c, 4 ở KT-114, 2 ở KT-125) đều nằm sau cờ `shell_fs`. Đây là cách tôi tránh
phải sửa kỳ vọng harness. Anh thấy chấp nhận được không? Nếu không, phải sửa kỳ vọng harness — việc
đó cần anh.

---

### Q5 — `cd` / `pwd` có thuộc "tương đương v1" không?

**Dữ kiện mâu thuẫn**:
- Mã v1 ở tag đóng băng **có** `cd`/`pwd`, có trong help của v1, có trong bảng sinh tự động của v1.
- Nhưng nhật ký v1 ghi vòng 222/223 **đã revert** `cd`/`pwd`; tag `r323` lại có.

**Vì sao cần anh**: đưa vào hay loại bỏ đều **đổi định nghĩa "tương đương v1"**. Tôi không được tự
đổi kỳ vọng.

**Lựa chọn**:
- **(A)** Nếu v1 lúc **đóng băng** thật sự có `cd`/`pwd` chạy được ⇒ **đưa vào** danh sách (vì "tương
  đương v1" phải so với **bản đóng băng**, không phải so với ý định đã revert).
- **(B)** Nếu nhật ký cho thấy chúng bị revert **trước** khi đóng băng ⇒ **loại**, ghi một dòng "đã
  xét và loại" kèm lý do.

**Đề xuất: (A)** — nguyên tắc "so với bản đóng băng" rõ ràng hơn "so với ý định". Nhưng đây là quyết
định của anh vì nó đổi định nghĩa mốc.

**Hiện trạng**: cả hai mục đang ở `deferred-K7` với `decision = "cho Agent 0"` — tức đã ghi nhận,
không im lặng.

---

### Q6 — KT-82a: số đo viết bằng chữ để qua luật máy — có làm lại không?

**Dữ kiện**:
- Có một luật máy (luật 1) cấm **số đo viết tay** trong tài liệu; số phải nằm trong khối trích
  nguyên văn từ log.
- Để đưa 58 con số ra khỏi văn xuôi, tôi viết chúng **bằng chữ** ("hai mươi mốt trên hai mươi
  mốt"). Như vậy qua được luật, **giữ được thông tin**, nhưng **luật không còn kiểm máy được** cho
  các giá trị đó — vì máy không đọc được chữ.
- Đây là đánh đổi thật: hoặc mất thông tin, hoặc mất khả năng kiểm máy.

**Lựa chọn**:
- **(A)** Làm lại đúng cách: đưa từng số vào khối `scope=` với dòng log nguyên văn. Tốn khoảng 2–3
  giờ, nhưng luật kiểm lại được.
- **(B)** Giữ nguyên, ghi nợ.

**Đề xuất: (A)** — nhưng **sau** khi K6 đóng, không chặn. Lý do: đây chính là tài liệu bằng chứng
đóng K6; để nó ngoài tầm kiểm máy là mâu thuẫn với lý do tồn tại của luật.

---

### Q7 — Sau K6: mở K7 (đóng lỗi v1) hay chuyển sang K4'/K5' (port kiến trúc)?

**Dữ kiện** — bảy lỗi v1:

| Mã | Lỗi | Trạng thái |
|---|---|---|
| N1 | Treo x86_64 sau `shell>` (~0,3–0,5 % boot) | **MỞ** — thiếu ca "đơn điệu sau migrate" + chéo hart |
| N2 | FS vi phạm read-your-own-write ở `MAX_IO=256` | **MỞ** — có test nhưng chưa có phép đo 50 boot / 0 đỏ |
| N3 | Pager nhận message lặp | **MỞ** — thiếu checker "đúng một lần" + ca 4 client |
| N4 | "Boot đỏ" thực chất là lỗi cổng đo | **ĐÓNG** |
| N5 | TLB cũ sau unmap trên x86 (AP→BSP + PCID) | **HOÃN K5'** — chỉ đóng được khi có x86 |
| N6 | `Ready` mà không trong runqueue | **ĐÓNG** |
| N7 | Kênh wake lệch 3 port | **MỞ** — crate có, test "3 port" chưa có |

Tức: **2 đóng · 1 hoãn K5' · 4 mở**. Bảng chi tiết `file:dòng` đã xong.

**Lựa chọn**:
- **(A)** Mở **K7 "Đóng lỗi v1"** với 4 lỗi N1/N2/N3/N7; N5 gắn K5'; mở luôn KT-81 (thiết bị giả
  nghiêm ngặt).
- **(B)** Chuyển sang **K4'/K5'** (port aarch64/x86_64) trước, để K7 sau.
- **(C)** Thứ tự khác.

**Đề xuất: (A).** Lý do: bốn lỗi v1 là **lỗi đúng đắn** (đọc được dữ liệu đã thu hồi, mất dữ liệu) —
để lâu trong khi mở thêm kiến trúc là nhân rủi ro lên ba lần. Và K7 đã có sẵn bảng.

---

### Q8 — Xác nhận hồi tố các quyết định tôi đã làm thay trong đêm

**Dữ kiện**: anh offline, người dùng ban hành luật "Agent 1 quyết định thay Agent 0" (2026-11-17).
Tôi đã làm 7 việc và ghi sổ, tất cả mang nhãn **CHỜ A0 XÁC NHẬN**:

| Mã | Việc | Có đổi kỳ vọng/ngưỡng không? |
|---|---|---|
| A1-188 | Tạm loại `v2/agent/**`, `v2/docs/**` khỏi build-inputs | **Có** — đổi định nghĩa cổng ⇒ chính là Q3 |
| A1-189 | Phát hiện soak chính thức chạy trên ảnh không feature; mở KT-90 đo thêm | Không — chỉ đo thêm |
| A1-190 | KT-89 + KT-90 xong | Không |
| A1-191 | Vá 3 lỗi thật trong cổng dấu vân tay | Không — sửa lỗi |
| A1-192 | KT-92/93/84/82a; **không** làm KT-91 vì đổi kỳ vọng harness | Không — từ chối đổi |
| A1-193 | Đêm 17/11: ảnh canonical, pre-measure, soak `--image`, 4 lỗ fail-open | Không |
| A1-194 | Đêm 18/11: 6 lệnh shell mới + review toàn dự án 19 phát hiện | Không — nhưng **đổi bề mặt sản phẩm** (15 lệnh) ⇒ Q4 |

**Đề xuất**: xác nhận cả 7, **trừ A1-188** (thay bằng quyết định Q3).

---

## PHẦN 3 — NẾU ANH CHỈ CÓ NĂM PHÚT

Ba việc quan trọng nhất, theo thứ tự:

1. **Q1** — K6 ĐẠT hay không. Đây là việc duy nhất chặn cả dự án. Đề xuất: **(B)**.
2. **Q3** — build-inputs có gồm `v2/agent/**` không. Nếu không chốt, mọi phép đo tiếp theo sẽ tự vô
   hiệu. Đề xuất: **(A)**.
3. **Q7** — sau K6 làm gì: K7 (đóng lỗi v1) hay K4'/K5' (port kiến trúc). Đề xuất: **(A)** K7 trước.

Bốn việc còn lại (Q2, Q4, Q5, Q6) có thể trả lời sau, **không chặn gì**.

---

## PHẦN 4 — SAU KHI ANH TRẢ LỜI

| Nếu anh chọn | Việc tôi làm ngay |
|---|---|
| Q1 = (A) hoặc (B) | Viết văn bản đóng K6 (bản nháp đã có, chỉ bỏ nhánh không chọn), ghi ba hạn chế, mở task đưa soak ảnh phát hành vào harness |
| Q1 = (C) | Ghi lý do vào sổ, chuyển sang việc không phụ thuộc |
| Q2 = (A) | Ghi caveat P20 vào văn bản đóng + mở mục nợ v2.1 |
| Q3 = (A) | Chốt định nghĩa build-inputs vào luật, gỡ nhãn "tạm" của A1-188 |
| Q4 = (A) | Sửa 6 mục trong `spec/v1-parity.toml`, chạy lại cổng, báo cáo |
| Q5 = (A) hoặc (B) | Cập nhật `spec/v1-parity.toml` + chạy lại parity |
| Q6 = (A) | Làm lại 58 số vào khối `scope=` (sau K6) |
| Q7 = (A) | Mở K7 + KT-81, cập nhật QUEUE |

---

## PHỤ LỤC — NGUỒN GỐC TỪNG DỮ KIỆN

*(để sau này đối chiếu; **không** cần đọc để quyết)*

| Dữ kiện | Nguồn trong repo |
|---|---|
| 120 xong / 3 bị chặn | `./v2/bin/v2 status` |
| Ảnh `a0287e4b`, 205.440 B | `v2/artifacts/a0287e4b….bin`; `v2/agent/JOURNAL/kt126-journal.md` |
| Ma trận 21/21, parity, shell-edge 8/8, persist | `v2/agent/JOURNAL/kt126-journal.md` |
| Sáu tiêu chí K6 | `v2/agent/reports/K6-EVIDENCE-INDEX.md` |
| Ba hạn chế | `K6-EVIDENCE-INDEX.md` §3 |
| Bảy lỗi v1 | `v2/agent/JOURNAL/kt83-table.md` |
| B6 | `v2/agent/BLOCKED-CHO-A0.md` §B6 |
| `cd`/`pwd` | `BLOCKED-CHO-A0.md` §B4 |
| Sổ quyết định A1 | `v2/agent/reports/A1-decision-log.md` |
| Bản nháp văn bản đóng K6 | `v2/agent/reports/K6-CLOSURE-DRAFT.md` |

---

## §1.5b — KẾT QUẢ SOAK TRÊN ẢNH MỚI NHẤT (nguyên văn)

```text
# KT-90 soak xong — 2026-11-18T01:31:18Z
# wall=370s tổng=1000/1000 ok=1000 fail=0 (370s)
# timeout-shell=0 panic=0 help-response=1000/1000 rx_bytes_min=10
# ảnh: sha256=a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13 changed=0 bytes=205440
# infra after: qemu_con_sot_total=0 qemu_ppid1=0 (trước: 0/0)
#   boot-1.log:91:shell>
#   boot-1.log:92:# shell: help cmds=15
#   boot-1.log:95:# shell: exit code=0
#   boot-1.log:100:# product halt: shell_exit=0 fault_seen=1 events=3 ...
# kt90: 1000/1000 vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch
# kt90: PASS
```

**Đọc dòng này thế nào**: 1.000 lần boot liên tiếp trên ảnh phát hành, mỗi lần đều thật sự vào được
shell, shell trả lời được lệnh, và thoát sạch. Không lần nào treo, không lần nào PANIC, ảnh không bị
ghi đè giữa chừng.

> **Ghi chú vận hành**: phép soak này do lead chạy trực tiếp, **chưa** ghi vào `STATE.md`/`JOURNAL`
> (đó là tệp điều khiển của Agent 2). Log nằm ở đường dẫn trên; khi tiếp tục, việc đầu tiên là ghi
> nó vào sổ.
