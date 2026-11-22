# Bất biến của phép kiểm — bốn lớp lỗi đã bắt được trong một tuần

Tệp này là **luật viết phép kiểm** cho dòng OS. Nó không phải thiết kế mới: nó ghi lại **bốn
lớp lỗi** mà chính bộ kiểm của repo đã mắc, mỗi lớp kèm **ca thật** và **cách sửa**. Mục tiêu: lần
sau không lặp lại.

Bối cảnh: trong một tuần, **nhiều lần** một phép kiểm đỏ vì **giả định sai của phép kiểm**, không
vì lỗi của sản phẩm — mỗi lần tốn một vòng soak dài để tìm ra. Cộng thêm hai lần phép kiểm **xanh
oan**. Tổng: năm sự cố, cùng một gốc.

> **Không** chép số đo vào tệp này (luật `verify-numbers` E1). Mọi con số đo được nằm trong
> `v2/agent/JOURNAL/kt*.md` và log tương ứng; tệp này chỉ dẫn luật + đường dẫn.

---

## 1. Luật 1 — bất biến **không** được phụ thuộc thứ tự/thời điểm giữa các hart

> Nếu điều kiện có thể **đảo** khi chỉ **đổi thứ tự tick** giữa các hart (không đổi mã sản phẩm),
> thì đó là **giả định thời điểm**, không phải **bất biến**.

Đây là lớp lỗi **nặng nhất** và **lặp nhiều nhất**.

### 1.1 Ca thứ nhất — `last > first` (KT-308, soak x86_64)

Phép kiểm: *trong cửa sổ đo, tác vụ ring (CPL ba) phải chạy được **nhiều** lệnh hơn lần đo
đầu* — so sánh `last` với `first`.

- **Đỏ lần đầu**: `first` khác không và `last` **bằng** `first`. Đã "sửa" thành "`last` không
nhỏ hơn `first`".
- **Đỏ lần sau**: `first` **bằng không** và `last` lớn hơn `first` — **ảnh gương** của lần đầu.
- **Cơ chế**: PIT chạy ở tần số cố định ⇒ cửa sổ rất ngắn. Khi host deschedule hart đúng lúc
  `iretq`, tick **kế tiếp** đã pending ⇒ hart vào lại ngắt **ngay** ⇒ **không** lệnh user nào chạy
  được trong **mọi** cửa sổ, bất kể `first`/`last` đo lúc nào. Hai lần đỏ là hai mặt của **cùng
  một** race.
- **Sửa**: bỏ **cả** hai điều kiện trên. Bất biến còn lại **không** phụ thuộc thời điểm:
  `n` khác không (tác vụ **đã** chạy thật) ∧ `resumed` ít nhất một (được `schedule()` chọn
  **lại**) ∧ `cs` ở CPL user ở **cả hai** lần preempt ∧ `last` **không nhỏ hơn** `first`
  (chỉ còn là **kiểm tỉnh táo**, không phải điều kiện chính).
- Nguồn: `v2/agent/JOURNAL/kt308-x86_64-soak2.md`.

### 1.2 Ca thứ hai — `first[0] != first[1]` (KT-310 → KT-313, soak x86_64)

Phép kiểm: *hai hart schedule phải **chọn tác vụ khác nhau** ở lần chọn đầu tiên*.

- **Đỏ** ở một phần nhỏ số boot với dòng `# sched: multi-hart` có `first` trùng nhau giữa hai hart —
  `double` bằng không (không double-pick), mọi hart đều chọn, `rounds` đủ lớn: **mọi điều kiện
  thật đều đạt**.
- **Cơ chế**: hàng đợi FIFO ba tác vụ; mỗi hart **trả tác vụ cũ về trước** rồi mới pop. Nếu hart
  phụ tick trước và kịp **đủ vòng** trước lượt đầu của BSP, tác vụ đầu đã quay lại **đầu** hàng
  đợi đúng lúc BSP pop lần đầu. Chuỗi các lượt `R…` khớp **chính xác** dãy `picks` của cả bốn boot
  đỏ.
- **Sửa**: thay bằng bất biến **đếm được**, không phụ thuộc thời điểm: `diff` khác không — **tồn tại**
  ít nhất một lượt hart phụ chọn tác vụ **khác** tác vụ BSP **đang chạy** tại thời điểm đó. Đó vẫn
  chứng minh đúng điều cần ("hai hart cùng schedule trên một hàng đợi và chọn khác nhau") mà
  **không** giả định thứ tự.
