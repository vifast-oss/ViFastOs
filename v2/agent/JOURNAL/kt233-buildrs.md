# KT-233 — đo nợ: `build.rs` sinh mã vào `$OUT_DIR` rồi `include!`

- **Người đo**: subagent L56 (task KT-233; nhánh `night3/L56-kt233`, từ `cbc2c2af`).
- **Nợ đo**: D2-180 §4 + D2-181 — "`build.rs` sinh mã vào `$OUT_DIR` rồi `include!` — **chưa đo**;
  lớp 2 chỉ **cảnh báo** (`W_DEPINFO_SKIPPED`) tệp trong `target/`".
- **Kết luận**: **5 dạng đề ra KHÔNG SỐNG** — lớp 1 bắt **cả 5**, ở **cả hai** chế độ.
  **SỐNG ở biến thể 6**: cùng cơ chế (`build.rs` → `$OUT_DIR` → `include!`) nhưng crate chứa nó
  nằm **ngoài ba gốc quét** của lớp 1 (`v2/kernel`, `v2/sim`, `v2/lib`) ⇒ `cargo check` RC=0,
  lớp 1 XANH, lớp 1 `--strict` XANH, lớp 2 XANH, và `./v2/bin/v2 check` **PASS**.
- **Báo oan**: **1** — đối chứng dương (`include!` `$OUT_DIR` với tệp **không** chứa `wake`) vẫn
  **ĐỎ** ở lớp 1 (`E_STRICT_WAKE_INCLUDE`): lớp 1 cấm **mọi** `include!` không literal, không
  phân biệt mã lành/mã độc.

---

## 1. Cách đo

- Worktree `/private/tmp/night3-L56-kt233`, `HEAD = cbc2c2af`, toolchain ghim `1.99.1`
  (`v2/rust-toolchain.toml`), `cargo` 1.100.0 mặc định của máy **không** dùng.
- Crate đặt mã: `v2/sim/vifast-sim` (host workspace, `Machine` + `Machine::wake` có thật trong
  `src/kobj.rs:5732`; `ObjId = u32`). Mã "độc" luôn là:

  ```rust
  pub fn evil(m: &mut crate::kobj::Machine, t: crate::kobj::ObjId) {
      let _ = m.wake(t);
  }
  ```

- Bốn phép đo cho **mỗi** dạng (đúng định nghĩa "đường lọt SỐNG" của đề):
  1. `RUSTUP_TOOLCHAIN=1.99.1 cargo check --manifest-path v2/Cargo.toml -p vifast-sim` → RC;
  2. `python3 v2/tools/check-wake-paths.py --root .` (lớp 1 thường);
  3. `python3 v2/tools/check-wake-paths.py --root . --strict` (lớp 1 `--strict`);
  4. `python3 v2/tools/check-wake-depinfo.py --root .` (lớp 2).
- Ba lớp chạy **song song** sau `cargo check`; log đầy đủ ở `v2/agent/JOURNAL/kt233-logs/`
  (`<dạng>-cargo.log`, `<dạng>-l1.log`, `<dạng>-l1strict.log`, `<dạng>-l2.log`,
  `<dạng>-patch.diff`).
- **Đường nền** (cây sạch, trước mọi ca): lớp 1 `errors=0` (32 s), lớp 2 `errors=0` (19,5 s).

## 2. Bảng kết quả

| Dạng | `cargo check` | lớp 1 | lớp 1 `--strict` | lớp 2 | KL |
|---|---|---|---|---|---|
| 1. `include!(concat!(env!("OUT_DIR"), "/evil.rs"))` | **0** | **1** `E_STRICT_WAKE_INCLUDE` | **1** | **0** (`W_DEPINFO_SKIPPED`) | không sống |
| 2. `include!` trong `macro_rules!` gọi từ tệp khác | **0** | **1** `E_STRICT_MACRO_INCLUDE` | **1** | **0** (`W_DEPINFO_SKIPPED`) | không sống |
| 3. `build.rs` ghi `src/gen_evil.rs` rồi `mod` | **0** | **1** `E_STRICT_TEST_ONLY_WAKE` | **1** | **1** `E_DEPINFO_WAKE` | không sống |
| 4. `#[path = concat!(env!("OUT_DIR"), "/evil.rs")] mod evil;` | **101** | **1** `E_STRICT_WAKE_INCLUDE` | **1** | **1** `E_DEPINFO_BUILD` | không sống (không biên dịch) |
| 5. `build.rs` sinh `macro_rules!` rồi `include!` | **0** | **1** `E_STRICT_WAKE_INCLUDE` | **1** | **0** (`W_DEPINFO_SKIPPED`) | không sống |
| **6. biến thể: crate thành viên ngoài 3 gốc quét** | **0** | **0** | **0** | **0** (`W_DEPINFO_SKIPPED`) | **SỐNG** |
| ĐC âm: `include!("evil_gen.rs")` literal trong `src/` | **0** | **1** `E_STRICT_TEST_ONLY_WAKE` | **1** | **1** `E_DEPINFO_WAKE` | đúng (ĐỎ) |
| ĐC dương: `include!` `$OUT_DIR`, tệp `ok.rs` **không** `wake` | **0** | **1** `E_STRICT_WAKE_INCLUDE` | **1** | **0** (`W_DEPINFO_SKIPPED`) | **báo oan** |

