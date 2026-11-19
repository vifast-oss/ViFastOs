# KT-214 — Nguyên mẫu: kiểm `wake` theo **danh sách tệp THẬT của rustc** (`--emit=dep-info`)

> **ĐÍNH CHÍNH KT-227 (2026-11-20, L53).** Journal này đo ở gốc `dc6e01ae` — **trước** khi
> KT-211/KT-213/KT-215 merge. Trên HEAD `83275d94`, checker tĩnh bắt **11/11** đường (6 đường
> trước đây XANH nay ĐỎ bằng fail-closed `E_STRICT_WAKE_INCLUDE`/`E_STRICT_TEST_ONLY_WAKE`).
> Vì vậy mọi câu "checker tĩnh chỉ bắt 5/11" dưới đây là **số đo lịch sử**, **không** phải hiện
> trạng. Giới hạn #6 dưới đây cũng **sai một phần**: lớp 1 **cũng không** bắt
> `#[cfg(feature = "ktest")] mod` (miễn test-only) — đã vá ở KT-227 P0-4. Bằng chứng tái lập:
> `kt224-logs/p02-p07-tai-lap-head.log`; chi tiết: `kt227-layer2-fixes.md`.

**Người làm**: L48 (sub-agent của Agent 1) · **Nhánh**: `night3/L48-kt214` · **Gốc**: `dc6e01ae`
**Công cụ mới**: [`v2/tools/check-wake-depinfo.py`](../../tools/check-wake-depinfo.py) (969 dòng, 20 ca `--selftest`)
**Log**: [`kt214-logs/`](kt214-logs/) — 11 đường × 4 log + bảng JSON + harness

---

## 0. KẾT LUẬN NGẮN

**ĐẠT CÓ ĐIỀU KIỆN.** Giả thuyết của lead (D2-171 §5) **đúng**: hỏi rustc danh sách tệp nó thật
sự biên dịch rồi quét `wake` trên đúng danh sách đó **bắt được cả 11/11** đường lọt đã biết —
kể cả **6** đường mà checker tĩnh hiện tại (`dc6e01ae`) vẫn **XANH**.

| Hạng mục | Kết quả |
|---|---|
| Nguyên mẫu bắt được | **11/11** đường lọt (mỗi đường: `cargo check` RC=0, tức repro **biên dịch được**) |
| Checker cũ (`--strict`) bắt được | **5/11** ở gốc `dc6e01ae` — lọt **6**: `include!` vị trí biểu thức trong `( … )`/`[ … ]` (5 biến thể) + `#[path]` trong inline module của tệp non-mod-rs. **Trên HEAD `83275d94`: 11/11** (KT-227 đính chính) |
| Chi phí (cache ấm) | **6,3 s** — so với checker cũ **21,3 s** (thường) / **26,3 s** (`--strict`) |
| Chi phí (kernel lạnh, giống `v2 check` sau clippy) | **10,9 s** |
| Chi phí (lạnh hoàn toàn, `--fresh`) | **14,7 s** |
| Phụ thuộc toolchain | **Không**. Stable **1.99.1** (ghim sẵn), **không** `-Z`, **không** nightly. `--emit=dep-info` là **mặc định** của cargo |
| Khuyến nghị | **Bổ sung lớp 2**, **không** thay thế checker tĩnh |

**Giá trị cốt lõi** không phải "bắt thêm 6 đường" — mà là **lớp lỗi biến mất**: nguyên mẫu không
suy luận cú pháp nào cả, nên không còn "bỏ sót vị trí token" hay "sai gốc phân giải". Mỗi lần
rustc đổi cách phân giải, nguyên mẫu **tự đúng** mà không cần vá.

---

## 1. Ý TƯỞNG VÀ CƠ CHẾ

### 1.1 Vì sao mười một bản vá trước đều thất bại

Mọi bản vá từ KT-171 đến KT-213 đều cùng một dạng: **tự suy luận cú pháp** (lexer → token-tree →
walker) rồi **đoán** xem rustc sẽ nạp tệp nào. Mỗi lần lại bỏ sót **một** vị trí hoặc **một** gốc:

