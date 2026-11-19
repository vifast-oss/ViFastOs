# KT-204 — Review **độc lập** KT-183 (`cd` vào thư mục con **sâu**)

**Ngày**: 2026-11-19/20 · **Task**: KT-204 (review KT-183) · **Nhánh**: `night3/L42-review183`
(tách từ `cd74d73e` — HEAD của tác giả) · **Worktree**: `/private/tmp/night3-L42-review183`.

**Vai**: reviewer độc lập (L42), **không** phải tác giả. Mọi số dưới đây **đo lại từ đầu** trên
worktree này; log của tác giả chỉ dùng để **đối chiếu**, không dùng làm bằng chứng.

**Phạm vi**: chỉ đọc/đo. **Không** sửa `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`. **Không**
sửa mã sản phẩm để vá; các đột biến dưới đây được **hoàn nguyên** (cây sạch lại đúng
`c59c1984…` của `shell_user.rs`, build lại trùng sha).

---

## 1. Kết luận ngắn

**ĐẠT CÓ ĐIỀU KIỆN.**

Phần **lõi** của KT-183 đúng và tái lập được **byte-for-byte**:

- `seed()` thêm `/docs/sub` (thư mục) + `/docs/sub/deep.txt` (4 B `deep`) đúng thứ tự "thư mục
  trước, tệp sau", `fsync` như cũ, `Exists` ⇒ bỏ qua; **không** đổi `TOTAL_BLOCKS`.
- Ca mới `kt183-cd-sau` (13 lệnh, 8 phép kiểm) **có răng thật**: đột biến A/B của tác giả
  (tái lập độc lập) và **hai** đột biến riêng của reviewer (M1, M3) đều làm ca **ĐỎ** đúng vế.
- Cây sạch: **18/18 khối PASS**, ca mới **8/8**, **73 dòng PASS / 0 FAIL**; selftest **46/46**.
- Ảnh build lại từ HEAD `cd74d73e` cho **đúng** `sha256=639ca6d1…`, `inputs=43df126a4a6702ec`,
  `dirty=0` — **trùng** số tác giả báo.

**Điều kiện** (không chặn phần lõi nhưng phải xử lý trước khi coi KT-183 DONE):

1. **Lỗ hổng ca kiểm M2** — `cd` có `/` cuối mà **không** cập nhật `cwd`: đột biến đi qua **toàn
   bộ** harness (ca mới 8/8 XANH, `kt137-cdpwd` cũng XANH). Ca mới **không** có lệnh `cd /docs/sub/`
   (có `/` cuối) nên không ghim vế này.
2. **Lỗ hổng ca kiểm M4** — off-by-one ở biên `MAX_PATH` 48 B (`>` → `>=`): đột biến đi qua **toàn
   bộ** harness. Ca mới không có đường dài gần 48 B nên không ghim biên.
3. **Journal tác giả có hai chỗ sai số** (mục §7): (a) "rev trong ảnh" — ảnh **không** nhúng `rev`
   (chỉ `algo/inputs/dirty`); (b) trích `kt183-logs/run-clean/qemu.log` nhưng tệp **không có**
   trong nhánh.
4. **Hệ quả K6 phải nói chính xác**: `pre-measure.sh` **TỪ CHỐI** ảnh cũ `b090cfe8…`
   (`inputs-mismatch`, exit 4) — đúng; nhưng `shell-edge.sh --check-image` **NHẬN** ảnh cũ (chỉ
   kiểm `algo=`, không kiểm `inputs=`) và `pin-artifact.sh --verify` cũng **PASS**. Câu "REV-GATE
   sẽ TỪ CHỐI" trong journal là **quá rộng**.

**Không chặn**: M5/M6 (ca mới không bắt `cd` vào tệp / đường không có, nhưng `kt137-cdpwd` **bắt**
— không phải lỗ hổng toàn harness); `v2 check` đỏ ở bước `rules` là **T-INFRA** (SIGTRAP bash 3.2,
xem §2.3), không phải lỗi mã.

---

## 2. Tái lập độc lập

### 2.1 Ảnh build lại (A1)

Lệnh (đúng đường chuẩn, **không** dùng `target/` của repo):

```bash
export VIFAST_ROLE=lead
export PATH="$HOME/.cargo/bin:$PATH"
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
  VIFAST_REPRO_LOG=/tmp/l42-build-cargo.log bash v2/tools/repro-build.sh
```

Kết quả (log `kt204-logs/kt204-build-clean.log`):

