# KT-256 — đóng ba nợ của luật `wake` (L2 + DEBT-KT227-1 + kiểm máy `--cfg test`)

**Ngày**: 2026-11-21 · **Task**: KT-256 · **Nhánh**: `task/kt253-wakedebt` · **Worktree**:
`/tmp/kt253-s3` · **Base**: `main` @ `60f082b9` · **Điều kiện đóng**: D2-190 (7 mục).
**Tiền đề**: `JOURNAL/kt252-wake-attack.md` §6 (nợ còn lại), `BYPASS-REGISTRY.md` (WB-01..WB-20),
`reports/CHECKER-WAKE-CONTRACT.md`, D2-188 (phạm vi từ `cargo metadata`), D2-190 (tiêu chí đóng).

**Ràng buộc đã giữ**: **không** push/merge, **không** chạm `main`, **không** sửa
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `spec/`, **không** sửa `v2/bin/v2`, **không**
nới ngưỡng, **không** tắt ca, **không** allowlist rộng. Mọi thay đổi có **test đỏ trước** (D2-05):
ca selftest mới được viết **trước** khi vá và **đỏ** trên bản cũ (bằng chứng §2.2).

---

## 1. Nợ **L2** — ba mẫu phạm vi ⇒ **một** định nghĩa sinh từ `cargo metadata`

### 1.1 Trước KT-256: ba định nghĩa vênh nhau

| Nơi | Mẫu | Dùng cho | Bỏ sót |
|---|---|---|---|
| `check-wake-paths.py` | `RE_TEST_ONLY_WAKE_SCOPE` = `^v2/(kernel|sim)/<crate>/(src/**.rs|build.rs)$` ∪ crate root thật | luật token `wake` | tệp sản phẩm **ngoài `src/`** (`init/*.rs`, `vifast-plat-user/lib.rs`); crate ở gốc **mới** |
| `check-wake-paths.py` | `MACRO_SCAN_ROOTS` = `v2/lib`, `v2/kernel`, `v2/sim` | thân `macro_rules!` | crate ở gốc **mới** (`v2/drivers/…`); `v2/tools` **được nâng** ở lớp dep-info nhưng lớp tĩnh vẫn mù |
| `check-wake-depinfo.py` | `RE_CRATE_SRC` = `^v2/(lib|kernel|sim|tools)/<crate>/src/` | dự phòng `hidden_files` | crate ở gốc **mới**; tệp nguồn **ngoài `src/`** |

### 1.2 Sau KT-256: **một** nguồn — `v2/tools/wake_scope.py`

`protected_scope(root)` (cache theo gốc cây) sinh `ProtectedScope` từ
`cargo metadata --no-deps --format-version 1` của **hai** workspace (`v2/Cargo.toml`,
`v2/kernel/Cargo.toml`) + **bao đóng path-dependency nội bộ** (D2-188), phân loại **bảo vệ** /
**từ chối rõ** (`v2/tools/`, **nâng** lên bảo vệ nếu bao đóng chứa crate bảo vệ). Ba **view** suy
từ cùng một đối tượng:

| View | Nội dung | Call site |
|---|---|---|
| `contains(relp)` | crate bảo vệ ∪ `NON_CRATE_SOURCE_TREES` | `hidden_files` (lớp dep-info) |
| `token_scope(relp)` | `contains` **và** (workspace `kernel` **hoặc** crate `vifast-sim` **hoặc** cây sản phẩm không-phải-crate) | `in_test_only_wake_scope` (luật token) |
| `macro_roots()` | `roots()` = thư mục crate bảo vệ ∪ cây nguồn không-phải-crate | `scan_macro_bodies` |

**Ngoại lệ có lý do** (không phải mẫu): `v2/kernel/vifast-plat-user` — shim ABI U-mode, **không
phải crate**, biên dịch bởi `vifast-kbin-riscv64/build.rs`, đóng vào ảnh sản phẩm.

**Ba mã lỗi mới** (fail-closed, đều có ca selftest chứng minh **có răng**):