| Vòng | Đường lọt | Chỗ suy luận sai |
|---|---|---|
| L40 (KT-201) | `macro_rules!` passthrough `:item` + `include!` trong **đối số lời gọi** | `_walk_items` bỏ nhóm sau `!` |
| L44 (KT-208) | `include!` ở **vị trí biểu thức** trong `( … )`/`[ … ]` | `_walk_items` chỉ đệ quy `{ … }` |
| L47 (KT-212) | `#[path]` trong **inline module** của tệp **non-mod-rs** | `resolve_include_targets` dùng `p.parent` thay vì `mod_dir` |

Ba lỗi này **không** có điểm chung về cú pháp — nhưng có **cùng một gốc**: chương trình tự viết
lại một phần việc của rustc. Vì vậy bản vá thứ mười hai (KT-211/KT-213) vẫn sẽ bỏ sót bản vá thứ
mười ba.

### 1.2 Cơ chế của nguyên mẫu

```
cargo check --workspace --message-format=json
        │
        ├─ rustc tự phân giải: mod, #[path], include!, macro_rules!, cfg_if!, cfg(...)
        │
        └─ cargo luôn truyền --emit=dep-info ⇒ <target>/…/deps/<crate>-<hash>.d
                │
                └─ "<target>: <tệp1> <tệp2> …"  ← DANH SÁCH TỆP THẬT rustc đã nạp
                        │
                        └─ áp luật `wake` (import từ check-wake-paths.py) trên đúng danh sách đó
```

**Đã đo bằng thực nghiệm** (không suy đoán): đích của `include!` **xuất hiện** trong `.d` với
đúng đường dẫn tương đối mà rustc đã dùng:

```
$ cat v2/kernel/target/debug/deps/vifast_core-*.d | tr ' ' '\n' | grep kt214
vifast-core/src/../../../lib/kt214_evil_core.rs      ← đích include! (đường dẫn nguyên văn)
vifast-core/src/kt214_p11.rs
```

Và **quan trọng**: rustc **vẫn ghi `.d` khi biên dịch lỗi** (đã đo với `E0308` lỗi kiểu và
`E0583` thiếu tệp module) ⇒ danh sách tệp không phụ thuộc việc build xanh.

### 1.3 Hai luật được áp (nguyên văn từ checker tĩnh, **không chép**)

| Luật | Mã | Nguồn định nghĩa | Allowlist |
|---|---|---|---|
| R1 — token `wake` | `E_DEPINFO_WAKE` | `CWP._check_wake_text` (import) | tệp định nghĩa `WAKE_PRODUCER` (`fn wake`/`wake_from`/`wake_from_unlink`) + tệp test-only (đồ thị module) |
| R2 — chuyển `Ready` | `E_DEPINFO_TRANSITION` | `CWP.RE_READY_TRANSITION` + `CWP.enclosing` + `CWP.test_line_ranges` | `CWP.WAKE_DOORS` + `CWP.NON_WAKE_READY_SITES` |

**Khác biệt có chủ đích** so với checker tĩnh: R1 áp cho **mọi** tệp rustc nạp, **không** lọc theo
`in_test_only_wake_scope`. Vì chính cái "khớp mẫu đường dẫn" là thứ đã sai ở L47
(`v2/kernel/vifast-core/evil.rs` nằm ngoài `src/` nên **không** được quét).

---

## 2. CÔNG CỤ

`v2/tools/check-wake-depinfo.py` — tệp **MỚI**, độc lập; **không** sửa `check-wake-paths.py`.

```
python3 v2/tools/check-wake-depinfo.py [--root R] [--json] [--selftest]
        [--no-build] [--fresh] [--unit all|host|kernel] [--timeout S] [--verbose] [--no-transition]
```

Hai đơn vị build (hai workspace độc lập — D2-26):

| Đơn vị | Manifest | Target | Vì sao |
|---|---|---|---|
| `host` | `v2/Cargo.toml` | host | lib + sim + tools |
| `kernel` | `v2/kernel/Cargo.toml` | `riscv64gc-unknown-none-elf` | **bắt buộc**: `cargo check` host-target cho kernel workspace **ĐỎ 30 lỗi** "invalid register `t3`/`t4`/`t6`" (`vifast-plat-riscv64` có asm chỉ hợp lệ trên triple đó) |