## 3. Vì sao 5 dạng đề ra không sống

Cả năm đều bị **lớp 1** bắt, không phải lớp 2:

- **Dạng 1, 5** — `include!`/`#[path]` **không literal** ⇒ `E_STRICT_WAKE_INCLUDE` ở **mọi** chế
  độ (KT-184/D2-161 đã đưa khối phân giải ra ngoài `if strict:`). Lớp 1 không cần biết đích là
  `$OUT_DIR`; nó chỉ cần biết **không phân giải tĩnh được** ⇒ fail-closed.
- **Dạng 2** — `include!` nằm trong thân `macro_rules!` ⇒ `E_STRICT_MACRO_INCLUDE`
  (`v2/sim/vifast-sim/src/genmac.rs:4`), đúng lớp H7/KT-177 + KT-202.
- **Dạng 3** — tệp `build.rs` ghi vào `src/` là tệp **`.rs` trong cây** ⇒ `iter_rs` quét, luật
  token `wake` bắt `E_STRICT_TEST_ONLY_WAKE`; lớp 2 cũng bắt `E_DEPINFO_WAKE` (rustc nạp tệp
  thật). Đây là dạng **duy nhất** mà lớp 2 một mình cũng đủ.
- **Dạng 4** — Rust **không cho** giá trị attribute không literal: `cargo check` RC=101
  (`malformed path attribute input`), nên không thể là đường lọt; lớp 1 vẫn báo
  `E_STRICT_WAKE_INCLUDE` (đúng, fail-closed).

**Điểm cốt lõi**: với cơ chế `$OUT_DIR` + `include!`, thứ đang đỡ cổng **không phải** lớp 2 (lớp 2
chỉ cảnh báo — đúng như nợ mô tả) mà là **lệnh cấm toàn phần** của lớp 1 với `include!`/`#[path]`
không literal. Đó là fail-closed **đúng hướng**, nhưng là "cấm" chứ không phải "đo": nó không
phân biệt mã sinh lành với mã sinh độc (xem §5).

## 4. Biến thể 6 — đường lọt SỐNG

### 4.1 Dựng

Thêm crate thành viên **ngoài** `v2/kernel`/`v2/sim`/`v2/lib`:

```
v2/extra/evil/Cargo.toml      # name = "vifast-evil"; dep path vifast-sim
v2/extra/evil/build.rs        # ghi $OUT_DIR/evil.rs = mã "độc" ở trên
v2/extra/evil/src/lib.rs      # include!(concat!(env!("OUT_DIR"), "/evil.rs"));
v2/Cargo.toml                 # members += "extra/evil"
```

`v2/extra/evil/src/lib.rs` **không** chứa token `wake`; token đó chỉ có trong tệp do `build.rs`
sinh ra: `v2/target/debug/build/vifast-evil-21dcaed65d956140/out/evil.rs`.

### 4.2 Số đo

| Phép đo | Kết quả |
|---|---|
| `cargo check -p vifast-evil` | **RC=0** |
| lớp 1 thường | **XANH** `errors=0 warnings=0` |
| lớp 1 `--strict` | **XANH** `errors=0 warnings=0` |
| lớp 2 | **XANH** `errors=0 warnings=1` — chỉ `W_DEPINFO_SKIPPED` cho `…/out/evil.rs` |
| `./v2/bin/v2 check` (cổng đầy đủ: fmt + clippy + luật + cả ba lớp wake) | **PASS** (3 m 09 s) |

Bằng chứng `.d`: `v2/target/debug/deps/vifast_evil-128b1201c00c2cfb.d` **có** liệt kê
`…/out/evil.rs` — tức lớp 2 **thấy** tệp, nhưng bỏ qua vì nằm trong thư mục build.

### 4.3 Nguyên nhân (hai lỗ độc lập, chỉ cần một cái là đủ để lớp 2 không cứu)

1. **Lớp 1 quét theo ba gốc cứng** — `iter_rs(root, ("v2/kernel", "v2/sim"))` + `v2/lib/**`
   (`check-wake-paths.py`). Crate thành viên workspace ở chỗ khác (`v2/extra`, `v2/tools`, …)
   **không** vào `files`, không vào `extra`, không vào `scan_macro_bodies` ⇒ mù hoàn toàn.
