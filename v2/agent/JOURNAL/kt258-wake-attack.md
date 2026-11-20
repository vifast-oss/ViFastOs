# KT-258 — vòng **tấn công mới** cho luật `wake`, trọng tâm **phạm vi** (sau KT-256)

**Ngày**: 2026-11-21 · **Task**: KT-258 · **Nhánh**: `task/kt258-wake2` · **Worktree**:
`/tmp/kt258-s1` · **Base**: `main` @ `4478662a` · **Điều kiện đóng**: D2-190 (7 mục).
**Tiền đề**: `JOURNAL/kt256-wake-debt.md` (phạm vi **một** nguồn sinh từ `cargo metadata`),
`JOURNAL/kt252-wake-attack.md` (khuôn harness), `BYPASS-REGISTRY.md` (WB-01..WB-20),
`reports/CHECKER-WAKE-CONTRACT.md`, D2-188 (phạm vi từ metadata), D2-190 (tiêu chí đóng),
D2-202 (nhiệm vụ vòng này).

**Ràng buộc đã giữ**: **không** push/merge, **không** chạm `main`, **không** sửa
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `spec/`, **không** sửa `v2/bin/v2`, **không**
nới ngưỡng, **không** tắt ca, **không** allowlist rộng. Mọi biến thể chèn vào cây đều **khôi
phục** bằng `trap` (`v2/kernel`, `v2/sim`, `v2/lib`, `v2/Cargo.toml`, `v2/kernel/Cargo.toml`,
crate ở gốc mới, nguồn ngoài cây). Mọi thay đổi có **test đỏ trước** (D2-05): ca selftest mới
**đỏ** trên bản checker cũ (bằng chứng §3.4).

---

## 1. Cách tấn công: **theo lớp**, không replay danh sách

KT-256 vừa **đổi phạm vi** từ ba mẫu regex sang `wake_scope.py` sinh từ `cargo metadata`. Đổi
phạm vi là **bề mặt mới** ⇒ vòng này **không** chọn mẫu từ WB-01..WB-20; lập **ma trận 8 nhóm**
theo đề bài, mỗi nhóm ≥ 3 biến thể. Mỗi biến thể phải **`cargo check --target
riscv64gc-unknown-none-elf` RC=0** mới tính là đường lọt, và phải chạy **cả ba** lượt kiểm
(`check-wake-paths.py`, `--strict`, `check-wake-depinfo.py`).

Harness: `v2/agent/JOURNAL/kt258-attack/run-attack.sh <nhóm> <tên> <ca.sh>` — chèn, đo, khôi
phục bằng `trap`, ghi một dòng TSV. Bảng đầy đủ: `v2/agent/JOURNAL/kt258-attack.tsv`; log từng
ca ở `kt258-attack/logs/<tên>/` (`cargo.log`, `cargo-rc.txt`, `cargo-extra.log` (cấu hình
host/`product-fs`), `lop1-thuong.log`, `lop1-strict.log`, `lop2.log`, `git-truoc.txt`,
`git-sau.txt`).

**Điểm khác KT-252**: mỗi ca có thêm `EXTRA_CARGO` — lệnh cargo ở **cấu hình khác** (host
workspace hoặc `--features product-fs`) để chứng minh mã **thật sự được biên dịch** ở đó, không
chỉ "cargo RC=0 vì crate không được build". Đây là chỗ KT-252 chưa đo và là chỗ ba đường lọt
nằm.

---

## 2. Kết quả ma trận — **27 biến thể, 4 đường lọt** (3 gốc)

