# KT-183 — `cd` vào thư mục con **sâu** (FS sản phẩm nhiều cấp)

**Ngày**: 2026-11-19/20 · **Task**: KT-183 (D2-164, hàng chờ đêm vòng 3) · **Nhánh**:
`night3/L39-kt183` (tách từ `main` tại `8f5a1189`) · **Worktree**: `/private/tmp/night3-L39-kt183`.

**Bối cảnh đo được** (D2-164): KT-158 mới seed FS sản phẩm **một** cấp `/docs` (+ `/docs/note.txt`);
D2-164 yêu cầu kiểm `cd` vào thư mục con **sâu** (nhiều cấp) — "seed thêm `/docs/sub` + tệp, kiểm
`cd /docs/sub`, `pwd`, `cd ../..`". Trước KT-183 FS sản phẩm **không** có thư mục sâu nên các vế
"nhiều cấp", "`..` lùi hai cấp", "chuẩn hoá `..`", "**kẹp** ở gốc" **không kiểm được**.

**Commit**: `4fc80f61` — `[KT-183] seed /docs/sub + deep.txt; ca kt183-cd-sau kiem cd thu muc sau
(8 phep kiem); dot bien .. / pwd hang / => ca DO` (commit nhật ký này là commit thứ hai).

---

## 1. Kết luận ngắn

- `seed()` nay tạo **năm** mục: `/README` (như cũ), `/docs` (như cũ), `/docs/note.txt` (như cũ),
  **`/docs/sub`** (`INODE_DIR`) và **`/docs/sub/deep.txt`** (`INODE_FILE`, 4 B `deep`), `fsync` sau
  mỗi lần tạo **như cũ**. `create` trả `Exists` khi đĩa đã seed lần trước ⇒ bỏ qua, **không** ghi
  đè dữ liệu người dùng (cùng chính sách `/README`/`/docs`).
- Ca kiểm **mới** `kt183-cd-sau` (13 lệnh, **8 phép kiểm**, mỗi lệnh một dòng phán quyết):
  `cd /docs/sub` ⇒ `pwd` in `/docs/sub`; `cat deep.txt` (tương đối) ⇒ đúng nội dung; `cd ../..` ⇒
  `/`; `cd /docs/sub/..` ⇒ `/docs` (chuẩn hoá `..`); `cd ../../..` từ `/docs/sub` ⇒ `/` (**kẹp**
  gốc, không âm); đối chứng `cd /docs` ⇒ `/docs`; hai phép kiểm **chuỗi** `cd`/`pwd` bắt lệch thứ tự.
- **Đột biến chứng minh ca kiểm có răng** (hai chiều, log lưu đủ):
  - **A — `..` không được xử lý** (`resolve_path`): ca mới **ĐỎ 5/8** (`cd-hai-cap`, `cd-chuan-hoa`,
    `cd-kep-goc`, `thu-tu-cd`, `thu-tu-pwd`); hoàn nguyên ⇒ **XANH 8/8**.
  - **B — `pwd` in hằng `/`** (`cmd_pwd`): ca mới **ĐỎ 4/8** (`cd-sub`, `cd-chuan-hoa`, `doi-chung`,
    `thu-tu-pwd`); hoàn nguyên ⇒ **XANH 8/8**.
- Ảnh sản phẩm **mới** (sau commit, cây sạch): `sha256=639ca6d1…`, **dấu vân tay build-inputs
  `43df126a4a6702ec`** (cũ `89f40ef16c97e6d5`), `dirty=0`, `rev=4fc80f61991f`.
- `bash v2/test/boot/shell-edge.sh --selftest` **PASS 46/46** (10/10 ca; 26 sai→ĐỎ + 20 đúng→XANH).
- `bash v2/test/boot/shell-edge.sh --image <ảnh mới>` **PASS toàn bộ** — 8/8 ca + `truncate-jobs` +
  `new-cmds-ok` + `new-cmds-edge` + `new-cmds-edge2` + `new-cmds-edge3` + `k7-uu-tien` + `k7-thu-muc` +
  `kt137-cdpwd` + `kt183-cd-sau`; **73 dòng PASS, 0 FAIL**.
- `./v2/bin/v2 check` **PASS** trên cây trước commit (`4fc80f61` + thay đổi của task) — log
  `kt183-logs/kt183-v2-check-precommit.log`; chạy lại trên HEAD cuối sau commit nhật ký.
