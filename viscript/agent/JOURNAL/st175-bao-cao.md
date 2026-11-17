# ST-175 — Báo cáo: cây `ps` bị xoá hàng nghìn tệp tracked + chặn cây bẩn (VD-177 §4)

Ngày: 2026-11-17 (JST) · Agent 2 dòng ViScript · Ngân sách 1 h (dừng mềm 17:20) · **Không** commit/push
(hook `vifast-guard`): bàn giao cho **Agent 1 (lead)** commit.

Phạm vi: **Phần 1 chỉ ĐỌC** — không `git restore`, không `git clean`, không xoá gì trên `ps`;
**Phần 2** sửa `viscript/bin/vis-remote` + 3 dòng tài liệu; **Phần 3** cổng mac.

Bằng chứng (tất cả trong `viscript/agent/JOURNAL/`):
`st175-dieutra-cay-ps.log` (điều tra), `st175-chan-cay-ban.log` (cây bẩn ⇒ từ chối), `st175-cho-phep-cay-ban.log`
(`--allow-dirty`), `st175-kiem-che-do.log` (kiểm 11 chế độ), `st175-gate-macos.log` (cổng; lần chạy bị huỷ:
`st175-gate-macos-lan1-huy.log`, có ghi lý do).

## 0. Kết luận ngắn

1. `[ĐO]` Cây `ps` hỏng **ở cấp repo**: **2297/4575** tệp tracked bị xoá khỏi đĩa (**50,2 %**), nhưng
   **`viscript/` KHÔNG bị xoá tệp nào** (0 tệp thiếu) — toàn bộ tệp bị xoá nằm ngoài dòng ViScript.
2. `[ĐO]` **Cả ba** lần gate `ps` (ST-161, ST-166, ST-167) chạy trên **đúng cây bẩn** `/root/vifast-linux`,
   **không** trên clone/worktree sạch. `PS_TREE=092fd5ab` của ST-167 là hash **của commit**
   (`git rev-parse HEAD:viscript` của `795df046`), **không** phải hash **cây làm việc** ⇒ log **không**
   chứng minh được “đo đúng mã của commit”. **Đây là khe hở bằng chứng cần ghi vào R29** (§2).
3. `[ĐO]` `bin/vis-remote` giờ **từ chối** chạy trên cây bẩn (`[exit=6]`, in số mục + 5 mục đầu + gợi ý);
   `--allow-dirty` **chỉ để chẩn đoán**. Cổng mac sau khi sửa: **PASS 10/10**, L0 **1184/0**, `[exit=0]`.

## 1. Phần 1 — điều tra (chỉ đọc)

### 1.1 Danh tính + cây nguồn `[ĐO]`

| Mục | Giá trị |
|---|---|
| Host / user | `ps` / **`root`** (uid 0) — `VIS_REMOTE_PS_SSH` mặc định `root@ps` |
| Repo | **`/root/vifast-linux`** (top-level git, remote `https://github.com/vifast-oss/ViFastOs.git`) |
| HEAD lúc điều tra | **`898c6de8`** (`898c6de820f79190ab01544462de71facf62d76a`, `main`, 14:24 JST 17/11) |
| Tệp tracked trong HEAD | **4575** |
| `git status --short \| wc -l` | **2338** = **2297 ` D`** + **25 ` M`** + **16 `??`** |

### 1.2 Tệp bị XOÁ theo thư mục cấp 1 `[ĐO]` (cộng đúng 2297)

| Thư mục cấp 1 | tracked trong HEAD | **bị xoá** |
|---|---:|---:|
| `v2` | 1915 | **1780** |
| `docs` | 284 | **281** |
| `crates` | 93 | **93** |
| `review` | 29 | 29 |
| `legacy` | 27 | 27 |
| `scripts` | 26 | 26 |
| `deploy` | 24 | 24 |
| `python-model` | 19 | 19 |
| `resources` | 7 | 7 |
| `spec` | 2 | 1 |
| `skills` | 1 | 1 |
| tệp ở gốc (9 tệp) | 9 | 9 |
| **`viscript`** | 2128 | **0** |
| `vis-host-linux`, `vis-host-wasm`, `vis`, `vis-dev`, `AGENTS.md` | 5+3+1+1+1 | **0** |
| **Tổng** | **4575** | **2297** |

