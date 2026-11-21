# KT-267 — ba nợ **ngoài** hợp đồng của luật `wake` (D2-205 §7)

**Ngày**: 2026-11-21 · **Task**: KT-267 · **Nhánh**: `task/kt267-wakedebt2` · **Worktree**:
`/tmp/kt268-s3` · **Base**: `main` @ `99a57e1a`.
**Tiền đề**: D2-205 (đóng KT-184/KT-194 theo hợp đồng hữu hạn D2-190 + chuyển ba nợ ngoài hợp
đồng sang KT-267), `JOURNAL/kt258-wake-attack.md` §7 (ba nợ có tên),
`reports/CHECKER-WAKE-CONTRACT.md` §4 + §7.3, `JOURNAL/kt264-struct-scope.md` (nợ §7.3 thứ ba
đã đóng ở KT-264).

**Ràng buộc đã giữ**: **không** push/merge, **không** chạm `main`, **không** sửa
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `spec/`, **không** sửa `v2/bin/v2`,
**không** nới ngưỡng, **không** tắt ca, **không** allowlist rộng. Mọi biến thể chèn vào cây đều
**khôi phục** bằng `trap` (`v2/kernel`, `v2/sim`, `v2/lib`, hai manifest workspace). Mọi thay đổi
có **test đỏ trước** (D2-05): ba ca âm (b) **đỏ** trên bản checker cũ (§3.3).

---

## 1. Cách đo

Harness mới `v2/agent/JOURNAL/kt267-attack/` (khuôn `kt264-attack/`, **không** sửa harness cũ):
`run-attack.sh <nhóm> <tên> <ca.sh>` chèn biến thể vào cây **thật**, đo
`cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf` + **ba**
lượt kiểm (`check-wake-paths.py`, `--strict`, `check-wake-depinfo.py`), rồi khôi phục bằng
`trap`. Mỗi ca có `EXTRA_CARGO` (cấu hình `--features product-fs`) để chứng minh mã **thật sự
được biên dịch**, không chỉ "cargo RC=0 vì crate không được build".

Chỉ tính là **đường lọt** khi `cargo` RC=0 **và** cả ba lượt `errors=0`. Bảng đầy đủ:
`kt267-attack.tsv`; log từng ca ở `kt267-attack/logs/<tên>/`.

---

## 2. Nợ (a) — `:expr` passthrough

### 2.1 Câu hỏi

Hợp đồng §4 nói `:expr` **không** bị từ chối vì `macro_rules! bail`
(`v2/kernel/vifast-plat-riscv64/src/user.rs:476`) dùng `$pages:expr, $e:expr` hợp lệ. Nợ:
`:expr` **có** chở được `include!`/`#[path]`/`mod` từ **tệp trong phạm vi** không?

### 2.2 Đo — hai ca thật

| Ca | Dựng gì | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| **A1** `a1-expr-include-call` | macro `($e:expr) => { $e }` **định nghĩa** ở `vifast-core/src/kt267_expr.rs`, **gọi** ở `vifast-core/src/kt267_call.rs` (cả hai **trong phạm vi**) với `include!("kt267_evil_expr.rs")` trong **đối số lời gọi**; tệp đích gọi `m.wake(t)` | 0 | 3 | 3 | 1 | **bị bắt** — `E_STRICT_MACRO_INCLUDE` ở **tệp gọi** (`kt267_call.rs:6`) |
| **A2** `a2-expr-block-path-mod` | `:expr` + **block** trong đối số chứa `#[path = "kt267_evil_mod.rs"] mod evil;`; tệp đích gọi `m.wake(t)` | 0 | 4 | 4 | 1 | **bị bắt** — `E_STRICT_MACRO_INCLUDE` (`#[path]`) + `E_STRICT_MACRO_MOD` (`mod`) ở **tệp gọi** |

**A2 là dạng mạnh nhất của `:expr`**: block là **biểu thức**, chứa được **item** `mod` + thuộc
tính `#[path]` — tức `:expr` **chở được** `mod`/`#[path]` **nếu** chúng nằm trong block ở **đối
số lời gọi**. Nhưng luật "đối số lời gọi" (KT-202) quét **toàn cây token** của đối số ⇒ **ĐỎ**.

### 2.3 `mod`/`#[path]` **trực tiếp** trong đối số `:expr` — lỗi cú pháp

