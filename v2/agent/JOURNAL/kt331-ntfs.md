# KT-331 — Guard tên tệp Windows/NTFS + chính sách `JOURNAL/` (Chu kỳ 2, Đợt 0)

- **Task**: KT-331 (P0, 2 h), Chu kỳ 2 / Đợt 0 — `KE-HOACH-CHU-KY-2-2026-11-22.md` §3.
- **Căn cứ**: D2-229 (4 tệp `kt219-logs/cases/` có `:`/`>` làm gãy `git clone` trên Windows),
  D2-230 (cấm commit raw `run-soak-*/boot-*.log`; bắt buộc kiểm tên tệp NTFS ở `git-guard` +
  `v2 check`), D2-235(3) (Đợt 0 **cấm** chạm `v2/kernel/**`, `v2/lib/**`), D2-238 (DAG `KT-348`).
- **Nhánh**: `task/kt331-ntfs`, base `main` @ `0809b93a`. Không push, không merge, không chạm `main`.
- **Môi trường**: macOS, worktree `/private/tmp/kt331-s1`; lead đang chạy soak độc quyền ⇒
  **không** chạy `qemu-system*` trong suốt task (chỉ `v2 check` + checker Python/bash).

## 1. Khảo sát

### 1.1 Bốn tệp lỗi cũ — **đã được Agent 0 đổi tên**, không còn tệp lỗi

`ls v2/agent/JOURNAL/kt219-logs/cases/ | grep -E '[:<>|?*]'` ⇒ **0 dòng** (68 tệp). Lần đổi
tên nằm trong lịch sử tại `a2378b43` (KT-00, `R100` = 100 % blob giữ nguyên):

| Tên cũ (gãy Windows) | Tên mới |
|---|---|
| `A15_mod_$n:ident_trong_macro_rules.cargo.log` | `A15_mod_n_ident_trong_macro_rules.cargo.log` |
| `A15_mod_$n:ident_trong_macro_rules.json` | `A15_mod_n_ident_trong_macro_rules.json` |
| `A3_symlink_src_link.rs_->__tmp.cargo.log` | `A3_symlink_src_link.rs_to__tmp.cargo.log` |
| `A3_symlink_src_link.rs_->__tmp.json` | `A3_symlink_src_link.rs_to__tmp.json` |

⇒ **Không đổi tên tệp nào** trong task này (đúng yêu cầu: chỉ báo, không sửa; đổi tên tệp
tracked sẽ làm gãy `git ls-files` của người khác). Quét toàn bộ **10.250 tên tracked** bằng
máy kiểm mới: **0 vi phạm** (xem §3). Con số `10250` là **thời điểm khảo sát**; sau commit
của task, máy kiểm đọc `10252` tên (thêm `check-filenames.py` + journal này) và vẫn
`PASS` — xem §6.

### 1.2 `v2/bin/v2 rules` đang kiểm gì, chèn bước mới ở đâu

- `rules <root>` (`_rules_impl`) chạy **5 luật cấu trúc** trên `SCAN_DIRS="kernel lib sim user
  std boot test tools"` (+ `spec/` khi quét cây v2): **R1** tệp kernel/lib ≤ 1.500 dòng,
  **R2** cấm `cfg(target_arch)` trong `kernel/vifast-core` + `lib/`, **R3** SPDX theo loại tệp,
  **R4** cấm path-dependency sang `crates/`, **R5** `unsafe` ngoài danh sách đóng (D2-17).
  In `R<k> <đường dẫn>[:dòng]: <lý do>`, exit 1 nếu có vi phạm.
- `rules` là **bước 3** của `v2 check` (`lane_step main 3`), cũng là bước duy nhất chạy
  `bash v2/test/rules/run.sh` (TAP 6 ca: 5 ca âm R1..R5 + 1 ca dương).
- **Chèn ở đâu**: sau khối R5, **trước `return "$found"`** — một **khối E1 riêng, có nhãn**
  (`E1_NTFS_*`), không gộp vào R1..R5 (mã lỗi không lẫn với luật cấu trúc). Khối E1 chỉ chạy khi
  `$root` là chính cây `v2/` hoặc gốc repo (fixture của `run.sh` là thư mục con — chạy E1 ở đó
  chỉ lặp lại cùng phép kiểm repo-wide, đo được ~0,6 s/lượt ⇒ 6 lượt vô ích).