9 tệp gốc bị xoá: `.cargo/config.toml`, `.gitignore`, `Cargo.lock`, `Cargo.toml`, `LICENSE-APACHE`,
`LICENSE-MIT`, `README.md`, `TRADEMARKS.md`, `ViFastOs-architecture-summary.txt`.
**16 mục ở gốc mất hẳn** (`.cargo`, `Cargo.*`, `crates/`, `deploy/`, `legacy/`, `python-model/`,
`resources/`, `review/`, `scripts/`, `skills/`, …); `docs/` chỉ còn `docs/viscript/`, `spec/` chỉ còn
`spec/v1-parity.toml`, `v2/` còn 135 tệp.
25 tệp ` M`: **22** thuộc dòng OS (`v2/kernel`, `v2/agent`, `spec/v1-parity.toml`), **3** thuộc dòng ViScript
(`viscript/agent/HOST-BAO-DAM.md`, `viscript/crates/vis-cli/src/main.rs`, `…/src/quyen.rs`).

### 1.3 Ba câu hỏi

**(Q1) Đường dẫn repo / user / HEAD?** `/root/vifast-linux`, user **root**, HEAD **`898c6de8`** (bảng 1.1).

**(Q2) `git status --short | wc -l` và đếm theo thư mục cấp 1?** **2338** mục = **2297 xoá + 25 sửa +
16 untracked** (50,2 % tệp tracked bị xoá); phân bố xoá ở bảng 1.2; **`viscript/` = 0 tệp bị xoá**.
Ghi chú phân biệt: mục “1” của `vis-host-linux`/`vis-host-wasm` trong bảng đếm *mọi mục bẩn* là tệp
`target/` **untracked** (vì `.gitignore` ở gốc đã bị xoá), **không** phải tệp bị xoá.

**(Q3) Ba lần gate `ps` chạy trên cây nào; tree hash `viscript` có khớp commit trong log?**

| Lần | Log | Thời điểm (UTC) | HEAD | `git status` trong header (VD-136) | Tree `viscript` **ghi trong log** | `HEAD:viscript` tính lại |
|---|---|---|---|---|---|---|
| ST-161 | `st161-gate-ps.log` | 11-17T01:08:29Z | `0df923c2` | 6 mục ` D` ở gốc, untracked 4 | **KHÔNG GHI** | `928ea71f93b4…` |
| ST-166 | `st166-do-truoc-ps.log`, `st166-sau-ps.log` | 11-17T03:32:50Z / 03:34:08Z | `6f8f63a5` | như trên, untracked 8 | **KHÔNG GHI** | `1d346dd90f26…` |
| ST-167 | `st167-soak-ps.log` (+`…-header.log`, `st167-ps-kiem-cuoi.log`) | 11-17T04:30:44Z | `795df046` | như trên, untracked 3 | `PS_TREE=[092fd5ab…]` (lấy bằng `git rev-parse HEAD:viscript`) | `092fd5ab25897…` |

- **Cây:** cả ba đều là `/root/vifast-linux` **và đã bẩn ngay từ ST-161** (header ghi ` D .cargo/config.toml| …`)
  ⇒ **không** lần nào là clone/worktree sạch (đúng yêu cầu VD-177 §4 chưa được thoả trước ST-175).
- **Khớp commit?** ST-161/166 **không ghi** tree hash ⇒ **không thể đối chiếu** (chỉ tính lại được hash
  *của commit*: `928ea71f…`, `1d346dd9…`). ST-167 ghi `PS_TREE=092fd5ab…` **khớp** `795df046:viscript`
  — nhưng đó là hash **của commit**, **không** phải cây làm việc (xem §2).
- **Phần được chứng minh:** 2297 tệp bị xoá **không** nằm trong `viscript/`. Hai khoá: (i) hiện tại
  `git ls-files -d -- viscript` = **0** và chưa từng có lần `git restore` nào được chạy; (ii) `mtime` thư mục
  `viscript/` = 2026-11-16 10:17 JST, **cũ hơn** ST-161 ⇒ không tệp nào của `viscript/` bị xoá ở bất kỳ đợt nào
  ⇒ đợt xoá **không** làm thiếu tệp nguồn của dòng ViScript.