- **Không** sửa `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/QUYET-DINH-*`,
  `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`.

## 2. Thay đổi

### 2.1 `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs`

- Thêm `const DOCS_DEEP: &[u8] = b"deep";` (4 B, **không** `\n` cuối — cùng quy ước `DOCS_NOTE`).
- `seed()` thêm hai bước **sau** `/docs/note.txt`:
  `create("/docs/sub", 0, INODE_DIR)` rồi `create("/docs/sub/deep.txt", 0, INODE_FILE)` + `write` +
  `fsync`. Thứ tự "thư mục trước, tệp sau" vì `create` đòi thư mục cha đã có (`resolve_parent`).
- **Không** đổi `TOTAL_BLOCKS`, **không** đổi `RAM_VOL_BLOCKS` (96), **không** đụng ngân sách
  `.data+.bss`: năm inode + bốn block dữ liệu nằm trong volume sẵn có. Dòng `# product fs:` trên ảnh
  mới in `used=6` (trước `used=4`) — **không** harness nào khẳng định `used=` (đã rà: `t26.sh` chỉ
  đòi có `# product fs: medium=`; `shell-edge.sh --check-image` cũng chỉ đòi chuỗi đó).

### 2.2 `v2/test/boot/shell-edge.sh`

- **Ca mới 17 `kt183-cd-sau`** (13 lệnh, 8 phép kiểm): tám hàm `ok_kt183_*` + `check_kt183_cd_sau`
  (dùng `emit_verdicts` sẵn có). Các phép kiểm **vị trí** (dòng `cd` thứ n + dòng `pwd` ngay sau,
  helper `kt183_next_pwd`) quy lỗi đúng lệnh — `cd /` xuất hiện **hai** lần và `cd /docs` **hai**
  lần nên đếm tổng không phân biệt được lệnh nào hỏng.
- `run_real` thêm khối (17) **trước** (8b) (để `/README` còn 79 byte — 8b truncate nó về 5); ca
  **không** tạo tệp ở gốc nên không đổi `ls n=` của ca nào khác.
- `--selftest` thêm **bốn** phép kiểm cho ca 17: log **đúng** (đầu ra thật của ảnh) + **ba** log
  **sai** ứng với ba kiểu hỏng thật — (a) trước KT-183 (FS **không** có `/docs/sub`), (b) đột biến
  "`..` không được xử lý", (c) đột biến "`pwd` in hằng `/`". `SELFTEST_CASES` 9 → **10**.
- **Cập nhật ca cũ `kt137-cdpwd`** (hệ quả **tất yếu** của seed mới, không phải nới ngưỡng):
  `/docs` nay có **hai** mục (`note.txt` + `sub`) nên `ok_kt137_ls_docs` đổi kỳ vọng
  `ls n=1 path=/docs` → **`ls n=2 path=/docs`** và thêm vế **`  d sub`** (×2) — vế mới **mạnh hơn**
  (chứng minh thư mục sâu hiện ra trong bảng `ls` của thư mục cha). Log giả **đúng** của ca 16 trong
  selftest cập nhật theo. Không phép kiểm nào bị bỏ/nới.

## 3. Ca kiểm mới — kết quả trên ảnh mới

Lệnh (13): `cd /docs/sub` `pwd` `cat deep.txt` `cd ../..` `pwd` `cd /docs/sub/..` `pwd`
`cd /docs/sub` `cd ../../..` `pwd` `cd /docs` `pwd` `cd /`.

| Phép kiểm | Kỳ vọng | Kết quả (ảnh `639ca6d1…`) |
|---|---|---|
| `cd-sub` | dòng `cd` đầu = `# shell: cd /docs/sub`; `pwd` ngay sau = `# shell: pwd /docs/sub` | PASS |
| `cat-deep` | đúng 1 dòng `deep` + 1 `# shell: cat ok bytes=4 first=deep` | PASS |
| `cd-hai-cap` | dòng `cd` thứ 2 = `# shell: cd /`; `pwd` ngay sau = `# shell: pwd /` | PASS |
| `cd-chuan-hoa` | dòng `cd` thứ 3 = `# shell: cd /docs`; `pwd` ngay sau = `# shell: pwd /docs` | PASS |
| `cd-kep-goc` | dòng `cd` thứ 5 = `# shell: cd /`; **0** dòng lỗi `cd` trong ca | PASS |
| `doi-chung` | `cd /docs` **cuối** ⇒ `pwd` ngay sau = `# shell: pwd /docs` | PASS |
| `thu-tu-cd` | chuỗi 7 dòng `cd` đúng thứ tự | PASS |
| `thu-tu-pwd` | chuỗi 5 dòng `pwd` đúng thứ tự (`/docs/sub`, `/`, `/docs`, `/`, `/docs`) | PASS |