| Nhóm | Biến thể | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| **P1** crate mới ở gốc | p1a-host-member — `v2/drivers/evil` thành viên workspace **host** | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p1b-host-member-sim-dep — `v2/net/evil` thành viên host + dep của `vifast-sim` | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | **p1c-feature-gated-closure** — `v2/drivers/evil` **path-dep tuỳ chọn** của `vifast-kbin-riscv64`, chỉ bật bởi `product-fs` | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT (WB-21)** |
| | **p1d-feature-gated-include** — như p1c nhưng `wake` trong tệp `include!` **trong** crate | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT (WB-22)** |
| | **p1e-feature-gated-include-outside** — như p1d nhưng tệp `include!` **ngoài** crate | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT (WB-22)** |
| **P2** crate ngoài ba gốc | p2a-kernel-closure — `v2/foo/evil` path-dep **thường** của `vifast-plat-common` | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p2b-host-closure — `v2/foo/evil` path-dep **thường** của `vifast-sim` | 0 | 0 | 0 | 2 | bị bắt (lớp dep-info) |
| | p2c-build-dep — `v2/foo/evil` **build-dependency** của `vifast-plat-common` (build script rỗng ⇒ cargo vẫn biên dịch) | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| **P3** `v2/tools` được nâng | p3a-tools-core-dep — `v2/tools/evil` phụ thuộc `vifast-core` | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p3b-tools-macro-wake — `v2/tools/evil` (được nâng) có `wake` trong thân `macro_rules!` | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| | p3c-tools-sim-dep — `v2/tools/evil` phụ thuộc `vifast-sim` (nâng bắc cầu) | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| **P4** nguồn ngoài `src/` | p4a-init-file — `vifast-kbin-riscv64/init/evil.rs` (không được build.rs liệt kê) | 0 | 1 | 1 | 0 | bị bắt (lớp tĩnh) |
| | p4b-buildrs-wake — `vifast-plat-common/build.rs` gọi `wake` (build script **được biên dịch thật**) | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| | p4c-plat-user — shim `vifast-plat-user/lib.rs` (không phải crate) | 0 | 1 | 1 | 0 | bị bắt (lớp tĩnh) |
| **P5** symlink | p5a-file-symlink — `src/evil.rs` → `/tmp/…/evil.rs` | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| | p5b-dir-symlink — `src/evil/` → `/tmp/…/dir/` (`mod.rs` có `wake`) | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| | p5c-symlink-inside — symlink tới tệp ngoài repo **không** đuôi `.rs` | 0 | 1 | 1 | 2 | bị bắt (cả ba) |
| **P6** `build.rs` + `$OUT_DIR` | p6a-outdir-include — `include!(concat!(env!("OUT_DIR"), …))` | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| | p6b-outdir-newroot — `build.rs` + `$OUT_DIR` trong crate ở gốc mới (path-dep thường) | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p6c-outdir-macro — `include!(concat!(…))` trong thân `macro_rules!` | 0 | 1 | 1 | 1 | bị bắt (cả ba) |
| **P7** metadata giả | p7a-path-outside — path-dep ra ngoài repo, **luôn** được build | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p7b-path-mod-noncrate — `#[path]` tới `v2/kernel/evil/lib.rs` (không phải crate) | 0 | 2 | 2 | 2 | bị bắt (cả ba) |
| | p7c-bin-path-noncrate — `[[bin]] path = "../evil/main.rs"` (không phải crate) | 0 | 1 | 1 | 2 | bị bắt (cả ba) |
| | **p7d-feature-gated-outside** — path-dep ra ngoài repo, **tuỳ chọn** chỉ bật bởi `product-fs` | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT (WB-23)** |
| **P8** dev-dep / build-dep | p8a-lib-dev-dep-product — `v2/lib/vifast-net` **dev**-dep `vifast-core` + mã **sản phẩm** gọi `wake` | **101** | – | – | – | **cargo từ chối** (`E0433`) ⇒ dev-dep **không** rò ra mã sản phẩm |
| | p8b-lib-build-dep-buildrs — `v2/lib/vifast-net` **build**-dep `vifast-core` + `build.rs` gọi `wake` | 0 | 0 | 0 | 1 | bị bắt (lớp dep-info) |
| | p8c-lib-normal-dep — `v2/lib/vifast-net` **normal** dep `vifast-core` + mã sản phẩm gọi `wake` | 0 | 1 | 1 | 1 | bị bắt (cả ba, `E_SCOPE_LIB_CORE`) |