- Tự kiểm của máy kiểm là **một bước riêng** trong `v2 check`: `step_selftest_filenames`
  (`lane_step B 72 "selftest tên tệp NTFS (KT-331)"`), không dùng chung dải id 1..40 của
  `check` nên đếm đỏ riêng bằng `lane_failed 72` (không nới `lane_fails` lên 72 — sẽ kéo bước
  51 `selfbreak` của `gate --fast` vào lượt đếm của `check`). Hai cổng `--fast`/`--dev` đếm tới
  72 (`lane_fails 72`); kiểm trùng id `lane_step` trên cả ba hàm: **0 trùng** (D2-235(5)).

### 1.3 `pre-commit` của `vifast-guard` có chỗ kiểm tên tệp chưa

**Chưa.** Bản gốc `v2/tools/git-guard/pre-commit` chỉ kiểm `VIFAST_ROLE` (fail-closed, KT-85 /
D2-123.4); **không** có phép kiểm tên tệp nào. Nối thêm vào **nhánh `role=lead`** (nhánh duy
nhất có thể commit), chạy **trước** dòng `CHO QUA` — nhờ vậy mọi ca cũ của `neg-test.sh` giữ
nguyên hành vi (CA-1/CA-2 vẫn deny vì vai; DƯƠNG-1 vẫn allow vì tên probe hợp lệ).

### 1.4 `rules --selftest` / mẫu ca âm sẵn có

`v2/bin/v2 rules` **không** có cờ `--selftest`; mẫu ca âm của luật là `v2/test/rules/run.sh`
(TAP `1..6`, mỗi ca đòi **đúng mã R<k>** trong đầu ra, không chấp nhận "exit 1 vô tội vạ").
Máy kiểm mới theo đúng khuôn đó: `--selftest` in `ok N - …` và đòi **đúng mã** `E1_NTFS_*`.
(Tiền lệ checker Python cùng thư mục: `v2/tools/check-no-cfg-test.py`, `check-scope-table.py`.)

## 2. Máy kiểm mới — `v2/tools/check-filenames.py`

Đặt cùng chỗ các checker khác (`v2/tools/`, khuôn Python + `--selftest`), không tạo kiến trúc mới.
Kiểm **mọi thành phần** đường dẫn (tên thư mục **và** tên tệp), nguồn duy nhất là **tên tệp đã
theo dõi Git** (`git ls-files -z`, chế độ `--staged` dùng `git diff --cached --no-renames
--diff-filter=ACMR`) — **không quét toàn đĩa**.

| Mã | Luật |
|---|---|
| `E1_NTFS_CHAR` | ký tự cấm `[<>:"\|?*]` hoặc ký tự điều khiển `\x00-\x1f` trong tên |
| `E1_NTFS_DEVICE` | tên thiết bị Windows: `CON`, `PRN`, `AUX`, `NUL`, `COM1..COM9`, `LPT1..LPT9` — **kể cả có đuôi** (`CON.txt`), không phân biệt hoa/thường |
| `E1_NTFS_TRAILING` | tên kết thúc bằng dấu chấm hoặc dấu cách (NTFS tự bỏ ⇒ tên khác trên Windows) |
| `E1_NTFS_LEN` | một thành phần > **255 byte** UTF-8 |
| `E1_NTFS_EMPTY` | tên rỗng |

- Thứ tự kiểm cố định EMPTY → CHAR → DEVICE → TRAILING → LEN ⇒ cùng một tên luôn ra cùng mã.
- `--no-renames` là **bắt buộc** ở chế độ `--staged`: đổi tên tệp xấu → tốt phải hiện thành
  xoá + thêm; nếu bật dò đổi tên thì đường dẫn **cũ** (xấu) cũng nằm trong diff và hook sẽ
  chặn chính bản sửa. Có ca tự kiểm riêng cho đúng tình huống này (ca 19).
