# Bất biến của phép kiểm — bốn lớp lỗi đã bắt được trong tuần 2026-11-16..22

Tệp này là **luật viết phép kiểm** cho dòng OS. Nó không phải thiết kế mới: nó ghi lại **bốn lớp
lỗi** mà chính bộ kiểm của repo đã mắc, mỗi lớp kèm **ca thật** và **cách sửa**. Mục tiêu: lần thứ
tư không xảy ra.

Bối cảnh: trong tuần này, **ba lần** một phép kiểm đỏ vì **giả định sai của phép kiểm**, không vì
lỗi của sản phẩm — mỗi lần tốn một vòng soak 600 boot để tìm ra. Cộng thêm hai lần phép kiểm
**xanh oan**. Tổng: năm sự cố, cùng một gốc.

---

## 1. Luật 1 — bất biến **không** được phụ thuộc thứ tự/thời điểm giữa các hart

> Nếu điều kiện có thể **đảo** khi chỉ **đổi thứ tự tick** giữa các hart (không đổi mã sản phẩm),
> thì đó là **giả định thời điểm**, không phải **bất biến**.

Đây là lớp lỗi **nặng nhất** và **lặp nhiều nhất** — ba lần.

### 1.1 Ca thứ nhất — `last > first` (KT-308, soak x86_64)

Phép kiểm: *trong cửa sổ đo, tác vụ ring 3 phải chạy được **nhiều** lệnh hơn lần đo đầu*
(`last > first`).

- **Đỏ lần 1** — boot 331: `first>0, last==first`. Đã "sửa" thành `last >= first`.
- **Đỏ lần 2** — boot 231: `first=0, last>first` — **ảnh gương** của lần 1.
- **Cơ chế**: PIT 100 Hz ⇒ cửa sổ 10 ms. Khi host deschedule hart đúng lúc `iretq`, tick **kế tiếp**
  đã pending ⇒ hart vào lại ngắt **ngay** ⇒ **0** lệnh user chạy được trong **mọi** cửa sổ, bất kể
  `first`/`last` đo lúc nào. Hai lần đỏ là hai mặt của **cùng một** race.
- **Sửa**: bỏ **cả** `last > first` **và** `first > 0`. Bất biến còn lại **không phụ thuộc thời
  điểm**: `n > 0` (tác vụ **đã** chạy thật) ∧ `resumed >= 1` (được `schedule()` chọn **lại**) ∧
  `cs == 0x1B` ở **cả hai** lần preempt (đúng là CPL3) ∧ `last >= first` (chỉ còn là **kiểm tỉnh
  táo**, không phải điều kiện chính).

### 1.2 Ca thứ hai — `first[0] != first[1]` (KT-310 → KT-313, soak x86_64)

Phép kiểm: *hai hart schedule phải **chọn tác vụ khác nhau** ở lần chọn đầu tiên*.

- **Đỏ** 5/600 boot (KT-313 soak lần 1) với `# sched: multi-hart harts=2 rounds=6 picks=[2, 4]
  first=[a, a] double=0` — `double=0` (không double-pick), mọi hart đều chọn, `rounds>=2`: **mọi
  điều kiện thật đều đạt**.
- **Cơ chế**: hàng đợi FIFO **3** tác vụ `[a,b,u]`; mỗi hart **trả tác vụ cũ về trước** rồi mới pop.
  Nếu hart phụ tick trước và kịp **3** lượt trước lượt đầu của BSP, `a` đã quay lại **đầu** hàng đợi
  đúng lúc BSP pop lần đầu. Chuỗi `R1..R6` khớp **chính xác** `picks=[2, 4]` của cả 4 boot đỏ.
- **Sửa**: thay bằng bất biến **đếm được**, không phụ thuộc thời điểm: `diff >= 1` — **tồn tại**
  ít nhất một lượt hart phụ chọn tác vụ **khác** tác vụ BSP **đang chạy** tại thời điểm đó. Đó vẫn
  chứng minh đúng điều cần ("hai hart cùng schedule trên một hàng đợi và chọn khác nhau") mà
  **không** giả định thứ tự.
- **Bằng chứng bản sửa đúng**: soak 2 (KT-313) — `first=[a, a]` xuất hiện **8/600** boot và **cả 8
  `ok 25` XANH**; `diff` phân bố 1..6, `diff >= 1` **600/600**; `double=0` **600/600**.
  (`v2/agent/JOURNAL/kt313-x86_64-soak3.md`)
