# PHIẾU GỬI AGENT 0 — vòng 8 (đêm 2026-11-20)

**Từ**: Agent 1, giữ quyền Agent 0 từ 18:52 JST 2026-11-19 (người điều phối cấp).
**Chỉ thị**: "tiếp tục triển khai 4 sub agent2 để thực thi tiếp cho tới 9h sáng mai".
**Báo cáo đầy đủ**: [`BAO-CAO-DEM-2026-11-20.md`](BAO-CAO-DEM-2026-11-20.md).

---

## 1. Trạng thái một khối

```text
HEAD          cbc2c2af   (main, đã push, chưa push: 0)
build-inputs  233 mục, dấu vân tay 43df126a4a6702ec, dirty=0
ảnh sản phẩm  639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81
              213 632 B, build=b08c5c57cdbdb156
K6            6/6 ĐẠT trên ảnh 639ca6d1 (KT-210)
task          DONE=202  READY=0  BLOCKED=0  REVIEW=2
luật wake     CHƯA ĐÓNG — KT-194 và KT-184 giữ REVIEW
checker       lớp 1: 295 ca; lớp 2: 37 ca; cả hai errors=0 trên cây thật
```

---

## 2. Ba việc ngài giao (D2-164) — đã xong

| Task | Kết quả | Ghi chú |
|---|---|---|
| **KT-181** quét thân macro ngoài `v2/lib`/`v2/kernel/*/src` | **DONE** | **Trung thực**: không dựng được đường lọt thật ⇒ đây là **phòng ngừa**, không phải lỗ hổng đã khai thác |
| **KT-182** đo tác động bản đồ trợ giúp cùng tệp | **DONE** | Đo **1019** test: **0** test chỉ dựa hàm sản phẩm cùng tệp ⇒ siết **miễn phí**; reviewer xác nhận bằng parser độc lập |
| **KT-183** `cd` thư mục con sâu | **DONE** | Ảnh mới `639ca6d1`; **19/19** khối PASS; K6 đo lại **6/6** |

---

## 3. Luật `wake` — tình hình thật