Đo bằng `rustc 1.100.0` (`--edition 2021`):

```
t1.rs: macro_rules! pass { ($e:expr) => { $e }; } fn main() { let _ = pass!(mod evil;); }
  ⇒ error: no rules expected keyword `mod`
t2.rs: ... pass!(#[path = "x.rs"] mod evil;);
  ⇒ error: expected expression, found keyword `mod`
t3.rs: ... pass!({ #[path = "x.rs"] mod evil; 1u8 });
  ⇒ error: couldn't find file `x.rs`   (cú pháp HỢP LỆ — chỉ thiếu tệp)
```

⇒ `:expr` **không** chở được `mod`/`#[path]` **trực tiếp**; chỉ chở được qua **block**, và block
trong **đối số lời gọi** đã bị luật KT-202 bắt (A2).

### 2.4 Kết luận (a) — **ĐÓNG**

`:expr` **không** chở được `include!`/`#[path]`/`mod` từ **tệp trong phạm vi**. Ba lý do:
1. `include!` trong **đối số lời gọi** ⇒ `_scan_macro_calls` bắt ở **tệp gọi** (A1);
2. `mod`/`#[path]` trong **block** ở đối số ⇒ cùng luật bắt (A2);
3. `mod`/`#[path]` **trực tiếp** ⇒ **lỗi cú pháp** rustc.

**Ngoài phạm vi**: tệp ngoài `macro_roots()` **không** gọi được `wake` — phải có `vifast-core`
trong bao đóng dependency, mà crate như vậy **được nâng** lên bảo vệ (`_classify_scope`,
KT-256) ⇒ **được quét**. Crate duy nhất bị "từ chối rõ" là `v2/tools/vifast-agentctl` — **không**
dependency (đo: `Cargo.toml` không có `[dependencies]`) ⇒ **không** gọi được `wake`. Vậy nợ
**không** khai thác được.

**Không** vá `:expr` (không từ chối cả macro) — cây thật vẫn dùng `bail` hợp lệ. Đóng nợ bằng
**4 ca selftest canh** (§5).

---

## 3. Nợ (b) — miễn luật arch theo tiền tố `virtio_`

### 3.1 Trước vá

`check-wake-paths.py` (dòng ~4468): `if _m_door and not p.name.startswith("virtio_") and ...` —
**bất kỳ** tệp `virtio_*.rs` nào trong `vifast-plat-*` được miễn `E_ARCH_DOORBELL`, kể cả
`virtio_evil.rs` **không** phải driver.

### 3.2 Vá — kiểm **ngữ nghĩa**

```python
RE_VIRTIO_DRIVER = re.compile(
    r"\b(?:SplitQueue|VqLayout)\b"                       # virtqueue của crate `vifast_virtio`
    r"|\b(?:struct|enum|trait|union)\s+Virtio[A-Za-z0-9_]*\b"   # kiểu `Virtio*`
    r"|\bimpl\b[^\n{;]*\bVirtio[A-Za-z0-9_]*\b"          # `impl … VirtioKind`
)

def _is_virtio_driver(text: str) -> bool:
    return bool(RE_VIRTIO_DRIVER.search(strip_rust_noise(text)))
```

Marker đọc trên **mã** đã khử chú thích/chuỗi (`strip_rust_noise`) ⇒ **comment không đánh lừa
được**. Luật `doorbell` **giữ nguyên** độ rộng (khớp cả chú thích) — chỉ **miễn** đổi từ tên tệp
sang ngữ nghĩa.

**Vì sao marker là `SplitQueue`/`VqLayout`**: năm driver thật **không** có kiểu `Virtio*` hay
`impl … VirtioKind` trong **mã** (chỉ có trong doc-comment); chúng **đều** dùng virtqueue
(`SplitQueue` 3 lần, `VqLayout` 4–9 lần mỗi tệp). Đo:

| Tệp | `SplitQueue` | `VqLayout` | `VIRTIO_` | `vifast_virtio` | `VirtioKind` (mã) |
|---|---:|---:|---:|---:|---:|
| `virtio_blk.rs` | 3 | 4 | 5 | 4 | 0 |
| `virtio_console.rs` | 3 | 9 | 2 | 9 | 0 |
| `virtio_gpu.rs` | 3 | 6 | 6 | 4 | 0 |
| `virtio_input.rs` | 3 | 6 | 1 | 4 | 0 |
| `virtio_net.rs` | 3 | 8 | 3 | 4 | 0 |

