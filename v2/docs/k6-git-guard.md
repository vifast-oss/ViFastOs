# Chặn ghi nhánh chính bằng máy — bốn hook git (KT-85 + KT-85b)

**Ngày**: 2026-11-17 · **Task**: KT-85 (hook `pre-commit`/`pre-push`) + **KT-85b** (hook
`reference-transaction`/`commit-msg`) · **Thi hành**: D2-123 mục bốn (a/b/c/d/e) + D2-125.1 +
**D2-125.2** (uỷ quyền Agent 1) · **Căn cứ sự cố**: A1-177 (index dùng chung bị nhiễm chéo
dòng), A1-180 (agent tự commit làm HEAD đỏ), **A1-183** (lead cherry-pick được commit vào
`refs/heads/main` mà hook không chặn) · **Vùng ghi**: `.git/hooks/{pre-commit,pre-push,
commit-msg,reference-transaction}`, `v2/tools/git-guard/`, tài liệu này,
`v2/agent/JOURNAL/kt85-*`, `v2/agent/JOURNAL/kt85b-*`.

Ba lần agent/người làm sai luật nhánh (A1-177, A1-180, A1-183) không phải sơ suất lẻ: mỗi lần
đều cho thấy **một đường mới** để ghi vào nhánh chính. D2-123 mục bốn chốt nguyên tắc:
**sửa bằng cấu trúc, không nhắc bằng lời**. Cấu trúc đó gồm hai phần và tài liệu này mô tả cả
hai:

- **bốn hook fail-closed** trong git (mục một, ba, bốn) — chặn theo **ai** (vai), theo **ref bị
  ghi** (nhánh bảo vệ) và theo **nội dung** (nhãn task);
- **worktree riêng cho từng task** (mục hai) — phần này mới là chỗ vá gốc A1-177, vì mỗi
  worktree có **index riêng**, nên hai dòng/task không thể stage lẫn của nhau.

**Đọc mục bảy trước khi tin vào hàng rào này.** `VIFAST_ROLE` là hàng rào **chống nhầm**, không
phải chống cố ý (D2-125.1). Người có quyền trên máy vẫn gỡ/tắt được hook; hàng rào **thật** là
bảo vệ nhánh trên GitHub (mục sáu).

Tài liệu **không** chứa số đo: mọi số đo nằm trong các log `v2/agent/JOURNAL/kt85-hook-neg.log`
và `v2/agent/JOURNAL/kt85b-hook-neg.log` (theo OD-K6-5: số phải trích nguyên văn từ log đã
commit; khi lead commit log xong, ai cần số thì mở đúng khối trong log).

## 1. Bốn hook — cài ở đâu, chặn gì

| Hook | Chặn | Chạy khi | Từ |
|---|---|---|---|
| `pre-commit` | `git commit` của vai không phải `lead` (kể cả `--amend`) | trước khi tạo commit | KT-85 |
| `pre-push` | `git push` của vai không phải `lead` (git vẫn gọi hook cho `--dry-run`) | trước khi gửi ref lên remote | KT-85 |
| **`commit-msg`** | commit message **thiếu nhãn** `[KT-##]`/`[ST-##]` ở đầu subject | sau khi có message, trước khi tạo commit | **KT-85b** |
| **`reference-transaction`** | **mọi** cập nhật nhánh bảo vệ (`refs/heads/main`, `refs/heads/master`, …) của vai không phải `lead` | mỗi lần git chuẩn bị đổi ref | **KT-85b** |

- **Vị trí cài**: `<git-common-dir>/hooks/{pre-commit,pre-push,commit-msg,reference-transaction}`
  — thư mục hook **chung**, nên hook áp cho **mọi** worktree của repo. Đây là điểm cố ý: agent
  làm trong `../wt-<mã>` vẫn bị chặn, không có đường "worktree riêng nên không bị kiểm".