| Mã | Khi nào | Vì sao |
|---|---|---|
| `E_SCOPE_METADATA` | cây **có** workspace manifest nhưng không dựng được phạm vi (không `cargo`, RC≠0, 0 crate) | quét-rồi-PASS với phạm vi rỗng là fail-open |
| `E_SCOPE_REDUCTION` | tệp `.rs` dưới ba gốc cũ **ngoài** phạm vi metadata | lưới an toàn **di trú**: định nghĩa mới không được hẹp hơn ba mẫu cũ |
| `E_SCOPE_LIB_CORE` | crate `v2/lib` có `vifast-core` trong **bao đóng normal dependency** | **DEBT-KT227-1** — xem §3 |

### 1.3 Chứng minh **không đổi hành vi** (cây thật)

| Phép đo | Trước KT-256 | Sau KT-256 |
|---|---|---|
| `check-wake-paths.py` (thường) | `errors=0` | **`errors=0`** (36,6 s) |
| `check-wake-paths.py --strict` | `errors=0` | **`errors=0`** (36,6 s) |
| `check-wake-depinfo.py` (cây thật) | `errors=0` | **`errors=0`** (42,6 s) |
| `check-wake-paths.py --selftest` | 411/411 | **420/420** |
| `check-wake-depinfo.py --selftest` | 65/65 | **67/67** |
| `./v2/bin/v2 check` | PASS | **PASS** (xem §6) |

Phạm vi cây thật (đo bằng `wake_scope.protected_scope`): **24 crate bảo vệ, 1 từ chối rõ**
(`vifast-agentctl`), `check_no_reduction` = **rỗng**, `check_lib_core_dependency` = **rỗng**.

### 1.4 Ca **có răng** — định nghĩa mới **bắt** thứ ba mẫu cũ bỏ sót

Đột biến **hai chiều**: cùng một cây fixture, chạy checker **bản cũ** (`git show HEAD:…`) và bản
mới. Log: `/tmp/kt256-teeth.log` (lệnh tái lập ở §7).

| Ca | Mẫu cũ | Bản cũ | Bản mới |
|---|---|---|---|
| **T1** `v2/kernel/vifast-kbin-riscv64/init/evil.rs` chứa `m.wake(t)` (chương trình U-mode, **sản phẩm**, ngoài `src/`) | `RE_TEST_ONLY_WAKE_SCOPE` **không khớp** | **XANH** (`errors=0`) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| **T2** `v2/kernel/vifast-plat-user/lib.rs` chứa `m.wake(t)` (shim ABI, **không phải crate**) | mẫu cũ **không khớp** | **XANH** | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| **T3** `v2/drivers/vifast-x/src/lib.rs` — crate ở **gốc mới**, thân `macro_rules!` chứa `wake` | `MACRO_SCAN_ROOTS` **mù** | **XANH** | **ĐỎ** `E_STRICT_MACRO_WAKE` |
| **T4** `v2/tools/tool/src/lib.rs` — crate công cụ **phụ thuộc** `vifast-core`, thân macro chứa `wake` | lớp tĩnh **không nâng** | **XANH** | **ĐỎ** `E_STRICT_MACRO_WAKE` (crate `tool` = `protected`, lý do "nâng") |

Bốn ca T1–T4 nằm trong `--selftest` của lớp tĩnh (mỗi ca khẳng định **đồng thời**: mẫu cũ không
khớp + checker ĐỎ đúng mã). Thêm **bốn** ca cho lưới an toàn: T5 `E_SCOPE_REDUCTION` (metadata
thiếu crate có thật), T6 `E_SCOPE_LIB_CORE` + **đối chứng dương** (chỉ dev-dep ⇒ XANH), T7
`E_SCOPE_METADATA`, T8 chính sách `v2/lib` ngoài luật token. Lớp dep-info thêm **hai** ca:
(40b) `contains` phủ crate ở gốc mới `v2/extra/…` (mẫu cũ `RE_CRATE_SRC` mù), (40c) `token_scope`
= ảnh sản phẩm + sim, **không** `v2/lib`.

**Không giảm phạm vi ở đâu**: `check_no_reduction` chạy trên **mọi** lượt quét cây thật (rỗng =
không giảm); ca T5 chứng minh nó **có răng** khi metadata bỏ sót crate.

---

## 2. **DEBT-KT227-1** — `v2/lib` ngoài luật token `wake`