### 3.3 Ca âm/dương trên cây thật

| Ca | Dựng gì | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| **B1** `b1-virtio-evil-code` | `vifast-plat-riscv64/src/virtio_evil.rs` (không marker) + `doorbell` trong **mã**; `mod virtio_evil;` trong `lib.rs` | 0 | 2 | 2 | 0 | **ĐỎ** `E_ARCH_DOORBELL` (2 dòng) |
| **B2** `b2-virtio-evil-comment` | như B1 nhưng `doorbell` chỉ trong **chú thích** | 0 | 1 | 1 | 0 | **ĐỎ** `E_ARCH_DOORBELL` (1 dòng) |

**Đỏ trước (D2-05)**: chạy **cùng** bộ selftest trên bản checker cũ (khôi phục
`p.name.startswith("virtio_")`) ⇒ **449/452**, ba ca âm (b) **`not ok`**; bản mới **452/452**.
Log: `/tmp/kt267-old-selftest.log` (chạy lại: §7).

**Ca dương**: năm driver thật ⇒ **XANH** — đo bằng `python3 v2/tools/check-wake-paths.py`
(`errors=0` cả hai chế độ) **và** ca selftest đối chứng trên **tệp thật** (§5).

### 3.4 Kết luận (b) — **ĐÓNG**

Miễn theo **ngữ nghĩa**, không theo tên tệp. `virtio_evil.rs` (không marker) ⇒ **ĐỎ**; driver
thật ⇒ **XANH**.

---

## 4. Nợ (c) — lớp dep-info chỉ kiểm feature mặc định

### 4.1 Đo — `wake` trong `proc_fs.rs` (chỉ biên dịch dưới `product-fs`)

`v2/kernel/vifast-kbin-riscv64/src/main.rs:435` khai `#[cfg(feature = "product-fs")] mod proc_fs;`
⇒ tệp **không** được biên dịch ở cấu hình mặc định.

| Phép đo | Kết quả |
|---|---|
| `cargo check --workspace` (mặc định) | RC=0 |
| `cargo check --workspace --features product-fs` | RC=0 |
| **lớp tĩnh** (`check-wake-paths.py`, cả hai chế độ) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` (`proc_fs.rs:660`) — tệp **trong phạm vi**, luật token **không** phụ thuộc feature |
| **lớp dep-info** (không cờ) | **XANH** `errors=0` (25,6 s) — **hở** |
| **lớp dep-info** `--product-fs` | **ĐỎ** `E_DEPINFO_WAKE` (`proc_fs.rs:660`) |

Ca: `c1-product-fs-wake` (`kt267-attack.tsv`); log `logs/c1-product-fs-wake/` +
`logs/c1-product-fs-wake/lop2-product-fs.log`.

### 4.2 Vá — lượt **tuỳ chọn** `--product-fs`

`check-wake-depinfo.py` thêm:

- `PRODUCT_FS_UNIT = ("kernel-product-fs", "v2/kernel/Cargo.toml",
  "riscv64gc-unknown-none-elf", "v2/kernel/target")` + `PRODUCT_FS_FEATURES = ("product-fs",)`;
- `run_unit(..., features=…)` ⇒ thêm `--features product-fs` vào lệnh cargo;
- `main` thêm cờ `--product-fs` (mặc định **TẮT**), thêm đơn vị thứ ba khi bật; target dir của
  lượt này = target dir của `kernel` (cùng workspace);
- `--no-build` + `--product-fs` ⇒ **từ chối** (exit 2): hai lượt build chia sẻ target dir nên
  dấu `--no-build` không xác định được thuộc lượt nào.

**Chi phí đo (ấm)**: 25,7 s → **45,9 s** ⇒ **+20 s**. (Task dự phóng +~40 s; đo thực tế thấp hơn
vì cache ấm. Build lạnh sẽ cao hơn — `--fresh` không đo trong task này.)

### 4.3 Kết luận (c) — **ĐÓNG**

Hai lớp **cùng** phủ:
1. **lớp tĩnh phủ đủ** — mọi tệp `product-fs` nằm trong phạm vi, luật token/transition/state_write
   **không** phụ thuộc feature (đo: `proc_fs.rs` ⇒ ĐỎ cả hai chế độ);
2. **lớp dep-info** có lượt **tuỳ chọn** `--product-fs` (đo: không cờ XANH, có cờ ĐỎ).

**Đề xuất lead**: nối `--product-fs` vào `gate --fast` (không tự sửa `v2/bin/v2`). Nếu nối, chi
phí `gate --fast` tăng ~+20 s ấm.

---

## 5. Selftest trước/sau

| Lớp | Trước | Sau | Tăng | Ca mới |
|---|---:|---:|---:|---|
| **tĩnh** (`check-wake-paths.py --selftest`) | **442/442** | **452/452** | **+10** | (a) **+4**: `:expr`+`include!` **khác tệp** ⇒ ĐỎ; `:expr`+block `#[path]` ⇒ ĐỎ; `:expr`+block `mod` ⇒ ĐỎ; đối chứng dương block biểu thức thường ⇒ XANH. (b) **+6**: 3 ca âm (`virtio_evil.rs` mã/chú thích; marker chỉ trong chú thích) + 2 ca dương (`SplitQueue`/`VqLayout`; kiểu `Virtio*`) + **1 đối chứng dương trên tệp THẬT** (5 driver virtio). |
| **dep-info** (`check-wake-depinfo.py --selftest`) | **67/67** | **70/70** | **+3** | (c) `PRODUCT_FS_UNIT`/`PRODUCT_FS_FEATURES` đúng; `run_unit` thêm `--features product-fs` (và **không** thêm khi rỗng); `--no-build`+`--product-fs` ⇒ exit 2. |