- Exit: 0 xanh / 1 có vi phạm / 2 dùng sai-không phải git worktree.

## 3. Kết quả

### 3.1 `--selftest`: **23 ca** — 19 ca âm ĐỎ **đúng mã** + 4 ca dương XANH

```
ok 1  - E1_NTFS_CHAR: ':' trong tên tệp            ok 13 - E2E tracked: E1_NTFS_CHAR cho dir>name/inner.txt
ok 2  - E1_NTFS_CHAR: '>' trong tên thư mục        ok 14 - E2E tracked: E1_NTFS_DEVICE cho CON.txt
ok 3  - E1_NTFS_DEVICE: thiết bị có đuôi tệp       ok 15 - E2E tracked: E1_NTFS_TRAILING (dấu cách cuối)
ok 4  - E1_NTFS_DEVICE: thiết bị chữ thường        ok 16 - E2E tracked: E1_NTFS_TRAILING (dấu chấm cuối)
ok 5  - E1_NTFS_TRAILING: dấu cách cuối tên        ok 17 - E2E tracked: có dòng tổng kết FAIL
ok 6  - E1_NTFS_TRAILING: dấu chấm cuối tên        ok 18 - E2E staged: tên xấu đang stage ⇒ ĐỎ đúng mã
ok 7  - E1_NTFS_LEN: 256 byte > 255 byte           ok 19 - E2E staged: đổi tên xấu → tốt KHÔNG bị chặn
ok 8  - E1_NTFS_EMPTY: tên rỗng                    ok 20 - hook: role=lead + tên xấu stage ⇒ TỪ CHỐI
ok 9  - E1_NTFS_CHAR: ký tự điều khiển U+001F      ok 21 - hook: tên hợp lệ ⇒ CHO QUA (giữ KT-85)
ok 10 - ca dương: tên hợp lệ (Unicode/… ) ⇒ XANH   ok 22 - hook: không VIFAST_ROLE ⇒ vẫn TỪ CHỐI
ok 11 - E2E tracked: có tên xấu ⇒ exit 1 (ĐỎ)      ok 23 - ca dương E2E: repo toàn tên hợp lệ ⇒ XANH
ok 12 - E2E tracked: E1_NTFS_CHAR cho bad:name.txt
# 23/23 ca đúng kỳ vọng
```

- **Đỏ trước (D2-05)**: khi **chưa** nối hook, ca 20 đỏ đúng nghĩa —
  `not ok 20 … (exit 0, ra 'vifast-guard(pre-commit): CHO QUA — VIFAST_ROLE=lead')`:
  hook cũ **cho commit tên xấu** đi qua. Log đỏ-trước: `/tmp/kt331-selftest-hook-red.log`;
  sau khi nối hook: **23/23 ok** (`/tmp/kt331-selftest-green.log`).
- Chạy trên repo thật: `E1 tên tệp NTFS: PASS (10250 tên tracked)` ⇒ **0 vi phạm** tồn đọng.
- `v2/bin/v2 rules` (đường nối E1) in `E1 tên tệp NTFS: PASS (10250 tên tracked)` và `rules:
  PASS (v2)`. Ca **đỏ của đường nối**: chạy với `V2_PY` trỏ checker trên một repo tạm có
  `bad:name.txt` ⇒ in `E1_NTFS_CHAR bad:name.txt: …` + `rules: FAIL (v2)`, **exit 1**.
- `bash v2/test/rules/run.sh` (mẫu ca âm sẵn có): **6/6 case đúng kỳ vọng** không đổi.

### 3.2 `.gitignore` — raw log soak (D2-230)

Thêm `v2/agent/JOURNAL/**/run-soak-*/boot-*.log`. Kiểm **không che tệp đang theo dõi** bằng
so sánh A/B (`git ls-files --cached --ignored --exclude-standard` trước/sau khi thêm dòng):

- Trước: **4.480** tệp tracked đã bị ignore (nợ **cũ** từ `run-*/` của KT-179 — raw log đã
  tracked từ trước; D2-230 nói **không** xoá lịch sử `JOURNAL/` cũ nên không chạm).
