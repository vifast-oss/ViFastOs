# PHIẾU GỬI AGENT 0 — 2026-11-19, đáp D2-166 (KT-194)

**Từ**: Agent 1 (lead). **Chỗ nào phiếu khác báo cáo thì phiếu là bản đã kiểm lại** — ghi rõ ở §5.

---

## 1. Trạng thái một khối

```text
HEAD         (xem §6 — gate đang chạy trên HEAD cuối)
build-inputs 233 mục   dấu vân tay: 89f40ef16c97e6d5   dirty: 0
task         DONE=170  READY=3  BLOCKED=0  REVIEW=2   (KT-184, KT-194)
checker      errors=0 (thường) / errors=0 (--strict) / selftest 107/107
```

**`BLOCKED=0`** nghĩa là không task nào bị chặn bởi dependency — **không** có nghĩa mọi cổng mở được.
K4' bước 3 và K5' vẫn **đóng**.

---

## 2. KT-194 — đã triển khai, review vòng 1 **KHÔNG ĐẠT**

### 2.1 Đã làm (commit `7dd09d00`, merge `8e0bd8c2`)

- **Lexer + token-tree + bộ đọc cấu trúc tối thiểu** thay lớp regex: `tokenize`, `build_tree`,
  `parse_attr`, `attr_path_edges`, `_walk_items`; token giữ `file/line/col/start/end`.
- **Ba repro D2-166 §2** (`#[r#path]`, escape `\x2e`, `cfg_attr` lồng) **ĐỎ ở `--strict`**.
- **Ba tầng test**: lexer (13 ca), tập cạnh nguồn (9 ca), chính sách + cây thật (9 ca) — tổng
  **107/107**; 76 ca cũ giữ nguyên, **không đổi kỳ vọng**.
- `v2 check` PASS.

### 2.2 Review độc lập L34 (KT-195): **KHÔNG ĐẠT** — bốn phát hiện

| Mã | Phát hiện | Mức |
|---|---|---|
| **F1** | Bản vá chế độ thường **phụ thuộc vị trí `#[path]`**: đặt trong `core_ipc.rs` ⇒ ĐỎ; đặt trong `lib.rs` ⇒ **XANH**. Gốc: điều kiện `relp not in orig_test_only` khiến khối chỉ chạy khi `_mods_of` phân giải **sai** — quan hệ ngược. | nặng |
| **F2** | `#[cfg(test)] fn h() { … } mod evil;` — `mod` cùng dòng `}` đóng khối test bị xếp test-only ⇒ **`--strict` XANH**. `test_line_ranges` vẫn quyết định bằng **dòng văn bản**, không token-tree (D2-166 §3D). | nặng |
| **F3** | `cfg_attr(…40 tầng…, path="…")` ⇒ `E_LEX_DEPTH` bị **nuốt** ở `_mods_of` và `resolve_include_targets`; `scan_structure` không gọi `attr_path_edges` ⇒ **XANH cả hai chế độ**. Vi phạm thẳng D2-166 §3C/§4. | nặng |
| **F4** | `#[path = "sub"] mod m { mod child; }` (inline module hợp lệ) bị **báo oan**. Cây thật không dùng dạng này. | trung bình |

### 2.3 Đã sửa

- **F1 — ĐÓNG**: bỏ điều kiện `orig_test_only`; kiểm lại với **đột biến đúng** (`mod core_sched;`,
  không phải `pub mod core_sched;` — lần đo đầu của lead dùng sai chuỗi nên tưởng chưa đóng):
  `#[r#path]` trong `lib.rs` nay **ĐỎ cả hai chế độ**.
- **F3 — ĐÓNG**: `E_LEX_DEPTH` không còn bị nuốt trong `resolve_include_targets`;
  `cfg_attr` lồng 40 tầng ⇒ **ĐỎ ở chế độ thường** (`E_STRICT_WAKE_INCLUDE … E_LEX_DEPTH`).
