# PHIẾU GỬI AGENT 0 — 2026-11-19, vòng 7 (KT-194 đóng)

**Từ**: Agent 1 (lead), **hành động với quyền A0** theo uỷ quyền của người điều phối lúc 18:52 JST
(A0 offline). Mọi quyết định ghi thành **D2-168**.

---

## 1. Trạng thái một khối

```text
HEAD         56e2cb2d (main)   dirty: 0   chưa push: 0
build-inputs 233 mục   dấu vân tay: 89f40ef16c97e6d5
task         DONE=172  READY=3  BLOCKED=0  REVIEW=0
checker      thường errors=0 / strict errors=0 / selftest 118/118
gate         PASS trên c8bef672 (HEAD tích hợp cuối)
```

`REVIEW=0` — **KT-184 và KT-194 đều đã đóng**. `BLOCKED=0` = không task nào bị chặn dependency;
K4' bước 3 và K5′ vẫn **đóng** theo D2-162/D2-163.

---

## 2. KT-194 — **DONE**

Review vòng 3 (KT-200, L36) trên `98694376`: **ĐẠT CÓ ĐIỀU KIỆN**.

### Ba lỗi vòng 2 — reviewer xác nhận đóng

| Lỗi | Bằng chứng |
|---|---|
| **F2b** (luật `wake` theo dòng) | `_test_char_ranges` dùng token-tree; 3 đột biến cùng dòng ⇒ **ĐỎ** `--strict`, `cargo check` RC=0; 2 đối chứng ⇒ **XANH** |
| **F4b** (inline module `#[path]` báo oan) | `#[cfg(test)] #[path="sub"] mod m { mod child; }` + `sub/child.rs` gọi `wake` ⇒ **XANH** cả hai chế độ; `cargo check --tests` RC=0 |
| **F4c** (`#[path]` trỏ thư mục, không thân) | **ĐỎ** `E_STRICT_WAKE_INCLUDE` cả hai chế độ |

### Ba điều kiện hợp đồng — đã sửa (`c8bef672`)

- **C1**: dòng 50 ghi sai mã lỗi macro — tách đúng `E_STRICT_MACRO_INCLUDE` (include/path),
  `E_STRICT_MACRO_MOD` (`mod NAME;`), `E_STRICT_MACRO_TT` (`:tt`).
- **C2**: §5 nói "ngoài `v2/kernel`/`v2/sim` không quét" — sai; `scan_lexical` **có** quét `v2/lib`.
- **C3**: §4 ghi thêm **báo oan có trước** (không phải hồi quy — đối chiếu checker `0fa0fe65` giống hệt).

### Hồi quy

76 ca cũ **không mất ca nào**; `--selftest` **118/118**; checker `0fa0fe65` chạy 76/76.

---

## 3. KT-184 — **DONE**

Luật gốc (H4 fixpoint + `include!`/`#[path]` literal + trong cây, **mọi chế độ**) nay được
lexer/token-tree thực thi và **reviewer độc lập xác nhận** trên `98694376`/`c8bef672`.
Repro H6/H7 bền: `v2/agent/JOURNAL/kt184-logs/repro-h6h7.sh` chạy lại được.

---

## 4. Gate

```text
gate --fast  PASS trên c8bef672 — EXIT=0, log /tmp/gate-12.log
```

Đây là gate trên **HEAD tích hợp cuối**, không phải gate cũ `a90f1d77` hay `8ef2b941`.

---

## 5. K6

Ảnh `b090cfe8…` (commit `6f2d2c1d`, vân tay `89f40ef16c97e6d5`, toolchain 1.99.1) vẫn là ảnh
nguồn; **6/6 ĐẠT**. KT-194 chỉ đổi **checker**, không đổi ảnh sản phẩm ⇒ theo D2-166 §6 **không**
chạy lại soak/parity; phần bị ảnh hưởng bởi checker đã chạy lại (`v2 check` + `--selftest`).

---

## 6. Nợ còn mở (không chặn đóng KT-184/194)

| Mã | Nội dung |
|---|---|
| KT-181 | quét thân `macro_rules!` ngoài `v2/lib` và `v2/kernel/*/src` |
| KT-182 | đo tác động bản đồ trợ giúp KT-163 |
| KT-183 | `cd` vào thư mục con sâu |
| C3 (aarch64) | 28/31 stub host chưa được kiểm |
| K4' bước 3 | **đóng** theo D2-162 |
| K5' | **đóng** theo D2-163 |

---

## 7. Quyết định với quyền A0 (D2-168)

1. KT-194 → **DONE** (review vòng 3 ĐẠT CÓ ĐIỀU KIỆN, ba điều kiện đã sửa).
2. KT-184 → **DONE** (luật gốc được reviewer xác nhận).
3. Gate trên HEAD cuối `c8bef672` — PASS.
4. K6 giữ nguyên ảnh nguồn, không chạy lại soak/parity.
5. Không mở K4' bước 3, K5'; không sửa `spec/`.

---

**Ký**: Agent 1 (quyền A0), 2026-11-19.
