# BÁO CÁO ĐÊM 2026-11-20 — Agent 1 (giữ quyền Agent 0)

**Khoảng thời gian**: 2026-11-19 23:45 → 2026-11-20 09:00 JST.
**Người thực hiện**: Agent 1, được người điều phối cấp quyền Agent 0 lúc 18:52 JST 2026-11-19.
**Chỉ thị**: "tiếp tục triển khai 4 sub agent2 để thực thi tiếp cho tới 9h sáng mai".

---

## 1. Trạng thái một khối

```text
HEAD          af1ba3c9   (main, đã push, chưa push: 0)
build-inputs  233 mục, dấu vân tay 43df126a4a6702ec, dirty=0
ảnh sản phẩm  639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81
              213 632 B, build=b08c5c57cdbdb156, inputs=43df126a4a6702ec
K6            6/6 ĐẠT trên ảnh 639ca6d1 (KT-210)
task          DONE=189  READY=1  BLOCKED=0  REVIEW=3
luật wake     CHƯA ĐÓNG — KT-194 và KT-184 giữ REVIEW
checker       lớp 1: 246 ca selftest; lớp 2: 20 ca; cả hai errors=0 trên cây thật
```

---

## 2. Việc đã làm trong đêm

### 2.1. Ba task READY của Agent 0 (D2-164)

| Task | Kết quả | Bằng chứng |
|---|---|---|
| **KT-181** quét thân `macro_rules!` ngoài `v2/lib`/`v2/kernel/*/src` | **DONE** | Quét cả ba gốc; 10 ca mới; **kết luận trung thực**: không dựng được đường lọt thật cho `wake` ngoài phạm vi cũ ⇒ đây là **phòng ngừa**, không phải lỗ hổng đã khai thác |
| **KT-182** đo tác động bản đồ trợ giúp cùng tệp | **DONE** | Đo **1019** test: **0** test chỉ dựa hàm sản phẩm cùng tệp; **155** test có gọi nhưng **cả 155** đều có token riêng ⇒ siết **miễn phí**; reviewer L41 xác nhận bằng parser độc lập; vá F1/F2 (KT-206) ⇒ selftest 36→**40** |
| **KT-183** `cd` thư mục con sâu | **DONE** | Seed `/docs/sub` + `deep.txt`; ca `kt183-cd-sau` 8 phép kiểm; đột biến `..`/`pwd` hằng **ĐỎ**; ảnh mới `639ca6d1`; reviewer L42 **ĐẠT CÓ ĐIỀU KIỆN** (M2/M4) → vá KT-205 ⇒ **19/19** khối PASS |

### 2.2. Luật `wake`/`include!`/`#[path]` — mười ba đường lọt

Đây là phần **nặng nhất** của đêm. Luật này đã bị phá **mười ba** lần trong ba ngày. Đêm nay
tìm thêm **ba** đường lọt mới và đóng cả ba.

| # | Đường lọt | Phát hiện | Trạng thái |
|---|---|---|---|
| 1 | `macro_rules!` passthrough `$i:item` + `include!` trong đối số lời gọi | L40 (KT-201) | **ĐÓNG** (KT-202 + KT-211) |
| 2–6 | `include!` ở vị trí biểu thức trong `( … )`/`[ … ]` (5 biến thể) | L44 (KT-208) | **ĐÓNG** (KT-211) |
| 7 | `#[path]` trong inline module của tệp **non-mod-rs** | L47 (KT-212) | **ĐÓNG** (KT-213) |
| 8 | `#[path]` trỏ ra ngoài `src/` qua `..` | L47 | **ĐÓNG** (KT-213) |
| 9 | `include!` trong `v2/lib` trỏ sang `v2/kernel` | L40/L45 | **ĐÓNG** (KT-209) |
| 10 | `macro_rules!` `:expr` + `include!` | L44 | **ĐÓNG một phần** — `:expr` không bị từ chối (cây thật có `bail` dùng `:expr`); `include!` trong đối số lời gọi đã bị bắt |
| 11 | `mod` trong lời gọi macro (`cfg_if!`) | L40 | **ĐÓNG** (KT-202) |
| 12 | thư mục tên `target` (`src/target/evil.rs`) | L47 | **ĐÓNG** (KT-215) |
| 13 | `#[cfg(test)]` trên **nhánh `match`** — vùng test nuốt nhánh sản phẩm | L49 (KT-218) | **ĐÓNG** (KT-219) |

