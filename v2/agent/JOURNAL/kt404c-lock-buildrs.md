# KT-404c — mở rộng `check-lockfile-sync.py` lên 3 lockfile + gom 3 `build.rs` về `vifast-kbuild`

Task: **KT-404c** (nợ của `KT-404`, xem `v2/agent/QUEUE.md` dòng `KT-404`).
Nhánh: `task/kt404c-lock-buildrs` · worktree `/private/tmp/vifast-wt/2` · base `main` @ `13c396ab`
(HEAD worktree `96120f27`) · `CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-2`.

Phạm vi: **hai** nợ, không chạm `spec/`, không nới ngưỡng, không sửa `expected.txt`/TAP/`accept()`.

1. `v2/tools/check-lockfile-sync.py` phủ **3** lockfile (`v2/Cargo.lock`, `v2/kernel/Cargo.lock`,
   `v2/user/Cargo.lock`) + ca `--selftest` cho lockfile thứ ba, **giữ nguyên** 10 ca cũ.
2. Gom máy móc còn lặp ở 3 `build.rs` của `vifast-kbin-{riscv64,aarch64,x86_64}` về
   `v2/kernel/vifast-kbuild/`, **đầu ra build phải GIỐNG HỆT** (`# build:` + sha ảnh + mọi artefact
   `$OUT_DIR`). Sha đổi ⇒ dừng, ghi rõ.

## 1. Lockfile — đỏ trước rồi mới sửa

Kế hoạch: thêm **ca kiểm** trước (fixture có lockfile thứ ba), chạy `--selftest` ⇒ phải **ĐỎ** vì
công cụ cũ chỉ đọc 2 lockfile; rồi mới mở rộng công cụ.

* Đỏ-trước (log `/private/tmp/kt404c/lockfile-red.txt`, nguyên văn trong báo cáo):
  `selftest 11/14 ok`, exit 1, ba ca ĐỎ: `not ok 11 - lockfile thứ ba (v2/user) lệch phiên bản ⇒ ĐỎ`,
  `not ok 12 - thiếu v2/user/Cargo.lock ⇒ ĐỎ (fail-closed)`,
  `not ok 14 - v2/user cùng phiên bản nhưng khác source ⇒ ĐỎ` — cả ba trả `rc=0` như kỳ vọng cũ
  (công cụ **mù** với `v2/user`).
* Sửa: `LOCKFILES` 3 mục; `run_check` so **cả ba cặp** (v2↔kernel, v2↔user, kernel↔user) — chỉ so
  với `v2/Cargo.lock` sẽ bỏ lọt cặp kernel↔user. Luật 4 (gói chỉ có một bên ⇒ xanh) giữ nguyên.
* Ca cũ: `_case` được thêm tham số `c_pkgs`/`write_c`, **mặc định** ghi lockfile thứ ba **đồng bộ
  với `v2/Cargo.lock`** ⇒ 10 ca cũ giữ nguyên ý nghĩa và kỳ vọng (vẫn so a↔b), không bị nhiễu.
* Nhãn bước trong `v2/bin/lib/check.sh` (bước 80) sửa "2 Cargo.lock" → "3 Cargo.lock"; **số bước
  không đổi** (`CHECK_STEPS_EXPECTED` không chạm).
* Xanh sau sửa (log `/private/tmp/kt404c/lockfile-green.txt`):
  `check-lockfile-sync: selftest 14/14 ok` (exit 0) và kiểm thật
  `v2/Cargo.lock=18 gói, v2/kernel/Cargo.lock=25 gói, v2/user/Cargo.lock=5 gói`;
  `OK — 3/3 cặp cùng phiên bản + cùng nguồn (B3), dùng chung v2↔kernel=15, v2↔user=2,
  kernel↔user=2` (exit 0). Hai crate dùng chung với `v2/user` là `vifast-ring`, `vifast-virtio`
  (`0.1.0`, `source=None`).

## 2. `build.rs` — baseline trước, refactor sau

### 2.1 Cách đo (quan trọng: danh tính phải **ghim**)

`# build:` và sha ảnh **không** so được một cách ngây thơ: `inputs` là dấu vân tay **nội dung**
`v2/kernel v2/lib v2/user` (`vbrev_inputs_calc` của `build-rev.sh`), nên sửa `build.rs` **làm đổi**
`inputs` ⇒ chuỗi nhúng trong ảnh đổi ⇒ sha đổi *có chủ đích*, không phải hồi quy. Vì vậy cả hai
lượt (trước/sau) build với **danh tính GHIM** qua env mà `identity::build_identity` ưu tiên:

```
VIFAST_BUILD_REV=13c396ab0000 VIFAST_BUILD_DIRTY=0 \
VIFAST_BUILD_INPUTS=0123456789abcdef VIFAST_BUILD_ALGO=kt404c-pin
```