- **Bản gốc trong repo**: `v2/tools/git-guard/<tên hook>`; `v2/tools/git-guard/install.sh`
  **copy** (không symlink) vào common dir. Copy để xoá worktree không làm gãy hook, và để
  `install.sh --check` so được sha256 giữa bản cài và bản gốc.
- Hook **không** chạy cho `git status`, `git log`, `git diff`, `git add`: các lệnh đó không đi
  qua hook nên không thể hỏng (có ca kiểm riêng trong log ca âm).
- Bản cài là **của repo**, không phải của một checkout: người mới clone phải chạy
  `v2/tools/git-guard/install.sh` (hook không đi theo git — mục bảy).
- **Chạy `install.sh` TRƯỚC `gate` lần đầu** (R4 của Agent 0, phiếu lượt bốn): `gate --fast` gọi
  hai ca âm git-guard, mà hai ca ấy cần hook **đã cài**; thiếu hook thì chúng tự in "hook CHƯA
  cài đúng — chạy …install.sh trước" và thoát với **mã lỗi môi trường** ⇒ cổng ĐỎ kèm lý do rõ,
  **không** im lặng bỏ qua. Đây là hành vi cố ý (bỏ qua thì cổng xanh mà phép kiểm không chạy),
  nên bản clone mới phải cài hook trước khi trông đợi cổng xanh.

### 1.1 Vì sao phải thêm `reference-transaction` (A1-183)

`pre-commit` chỉ chạy cho **`git commit`**. Mọi lệnh khác cũng ghi vào nhánh chính mà **không**
đi qua nó: `git cherry-pick`, `git merge`, `git reset`, `git rebase`, `git branch -f`,
`git update-ref`, `git commit-tree` + `update-ref`. A1-183 chính là ca cherry-pick: ref đổi,
hook im lặng.

`reference-transaction` là hook **duy nhất** nằm dưới **mọi** lệnh đổi ref (từ bản git ghi ở §3.1). Chặn ở
đây là chặn theo **bản chất của thao tác** (cập nhật ref), không theo **tên lệnh** — nên không
còn đường vòng bằng lệnh khác. Hook nhận trên stdin các dòng `<old> SP <new> SP <ref>` và
tham số `$1` = trạng thái transaction:

- `preparing`, `prepared` ⇒ **có hiệu lực**; hook trả mã thoát khác không ⇒ git **huỷ cả transaction**
  (không ref nào đổi);
- `committed`, `aborted` ⇒ chỉ là thông báo sau khi xong; hook **cho qua** (mã thoát bị bỏ qua).

> Ghi chú kỹ thuật: tài liệu git nói rõ mã thoát **chỉ** có tác dụng ở `preparing`/`prepared`.
> git đời mới (bản trên máy này ghi trong `v2/agent/JOURNAL/kt85b-git-version.log`) gọi hook
> **bốn** trạng thái, không phải ba. Vì vậy hook của ta
> chặn ở **cả hai** trạng thái đầu, và fail-closed với trạng thái lạ — nếu chỉ kiểm `prepared`
> thì có đường lọt qua ở `preparing`.

### 1.2 Vì sao thêm `commit-msg`

Luật repo (AGENTS.md §7): mỗi commit mở đầu bằng `[KT-##]` hoặc `[ST-##]`. KT-85 chặn **ai**
commit nhưng không kiểm **nội dung**; commit không nhãn làm mất đường truy vết task ⇔ journal ⇔
cổng. `commit-msg` kiểm đúng một điều: subject khớp `^\[(KT|ST)-[0-9]+[a-z]?\]`, và **miễn trừ**
message do git tự sinh (`Merge …`, `Revert "…"`) để `git merge`/`git revert` của lead không bị
chặn oan. Hook này **không** kiểm vai — việc "ai được commit" đã do `pre-commit` lo.

## 2. Quy trình worktree cho **mọi** agent (bắt buộc)