- **Giữ lại để chẩn đoán**: `first=[…]` **vẫn in** trong log — nó là **manh mối**, không phải điều
  kiện. Bỏ hẳn khỏi log là bỏ mất khả năng thấy race.

### 1.3 Ca thứ ba — `first[0] != first[i]` với **bốn** hart (KT-315 → KT-317)

Cùng lỗi, mở rộng lên bốn hart: điều kiện đổi thành "BSP chọn khác **mọi** hart phụ". Với hàng đợi
**bốn** tác vụ `[a,b,u,c]`, hart phụ chỉ cần kịp **bốn** lượt là đủ để tái hiện. KT-317 hợp nhất
KT-313 vào KT-315 ⇒ `mh_ok(rounds, picks, double, diff)`, `MH_DIFF` đếm cho **cả ba** hart phụ.

**Bài học phụ (KT-315)**: "bốn hart phải chọn **bốn giá trị đôi một khác**" là **giả định sai kiểu
KT-308** — `first=[a, b, u, u]` hoàn toàn hợp lệ. Đừng đòi nhiều hơn thứ được thiết kế bảo đảm.

### 1.4 Cách nhận ra lớp lỗi này **trước** khi soak

Trả lời ba câu:

1. **Thứ tự** nào phép kiểm giả định? (hart nào tick trước, lần chọn nào xảy ra trước)
2. Thứ tự ấy có **cơ chế bảo đảm** không — hay chỉ "thường đúng"? (cổng khởi động? cổng giữ? khoá?
   `HOLD_TICKS`?)
3. Nếu **đảo** thứ tự ấy, phép kiểm có **đỏ oan** không?

Nếu (2) là "thường đúng" ⇒ **đổi bất biến**, đừng tăng timeout, đừng thêm retry, đừng siết cổng giữ
để "làm nó tất định" (KT-315 đã siết `MH_HOLD_TICKS` và vẫn còn 8/600 — vì race nằm ở **phía BSP
trả tác vụ**, không phải ở phía hart phụ chờ).

---

## 2. Luật 2 — ca âm phải **chứng minh được** là đã đột biến

Ca âm (negative case) xanh vì `sed` **không khớp** còn tệ hơn không có ca âm: nó tạo **cảm giác an
toàn giả**.

### 2.1 Ca thật — KT-303, ca âm (u) xanh oan

Ca âm dùng `sed 's/ attempts=1 …'` để phá dòng evidence. Nhưng lần chạy thật có thể in
`attempts=2` ⇒ `sed` **không khớp** ⇒ không đột biến ⇒ ca "âm" **xanh** — và nó xanh vì **không có
gì bị phá**.

**Sửa**: `sed -E 's/ attempts=[12] …'` — phủ **hết** miền giá trị, không chỉ giá trị đã thấy.

### 2.2 Luật

- Mẫu `sed`/biến đổi của ca âm phải phủ **toàn miền** giá trị hợp lệ của trường bị phá, không chỉ
  giá trị **đã quan sát một lần**.
- Tốt nhất: ca âm khẳng định **đã phá** (so `grep -c` trước/sau) rồi mới khẳng định bộ kiểm **ĐỎ**.
  Nếu không phá được ⇒ ca âm phải **ĐỎ** (báo "không đột biến được"), không được im lặng xanh.
- Ranh giới: đây **không** phải nới ngưỡng, chỉ là làm ca âm **thật sự** là ca âm.

---

## 3. Luật 3 — kiểm **giá trị cực đại**, không chỉ giá trị thường

### 3.1 Ca thật — KT-312, `debug_assert!` sai miền giá trị

Bản nháp thu hẹp `Meta.untyped` từ `Option<usize>` sang `Option<u8>` đặt
`debug_assert!(u < MAX_UNTYPEDS)`. **Sai**: `untyped` là chỉ số **object** (`< MAX_OBJECTS`), không
phải chỉ số region (`< MAX_UNTYPEDS`). Test "giá trị cực đại" (`untyped = 15`, CNode 1000 slot)
bắt ngay; nếu chỉ test giá trị nhỏ thì lỗi lọt vào ảnh.

Cùng lớp: test `kt313_first_collision_is_not_a_failure` — **tái hiện đúng chuỗi race** thay vì chỉ
kiểm "trường hợp thường". Một test hồi quy cho race phải dựng **đúng thứ tự** đã gây race.

### 3.2 Luật

- Kiểm **cả hai đầu**: giá trị **cực đại** và giá trị **cực tiểu** (0, rỗng, `None`), không chỉ
  giá trị điển hình.
- Khi thu hẹp kiểu/bitfield: viết `assert!` **miền giá trị chứng minh được** ngay cạnh
  `assert_eq!(size_of)`.