Đầu ra trên **cây sạch** (`dc6e01ae`):

```
# đơn vị host: cargo check … → RC=0 (0.09 s), 20 artifact, 20 tệp .d, 74 tệp .rs
# đơn vị kernel: cargo check … --target riscv64gc-unknown-none-elf → RC=0 (0.08 s), 18 artifact, 17 tệp .d, 112 tệp .rs
# rustc nạp: 145 tệp .rs trong cây (gộp 2 đơn vị)
# quét: 143 tệp (miễn 2 tệp cửa wake + 0 tệp test-only)
# test-only: không cần (không có tệp tên test trong danh sách nạp)
# tệp CHỈ tới được qua #[path]/include!/macro (ngoài mọi <crate>/src): 0
# tệp ngoài phạm vi luật wake tĩnh (in_test_only_wake_scope): 59
# check-wake-depinfo: errors=0 warnings=0 (5.62 s)
```

**"Tệp CHỈ tới được qua `#[path]`/`include!`/macro"** là chỉ số cốt lõi: trên cây sạch = **0**;
trên **mỗi** đường lọt = **1** (đúng tệp chứa `wake`). Đó là thứ checker tĩnh không cho biết.

### 2.1 `--selftest` — 20/20

Không cần cargo (0,21 s). Kiểm tầng đọc `.d` + cả hai luật trên cây dựng tay:

| # | Ca | Kỳ vọng |
|---|---|---|
| 1–4 | parse `.d`: nối dòng `\`, gom nhiều dòng đích, resolve tương đối/tuyệt đối, escape `\ ` | đúng |
| 5–6 | cây sạch ⇒ XANH; tệp định nghĩa `fn wake` được miễn | XANH |
| 7–8 | tệp ngoài `<crate>/src` (đích `include!`) chứa `wake` ⇒ ĐỎ + nằm trong `hidden_files` | ĐỎ |
| 9–10 | tệp dưới `v2/kernel` nhưng **ngoài** `src/` (lớp L47) ⇒ ĐỎ; và **không** khớp `in_test_only_wake_scope` | ĐỎ |
| 11 | tệp test-only (đồ thị module) chứa `wake` ⇒ XANH | XANH |
| 12–13 | `wake` trong chú thích/chuỗi ⇒ XANH; biến cục bộ tên `wake` ⇒ XANH | XANH |
| 14–16 | R2: ngoài cửa ⇒ ĐỎ; trong `WAKE_DOORS` ⇒ XANH; trong `NON_WAKE_READY_SITES` ⇒ XANH | đúng |
| 17 | chỉ lấy `.d` của unit có artifact trong lần chạy (bỏ `.d` cũ) | đúng |
| 18–19 | `hidden_files` không báo oan tệp chuẩn; `outside_static_scope` đúng mẫu | đúng |
| 20 | kết quả tuần tự hoá được sang JSON | đúng |

---

## 3. BẢNG MƯỜI MỘT ĐƯỜNG LỌT

Mỗi đường dựng **một lần** trên worktree, `cargo check` **RC=0** (repro **biên dịch được** — không
phải mã chết), đo **cả hai** checker, rồi **hoàn nguyên**. Log từng bước:
[`kt214-logs/pNN-*.log`](kt214-logs/); bảng máy đọc: [`bang-11-duong-lot.json`](kt214-logs/bang-11-duong-lot.json).

| # | Đường lọt | Nguồn | cargo | checker cũ (thường) | checker cũ `--strict` | **nguyên mẫu** | tệp ẩn nguyên mẫu chỉ ra |
|---|---|---|---|---|---|---|---|
| 1 | `macro_rules!` passthrough `:item` + `include!` trong **đối số lời gọi** | L40 | RC=0 | **ĐỎ** (2) | **ĐỎ** (2) | **ĐỎ** (1) | `v2/lib/kt214_evil_core.rs` |
| 2 | `include!` vị trí biểu thức trong `( … )` | L44 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 3 | `include!` vị trí biểu thức trong `[ … ]` | L44 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 4 | `(0u8, include!{…})` | L44 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 5 | `(|| include!{…})` | L44 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 6 | `Some(include!{…})` | L44 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 7 | `#[path]` trong **inline module** của tệp **non-mod-rs** | L47 | RC=0 | XANH | **XANH** | **ĐỎ** (1) | `v2/kernel/vifast-core/kt214_p07_evil.rs` |
| 8 | `#[path]` trỏ ra ngoài `src/` qua `..` | L47 | RC=0 | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) | `v2/kernel/lib/kt214_p08_evil.rs` |
| 9 | `include!` trong `v2/lib` trỏ sang `v2/kernel` | L40/L45 | RC=0 | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) | `v2/kernel/vifast-core/kt214_p09_evil.rs` |
| 10 | `macro_rules!` `:expr` + `include!` | L44 | RC=0 | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) | `v2/lib/kt214_evil_expr.rs` |
| 11 | `mod` khai trong lời gọi macro (`cfg_if!`) | L40 | RC=0 | **ĐỎ** (4) | **ĐỎ** (4) | **ĐỎ** (1) | `v2/lib/kt214_evil_core.rs` |