Khối `/docs/sub` dán nguyên văn từ `kt183-logs/run-clean/qemu.log`:

```text
# shell: cd /docs/sub
shell>
# shell: pwd /docs/sub
shell>
deep
# shell: cat ok bytes=4 first=deep
shell>
# shell: cd /
shell>
# shell: pwd /
shell>
# shell: cd /docs
shell>
# shell: pwd /docs
shell>
# shell: cd /docs/sub
shell>
# shell: cd /
shell>
# shell: pwd /
shell>
# shell: cd /docs
shell>
# shell: pwd /docs
shell>
# shell: cd /
shell>
# shell: echo __END_17__
```

`ls` trong `/docs` trên ảnh mới (thấy thư mục sâu `sub`):

```text
  f note.txt
  d sub
# shell: ls n=2 path=/docs
```

## 4. Đột biến — chứng minh ca kiểm có răng

Hai đột biến **độc lập**, mỗi đột biến: sửa mã → build ảnh → chạy `shell-edge.sh --image` → kỳ vọng
ca mới **ĐỎ**; hoàn nguyên → build lại (sha **trùng** ảnh xanh trước đột biến) → chạy lại → **XANH**.

| # | Tên | Lệnh đột biến (mã sản phẩm) | Kỳ vọng | Kết quả đo |
|---|---|---|---|---|
| A | `..` không được xử lý | `shell_user.rs::resolve_path`: `if comp == b".." {` → `if comp == b".." && comp.len() > 2 {` (điều kiện **không bao giờ** đúng ⇒ `..` thành thành phần thường, gửi nguyên xuống FS) | ca mới **ĐỎ** | **ĐỎ 5/8**: `cd-hai-cap`, `cd-chuan-hoa`, `cd-kep-goc`, `thu-tu-cd`, `thu-tu-pwd` FAIL; `cd-sub`/`cat-deep`/`doi-chung` PASS. Ảnh `6e477524…`; log `kt183-logs/kt183-shell-edge-mut-dotdot.log` |
| A' | hoàn nguyên A | `cp` bản gốc `shell_user.rs` | ca mới **XANH** | **XANH 8/8**; ảnh rebuild `sha256=9e2b7849…` **trùng** ảnh xanh trước đột biến; log `kt183-logs/kt183-shell-edge-revert-green.log` |
| B | `pwd` in hằng `/` | `shell_user.rs::cmd_pwd`: `put(cwd.as_slice());` → `put(b"/");` | ca mới **ĐỎ** | **ĐỎ 4/8**: `cd-sub`, `cd-chuan-hoa`, `doi-chung`, `thu-tu-pwd` FAIL; `cat-deep`/`cd-hai-cap`/`cd-kep-goc`/`thu-tu-cd` PASS. Ảnh `caeb2b50…`; log `kt183-logs/kt183-shell-edge-mut-pwd.log` |
| B' | hoàn nguyên B | `cp` bản gốc `shell_user.rs` | ca mới **XANH** | **XANH 8/8**; ảnh rebuild `sha256=9e2b7849…` **trùng** ảnh xanh trước đột biến; log `kt183-logs/kt183-shell-edge-revert-green.log` |

Trích dòng phán quyết đỏ (đột biến A):