**Đường lọt #13 là nguy hiểm nhất**: nó lọt **cả hai lớp** kiểm, vì hai lớp **dùng chung**
`_test_char_ranges`. Đây là bài học kiến trúc: hai lớp "độc lập" vẫn có thể chia sẻ một hàm.

### 2.3. Hai lớp kiểm

| Lớp | Công cụ | Cách kiểm | Selftest | Chi phí |
|---|---|---|---|---|
| **1** | `check-wake-paths.py` | lexer + token-tree + walker tĩnh | **259** ca | ~21 s |
| **2** | `check-wake-depinfo.py` | hỏi **rustc** danh sách tệp thật (`--emit=dep-info`) | **27** ca | 8,3 s ấm / 26,9 s lạnh |

Lớp 2 được nối vào `v2 check` (KT-217, D2-173). **Con số "lớp 2 bắt 11/11, lớp 1 bắt 5/11"
là số đo ở `dc6e01ae` — trước KT-211/213/215.** Trên HEAD hiện tại, **lớp 1 bắt 11/11**; giá trị
thật của lớp 2 là **cơ chế độc lập** + phủ `v2/lib`, không phải "lấp 6/11". (Đính chính KT-227.)

Hai lớp **bù nhau**:

- Lớp 1 phủ **rộng**: mọi tệp, mọi nhánh `#[cfg]`, đích `include!`/`#[path]` kể cả tệp không
  được nạp.
- Lớp 2 phủ **sâu**: đúng danh sách rustc nạp, kể cả `v2/lib`.

**Nhưng hai lớp KHÔNG đủ.** Reviewer L53 (KT-224) tìm được **năm** đường lọt **lọt cả hai lớp**
cùng lúc — điều mà reviewer trước (KT-222) không thấy vì chỉ chọn mẫu từ danh sách đã biết:

| # | Đường lọt cả hai lớp | Gốc |
|---|---|---|
| B3b | `#[cfg(feature = "ktest")] mod` + tệp riêng trong `v2/kernel` | lớp 1 coi module `ktest` là test-only; lớp 2 không nạp (feature tắt) |
| B12 | `#[path = "*.txt"]` trong `v2/lib` | lớp 1 không áp luật token trên `v2/lib`; lớp 2 bỏ vì đuôi tệp |
| B13 | symlink → `/tmp` trong `v2/lib` | lớp 2 `resolve()` ra ngoài |
| B14 | `#[path]` vào `src/target/` trong `v2/lib` | lớp 2 lọc `"target" in parts` |
| B15 | tên tệp có dấu cách trong `v2/lib` | lớp 2 parse `\ ` sai |

Cộng **tám** đường lọt riêng lớp 2 (lớp 1 bắt) và **hai lỗi nghiêm trọng**: **fail-open** (`.d`
rỗng/thiếu ⇒ `errors=0`, RC=0) và **parse `\ ` sai**.

KT-227 vá cả năm: **5 → 0** đường lọt cả hai lớp, **8 → 2** đường lọt riêng lớp 2 (cả hai đều
bị lớp 1 bắt), **0** báo oan. Nhưng **bài học vẫn còn**: reviewer chọn mẫu từ danh sách đã biết
thì **không thể** tìm đường lọt mới.

### 2.4. K6 — đo lại trên ảnh mới

KT-183 đổi `v2/kernel` ⇒ dấu vân tay `89f40ef16c97e6d5` → `43df126a4a6702ec` ⇒ ảnh K6 cũ
`b090cfe8…` không còn là ảnh nguồn. Đo lại (KT-210):

| # | Tiêu chí | Kết quả |
|---|---|---|
| F1 | Shell in `shell>` ở smp 1/2/4 | `shell_prompt_lines=2` cả ba |
| F2 | Parity + đọc lại sau reboot + persist | `parity smp=1 exit=0`, `smp=4 exit=0`; đĩa **không đổi** giữa hai boot |
| F3 | Ảnh tắt shell phải đỏ | `shell_prompt_lines=0`, `parity_noshell_exit=5` |
| F4 | Ma trận KTEST + soak 1000 boot | `matrix_exit=0`; `soak: ok=1000 fail=0 wall=297s` |
| F5 | `.data + .bss ≤ 262144` | `data+bss=224288` |
| F6 | Ảnh sản phẩm không chứa mã harness | product `hits=0`, ảnh âm `hits=25` |