Luật (D2-123 mục bốn (a)): **agent không làm trên nhánh chính**. Mỗi task một worktree, một
nhánh `task/<mã>`; xong thì **báo lead**, lead commit/merge — agent không tự `git add`,
không tự commit, không tự push, **không cherry-pick/merge vào `main`**.

```bash
# (a) Tạo worktree riêng cho task (đặt cạnh repo, không đè lên checkout chính)
git -C /Users/mps/dvchd/ViFastOs worktree add ../wt-<mã> -b task/<mã>
cd ../wt-<mã>

# (b) Làm toàn bộ việc trong worktree này: sửa tệp, chạy cổng, ghi log
./v2/bin/v2 check

# (c) Xong: KHÔNG commit. Gửi lead danh sách tệp + log + kết quả cổng
#    (ví dụ: "KT-85b sẵn sàng trong /Users/mps/dvchd/wt-kt85b, nhánh task/kt85b,
#     hai hook mới + install.sh + tài liệu, check PASS")

# (d) LEAD (Agent 1) commit trong đúng worktree đó rồi merge/cherry-pick sang nhánh chính
#     — cả ba lệnh đều phải có VIFAST_ROLE=lead, nếu không reference-transaction sẽ chặn:
VIFAST_ROLE=lead git -C ../wt-<mã> commit -m '[KT-<mã>] <tóm tắt>'
VIFAST_ROLE=lead git -C /Users/mps/dvchd/ViFastOs merge --no-ff task/<mã>
# hoặc: VIFAST_ROLE=lead git -C /Users/mps/dvchd/ViFastOs cherry-pick <sha>

# (e) Dọn sau khi lead đã merge
git -C /Users/mps/dvchd/ViFastOs worktree remove ../wt-<mã>
git -C /Users/mps/dvchd/ViFastOs branch -D task/<mã>
```

Vì sao quy trình này chặn đúng lỗi cũ:

- mỗi worktree có **index riêng** ⇒ tệp của task này không thể bị `git add` từ dòng khác cuốn
  vào commit (A1-177 không tái diễn được, kể cả khi hai agent chạy song song);
- `git commit` bị chặn nếu không có `VIFAST_ROLE=lead` ⇒ agent không tạo được commit "tự phát"
  làm đỏ HEAD (A1-180);
- **mọi** đường đổi `refs/heads/main` bị chặn ở tầng ref ⇒ kể cả cherry-pick/merge/reset/rebase
  (A1-183);
- commit message thiếu nhãn bị chặn ⇒ không còn commit "vô danh" trong lịch sử;
- lead thấy diff đúng một nhánh task, không phải một cây bẩn lẫn nhiều dòng.

Điều **không** được làm: commit/push trên checkout chính; `git add` ở checkout chính (kể cả
`git add -N`); `git push --force`; viết lại lịch sử (ba commit cũ `795df046`/`ccabd069`/`9af541a0`
giữ nguyên — D2-123 mục bốn (e)).

## 3. Cài, kiểm, gỡ

```bash
v2/tools/git-guard/install.sh            # cài cả bốn hook (idempotent: chạy lại không đổi gì)
v2/tools/git-guard/install.sh --check    # chỉ kiểm, không ghi; mã thoát bằng không = đúng, khác không = lệch
v2/tools/git-guard/install.sh --uninstall# gỡ (chỉ gỡ hook của vifast-guard)
v2/tools/git-guard/install.sh --repo DIR # áp cho repo khác (dùng cho ca tự kiểm/clone tạm)
v2/tools/git-guard/install.sh --print-support # in phiên bản git + hook nào cài được trên máy này
```

- **Idempotent**: lần hai in "đã cài đúng, không đổi" (so sha256), không ghi đè vô ích.
- **`--check` kiểm cả bốn hook**: tồn tại, có quyền chạy, nội dung trùng bản gốc, và
  `core.hooksPath` không bị trỏ chỗ khác. Đây là điều kiện đầu vào của `neg-test*.sh` (không cài
  đúng thì ca âm vô nghĩa vì hook không chạy).