### 2.1 Đo lại hôm nay (2026-11-21)

```bash
cargo metadata --no-deps --format-version 1 --manifest-path v2/Cargo.toml \
 | python3 -c "…bao đóng normal dependency của từng crate v2/lib…"
```

Kết quả (log đầy đủ: `/tmp/kt256-debt-measure.log`):

| crate `v2/lib` | normal path-deps | bao đóng normal | dev path-deps | có `vifast-core`? |
|---|---|---|---|---|
| `vifast-abi` | — | — | — | **không** |
| `vifast-alloc` | — | — | — | **không** |
| `vifast-elf` | — | — | — | **không** |
| `vifast-fs` | — | — | `vifast-elf` | **không** |
| `vifast-fsutil` | `vifast-fs` | `vifast-fs` | — | **không** |
| `vifast-idl` | — | — | — | **không** |
| `vifast-model` | `vifast-abi` | `vifast-abi` | `vifast-sim` | **không** |
| `vifast-net` | — | — | — | **không** |
| `vifast-objmem` | — | — | — | **không** |
| `vifast-pte` | — | — | — | **không** |
| `vifast-ring` | — | — | — | **không** |
| `vifast-scenario` | — | — | — | **không** |
| `vifast-sync` | — | — | — | **không** |
| `vifast-traceformat` | — | — | `vifast-sim` | **không** |
| `vifast-vfs` | `vifast-abi`, `vifast-fs` | `vifast-abi`, `vifast-fs` | — | **không** |
| `vifast-virtio` | — | — | — | **không** |

**Kết luận**: **không** crate `v2/lib` nào phụ thuộc `vifast-core` bằng **normal dependency**
(trực tiếp hay bắc cầu). Hai crate có **dev**-dependency `vifast-sim` (`vifast-model`,
`vifast-traceformat`), và `vifast-sim` có **dev**-dependency `vifast-core` — nhưng dev-dependency
**không** làm `wake()` gọi được từ **mã sản phẩm** `v2/lib`, và mã test đã được miễn theo
`is_test_file`. ⇒ Nợ vẫn **latent**, đúng như ghi ở KT-242.

### 2.2 Cách đóng: ca **canh** điều kiện (không mở rộng luật khi chưa cần)

`wake_scope.ProtectedScope.check_lib_core_dependency()` — chạy trên **mọi** lượt quét cây thật
(`scan_v2`), **ĐỎ** `E_SCOPE_LIB_CORE` khi một crate `v2/lib` có `vifast-core` trong bao đóng
**normal** dependency. Thông điệp nêu rõ hai lối: **mở rộng luật** (`token_scope`) hoặc **gỡ phụ
thuộc**.

**Răng đo được** (hai chiều, trong `--selftest`):

- T6: metadata chèn `vifast-model → vifast-core` (**normal**) ⇒ **ĐỎ** `E_SCOPE_LIB_CORE`;
- T6 đối chứng dương: cùng phụ thuộc nhưng `kind = "dev"` ⇒ **XANH** (thiếu vế này thì một luật
  "luôn đỏ" cũng qua được ca T6).

**Vì sao không mở rộng luật token cho `v2/lib` ngay**: (i) đo được **0** token `wake` trong
`v2/lib` nên mở rộng là **miễn phí về báo oan** nhưng (ii) luật token chỉ có nghĩa khi crate
**gọi được** `wake()`; mở rộng bây giờ là bảo vệ một bề mặt **không tồn tại**, và (iii) đề bài
KT-256 chốt: **không** phụ thuộc ⇒ đóng bằng ca canh. Khi ca canh **ĐỎ**, người thêm phụ thuộc
buộc phải mở rộng luật — đúng nghĩa "đóng latent".

---

## 3. Kiểm **máy** "build sản phẩm **không** có `--cfg test`"

### 3.1 Làm được — và đã làm

`v2/tools/check-no-cfg-test.py`:

1. **Build thật** ảnh sản phẩm: `cargo build --manifest-path v2/kernel/Cargo.toml -p
   vifast-kbin-riscv64 --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --release
   --features product-fs -v`, `CARGO_TARGET_DIR` = thư mục tạm **mới** (target dir cũ ⇒ cargo in
   `Fresh`, **không** có dòng lệnh rustc ⇒ không chứng minh được gì).
