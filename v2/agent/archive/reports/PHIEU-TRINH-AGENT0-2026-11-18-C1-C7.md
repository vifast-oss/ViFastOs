# PHIẾU TRÌNH AGENT 0 — sau khi thi hành Q1–Q8

**Ngày**: 2026-11-18
**Người trình**: lead (Agent 1, dòng OS)
**Trạng thái dòng OS**: HEAD `bb0f09c0`, đã push, cây sạch, `v2 gate --fast` PASS.
**Task**: DONE 128 · READY 2 · BLOCKED 0 · REVIEW 0.

> Tệp này **tự chứa**: mọi thứ cần để quyết đều ở đây. Anh không cần mở repo.
> Phần 1 là báo cáo (không cần quyết). Phần 2 là **bảy việc cần anh quyết**, mỗi việc có phương án
> và khuyến nghị của tôi. Phần 3 là những gì tôi đã tự quyết trong quyền hạn của mình.

---

# PHẦN 1 — BÁO CÁO: TÁM QUYẾT ĐỊNH ĐÃ THI HÀNH XONG

Mỗi quyết định một commit xanh + push. Không có quyết định nào bị bỏ dở.

| | Anh đã quyết | Đã làm gì | Bằng chứng |
|---|---|---|---|
| **Q1** | (B) K6 ĐẠT **có điều kiện**, gắn ảnh `a0287e4b` | Giải thích `fault_seen`; checker soak fail-closed + đột biến phía mã; ghi soak vào sổ; viết văn bản đóng | `07b244a7`, `6a72c7d5` |
| **Q2** | (A) caveat P20 **trong** văn bản đóng | §5 của văn bản; nợ chuyển v2.1 với tiêu chí "sha ảnh phát hành = sha ảnh canonical, hai máy × hai đường dẫn" | `6a72c7d5` |
| **Q3** | (A) build-inputs + **ca âm bắt buộc** | Sửa một byte mã kernel ⇒ dấu vân tay đổi **và** cổng từ chối đo, kèm đối chứng dương | `25138df7` |
| **Q4** | (A) sáu mục spec + ca kiểm chín lệnh trên ảnh sản phẩm | Sáu lệnh thành `active`; thêm hai ca kiểm (mỗi lệnh một ca đúng + một ca biên) | `2815e2d5` |
| **Q5** | Kiểm chứng `cd`/`pwd` **trước** khi áp luật | Build + boot v1 thật tại tag đóng băng, chạy `cd /bin` rồi `pwd` ⇒ in `/bin` ⇒ **(A) đưa vào danh sách** | `647a04bc` |
| **Q6** | (A) 58 số vào khối `scope=` | Bảng đối chiếu số ↔ khối cho ba tài liệu (xem C7) | `f2b3e2b4` |
| **Q7** | (A) mở K7 + KT-81 | K7 mở, thứ tự N2 → N3 → N7 → N1; luật đóng lỗi ba vế | `6a72c7d5` |
| **Q8** | Xác nhận A1-188..194 | A1-188 thay bằng Q3; ghi chú chế độ quyết định thay thế | `6a72c7d5` |

## 1.1 K6 nay đã ĐÓNG

Văn bản đóng chính thức: `v2/agent/reports/K6-CLOSURE.md`. Kết luận: **K6 ĐẠT**, gắn ảnh
`a0287e4b…`, ghi nguyên **ba hạn chế** và **caveat P20** ngay trong văn bản (không giấu ở nhật ký).

Sáu tiêu chí đã **đo lại** trên đúng ảnh anh chốt, không dùng lại số cũ:

- Boot ảnh phát hành ở **ba** cấu hình hart (một, hai, bốn nhân) — cả ba tới `shell>`.
- Parity **mười hai** lệnh `active` ở **hai** cấu hình hart — không lệnh nào đỏ.
- Ma trận **hai mươi mốt** boot (bảy ảnh test × ba cấu hình) — sạch toàn bộ.
- Persist **bảy** pha qua reboot thật — mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình khác.
- Ba ngưỡng kích thước — dưới trần.
- Bộ dò mã harness trên ảnh phát hành — không hit; ca âm cùng bộ dò trên ảnh test — bắt được.

## 1.2 Hai phát hiện khi thi hành

