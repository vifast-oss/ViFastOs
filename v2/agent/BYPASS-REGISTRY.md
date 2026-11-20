# BYPASS REGISTRY — luật `wake` / `transition_tcb`

**Lập**: 2026-11-20, theo **D2-193 §3** (Agent 0): "Không dùng lại các số thứ tự đường lọt đang
mâu thuẫn; lập registry có mã ổn định và ánh xạ tên cũ."

**Quy ước mã**: `WB-NN` (Wake Bypass). Mã **ổn định vĩnh viễn** — không đánh lại số khi thêm
đường mới; đường mới lấy số **kế tiếp**. Cột "Tên cũ" giữ nguyên văn các tên đã dùng trong
journal/báo cáo để tra cứu.

**Trạng thái**: `CLOSED` = đã vá + có repro ĐỎ + có đối chứng; `OPEN` = còn sống (đã dựng được
repro biên dịch được mà cả hai cơ chế kiểm đều XANH); `PARTIAL` = vá một phần.

---

## Bảng chính

| Mã | Tên cũ | Mô tả | Phát hiện | Trạng thái | Vá bởi |
|---|---|---|---|---|---|
| **WB-01** | #1 | `macro_rules!` passthrough `$i:item` + `include!` trong đối số lời gọi | L40 (KT-201) | CLOSED | KT-202 + KT-211 |
| **WB-02** | #2 | `include!` vị trí biểu thức trong `( … )` — `sink(include!{…})` | L44 (KT-208) | CLOSED | KT-211 |
| **WB-03** | #3 | `include!` vị trí biểu thức trong `[ … ]` | L44 | CLOSED | KT-211 |
| **WB-04** | #4 | `(0u8, include!{…})` | L44 | CLOSED | KT-211 |
| **WB-05** | #5 | `(include!{…})` | L44 | CLOSED | KT-211 |
| **WB-06** | #6 | `(‖ include!{…})` / `Some(include!{…})` | L44 | CLOSED | KT-211 |
| **WB-07** | #7 / #11 (hệ cũ) | `#[path]` trong inline module của tệp **non-mod-rs** | L47 (KT-212) | CLOSED | KT-213 |
| **WB-08** | #8 | `#[path]` trỏ ra ngoài `src/` qua `..` | L47 | CLOSED | KT-213 |
| **WB-09** | #9 | `include!` trong `v2/lib` trỏ sang `v2/kernel` | L40/L45 | CLOSED | KT-209 |
| **WB-10** | #10 | `macro_rules!` `:expr` + `include!` | L44 | PARTIAL | KT-202 (chỉ đóng dạng trong đối số lời gọi; `:expr` không bị từ chối vì cây thật có `bail`) |
| **WB-11** | #11 (hệ mới) | `mod` trong lời gọi macro (`cfg_if!`) | L40 | CLOSED | KT-202 |
| **WB-12** | #12 | thư mục tên `target` (`src/target/evil.rs`) | L47 | CLOSED | KT-215 (lớp 1) + KT-226 (lớp 2) |
| **WB-13** | #13 | `#[cfg(test)]` trên **nhánh `match`** — vùng test nuốt nhánh sản phẩm | L49 (KT-218) | CLOSED | KT-219 |
| **WB-14** | #14 | `macro_rules!` **lồng trong thân** macro khác che tên macro "an toàn" | L54 (KT-229) | CLOSED | KT-230 |
| **WB-15** | #15 | `use TcbState as S;` + `m.transition_tcb(t, S::Ready)` | L54 | CLOSED | KT-231 |
| **WB-16** | #16 | `Ready` qua **helper liên hàm / `const` / `static` / closure / hàm trả về** (6 dạng) | L57 (KT-234) | **CLOSED** ở tầng **API** (2026-11-21) | KT-237 (thu hẹp API `NonReadyTarget`). Bằng chứng: ô **B1–B5** PASS — `cargo check` RC=101 `E0308` `expected NonReadyTarget, found TcbState`; đối chứng dương PASS |
| **WB-17** | #17 | `transition_tcb` làm **fn-pointer / generic `F: Fn` / trait object / UFCS** (7 biến thể) | L55 (KT-232) | **CLOSED** ở tầng **API** (2026-11-21) | KT-237 (API) + KT-239 (UFCS). Bằng chứng: ô **A1–A4** PASS — `E0308`/`E0631` tại chính `transition_tcb`; đối chứng dương PASS |
| **WB-18** | #18 | crate thành viên workspace **ngoài ba gốc quét** + `$OUT_DIR` | L56 (KT-233) | **CLOSED** | KT-238 (phạm vi từ `cargo metadata`, 22 crate) |
| **WB-19** | — | **cửa thứ bảy**: `impl`/`fn` **lồng trong thân cửa** gọi thẳng `write_state` ⇒ **cả hai lớp XANH** | review độc lập (luồng 2) | **CLOSED** | vá `c0b48f3b`; **lead xác minh độc lập** trên `0018f5ab` — `JOURNAL/kt242-wake-verify.md` (cả 3 lớp ĐỎ, `cargo` RC=0) |
| **WB-20** | — | **cửa thứ bảy qua TRÙNG TÊN**: `fn wake_step` (cùng tên 6 cửa) ở `impl` **kiểu khác** / **module con** / **trait impl** / **free fn** trong `state_write.rs` gọi `write_state(t, Ready)` ⇒ **cả ba lớp XANH**, `cargo RC=0` | KT-252 (vòng tấn công mới, nhóm G1) | **CLOSED** (2026-11-21) | vá `_check_state_write`: cửa phải là method của **cùng kiểu** với `write_state` (`_enclosing_impl_type`); +5 ca selftest có răng + đối chứng dương. Bằng chứng: 4 biến thể XANH trước vá → **ĐỎ cả ba lớp** sau vá — `JOURNAL/kt252-wake-attack.md` §2–§3 |