Cùng thư mục nguồn + cùng `CARGO_TARGET_DIR` (nợ P20: sha phụ thuộc thư mục build) ⇒ hiệu số sha
chỉ còn do **hành vi build**. Bộ feature **tối đa** cho từng arch (phủ hết nhánh của build.rs):

| arch | lệnh | feature |
|---|---|---|
| riscv64 | `cargo build -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64` | `product-all,user` |
| aarch64 | `… -p vifast-kbin-aarch64` | `ktest` |
| x86_64 | `… -p vifast-kbin-x86_64` | `ktest` |

Kịch bản: `/private/tmp/kt404c-capture.sh <tag>`; manifest `/private/tmp/kt404c/<tag>/manifest.txt`
(sha ảnh + dòng `# build:` + sha **từng** artefact trong `OUT_DIR` thật = thư mục mới nhất có `*.elf`).

### 2.2 Baseline (tag `before`)

| arch | sha256 ảnh | `# build:` |
|---|---|---|
| riscv64 | `91995c112a7f19e40ef6b4bbdbd9c336cba96f7a4d2dd48276ced7b8ef9a40a8` (611 896 B) | `# build: algo=kt404c-pin inputs=0123456789abcdef dirty=0` |
| aarch64 | `38099617678df4f8f598e8a158c8740c7c206384859283a9a7b4b62d86331107` (502 848 B) | không nhúng dòng `# build:` (chỉ `env!("VIFAST_BUILD_INPUTS")`, xem §4) |
| x86_64 | `e812df6c307e0c00ed567f00489c9ead3c21b5f1a56b93f5273112e3519c7427` (347 432 B) | như aarch64 |

54 artefact `$OUT_DIR` (riscv64 38, aarch64 9, x86_64 7) làm manifest.

### 2.3 Refactor

Thêm vào `vifast-kbuild` (crate **host**, `#![forbid(unsafe_code)]`, 0 phụ thuộc ngoài):

* `src/cargo.rs` — `feature`, `target_is`, `link_kernel` (3 dòng `rustc-link-arg` + `linker.ld`),
  `rerun_if_changed`, `rerun_if_features`, `cfgs` (sinh `--check-cfg=…` + `--cfg …` theo bảng).
* `src/user.rs` — `Program`/`Build`/`Link` + `build()` (vòng dựng artefact U-mode: sinh `.ld` theo
  VA, gọi `$RUSTC`, `rerun-if-changed` từng nguồn) + `Rlib`/`rlibs()` (bảng rlib `--extern`).
  `build.rs` chỉ còn **dữ liệu**: `Program::new(...).at(VA).with(cfgs)`.
* Ba `build.rs` còn: `WATCH` (tệp phải theo dõi), `INIT_PROGRAMS`/`PROGRAMS` (bảng), `LINKER`,
  `LINK_ARGS`, và các nhánh feature của arch.

Nguyên tắc giữ **byte-identical**: `user::build` gọi `compile::compile_user_elf` với **nguyên giá
trị + nguyên thứ tự** tham số như ba vòng lặp cũ (cùng `cwd`, cùng đường dẫn nguồn **không**
`canonicalize` — A1-184/P20, cùng `--extern`, cùng `link-arg`); `compile.rs` không sửa một dòng.
`WATCH` bỏ 29 mục là **nguồn chương trình** vì `user::build` đã in `rerun-if-changed` cho đúng tệp
đó (không đổi đầu ra, chỉ bỏ dòng trùng).

### 2.4 So sánh trước/sau

`diff before/manifest.txt final/manifest.txt` ⇒ **rỗng**: 3/3 sha ảnh **giống hệt**, 54/54 artefact
`$OUT_DIR` **giống hệt**, dòng `# build:` của riscv64 **giống hệt** ⇒ refactor **không đổi một byte**
đầu ra nào.

## 3. Số dòng `build.rs`

| tệp | trước | sau | Δ |
|---|---:|---:|---:|
| `vifast-kbin-riscv64/build.rs` | 412 | 287 | −125 |
| `vifast-kbin-aarch64/build.rs` | 161 | 136 | −25 |
| `vifast-kbin-x86_64/build.rs` | 125 | 111 | −14 |
| **tổng** | **698** | **534** | **−164 (−23,5 %)** |

Phần lớn phần còn lại là **dữ liệu** (56 → 27 đường dẫn `WATCH`, 24 dòng `INIT_PROGRAMS`) và chú
thích nguồn gốc KT-xxx đã cố ý giữ. Mục tiêu `~450` **chưa đạt**; xem §5 (nợ).

## 4. Phát hiện (ngoài phạm vi sửa)

1. **`cfg!(feature = "product-net")` trong `vifast-kbin-riscv64/build.rs` là nhánh chết**: build
   script không nhận `--cfg feature=…` (feature đến bằng `CARGO_FEATURE_*`) nên `procsrv` **chưa
   bao giờ** nhận `--cfg product_net`. `KT-404c` **giữ nguyên** vì sửa sẽ **đổi byte** ELF
   `procsrv` (ngoài phạm vi "đầu ra giống hệt") ⇒ ghi nợ, cần task riêng có đo lại ảnh.