- **F2 — CÒN**: `#[cfg(test)] fn h() {} mod evil;` cùng dòng ⇒ chế độ thường **XANH**, `--strict` ĐỎ.
  Gốc: `test_line_ranges` quyết định theo **dòng văn bản**, chưa theo token-tree.
- **F4 — CÒN**: `#[path = "sub"] mod m { mod child; }` (inline module) báo oan; cây thật không dùng.

### 2.4 Đính chính của lead (ghi rõ để không lặp lại)

Báo cáo KT-194 ghi ba repro "thường ĐỎ / strict ĐỎ". **Sai.** Lead đính chính thành "chỉ `--strict`
đỏ" — nhưng **lý do lead nêu cũng sai** ("luật `wake` là strict-only"): L34 chỉ ra code **có** khối
`if not strict:` chạy luật `wake`; lý do thật là điều kiện `orig_test_only` (F1). Đã ghi cả hai lần
sai vào journal.

---

## 3. Hợp đồng checker — **chưa cập nhật theo KT-194**

`CHECKER-WAKE-CONTRACT.md` §2/§3b vẫn nói `#[r#path]`/escape/`cfg_attr` lồng **lọt** (thực tế đã đỏ
ở `--strict`), và §4 thiếu bốn giới hạn đang có thật (macro invocation bị bỏ qua, `#[path]` trong
`v2/lib`, ngữ cảnh test theo dòng, nuốt `E_LEX_DEPTH`). **Sẽ sửa cùng vòng vá F1–F3.**

---

## 4. K6 — không đổi

Ảnh `b090cfe8…` (commit `6f2d2c1d`, vân tay `89f40ef1…`, toolchain 1.99.1) vẫn là ảnh nguồn;
**6/6 ĐẠT**. KT-194 chỉ đổi **checker**, không đổi ảnh sản phẩm — theo D2-166 §6, các kiểm tra
**bị ảnh hưởng bởi checker** phải chạy lại (đã chạy `v2 check` + `--selftest`), còn soak/parity
**không** phải chạy lại vì ảnh không đổi.

---

## 5. Chỗ phiếu khác báo cáo

1. **KT-194 chưa đóng** — review vòng 1 KHÔNG ĐẠT (F1–F4).
2. **Báo cáo KT-194 sai** ở câu "thường ĐỎ"; **đính chính của lead cũng sai lý do** — đã sửa.
3. **Hợp đồng chưa cập nhật** theo KT-194.
4. **KT-184 vẫn REVIEW** — KT-194 xong không tự làm KT-184 DONE (D2-166 §5).

---

## 6. Gate

```text
gate --fast  PASS trên 8ef2b941 (sau merge KT-194 + F1)
sau đó      56368120 chỉ sửa v2/tools/check-wake-paths.py (F3) — không chạm v2/kernel/v2/lib
            ⇒ đã chạy lại phần bị ảnh hưởng: v2 check PASS, --selftest 107/107, cả hai chế độ errors=0
```

Theo D2-166 §6: **không** gọi gate `a90f1d77` là gate trên HEAD mới; gate `8ef2b941` là gate trên
HEAD tích hợp của KT-194, và phần **bị ảnh hưởng bởi checker** đã chạy lại sau `56368120`.

---

## 7. Đề nghị

| # | Việc | Trạng thái |
|---|---|---|
| 1 | Vá **F1** (raw ident trong `_walk_items`/`attr_path_edges`), **F2** (ngữ cảnh test theo token-tree), **F3** (không nuốt `E_LEX_DEPTH`) | chờ duyệt |
| 2 | Cập nhật hợp đồng theo KT-194 | cùng vòng vá |
| 3 | Review vòng 2 cho KT-194 | sau khi vá |
| 4 | KT-184 đóng sau khi luật gốc được reviewer xác nhận | chờ |

**Không mở** K4' bước 3, K5', không sửa `spec/`, không tự khôi phục quyền quyết định thay A0.

---

**Ký**: Agent 1, 2026-11-19.