```text
REPRO machine=Darwin-arm64 root=/private/tmp/night3-L42-review183
      sha256=639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81
      size=213632 wall_s=18 algo=kt134-v4 inputs=43df126a4a6702ec dirty=0
      rev=cd74d73ed0d1 build=b08c5c57cdbdb156 text=131202
```

- **sha256 trùng** `639ca6d1…` tác giả báo. **size trùng** 213632. **inputs trùng**
  `43df126a4a6702ec`. `dirty=0`.
- `rev=cd74d73ed0d1` (HEAD của worktree review) **khác** `4fc80f61991f` trong log tác giả — **không
  phải** sai: `rev` **không** được nhúng vào ảnh (xem §7.1), nên cùng nội dung nguồn ⇒ cùng sha
  bất kể rev. Đã kiểm: `strings` ảnh chỉ có `# build: algo=… inputs=… dirty=…`, **không** có `rev=`.
- **Đối chứng dương sau khi hoàn nguyên toàn bộ đột biến**: build lại lần hai từ cây sạch
  (`shell_user.rs` sha `c59c1984…`) cho **đúng** `sha256=639ca6d1…`, `dirty=0` — cây tất định,
  không sót thay đổi.

### 2.2 `shell-edge.sh` trên ảnh tái lập (A2)

```bash
SHELL_EDGE_LOGDIR=/tmp/l42-logs-clean \
  bash v2/test/boot/shell-edge.sh --image /tmp/l42-image-clean.bin
```

Kết quả (log `kt204-logs/kt204-shell-edge-clean.log`, QEMU đầy đủ
`kt204-logs/kt204-qemu-clean.log`):

- **18/18 khối PASS**: 8 ca + `truncate-jobs` + `new-cmds-ok` + `new-cmds-edge` +
  `new-cmds-edge2` + `new-cmds-edge3` + `k7-uu-tien` + `k7-thu-muc` + `kt137-cdpwd` +
  `kt183-cd-sau`.
- **73 dòng PASS / 0 FAIL**; dòng cuối `# shell-edge.sh: PASS`.
- Ca `kt183-cd-sau`: **8/8** (`cd-sub`, `cat-deep`, `cd-hai-cap`, `cd-chuan-hoa`, `cd-kep-goc`,
  `doi-chung`, `thu-tu-cd`, `thu-tu-pwd`).
- Khối `/docs/sub` trong QEMU log **trùng nguyên văn** khối tác giả dán ở journal §3.
- `--selftest`: **PASS 46/46** (10/10 ca; 26 sai→ĐỎ + 20 đúng→XANH) — log
  `kt204-logs/kt204-selftest.log`.

### 2.3 `./v2/bin/v2 check` (A3)

**FAIL** — nhưng là **T-INFRA**, không phải lỗi mã:

```text
--- fmt: OK
--- clippy: OK
--- luật cấu trúc R1..R5
./v2/bin/v2: line 255: 49547 Trace/BPT trap: 5       ( rules "$V2_ROOT" )
--- luật cấu trúc: FAIL
--- self-test luật: OK
--- luật máy K6 (KT-80: 1,2,3,7,8): OK
--- selftest build-inputs: OK
--- selftest danh tính ảnh: OK
--- selftest t26: OK
--- ghim thuật toán dấu vân tay: OK
--- selftest ghim thuật toán: OK
--- selftest shell-edge: OK
--- check-wake-paths: OK
--- build (L0 compile): OK
v2 check: 1 bước FAIL
```

- Chạy **hai lần**, **cả hai** chết đúng bước `rules` bằng SIGTRAP (`Trace/BPT trap: 5`) — đúng
  lớp **T-INFRA KT-155** đã ghi trong `v2/bin/v2` (bash 3.2 hỏng heap sau khi `rules` quét cây;
  workaround subshell **không** đủ trên máy này; máy đang bận (loadavg 1m 7.48 đo ở lượt
  `shell-edge` chạy song song)).
- **Tách riêng** hàm `rules` ra bash sạch: `RULES_EXIT=0`, **không** in dòng lỗi — luật cấu trúc
  **PASS** khi không bị hỏng heap.
- Log tác giả `kt183-v2-check-final.log` (HEAD `febba904`) **PASS** toàn bộ; diff `febba904..cd74d73e`
  chỉ là journal/log (không đổi mã), nên **không** có lý do mã để đỏ.