- **Bất biến `core.hooksPath`**: nếu biến này trỏ chỗ khác thì hook của repo **không bao giờ
  chạy** mà không ai thấy. `install.sh` **từ chối** cài khi bất biến sai và in cách sửa:
  `git config --unset core.hooksPath`. Không có cờ "kệ đi" — fail-closed.
- Hook lạ đã có sẵn ở cùng đường dẫn: `install.sh` sao lưu thành `<tên>.bak.<thời điểm>` rồi
  mới cài, in rõ đường dẫn sao lưu. `--uninstall` chỉ gỡ hook có nhãn `vifast-guard`.
- Quyền chạy hook là điều kiện sống còn: file không có quyền chạy thì git **bỏ qua im lặng**;
  `--check` báo đỏ đúng ca đó.

### 3.1 Kiểm phiên bản git — `reference-transaction` cần git đủ mới

- **Ngưỡng tối thiểu**: bản git-2.28 — hook `reference-transaction` ra đời từ bản đó.
- **Máy hiện tại**: đủ mới (phiên bản đầy đủ + kết luận `install.sh --print-support` nằm trong
  log `v2/agent/JOURNAL/kt85b-git-version.log`; log cài đặt: `kt85b-install-check.log`). Theo
  OD-K6-5 tài liệu **không** chép số từ log vào prose — cần số thì mở đúng dòng trong log.

`install.sh` đọc `git version` rồi quyết định:

- **đủ mới** ⇒ cài **cả bốn** hook;
- **cũ hơn ngưỡng** ⇒ **không cài** `reference-transaction` (cài cũng không bao giờ chạy), in
  `[SKIP]` kèm lý do, **vẫn cài ba hook còn lại**, cuối cùng in **CẢNH BÁO**: trên máy đó lỗ hổng
  A1-183 (cherry-pick/merge/reset vào nhánh chính) **chưa** được vá ⇒ phải nâng git hoặc dựa vào
  bảo vệ nhánh GitHub (§6). `--check` trên git cũ báo `[SKIP]` — **không** phải lỗi cài đặt;
- **không đọc được phiên bản** ⇒ coi như **cũ** (fail-closed: thà báo thiếu hook còn hơn cài hook
  không bao giờ chạy).

Máy hiện tại đủ mới nên **cả bốn hook đã cài** (log cài đặt: `kt85b-install-check.log`).

## 4. Luật vai — ai được ghi cái gì

- Nguồn vai **duy nhất**: biến môi trường `VIFAST_ROLE`. Hook **không** tự đặt biến, **không**
  đọc `.git/config`, **không** đọc tệp cấu hình nào — nếu đọc được từ tệp thì agent sửa tệp là
  xong, luật vô nghĩa.
- **Chỉ Agent 1 (lead)** đặt `VIFAST_ROLE=lead` trong môi trường của lệnh commit/push/merge/
  cherry-pick/reset. Agent 2 và reviewer **không** đặt biến; không có biến nào khác thay thế.

| `VIFAST_ROLE` | `git commit` | `git push` | `commit-msg` (nhãn) | ghi `refs/heads/main` (commit/cherry-pick/merge/reset/rebase/update-ref) |
|---|---|---|---|---|
| chưa đặt (hoặc rỗng) | **từ chối** (fail-closed) | **từ chối** | kiểm như thường | **từ chối** |
| `agent2`, `reviewer`, vai khác | **từ chối** | **từ chối** | kiểm như thường | **từ chối** |
| `Lead`, `lead ` (sai hoa/thừa dấu cách) | **từ chối** | **từ chối** | kiểm như thường | **từ chối** |
| `lead` | cho qua | cho qua | kiểm như thường | cho qua |
| `lead` + message không nhãn | **từ chối** (do `commit-msg`) | — | **từ chối** | — |

- Fail-closed là chủ ý: biến **chưa đặt** bị coi là "không đủ tư cách", không mặc định cho qua.
  Nếu lead quên đặt biến, lệnh bị chặn (đúng luật) — đặt lại biến rồi chạy lại, không có đường
  tắt.