### 1.4 Bằng chứng thô `[ĐO]`

| Lệnh | Log trên `ps` (`/root/vislogs/`) | sha256(log trên `ps`) | exit |
|---|---|---|---|
| điều tra A | `vis-remote-20261117T061016Z.log` | `081417f0d89ce5f18ff8f2ee128ed4a742ad6fdaa55d44612f7f8c0de6822620` | 0 |
| điều tra B | `vis-remote-20261117T061118Z.log` | `2f5115462369b56f74722e30ba90059a91827769e0ad6cb653d01969239cbe2d` | 0 |
| điều tra D | (không đọc sha) | `91dec59746e719feed26ace97bfcf07192c2f14dda51dba212b809305eb6b76b` | 0 |
| chặn cây bẩn | `vis-remote-20261117T061612Z.log` | `af2fd00ce7bf147d58a128dbb5b9c46aaa8b8375e328e478856e7a119337b29d` | **6** |
| `--allow-dirty` | `vis-remote-20261117T061617Z.log` | `2956f9aba0a5a946c2937e9c5ac3ba711e9d5ac774551c71f56a26d7d1a711b3` | 0 |

## 2. Khe hở bằng chứng — đề nghị ghi vào **R29**

`[ĐO]` `git rev-parse HEAD:viscript` trả hash **của cây trong commit**, không liên quan tới tệp đang nằm
trên đĩa. Ba log gate `ps` chỉ ghi `HEAD` (+ ST-167 ghi `HEAD:viscript`) ⇒ **không** có khoá nào ràng buộc
“mã đã được đo” với “mã trong commit”.

`[ĐO]` Bằng chứng cây `ps` là **bản sao cây làm việc của mac**, không phải bản của commit: `bin/vis-linux sync`
rsync `SYNC_DIRS`/`SYNC_FILES` (`viscript`, `docs/viscript`, `vis-host-linux`, `vis-host-wasm`, `.git`,
`AGENTS.md`, `vis-dev`, `vis`) sang `ps`, **kể cả thay đổi chưa commit**. Đo cùng lúc (sha256):

| tệp | cây làm việc `ps` | cây làm việc mac | bản trong `HEAD` của `ps` (`898c6de8`) |
|---|---|---|---|
| `viscript/crates/vis-cli/src/quyen.rs` | `b5747ad2…` | `b5747ad2…` | `b5f1f9cd…` |
| `viscript/crates/vis-cli/src/main.rs` | `162c5fce…` | `162c5fce…` | `5d2a2ef0…` |
| `viscript/agent/HOST-BAO-DAM.md` | `7d7ad7ac…` | `7d7ad7ac…` | `59bb01db…` |

Hệ quả cần ghi vào R29:
1. Số đo ST-167 **vẫn dùng được** (đo trên đúng nội dung mac đang có; `viscript/` không thiếu tệp), nhưng
   **không tái lập được** từ `HEAD` — muốn tái lập phải có `git status --porcelain` rỗng trong log.
2. Từ nay bằng chứng hợp lệ phải gồm **cả hai**: `git status --porcelain` **rỗng** (ngoài tệp bị ignore)
   **và** dòng `# ST-175: cay SACH …` do `vis-remote` tự ghi (Phần 2).
3. Gate `ps`/`psvn` chỉ chạy trên **clone/worktree sạch**; cây `/root/vifast-linux` cũ **không** dùng để đo nữa
   (khớp VD-177: sau ST-171 clone mới).

## 3. Phần 2 — thay đổi `bin/vis-remote` + `HOST-BAO-DAM.md`

**Bảo đảm 5 (mới) trong `viscript/bin/vis-remote`:** trước khi chạy lệnh trên `ps`/`psvn`, kiểm
`git -C <repo> status --porcelain` (đã bỏ qua tệp bị `.gitignore`):
- **không rỗng ⇒ TỪ CHỐI**: in `so_muc_ban=N`, **5 mục đầu**, gợi ý (`git restore .` / `git clean -fd` /
  clone sạch), dặn `--allow-dirty` chỉ để chẩn đoán; ghi `.exit` = **6**; lệnh **không** chạy; `vis-remote`
  thoát **6**.