---

## Ánh xạ hệ đánh số cũ → mã ổn định

Có **hai** hệ đánh số từng chạy song song (D2-175 đã chốt một hệ, nhưng tài liệu cũ vẫn còn):

| Hệ cũ | Ghi chú | Ánh xạ |
|---|---|---|
| `kt212/213/215` gọi #11 = `#[path]` non-mod-rs | hệ **theo thứ tự phát hiện** | → **WB-07** |
| `kt217` gọi #11 = `mod` trong lời gọi macro | hệ **theo nhóm cơ chế** | → **WB-11** |
| `kt219` gọi #13 | trùng cả hai hệ | → **WB-13** |
| `kt229/230` gọi #14 | — | → **WB-14** |
| `kt231` gọi #15 | — | → **WB-15** |
| `kt234` gọi #16 | — | → **WB-16** |
| `kt232` gọi #17 | — | → **WB-17** |
| `kt233` gọi #18 | — | → **WB-18** |

**Từ nay**: mọi tài liệu mới dùng **WB-NN**; khi trích tài liệu cũ thì ghi kèm tên cũ.

---

## Ba đường **từng chặn nghiệm thu** (D2-193 §2) — đóng ở tầng **API**; **WB-19** đã đóng ở tầng **checker**

**WB-16, WB-17, WB-18 từng là P0 chặn nghiệm thu luật `wake`** — **không** được ghi vào "nợ không
chặn". Mỗi đường đều có repro **biên dịch được** (`cargo check` RC=0) mà **cả hai cơ chế kiểm
đều XANH**. **Cả ba đã đóng**: WB-18 (KT-238), WB-16/WB-17 (KT-237 + KT-239; nghiệm thu KT-241
2026-11-21 — ma trận tấn công 28 ô, xem `JOURNAL/kt241-attack/`).

**Phạm vi đóng — đọc kỹ**: WB-16/WB-17 đóng ở tầng **API** (thu hẹp kiểu ⇒ đường gọi sai **không
biểu diễn được**, `cargo check` từ chối). Điều này **KHÔNG** có nghĩa "luật `wake` đã đóng":
**WB-19** là lỗ ở tầng **checker** (`impl`/`fn` lồng trong thân cửa gọi thẳng `write_state` ⇒ cả
hai lớp XANH) và **đang OPEN**. Hai tầng khác nhau — **không** suy từ "WB-16/17 CLOSED" ra "checker
kín".

| Mã | Repro đã dựng | Bằng chứng | Trạng thái |
|---|---|---|---|
| WB-16 | `fn helper(m,t,s){ m.transition_tcb(t,s) }` + `helper(m,t,TcbState::Ready)`; `const`/`static`; closure | `JOURNAL/kt234-interfn.md` + `kt234-logs/`; **nghiệm thu**: ô **B1–B5** PASS, `E0308` `expected NonReadyTarget, found TcbState` | **CLOSED** |
| WB-17 | `let tt = Machine::transition_tcb; tt(m,t,TcbState::Ready)`; generic `F: Fn`; trait object; UFCS `M::transition_tcb(m,t,Ready)` | `JOURNAL/kt232-fnptr.md` + `kt232-logs/`; **nghiệm thu**: ô **A1–A4** PASS, `E0308`/`E0631` | **CLOSED** |
| WB-18 | crate `v2/extra/evil` ngoài ba gốc + `build.rs` ghi `$OUT_DIR/evil.rs` | `JOURNAL/kt233-buildrs.md` + `kt233-logs/`; KT-238 | **CLOSED** |

**Hướng đóng** (D2-187, D2-188): **không** vá bằng phân tích liên hàm; **thu hẹp API** để đường
gọi sai **không biểu diễn được** (kiểu đích chuyển trạng thái loại trừ `Ready`; primitive ghi
trạng thái thô sau ranh giới riêng tư hẹp), và **phạm vi bảo vệ sinh từ `cargo metadata`**.

---

## Ghi chú về tính đầy đủ

Registry này liệt kê **các đường đã được dựng repro**. Nó **không** phải danh sách đầy đủ mọi
đường có thể tồn tại — D2-190 §7 nói rõ: đóng luật nghĩa là **đạt hợp đồng hữu hạn đã công bố**,
không phải chứng minh được mọi chương trình Rust.