**Tổng (ở gốc `dc6e01ae`)**: checker cũ **5/11** (đường 1, 8, 9, 10, 11) · nguyên mẫu **11/11**.
**Trên HEAD `83275d94` (KT-227 đo lại)**: checker cũ **11/11** — p02–p06 nay ĐỎ
`E_STRICT_WAKE_INCLUDE`, p07 nay ĐỎ `E_STRICT_TEST_ONLY_WAKE` (log
`kt224-logs/p02-p07-tai-lap-head.log`).

### 3.1 Đọc bảng cho đúng — ba điều quan trọng

**(a) Sáu đường lọt của checker cũ đúng là sáu đường đã biết.** Đường 2–6 = lớp L44 (`include!`
trong `( … )`/`[ … ]`, KT-211 chưa làm); đường 7 = lớp L47 (gốc phân giải `#[path]`, KT-213 chưa
làm). Không có đường lọt **mới** nào do nguyên mẫu phát hiện — điều này **xác nhận** danh sách
đường lọt của lead là đầy đủ, không phải nguyên mẫu tìm thêm.

**(b) Đường 8 và 9: checker cũ bắt được, nhưng bắt "nhầm lý do".** Cả hai bị bắt bởi luật
**fail-closed** "đích trỏ ra ngoài cây, không quét được" (`E_STRICT_WAKE_INCLUDE`), **không** phải
bởi phân giải đúng:

| # | Checker cũ phân giải tới | rustc nạp thật | Vì sao checker cũ vẫn ĐỎ |
|---|---|---|---|
| 8 | `v2/lib/kt214_p08_evil.rs` (decoy) | `v2/kernel/lib/kt214_p08_evil.rs` (**evil**) | đích checker giải ra **ngoài** `v2/kernel`+`v2/sim` ⇒ fail-closed |
| 9 | — (không phân giải) | `v2/kernel/vifast-core/kt214_p09_evil.rs` (**evil**) | KT-209/L3: `include!` của `v2/lib` trỏ ra ngoài `v2/lib` ⇒ fail-closed |

⇒ Nếu đích **rơi vào trong** cây (đường 7), luật fail-closed **không** cứu được — đó đúng là lỗ
L47. Nói cách khác: checker cũ đóng được lớp này **một phần nhờ may**, không nhờ phân giải đúng.

**(c) Đường 1, 10, 11: checker cũ bắt được nhờ luật **macro**, không nhờ phân giải `include!`.**
Cả ba đều ĐỎ vì `E_STRICT_MACRO_TT`/`E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD` — tức luật
"thân/lời gọi macro chứa mẫu nguy hiểm ⇒ ĐỎ", **không** phải vì checker biết rustc nạp tệp nào.
Nguyên mẫu bắt bằng **đúng tệp chứa `wake`** (`v2/lib/kt214_evil_core.rs`), nên nó **vẫn đúng**
nếu ai đó gỡ luật macro hoặc viết macro không khớp mẫu.