```text
# kt183-cd-sau[cd-hai-cap]: FAIL (dòng 'cd' thứ 2 là '# shell: cd loi code=2 /docs/sub/../..', mong '# shell: cd /')
# kt183-cd-sau[cd-chuan-hoa]: FAIL (dòng 'cd' thứ 3 là '# shell: cd loi code=2 /docs/sub/..', mong '# shell: cd /docs')
# kt183-cd-sau[cd-kep-goc]: FAIL (dòng 'cd' thứ 5 là '# shell: cd loi code=2 /docs/sub/../../..', mong '# shell: cd /' (kẹp ở gốc))
# kt183-cd-sau[thu-tu-cd]: FAIL (chuỗi dòng 'cd' lệch: thấy [# shell: cd /docs/sub|# shell: cd loi code=2 /docs/sub/../..|…])
# kt183-cd-sau[thu-tu-pwd]: FAIL (chuỗi dòng 'pwd' lệch: thấy [# shell: pwd /docs/sub|# shell: pwd /docs/sub|…])
# kt183-cd-sau: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
```

Trích dòng phán quyết đỏ (đột biến B):

```text
# kt183-cd-sau[cd-sub]: FAIL (sau 'cd /docs/sub' là '# shell: pwd /', mong '# shell: pwd /docs/sub')
# kt183-cd-sau[cd-chuan-hoa]: FAIL (sau 'cd /docs/sub/..' không phải '# shell: pwd /docs')
# kt183-cd-sau[doi-chung]: FAIL (sau 'cd /docs' là '# shell: pwd /', mong '# shell: pwd /docs')
# kt183-cd-sau[thu-tu-pwd]: FAIL (chuỗi dòng 'pwd' lệch: thấy [# shell: pwd /|# shell: pwd /|# shell: pwd /|# shell: pwd /|# shell: pwd /])
# kt183-cd-sau: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
```

**Selftest** (không cần QEMU) cũng chứng minh ba kiểu hỏng trên: log giả (a) trước KT-183 ⇒ **3 PASS
+ 5 FAIL**; (b) `..` không xử lý ⇒ **3 PASS + 5 FAIL**; (c) `pwd` hằng `/` ⇒ **4 PASS + 4 FAIL**;
log đúng ⇒ **8 PASS + 0 FAIL**. Tổng `--selftest`: **PASS 46/46** (10/10 ca).

## 5. Ảnh sản phẩm **mới** + ghim

Lệnh (đúng đường chuẩn):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" bash v2/tools/repro-build.sh
bash v2/tools/pin-artifact.sh <ảnh>
bash v2/test/boot/shell-edge.sh --image v2/artifacts/639ca6d1….bin
```

**Lượt đo chính** (build **sau** commit `4fc80f61`, cây sạch ⇒ `dirty=0`, danh tính khớp đúng HEAD):

```text
commit      4fc80f61991fe73807273d55691583d9a699edba   (rev trong ảnh: 4fc80f61991f)
image       639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81
size        213632 B
inputs      43df126a4a6702ec   dirty=0   build=b08c5c57cdbdb156   text=131202
toolchain   1.99.1-aarch64-apple-darwin (rustc 1.99.1 (58cf8f9c3 2026-10-13),
            cargo 1.99.1 (5f94df478 2026-08-27); RUSTUP_TOOLCHAIN ghim theo v2/rust-toolchain.toml)
qemu        11.1.3 (/opt/homebrew/bin/qemu-system-riscv64)
opensbi     /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin
log         kt183-logs/kt183-build-clean.log, kt183-logs/kt183-pin-clean.log,
            kt183-logs/kt183-shell-edge-clean.log, kt183-logs/run-clean/qemu.log
```

Dòng danh tính trong ảnh + dòng `# product fs:` (đổi `used=4` → `used=6` vì thêm hai mục):

```text
# build: algo=kt134-v4 inputs=43df126a4a6702ec dirty=0
# product fs: medium=ram blocks=96 used=6 total=66 volatile=1 why=khong-co-pci const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
```

Kết quả `shell-edge.sh --image` trên ảnh mới: **73 dòng PASS, 0 FAIL**; dòng cuối
`# shell-edge.sh: PASS`.

**Ảnh xanh trước commit** (cùng nội dung nguồn, `dirty=1`, dùng cho đột biến/hoàn nguyên):
`sha256=9e2b784914cfef2140f7107073aa05c8a4b46ff6f5bff47c566c9a4805e88de8`,
`inputs=89f40ef16c97e6d5 dirty=1 rev=8f5a11892500` — log `kt183-logs/kt183-build-green-precommit.log`,
`kt183-logs/kt183-shell-edge-green-precommit.log`.

## 6. Hệ quả với K6

