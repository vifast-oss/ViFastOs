# KT-346 — Tách userland sản phẩm sang `v2/user/` + đóng nợ P20 (3 arch)

**Ngày**: 2026-11-24 (khung 05:30–08:15 JST) · **Nhánh**: `task/kt346b` · **Base**: `main` @ `741cc840`
**Lệnh**: D2-252 (Pha A), D2-239, D2-233, D2-245; AGENTS §6.3 (nợ P20) · **Ngân sách**: 3 h
**Commit**: `4f2a8892` (mã + `v2/user/`) + commit journal (dưới)

## 1. Việc làm

### 1.1 Tách cây `v2/user/` (cây nguồn độc lập, không phải crate)

`git mv` **7 tệp** (rename 100 %, không đổi một byte nội dung) + thêm `v2/user/README.md`:

| Trước | Sau | Ai dùng (trước → sau) |
|---|---|---|
| `v2/kernel/vifast-plat-user/lib.rs` | `v2/user/runtime/lib.rs` | `vifast_user_abi` — `build.rs` của **cả 3** kbin (trước: 3 build.rs trỏ `../vifast-plat-user/lib.rs`; sau: `../../user/runtime/lib.rs`) |
| `v2/kernel/vifast-plat-user/programs/{echo,procsrv,shell,shell_cmds}.rs` | `v2/user/programs/` | 3 build.rs → `$OUT_DIR/{shell,procsrv,echo}.elf` (riscv64 còn dùng shell/procsrv cho ảnh `ktest`) |
| `v2/kernel/vifast-kbin-riscv64/init/hello_user.rs` | `v2/user/programs/hello.rs` | riscv64 `product-exec` → `hello.elf` (`proc_exec.rs` `include_bytes!`) |
| `v2/kernel/vifast-kbin-riscv64/init/netsrv_prod_user.rs` | `v2/user/programs/netsrv_prod.rs` | riscv64 `product-net` → `netsrv_prod.elf` |

**Tổng `v2/user/`**: **8 tệp** (7 `.rs` + `README.md`) — **3.017 dòng** `.rs`:
`runtime/lib.rs` 936 · `programs/shell.rs` 1.414 · `programs/shell_cmds.rs` 243 ·
`programs/procsrv.rs` 178 · `programs/netsrv_prod.rs` 138 · `programs/echo.rs` 75 ·
`programs/hello.rs` 33.

- `v2/kernel/vifast-plat-user/` **không còn tồn tại** (không để façade nửa vời: một cây nguồn duy
  nhất). **Tên ELF giữ nguyên** ⇒ **không** phải sửa `include_bytes!`/`concat!(env!("OUT_DIR"), …)`
  ở bất kỳ `src/` nào (`k5user.rs`, `proc_rt.rs`, `shellpath.rs`, `proc_exec.rs`, `userload.rs`).
- **`--remap-path-prefix`**: riscv64 đã có `remap_flags()` (KT-92/A1-184); KT-346 port nguyên khuôn
  sang `vifast-kbin-aarch64/build.rs` + `vifast-kbin-x86_64/build.rs` (đọc
  `CARGO_ENCODED_RUSTFLAGS`/`RUSTFLAGS`, truyền lại cho **mọi** lần gọi `$RUSTC` lồng nhau: shim
  `rlib`, rlib phụ, ELF user). Nhờ vậy đường dẫn tuyệt đối của cây build **không** vào byte ELF
  user ⇒ không vào ảnh qua `include_bytes!`.

### 1.2 Dấu vân tay `kt134-v4` → `kt134-v5`

- `v2/test/boot/build-rev.sh`: `VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/user"`,
  `VBREV_ALGO=kt134-v5`.
- `v2/test/boot/build-rev.algo.pin`: cặp (sha256 script, `kt134-v5`) cập nhật **cùng commit**.
- `v2/tools/wake_scope.py`: `NON_CRATE_SOURCE_TREES` `v2/kernel/vifast-plat-user` → `v2/user`;
  `_FALLBACK_TOKEN` phủ thêm `user` (mẫu dự phòng dùng khi không dựng được `cargo metadata` —
  đúng đường selftest, nếu thiếu thì ca T2 mất răng).