2. **`# build:` chỉ có trong ảnh riscv64**: `vifast-kbin-aarch64/src/main.rs:463` và
   `…-x86_64/src/main.rs:847` chỉ dùng `env!("VIFAST_BUILD_INPUTS")`, không dựng chuỗi
   `# build: algo=… inputs=… dirty=…` ⇒ `vbrev_image_identity` của hai arch đó trả `no-build-line`.
   Với hai arch này bằng chứng danh tính là **sự có mặt của `inputs` ghim** trong ảnh
   (`0123456789abcdef`: aarch64 2 lần, x86_64 1 lần — giống hệt trước/sau) + sha ảnh.

## 5. Việc còn lại

* `build.rs` 534 dòng (chưa đạt `~450`): phần còn lại là bảng dữ liệu của arch + chú thích KT-xxx;
  muốn xuống nữa phải **dời dữ liệu** (bảng `WATCH`/`INIT_PROGRAMS`) vào crate theo arch riêng,
  tức đổi kiến trúc chứ không phải gom lặp — đề xuất cân nhắc ở task sau.
* Nợ (1): nhánh `cfg!(feature = "product-net")` chết — cần task riêng (đổi byte `procsrv`).
* Nợ (2): 2 arch chưa in dòng `# build:`.

## 6. T0 và T1

### 6.1 T0 — `./v2/bin/v2 check`

`PASS`, exit 0, `TỔNG check 190 745 ms`. Bước liên quan trực tiếp:
`đồng bộ 3 Cargo.lock (B3, KT-407/KT-404c) 188 ms` + `selftest đồng bộ lockfile 233 ms` (xanh).

Trước khi sửa xong phần `run_check`, `v2 check` từng báo `selftest đồng bộ lockfile (FAIL)` — đúng
như chứng cứ đỏ-trước của §1 (công cụ còn mù `v2/user`), không phải hồi quy khác.

### 6.2 T1 — `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev`

**`v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)`**, `TỔNG gate --dev (T1) 532 057 ms`;
80 bước = **66 chạy / 14 bỏ (T1)** ⇒ **65 PASS / 1 FAIL**.

Bước ĐỎ duy nhất: `step 46 "selftest pre-measure (KT-95)"`, log nguyên văn
(`$TMPDIR/v2lanes.*/46.log`):

```
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
# pre-measure-selftest: FAIL
```

Nguyên nhân **tất định**, không phải hồi quy của KT-404c: ca (a) dựng ảnh giả `dirty=0` khớp dấu
vân tay **của cây hiện tại** rồi đòi `PRE-MEASURE: OK`; cây worktree đang có thay đổi **chưa
commit** trong `v2/kernel` (3 `build.rs`, `vifast-kbuild/src/*`) nên pre-measure từ chối đúng luật.
Tái lập trực tiếp (cùng công cụ, không sửa gì):

```
# cây nguồn (build-inputs): dirty=1
PRE-MEASURE: REFUSE reason=tree-dirty      EXIT=7
```

Đối chứng: chạy **cùng** `pre-measure.sh --selftest` trong repo chính `/Users/mps/dvchd/ViFastOs`
(19 tệp chưa track nhưng **không** nằm trong `v2/kernel v2/lib v2/user`) ⇒
`# pre-measure-selftest: PASS (a/b/c/d/e/f/g)`. Tức bước này chỉ xanh trên cây **sạch** —
Lead commit rồi chạy lại là xanh; agent bị cấm commit nên **không thể** làm xanh bước này trong
worktree. **Không** retry (AGENTS §4.5), **không** nới ngưỡng.

## 7. Trạng thái cây khi bàn giao

* **Không** `git add/commit/push/merge/stash` — Lead commit.
* Ảnh trong `CARGO_TARGET_DIR` (`target-task/slot-2`) **đã được dựng lại bằng danh tính TỰ NHIÊN**
  sau khi đo (không còn `algo=kt404c-pin`): riscv64 in `# build: algo=kt134-v5
  inputs=fe2eca65d9b93e6d dirty=1` (dirty=1 đúng vì cây còn thay đổi chưa commit); aarch64/x86_64
  không nhúng dòng `# build:` (§4.2). T2 của Lead dựng lại khi commit/cây sạch ⇒ `dirty=0`.
* Chứng cứ "mã đã đo = mã đang có": mtime của **mọi** tệp Rust đã sửa (07:22–07:26) **trước**
  mtime của manifest `final` (07:28:12); `cargo fmt` sau đó không đổi một byte (`git diff --stat`
  giữ nguyên 477/560).
* Một tệp nhật ký duy nhất: `v2/agent/JOURNAL/kt404c-lock-buildrs.md`.