- Khi từ chối, hook in: **ai** đang chạy (`user.name`/`user.email`), **ở đâu** (nhánh, toplevel,
  checkout chính hay worktree liên kết), **ref/câu bị chạm**, **vì sao**, và **phải làm gì**
  (tạo worktree `../wt-<mã>` trên nhánh `task/<mã>`, báo lead; lead đặt biến rồi commit/merge).
  Kèm một dòng máy đọc được `vifast-guard: RESULT=DENY hook=...` để log/CI grep được.
- **Nhánh bảo vệ**: `refs/heads/main` và `refs/heads/master` **luôn** bảo vệ, không tắt được
  bằng biến. Danh sách cấu hình `VIFAST_PROTECTED_REFS` (cách nhau bởi dấu cách/phẩy/hai chấm)
  chỉ **thêm** ref được bảo vệ — cố ý **không** cho bớt, vì nếu bớt được thì agent chỉ cần đặt
  biến rỗng là qua.
- **Quyền của người** (không phải agent): `git commit --no-verify` / `git push --no-verify`
  **bỏ qua** `pre-commit`, `commit-msg`, `pre-push` (git bỏ qua cả cụm hook đó). Riêng
  `reference-transaction` **không** có cờ `--no-verify` — tắt nó phải dùng
  `git -c core.hooksPath=…` hoặc gỡ hook. Đó vẫn là quyền của người, không phải của agent; agent
  dùng là **vi phạm luật repo**, không phải "tính năng".

## 5. Bằng chứng ca âm

Hai log, hai phạm vi — không trộn:

| Log | Script | Phủ |
|---|---|---|
| `v2/agent/JOURNAL/kt85-hook-neg.log` | `v2/tools/git-guard/neg-test.sh` | KT-85: commit/push bị từ chối theo vai, `--no-verify`, không phá `git status`/`git log` |
| `v2/agent/JOURNAL/kt85b-hook-neg.log` | `v2/tools/git-guard/neg-test-kt85b.sh` | **KT-85b**: cherry-pick / merge / reset vào `main`, `commit-msg`, cổng phiên bản git, PROBE trên repo thật |

Bốn ca âm **bắt buộc** của D2-125.2 (đều có trong log `kt85b`):

- **cherry-pick**: `VIFAST_ROLE=agent2 git cherry-pick <sha>` trên nhánh `main` ⇒ **bị từ
  chối**, `main` không đổi (đúng ca A1-183);
- **merge**: `VIFAST_ROLE=agent2 git merge --no-ff <nhánh>` ⇒ **bị từ chối**;
- **reset**: `VIFAST_ROLE=agent2 git reset --hard HEAD~1` ⇒ **bị từ chối**, `main` không đổi;
- **commit-msg**: `git commit -m "khong co nhan"` ⇒ **bị từ chối** (vai `agent2` thì `pre-commit` chặn trước —
   git chạy `pre-commit` **trước** `commit-msg`; vai `lead` thì `commit-msg` chặn — log có cả hai
   để chứng minh hook nhãn chạy độc lập), còn `VIFAST_ROLE=lead git commit -m "[KT-00] thu"` ⇒
   **cho qua**; lead `git merge` (message `Merge …` tự sinh) cũng cho qua (miễn trừ đúng).

Kèm theo trong cùng log:

- **hai ca dương**: lead cherry-pick vào `main` và lead merge vào `main` ⇒ **đi qua** (hook không
  chặn oan vai lead);
- **PROBE trên repo thật** (không thể làm đổi lịch sử): bật bảo vệ một nhánh nháp qua
  `VIFAST_PROTECTED_REFS`, `git update-ref` nhánh đó từ **worktree tạm** bằng vai `agent2` ⇒ bị
  từ chối, nhánh nháp **không** được tạo; cùng lệnh với vai `lead` ⇒ qua (đối chứng: chặn là do
  **vai**, không phải do lệnh sai), rồi xoá nhánh nháp;