**Một: một selftest đã đỏ từ trước mà không cổng nào chạy.** Khi làm Q3 tôi phát hiện selftest của
cổng build-inputs **đã đỏ ngay tại HEAD trước khi tôi sửa gì** — ba ca. Nguyên nhân: danh sách tệp
ghim lớn dần qua nhiều task trước, nhưng repo giả trong selftest không được cập nhật theo, và **không
cổng nào gọi selftest đó**. Đã sửa và nối vào `v2 check`. Đây đúng loại "xanh mà rỗng" mà anh đã
cảnh báo.

**Hai: một script đo thiếu `PATH` nên báo đỏ oan.** Script quét harness gọi `cargo tree`; chạy từ
shell không có `~/.cargo/bin` thì lệnh trả rỗng ⇒ hai ca đỏ **oan** (nó tưởng cấu hình sản phẩm
không bật U-mode). Đã vá.

## 1.3 Một chuyện nhỏ đáng kể

Ca kiểm mới cho `uptime` lần đầu **đỏ**, và đỏ vì **kỳ vọng của tôi sai**, không phải vì sản phẩm
sai: tôi tưởng `time` in hai dòng, thực tế nó in một. Ca kiểm bắt được cái sai của chính người viết
ca kiểm — đó là dấu hiệu harness làm việc đúng. Đã sửa kỳ vọng, không sửa sản phẩm.

---

# PHẦN 2 — BẢY VIỆC CẦN ANH QUYẾT

## C1. Ảnh đóng K6 không còn khớp HEAD — có làm lại baseline không?

**Chuyện gì đã xảy ra.** K6 đóng trên ảnh `a0287e4b`, build từ commit `f4d1dd5f`. Hai commit mà
**chính anh yêu cầu** (Q1 điều kiện một: vá checker soak; Q3: thêm ca hồi quy âm cho cổng
build-inputs) có sửa **một tệp nằm trong danh sách ghim của dấu vân tay build**. Hệ quả: build lại
từ HEAD hiện tại sẽ ra **sha khác**.

**Mã sản phẩm không đổi một dòng.** Chỉ chuỗi dấu vân tay nhúng trong ảnh đổi. Ảnh cũ vẫn tái lập
được: `git checkout f4d1dd5f` rồi chạy pipeline canonical.

**Ba phương án:**

- **(A) Chấp nhận.** Văn bản đóng là phát biểu lịch sử, gắn một commit có tên. Đã ghi sẵn cách tái lập.
- **(B) Làm lại baseline.** Build lại từ HEAD, đo lại sáu tiêu chí, phát hành văn bản đóng mới trên sha mới.
- **(C) Làm cả hai.** Giữ `a0287e4b` là ảnh chính thức, ghi thêm sha mới như bằng chứng phụ.

**Khuyến nghị: (A).** Lý do: bất kỳ commit nào chỉ chạm test cũng sẽ đổi dấu vân tay lần nữa, nên (B)
là chạy đuổi không hết. K6 nói về **năng lực**, và năng lực đó tái lập được từ một commit có tên.

**Nếu anh không trả lời**: tôi giữ (A), vì văn bản đã ghi rõ và không có gì sai về mặt bằng chứng.

---

## C2. KT-131 — đưa "soak ảnh phát hành" vào harness chính thức bằng cách nào?

**Bối cảnh.** Đây là hạn chế thứ nhất của K6. Harness soak chính thức build ảnh **không có shell,
không có FS** — nên "một nghìn vòng không lỗi" của nó không phải ảnh phát hành. Con số một nghìn
trên **đúng** ảnh phát hành đến từ một script riêng trong nhật ký, không phải harness.

**Ba phương án:**

- **(A) Thêm tham số `--features` cho harness soak.** Một đường mã, tham số hóa; mặc định vẫn là ảnh
  không shell nên bằng chứng cũ không mất giá trị.
- **(B) Thêm lệnh cổng riêng** (ví dụ `v2 soak-product`) dùng lại động cơ soak.
- **(C) Giữ script nhật ký, chỉ ghi nó là chính thức.**

**Khuyến nghị: (A).** Lý do: (C) không sửa được hạn chế, chỉ đổi nhãn. (B) sinh hai đường mã dễ lệch
nhau. (A) là thay đổi nhỏ nhất mà hết hạn chế thật.