### 3.2 Đường 7 — tái lập **nguyên văn** repro của L47

Đây là đường quan trọng nhất vì nó là đường **sống** gần nhất (KT-213 chưa làm):

```
v2/kernel/vifast-core/src/kt214_p07.rs        (tệp non-mod-rs)
    mod inner {
        #[path = "../../../kt214_p07_evil.rs"]
        mod baz;
    }

rustc giải:  src/kt214_p07/inner/ + ../../../kt214_p07_evil.rs = vifast-core/kt214_p07_evil.rs  ← EVIL
checker giải: src/inner/        + ../../../kt214_p07_evil.rs = v2/kernel/kt214_p07_evil.rs      ← DECOY (sạch)
```

| Kiểm | Kết quả |
|---|---|
| `cargo check -p vifast-core` | **RC=0** (chỉ warning `dead_code`) |
| `check-wake-paths.py` | **RC=0**, `errors=0` — **XANH** |
| `check-wake-paths.py --strict` | **RC=0**, `errors=0` — **XANH** |
| `check-wake-depinfo.py --unit kernel` | **RC=1**, `E_DEPINFO_WAKE v2/kernel/vifast-core/kt214_p07_evil.rs:6` |

Log: [`p07-cargo.log`](kt214-logs/p07-cargo.log), [`p07-static-strict.log`](kt214-logs/p07-static-strict.log),
[`p07-depinfo.log`](kt214-logs/p07-depinfo.log).

---

## 4. CHI PHÍ

Đo trên máy này (Apple Silicon, `cargo` 1.99.1 ghim), mỗi cấu hình lặp 2–3 lần lấy **trung vị**:

| Cấu hình | Thời gian | Ghi chú |
|---|---|---|
| `check-wake-paths.py` (thường) | **21,3 s** | quét 224 tệp + 5 luật cấu trúc |
| `check-wake-paths.py --strict` | **26,3 s** | thêm luật allowlist |
| `check-wake-paths.py --selftest` | 6,0 s | |
| **`check-wake-depinfo.py` (cache ấm)** | **6,3 s** | build 0,17 s + quét 6,1 s |
| `check-wake-depinfo.py --no-build` | 6,0 s | chỉ quét, dùng `.d` đã có |
| `check-wake-depinfo.py` (kernel lạnh, host ấm — **giống `v2 check` sau clippy**) | **10,9 s** | kernel build 4,65 s |
| `check-wake-depinfo.py --fresh` (lạnh hoàn toàn) | **14,7 s** | host 3,6 s + kernel 4,3 s + quét 6,1 s |
| `check-wake-depinfo.py --selftest` | **0,21 s** | không cần cargo |

### 4.1 Có cần build lại không? Cache có giúp không?

**Có, và cache giúp rất nhiều** — nhưng có ba mức:

| Tình huống | Build | Tổng |
|---|---|---|
| Vừa chạy `cargo check`/`clippy` cùng target dir | **0,17 s** | 6,3 s |
| `v2 check` vừa chạy `clippy --workspace --all-targets` (host ấm) nhưng kernel chưa build | **4,65 s** | 10,9 s |
| Máy mới / `target/` bị xoá | **7,9 s** | 14,7 s |

**Đo được điều bất ngờ có lợi**: sau `cargo clippy --workspace --all-targets`, `cargo check
--workspace` chỉ **0,37 s** — clippy và check **dùng chung** artifact `.rmeta` trong cùng target
dir. Nghĩa là nếu nối nguyên mẫu vào **sau bước clippy của `v2 check`**, phần build của đơn vị
`host` gần như **miễn phí**; chỉ đơn vị `kernel` (riscv64, `v2 check` không build) là phải trả
4,65 s.

### 4.2 Quét 6,1 s — đi đâu?

| Bước | Thời gian |
|---|---|
| đọc 145 tệp + `find_functions` | 0,16 s |
| `_shadowed_safe_macros` | 0,03 s |
| `_check_wake_text` × 145 | **5,75 s** |
| `test_only_modules` (đồ thị module) | **0 s** — chỉ dựng khi danh sách nạp có tệp tên test; bản build sản phẩm không có |