- **rỗng ⇒ chạy**, và log có dòng `# ST-175: cay SACH (git status --porcelain rong) — duoc phep chay/do`.
- `--allow-dirty`: **bỏ** kiểm (chẩn đoán). `--require-clean`: **bật** kiểm cả trên `mac` (mac mặc định
  **không** kiểm vì chính công cụ ghi log vào repo và cây làm việc của agent bẩn là bình thường).
  Hai cờ loại trừ nhau ⇒ `[exit=2]`.
- Giữ nguyên hành vi cũ: `--check` (chẩn đoán, **không** bị kiểm chặn), `--hosts` (thêm cột `kiem_cay_ban`),
  mã thoát đọc từ **tệp** `.exit`, log ngoài repo. `usage()` đổi sang `awk` để không lệ thuộc số dòng.
- Mã thoát mới: **6 = cây nguồn bẩn (BLOCKED-DIRTY)**, ghi trong khối “Mã thoát” của `--help`.

**`viscript/agent/HOST-BAO-DAM.md`**: thêm mục ngắn “Cây nguồn để đo phải SẠCH (ST-175, VD-177 §4)”
(3 gạch đầu dòng): (i) `vis-remote` từ chối cây bẩn, exit 6, mac mặc định không kiểm; (ii) `--allow-dirty`
chỉ để chẩn đoán kèm tên log bằng chứng; (iii) `HEAD:viscript` là hash **của commit**, không phải bằng chứng
“cây sạch”.

**Kiểm chứng** (`st175-kiem-che-do.log`, 11 mục, tất cả có `[exit=]`): `--hosts` (mac=skip, ps/psvn=check);
cây sạch ⇒ chạy; cây bẩn ⇒ `[exit=6]` và **không** có dòng đầu ra của lệnh; cây bẩn + mac không cờ ⇒ chạy
(mac skip); hai cờ ⇒ `[exit=2]`; `--allow-dirty` ⇒ `[exit=0]`; `--check` ⇒ `[exit=0]` (`true`=0, `false`=1);
mã thoát thật của lệnh vẫn ra từ tệp `.exit` (ví dụ `exit 7` ⇒ `[exit=7]`); tệp **bị gitignore** không làm
cây bẩn (cây vẫn được coi là sạch, `[exit=0]`).

## 4. Phần 3 — cổng trên mac

`[ĐO]` `./vis-dev gate --fast` (HEAD `ffe75569`) ⇒ **PASS 10/10**, L0 **1184 passed / 0 failed**, `[exit=0]`
(`st175-gate-macos.log`, bắt đầu sau khi chốt nội dung tệp, kết thúc 15:25:44 JST — **trước** mốc đóng băng 15:27).
10 bước: fmt, clippy, luật R1..R4, self-test case âm, vis explain, báo cáo Khối E (28 báo cáo / 197 hàng),
fixtures báo cáo, build, doc-examples (24 khối: 18 xanh + 6 ngoại lệ), L0 test. **Không** đụng `crates/`.
Lần chạy thứ nhất bị **huỷ** vì tôi sửa 1 dòng header giữa lúc chạy (`st175-gate-macos-lan1-huy.log`, ghi rõ
lý do, **không** tính là bằng chứng — không phải retry để lấy xanh).

## 5. Bất thường (phân loại rõ)

1. `[ĐO]` Tập tệp còn đủ 100 % trên `ps` **đúng bằng** danh sách đồng bộ của `bin/vis-linux` (+`docs`, `v2`,
   `spec` bị xoá một phần). `[GT]` Khớp một lần **rsync `--delete` với danh sách `--include` hẹp** hoặc dọn
   đĩa thủ công; **không** khớp `sync_tree()` hiện tại (`viscript/bin/vis-linux:94-118` chỉ `--delete` **trong
   từng cây con**, không chạm `crates/`, `v2/`). `/root/.bash_history` không có dòng khớp ⇒ chưa xác định được lệnh.
2. `[ĐO]` `mtime` rời rạc: `/root/vifast-linux` 17/11 13:00:26, `spec` 13:00:26, `v2` 13:06:05, `docs` 16/11 09:57,
   `viscript` 16/11 10:17 ⇒ xoá **nhiều đợt**, không một lần.
3. `[ĐO]` `.git` trên `ps` là bản rsync từ mac ⇒ `git worktree list` có **28** worktree `prunable` đường dẫn
   `/private/tmp/…` của mac + 3 stash cũ; disk `/` 174G/233G (77 %).
