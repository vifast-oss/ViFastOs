# KT-139 — Nối ba script tự kiểm **độc lập** vào cổng + bật `--standalone-red` vĩnh viễn

**Ngày**: 2026-11-18 · **Nguồn**: Agent 0 chốt — *"NỐI cả ba vào cổng, rồi bật `--standalone-red`
vĩnh viễn. Không thêm ngoại lệ."*
**Phạm vi ghi**: `v2/bin/v2`, `v2/agent/JOURNAL/kt139-journal.md`.
**Không** sửa `spec/`, không đổi ngưỡng, không sửa kỳ vọng có sẵn, không commit/push, không chạy QEMU.

---

## 0. Hiện trạng trước khi làm

`python3 v2/tools/check-selftest-wiring.py` ở HEAD `f1077036`:

```text
check-selftest-wiring: PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)
```

Nhưng dòng `PASS` đó chỉ nói về **lớp có cờ** (`--selftest`/`--red-selftest`). Mục
"BÁO CÁO — script tự kiểm độc lập (không cờ; KHÔNG tính mồ côi đỏ)" in ra **ba dòng `KHÔNG`**:

```text
| v2/test/rules/selfbreak.sh | KHÔNG | — |
| v2/tools/git-guard/neg-test-kt85b.sh | KHÔNG | — |
| v2/tools/git-guard/neg-test.sh | KHÔNG | — |
```

Đây đúng lớp lỗi đã làm `build-rev.sh --selftest` **đỏ âm thầm** (KT-128): viết xong, chạy tay một
lần, rồi để mục. Cổng xanh vì lớp này **không nằm trong phạm vi đỏ** — không phải vì nó không hỏng.

---

## 1. Ba script — thời gian **đo thật** và cổng nhận

Đo bằng `date +%s%N` trước/sau mỗi lần chạy (không đoán, không lấy số từ tài liệu):

| script | thời gian đo (ms) | vào cổng nào | vì sao |
|---|---|---|---|
| `v2/test/rules/selfbreak.sh` | **94.878** (lần 1, PASS) · **105.275** (worktree HEAD sạch, PASS) | `gate --fast` | 95–105 s ≫ 5 s. Nó **copy cả cây** 6 lần (`copy_repo` chép ~5.991 tệp đã `git ls-files`, riêng `v2/agent` ~120 MB) rồi chạy `verify-numbers.py` 5 luật trên từng bản copy. Không cần QEMU, nhưng đặt vào `check` là làm `check` chậm gấp nhiều lần. |
| `v2/tools/git-guard/neg-test.sh` | **12.219** (PASS) | `gate --fast` | 12 s > 5 s. Nó `git worktree add` một worktree tạm, tạo commit, `git init --bare` trong `/tmp`, push thử, rồi dọn + kiểm bất biến HEAD/index/số worktree. Có **ghi** lên repo (worktree + nhánh tạm) ⇒ không được chen vào `check`. |
| `v2/tools/git-guard/neg-test-kt85b.sh` | **26.320** (PASS) | `gate --fast` | 26 s > 5 s. Nó `git clone --local` cả repo vào `/tmp` (hardlink object) rồi cài hook vào clone, thử cherry-pick/merge/reset/commit-msg, cộng một PROBE `update-ref` trên repo thật. Nặng nhất trong hai script git-guard. |

**Nguyên tắc phân loại đã dùng** (đúng đề bài): nhanh (< ~5 s, không QEMU) ⇒ `check`; chậm (QEMU,
dựng ảnh, hoặc > ~5 s) ⇒ `gate --fast`. **Cả ba đều chậm** ⇒ cả ba vào `gate --fast`. Không script
nào cần QEMU.

### 1.1 Hai lần đo bị nhiễu — ghi lại trung thực

**(a) `neg-test.sh` lần đầu ĐỎ, không phải vì script hỏng.** Lần đo đầu (10.691 ms) kết thúc:

```text
KẾT LUẬN: FAIL — số worktree KHÁC trước (30 → 16)
TỔNG KẾT: FAIL — 1/7 ca sai kỳ vọng
```