Đêm nay tìm thêm **ba** đường lọt mới (**#11, #12, #13**) và đóng cả ba. Tổng cộng **mười ba**
đường lọt trong ba ngày.

**Đường lọt #13 là nguy hiểm nhất**: `#[cfg(test)]` đặt trên **nhánh `match`** làm vùng test
nuốt nhánh sản phẩm — nó lọt **cả hai lớp** kiểm vì hai lớp **dùng chung** `_test_char_ranges`.

**Hai lớp hiện có**:

| Lớp | Cách kiểm | Bắt được | Không bắt được |
|---|---|---|---|
| **1** tĩnh | lexer + token-tree + walker | mọi tệp, mọi nhánh `#[cfg]`, đích `include!`/`#[path]` | vị trí token lạ (đã vá 13 lần) |
| **2** dep-info | hỏi **rustc** danh sách tệp thật | đúng danh sách nạp, kể cả `v2/lib` | mã do macro sinh, nhánh `#[cfg]` tắt |

**Nhưng hai lớp KHÔNG đủ.** Reviewer L53 tìm được **năm** đường lọt **lọt cả hai lớp** cùng lúc
(B3b `#[cfg(feature="ktest")] mod`; B12–B15 trong `v2/lib`: `.txt`, symlink, `src/target/`, dấu
cách) — reviewer trước đó (L51) chấm "ĐÓNG ĐƯỢC" vì chỉ chọn mẫu từ danh sách đã biết. KT-227
đã vá: **5 → 0**.

**Rồi reviewer L54 tấn công THEO LỚP (không theo danh sách) và tìm thêm HAI đường lọt SỐNG:**

| # | Đường lọt | Gốc | Vá |
|---|---|---|---|
| **#14** | `macro_rules!` **lồng trong thân** `macro_rules!` khác che tên macro "an toàn" ⇒ `matches!(m, wake, t)` mở rộng thành `m.wake(t)` | `_shadowed_safe_macros` chỉ thấy `macro_rules!` **literal** | **KT-230** |
| **#15** | `use TcbState as S;` + `m.transition_tcb(t, S::Ready)` | `RE_READY_TRANSITION` khớp **văn bản** `TcbState::Ready` | **KT-231** |

Cả hai lọt **cả hai lớp**. Đây là **xác nhận mạnh nhất** cho bài học: tấn công **theo lớp** tìm
ra đường lọt mà **13 đường cũ không có**.

Lớp 2 đã nối vào `v2 check` (D2-173). Chi phí **+8 s** ấm.

**Tôi KHÔNG tuyên bố luật đã đóng.** Mỗi lần tuyên bố đóng trước đây đều bị reviewer kế tiếp
phá trong vài giờ — **đêm nay đã xảy ra đúng như vậy**: L51 chấm "ĐÓNG ĐƯỢC", L53 phá trong
chưa đầy một giờ. D2-178 đã rút tuyên bố đó. Đề nghị: **một vòng tấn công theo LỚP** (mọi tổ hợp
`#[cfg]` × vị trí × gốc phân giải), **không** theo danh sách đường lọt đã biết.

---

## 4. Quyết định đã ban hành (quyền A0)

| Mã | Nội dung |
|---|---|
| **D2-169** | Đảo ngược D2-168: KT-194/KT-184 về REVIEW do đường lọt **sống** |
| **D2-170** | Đường lọt #10; KT-202 KHÔNG ĐẠT; mở KT-211 |
| **D2-171** | Đường lọt #11; mở KT-213; đề xuất kiến trúc `dep-info` |
| **D2-172** | KT-213 ĐẠT; đường lọt #12; mở KT-215 |
| **D2-173** | Nối lớp 2 vào `v2 check` |
| **D2-174** | Đường lọt #13, lọt **cả hai** lớp; mở KT-219 |
| **D2-175** | Đính chính D2-174; chốt hệ đánh số; dãy selftest là DAG |
| **D2-176** | KT-211 ĐẠT CÓ ĐIỀU KIỆN; 4 báo oan; mở KT-225 |
| **D2-177** | Reviewer cuối chấm ĐÓNG ĐƯỢC (phạm vi 7 đường lọt) |
| **D2-178** | **ĐÍNH CHÍNH D2-177**: luật `wake` **CHƯA đóng**; 5 đường lọt cả hai lớp |
| **D2-179** | Đường lọt **#14** (macro lồng che tên an toàn); mở KT-230 |
| **D2-180** | KT-230 ĐẠT; đường lọt **#15** (alias kiểu che `TcbState::Ready`); mở KT-231 |
| **D2-181** | KT-231 ĐẠT (đóng #15) |

---

## 5. Bốn câu hỏi cho ngài

1. **Luật `wake` đóng thế nào?** Đề nghị: reviewer độc lập chạy **một vòng tấn công đầy đủ**
   trên HEAD cuối; nếu **không** tìm được đường lọt nào **sống** (mã biên dịch được + checker
   xanh) thì đóng. Ngài có đồng ý tiêu chí này không, hay muốn chặt hơn?
2. **Lớp 2 (`dep-info`) có nên thành bắt buộc?** Nó thêm **8 s** vào `v2 check` và cần `cargo`
   chạy được. Nếu ngài đồng ý, nó đã nằm trong `v2 check` rồi; nếu không, tôi gỡ ra.
3. **Lớp 3 (`cargo expand`)?** Đây là cách duy nhất bắt mã do `macro_rules!` sinh ra — lỗ hổng
   đã biết của **cả hai** lớp. Cần nightly ⇒ phải quyết riêng. Ngài có muốn mở không?
4. **K4' bước 3 và K5'?** Tôi **không** tự mở. Ngài có muốn mở sau khi luật `wake` đóng không?

---

## 6. Nợ còn mở (không chặn)

| Nợ | Mức | Ghi chú |
|---|---|---|
| ~~KT-211, KT-214 chưa có reviewer độc lập~~ | — | **ĐÃ ĐÓNG**: KT-223 (L52) và KT-224 (L53) |
| `v2/lib` ngoài luật token của lớp 1 | P2 | **DEBT-KT227-1**: không nguy hiểm (không crate `v2/lib` nào phụ thuộc core/sim ở `[dependencies]`; `vifast-model::Machine` không có `wake`) |
| B4b `#[cfg(any())] mod` tệp riêng | P2 | lớp 2 không thấy (nhánh tắt); lớp 1 bắt |
| B17 OUT_DIR/build.rs | P2 | lớp 2 không thấy nhưng **không im lặng** (`W_DEPINFO_SKIPPED`); lớp 1 bắt |
| Truyền `Ready` qua tham số liên hàm | P2 | phân tích cục bộ không thấy |
| `transition_tcb` dùng làm **giá trị** (fn-pointer) | P2 | `_transition_calls` chỉ thấy **lời gọi** — reviewer sau thử **trước** |
| Proc-macro che tên an toàn | P3 | cây **không** có crate `proc-macro = true` (đo = 0) |
| `build.rs` sinh `$OUT_DIR` + `include!` ẩn trong macro | P2 | **chưa đo** |
| `:expr` passthrough chưa từ chối | P2 | cây thật có `bail` dùng `:expr`; từ chối sẽ báo oan |
| Tệp `.rs` không thuộc crate ngoài mẫu `wake` | P2 | đo **0** token `wake`; không có `Machine` |
| `virtio_` tiền tố tên tệp (luật arch) | P3 | rủi ro thấp |
| Bash 3.2 SIGTRAP | P2 | đã vá bằng KT-207; gốc cần bash ≥ 4 |
| C3 aarch64 (28/31 stub host) | P2 | nợ cũ |
| K4' bước 3, K5' | — | **đóng** theo quyết định của ngài |

---

## 7. Việc tiếp theo (đề xuất)

1. Reviewer độc lập tấn công HEAD cuối (một vòng đầy đủ) — quyết định đóng luật `wake`.
2. Reviewer độc lập cho KT-211 và KT-214.
3. Nếu ngài duyệt: mở K4' bước 3.