- Sau: **4.480** tệp — **thêm 0, bớt 0**. Dòng mới là tập con của `run-*/` đã có, ghim đúng
  quy ước D2-230; mẫu `v2/agent/JOURNAL/kt999/run-soak-1/boot-0.log` khớp `.gitignore:21`.
- `git check-ignore -v .gitignore v2/bin/v2 v2/tools/git-guard/pre-commit
  v2/tools/check-filenames.py` ⇒ không tệp nào của task bị ignore.

### 3.3 Nối `pre-commit`

`v2/tools/git-guard/pre-commit` gọi `check-filenames.py --root <top> --staged` khi
`VIFAST_ROLE=lead`, **trước** dòng `CHO QUA`; vi phạm ⇒ `RESULT=DENY
reason=ntfs-filename` + in đủ mã/đường dẫn + gợi ý sửa, exit 1. Không có checker (checkout cũ)
⇒ bỏ qua **kèm cảnh báo**; có checker mà thiếu python3 ⇒ fail-closed (cổng repo vốn đòi python3).
Đã cài hook mới bằng `v2/tools/git-guard/install.sh` để `install.sh --check` xanh và hook có
hiệu lực cho mọi worktree; `neg-test.sh` (5 ca KT-85 cũ) vẫn PASS — xem §4.

## 4. Cổng

- **`./v2/bin/v2 check` — PASS** trên cây **cuối** (log `/tmp/kt331-v2check-final.log`):
  **41/41 bước OK, 0 FAIL** (trước KT-331 là 40 bước), `v2 check: PASS`, tổng `100489 ms`
  theo log cổng. Hai chỗ mới có mặt trong log: `E1 tên tệp NTFS: PASS (10250 tên tracked)`
  ở bước `luật cấu trúc R1..R5 + tên tệp NTFS (KT-331)`, và
  `--- selftest tên tệp NTFS (KT-331): OK` (bước 72). Luật máy K6 (`verify-numbers`) vẫn OK
  sau khi thêm §1.3 vào `v2/docs/k6-git-guard.md` (không ghi số đo viết tay vào tài liệu).
- **`bash v2/tools/git-guard/neg-test.sh` — PASS 7/7 ca** (`rc=0`, log
  `/tmp/kt331-neg-test.log`): CA-1/CA-2 deny (giữ fail-closed), D1/D2 `VIFAST_ROLE=lead` allow,
  git status/log không bị phá, `--no-verify` vẫn là quyền của người; `HEAD` checkout chính và
  fingerprint index **không đổi**; nhánh/worktree tạm đã dọn (số worktree trở về như trước).
- **Cài hook mới**: `bash v2/tools/git-guard/install.sh` → `install.sh --check` **PASS**
  (trước khi cài, `--check` báo `[FAIL] pre-commit lệch bản gốc` — đúng vì bản gốc vừa đổi).
  Hook áp cho **75 worktree** đang đăng ký (hook nằm ở `git-common-dir`).
- `bash v2/test/rules/run.sh` — **6/6 case** đúng kỳ vọng (mẫu ca âm sẵn có của 5 luật).
- `python3 v2/tools/check-selftest-wiring.py --root .` — PASS; `check-filenames.py` được nhận
  là script tự kiểm **có cổng gọi** (`v2/bin/v2`, `step_selftest_filenames`).
- **Không** chạy `qemu-system*` (máy đang soak độc quyền của lead).
- **Không** chạm `v2/kernel/**`, `v2/lib/**`, `spec/**`:
  `git diff main --stat -- v2/kernel v2/lib spec` **rỗng**.

### 4.1 Giới hạn chuyển tiếp (đã kiểm chứng)

Hook **fail-open** khi worktree đứng ở **commit trước KT-331** (chưa có
`v2/tools/check-filenames.py`): worktree tạm cắt từ `0809b93a` ⇒ hook in
`CẢNH BÁO — chưa có … (worktree ở commit trước KT-331), bỏ qua` rồi `CHO QUA`. Đây là **chủ ý**:
75 worktree đang đăng ký đều ở commit cũ; fail-closed sẽ chặn oan mọi agent ngay lúc chuyển tiếp.
Vế fail-closed thuộc về **cổng**: `v2 rules`/`v2 check` báo `E1 … FAIL (thiếu …)` nếu thiếu máy
kiểm. Sau khi commit KT-331, worktree cắt từ commit đó có checker ⇒ hook chặn thật (§4.2).
(Các nhánh/worktree thử đã xoá; số worktree trở về `75`; `HEAD` nhánh này không đổi.)