2. **Phân tích** từng dòng `Running \`…rustc …\`` bằng `shlex`: mọi predicate của `--cfg` phải
   **khác** `test`; `--test` (harness) ⇒ ĐỎ. **Không** nhầm `--check-cfg 'cfg(docsrs,test)'`
   (khác tên cờ) và `--cfg 'feature="ktest"'` (predicate là `feature`).
3. **Fail-closed**: log **không** có dòng rustc nào ⇒ ĐỎ; `--cfg test` xuất hiện **ngoài** dòng
   rustc (echo `RUSTFLAGS`, wrapper tự in) ⇒ ĐỎ.

**Kết quả đo trên cây thật** (log: `/tmp/cfgtest-real.log`):

```
# check-no-cfg-test: mode=build=vifast-kbin-riscv64/vifast-kbin-riscv64
  target=riscv64gc-unknown-none-elf features=product-fs rustc_lines=17 cfg_predicates=12 errors=0
real 0m41.745s   RC=0
```

12 predicate `--cfg` đo được: `feature="block"`, `feature="default"` (×3), `feature="product-fs"`
(×2), `feature="product-shell"` (×2), `feature="umode"`, `feature="user"` (×3) — **không** có
`test`. ⇒ Giả định "ảnh sản phẩm không bật `cfg(test)`" nay **đã kiểm máy**, không còn là giả định.

### 3.2 Ca **âm** — hai tầng

| Tầng | Cách | Kết quả |
|---|---|---|
| **Đơn vị** (10 ca, không cần cargo) | log tổng hợp: `--cfg test`, `--cfg=test`, `--cfg 'test'`, `--test`, `--cfg test` ngoài dòng rustc, log rỗng ⇒ **ĐỎ**; `--cfg 'feature="ktest"'`, `--check-cfg 'cfg(test)'`, `--test-threads=1` ⇒ **XANH** | **10/10 ok** |
| **End-to-end** (`--selftest-live`) | build **thật** với `RUSTFLAGS=--cfg test` ⇒ phép kiểm phải **ĐỎ** | **1/1 ok** (đã ĐỎ đúng) |

### 3.3 Giới hạn — nói rõ, không tuyên bố quá

- Phép kiểm chứng minh **dòng lệnh rustc của lần build đang kiểm** không có `--cfg test`. Nó
  **không** chứng minh mọi cấu hình/feature khác cũng vậy — muốn vậy phải chạy cho từng cấu hình
  (tham số `--features`).
- **Chưa nối vào `v2/bin/v2`** (task cấm sửa): hiện chạy **độc lập**. Đề xuất nối vào
  `gate --fast` (một dòng `run_step`), **không** nối `v2 check` (cổng nhanh, build ảnh ~40 s).
  Đã ghi ngoại lệ **có lý do** trong `check-selftest-wiring.py` (in ra minh bạch, không phải
  allowlist rộng).

---

## 4. Bảng **7 mục D2-190** — trạng thái sau KT-256