- Khi sửa một race: viết test **tái hiện** đúng chuỗi đã gây race, và nó phải **đỏ** trên mã cũ.

---

## 4. Luật 4 — tính từ **cứng** trong phép kiểm phải là **phép tính có ghi**, không phải số chép tay

### 4.1 Ca thật — KT-290, tiền đề test sai sau khi thêm tính năng

Sim `kt62_cap_loi_dung_ma` kỳ vọng `Call` trên cap TCB nhãn 0 trả `NotSupported(18)`. KT-290 cài
`Tcb.Configure` ⇒ cùng lời gọi giờ trả `InvalidArgument(5)`. Test đỏ **vì tiền đề cũ**, không vì
hồi quy. Test nào khẳng định "tính năng X **chưa** cài" phải **đổi** khi X được cài.

### 4.2 Ca thật — KT-309 × KT-312, hai task cùng vòng sửa cùng một pin

KT-309 thêm `Message.extra_snaps` (+128 B), KT-312 thu hẹp `Meta` (−40 B). Riêng lẻ mỗi bên sửa
`tls_tests.rs` đúng; **hợp nhất** phải **cộng cả hai** và đổi trần `footprint` — nếu chỉ "sửa cho
xanh" thì pin thành dấu cao su.

### 4.3 Luật

- Mọi hằng trong phép kiểm phải **suy ra** từ `MAX_*`/`size_of`/hằng ABI sinh tự động — **không**
  chép số. Khi phải chép (ví dụ `196_608`), ghi **một** chỗ và mọi nơi khác trỏ về.
- Khi **giảm** một thành phần: dùng **đại số có dấu** (`isize`), ghi rõ khoản tăng **và** khoản
  giảm. "Thu hồi 10.240 B" phải là phép trừ hiện trong test, không phải số mới.
- Trần (pin/budget) **chỉ được hạ**, không được nâng để qua cổng. Nếu hợp nhất buộc phải đổi trần,
  trần mới = trần cũ + phần **tăng có ghi** của task kia, và **không** hạ được mức bảo đảm của trần
  cũ (KT-317: `184_320 + 512 = 184_832` vẫn chốt đúng "thu hồi ≥ 10.200 B").
- Đột biến để kiểm pin **có răng**: tăng một `MAX_*` ⇒ pin phải **ĐỎ** (KT-312 M2:
  `MAX_TCBS 64→80` ⇒ `197_368 > 196_608` ⇒ ĐỎ).

---

## 5. Checklist trước khi thêm/ sửa một phép kiểm

| # | Câu hỏi | Nếu "không" |
|---|---|---|
| 1 | Bất biến có đảo khi chỉ đổi **thứ tự tick** giữa các hart? | Đổi bất biến (§1) |
| 2 | Đã trả lời "cơ chế nào **bảo đảm** thứ tự ấy"? | Đổi bất biến (§1.4) |
| 3 | Ca âm **chứng minh** đã phá được dòng bị phá? | Sửa mẫu `sed`, phủ toàn miền (§2) |
| 4 | Đã kiểm **giá trị cực đại** và cực tiểu? | Thêm ca (§3) |
| 5 | Mọi hằng là **phép tính** từ hằng nguồn, có dấu? | Suy ra, đừng chép (§4) |
| 6 | Test "tính năng X **chưa** cài" còn đúng không? | Đổi tiền đề (§4.1) |
| 7 | Đột biến của phép kiểm **có răng** (đỏ đúng lý do)? | Chạy lại với đột biến (§4.3) |
| 8 | Có **giữ** trường chẩn đoán trong log (không chỉ điều kiện)? | Giữ lại `first=`, `attempts=`… (§1.2) |

## 6. Liên quan

- `v2/docs/k6-harness.md` — hợp đồng `qemu_guard.sh`, `qg_infra` / `MEASUREMENT_INVALID` (D2-116):
  số **thời gian** chỉ dùng khi `qemu_foreign=0`.
- `v2/agent/DECISIONS.md` — D2-05 (test đỏ trước), D2-07 (không retry để lấy xanh), D2-84 (pin
  `Machine`), D2-116 (đo hợp lệ), D2-205 (K3 đóng), D2-224.
- Journal ca thật: `kt298-x86_64-gp.md`, `kt303-*.md`, `kt308-x86_64-soak2.md`,
  `kt312-machinesize.md`, `kt313-x86_64-soak3.md`, `kt315-x86_64-fourhart.md`,
  `kt317-merge313.md`.