**6/6 ĐẠT.**

### 2.5. Hạ tầng

- **KT-207**: `v2 check` đỏ oan ở bước `luật cấu trúc` do bash 3.2 hỏng heap (SIGTRAP). Workaround
  subshell của KT-155 **không đủ** (subshell là fork, thừa hưởng heap hỏng). Nay chạy `rules`
  trong **tiến trình bash mới**; 3/3 lượt `v2 check` xanh dưới tải.
- **KT-217**: nối lớp 2 vào `v2 check`.

---

## 3. Điều **chưa** đóng — nói thẳng

1. **Luật `wake` CHƯA đóng.** KT-194 và KT-184 giữ **REVIEW**. Mười ba đường lọt đã bị phá;
   đêm nay đóng được ba đường mới nhất, nhưng **không** có cơ sở để tuyên bố "đã đóng" — mỗi
   lần tuyên bố đóng trước đây đều bị reviewer kế tiếp phá trong vòng vài giờ. **Đêm nay đã
   xảy ra đúng như vậy**: reviewer L51 chấm "ĐÓNG ĐƯỢC", rồi reviewer L53 tìm tiếp **năm**
   đường lọt **cả hai lớp** trong vòng chưa đầy một giờ. D2-178 đã rút tuyên bố đó.
2. **KT-211 và KT-214 chưa có reviewer độc lập** — chỉ tác giả tự chấm. Ghi thành nợ có tên.
3. **`:expr` passthrough chưa từ chối** (KT-202) — cây thật có `macro_rules! bail` dùng `:expr`
   hợp lệ; từ chối sẽ báo oan. Đường còn hở chỉ khi macro được gọi từ tệp ngoài phạm vi quét.
4. **Tệp `.rs` không thuộc crate** (`vifast-plat-user/lib.rs`, `init/*.rs`) ngoài mẫu `wake`.
   Đo: **0** token `wake`; chúng không có `Machine`.
5. **`virtio_` tiền tố tên tệp** trong luật arch — chưa vá (rủi ro thấp, cần kiểm ngữ nghĩa).
6. **Bash 3.2 SIGTRAP** — đã vá bằng KT-207 nhưng **gốc chưa chữa**; cần bash ≥ 4.
7. **K4' bước 3 và K5' vẫn đóng** — tôi **không** tự mở.
8. **C3 aarch64** (28/31 stub host chưa kiểm) — nợ cũ, chưa chạm.

---

## 4. Khuyến nghị cho Agent 0

1. **Đừng đóng luật `wake` chỉ vì đêm nay xanh.** Đề nghị: thêm **một vòng tấn công độc lập
   nữa** trên HEAD cuối, và chỉ đóng khi reviewer **không** tìm được đường lọt trong một vòng
   đầy đủ.
2. **Giữ hai lớp.** Lớp 2 (dep-info) là lớp duy nhất không tự suy luận cú pháp; nó bắt được
   những gì lớp 1 bỏ sót. Chi phí +8 s ấm là rẻ so với việc luật bị phá lần thứ mười bốn.
3. **Kiểm danh sách phụ thuộc chung giữa hai lớp.** Đường lọt #13 lọt cả hai vì chúng dùng
   chung `_test_char_ranges`. Đề nghị: mọi hàm dùng chung phải có ca selftest **riêng** ở cả
   hai lớp.
4. **Cân nhắc `cargo expand`** cho lớp 3 (mã do `macro_rules!` sinh ra) — hiện là lỗ hổng đã
   biết của cả hai lớp. Cần nightly ⇒ phải quyết riêng.
5. **Sổ trạng thái**: `DONE=189 READY=1 BLOCKED=0 REVIEW=3`. `BLOCKED=0` **không** có nghĩa
   mọi cổng đều mở được — K4'/K5' vẫn đóng theo quyết định của A0.

---

## 5. Quyết định đã ban hành trong đêm (quyền A0)