| # | Tiêu chí D2-190 | Trạng thái | Bằng chứng |
|---|---|---|---|
| 1 | **Hợp đồng bảo vệ rõ**: crate, target, feature, nguồn sinh, phần hỗ trợ và phần từ chối | **ĐỦ** (sau KT-256 mạnh hơn: phạm vi **một** nguồn sinh từ `cargo metadata`, ba view tường minh, ba mã lỗi phạm vi) | `CHECKER-WAKE-CONTRACT.md` §1; `wake_scope.py`; `E_SCOPE_*` |
| 2 | **#16–#18 bị chặn**, gồm lỗi đọc sai vị trí đối số khi gọi UFCS | **ĐỦ** (WB-16/17 đóng ở tầng API — `E0308`/`E0631`; WB-18 đóng ở tầng phạm vi — KT-238 + KT-256) | `BYPASS-REGISTRY.md`; `JOURNAL/kt241-attack/`; §1.4 |
| 3 | **Toàn bộ repro cũ giữ bền** + đối chứng hợp lệ | **ĐỦ** (411→420 ca lớp tĩnh, 65→67 ca lớp dep-info; **không** ca nào bị xoá; ca cũ đổi kỳ vọng chỉ khi luật **mạnh lên** — ghi ở §1.4) | `--selftest` hai lớp |
| 4 | **Ranh giới API được compiler cưỡng chế**, không chỉ checker tự xác nhận | **ĐỦ** (KT-241: `NonReadyTarget` không có `Ready`; `write_state` private; mỏ neo kiểu `E_ANCHOR_*`) | `JOURNAL/kt241-checker.md`; `_check_type_anchors` |
| 5 | **Reviewer khác tác giả** tấn công theo nhóm | **ĐỦ cho tới HEAD này** (KT-252: 19 biến thể/6 nhóm; KT-242: 6 đường lọt; KT-241: 28 ô) — **nhưng** chưa có vòng tấn công **mới** nào trên HEAD **sau KT-256** | `JOURNAL/kt252-wake-attack.md`; `kt242-wake-review.md` |
| 6 | **Gate trên commit tích hợp cuối PASS**, có log + danh tính công cụ | **CHƯA** — KT-256 chỉ chạy `v2 check` (T0/T1-ish) trên nhánh task; **T2 `gate --fast` trên commit tích hợp** do lead chạy sau merge | §6 |
| 7 | **Không còn đường lọt đã biết** nằm trong hợp đồng bảo vệ | **ĐỦ cho luật `wake`** (WB-01..WB-20 CLOSED; nợ L2 + DEBT-KT227-1 đóng ở task này; `--cfg test` nay kiểm máy). Hợp đồng §4 còn **hai** mục mở **có tên** nhưng **ngoài** luật `wake`: `:expr` passthrough (chỉ hở khi macro được gọi từ ngoài phạm vi sản phẩm) và miễn luật arch theo tiền tố `virtio_` | `BYPASS-REGISTRY.md`; `CHECKER-WAKE-CONTRACT.md` §4 |

**Kết luận**: **KHÔNG** tuyên bố đóng KT-184/KT-194. Còn **mục 6** (gate T2 trên commit tích hợp
— lead) và **mục 5** cần một vòng tấn công **mới** trên HEAD sau KT-256 (reviewer khác tác giả).
Mục 7 đạt **theo hợp đồng hữu hạn đã công bố**, đúng câu chữ D2-190 §7 — không phải "chứng minh
mọi chương trình Rust".

---

## 5. Đề xuất cho lead (không tự làm)

1. **Nối `check-no-cfg-test.py` vào `gate --fast`** — một dòng `run_step` trong `v2/bin/v2`
   (task cấm sửa). Nếu nối, nên dùng `--log` + lưu log làm bằng chứng, hoặc chạy build thật
   (~40 s) trong T2.
2. **Vòng tấn công mới trên HEAD sau KT-256** (D2-190 §5) — trọng tâm: phạm vi (crate mới ở gốc
   mới, `v2/tools` nâng, cây không-phải-crate), `--cfg test` qua `build.rs`/`.cargo/config.toml`,
   và `v2/lib` nếu ai đó thêm phụ thuộc `vifast-core`.
3. **T2 `gate --fast`** trên commit tích hợp (mục 6 của D2-190).

---

## 6. Số đo selftest trước/sau **từng lớp**

| Phép đo | Trước KT-256 | Sau KT-256 | Δ |
|---|---|---|---|
| `check-wake-paths.py --selftest` | 411/411 | **420/420** | **+9** (T1–T8 + ca chốt chữ ký cập nhật) |
| `check-wake-depinfo.py --selftest` | 65/65 | **67/67** | **+2** ((40b), (40c)) |
| `check-no-cfg-test.py --selftest` | — (mới) | **10/10** | +10 |
| `check-no-cfg-test.py --selftest-live` | — (mới) | **1/1** | +1 |
| `check-selftest-wiring.py --selftest` | 9/9 | **9/9** | 0 |
| `check-wake-paths.py` cây thật | `errors=0` | **`errors=0`** | 0 |
| `check-wake-paths.py --strict` cây thật | `errors=0` | **`errors=0`** | 0 |
| `check-wake-depinfo.py` cây thật | `errors=0` | **`errors=0`** | 0 |
| `./v2/bin/v2 check` | PASS | **PASS** | 0 |