**Tổng**: **27 biến thể**; **4 đường lọt** thuộc **3 gốc** (WB-21, WB-22, WB-23); **22 bị bắt**;
**1 không biên dịch** (p8a — đúng thiết kế: dev-dep không làm `wake()` gọi được từ mã sản phẩm).
Sau vá: **4/4 đường lọt thành ĐỎ** (bảng §3.4).

**Ghi chú dựng ca** (không tính là đường lọt):
- **p1a/p1b** — crate ở gốc mới **không thể** là thành viên workspace **kernel**: cargo đòi thành
  viên phải **nằm dưới** gốc workspace (`error: workspace member … is not hierarchically below the
  workspace root`). Vì vậy crate ở gốc mới chỉ vào được ảnh sản phẩm qua **path-dependency**
  (p1c/p2a/p2c/p6b/p7d) — và đó chính là chỗ ba đường lọt nằm.
- **p2b** — crate trong cây thư mục workspace host phải `exclude` khỏi workspace host (không dùng
  `[workspace]` riêng: cargo báo `multiple workspace roots found in the same workspace`).
- **p2c** — build-dependency **chỉ được biên dịch** khi crate có `build.rs`; build script rỗng
  cũng đủ (đo: `Compiling vifast-evil`). Bản đầu không có `build.rs` ⇒ cargo chỉ cảnh báo
  "unused build dependency" và **không** biên dịch ⇒ ca **không hợp lệ**, đã dựng lại.
- **p8a/b/c** — không dùng `vifast-abi` (nó là dependency của `vifast-core` ⇒ **chu trình**);
  dùng `vifast-net` (crate lib **không** nằm trong bao đóng `vifast-core`).

---

## 3. Ba gốc đường lọt — **phạm vi định nghĩa rộng hơn vòng lặp / cấu hình build**

### 3.1 WB-21 — crate trong **bao đóng sản phẩm** ở gốc mới: luật token **không bao giờ** chạy

**Repro** (p1c): `v2/drivers/evil` (gốc mới, **không** phải thành viên workspace) là
**path-dependency tuỳ chọn** của `vifast-kbin-riscv64`, chỉ bật bởi feature `product-fs` (feature
của **ảnh sản phẩm**); `src/lib.rs` gọi `m.wake(t)`.

| Phép đo | Kết quả |
|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf` | **RC=0** (feature tắt ⇒ crate không build) |
| `cargo check -p vifast-kbin-riscv64 --features product-fs` | **RC=0** — log: `Checking vifast-evil v0.1.0` ⇒ **mã có `wake` được biên dịch vào ảnh sản phẩm** |
| `check-wake-paths.py` (thường) | **XANH** `errors=0` |
| `check-wake-paths.py --strict` | **XANH** `errors=0` |
| `check-wake-depinfo.py` | **XANH** `errors=0` |

**Gốc** — hai lỗi phạm vi **cùng lúc**:
1. `wake_scope.token_scope()` trả `False` cho crate `workspace="closure"` (chỉ
   `TOKEN_RULE_WORKSPACES`/`TOKEN_RULE_EXTRA_CRATES`), dù `contains()` trả `True` và crate **được
   biên dịch vào ảnh**;
2. lớp tĩnh chỉ duyệt `V2_SCAN` = `v2/kernel` + `v2/sim` (`iter_rs`), nên crate ở gốc mới **không
   bao giờ** vào `files` — kể cả khi `token_scope` trả `True`.

Lớp dep-info không cứu được vì nó chỉ kiểm cấu hình **`cargo check --workspace` với feature mặc
định**; path-dep tuỳ chọn bật bởi `product-fs` **không** được build ở đó.

**Vá** (fail-closed, không allowlist):
- `wake_scope.ProtectedScope.product_closure()` — bao đóng **sản phẩm**: đi từ crate workspace
  `kernel` + `vifast-sim` theo `deps` (path-dependency nội bộ, **kể cả** `optional`/build/dev).
  `token_scope()` trả `True` cho crate trong bao đóng đó — **trừ `v2/lib`** (chính sách KT-256
  giữ `v2/lib` ngoài luật token, canh bằng `E_SCOPE_LIB_CORE`; mọi đường tới `wake()` từ `v2/lib`
  đều phải qua `vifast-core`).
- `check-wake-paths._scope_token_targets()` — duyệt **`scope.roots()`** (mọi crate bảo vệ + cây
  nguồn không-phải-crate) thay vì chỉ `V2_SCAN`; trả các tệp còn thiếu để **cả hai** chế độ dùng
  chung một tập đích.

**Sau vá** (p1c-sau-va): `E_STRICT_TEST_ONLY_WAKE` tại `v2/drivers/evil/src/lib.rs:6` — **ĐỎ cả
hai chế độ lớp tĩnh**.

### 3.2 WB-22 — `include!`/`#[path]` trong crate bao đóng sản phẩm: luật **cấu trúc** không chạy