Ghi chú: `test_only_modules` tốn **9,4 s** nếu luôn dựng. Nguyên mẫu chỉ dựng khi cần (mẫu tên chỉ
có thể làm ta **quét nhiều hơn**, không thể miễn thêm ⇒ fail-closed). Đây là lý do 6,3 s thay vì
15,7 s.

### 4.3 So sánh công bằng

Checker tĩnh **đắt hơn** nguyên mẫu (21–26 s so với 6–11 s) vì nó quét **224** tệp và chạy **5
luật cấu trúc** trên token-tree tự viết. Nguyên mẫu chỉ quét **145** tệp (đúng tập rustc nạp) và
chỉ hai luật. **Chi phí không phải lý do để không dùng nguyên mẫu** — lý do là nó **cần build**.

---

## 5. PHỤ THUỘC TOOLCHAIN

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| Có cần nightly? | **Không** | chạy trên `1.99.1-aarch64-apple-darwin` (ghim trong `v2/rust-toolchain.toml`) |
| Có dùng `-Z`? | **Không** | không có `-Zunpretty`, không `-Zbinary-dep-depinfo` |
| `--emit=dep-info` có phải bật tay? | **Không** | cargo **luôn** truyền `--emit=dep-info` cho mỗi unit; `.d` có sẵn sau `cargo check` thường |
| Có cần `cargo metadata`/`build-plan`? | **Không** | chỉ đọc `.d` + `--message-format=json` (ổn định từ lâu) |
| Có phụ thuộc `target/` layout? | **Có, nhẹ** | đường dẫn `<target>/[<triple>/]debug/deps/*.d`; đổi profile (`--release`) phải đổi `debug` → `release` (hiện **hard-code** `debug`) |

**Rủi ro toolchain duy nhất**: nếu rustc đổi định dạng `.d` (make dep-info) — nhưng định dạng này
là hợp đồng với `make`/`cargo` từ nhiều năm và **cargo phụ thuộc nó** để rebuild, nên rủi ro thấp.

---

## 6. RỦI RO VÀ GIỚI HẠN

| # | Rủi ro | Mức | Ghi chú |
|---|---|---|---|
| 1 | **Cần build** — không chạy được trên cây chưa build | **Cao** | 0,17 s (ấm) → 7,9 s (lạnh); `v2 check` đã có clippy nên host gần như miễn phí, kernel 4,65 s |
| 2 | **Build ĐỎ ⇒ danh sách tệp không đầy đủ** | Trung bình | Đã xử lý: khi RC≠0, lấy **mọi** `.d` trong thư mục (superset, fail-closed) + báo `E_DEPINFO_BUILD`. Đo được: rustc **vẫn ghi `.d`** khi lỗi kiểu/thiếu tệp |
| 3 | **Khó báo dòng/cột** | Thấp | Thực ra **không** khó: `.d` cho **tệp**, còn dòng/cột do `_check_wake_text` tính trên nội dung tệp — đầu ra có `tệp:dòng` đầy đủ (xem bảng §3) |
| 4 | **Khó phân biệt test-only** | Trung bình | Bản build sản phẩm **không** nạp tệp test ⇒ không cần miễn. Nếu có nạp (feature `ktest`), nguyên mẫu dựng đồ thị module (9,4 s) và dùng **kết quả của nó**, không dùng mẫu tên |
| 5 | **Không thấy mã do macro sinh ra** | **Có** | `include!`/`#[path]` thì thấy (rustc nạp tệp thật), nhưng `macro_rules!` **sinh** ra `m.wake(t)` từ token **không** có trong tệp nào ⇒ `.d` không giúp. Đây là lý do **phải giữ** luật macro của checker tĩnh |
| 6 | **Không thấy `wake` trong mã chỉ có ở nhánh `cfg` không bật** | **Có** | Bản build chỉ nạp nhánh `cfg` đang bật. **ĐÍNH CHÍNH KT-227**: câu "checker tĩnh quét **mọi** nhánh" **sai** với `#[cfg(feature = "ktest")] mod` — lớp 1 miễn tệp đó vì `test_only_modules` coi module ktest là test-only ⇒ **cả hai lớp cùng mù** (đường lọt B3b của KT-224). Đã vá ở KT-227 P0-4 (fail-closed). Với `#[cfg(any())]`/`#[cfg(test)]` thì lớp 1 vẫn quét. |
| 7 | **Phụ thuộc profile/target dir** | Thấp | hard-code `debug`; đổi sang `--release` phải sửa |
| 8 | **Không thay được luật cấu trúc** | — | `E_STRICT_TRANSITION`/`E_STRICT_STALE`/`E_LEX_*`/`E_ARCH_*` vẫn cần checker tĩnh |