Nguyên nhân: repo đang có **14 mục worktree rác** (đăng ký còn, thư mục đã bị xoá, di sản của các
task trước). Chính `cleanup_trap` của script gọi `git worktree prune` ⇒ 14 mục rác biến mất ⇒ bất
biến "số worktree trở về như trước" sai. Chạy lại ngay sau đó (12.219 ms): **PASS 7/7**, `16 → 16`.
Đây là **hàng rào thật** của script (nó bắt được rác), không phải flake — nhưng nó có nghĩa: **một
mục worktree rác còn sót là `gate --fast` ĐỎ**. Ghi ở §6.

**(b) `selfbreak.sh` ĐỎ trong working tree hiện tại — do task song song, không do KT-139.** Xem §6.1.

---

## 2. Đã nối vào cổng (chỉ **thêm** dòng)

`v2/bin/v2`, hàm `do_gate_fast`, ngay sau bước `selftest persist đỏ-trước (KT-75)`:

```bash
run_step "selfbreak 5 luật K6 (KT-80)" bash "$V2_ROOT/test/rules/selfbreak.sh" || return 1
run_step "ca âm git-guard (KT-85)" bash "$V2_ROOT/tools/git-guard/neg-test.sh" || return 1
run_step "ca âm git-guard b (KT-85b)" bash "$V2_ROOT/tools/git-guard/neg-test-kt85b.sh" || return 1
```

Đúng khuôn có sẵn của `do_gate_fast` (`run_step "<nhãn>" bash "$V2_ROOT/..." || return 1`).
Kèm chú thích tiếng Việt giải thích **vì sao** (chậm ⇒ `gate --fast`; `selfbreak.sh` là đường duy
nhất chạm `--self-test`; hai script git-guard cần hook đã cài).

**Không sửa dòng nào của người khác.** Bằng chứng (`diff` bản gốc ↔ bản sau):

```text
=== dòng bị XOÁ so với bản gốc (chỉ được có 1: dòng thêm cờ) ===
<         run_step "cổng selftest có cổng gọi (KT-134)" "$PY" "$V2_ROOT/tools/check-selftest-wiring.py" || fails=$((fails + 1))
=== số dòng thêm ===
20
```

Đúng **một** dòng bị thay: dòng `check-selftest-wiring.py` trong `do_check` — chính dòng đề bài bảo
thêm cờ (§3). 20 dòng còn lại là **thêm mới** (chú thích + ba lời gọi). Mọi dòng khác của
`v2/bin/v2` còn nguyên.

---

## 3. `--standalone-red` bật **vĩnh viễn** trong `v2 check`

`do_check`, dòng gọi `check-selftest-wiring.py` (bản không `--selftest`) nay:

```bash
run_step "cổng selftest có cổng gọi (KT-134)" "$PY" "$V2_ROOT/tools/check-selftest-wiring.py" --standalone-red || fails=$((fails + 1))
```

**Vì sao bật vĩnh viễn**: công cụ có **hai** lớp. Lớp có cờ vốn đã đỏ khi mồ côi; lớp **độc lập**
(không cờ, nhận theo tên tệp) trước đây **chỉ báo cáo** ⇒ ba script mồ côi nằm im mà cổng vẫn xanh.
Bật cờ để **script thứ tư** rơi vào lớp này là ĐỎ ngay, không chờ ai rà tay.

**Không thêm ngoại lệ nào** — `NGOAI_LE` vẫn rỗng, và công cụ tự xác nhận:

```text
NGOẠI LỆ (có lý do — không tính mồ côi)
- (không có ngoại lệ nào được dùng)
```

### 3.1 Kết quả sau khi nối — dòng tổng kết nguyên văn

```text
check-selftest-wiring: PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)
```

Và cả **10** script lớp độc lập nay đều `CÓ`:

```text
BÁO CÁO — script tự kiểm độc lập (không cờ; ĐÃ tính mồ côi đỏ)
| script | cổng gọi? | cổng nào |
|---|---|---|
| v2/test/abi/run_fixtures.sh | CÓ | v2/bin/v2:332 |
| v2/test/boot/guard_selftest.sh | CÓ | v2/bin/v2:404 |
| v2/test/boot/selftest.sh | CÓ | v2/bin/v2:403 |
| v2/test/gen/run_fixtures.sh | CÓ | v2/bin/v2:342 |
| v2/test/gen/run_idl_fixtures.sh | CÓ | v2/bin/v2:358 |
| v2/test/report/run_fixtures.sh | CÓ | v2/bin/v2:322 |
| v2/test/rules/selfbreak.sh | CÓ | v2/bin/v2:417 |
| v2/test/scn/run_fixtures.sh | CÓ | v2/bin/v2:308 |
| v2/tools/git-guard/neg-test-kt85b.sh | CÓ | v2/bin/v2:423 |
| v2/tools/git-guard/neg-test.sh | CÓ | v2/bin/v2:422 |
```