**Repro** (p1d, p1e): như p1c nhưng `wake` nằm trong tệp được `include!`:
- p1d: `include!("evil_body.rs")` — đích **trong** crate;
- p1e: `include!("../../../agent/kt258-evil-src.rs")` — đích **ngoài** crate.

Cả hai: `cargo` RC=0 (kể cả `--features product-fs`), **cả ba lượt kiểm XANH**.

**Gốc**: `resolve_include_targets()` seed worklist từ `files` (`V2_SCAN`) + `v2/lib`; tệp trong
crate ở gốc mới **không** được seed ⇒ cạnh `include!` **không** được phân giải ⇒ đích ngoài cây
không bị kiểm. (p1d còn lọt thêm vì đích nằm trong crate bao đóng — luật token cũng mù, xem
WB-21; sau khi vá WB-21 thì p1d bị bắt **hai** mã.)

**Vá**: seed worklist bằng `files + scope_targets` (tệp ngoài `V2_SCAN`). Đích của crate ngoài
`v2/kernel`+`v2/sim` bị **từ chối rõ** (`E_STRICT_WAKE_INCLUDE`) — đúng D2-188 §2 ("kiểm tra,
hoặc từ chối rõ vì ngoài hợp đồng"), không phải bỏ qua.

**Sau vá**: p1d → `E_STRICT_WAKE_INCLUDE` + `E_STRICT_TEST_ONLY_WAKE`; p1e →
`E_STRICT_WAKE_INCLUDE` — **ĐỎ cả hai chế độ**.

### 3.3 WB-23 — path-dependency **ra ngoài gốc repo**: `cargo metadata` **bỏ im lặng**

**Repro** (p7d): crate ở `/tmp/kt258-evil/crate` (ngoài repo) là path-dep **tuỳ chọn** của
`vifast-kbin-riscv64`, chỉ bật bởi `product-fs`; `src/lib.rs` gọi `m.wake(t)`.

| Phép đo | Kết quả |
|---|---|
| `cargo check … --target riscv64gc-unknown-none-elf` | **RC=0** |
| `cargo check -p vifast-kbin-riscv64 --features product-fs` | **RC=0** ⇒ mã được biên dịch vào ảnh |
| ba lượt kiểm | **XANH cả ba** |

**Gốc**: `_crate_from_package()` bỏ **im lặng** mọi package/dependency có `manifest_path`/`path`
**ngoài gốc repo** (`_rel_or_none` trả `None` ⇒ `continue`) ⇒ crate không vào phạm vi; lớp tĩnh
không quét được (ngoài gốc); lớp dep-info chỉ thấy nếu cấu hình build đang kiểm **có** build nó
(feature tắt ⇒ không). Đây đúng lớp lỗi D2-188 §2 cấm: "không được chỉ cảnh báo rồi PASS khi bỏ
qua nguồn có thể ảnh hưởng luật".

**Vá**: `CrateScope.outside_deps` ghi nhận path-dep ngoài gốc; `ProtectedScope.check_outside_dependencies()`
⇒ `E_SCOPE_OUTSIDE` (lớp tĩnh) và `E_DEPINFO_SCOPE` (lớp dep-info, cùng hàm). **Từ chối rõ**, không
im lặng.

**Sau vá**: p7d → `E_SCOPE_OUTSIDE` (l1, l1s) + `E_DEPINFO_SCOPE` (l2) — **ĐỎ cả ba**.

### 3.4 Đột biến **hai chiều** — trước vá XANH, sau vá ĐỎ

| Biến thể | Trước vá | Sau vá | Mã sau vá |
|---|---|---|---|
| p1c-feature-gated-closure | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRICT_TEST_ONLY_WAKE` |
| p1d-feature-gated-include | **XANH cả ba** | **ĐỎ** l1+l1s (2 lỗi) | `E_STRICT_WAKE_INCLUDE` + `E_STRICT_TEST_ONLY_WAKE` |
| p1e-feature-gated-include-outside | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRICT_WAKE_INCLUDE` |
| p7d-feature-gated-outside | **XANH cả ba** | **ĐỎ** l1+l1s+l2 | `E_SCOPE_OUTSIDE` / `E_DEPINFO_SCOPE` |

Log: `kt258-attack/logs/<tên>-sau-va/`. Đây là **đột biến hai chiều** thật, không phải "thêm ca
cho có".

---

## 4. Ca selftest **có răng** (lớp tĩnh: 420 → **430**)

| Ca | Khẳng định | Răng |
|---|---|---|
| **U1** | `wake` trong crate ở **gốc mới** thuộc bao đóng sản phẩm ⇒ **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; **đồng thời** `sc.token_scope(relp)` = True và mẫu cũ `_OLD_TOKEN_PAT` **không** khớp | trước vá: `token_scope`=False + ngoài `V2_SCAN` ⇒ XANH ⇒ ca **fail** |
| **U1s** | như U1 ở `--strict` ⇒ ĐỎ (cùng tập đích, hai chế độ) | trước vá: XANH |
| **U2** | **đối chứng dương**: cùng crate, **không** có `wake` ⇒ XANH | thiếu vế này thì một luật "luôn đỏ" cũng qua U1 |
| **U3** | crate bao đóng **không** thuộc sản phẩm/oracle (dep của crate host thường) ⇒ **XANH** | chứng minh phạm vi **không** mở rộng mù |
| **U4** | `v2/lib` trong bao đóng sản phẩm vẫn **ngoài** luật token (chính sách KT-256) | chốt chính sách, không âm thầm đổi |
| **U4b** | cùng crate đó phụ thuộc `vifast-core` ⇒ `E_SCOPE_LIB_CORE` | ca canh DEBT-KT227-1 **có răng** |
| **U5** | `include!` trong crate bao đóng trỏ **ra ngoài** `v2/kernel`+`v2/sim` ⇒ ĐỎ `E_STRICT_WAKE_INCLUDE` | trước vá: XANH |
| **U5b** | **đối chứng dương**: `include!` trỏ tới tệp **trong** `v2/kernel` ⇒ XANH | luật không phải luôn-đỏ |
| **U6** | path-dependency ra **ngoài** gốc repo ⇒ `E_SCOPE_OUTSIDE` | trước vá: XANH |
| **U6b** | **đối chứng dương**: path-dependency **trong** repo ⇒ không `E_SCOPE_OUTSIDE` | luật không phải luôn-đỏ |

**Một ca đổi kỳ vọng** (không xoá): ca chốt chữ ký "KT-241 `tr`: 4 call site" → **5** call site
(lớp 1 ×4 + lớp 2 ×1) vì WB-21 thêm **một** call site của **cùng** hàm luật cho tập đích ngoài
`V2_SCAN`; ca vẫn khẳng định **không** call site nào truyền `test_line_ranges`.

---

## 5. Số đo trước/sau **từng lớp**

| Phép đo | Trước KT-258 | Sau KT-258 |
|---|---|---|
| `check-wake-paths.py --selftest` | 420/420 | **430/430** (+10) |
| `check-wake-depinfo.py --selftest` | 67/67 | **67/67** (0 — dùng chung hàm `wake_scope`) |
| `check-wake-paths.py` cây thật | `errors=0` | **`errors=0`** |
| `check-wake-paths.py --strict` cây thật | `errors=0` | **`errors=0`** |
| `check-wake-depinfo.py` cây thật | `errors=0` | **`errors=0`** (21,3 s) |
| `./v2/bin/v2 check` | PASS | **PASS** (log `/tmp/kt258-final/v2check.log`) |

**Không ca nào bị xoá.** Phạm vi cây thật **không đổi** (không có crate ở gốc mới, không có
path-dep ngoài repo, không có crate `v2/lib` trong bao đóng sản phẩm nào phụ thuộc `vifast-core`)
⇒ không báo oan.

---

## 6. Bảng **7 mục D2-190** — trạng thái sau KT-258

| # | Tiêu chí D2-190 | Trạng thái | Bằng chứng |
|---|---|---|---|
| 1 | **Hợp đồng bảo vệ rõ**: crate, target, feature, nguồn sinh, phần hỗ trợ và phần từ chối | **ĐỦ** (mạnh hơn sau KT-258: phạm vi = metadata + **bao đóng sản phẩm**; ba view; **bốn** mã phạm vi `E_SCOPE_METADATA`/`E_SCOPE_REDUCTION`/`E_SCOPE_LIB_CORE`/`E_SCOPE_OUTSIDE`; phần **từ chối rõ** gồm path-dep ngoài gốc) | `CHECKER-WAKE-CONTRACT.md` §1/§3; `wake_scope.py`; §3 ở trên |
| 2 | **#16–#18 bị chặn**, gồm lỗi đọc sai vị trí đối số khi gọi UFCS | **ĐỦ** (WB-16/17 đóng ở tầng API — `E0308`/`E0631`; WB-18 đóng ở tầng phạm vi — KT-238 + KT-256; vòng này **tái xác nhận** bằng p2a/p2c/p6b/p7a — mọi biến thể "crate ngoài ba gốc" **luôn được build** đều bị lớp dep-info bắt) | `BYPASS-REGISTRY.md`; §2 |
| 3 | **Toàn bộ repro cũ giữ bền** + đối chứng hợp lệ | **ĐỦ** (420→430 ca lớp tĩnh, 67→67 ca lớp dep-info; **không** ca nào bị xoá; một ca đổi kỳ vọng 4→5 call site vì luật **mạnh lên**, ghi ở §4) | `--selftest` hai lớp |
| 4 | **Ranh giới API được compiler cưỡng chế**, không chỉ checker tự xác nhận | **ĐỦ** (KT-241: `NonReadyTarget` không có `Ready`; `write_state` private; mỏ neo kiểu). Vòng này **đo thêm**: dev-dep `vifast-core` **không** làm mã sản phẩm `v2/lib` gọi được `wake` (`cargo` **E0433**, p8a) | §2 (P8); `JOURNAL/kt241-checker.md` |
| 5 | **Reviewer khác tác giả** tấn công theo nhóm | **ĐỦ cho vòng này** — **27 biến thể/8 nhóm** trên HEAD **sau KT-256**, đúng nhóm D2-190 §5 (crate mới/dependency mới/đường dẫn đặc biệt; macro/cfg/nguồn sinh; helper/closure/const/static; alias/UFCS/fn-pointer) | §2; `kt258-attack.tsv` |
| 6 | **Gate trên commit tích hợp cuối PASS**, có log + danh tính công cụ | **CHƯA** — task chỉ chạy `v2 check` + hai selftest trên nhánh task; **T2 `gate --fast` trên commit tích hợp** do lead chạy sau merge | §5 |
| 7 | **Không còn đường lọt đã biết** nằm trong hợp đồng bảo vệ | **ĐỦ cho luật `wake`** sau khi đóng WB-21/WB-22/WB-23 (mọi đường lọt dựng được trong vòng này đều **ĐỎ** sau vá). Hợp đồng §4 còn **ba** mục mở **có tên** nhưng **ngoài** luật `wake`: `:expr` passthrough; miễn luật arch theo tiền tố `virtio_`; **luật cấu trúc** (`mod` graph / `scan_lexical`) **chưa** mở cho crate ngoài `V2_SCAN` (chỉ `include!`/`#[path]` được mở ở WB-22) | `BYPASS-REGISTRY.md`; §7 |

**Kết luận**: **KHÔNG** tự tuyên bố đóng KT-184/KT-194. Còn **mục 6** (gate T2 trên commit tích
hợp — lead) và **mục 7** còn **ba nợ có tên** ngoài luật `wake`. Mục 5 đạt **cho một vòng nữa**;
mục 1–4 đạt.

---

## 7. Nợ còn lại **có tên** (không đổi, ghi để vòng sau không phát hiện lại)

1. **`:expr` passthrough** (KT-202) — `$…:expr` chở được `include!` ở vị trí biểu thức; chỉ hở
   khi macro được gọi từ tệp ngoài phạm vi sản phẩm. **Chưa vá** (cây thật có `bail` dùng `:expr`
   hợp lệ).
2. **Miễn luật arch theo tiền tố `virtio_`** — `RE_DOORBELL` bỏ qua tệp tên bắt đầu `virtio_`;
   phân biệt "thiết bị virtio thật" cần kiểm ngữ nghĩa. **Chưa vá**.
3. **Luật cấu trúc cho crate ngoài `V2_SCAN`** — WB-22 mở **worklist `include!`/`#[path]`**;
   `scan_lexical`/`scan_structure`/`mod` graph vẫn chỉ chạy `v2/kernel`+`v2/sim`(+`v2/lib` cho
   lexical). Hệ quả: một crate trong bao đóng sản phẩm có **lỗi từ vựng** hoặc **`mod` graph lạ**
   không bị lớp tĩnh bắt; lớp dep-info bắt nếu cấu hình build đang kiểm **có** build nó. **Chưa
   vá** — cần mở rộng `scan_structure` + `_inside_scan_tree` theo phạm vi (việc của task sau).
4. **Lớp dep-info chỉ kiểm feature mặc định** — `cargo check --workspace` không bật `product-fs`;
   mã chỉ tồn tại dưới feature của ảnh sản phẩm **không** được lớp dep-info thấy. Với luật `wake`
   điều này **đã được bù** bằng lớp tĩnh (quét **mọi** tệp trong phạm vi, không phụ thuộc
   feature) sau WB-21/WB-22; nhưng các luật khác của lớp dep-info (R2/R3/R4) vẫn chỉ thấy cấu
   hình mặc định. **Chưa vá** — cần thêm lượt `cargo check --features product-fs` vào lớp dep-info
   (chi phí ~+40 s).

---

## 8. Đề xuất cho lead (không tự quyết)

1. **Chạy T2 `gate --fast` trên commit tích hợp** (mục 6 D2-190) — điều kiện **bắt buộc** còn
   thiếu để **đề nghị** đóng KT-184/KT-194.
2. **Cân nhắc đóng KT-184/KT-194** khi (a) T2 xanh trên commit tích hợp, (b) lead chấp nhận ba nợ
   §7 là **ngoài** hợp đồng hữu hạn đã công bố (D2-190 §7). **Đề xuất của tôi**: **chưa đóng** —
   nợ §7.3 (luật cấu trúc cho crate ngoài `V2_SCAN`) là **cùng lớp lỗi** với WB-22 và có thể dựng
   lại đường lọt bằng `mod` graph; nên mở một task nhỏ (≤1,5 h) mở rộng `scan_structure` trước.
3. **Nối `check-no-cfg-test.py` vào `gate --fast`** (đề xuất của KT-256, vẫn mở).
4. **Thêm lượt `--features product-fs` vào lớp dep-info** (nợ §7.4) — nếu muốn lớp dep-info phủ
   đúng cấu hình ảnh sản phẩm.

---

## 9. Tái lập

```bash
cd /tmp/kt258-s1
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + cây thật
python3 v2/tools/check-wake-paths.py --selftest          # 430/430
python3 v2/tools/check-wake-depinfo.py --selftest        # 67/67
python3 v2/tools/check-wake-paths.py                     # errors=0
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py                   # errors=0

# (b) một biến thể (tự khôi phục bằng trap)
bash v2/agent/JOURNAL/kt258-attack/run-attack.sh "P1-crate-moi-o-goc" \
     p1c-feature-gated-closure v2/agent/JOURNAL/kt258-attack/cases/p1c-feature-gated-closure.sh

# (c) cổng
./v2/bin/v2 check
```

**Lưu ý cho người chạy lại**: harness khôi phục `v2/{kernel,sim,lib}` + **hai manifest workspace**
(`v2/Cargo.toml`, `v2/kernel/Cargo.toml`) + crate ở gốc mới (`v2/{drivers,net,foo,tools}`) + tệp
phụ `v2/agent/kt258-evil-src.rs` + `/tmp/kt258-evil`. `git clean -fd` **không** xoá tệp bị
`.gitignore` ⇒ harness xoá tường minh `v2/{kernel,sim,lib}/*/src/target`.

---

## 10. Tệp thay đổi trong KT-258

| Tệp | Thay đổi |
|---|---|
| `v2/tools/wake_scope.py` | `product_closure()` (bao đóng sản phẩm); `token_scope()` mở cho crate trong bao đóng (trừ `v2/lib`); `CrateScope.outside_deps` + `check_outside_dependencies()` (WB-23) |
| `v2/tools/check-wake-paths.py` | `_scope_token_targets()` (duyệt `scope.roots()`); `scope_targets` dùng cho **cả hai** chế độ; seed worklist `include!`/`#[path]` bằng `files + scope_targets` (WB-22); `E_SCOPE_OUTSIDE`; **+10 ca** selftest (U1, U1s, U2, U3, U4, U4b, U5, U5b, U6, U6b); ca chốt chữ ký 4→5 call site |
| `v2/tools/check-wake-depinfo.py` | `E_DEPINFO_SCOPE` cho path-dep ngoài gốc (cùng hàm `wake_scope`) |
| `v2/agent/BYPASS-REGISTRY.md` | thêm **WB-21**, **WB-22**, **WB-23** (CLOSED) |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §1 (phạm vi = metadata + bao đóng sản phẩm), §3 (`E_SCOPE_OUTSIDE`), §4 (ba nợ §7) |
| `v2/agent/JOURNAL/kt258-wake-attack.md` | nhật ký này |
| `v2/agent/JOURNAL/kt258-attack.tsv` | bảng ma trận 27 biến thể (+ 4 dòng sau vá) |
| `v2/agent/JOURNAL/kt258-attack/**` | harness + 27 ca + log + `run-attack.sh` |

**Không** sửa: `v2/kernel/**`, `v2/sim/**`, `v2/lib/**`, `spec/`, `v2/bin/v2`,
`v2/agent/{STATE,QUEUE,DECISIONS}.md`.