**Hai giới hạn #5 và #6 là quan trọng nhất**: chúng chứng minh nguyên mẫu **không thể thay thế**
checker tĩnh, chỉ **bổ sung**. Cụ thể:

- #5: `macro_rules!` sinh mã — checker tĩnh bắt bằng luật "thân macro chứa `wake`/`include!`/`mod`/
  `:tt`" (`E_STRICT_MACRO_*`). Nguyên mẫu **không** thấy gì vì không có tệp nào chứa token đó.
- #6: `#[cfg]` tắt — checker tĩnh quét mọi nhánh **trừ** `#[cfg(feature = "ktest")] mod` (đính chính KT-227: cả hai lớp cùng mù ở đó); nguyên mẫu chỉ thấy nhánh được biên dịch. Một
  `m.wake(t)` trong `#[cfg(feature = "ktest")]` sẽ **lọt** nguyên mẫu nếu build không bật feature.

---

## 7. KHUYẾN NGHỊ CHO A0

### 7.1 Trả lời câu hỏi của phiếu

| Câu hỏi | Trả lời |
|---|---|
| Nguyên mẫu có **thay được** `check-wake-paths.py`? | **KHÔNG.** Hai lý do đo được: (a) không thấy mã do `macro_rules!` sinh ra; (b) không thấy nhánh `#[cfg]` không bật. Ngoài ra nó không chạy được khi cây chưa build. |
| Nó **bổ sung** được gì? | **Đúng lớp lỗi đã phá 11 lần**: "checker tự suy luận cú pháp ⇒ bỏ sót vị trí/gốc phân giải". Ở gốc `dc6e01ae` nó bắt **6/11** đường mà checker tĩnh XANH; **trên HEAD `83275d94` checker tĩnh đã bắt 11/11** (KT-227 đính chính) ⇒ giá trị hiện tại là **cơ chế độc lập** + phủ `v2/lib`, không phải "lấp 6/11". |
| Có nên chạy trong `gate --fast` sau bước build? | **Có** — nhưng đặt ở `v2 check` **sau clippy** rẻ hơn: host gần như miễn phí (0,37 s), chỉ kernel 4,65 s. |

### 7.2 Đề xuất cụ thể

**Giữ checker tĩnh làm lớp 1** (nhanh, không cần build, phủ `cfg` + macro + luật cấu trúc) —
**thêm nguyên mẫu làm lớp 2** (độc lập hoàn toàn về cơ chế, phủ đúng tập tệp rustc nạp).

Ba phương án, xếp theo chi phí:

| Phương án | Chỗ đặt | Chi phí thêm | Khi nào |
|---|---|---|---|
| **A (khuyến nghị)** | `v2 check`, **ngay sau** bước `clippy` | **+4,8 s** (kernel 4,65 s + quét 6,1 s − phần đã có) | Ngay bây giờ được — `v2 check` đã 60 s+, thêm 5 s là chấp nhận được |
| **B** | `gate --fast`, sau `cargo test --workspace` (L0) | +6,3 s (cả hai unit đã ấm) | Nếu A0 muốn `v2 check` giữ < 60 s |
| **C** | Lệnh rời `v2 wake-depinfo`, chạy trong `nightly` | 0 | Nếu A0 chưa muốn thêm chi phí vào cổng |