Bước này cũng đã chạy **bên trong cổng** (`./v2/bin/v2 gate --fast`, log `/tmp/gate-fast-kt139.log`):

```text
--- cổng selftest có cổng gọi (KT-134)
...
check-selftest-wiring: PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)
--- cổng selftest có cổng gọi (KT-134): OK
```

---

## 4. Bằng chứng `verify-numbers.py --self-test` nay được cổng chạm tới

### 4.1 `selfbreak.sh` **thật sự gọi** `--self-test` — không phải chỉ nhắc trong chú thích

Hai lời gọi thật trong `case_law8` (dòng 234 và 250), cả hai đều là **lệnh chạy**, không phải chú thích:

```bash
233:    # đối chứng dương: verifier nguyên bản ⇒ self-test XANH (13/13 ca ĐỎ đúng mã).
234:    "$PY" "$d/v2/tools/verify-numbers.py" --root "$d" --self-test >/dev/null 2>&1 || {
...
250:    out="$("$PY" "$d/v2/tools/verify-numbers.py" --root "$d" --self-test 2>&1)"
```

### 4.2 Bằng chứng **thực thi** — `bash -x` bắt được đúng câu lệnh đã chạy

`bash -x v2/test/rules/selfbreak.sh`, lọc các dòng **thực thi** (`+ `) chạm `--self-test`:

```text
663:+ /opt/homebrew/bin/python3.12 /var/folders/.../kt80-selfbreak.96zZQ5/case-5/v2/tools/verify-numbers.py --root /var/folders/.../kt80-selfbreak.96zZQ5/case-5 --self-test
```

(Lời gọi ở dòng 234 không hiện trong trace vì chính nó có `>/dev/null 2>&1` — trace của bash đi
vào stderr của lệnh đó. Lời gọi ở dòng 250 thì hiện, và **đó là lời gọi quyết định**: nó phải ĐỎ
với mã `E8_CASE_GREEN`.)

### 4.3 Bằng chứng **ngữ nghĩa** — ca 5 chỉ xanh nếu `--self-test` chạy thật

`case_law8` làm hai bước: (i) `--self-test` trên verifier **nguyên bản** phải XANH; (ii) làm yếu
verifier (miễn số thập phân `x.y`) rồi `--self-test` phải **ĐỎ đúng mã `E8_CASE_GREEN`**. Không
chạy `--self-test` thì không bước nào có kết quả, và ca 5 không thể xanh. Kết quả thật:

```text
    đối chứng dương: self-test XANH trên verifier nguyên bản
ok 5 - luat 8: verifier bị làm yếu ⇒ self-test ĐỎ (E8_CASE_GREEN)
```

Và `--self-test` của verifier chạy **21 ca** (log cổng, bước `luật máy K6`):

```text
ok - luật 8: cases=21 (12927 ms)
```

**Kết luận**: chuỗi `gate --fast` → `selfbreak.sh` → `verify-numbers.py --self-test` **thông**.
Trước KT-139, `selfbreak.sh` không có cổng nào gọi ⇒ `--self-test` **không cổng nào chạm tới**.

---

## 5. Đột biến — chứng minh đỏ **thật**

Gỡ **một** dòng gọi (dòng `selfbreak.sh`) khỏi `v2/bin/v2`, rồi chạy
`python3 v2/tools/check-selftest-wiring.py --standalone-red`. Exit **1**. Thông điệp đỏ **nguyên văn**:

```text
MỒ CÔI (ĐỎ)
- E_ORPHAN v2/test/rules/selfbreak.sh: script tự kiểm độc lập, không cổng nào gọi (đã quét v2/bin/v2 + v2/test/rules/ (9 tệp))

BÁO CÁO — script tự kiểm độc lập (không cờ; ĐÃ tính mồ côi đỏ)
| script | cổng gọi? | cổng nào |
|---|---|---|
| v2/test/abi/run_fixtures.sh | CÓ | v2/bin/v2:332 |
| v2/test/boot/guard_selftest.sh | CÓ | v2/bin/v2:404 |
| v2/test/boot/selftest.sh | CÓ | v2/bin/v2:403 |
| v2/test/gen/run_fixtures.sh | CÓ | v2/bin/v2:342 |
| v2/test/gen/run_idl_fixtures.sh | CÓ | v2/bin/v2:358 |
| v2/test/report/run_fixtures.sh | CÓ | v2/bin/v2:322 |
| v2/test/rules/selfbreak.sh | KHÔNG | — |
| v2/test/scn/run_fixtures.sh | CÓ | v2/bin/v2:308 |
| v2/tools/git-guard/neg-test-kt85b.sh | CÓ | v2/bin/v2:422 |
| v2/tools/git-guard/neg-test.sh | CÓ | v2/bin/v2:421 |

check-selftest-wiring: FAIL (1 script tự kiểm không cổng nào gọi: v2/test/rules/selfbreak.sh)
```

Đỏ **đúng tên script bị gỡ** (`v2/test/rules/selfbreak.sh`), và chỉ nó — hai script git-guard vẫn
`CÓ`. Đột biến **không** đỏ vì lý do khác.

### 5.1 Khôi phục — sha256 về đúng như trước đột biến

| mốc | sha256 `v2/bin/v2` |
|---|---|
| gốc (HEAD `f1077036`, trước mọi sửa) | `8f8264eb4c511a9683f3e4737dc9389baca3cb533582b8591514c33206ed800f` |
| sau khi nối (trước đột biến) | `79b7c294f199ea239a7faef0d5aa8e95c22a86e5f06f95b888fd4f4646890bb7` |
| **sau đột biến** (đã gỡ 1 dòng) | `5f27f6385da92a886ce84f62d7db13b069ff92008489c2d48cedda2cb0ed82d9` |
| **sau khôi phục** | `79b7c294f199ea239a7faef0d5aa8e95c22a86e5f06f95b888fd4f4646890bb7` ✔ **trùng mốc "sau khi nối"** |

Sau khôi phục: `bash -n v2/bin/v2` OK, và
`check-selftest-wiring.py --standalone-red` trở lại
`PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)`.

---

## 6. Trạng thái cổng đầy đủ + các điều kiện **phải biết**

### 6.1 `gate --fast` hiện **ĐỎ ở một bước có từ TRƯỚC**, không liên quan KT-139

`./v2/bin/v2 gate --fast` (76.807 ms) dừng ở `do_check` với **1 bước FAIL**:

```text
--- bảng đối chiếu số ↔ khối scope= (KT-134)
not ok - v2/docs/k6-exec.md: E_AMBIGUOUS_FIELD: `observed` có 2 giá trị khác nhau ...
not ok - v2/docs/k6-persist.md: E_AMBIGUOUS_FIELD: `ok` có 2 giá trị khác nhau ...
# check-scope-table: FAIL (6 lỗi)
--- bảng đối chiếu số ↔ khối scope= (KT-134): FAIL
...
v2 check: 1 bước FAIL
```

**Đây là lỗi có sẵn, không do KT-139.** Bằng chứng: chạy `check-scope-table.py` trên **worktree
sạch ở HEAD** (`git worktree add --detach … HEAD`) cũng `EXIT=1` với **đúng 6 lỗi đó**. Công cụ này
chỉ đọc `v2/docs/*` + `v2/agent/reports/*`; KT-139 chỉ sửa `v2/bin/v2`. Vì `do_check` đỏ, `do_gate_fast`
`return 1` ngay ⇒ **cổng đầy đủ không đi tới ba bước mới** trong lần chạy này.

Vì vậy ba bước mới được chứng minh bằng cách chạy **nguyên văn ba dòng `run_step`** trích từ
`v2/bin/v2` trong một harness có `run_step` y hệt bản của cổng (143.511 ms):

```text
--- selfbreak 5 luật K6 (KT-80)
--- selfbreak 5 luật K6 (KT-80): FAIL        ← xem §6.2
--- ca âm git-guard (KT-85)
--- ca âm git-guard (KT-85): OK
--- ca âm git-guard b (KT-85b)
--- ca âm git-guard b (KT-85b): OK
```

### 6.2 `selfbreak.sh` ĐỎ **trong working tree hiện tại** — do task song song KT-140