- **cổng phiên bản**: dùng một `git` giả chỉ nói dối `git version` (bản cũ hơn ngưỡng) ⇒
  `install.sh` in `[SKIP] reference-transaction`, **không** cài hook đó, **vẫn** cài ba hook kia,
  và `--check` trả mã thành công;
- **bất biến**: `git rev-parse refs/heads/main` của repo **thật** trước/sau **không đổi**,
  `HEAD` không đổi, **fingerprint index** (`git ls-files -s | sha256`) của checkout chính không
  đổi, số worktree trở về như trước, thư mục tạm đã xoá.

Về cách thử: `refs/heads/main` đang được checkout ở checkout chính nên git **không** cho worktree
thứ hai checkout cùng nhánh. Vì vậy ba ca cherry-pick/merge/reset được thử trong **clone cục bộ
tạm** (`git clone --local`, không mạng), có cài đúng bộ hook bằng `install.sh --repo`; repo thật
chỉ được **đọc** và chỉ nhận phép PROBE không thể đổi lịch sử. Đây là cách duy nhất vừa thử
được đúng `refs/heads/main` vừa bảo đảm repo thật không thể bị đổi dù hook hỏng.

Log **không** trích số vào tài liệu này: tại thời điểm viết, log chưa được commit (agent không
tự commit — chính luật của task này); theo OD-K6-5, log phải **commit trước** báo cáo thì số mới
được trích.

Tái lập: `v2/tools/git-guard/neg-test.sh` và `v2/tools/git-guard/neg-test-kt85b.sh` (tự tạo/dọn
thư mục tạm; `--keep` giữ lại khi có ca hỏng, `--no-cleanup` giữ nguyên để soi).

## 6. Nếu dùng GitHub — bật bảo vệ nhánh chính (việc phải làm, không phải tuỳ chọn)

Repo hiện là **cục bộ**, chưa có remote GitHub, nên phần này **chưa thi hành** (D2-123 mục bốn
(c) yêu cầu ghi hướng dẫn; Agent 1 gửi cho người dùng khi đưa repo lên). Khi đưa repo lên GitHub,
làm **đủ** các bước sau cho `main` (**Settings → Branches → Add branch protection rule**, hoặc
**Settings → Rules → Rulesets** nếu dùng ruleset mới):

- **Require a pull request before merging** — không cho push thẳng vào `main`; đặt **ít nhất một
  approving review** và bật **Dismiss stale approvals** khi có commit mới (review cũ không còn
  giá trị sau khi nhánh đổi).
- **Require status checks to pass** — chọn check của cổng (`v2 check`, `v2 gate --fast` khi CI đã
  nối) và bật **Require branches to be up to date before merging**.
- **Do not allow force pushes** — không cho `push --force`/`--force-with-lease` viết lại lịch sử
  `main` (khớp D2-123 mục bốn (e)).
- **Do not allow deletions** — không cho xoá nhánh `main`.
- **Restrict who can push** (hoặc **Restrict updates** trong ruleset) — chỉ tài khoản/khoá của
  **Agent 1** được ghi `main`; agent khác chỉ được mở PR từ nhánh `task/<mã>`.
- **Require linear history** (tuỳ chọn) — chặn merge commit do máy tạo, dễ soi lịch sử hơn.
- Với **tag phát hành**: bật bảo vệ tag cùng lý do (không cho ghi đè/xoá tag đã phát hành).

Ghi lại **ai** đã bật (Agent 1), **ngày**, và ảnh chụp/đường dẫn cấu hình vào nhật ký vòng — để
lần review sau kiểm được, không chỉ tin lời.