**Nếu chọn A hoặc B**: xoá mục `NGOAI_LE` cho `v2/tools/check-wake-depinfo.py` trong
`v2/tools/check-selftest-wiring.py` (đã thêm trong task này, có ghi lý do) — khi ấy công cụ **có**
cổng gọi và mục ngoại lệ sẽ sinh `W_EXEMPT_UNUSED`.

### 7.3 Việc **không** nên làm

- **Không** gỡ luật macro của checker tĩnh (`E_STRICT_MACRO_*`) vì "đã có lớp 2" — lớp 2 **không**
  thấy mã do macro sinh (giới hạn #5).
- **Không** gỡ luật `#[cfg]`/test-only của checker tĩnh — lớp 2 chỉ thấy nhánh được biên dịch
  (giới hạn #6).
- **Không** dùng nguyên mẫu làm **cổng duy nhất** cho luật `wake`: nó phụ thuộc build xanh.

---

## 8. VIỆC ĐÃ LÀM / CHƯA LÀM

### Đã làm

- `v2/tools/check-wake-depinfo.py` — công cụ **mới**, 969 dòng, `--selftest` 20/20, `--json`,
  `--no-build`, `--fresh`, `--unit`, `--verbose`, `--no-transition`.
- Đo **11/11** đường lọt × **2** checker, mỗi đường `cargo check` RC=0, hoàn nguyên sau khi đo.
- Đo chi phí 7 cấu hình (trung vị 2–3 lần).
- `v2 check` xanh trên cây có công cụ mới (xem §9).
- Thêm mục `NGOAI_LE` **có lý do** cho công cụ trong `check-selftest-wiring.py` (nếu không,
  `v2 check` ĐỎ `E_ORPHAN` — công cụ chưa nối vào cổng nào).

### Chưa làm (và vì sao)

| Việc | Vì sao chưa |
|---|---|
| Nối vào `v2 check`/`gate --fast` | **Quyết định của A0** — phiếu này là nguyên mẫu, không được tự đổi cổng |
| `--release` profile | Chưa cần; hard-code `debug` có ghi chú |
| `-Zbinary-dep-depinfo` (nightly) để thấy cả `.rlib` | Không cần cho mục tiêu này, và **cấm** phụ thuộc nightly |
| Đo trên cây có `cfg(feature = "ktest")` bật | Ngoài phạm vi 2,5 h; giới hạn #6 đã ghi rõ |
| So sánh với `-Zunpretty=expanded` | Cần nightly ⇒ loại |

---

## 9. BẰNG CHỨNG CỔNG

```
$ ./v2/bin/v2 check
...
v2 check: PASS
```

Log: [`kt214-logs/v2check.log`](kt214-logs/v2check.log).

**Không** chạy `gate --fast` (bị cấm trong vòng này). **Không** sửa `check-wake-paths.py`.

---

## Phụ lục A — Tệp đã chạm

| Tệp | Loại |
|---|---|
| `v2/tools/check-wake-depinfo.py` | **mới** (công cụ nguyên mẫu) |
| `v2/tools/check-selftest-wiring.py` | sửa **1 khối** `NGOAI_LE` (thêm mục có lý do) |
| `v2/agent/JOURNAL/kt214-depinfo-prototype.md` | **mới** (tệp này) |
| `v2/agent/JOURNAL/kt214-logs/**` | **mới** (48 tệp log + harness + bảng JSON) |

**Không** chạm: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`,
`v2/tools/check-wake-paths.py`.

## Phụ lục B — Cách tái lập

```bash
cd /private/tmp/night3-L48-kt214
export PATH="$HOME/.cargo/bin:$PATH"

# 1) tự kiểm công cụ (0,21 s, không cần cargo)
python3 v2/tools/check-wake-depinfo.py --selftest

# 2) chạy trên cây sạch (6,3 s khi cache ấm)
python3 v2/tools/check-wake-depinfo.py

# 3) đo lại 11 đường lọt (≈13 phút: 11 × (cargo + 2×checker tĩnh + nguyên mẫu))
python3 v2/agent/JOURNAL/kt214-logs/harness-11-duong-lot.py
```