- `v2/tools/check-wake-paths.py`: 4 ca selftest dùng đường dẫn shim đổi sang `v2/user/runtime/lib.rs`.
- `v2/bin/lib/util.sh`: R5/D2-17 — allowlist `unsafe` thêm `"$root"/user/runtime/*` (cùng **một**
  tệp shim `git mv`, D2-17 vẫn cho `unsafe` ở shim ABI; chỉ đổi đường dẫn).
- `v2/tools/repro-check.sh`: bản sao build thêm `v2/user` + `v2/rust-toolchain.toml`; thêm `--arch`;
  **quan trọng**: mỗi bản sao được `git init` + commit nội dung vừa chép (**repo git riêng**). Trước
  KT-346 bản sao **không có `.git`** ⇒ `vbrev_inputs_calc` trả `unknown` ⇒ `vbrev_image_identity`
  (KT-145, đòi `inputs=[0-9a-f]+`) **từ chối** ⇒ `repro-check.sh` **không thể xanh** trên macOS.
  Nay mỗi bản sao có danh tính **thật** (`dirty=0`, `inputs` = dấu vân tay nội dung), và phép so
  danh sách tệp bỏ `.git/` (không phải đầu vào build).
- `v2/tools/repro-build.sh`: thêm `--arch riscv64|aarch64|x86_64` (target/bin/package theo arch).

### 1.3 P20 — danh tính bản dựng cho **cả ba** arch

- Port `build_identity` (KT-86/D2-125.3) sang `vifast-kbin-aarch64/build.rs` +
  `vifast-kbin-x86_64/build.rs`: `algo_from_script`/`inputs_from_script` (gọi **chính**
  `v2/test/boot/build-rev.sh` — một nguồn sự thật, không chép lại định nghĩa), `git_head12`,
  `git_dirty` (phạm vi `v2/kernel`+`v2/lib`+`v2/user`), `identity_watch_paths` (watch từng crate +
  `v2/user`).
- `BUILD_LINE` (const `concat!` từ `cargo:rustc-env`) trong `src/main.rs` hai arch; in **ngay sau**
  `# conf image:` trong `product::boot` — aarch64 sau `image::report_image_digest()`, x86_64 sau
  `writeln!("# conf image: …")`. Định dạng khớp riscv64: `# build: algo=<algo> inputs=<16 hex|unknown> dirty=<0|1>`.

### 1.4 Ca kiểm mới trong `build-rev.sh --selftest` (test đỏ-trước)

- `(l)` sửa `v2/user/programs/shell.rs` rồi commit ⇒ dấu vân tay **PHẢI** đổi (phép kiểm **dương**
  của phạm vi `v2/user`; ĐỎ nếu bỏ `v2/user` khỏi `VBREV_INPUT_ROOTS`).
- `(m)` **hai thư mục khác nhau, cùng nội dung ⇒ dấu vân tay PHẢI GIỐNG** (bất biến P20: danh tính
  bền không phụ thuộc đường dẫn build; ĐỎ nếu chèn đường dẫn tuyệt đối vào chuỗi băm).

## 2. Bằng chứng

### 2.1 P20 (i) — build ở **hai thư mục khác nhau**, cùng nội dung (commit `4f2a8892`)

Hai worktree **tách biệt** `git worktree add --detach /tmp/kt346-pa <sha>` và
`/tmp/kt346-pb <sha>` (cùng commit, hai đường dẫn tuyệt đối khác nhau), build **cả 3 arch** bằng
`v2/tools/repro-build.sh --arch <arch> --target-dir /tmp/kt346-{pa,pb}-t/<arch>`.

**Hai dòng `# build:` đọc từ log boot thật** (QEMU, ảnh sản phẩm canonical):

```
PA riscv64: # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
PB riscv64: # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
PA aarch64: # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
PB aarch64: # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
PA x86_64 : # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
PB x86_64 : # build: algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0
```

⇒ **GIỐNG NHAU** (cùng `inputs`, cùng `dirty`, cùng `algo`) trên cả **6** lượt build/boot.

**sha256 ảnh sản phẩm canonical (byte-identical giữa hai thư mục)**:

| arch | PA sha256 | PB sha256 | size | build= (FNV .text) |
|---|---|---|---|---|
| riscv64 | `8323829e235c2d587153cd12a8541587d264d4235e4cb18af43be287a92875ea` | *(giống)* | 82.864 | `ee0f458d1bfacd79` text=51.764 |
| aarch64 | `c29c6f6b4a895f7d81687182370ccca234c98085a1c35c0434fde0e0ce2fd0f3` | *(giống)* | 82.640 | `29f255e1f6e46846` text=5.840 |
| x86_64 | `ddadfeb79bb4c308537e3b9e1a60eec1ef6daa3f34f77ce831d141cefd817446` | *(giống)* | 86.880 | `a61e1b79c410454f` text=69.632 |

(repro-build in `REPRO … algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0 rev=4f2a889200c8`.)

### 2.2 P20 (ii) — sửa **một** tệp userland ⇒ dấu vân tay **ĐỔI**

Trong `/tmp/kt346-pa` (sạch, HEAD `4f2a8892`), thêm **một dòng chú thích** vào
`v2/user/programs/shell.rs` rồi commit:

```
fp_before=1062d73a96dc4409 count=396
fp_after =6bf8b328f2c65335 count=396   (commit f14186d5)
```

⇒ dấu vân tay **đổi** (396 mục) — `v2/user` thực sự nằm trong phạm vi.

### 2.3 Hai đột biến ĐỎ

**(i) chèn đường dẫn tuyệt đối vào chuỗi băm** (`printf 'absolute-root\t%s\n' "$(cd "$root" && pwd -P)" >>"$oids"`):
cùng nội dung, hai thư mục ⇒ `PA_fp=c137246bcdc90e06`, `PB_fp=3fc253ff5f2f83bd` ⇒ **KHÁC**; ca `(m)`
của selftest **ĐỎ** đúng thông điệp:

```
# vbrev-selftest (m) FAIL: cùng nội dung, hai thư mục KHÁC nhau ⇒ dấu vân tay khác
#   (e745cada99bed5f3 vs 5f1d9ea9faf82e39) — dấu vân tay còn phụ thuộc đường dẫn build
# vbrev-selftest: FAIL   (exit 1)
```

**(ii) bỏ `v2/user` khỏi `VBREV_INPUT_ROOTS`** (`="v2/kernel v2/lib"`): sửa + commit đúng tệp
`v2/user/programs/shell.rs` mà dấu vân tay **KHÔNG** đổi (`fp_before=fp_after=cae6f62faabf30d4`,
388 mục); ca `(l)` của selftest **ĐỎ**:

```
# vbrev-selftest (l) FAIL: sửa  mà dấu vân tay KHÔNG đổi (9f1d2d9b482dbea1)
#   — 'v2/user' thiếu trong VBREV_INPUT_ROOTS
# vbrev-selftest: FAIL   (exit 1)
```

### 2.4 `exec /hello.elf` từ FS — **CHƯA ĐÓNG** (nói thẳng)

`run`/`runfs`/`exec` trên ảnh sản phẩm riscv64 vẫn dùng **bảng ảnh nhúng** (`proc_exec.rs`
`static HELLO_ELF = include_bytes!($OUT_DIR/hello.elf)`, khoá `"/hello.elf"`); **chưa** đọc ELF từ
FS. Chứng minh cũ (`# product exec: path=/hello.elf … check=1`, `# hello: run ok`) vẫn nguyên giá
trị nhưng **không** phải bằng chứng "đọc từ FS" ⇒ **không** dùng nó để tuyên bố tiêu chí này.
Lý do kỹ thuật + việc còn lại ghi ở §5 (nợ).

### 2.5 `repro-check.sh` — cả **3 arch**, bản sao có `.git` (danh tính **thật**)

`bash v2/tools/repro-check.sh [-q] --arch {riscv64,aarch64,x86_64}` — mỗi lượt dựng **hai bản sao**
ở hai đường dẫn tuyệt đối khác nhau (ngắn/dài), mỗi bản sao là **repo git riêng**, build ảnh sản
phẩm canonical, so sha256:

| arch | sha256 (hai thư mục) | size | build=/text= | kết quả |
|---|---|---|---|---|
| riscv64 | `8323829e235c2d587153cd12a8541587d264d4235e4cb18af43be287a92875ea` | 82.864 | `ee0f458d1bfacd79` / 51.764 | **PASS** |
| aarch64 | `c29c6f6b4a895f7d81687182370ccca234c98085a1c35c0434fde0e0ce2fd0f3` | 82.640 | `29f255e1f6e46846` / 5.840 | **PASS** |
| x86_64 | `ddadfeb79bb4c308537e3b9e1a60eec1ef6daa3f34f77ce831d141cefd817446` | 86.880 | `a61e1b79c410454f` / 69.632 | **PASS** |

Ba sha256 **trùng khớp** với ba sha256 ở §2.1 (đo bằng phương pháp độc lập: `git worktree` +
`repro-build.sh` thay vì `repro-check.sh`) ⇒ hai đường đo cùng kết luận.

## 3. Cổng đã chạy

| Lệnh | Kết quả |
|---|---|
| `bash v2/test/boot/build-rev.sh --selftest` | PASS **a..m** (14 ca; thêm (l)/(m)) |
| `python3 v2/tools/check-wake-paths.py --selftest` | **461/461** ok, exit 0 |
| `bash v2/tools/check-algo-pin.sh --strict` | OK `algo=kt134-v5` |
| `./v2/bin/v2 check` | PASS (exit 0) |
| `./v2/bin/v2 gate --dev` (T1) | **PASS** (`registered=…`, TỔNG 693.146 ms; log `/tmp/kt346-gate2.log`) |
| `bash v2/test/boot/run-x86_64.sh` | **37/37 kiểm sạch, tests=47, exc=6, ca âm 32/32** (exit 0) |
| `bash v2/test/boot/run-aarch64.sh` | **37/37 kiểm sạch, tests=46, ca âm 27/27** (exit 0) |
| `bash v2/test/boot/ktest.sh 1 matrix` | **27/27 boot sạch** (`ktest.sh: PASS`, smp=matrix, NBOOT=1) |
| `bash v2/test/boot/product-riscv64.sh` | **PASS** `checks=0 fails=0` + `REV-GATE: ACCEPT inputs=1062d73a96dc4409 current_inputs=1062d73a96dc4409 head_rev=4f2a889200c8 count=396` (lượt 2 — xem T-INFRA dưới) |
| `bash v2/tools/repro-check.sh` | **PASS** riscv64 `sha256=8323829e…` — **2 thư mục khác nhau, cùng sha256** |
| `bash v2/tools/repro-check.sh --arch aarch64` | **PASS** `sha256=c29c6f6b…` |
| `bash v2/tools/repro-check.sh --arch x86_64` | **PASS** `sha256=ddadfeb7…` |

### 3.1 Số dòng tệp chạm trần (R1 ≤ 1.500/tệp)

| tệp | dòng | ghi chú |
|---|---:|---|
| `v2/user/programs/shell.rs` | **1.414** | **không đổi** một dòng (chỉ `git mv`); còn 86 dòng tới trần |
| `v2/kernel/vifast-kbin-riscv64/build.rs` | 751 | +24 dòng (đổi đường dẫn + `v2/user` trong watch) |
| `v2/kernel/vifast-kbin-aarch64/build.rs` | 539 | 271 → 539 (port `build_identity` + `remap_flags`) |
| `v2/kernel/vifast-kbin-x86_64/build.rs` | 484 | 216 → 484 (như trên) |

(các tệp nóng khác **không chạm**: `core_ipc.rs` 1.498, `plat-x86_64/asm.rs` 1.475,
`machine.rs` 1.430 — KT-346 không sửa.)

### 3.2 Cây sạch + HEAD cuối

- `git status --porcelain` = **0 dòng**; HEAD = commit tài liệu này trên `4f2a8892` (hash ở báo cáo cuối).
- Dấu vân tay tại HEAD cuối: `algo=kt134-v5 inputs=1062d73a96dc4409 dirty=0 count=396`
  — **trùng** dấu vân tay của 6 ảnh ở §2.1 ⇒ ảnh/bằng chứng vẫn hợp lệ sau commit tài liệu.

### 3.3 Ghi chú cổng