- **Bằng chứng bản sửa đúng**: soak lần sau — khuôn `first` trùng **vẫn xuất hiện** ở một phần
  nhỏ số boot và **mọi** boot đó đều XANH ở ca TAP `multi-hart`; `diff` khác không đúng ở
  **toàn bộ** số boot;
  `double` bằng không đúng ở **toàn bộ** số boot. (`v2/agent/JOURNAL/kt313-x86_64-soak3.md`)
- **Giữ lại để chẩn đoán**: `first=[…]` **vẫn in** trong log — nó là **manh mối**, không phải điều
  kiện. Bỏ hẳn khỏi log là bỏ mất khả năng thấy race.

### 1.3 Ca thứ ba — `first[0] != first[i]` với **bốn** hart (KT-315 → KT-317)

Cùng lỗi, mở rộng lên bốn hart: điều kiện đổi thành "BSP chọn khác **mọi** hart phụ". Với hàng đợi
bốn tác vụ, hart phụ chỉ cần kịp thêm **một** vòng là đủ để tái hiện. KT-317 hợp nhất KT-313 vào
KT-315 ⇒ `mh_ok(rounds, picks, double, diff)`, `MH_DIFF` đếm cho **cả ba** hart phụ.

**Bài học phụ (KT-315)**: "bốn hart phải chọn **bốn giá trị đôi một khác**" là **giả định sai kiểu
KT-308** — `first` có hai hart phụ trùng nhau hoàn toàn hợp lệ. Đừng đòi nhiều hơn thứ được thiết kế bảo đảm.

### 1.4 Cách nhận ra lớp lỗi này **trước** khi soak

Trả lời ba câu:

Một: **Thứ tự** nào phép kiểm giả định? (hart nào tick trước, lần chọn nào xảy ra trước)
Hai: Thứ tự ấy có **cơ chế bảo đảm** không — hay chỉ "thường đúng"? (cổng khởi động? cổng giữ? khoá?
   `MH_HOLD_TICKS`?)
Ba: Nếu **đảo** thứ tự ấy, phép kiểm có **đỏ oan** không?

Nếu câu hai là "thường đúng" ⇒ **đổi bất biến**, đừng tăng timeout, đừng thêm retry, đừng siết cổng
giữ để "làm nó tất định" (KT-315 đã siết `MH_HOLD_TICKS` và race **vẫn còn** — vì race nằm ở **phía
BSP trả tác vụ**, không phải ở phía hart phụ chờ).

---

## 2. Luật 2 — ca âm phải **chứng minh được** là đã đột biến

Ca âm (negative case) xanh vì `sed` **không khớp** còn tệ hơn không có ca âm: nó tạo **cảm giác an
toàn giả**.

### 2.1 Ca thật — KT-303, ca âm xanh oan

Ca âm dùng `sed` khớp **đúng một** giá trị của `attempts` để phá dòng evidence. Nhưng lần chạy
thật có thể in giá trị **khác** ⇒ `sed` **không khớp** ⇒ không đột biến ⇒ ca "âm" **xanh** — và nó
xanh vì **không có gì bị phá**.

**Sửa**: mẫu `sed` phủ **hết** miền giá trị (ví dụ lớp ký tự của mọi giá trị quan sát được),
không chỉ giá trị đã thấy **một lần**.

### 2.2 Luật

- Mẫu `sed`/biến đổi của ca âm phải phủ **toàn miền** giá trị hợp lệ của trường bị phá, không chỉ
  giá trị **đã quan sát một lần**.
- Tốt nhất: ca âm khẳng định **đã phá** (so `grep -c` trước/sau) rồi mới khẳng định bộ kiểm **ĐỎ**.
  Nếu không phá được ⇒ ca âm phải **ĐỎ** (báo "không đột biến được"), không được im lặng xanh.
- Ranh giới: đây **không** phải nới ngưỡng, chỉ là làm ca âm **thật sự** là ca âm.

---

## 3. Luật 3 — kiểm **giá trị cực đại**, không chỉ giá trị thường

### 3.1 Ca thật — KT-312, `debug_assert!` sai miền giá trị

Bản nháp thu hẹp `Meta.untyped` từ `Option<usize>` sang kiểu hẹp (`u8`) đặt
`debug_assert!(u < MAX_UNTYPEDS)`. **Sai**: `untyped` là chỉ số **object** (`< MAX_OBJECTS`), không
phải chỉ số region (`< MAX_UNTYPEDS`). Test "giá trị cực đại" bắt ngay; nếu chỉ test giá trị nhỏ
thì lỗi lọt vào ảnh.

Cùng lớp: test `kt313_first_collision_is_not_a_failure` — **tái hiện đúng chuỗi race** thay vì chỉ
kiểm "trường hợp thường". Một test hồi quy cho race phải dựng **đúng thứ tự** đã gây race.