```text
not ok 1 - luat 1: số dạng key=value ở prose ⇒ ĐỎ (E1_NUMBER_ASSIGNED)
    (kỳ vọng XANH nhưng exit=1)
    not ok - luat 1: v2/docs/k6-persist.md:227: E1_SCOPE_MISSING [no-git] `kt140-persist-red/b02.log`: không thấy log [kt140-persist-red/b02.log] trong repo
    not ok - luat 1: v2/docs/k6-persist.md:220: E1_NUMBER_OUTSIDE ...
    not ok - luat 1: 11 lỗi (docs_checked=17 pre_existing=0 excluded=1)
# selfbreak: 1 lỗi / 5 ca
```

Nguyên nhân **không phải** KT-139: `v2/docs/k6-persist.md` đang bị **task khác sửa** (`M`, chưa
commit) và trích số đo từ `v2/agent/JOURNAL/kt140-persist-red/` — thư mục đó **chưa `git add`**
(`??`). `copy_repo` của `selfbreak.sh` **cố ý** chỉ chép tệp đã có trong index (D2-105a: "không
chép bản nháp chưa `git add` của task song song") ⇒ bản copy thiếu log ⇒ luật 1 ĐỎ trên bản copy.

**Chứng minh script lành**: chạy **cùng script** trên worktree sạch ở HEAD:

```text
HEAD_RUN rc=0 ms=105275
ok 1 - luat 1: số dạng key=value ở prose ⇒ ĐỎ (E1_NUMBER_ASSIGNED)
ok 2 - luat 2: báo cáo commit trước log ⇒ ĐỎ (E2_ORDER)
ok 3 - luat 3: bộ đếm proven=false nằm trong khối ok ⇒ ĐỎ (E3_UNPROVEN_IN_OK)
ok 4 - luat 7: bản trích bỏ một cột ⇒ ĐỎ (E7_COLUMN_MISMATCH)
ok 5 - luat 8: verifier bị làm yếu ⇒ self-test ĐỎ (E8_CASE_GREEN)
# selfbreak: 5/5 ca phá ĐỎ đúng mã; cây thật không đổi
```

⇒ `selfbreak.sh` **chạy được** trong cổng; nó chỉ đỏ khi cây làm việc **tự mâu thuẫn** (tài liệu
đã sửa trích log chưa `git add`). Đỏ đó là **đúng** — nhưng nó là **hàng rào liên-task**, không
phải hàng rào của KT-139. Ghi ở §7.

### 6.3 Điều kiện tiên quyết của hai script git-guard

Cả hai gọi `v2/tools/git-guard/install.sh --check` trước khi làm gì, và **exit 2** nếu hook chưa cài:

```text
neg-test.sh: hook CHƯA cài đúng — chạy v2/tools/git-guard/install.sh trước
```

Hook **không đi theo git** (`v2/docs/k6-git-guard.md` §"hook không đi theo git") ⇒ trên một bản
clone mới, `gate --fast` sẽ ĐỎ ở hai bước này cho tới khi ai đó chạy
`bash v2/tools/git-guard/install.sh`. Trong repo này hook **đã cài đúng** (`install.sh --check` →
`PASS — check đạt (pre-commit pre-push commit-msg reference-transaction)`), nên cả hai chạy xanh.

### 6.4 R2 — `selfbreak.sh` đỏ khi cây làm việc đang bị task khác sửa dở: **KHÔNG phải script hỏng**

Agent 0 chấp nhận giải thích này (phiếu lượt 4, mục "Hàng rào", R2) và yêu cầu **ghi vào
JOURNAL** để lần sau không ai đọc log đỏ rồi kết luận script hỏng. Ghi ở đây.

**Cơ chế.** `selfbreak.sh` không chạy verifier trên cây thật; nó dựng một **bản copy tối thiểu**
rồi chạy năm luật trên bản copy ấy. Danh sách tệp chép lấy từ `git ls-files` — tức **chỉ tệp đã
có trong index**. Hệ quả:

- Tệp **đã tracked** mà task khác đang sửa dở: **vẫn được chép**, kèm nội dung mới trong cây làm
  việc. Không gây đỏ.
- Tệp **mới, chưa `git add`** (điển hình: log bằng chứng mới, hoặc tài liệu mới): **không** được
  chép. Nếu một tài liệu đã tracked trích khối ```` ```text scope=<log> ```` trỏ tới tệp mới ấy,
  bản copy có tài liệu mà **không** có log ⇒ luật 1 báo `E1_SCOPE_LOG_MISSING` ⇒ `selfbreak.sh`
  đỏ.

**Vì sao đỏ đó là ĐÚNG, không phải hỏng.** Bản copy cố ý chỉ chép tệp trong index (D2-105a): nếu
chép cả bản nháp chưa `git add` của task khác thì phép thử sẽ phụ thuộc vào trạng thái dở dang của
task ấy, và hai task chạy song song sẽ làm nhau đỏ/xanh thất thường. Đỏ ở đây nói đúng một điều:
**cây làm việc đang tự mâu thuẫn** — tài liệu đã trích một log chưa vào repo.

**Cách phân biệt "đỏ đúng" với "script hỏng".** Đọc thông điệp: nếu là `E1_SCOPE_LOG_MISSING` (hoặc
cùng họ `E1_*` chỉ vào một tệp **chưa `git add`**), đó là hàng rào liên-task — `git add` tệp ấy
rồi chạy lại. Nếu là lỗi cú pháp, `command not found`, hay exit khác 0 mà **không** có mã `E1_*`,
đó mới là script hỏng. Không có ngoại lệ nào được thêm vào script để né trường hợp này.

---

## 7. Tổng kết — cái gì đã xong, cái gì là **hàng rào cần biết**

**Xong:**

1. Ba script mồ côi đã **có cổng gọi** (`gate --fast`), đúng khuôn `run_step … || return 1`.
2. `--standalone-red` **bật vĩnh viễn** trong `v2 check`; **0 ngoại lệ**, **0 mồ côi**.
3. `verify-numbers.py --self-test` nay **được cổng chạm tới** qua `selfbreak.sh` (chứng minh ở §4).
4. Đột biến gỡ một dòng ⇒ **ĐỎ đúng tên**; khôi phục ⇒ sha256 trùng.
5. Chỉ **thêm** dòng vào `v2/bin/v2`; đúng **một** dòng bị thay là dòng đề bài bảo thêm cờ.

**Hàng rào cần biết (không phải ngoại lệ, không được nới):**

| # | Hàng rào | Hệ quả |
|---|---|---|
| R1 | `check-scope-table.py` **đỏ sẵn ở HEAD** (6 `E_AMBIGUOUS_FIELD` trong `v2/docs/k6-exec.md`, `v2/docs/k6-persist.md`) | `gate --fast` đỏ ở `do_check` **trước** ba bước mới ⇒ ba bước chưa chạy được trong một lần `gate --fast` trọn vẹn cho tới khi task sở hữu hai tài liệu đó sửa xong. **Không** sửa hộ (không thuộc phạm vi KT-139). |
| R2 | `selfbreak.sh` đỏ khi có task song song sửa tài liệu trích log **chưa `git add`** | `gate --fast` đỏ trong lúc task khác đang làm dở. Đây là **đúng** theo D2-105a, nhưng là hàng rào **liên-task**: chạy cổng khi cây làm việc đang bị task khác sửa dở sẽ đỏ. **Không phải script hỏng** — cơ chế và cách phân biệt ở §6.4 (Agent 0 chấp nhận, lượt 4). |
| R3 | `neg-test.sh` đỏ nếu còn **mục worktree rác** (đăng ký còn, thư mục mất) | Lần chạy đầu trong repo này đỏ vì 14 mục rác; chính script `prune` chúng. Sau đó xanh. |
| R4 | Hai script git-guard cần **hook đã cài** (`install.sh`) | Bản clone mới: `gate --fast` đỏ kèm thông điệp rõ cho tới khi cài hook. |
| R5 | `neg-test.sh` dùng **tên nhánh cố định** `task/kt85neg` | Hai `gate --fast` chạy **song song** sẽ đụng nhánh ⇒ một bên đỏ. Cổng vốn không được chạy song song (AGENTS.md §6: nightly/soak không chạy song song với cổng khác). **ĐÃ VÁ ở KT-142** (Agent 0 lượt 4): tên nhánh nay là `task/kt85neg-<pid>-<epoch>`, và `gate --fast` có thêm ca `--selftest-parallel` chạy đúng hai lần song song, đòi cả hai PASS. |

**Không thêm ngoại lệ nào.** Không nới ngưỡng, không sửa kỳ vọng có sẵn, không commit, không push,
không chạy QEMU.
