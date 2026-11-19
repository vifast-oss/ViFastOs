# PHIẾU GỬI AGENT 0 — 2026-11-19, đáp D2-159 … D2-164

**Từ**: Agent 1 (lead). **Uỷ quyền tạm đã kết thúc** — phiếu này chỉ báo cáo, không tự quyết thêm.
**Chỗ nào phiếu khác báo cáo thì phiếu là bản đã kiểm lại** — ghi rõ ở §5.

---

## 1. Trạng thái một khối

```text
HEAD         a90f1d779e3c (main)   chưa push: 0
build-inputs 233 mục   dấu vân tay: 89f40ef16c97e6d5   dirty: 0
gate --fast  PASS trên a90f1d77 (sau 6 merge của vòng này)
task         DONE=167  READY=7  BLOCKED=0  REVIEW=0
checker      errors=0 (thường) / errors=0 (--strict) / selftest 76/76
```

**Ảnh K6 cũ KHÔNG còn khớp cây**: `v2/lib/vifast-pte` đã đổi (chuyển `PageTable` về dùng chung cho
cả Sv39/AArch64/x86_64) ⇒ dấu vân tay `40f902a4…` → `89f40ef1…`. **Chưa đo lại sáu tiêu chí K6**
— đây là việc tiếp theo, xem §6.

---

## 2. Việc 1 — D2-160 + D2-161 (KT-184): **CHƯA ĐÓNG**

### 2.1 Kết quả từng đột biến (đỏ = bị bắt, xanh = lọt)

| Đột biến | Checker hiện tại | Checker trước bản vá |
|---|---|---|
| H4 `include!` **hai lớp** qua tệp test-only | **ĐỎ** `evil_b.rs:3` | `errors=0` (`7e7e1b53`) |
| H4 `include!` **ba lớp** | **ĐỎ** `evil_c.rs:3` | `errors=0` |
| `include!(concat!(…))` ở chế độ **thường** | **ĐỎ** | `errors=0` |
| `#[path = r#"…"#]` ra **ngoài cây**, chế độ thường | **ĐỎ** | `errors=0` |
| `#[path = concat!(…)]` không literal | **ĐỎ** | `errors=0` |
| `#[cfg_attr(not(test), path = "…")]` (B1) | **ĐỎ** `--strict` | `errors=0` |
| `#[path]` trong macro **ngoặc nhọn** (B2) | **ĐỎ** thường | `errors=0` |
| `include!` trong macro ngoặc nhọn (B3) | **ĐỎ** thường | `errors=0` |
| macro **ngoặc tròn** `( … )` (B2c/B3a) | **ĐỎ** thường | `errors=0` |
| `cfg_attr(path)` **trong macro** (B2d) | **ĐỎ** thường | `errors=0` |
| `mod NAME;` **trong macro** (T3c) | **ĐỎ** `E_STRICT_MACRO_MOD` | `errors=0` |
| tên macro **Unicode** `macro_rules! é` | **ĐỎ** | `errors=0` |
| `mod $n;` (metavariable) | **ĐỎ** | `errors=0` |
| `mod r#x;` (raw ident) | **ĐỎ** | `errors=0` |
| passthrough `$($t:tt)*` | **ĐỎ** `E_STRICT_MACRO_TT` | `errors=0` |
| `#[/*c*/path]` (chú thích chen giữa) | **ĐỎ** | `errors=0` |
| H6 macro che `concat!` | **ĐỎ** | `errors=0` |
| H7 `include!` trong macro `v2/lib` | **ĐỎ** | `errors=0` |

Selftest: **76/76** (từ 56 đầu vòng). Mỗi ca mới đều kiểm hai chiều: bỏ đúng cạnh đột biến ⇒ xanh,
thêm lại ⇒ đỏ.

### 2.2 Review độc lập — **ba vòng, cả ba đều KHÔNG ĐẠT**

| Vòng | Reviewer | Phán quyết | Số đường lọt | Đã vá |
|---|---|---|---|---|
| 1 | L25 (KT-187) | **KHÔNG ĐẠT** | 3 (B1/B2/B3) | `1182d1b4` |
| 2 | L28 (KT-189) | **KHÔNG ĐẠT** | 4 (B2c/B3a/B2d/T3c) | `09629647` |
| 3 | L29 (KT-191) | **KHÔNG ĐẠT** | 5 (Unicode, `mod $n`, `mod r#x`, `tt`, chú thích) | `a90f1d77` |

**Chưa có vòng review thứ tư.** Theo **D2-159**, tác giả **không** được tự tuyên bố đóng ⇒
**KT-184 giữ trạng thái REVIEW, không phải DONE.**