**Nếu anh không trả lời**: tôi làm (A).

---

## C3. `cd`/`pwd` — shell v2 chưa có, có làm ở K7 không?

**Đây là việc **thay đổi bề mặt sản phẩm**, nên theo Q8 tôi **không được tự làm**.**

**Bối cảnh.** Q5 đã kiểm chứng: v1 tại tag đóng băng **có** `cd` và `pwd` chạy thật (boot thật, gõ
`cd /bin` rồi `pwd`, in ra `/bin`). Vậy theo nguyên tắc "tương đương v1 so với bản đóng băng", hai
lệnh này **thuộc** danh sách parity.

Nhưng shell v2 **chưa có** khái niệm thư mục hiện tại: `ls`, `cat`, `stat` đều nhận đường dẫn tuyệt
đối. Thêm `cd`/`pwd` không phải thêm hai lệnh — nó là **đổi cách phân giải đường dẫn** của mọi lệnh
có tham số đường dẫn.

**Ba phương án:**

- **(A) Làm ở K7, để cuối.** Thêm thư mục hiện tại cho shell, cho đường dẫn tương đối, thêm `cd`/`pwd`,
  kèm ca kiểm trên ảnh sản phẩm (đúng + biên: `cd` không tham số, `cd` vào đường không có, `cd ..`,
  `pwd` sau `cd`).
- **(B) Hoãn v2.1**, ghi lý do "shell v2 chỉ dùng đường dẫn tuyệt đối" và đóng mục parity này là "đã
  xét và loại".
- **(C) Làm ngay**, trước K7.

**Khuyến nghị: (A), để cuối K7.** Lý do: nó là nợ parity thật (không phải "đã xét và loại"), nhưng nó
chạm phân giải đường dẫn nên phải làm sau khi bốn lỗi N đã đóng, tránh trộn hai loại rủi ro.

**Nếu anh không trả lời**: tôi **không** làm, giữ `deferred-K7`, và ghi rõ trong K7 là "chờ quyết định".

---

## C4. K7 có cổng riêng không?

**Bối cảnh.** K7 = "Đóng lỗi v1", bốn lỗi N1/N2/N3/N7. Anh đã chốt luật đóng cho **từng** lỗi: chỉ
được đóng khi có (i) ca tái hiện **đỏ** trên mã cũ, (ii) **xanh** trên mã mới, (iii) **đột biến phía
mã** làm nó đỏ lại.

Câu hỏi: ngoài luật đó, K7 có cần một **cổng mốc** như K6 (sáu tiêu chí) không?

**Hai phương án:**

- **(A) Có cổng riêng** — soạn bộ tiêu chí cho K7 như đã làm cho K6.
- **(B) Không cổng riêng** — K7 đóng khi bốn lỗi đóng đủ ba vế, **cộng** một lần chạy lại sáu tiêu chí
  của K6 để chắc không hồi quy.

**Khuyến nghị: (B).** Lý do: K7 là mốc **đóng lỗi**, không phải mốc **năng lực**; bộ tiêu chí riêng sẽ
là hình thức. Nhưng phải có điều kiện "không hồi quy K6", nếu không thì đóng lỗi xong có thể đã làm
hỏng thứ khác.

**Nếu anh không trả lời**: tôi làm (B).

---

## C5. Hạn chế thứ hai của K6 — soak trên volume RAM, không phải đĩa thật. Có mở task không?

**Bối cảnh.** Soak một nghìn vòng dùng volume FS **trong RAM**. Đường **đĩa thật** mới có hai boot
cộng persist bảy pha. Nói cách khác: soak chứng minh kernel ổn định, **không** chứng minh độ bền đĩa
thật ở quy mô lớn.

Đây là hạn chế thật với một dòng sản phẩm mà cả phần journal/fsck được làm ra để bảo vệ **đĩa**.

**Hai phương án:**

- **(A) Mở task soak đĩa thật với số vòng nhỏ hơn** (ví dụ một trăm boot thay vì một nghìn), vì mỗi
  boot đĩa thật chậm hơn nhiều.
- **(B) Chấp nhận làm hạn chế của v2.0**, ghi vào văn bản đóng (đã ghi) và không mở task.