- **Kết luận**: `v2 check` **không** đạt trên máy review vì T-INFRA (12/13 bước còn lại **OK**);
  **không** quy cho KT-183. Log: `kt204-logs/kt204-v2-check-run1.log`,
  `kt204-v2-check-run2.log`.

---

## 3. Đột biến của reviewer (B4)

Mỗi đột biến: sửa `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` → build ảnh bằng
`repro-build.sh` → chạy **toàn bộ** `shell-edge.sh --image` → ghi ĐỎ/XANH + ca bắt được → hoàn
nguyên (`git checkout --`). Ảnh đột biến có `dirty=1` (đúng: cây bẩn), sha riêng.

| # | Tên | Sửa (mã sản phẩm) | Kỳ vọng | Kết quả đo | Ca bắt được |
|---|---|---|---|---|---|
| **M1** | `..` lùi **hai** thành phần (sai một cấp) | `resolve_path`: bọc khối lùi trong `for _ in 0..2` | ĐỎ | **ĐỎ 3/8** | `cd-chuan-hoa`, `thu-tu-cd`, `thu-tu-pwd` |
| **M2** | `cd` có `/` cuối **không** cập nhật `cwd` | `cmd_cd`: nếu `arg` kết thúc `/` và `len>1` ⇒ in `# shell: cd <path>` rồi `return` (không `cwd.set`) | ĐỎ | **XANH 8/8** (kt137 cũng XANH) | **KHÔNG — lỗ hổng** |
| **M3** | `pwd` in đường **chưa chuẩn hoá** | `cmd_cd`: `cwd.set(&out[..len])` → `cwd.set(arg thô)` | ĐỎ | **ĐỎ 3/8** | `cd-hai-cap`, `cd-chuan-hoa`, `thu-tu-pwd` (+ `kt137-cdpwd` ĐỎ) |
| **M4** | biên `MAX_PATH` off-by-one | `resolve_path`: `len+sep+comp.len() > CWD_MAX` → `>=` | ĐỎ | **XANH 8/8** (kt137 cũng XANH) | **KHÔNG — lỗ hổng** |
| **M5** | `cd` vào **tệp** được chấp nhận | `cmd_cd`: `if kind != KIND_DIR` → `if false && …` | ĐỎ | **XANH 8/8**; `kt137-cdpwd` **ĐỎ** (`cd-tep`, `pwd`, `cd-thu-tu`) | ca mới **không**; ca cũ **có** |
| **M6** | `cd` vào đường **không có** được chấp nhận | `cmd_cd`: `if status != ST_OK` → `if false && …` | ĐỎ | **XANH 8/8**; `kt137-cdpwd` **ĐỎ** (`cd-khong-co`, `cd-thu-tu`) | ca mới **không**; ca cũ **có** |
| **A** | (tác giả) `..` **không** xử lý | `if comp == b".."` → `if comp == b".." && comp.len() > 2` | ĐỎ 5/8 | **ĐỎ 5/8** — **trùng** tác giả | `cd-hai-cap`, `cd-chuan-hoa`, `cd-kep-goc`, `thu-tu-cd`, `thu-tu-pwd` |
| **B** | (tác giả) `pwd` in hằng `/` | `cmd_pwd`: `put(cwd.as_slice())` → `put(b"/")` | ĐỎ 4/8 | **ĐỎ 4/8** — **trùng** tác giả | `cd-sub`, `cd-chuan-hoa`, `doi-chung`, `thu-tu-pwd` |

**Xác nhận đột biến M2/M4 là thật** (không phải no-op) bằng probe tay trên ảnh đột biến:

- M2: `cd /docs/sub/` in `# shell: cd /docs/sub` nhưng `pwd` in `/` (cwd **không** đổi) — log
  `kt204-logs/kt204-probe-m2.log`.
- M4: `cd /docs/sub/` + 38×`a` (đúng 48 B) in `# shell: duong qua dai` thay vì `cd loi code=11`
  (ảnh sạch) — log `kt204-logs/kt204-probe-m4.log`.

**Đối chứng dương (B5)**: cây sạch (sau hoàn nguyên) chạy lại **XANH toàn bộ** — 18/18 khối,
73 PASS / 0 FAIL; build lại **trùng** `639ca6d1…`. Không báo oan.

---

## 4. Lỗ hổng ca kiểm (kết luận từ §3)