**Vì sao vẫn cần, dù đã có bốn hook:** hook là rào cản **cục bộ** trên máy này. Người/agent có
quyền trên máy vẫn bỏ qua được (`--no-verify`, `git -c core.hooksPath=…`, gỡ hook), và một bản
clone ở máy khác thì **không có** hook nào của ta. Bảo vệ nhánh trên server là ranh giới **duy
nhất** mà agent không tự vượt được — đó là hàng rào thật; bốn hook chỉ là hàng rào chống nhầm
(D2-125.1).

## 7. Giới hạn đã biết (phần chưa đóng)

Trung thực về phạm vi — bốn hook là **rào cản cấu trúc + dấu vết**, **không** phải ranh giới
bảo mật:

- **`VIFAST_ROLE` chống nhầm, không chống cố ý (D2-125.1).** Người có quyền vẫn: `git commit
  --no-verify` (bỏ qua `pre-commit` + `commit-msg`), `git push --no-verify`, `git -c
  core.hooksPath=/dev/null …` (tắt **cả** `reference-transaction`), hoặc gỡ hook bằng
  `--uninstall`. Không có hook cục bộ nào chặn được người có quyền trên máy — và không nên cố.
- **`git reset --hard` bị từ chối vẫn có thể làm bẩn cây làm việc.** git ghi index + cây làm
  việc **trước** khi cập nhật ref; khi `reference-transaction` huỷ transaction thì **ref (lịch
  sử) nguyên vẹn** nhưng index/cây có thể đã ở bản của commit đích. Log ca âm có ghi lại đúng
  hiện tượng này (mục CA C-b). Cách sửa: lead đưa cây về bằng
  `VIFAST_ROLE=lead git reset --hard <sha-main>`. Đây là lý do phải kiểm **cả** `git status`
  sau một lần bị từ chối, không chỉ nhìn `rev-parse`.
- `git commit-tree` + `git update-ref` **trên máy khác**, `git push` từ **máy khác**, thao tác
  trực tiếp trên server: hook cục bộ không thấy. Chỉ bảo vệ nhánh trên GitHub (mục sáu) mới chặn.
- Hook không kiểm **nội dung diff**: không kiểm tệp có nằm trong vùng ghi của task hay không,
  không kiểm cổng xanh. Việc đó vẫn là của lead (`git show --stat`) và `v2 check`.
- `commit-msg` chỉ kiểm **subject**: message không nhãn nhưng nhãn nằm ở thân bài vẫn bị từ chối
  (cố ý — nhãn phải ở đầu), còn message có nhãn mà nội dung sai thì hook không biết. Các dạng
  `fixup!`/`squash!`/`amend!` **không** được miễn trừ.
- Hook chạy theo **môi trường của process gọi git**: terminal của lead đã `export
  VIFAST_ROLE=lead` thì mọi lệnh trong terminal đó đều qua — kể cả lệnh của agent nếu chung
  shell. Vì vậy luật "chỉ Agent 1 đặt biến" là luật **vận hành**, không phải thứ hook cưỡng chế.
- Người mới clone / máy khác: phải chạy `install.sh` (hook không nằm trong git); nếu git của máy
  đó cũ hơn ngưỡng ở §3.1 thì **không có** `reference-transaction` — lỗ hổng A1-183 vẫn còn ở đó.
- Branch protection GitHub: **chưa bật** (repo cục bộ) — mục sáu là việc phải làm, là hàng rào
  thật.

## 8. Gỡ / hoàn nguyên

```bash
v2/tools/git-guard/install.sh --uninstall   # gỡ cả bốn hook của vifast-guard (giữ hook lạ)
v2/tools/git-guard/install.sh --check       # xác nhận đã gỡ (báo CHƯA cài từng hook)
```

Muốn tạm thời tắt mà không gỡ: **không** đổi `core.hooksPath` (làm hook của người khác chết
theo); gỡ bằng `--uninstall` rồi cài lại sau. Nếu phải commit gấp với tư cách người thật, dùng
`--no-verify` cho commit/push — đúng quyền của người, và không để lại cấu hình lệch (lưu ý:
`--no-verify` **không** bỏ qua `reference-transaction`).