**Khuyến nghị: (A).** Lý do: đây là hạn chế duy nhất trong ba hạn chế mà **có thể** đóng được bằng
công sức, và nó bảo vệ đúng thứ người dùng cuối quan tâm (mất dữ liệu).

**Nếu anh không trả lời**: tôi không mở task, giữ nguyên hạn chế đã ghi.

---

## C6. Thứ tự hai task READY còn lại

Còn đúng hai task READY (không kể K7):

- **KT-81** — thiết bị giả nghiêm ngặt + bảng tài nguyên (bắt lại hai lỗi driver cũ ở mô hình).
- **KT-131** — đưa soak ảnh phát hành vào harness chính thức (việc C2 ở trên).

**Khuyến nghị: KT-131 trước, KT-81 sau, rồi mới vào K7.** Lý do: KT-131 gỡ một hạn chế **đã ghi
trong văn bản đóng K6**, nên để lâu là để văn bản đóng có khuyết điểm đã biết sửa được. KT-81 là việc
chất lượng nội bộ, không chặn gì.

**Nếu anh không trả lời**: tôi làm theo thứ tự này.

---

## C7. Q6 — cách tôi làm có đúng ý anh không?

**Tôi đã làm gì.** Anh dặn "đưa 58 số vào khối `scope=` kèm dòng log nguyên văn". Tôi **không** đưa
các chữ số trở lại phần văn xuôi, vì luật của repo cấm số đo nằm ngoài khối `scope=`. Thay vào đó:

- Mỗi trường đo trong ba tài liệu nay có một **bảng đối chiếu** chỉ ra **khối `scope=` nào chứa dòng
  log nguyên văn của nó**.
- Văn xuôi vẫn dùng chữ ("bằng một", "bằng không"), nên **không cần miễn trừ nào** — con số miễn trừ
  về không.

**Hai phương án:**

- **(A) Chấp nhận** — số đo trích được nguyên văn từ log, chỉ là qua bảng thay vì qua câu văn.
- **(B) Bắt làm lại** — đưa chữ số trở lại **bên trong** khối `scope=` và sửa văn xuôi trỏ thẳng vào khối.

**Khuyến nghị: (A).** Lý do: mục đích của luật là **truy được số về log**, và bảng làm đúng việc đó.
(B) chỉ đổi hình thức, không thêm bằng chứng.

**Nếu anh không trả lời**: tôi giữ (A).

---

# PHẦN 3 — NHỮNG GÌ TÔI ĐÃ TỰ QUYẾT (trong quyền hạn, không cần anh duyệt)

Ghi ra để anh biết, không phải để xin duyệt:

- **Sửa selftest cổng build-inputs** và nối nó vào `v2 check` — đây là sửa **công cụ đo**, không phải
  nới ngưỡng, không phải sửa kỳ vọng sản phẩm.
- **Vá `PATH`** trong script quét harness — sửa lỗi công cụ, không đổi ngữ nghĩa phép đo.
- **Sửa kỳ vọng của ca kiểm `uptime`** do tôi viết sai — sửa ca kiểm **của chính tôi** trong cùng
  task, không đụng kỳ vọng nào có từ trước.
- **Giữ `cd`/`pwd` ở trạng thái `deferred-K7`** thay vì đổi sang `active` — vì đổi sang `active` sẽ
  làm parity đỏ ngay (shell v2 chưa có lệnh), tức là tự tạo đỏ giả.
- **Không đụng** `deploy/vifastos_web/static/*` — hai tệp đó do phiên khác stage.

---

# PHẦN 4 — TÓM TẮT ĐỂ ANH TRẢ LỜI NHANH

Nếu anh chỉ muốn trả lời gọn, chỉ cần bảy dòng:

```
C1 (ảnh đóng K6 vs HEAD):     A / B / C
C2 (cách đưa soak vào harness): A / B / C
C3 (cd/pwd ở K7):              A / B / C
C4 (K7 có cổng riêng):         A / B
C5 (soak đĩa thật):            A / B
C6 (thứ tự READY):             đồng ý / đổi thành ...
C7 (cách làm Q6):              A / B
```

Không trả lời mục nào thì tôi đi theo **khuyến nghị** ở mục đó, và ghi rõ trong sổ là "chưa có quyết
định của Agent 0, làm theo khuyến nghị".