1. **M2 — `cd` có `/` cuối**: ca mới **không** có lệnh nào kết thúc bằng `/` (trừ `cd /`), nên
   đột biến "in ra nhưng không đổi `cwd`" đi qua **cả** ca mới **và** `kt137-cdpwd`. Đây là lỗ
   hổng **toàn harness**, không chỉ ca mới. Đề nghị thêm `cd /docs/sub/` + `pwd` (kỳ vọng
   `/docs/sub`) vào ca 17 — một lệnh, một phép kiểm.
2. **M4 — biên `MAX_PATH` 48 B**: không ca nào dùng đường dài gần 48 B. Đột biến `>` → `>=` (từ
   chối oan đường đúng 48 B) đi qua toàn harness. Đề nghị thêm một lệnh `cd` với đường 48 B
   (kỳ vọng `cd loi code=11` do tên thành phần > `MAX_NAME`, **không** phải `duong qua dai`) và
   một lệnh 49 B (kỳ vọng `duong qua dai`).
3. **M5/M6 — không phải lỗ hổng toàn harness**: ca mới không kiểm `cd` vào tệp / đường không có,
   nhưng `kt137-cdpwd` **bắt** cả hai. Ghi nhận là **khoảng trống của ca mới**, không phải lỗi
   chặn.

---

## 5. Kiểm phạm vi và tính đúng (C6–C9)

### 5.1 `ok_kt137_ls_docs` — **mạnh hơn**, không nới ngưỡng (C6)

Diff: `ls n=1 path=/docs` → `ls n=2 path=/docs` (bắt buộc: `/docs` nay có **hai** mục
`note.txt` + `sub`), **thêm** vế `  d sub` ×2. Vế mới **chứng minh thư mục sâu hiện ra trong bảng
`ls` của thư mục cha** — **mạnh hơn** bản cũ (bản cũ chỉ đếm `note.txt`). Không phép kiểm nào bị
bỏ/nới. Selftest ca 16 cập nhật log giả "đúng" theo; vẫn PASS.

Các ca cũ khác **không** bị ảnh hưởng: `ok_ls_root` (new-cmds-ok) và `ok_kt137_ls_cwd` dùng regex
`n=[0-9]+` (không ghim số), gốc `/` không đổi số mục (`/docs/sub` nằm **trong** `/docs`). `t26.sh`
chỉ đòi chuỗi `# product fs: medium=`, **không** khẳng định `used=`; `used=4 → used=6` không làm
đỏ ca nào (đã rà `v2/test`, `v2/tools`).

### 5.2 `DOCS_DEEP` + `deep.txt` (C7)

`const DOCS_DEEP: &[u8] = b"deep";` — **đúng 4 byte**, không `\n` cuối (cùng quy ước
`DOCS_NOTE`). Ảnh sạch: `cat deep.txt` in đúng một dòng `deep` + `# shell: cat ok bytes=4
first=deep`. Khớp.

### 5.3 Kẹp gốc (C8)

- `cd ../../..` từ `/docs/sub` ⇒ `# shell: cd /`, `pwd` `/` — **kẹp** ở gốc, không âm, không lỗi.
- Probe thêm **5 cấp** `cd ../../../../..` từ `/docs/sub` ⇒ `/`, `pwd` `/` — **không** tràn/panic.
- Probe `cd /docs/sub/../..` ⇒ `/`; `cd /docs/sub/deep.txt/..` ⇒ `/docs/sub`;
  `cd /docs/sub/deep.txt/../..` ⇒ `/docs` — chuẩn hoá **từ vựng** đúng.

### 5.4 Đường tương đối sau khi thêm cấp (C9)

`cd /docs/sub` rồi `cat ../note.txt` ⇒ `ghi chu cua docs` + `# shell: cat ok bytes=16 first=ghi chu `
— đọc đúng `/docs/note.txt`. Khớp.

### 5.5 Thử biên thêm (probe tay trên ảnh sạch — `kt204-logs/kt204-probe-clean.log`)

| Lệnh | Kết quả | Đánh giá |
|---|---|---|
| `cd /docs/sub/deep.txt` (tệp) | `# shell: cd khong phai thu muc /docs/sub/deep.txt` | đúng |
| `cd /docs/sub/khongco` | `# shell: cd khong co /docs/sub/khongco` | đúng |
| `cd /docs/sub/` ( `/` cuối) | `# shell: cd /docs/sub`, `pwd` `/docs/sub` | đúng (nhưng **không** được ghim — M2) |
| `cd /docs//sub` | `# shell: cd /docs/sub` | đúng |
| `cd /docs/./sub` | `# shell: cd /docs/sub` | đúng |
| `cd /docs/sub/../sub` | `# shell: cd /docs/sub` | đúng |
| `cd /docs/sub/deep.txt/x` | `# shell: cd loi code=7` (NotDir) | đúng |
| `cd /docs/sub/` + 38×`a` (48 B) | `# shell: cd loi code=11` (NameTooLong) | **không** tràn/panic; đúng mã |
| `cd /docs/sub/` + 39×`a` (49 B) | `# shell: duong qua dai` | đúng (shell chặn sớm) |