- Đổi `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` ⇒ **dấu vân tay build-inputs đổi**
  `89f40ef16c97e6d5` → **`43df126a4a6702ec`** (định nghĩa `kt134-v4`, băm nội dung `v2/kernel` +
  `v2/lib` + tệp ghim ở HEAD `4fc80f61`).
- **Ảnh K6 cũ `b090cfe8…` và ảnh sản phẩm cũ `7044cd5b…` (cùng `inputs=89f40ef16c97e6d5`) KHÔNG
  còn là ảnh nguồn** cho cây hiện tại: `REV-GATE` sẽ **TỪ CHỐI** chúng khi so với dấu vân tay mới.
  Mọi kết quả K6 đo trên hai ảnh đó (KT-192: F1..F6) **không** tự động chuyển sang HEAD mới.
- **Việc lead phải quyết**: chạy lại K6 (hoặc tối thiểu các tiêu chí bị ảnh hưởng) trên ảnh mới
  `639ca6d1…` / HEAD `4fc80f61`. Task này **không** tự chạy `gate --fast`/K6 (theo phân công: lead
  chạy tuần tự). Đây là hệ quả **bắt buộc báo**, không được giấu.
- Ảnh mới đã **ghim** trong worktree: `v2/artifacts/639ca6d1….bin` (gitignored — không nằm trong
  commit; lead có thể copy sang checkout chính hoặc build lại bằng lệnh ở §5).

## 7. Giới hạn còn lại

- Ca mới kiểm **hai** cấp (`/docs/sub`); **chưa** kiểm cấp sâu hơn (ba cấp trở lên) hay đường dài
  gần `MAX_PATH` (48 B) — FS sản phẩm hiện chỉ seed tới hai cấp.
- `cd` vào thư mục sâu **không tồn tại** (`/docs/sub/khongco`) chưa có ca riêng; ca `kt137-cdpwd`
  đã kiểm `cd /docs/khongco` (một cấp) và `cd /khongco` (gốc).
- Đột biến B (`pwd` hằng `/`) làm **cả** `kt137-cdpwd` đỏ (đã biết từ KT-158) — đúng kỳ vọng, không
  phải hồi quy.
- Ảnh ghim nằm trong `v2/artifacts/` của **worktree** (gitignored); checkout chính
  `/Users/mps/dvchd/ViFastOs` **không** bị sửa (đúng luật task).

## 8. Log (tái lập được)

| Tệp | Nội dung |
|---|---|
| `kt183-logs/kt183-build-green-precommit.log` | build ảnh xanh trước commit (`9e2b7849…`, `dirty=1`) |
| `kt183-logs/kt183-shell-edge-green-precommit.log` | `shell-edge --image` ảnh xanh trước commit (PASS) |
| `kt183-logs/kt183-build-mut-dotdot.log` | build ảnh đột biến A (`6e477524…`) |
| `kt183-logs/kt183-shell-edge-mut-dotdot.log` | `shell-edge --image` đột biến A (**ĐỎ** — ca mới 5/8 FAIL) |
| `kt183-logs/kt183-build-mut-pwd.log` | build ảnh đột biến B (`caeb2b50…`) |
| `kt183-logs/kt183-shell-edge-mut-pwd.log` | `shell-edge --image` đột biến B (**ĐỎ** — ca mới 4/8 FAIL) |
| `kt183-logs/kt183-build-revert-green.log` | build sau hoàn nguyên (`9e2b7849…` — **trùng** ảnh xanh) |
| `kt183-logs/kt183-shell-edge-revert-green.log` | `shell-edge --image` sau hoàn nguyên (**XANH**) |
| `kt183-logs/kt183-build-clean.log` | build ảnh chính sau commit (`639ca6d1…`, `dirty=0`) |
| `kt183-logs/kt183-pin-clean.log` | `pin-artifact.sh` ảnh chính |
| `kt183-logs/kt183-shell-edge-clean.log` | `shell-edge --image` ảnh chính (**PASS**, 73 PASS / 0 FAIL) |
| `kt183-logs/run-clean/qemu.log` | log QEMU đầy đủ của lượt chính |
| `kt183-logs/kt183-v2-check-precommit.log` | `./v2/bin/v2 check` trên cây trước commit (**PASS**) |
| `kt183-logs/kt183-v2-check-final.log` | `./v2/bin/v2 check` trên HEAD cuối (**PASS**) |