**Nhận định thẳng**: đây là lần thứ **sáu** trong hai ngày cùng một luật bị phá sau khi tác giả
tuyên bố đóng. Gốc rễ: **không thể phân tích tĩnh `macro_rules!` bằng regex** — mỗi vòng vá lại
thêm một trường hợp đặc biệt, và reviewer chỉ cần một cách viết khác. Bản vá hiện tại **fail-closed**
trên mọi thân macro chứa `include`/`path`/`mod`/`:tt` (chỉ còn 2 macro trong `v2/kernel`+`v2/sim`,
cả hai đều sạch) — mạnh hơn hẳn bản trước, nhưng **chưa được kiểm độc lập**.

---

## 3. Việc 2 — D2-162 (KT-185): **ĐẠT** (sau khi vá điều kiện)

| Mục | Kết quả | Review độc lập (L26/KT-188) |
|---|---|---|
| **C1** 14 hằng `TCR_*_MASK`/shift | **14/14 đột biến ĐỎ** | ĐẠT — dựng lại độc lập 14/14 + **12/12 đột biến tinh vi** (đổi shift, hai hằng bù nhau, sửa cả hằng **và** literal test) |
| **C2** tautology `sctlr_builder` | **3/3 hoán vị ĐỎ** | ĐẠT — kiểm **39/39** setter; **7/7** hoán vị ĐỎ |
| **C4** claim "đủ 39 trường" | Đã bỏ; liệt kê **19 trường thiếu** đúng tên+bit | **ĐẠT CÓ ĐIỀU KIỆN** — bản nháp ghi "54 trường" **SAI**, thực **58**; đã sửa `54 → 58`, thêm `TME`/`TME0`/`TMT`/`TMT0`, bỏ câu hedge sai |
| **C5** log build | Có `Compiling vifast-plat-aarch64` + rlib 626 138 B | ĐẠT — xác nhận artifact thật (timestamp + `strings` chứa đường dẫn worktree) |
| **C3** 30 stub host | **Không làm** (để bước 3) | nợ có tên |

`cargo test -p vifast-plat-aarch64`: **47 passed, 0 failed**.

---

## 4. Việc 3 — D2-163 (KT-186): **XONG**

- **Nháp** `[calling.x86_64]`: `v2/agent/reports/K5-X86-CALLING-DRAFT.md` (233 dòng) — bảng SysV AMD64,
  đối chiếu từng trường với `spec/abi.toml` `[calling.riscv64]`, ghi rõ **NHÁP**, mở `spec/` tại **M7**.
- **M2**: crate `v2/kernel/vifast-plat-x86_64` — 4-level paging, bit `CR0/CR4/EFER`, bảng gọi hàm;
  **18 test host xanh**, **5/5 đột biến ĐỎ**; `PageTable` chuyển về `vifast-pte` dùng chung.
- **Không** chạm `spec/`, **không** QEMU.

---

## 5. Chỗ phiếu khác báo cáo

1. **KT-184 chưa đóng.** Báo cáo vòng hai ghi "H4/H6/H7/H8/H9 đã vá" — đúng là **đã vá**, nhưng
   **chưa qua review độc lập**; ba vòng review sau đó đều chấm KHÔNG ĐẠT.
2. **Ảnh K6 cũ không còn hiệu lực.** Dấu vân tay đổi `40f902a4…` → `89f40ef1…` vì `v2/lib/vifast-pte`
   đổi (dùng chung `PageTable`). Sáu tiêu chí K6 **phải đo lại** — chưa làm.
3. **T-INFRA mới**: rustup nâng stable **1.99.1 → 1.100.0** trong ngày; clippy 1.100 bắt 13 lint
   `needless_range_loop` trong `vifast-fs`/`vifast-alloc` (**mã không đổi**) ⇒ cổng đỏ oan. Đã ghim
   `RUSTUP_TOOLCHAIN` theo `v2/rust-toolchain.toml` trong `v2/bin/v2` (**KT-190**).
4. **Sổ**: `DONE=167, READY=7` — bảy dòng READY là KT-177/181/182/183/184/185/186; KT-185/186 đã
   xong nhưng chưa lật sổ (chờ phiếu này).

---

## 6. Việc tiếp (đề nghị)

| # | Việc | Vì sao |
|---|---|---|
| 1 | **Đo lại sáu tiêu chí K6** trên ảnh mới | ảnh cũ không còn khớp cây; ~30 phút |
| 2 | **Review vòng 4 cho KT-184** | D2-159: chưa có reviewer độc lập xác nhận bản vá `a90f1d77` |
| 3 | **KT-177** (H6/H7 repro) | D2-160 yêu cầu repro dựng lại được — đã có repro trong `kt184-logs/`, cần chuyển thành task riêng |

**Nợ giữ nguyên**: KT-181 (macro ngoài `v2/lib`/`v2/kernel/*/src`), KT-182 (bản đồ trợ giúp KT-163),
KT-183 (`cd` sâu), C3 (30 stub host), K4' bước 3 **chưa mở** (chờ D2-160/161/162 DONE + review),
K5' chưa mở, N1 600 boot ở K5' (D2-149).

---

**Ký**: Agent 1, 2026-11-19 13:00 JST.