**Không** phát hiện tràn/panic ở biên `MAX_PATH` 48 B trên mã sạch.

---

## 6. Hệ quả K6 (D10–D11)

### 6.1 Vân tay build-inputs **thật sự** đổi (D10)

`./v2/bin/v2 status` trong worktree (`kt204-logs/kt204-status.log`):

```text
HEAD        cd74d73ed0d1 (night3/L42-review183)  chưa push: 4
build-inputs 233 mục  dấu vân tay: 43df126a4a6702ec  dirty: 0
```

**Đúng** `43df126a4a6702ec` như tác giả báo (cũ `89f40ef16c97e6d5`). Dòng `REPRO` của build cũng
in `inputs=43df126a4a6702ec`. **Xác nhận.**

### 6.2 Ảnh K6 cũ `b090cfe8…` **không** còn khớp (D11)

Ảnh cũ (đọc từ checkout chính, **không** sửa):
`/Users/mps/dvchd/ViFastOs/v2/artifacts/b090cfe8….bin`, `# build: algo=kt134-v4
inputs=89f40ef16c97e6d5 dirty=0`.

| Đường kiểm | Kết quả | Ý nghĩa |
|---|---|---|
| `pre-measure.sh <ảnh cũ>` | **REFUSE** `reason=inputs-mismatch`, exit **4** | **Đường K6 TỪ CHỐI** — đúng hệ quả |
| `shell-edge.sh --check-image <ảnh cũ>` | **ACCEPT** `inputs=89f40ef16c97e6d5`, exit 0 | **KHÔNG** từ chối — chỉ kiểm `algo=` |
| `pin-artifact.sh --verify --dir v2/artifacts` | `VERIFY: ok=31 bad=0 legacy=4`, exit 0 | **KHÔNG** phát hiện lệch vân tay |

**Kết luận**: hệ quả K6 là **thật** — mọi kết quả K6 đo trên `b090cfe8…`/`7044cd5b…` **không**
chuyển sang HEAD mới; `pre-measure.sh` sẽ chặn. **Nhưng** câu trong journal tác giả "REV-GATE sẽ
TỪ CHỐI chúng" chỉ đúng với `pre-measure.sh`; `shell-edge.sh --check-image` và
`pin-artifact.sh --verify` **vẫn nhận** ảnh cũ (chúng chỉ kiểm định nghĩa `algo=`/tên tệp). Cần
ghi rõ để người sau không tưởng đã được chặn ở mọi cửa.

---

## 7. Phát hiện khác (journal tác giả)

### 7.1 "rev trong ảnh" — **sai**

Journal §5 ghi `commit 4fc80f61… (rev trong ảnh: 4fc80f61991f)`. Ảnh **không** nhúng `rev`:
`strings` ảnh chỉ có `# build: algo=kt134-v4 inputs=43df126a4a6702ec dirty=0`; `build.rs` **có**
đặt `cargo:rustc-env=VIFAST_BUILD_REV`, nhưng `main.rs` chỉ dùng `ALGO/INPUTS/DIRTY` để in dòng
`# build:` — `REV` **không** được tham chiếu nên không vào ảnh (rev chỉ in ở dòng `REPRO` của
**log build**). Bằng
chứng chéo: build từ `4fc80f61`, `0118d1ea`, `cd74d73e` cho **cùng** sha `639ca6d1…` dù `rev`
khác — nếu rev nằm trong ảnh thì sha **phải** khác. Sửa: bỏ chữ "trong ảnh", ghi "rev của lượt
build".

### 7.2 `kt183-logs/run-clean/qemu.log` — **thiếu**

Journal §3 ("dán nguyên văn từ `kt183-logs/run-clean/qemu.log`") và §8 (bảng log) trích tệp này,
nhưng **không có** trong nhánh (`ls v2/agent/JOURNAL/kt183-logs/` không có `run-clean/`;
`git ls-files` cũng không). Khối trích trong journal **khớp** đầu ra thật (reviewer đã tái lập),
nên nội dung không sai — nhưng **bằng chứng gốc** thiếu. Đề nghị bổ sung tệp hoặc bỏ tham chiếu.
(Reviewer đã lưu QEMU log đầy đủ của lượt tái lập: `kt204-logs/kt204-qemu-clean.log`.)