### 4.2 Kiểm lại hook trên worktree cắt từ commit KT-331

Worktree tạm cắt từ `HEAD` (commit KT-331), hook **đã cài** (`install.sh --check` PASS):

- **Ca âm**: stage `probe:bad.txt`, `VIFAST_ROLE=lead git commit` ⇒ **rc=1**, in
  `TỪ CHỐI COMMIT — tên tệp không tương thích Windows/NTFS` + `E1_NTFS_CHAR probe:bad.txt:` +
  `RESULT=DENY hook=pre-commit reason=ntfs-filename`; `HEAD` worktree **không đổi** — commit bị
  chặn thật (log `/tmp/kt331-e2e-bad.log`).
- **Ca dương**: stage `probe-good.txt`, cùng vai ⇒ **rc=0**, `CHO QUA` — không phá KT-85
  (log `/tmp/kt331-e2e-good.log`).
- Dọn: nhánh + worktree tạm đã xoá; số worktree trở về `75`; `HEAD` nhánh task không đổi.

## 5. Tệp thay đổi

| Tệp | Việc |
|---|---|
| `v2/tools/check-filenames.py` | **mới** — máy kiểm tên tệp Windows/NTFS + `--selftest` 23 ca |
| `v2/bin/v2` | khối `E1` trong `rules`, nhãn bước 3, `step_selftest_filenames` (lane_step 72), `lane_failed`, đếm `lane_fails 72` ở hai cổng, help |
| `v2/tools/git-guard/pre-commit` | gọi checker `--staged` khi `VIFAST_ROLE=lead`, **trước** dòng `CHO QUA`; header + `ntfs_check` |
| `.gitignore` | `v2/agent/JOURNAL/**/run-soak-*/boot-*.log` (D2-230) |
| `v2/docs/k6-git-guard.md` | §1.3: luật `E1_*`, cách sửa, giới hạn chuyển tiếp; bảng hook nêu KT-331 |
| `v2/agent/JOURNAL/kt331-ntfs.md` | journal này |

**Không** sửa: `v2/kernel/**`, `v2/lib/**`, `spec/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`docs/v1/**`. **Không** đổi tên tệp nào (4 tệp lỗi đã được Agent 0 đổi tên ở `a2378b43`).
**Không** chạy QEMU. Commit trên nhánh `task/kt331-ntfs` (uỷ quyền `VIFAST_ROLE=lead` theo
D2-198, hết hiệu lực cuối tuần; **không** push, **không** merge, **không** chạm `main`).

## 6. Trạng thái sau commit (đo lại trên cây đã commit)

- `git status` **sạch**; commit cuối trên `task/kt331-ntfs`, cha `0809b93a`; **không** push/merge.
- `check-filenames.py --selftest` trên cây đã commit: **23/23 ok**, `rc=0`; checker đọc
  **10252** tên tracked ⇒ `PASS`; `v2 rules --root v2` ⇒ `PASS`.
- `install.sh --check` ⇒ `PASS` (hook đã cài khớp bản gốc của commit này).
- `neg-test.sh` sau commit ⇒ **PASS 7/7 ca**, `HEAD` checkout chính và số worktree (75)
  **không đổi**.
- §4.1/§4.2: worktree ở commit **trước** KT-331 ⇒ hook bỏ qua kèm cảnh báo; worktree cắt từ
  commit KT-331 ⇒ hook **TỪ CHỐI** tên xấu (`E1_NTFS_CHAR`) và **CHO QUA** tên hợp lệ.
- `git diff 0809b93a --stat -- v2/kernel v2/lib spec` **rỗng**; `git ls-files --cached
  --ignored --exclude-standard` vẫn **4480** tệp (không che thêm tệp tracked nào).
- **Không** chạy `qemu-system*` trong suốt task.