**Không ca nào bị xoá.** Hai ca đổi **kỳ vọng** vì luật **mạnh lên** (không phải nới):
`check-wake-paths.py` ca chốt chữ ký `in_test_only_wake_scope` (thêm tham số `scope`, `root` vẫn
bắt buộc); `check-wake-depinfo.py` ca (10) — tệp ngoài `src/` của crate bảo vệ nay **thuộc** phạm
vi tĩnh (lỗ cũ đã đóng), trước đây ca này khẳng định **đúng cái lỗ** đó.

---

## 7. Tái lập

```bash
cd /tmp/kt253-s3
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + cây thật
python3 v2/tools/check-wake-paths.py --selftest          # 420/420
python3 v2/tools/check-wake-depinfo.py --selftest        # 67/67
python3 v2/tools/check-wake-paths.py                     # errors=0
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py --root .          # errors=0

# (b) phạm vi (một nguồn)
python3 - <<'EOF'
import sys; sys.path.insert(0, "v2/tools")
import wake_scope
from pathlib import Path
sc = wake_scope.protected_scope(Path("."))
print(len(sc.protected()), "bảo vệ;", len(sc.refused()), "từ chối rõ")
print("no_reduction:", sc.check_no_reduction(Path(".")))
print("lib_core:", sc.check_lib_core_dependency())
EOF

# (c) đột biến hai chiều T1–T4 (bản cũ từ git vs bản mới)
python3 /tmp/kt256-teeth.py

# (d) `--cfg test`
python3 v2/tools/check-no-cfg-test.py --selftest         # 10/10
python3 v2/tools/check-no-cfg-test.py                    # build thật, errors=0
python3 v2/tools/check-no-cfg-test.py --selftest-live    # ca âm end-to-end

# (e) cổng
./v2/bin/v2 check
```

---

## 8. Tệp thay đổi trong KT-256

| Tệp | Thay đổi |
|---|---|
| `v2/tools/wake_scope.py` | **mới** — **một** định nghĩa phạm vi sinh từ `cargo metadata`; ba view (`contains`/`token_scope`/`macro_roots`); `check_no_reduction`; `check_lib_core_dependency`; cache theo gốc cây |
| `v2/tools/check-wake-paths.py` | xoá `RE_TEST_ONLY_WAKE_SCOPE`/`_crate_root_relps`/`MACRO_SCAN_ROOTS`; `in_test_only_wake_scope` → `wake_scope`; `scan_macro_bodies` nhận `scope`; `scan_v2` phát `E_SCOPE_METADATA`/`E_SCOPE_REDUCTION`/`E_SCOPE_LIB_CORE`; **+9 ca** selftest |
| `v2/tools/check-wake-depinfo.py` | xoá `Scope`/`CrateInfo`/`scope_from_metadata`/`load_scope`/`RE_CRATE_SRC`/`RE_BUILD_RS` (alias sang `wake_scope`); `hidden_files` dùng `contains`; ca (10) cập nhật theo lỗ đã đóng; **+2 ca** |
| `v2/tools/check-no-cfg-test.py` | **mới** — kiểm máy `--cfg test` (build thật + log + ca âm đơn vị/end-to-end) |
| `v2/tools/check-selftest-wiring.py` | **một** ngoại lệ **có lý do** cho `check-no-cfg-test.py` (build ảnh ~40 s, không phù hợp `v2 check`; đề xuất nối `gate --fast`) |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §1 (một định nghĩa phạm vi + ba view + lưới không-giảm), §3 (ba mã `E_SCOPE_*`), §6 (cách chạy `check-no-cfg-test.py`) |
| `v2/agent/JOURNAL/kt256-wake-debt.md` | nhật ký này |

**Không** sửa: `v2/kernel/**`, `v2/sim/**`, `v2/lib/**`, `spec/`, `v2/bin/v2`,
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, `BYPASS-REGISTRY.md` (không có **đường lọt mới** nào được
phát hiện ⇒ không có mã `WB-*` mới; nợ L2/DEBT-KT227-1 là nợ **phạm vi**, không phải bypass).