### 7.3 `v2 check` — T-INFRA, không phải mã

Xem §2.3. Không quy cho KT-183; nhưng nếu lead cần "v2 check PASS" trên HEAD này thì phải chạy
trên máy không bị hỏng heap bash 3.2 (hoặc chờ load thấp), **không** được coi là nới ngưỡng.

---

## 8. Log (tái lập được)

| Tệp | Nội dung |
|---|---|
| `kt204-logs/kt204-build-clean.log` | build tái lập từ HEAD `cd74d73e` — sha `639ca6d1…`, `inputs=43df126a4a6702ec`, `dirty=0` |
| `kt204-logs/kt204-shell-edge-clean.log` | `shell-edge --image` ảnh tái lập — **18/18 khối, 73 PASS / 0 FAIL** |
| `kt204-logs/kt204-qemu-clean.log` | QEMU log đầy đủ của lượt tái lập (khối `/docs/sub` nguyên văn) |
| `kt204-logs/kt204-selftest.log` | `--selftest` — **PASS 46/46** (10/10 ca) |
| `kt204-logs/kt204-v2-check-run1.log`, `…run2.log` | `./v2/bin/v2 check` — **FAIL** bước `rules` (SIGTRAP, T-INFRA), 11 bước còn lại OK |
| `kt204-logs/kt204-probe-clean.log` | probe tay trên ảnh sạch (tệp/không có/`/` cuối/`//`/`.`/48 B/49 B/5 cấp) |
| `kt204-logs/kt204-mut-m1-*.log` | đột biến M1 (`..` lùi hai cấp) — **ĐỎ 3/8** |
| `kt204-logs/kt204-mut-m2-*.log`, `kt204-probe-m2.log` | đột biến M2 (`/` cuối không đổi cwd) — **XANH 8/8** ⇒ **lỗ hổng** |
| `kt204-logs/kt204-mut-m3-*.log` | đột biến M3 (`pwd` chưa chuẩn hoá) — **ĐỎ 3/8** |
| `kt204-logs/kt204-mut-m4-*.log`, `kt204-probe-m4.log` | đột biến M4 (biên 48 B off-by-one) — **XANH 8/8** ⇒ **lỗ hổng** |
| `kt204-logs/kt204-mut-m5-*.log` | đột biến M5 (`cd` vào tệp) — ca mới XANH, `kt137-cdpwd` ĐỎ |
| `kt204-logs/kt204-mut-m6-*.log` | đột biến M6 (`cd` đường không có) — ca mới XANH, `kt137-cdpwd` ĐỎ |
| `kt204-logs/kt204-mut-a-*.log` | tái lập đột biến A của tác giả — **ĐỎ 5/8** (trùng) |
| `kt204-logs/kt204-mut-b-*.log` | tái lập đột biến B của tác giả — **ĐỎ 4/8** (trùng) |
| `kt204-logs/kt204-pre-measure-old-k6.log` | `pre-measure.sh` ảnh K6 cũ — **REFUSE inputs-mismatch** (exit 4) |
| `kt204-logs/kt204-check-image-old-k6.log` | `shell-edge --check-image` ảnh cũ — **ACCEPT** (chỉ kiểm `algo=`) |
| `kt204-logs/kt204-pin-verify.log` | `pin-artifact.sh --verify` — `ok=31 bad=0 legacy=4` (không bắt lệch vân tay) |
| `kt204-logs/kt204-status.log` | `./v2/bin/v2 status` — vân tay `43df126a4a6702ec`, `dirty=0` |

**Cách dựng lại một đột biến** (ví dụ M2):

```bash
cd /private/tmp/night3-L42-review183
export VIFAST_ROLE=lead PATH="$HOME/.cargo/bin:$PATH"
# sửa shell_user.rs theo bảng §3 (M2: chèn khối "nếu arg kết thúc '/' thì in rồi return")
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
  bash v2/tools/repro-build.sh            # lấy image=… ở dòng REPRO
SHELL_EDGE_LOGDIR=/tmp/l42-logs-m2 \
  bash v2/test/boot/shell-edge.sh --image <image>
git checkout -- v2/kernel/vifast-kbin-riscv64/init/shell_user.rs   # hoàn nguyên
```