4. `[ĐO]` `.gitignore` gốc bị xoá ⇒ `target/` hiện lên untracked; do đó **hiện tại `ps` luôn bị từ chối** —
   đúng ý đồ (phải clone sạch). 2297 tệp bị xoá vẫn nằm trong index/HEAD (`git ls-files -d` = 2297) nên
   `git restore .` phục hồi được, nhưng **KHÔNG chạy** (chờ A0 đọc kết quả).

## 6. Đề xuất commit message (lead dùng)

```
[ST-175] vis-remote từ chối cây bẩn (exit 6) + điều tra cây ps: 2297/4575 tệp tracked bị xoá (viscript 0), 3 gate ps trước đây chạy trên cây bẩn, PS_TREE chỉ là hash của commit

- vis-remote: bảo đảm 5 — kiểm `git status --porcelain` trước khi chạy trên ps/psvn; bẩn ⇒ từ chối,
  in số mục + 5 mục đầu + gợi ý, exit 6, lệnh không chạy; `--allow-dirty` (chỉ chẩn đoán),
  `--require-clean` (bật kiểm trên mac); mac mặc định không kiểm; --check/--hosts/.exit giữ nguyên
- HOST-BAO-DAM.md: mục "Cây nguồn để đo phải SẠCH" (3 dòng)
- điều tra (chỉ đọc): /root/vifast-linux, user root, HEAD 898c6de8, status 2338 = 2297 xoá + 25 sửa
  + 16 untracked; xoá theo cấp 1: v2 1780, docs 281, crates 93, review 29, legacy 27, scripts 26,
  deploy 24, python-model 19, resources 7, spec 1, skills 1, 9 tệp gốc; viscript 0
- ST-161/166/167 đều chạy trên cây bẩn; chỉ ST-167 ghi PS_TREE=092fd5ab = 795df046:viscript
  (hash CỦA COMMIT, không phải cây làm việc) ⇒ khe hở bằng chứng, ghi R29
- log: JOURNAL/st175-dieutra-cay-ps.log, st175-chan-cay-ban.log, st175-cho-phep-cay-ban.log,
  st175-kiem-che-do.log, st175-gate-macos.log; gate mac PASS 10/10 L0 1184/0 [exit=0] HEAD ffe75569
- không khôi phục/xoá gì trong cây cũ; không đụng crates/
```

## 7. Tệp đã đổi/thêm (index sạch: `git diff --cached` rỗng)

| Trạng thái | Tệp |
|---|---|
| `M` | `viscript/bin/vis-remote` (bảo đảm 5; `--allow-dirty`/`--require-clean`; `--hosts`; `usage()`) |
| `M` | `viscript/agent/HOST-BAO-DAM.md` (mục “Cây nguồn để đo phải SẠCH”) |
| `??` | `viscript/agent/JOURNAL/st175-dieutra-cay-ps.log` |
| `??` | `viscript/agent/JOURNAL/st175-chan-cay-ban.log` |
| `??` | `viscript/agent/JOURNAL/st175-cho-phep-cay-ban.log` |
| `??` | `viscript/agent/JOURNAL/st175-kiem-che-do.log` |
| `??` | `viscript/agent/JOURNAL/st175-gate-macos.log` |
| `??` | `viscript/agent/JOURNAL/st175-gate-macos-lan1-huy.log` |
| `??` | `viscript/agent/JOURNAL/st175-bao-cao.md` (tệp này) |

## 8. Việc còn lại (cho lead / A0)

1. Commit 9 tệp ở §7 bằng quyền lead (Agent 2 **không** commit — hook `vifast-guard`).
2. Cập nhật `viscript/agent/STATE.md` (Agent 2 chưa sửa để tránh đụng phiên khác).
3. Ghi **khe hở bằng chứng** (§2) vào R29 và quy định: gate `ps`/`psvn` chỉ chạy trên clone/worktree sạch,
   log phải có `git status --porcelain` rỗng + dòng `# ST-175: cay SACH`.
4. **Không** khôi phục cây `/root/vifast-linux` cho tới khi A0 đọc kết quả; sau ST-171 dùng **clone mới**.

`[exit=0]`