- **T-INFRA (1 lần, D2-07)**: lượt đầu `product-riscv64.sh` chạy **song song** với `ktest.sh 1 matrix`
  (27 boot) ⇒ 10 mục `not ok` ở các boot thứ 3–4 (`kt343-alias-exec.log` chỉ có boot + halt, shell
  **không** lên prompt trong `READY_DS`), trong khi boot 1–2 (`product`, `runfs`) xanh và
  `data+bss=224472` **đúng** baseline KT-343. Chạy **lại một lần** trên máy rảnh: **PASS**
  `fails=0` + `REV-GATE ACCEPT`. Phân loại: **quá tải QEMU song song** (27+4 boot cùng lúc), không
  phải hồi quy mã — bằng chứng: ảnh `.data+.bss` không đổi, `ktest matrix` cùng thời điểm xanh,
  lượt chạy đơn lẻ xanh 100 %.
- TAP **giữ nguyên**: aarch64 **46**, x86_64 **47**, riscv64 `ktest matrix` **27/27**;
  ca âm x86_64 **32/32**, aarch64 **27/27**.

## 4. P20 còn phụ thuộc gì (nói thẳng)

- **Đã đóng**: (a) cả 3 arch đều có `build_identity` + dòng `# build:` trên ảnh sản phẩm;
  (b) `v2/user` nằm trong dấu vân tay (`kt134-v5`); (c) dấu vân tay **không** phụ thuộc thư mục
  build (6 boot ở 2 thư mục, ca `(m)` + đột biến i chứng minh hai chiều); (d) `repro-check.sh`
  chạy được cho **cả 3 arch** trên macOS với `inputs` **thật** (`dirty=0`), sha256 khớp.
- **Còn phụ thuộc**: (1) **git**: dấu vân tay băm blob **ở HEAD** ⇒ cần `.git` + `cargo metadata
  --locked --offline`; bản sao/nhánh không có git ⇒ `inputs=unknown` (cổng fail-closed, nhưng là
  giới hạn của định nghĩa — không phải "đã đóng"). (2) **toolchain ghim** `1.100.0` + `rustc-canon`
  wrapper vẫn là điều kiện để sha trùng; máy khác chưa đo (`OD-K6-3` vế 2 máy; Windows nợ `v2.1`).
  (3) **`include_bytes!` chưa gỡ**: ELF user vẫn được nhúng ở `build.rs` (đóng gói vào đĩa/initrd
  FS là tiêu chí của `KT-347`/`KT-346` mục 4 — chưa làm, xem §4 nợ 1). (4) `dirty` của bản build
  vẫn phụ thuộc **trạng thái** cây (đúng thiết kế) — không phải phụ thuộc đường dẫn.

## 5. Nợ (nói thẳng)

1. **`exec` đọc ELF từ FS — CHƯA LÀM** (tiêu chí 4 của task). Cần: (a) seed `/hello.elf` vào đĩa
   FS bằng `vifast-fsutil` ở `v2/test/boot/product-riscv64.sh` (hiện đĩa `dd if=/dev/zero` 4 MiB,
   FS format lúc boot, `/hello.elf` **không** có trong FS); (b) API đọc cả tệp từ `proc_fs`
   (hiện chỉ `OP_CAT` 24 B/lần qua IPC); (c) `proc_exec` dùng buffer đọc FS thay bảng nhúng.
   Nên làm ở `KT-347` (U1a cũng cần `exec` từ FS) cùng lúc, tránh sửa runner hai lần.
2. **`repro-check.sh` trên macOS — ĐÃ VÁ trong task này**: bản sao nay là **repo git riêng**
   (`git init` + commit) nên ảnh mang `inputs` **thật** + `dirty=0`; cả 3 arch PASS (§2.5). Vẫn
   còn: `OD-K6-3` cần **máy thứ hai** (Windows giữ nợ `v2.1` theo B2/D2-122.4); và `repro-check`
   vẫn chưa nằm trong `gate`/`nightly` (chạy tay, ~1–2 phút/arch).
3. `# build:` mới in trên **ảnh sản phẩm** hai arch (đúng yêu cầu P20); ảnh `ktest` của
   aarch64/x86_64 **không** in (riscv64 cũng vậy) — cổng `vbrev_gate` hiện chỉ chạy riscv64.
4. `v2/user` chưa có kiểm `cargo fmt`/clippy riêng (như trước, userland biên dịch bằng `$RUSTC`
   trực tiếp); R1 vẫn đo `shell.rs` = 1.414/1.500.