**Đỏ trước (D2-05)**: ba ca âm (b) **`not ok`** trên bản checker cũ (449/452); ca (c) `run_unit`
**TypeError** trên bản cũ (không có tham số `features`). Ca (a) là **ca canh** (luật KT-202 đã
có) — không phải ca luật mới.

---

## 6. Tệp thay đổi trong KT-267

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | `RE_VIRTIO_DRIVER` + `_is_virtio_driver()` (b); miễn `doorbell` theo ngữ nghĩa thay tên tệp; **+10 ca** selftest (4 cho (a), 6 cho (b)) |
| `v2/tools/check-wake-depinfo.py` | `PRODUCT_FS_UNIT`/`PRODUCT_FS_FEATURES`; `run_unit(features=…)`; cờ `--product-fs`; từ chối `--no-build`+`--product-fs`; **+3 ca** selftest |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §4: ba dòng nợ → **ĐÓNG** (bằng chứng đo); §6: lệnh `--product-fs`; **§7.3 mới**: bảng trạng thái ba nợ |
| `v2/agent/JOURNAL/kt267-debts.md` | nhật ký này |
| `v2/agent/JOURNAL/kt267-attack/**` | harness + 5 ca + log + `kt267-attack.tsv` |

**Không** sửa: `v2/kernel/**`, `v2/sim/**`, `v2/lib/**`, `spec/`, `v2/bin/v2`,
`v2/agent/{STATE,QUEUE,DECISIONS}.md`.

---

## 7. Tái lập

```bash
cd /tmp/kt268-s3
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + cây thật
python3 v2/tools/check-wake-paths.py --selftest          # 452/452
python3 v2/tools/check-wake-depinfo.py --selftest        # 70/70
python3 v2/tools/check-wake-paths.py                     # errors=0
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py                   # errors=0
python3 v2/tools/check-wake-depinfo.py --product-fs      # errors=0 (+20 s ấm)

# (b) một biến thể (tự khôi phục bằng trap)
bash v2/agent/JOURNAL/kt267-attack/run-attack.sh "B-virtio" \
     b1-virtio-evil-code v2/agent/JOURNAL/kt267-attack/cases/b1-virtio-evil-code.sh

# (c) cổng
./v2/bin/v2 check
```

**Đỏ trước (b)**: chép `check-wake-paths.py` thành `v2/tools/_kt267_old.py`, đổi
`if _m_door and not virtio_driver and ...` thành
`if _m_door and not p.name.startswith("virtio_") and ...`, chạy `--selftest` ⇒ **449/452**
(ba ca âm (b) `not ok`); xoá tệp tạm.