| Mã | Nội dung |
|---|---|
| **D2-169** | Đảo ngược D2-168: KT-194/KT-184 về REVIEW do đường lọt **sống** qua lời gọi macro |
| **D2-170** | Đường lọt #10; KT-202 KHÔNG ĐẠT; mở KT-211 (quét token toàn cục) |
| **D2-171** | Đường lọt #11 (gốc phân giải `#[path]`); mở KT-213; đề xuất kiến trúc `dep-info` |
| **D2-172** | KT-213 ĐẠT; đường lọt #12 (`src/target/`); mở KT-215 |
| **D2-173** | Nối lớp 2 (`dep-info`) vào `v2 check` |
| **D2-174** | Đường lọt #13 (nhánh `match`), lọt **cả hai** lớp; mở KT-219 |
| **D2-175** | Đính chính D2-174; chốt hệ đánh số 13 đường lọt; dãy selftest là DAG |
| **D2-176** | KT-211 ĐẠT CÓ ĐIỀU KIỆN; 4 báo oan; mở KT-225 |
| **D2-177** | Reviewer cuối chấm ĐÓNG ĐƯỢC trên `be3a0cd8` (phạm vi 7 đường lọt) |
| **D2-178** | **ĐÍNH CHÍNH D2-177**: luật `wake` **CHƯA đóng**; KT-214 KHÔNG ĐẠT; 5 đường lọt cả hai lớp |

---

## 6. Sub-agent đã dùng

| Tên | Task | Kết quả |
|---|---|---|
| L37 | KT-181 + KT-202 | ĐẠT CÓ ĐIỀU KIỆN (`:expr` không từ chối — có lý do) |
| L38 | KT-182 | ĐẠT |
| L39 | KT-183 | ĐẠT |
| L40 | KT-201 (review vòng 4) | **KHÔNG ĐẠT** — tìm đường lọt #1 |
| L41 | KT-203 (review KT-182) + KT-206 | ĐẠT CÓ ĐIỀU KIỆN → ĐẠT |
| L42 | KT-204 (review KT-183) | ĐẠT CÓ ĐIỀU KIỆN |
| L43 | KT-205 (vá M2/M4) | ĐẠT |
| L44 | KT-208 (review KT-202) | **KHÔNG ĐẠT** — tìm đường lọt #2–#6 |
| L45 | KT-209 | ĐẠT |
| L46 | KT-211 + KT-216 (merge) | ĐẠT |
| L47 | KT-212 (tấn công) + KT-213 + KT-215 | ĐẠT — tìm #7, #8, #12 |
| L48 | KT-214 (nguyên mẫu dep-info) | ĐẠT CÓ ĐIỀU KIỆN |
| L49 | KT-218 (tấn công cuối) + KT-219 | ĐẠT — tìm #13 |
| L50 | KT-220 (audit đêm) | TRUNG THỰC, 1 lệch nhỏ |
| L51 | KT-222 (verify cuối) + KT-226 | ĐÓNG ĐƯỢC **trong phạm vi 7 đường lọt đã chọn** — bị D2-178 rút |
| L52 | KT-223 (review KT-211) + KT-225 | ĐẠT CÓ ĐIỀU KIỆN → ĐẠT — tìm 4 báo oan + đường lọt `.txt` |
| L53 | KT-224 (review KT-214) + KT-227 + KT-228 | **KHÔNG ĐẠT** → ĐẠT — tìm **5 đường lọt cả hai lớp** |

**Tổng**: 17 sub-agent, 4 chạy song song mỗi lượt.

---

## 7. Ghi chú về tính trung thực

Audit độc lập KT-220 kiểm **190** khẳng định: **183 khớp**, **1 lệch**, **6 không nguồn**,
**1 không chứng cứ**. Không có bịa đặt hệ thống. Các lệch là số dẫn xuất viết tay, không đổi
kết luận kỹ thuật nào. Audit cũng tự dựng lại **4** đột biến và xác nhận cả 4 **ĐỎ** đúng như
journal khai.

Ba journal đã **tự đính chính tại chỗ** (giữ nguyên lịch sử, thêm dấu ⚠): `kt183` (rev trong
ảnh — sai; log QEMU thiếu), `kt203`/`kt204`/`kt206`/`kt208` (ghi rõ `v2 check` đỏ là T-INFRA
SIGTRAP, không giấu).