### 3.2 Luật

- Kiểm **cả hai đầu**: giá trị **cực đại** và giá trị **cực tiểu** (rỗng, `None`), không chỉ giá
  trị điển hình.
- Khi thu hẹp kiểu/bitfield: viết `assert!` **miền giá trị chứng minh được** ngay cạnh
  `assert_eq!(size_of)`.
- Khi sửa một race: viết test **tái hiện** đúng chuỗi đã gây race, và nó phải **đỏ** trên mã cũ.

---

## 4. Luật 4 — hằng trong phép kiểm phải là **phép tính có ghi**, không phải số chép tay

### 4.1 Ca thật — KT-290, tiền đề test sai sau khi thêm tính năng

Sim `kt62_cap_loi_dung_ma` kỳ vọng `Call` trên cap TCB trả `NotSupported`. KT-290 cài
`Tcb.Configure` ⇒ cùng lời gọi giờ trả `InvalidArgument`. Test đỏ **vì tiền đề cũ**, không vì hồi
quy. Test nào khẳng định "tính năng X **chưa** cài" phải **đổi** khi X được cài.

### 4.2 Ca thật — KT-309 × KT-312, hai task cùng vòng sửa cùng một pin

KT-309 thêm trường vào `Message`, KT-312 thu hẹp `Meta`. Riêng lẻ mỗi bên sửa `tls_tests.rs` đúng;
**hợp nhất** phải **cộng cả hai** và đổi trần `footprint` — nếu chỉ "sửa cho xanh" thì pin thành dấu
cao su.

### 4.3 Luật

- Mọi hằng trong phép kiểm phải **suy ra** từ `MAX_*`/`size_of`/hằng ABI sinh tự động — **không**
  chép số. Khi phải chép một trần, ghi **một** chỗ và mọi nơi khác trỏ về.
- Khi **giảm** một thành phần: dùng **đại số có dấu** (`isize`), ghi rõ khoản tăng **và** khoản
  giảm. "Thu hồi" phải là phép trừ hiện trong test, không phải một số mới.
- Trần (pin/budget) **chỉ được hạ**, không được nâng để qua cổng. Nếu hợp nhất buộc phải đổi trần,
  trần mới = trần cũ + phần **tăng có ghi** của task kia, và **không** hạ được mức bảo đảm của trần
  cũ (KT-312 + KT-311 vẫn chốt đúng "thu hồi ≥ mức KT-312 công bố").
- Đột biến để kiểm pin **có răng**: tăng một `MAX_*` ⇒ pin phải **ĐỎ** (KT-312 M2).

---

## 5. Checklist trước khi thêm/sửa một phép kiểm

| # | Câu hỏi | Nếu "không" |
|---|---|---|
| Một | Bất biến có đảo khi chỉ đổi **thứ tự tick** giữa các hart? | Đổi bất biến (§1) |
| Hai | Đã trả lời "cơ chế nào **bảo đảm** thứ tự ấy"? | Đổi bất biến (§1.4) |
| Ba | Ca âm **chứng minh** đã phá được dòng bị phá? | Sửa mẫu `sed`, phủ toàn miền (§2) |
| Bốn | Đã kiểm **giá trị cực đại** và cực tiểu? | Thêm ca (§3) |
| Năm | Mọi hằng là **phép tính** từ hằng nguồn, có dấu? | Suy ra, đừng chép (§4) |
| Sáu | Test "tính năng X **chưa** cài" còn đúng không? | Đổi tiền đề (§4.1) |
| Bảy | Đột biến của phép kiểm **có răng** (đỏ đúng lý do)? | Chạy lại với đột biến (§4.3) |
| Tám | Có **giữ** trường chẩn đoán trong log (không chỉ điều kiện)? | Giữ lại `first=`, `attempts=`… (§1.2) |

## 6. Liên quan

- `v2/docs/k6-harness.md` — hợp đồng `qemu_guard.sh`, `qg_infra` / `MEASUREMENT_INVALID` (D2-116):
  số **thời gian** chỉ dùng khi không có QEMU lạ (`qemu_foreign` bằng không).
- `v2/agent/DECISIONS.md` — D2-05 (test đỏ trước), D2-07 (không retry để lấy xanh), D2-84 (pin
  `Machine`), D2-116 (đo hợp lệ), D2-205 (K3 đóng).
- Journal ca thật: `kt298-x86_64-gp.md`, `kt303-*.md`, `kt308-x86_64-soak2.md`,
  `kt312-machinesize.md`, `kt313-x86_64-soak3.md`, `kt315-x86_64-fourhart.md`,
  `kt317-merge313.md`.