2. **Lớp 2 bỏ tệp trong thư mục build** — `_is_build_dir` lọc `…/target/…` rồi chỉ ghi
   `W_DEPINFO_SKIPPED` (KT-227 P0-3). Tệp `$OUT_DIR` **là** mã sản phẩm biên dịch thật, nhưng
   bị coi là "thư mục build" ⇒ không quét.

Hệ quả: **hai lớp bù trừ nhau đúng ở chỗ yếu nhất** — lớp 1 mù theo *vị trí crate*, lớp 2 mù theo
*vị trí tệp*; giao của hai vùng mù là `$OUT_DIR` của một crate ngoài ba gốc.

### 4.4 Phạm vi / mức nghiêm trọng

- Cần **thêm một crate vào workspace** (hoặc thêm `build.rs` + dep vào crate sẵn có **ngoài** ba
  gốc — `v2/tools/vifast-agentctl` là ứng viên: chỉ cần thêm `vifast-sim` làm dependency là cùng
  đường). Đây là thay đổi **cấu trúc** nhìn thấy được khi review, **không** phải đường lọt chỉ
  sửa một tệp.
- Nhưng `./v2/bin/v2 check` **không** bắt: fmt/clippy/luật cấu trúc/cả ba lớp wake đều XANH.
  Nghĩa là nếu crate đó lọt qua review, cổng về sau **không** còn là lưới an toàn cho luật `wake`.
- Đây **đúng cơ chế nợ** D2-180 §4/D2-181 (build.rs → `$OUT_DIR` → `include!`), chỉ khác **crate
  chứa nó**. Nợ "chưa đo" nay đã đo: **5/5 dạng trong cây là ĐỎ; dạng ngoài cây là SỐNG**.

## 5. Báo oan (đối chứng dương)

`include!(concat!(env!("OUT_DIR"), "/ok.rs"))` với `ok.rs` chỉ có `pub fn ok() -> u32 { 42 }`:

- `cargo check` RC=0; lớp 2 XANH (chỉ cảnh báo tệp build);
- **lớp 1 ĐỎ** `E_STRICT_WAKE_INCLUDE` — "`include!` không phải literal chuỗi — không phân giải
  được đích".

Tức lớp 1 **không** phân biệt được mã sinh lành với mã sinh độc: mọi `build.rs` sinh mã Rust rồi
`include!` đều bị chặn, kể cả khi tệp sinh không hề chạm `wake`. Đây là **báo oan có chủ đích**
(fail-closed) — nhưng cần ghi nhận vì nó là lý do **duy nhất** khiến 5 dạng đề ra "không sống":
nếu một ngày nào đó nới lệnh cấm này (ví dụ allowlist `concat!(env!("OUT_DIR"), …)`), **cả 5 dạng
sống lại ngay** trừ dạng 3.

## 6. Đề xuất (task này **chỉ đo**, không sửa công cụ)

1. **Lớp 1 — gốc quét theo workspace member**, không theo ba thư mục cứng: đọc `members` của
   `v2/Cargo.toml` + `v2/kernel/Cargo.toml` rồi quét `src/**` + `build.rs` của **mọi** thành viên.
   Đây là bản vá nhỏ nhất đóng biến thể 6 mà không nới gì.
2. **Lớp 2 — tệp trong `target/` mà rustc nạp là mã sản phẩm**: hoặc quét luôn (nó là mã biên
   dịch thật), hoặc fail-closed khi tệp đó chứa `wake`/`transition_tcb(…, Ready)` thay vì chỉ
   `W_DEPINFO_SKIPPED`. Ít nhất: **đếm** số tệp bị bỏ và ĐỎ khi > 0 tệp bị bỏ **có** token `wake`.
3. **Đối chứng dương thành ca selftest**: `include!` `$OUT_DIR` + tệp lành phải XANH — nếu giữ
   lệnh cấm toàn phần thì ghi rõ trong hợp đồng §4 là "cấm có chủ đích", kèm ca khẳng định
   (để lần sau không ai "vá" bằng cách nới nó mà không biết hậu quả).

## 7. Bằng chứng

- Log: `v2/agent/JOURNAL/kt233-logs/` — `{1,2,3,4,5,neg,pos,6}-{cargo,l1,l1strict,l2}.log`,
  `{1,2,3,4,5,neg,pos,6}-patch.diff`, `6-v2check.log`.
- Tệp sinh thật (biến thể 6): `v2/target/debug/build/vifast-evil-21dcaed65d956140/out/evil.rs`
  (nội dung trong `6-patch.diff`); `.d` tương ứng: `v2/target/debug/deps/vifast_evil-*.d`.
- Cây sau khi đo: `git status` sạch (chỉ còn `v2/agent/JOURNAL/kt233-logs/` + journal này);
  `v2/Cargo.toml`, `v2/Cargo.lock`, `v2/sim/vifast-sim` đã revert.
