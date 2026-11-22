# KT-333e — cắt thời gian cổng (1 lượt `wake`) + gom kho (phiếu gốc & raw `.log`)

**Task**: `KT-333e` (D2-247 mục 3b) · **Ngày**: 2026-11-23 (JST) · **Nhánh**: `task/kt333e-clean`
(base `main` @ `444975b9`, sau `KT-333d` `ecb97e91`/`444975b9`) · **Worktree**: `/tmp/kt333e-s1`
· **Quy chế**: D2-245/D2-248 — sub-agent **chỉ T0 + T1** (`gate --dev`), **không** `gate --fast`;
`spec/`, 19 tệp `[[legacy]]`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `.github/**` **không** sửa.

Mục tiêu (3 hạng mục, đúng thứ tự A → B → C):

| Mục | Yêu cầu | Kết quả |
|---|---|---|
| **A** | Gộp `check-wake-paths.py` (normal + `--strict`) thành **một lượt quét**, giữ **toàn bộ** luật | ✅ `--combined`, `v2 check` 2 bước → 1 bước; **−38,8 s CPU/cổng** (số thật, xem §1.4) |
| **B** | Gom phiếu/báo cáo cũ ở gốc repo → `v2/agent/archive/reports/` | ✅ **20 tệp** (19 báo cáo `.md` + 1 tổng hợp `.txt`); gốc repo không còn tệp phiếu |
| **C** | Tách raw `.log` Chu kỳ 1 ngoài khối `scope=` sang nhánh/tag `cycle1-raw-logs`, xoá khỏi cây | ✅ **7.546** `.log` → tag `cycle1-raw-logs`; **222** `.log` giữ lại (119 nguồn `scope=`/`scope-red=` + 99 đích liên kết tài liệu đang dùng + 8 log đặc biệt − giao 0, cộng 1 log được giữ vì tài liệu đã lưu trữ trỏ tới); repo 10.373 → **2.827** tệp |

---

## 1. A — `check-wake-paths.py`: một lượt quét cho cả hai chế độ

### 1.1 Khảo sát trước (theo yêu cầu: gọi mấy lần, mỗi lần bao lâu)

- **Số lần gọi trong cổng**: `v2 check` đăng ký **3** bước dùng checker này
  (`v2/bin/lib/check.sh`): id **20** `wake paths` (`step_wake_paths`), id **21** `selftest wake paths`,
  id **22** `wake paths --strict` (`step_wake_paths_strict`). Như vậy mỗi cổng **đọc cùng một cây
  hai lần** (một lượt thường + một lượt `--strict`), cộng một lượt selftest.
- **Đo `# step-time` lịch sử** (`v2/agent/JOURNAL/kt242-lanes-timing.md`, máy rảnh):
  `wake paths` **27,5 s** (có lượt 27,2 / 40,3 s), `wake paths --strict` **27,5 s** (28,1 / 36,6 s),
  `selftest wake paths` **38,2–57,0 s** ⇒ ba bước ≈ **95–130 s tường** mỗi lượt cổng.
  D2-245 ghi **186,5 s CPU** cho bước 20–22 tại `bfb9a13d`.
- **Đo lại trên worktree này** (`/usr/bin/time -p`, cùng cây, trước khi sửa; interpreter
  `python3` = 3.9, máy đang có 3 sub-agent + lead chạy T2 nên wall bị nhiễu):
  `normal` **51,49 s CPU** (wall 61,3), `--strict` **52,65 s CPU** (wall 59,9),
  `--selftest` **47,05 s CPU** (wall 54,2) — **452/452** ca.

⇒ Kết luận khảo sát: hai bước 20/22 là **hai lượt đọc/parse/token-hoá cùng một cây** (381 tệp
`v2/kernel`+`v2/sim`+`v2/lib`), phần lớn công việc **trùng nhau**; chỉ khác **khối luật `wake`**
và kiểm `extra_doors` (chỉ `--strict`).

### 1.2 Thiết kế: `--combined` = một lượt quét, hai bảng kết quả

`check-wake-paths.py` thêm cờ **`--combined`** (và tham số `combined=True` cho `scan_v2`):

- **Phần chung chạy đúng một lần**: đọc tệp, token hoá (`tokenize`/`build_tree`), `find_functions`,
  `_test_char_ranges`, đồ thị module (`test_only_modules`), `include!`/`#[path]`
  (`resolve_include_targets`), luật cấu trúc (`scan_lexical`/`scan_structure`/`scan_structure_scope`),
  luật kênh 1–5, `_check_state_write`, `_check_type_anchors`/`_check_nonready_conv`, `include!`/`#[path]`
  trong thân macro (`scan_macro_bodies`).
- **Chụp report chung** (`normal_rep = bản sao report`) ngay **sau** luật chung cuối cùng; từ đó chỉ còn
  hai khối phụ thuộc chế độ chạy **nguyên văn** như trước:
  - khối `strict` (luật token `wake` đầy đủ + phạm vi `token_scope` + `extra` + `extra_doors`);
  - khối `not strict` (luật token `wake` cho tệp sản phẩm, dùng `extra`/`scope_targets`).
  Ở chế độ gộp, khối `strict` ghi vào report **strict**, khối `not strict` ghi vào report **thường**;
  luật chung cuối (thân macro) ghi vào **cả hai** qua `_Tee` (chỉ chuyển tiếp `err/warn/note`).
- **In hai bảng như hai lượt riêng** (cùng thứ tự `#`/`W`/`E`/dòng tổng `mode=… strict=0|1`), đỏ nếu
  **một trong hai** đỏ. Vì vậy log cổng **so từng dòng** được với bản trước.
- `--combined` + `--strict` cùng lúc ⇒ dùng sai (exit 2). `--combined --json` in
  `{"combined": true, "normal": …, "strict": …}`. v1 (`crates/`) không có luật riêng theo `strict`:
  nhánh gộp chạy hai lượt `scan_v1` như cũ (rẻ), **không** trộn gì của v1 vào v2.

**Nối vào cổng** (`v2/bin/lib/check.sh`, `v2/bin/lib/steps_check.sh`, `v2/bin/lib/consts.sh`):
bước **20** = `wake paths + --strict (… KT-333e: một lượt)` gọi `--combined`; bước **22** (bản riêng
`--strict`) **gộp vào 20**; bước 21 (selftest) giữ nguyên. `CHECK_STEPS_EXPECTED` **45 → 44**
(T2 **76 → 75**), là thay đổi **có chủ ý**, không bỏ luật nào (xem bằng chứng §1.3).

### 1.3 Bằng chứng **cùng độ phủ** (không nới ngưỡng, không bỏ luật)

1. **Xuất ra trên cây thật trùng từng byte với hai lượt cũ**:
   `cat <output normal> <output --strict>` vs `<output --combined>` ⇒ `diff` **rỗng**
   (`BYTE-IDENTICAL`), kiểm **hai lần** với hai interpreter: `python3` 3.9 và `python3.12`
   (interpreter của cổng). Hai bảng đều `errors=0 warnings=0`, giữ đủ các dòng `# kênh …`/`# kiểu thiết kế`.
2. **Selftest: 452/452 cũ giữ nguyên, +9 ca mới = 461/461** (`# selftest: 461/461 ok`).
   5 ca mới chứng minh **tính tương đương** của chế độ gộp: trên **cùng** cây, `errors/warnings/info`
   của lượt gộp phải **bằng đúng** lượt riêng tương ứng — chạy cho cây xanh, cây đỏ luật arch
   (`IpiKind::Wake`), cây đỏ **chữ `doorbell`** trong `vifast-plat-*/` (KT-267(b), bài học KT-338c),
   cây đỏ luật token `wake(`, và cây có `extra_doors` (luật **chỉ** `--strict`: `E_STRICT_STALE`
   **không** rò sang bảng thường).
3. **Rà 38 mã lỗi của checker** (`r.err("E_…")`): trước bản này **5 mã không xuất hiện** trong
   `selftest()` — `E_CHANNEL_MISSING`, `E_CONSUMER_MISSING`, `E_LEX_UNTERMINATED_CHAR`,
   `E_LEX_UNKNOWN`, `E_V1_NO_CHANNEL`. Nay thêm **4 ca âm** cho 4 mã đầu-cuối
   (`E_LEX_UNKNOWN` là nhánh **không thể tới**: `punct` nuốt mọi ký tự, có `# pragma: no cover` —
   ghi rõ lý do, không dựng ca giả). Các mã còn lại **đã có** ca (kể cả `E_ARCH_DOORBELL` 5 ca:
   `IpiKind::Wake`, chữ `doorbell` trong mã/chú thích, `virtio_evil.rs` không phải driver virtio).
4. **Cổng**: `v2 check` **PASS**, `# gate-steps registered=44 ran=44 skipped=0 failed=0`;
   bảng `selftest có cổng gọi` vẫn thấy `check-wake-paths.py | CÓ` ở `steps_check.sh:57`.
   `verify-numbers.py` (`ok=5 fail=0`) và `check-scope-table.py` (PASS) trước/sau **không đổi**.

### 1.4 Đo trước/sau (số thật, nói thẳng phần không đạt)

Interpreter **`python3.12`** (đúng interpreter cổng dùng), cùng cây, cùng máy (đang có 3 sub-agent +
lead chạy T2 ⇒ wall bị nhiễu; **CPU** mới là số so sánh được):

| Lượt | CPU (user+sys) | wall |
|---|---:|---:|
| `wake paths` (riêng) | 41,95 s | 96,0 s |
| `wake paths --strict` (riêng) | 42,41 s | 68,4 s |
| **Cộng hai lượt cũ** | **84,36 s** | — |
| `--combined` (một lượt) | **45,57 s** | 154,2 s |

⇒ **−38,8 s CPU/cổng (−46,0 %)**, đạt **92 %** của **trần lý thuyết** của phép gộp (một lượt đầy đủ
≈ 42,4 s CPU). Đo bằng `python3` 3.9 (lượt đo đầu, cùng tải): 51,49 + 52,65 = 104,14 → 54,98 s
⇒ **−49,2 s CPU**.

**Không đạt mục tiêu −60 s**, lý do **định lượng**: D2-245 ước −55…−60 s từ 186,5 s CPU cho **ba**
bước tại `bfb9a13d`, tức ngầm định mỗi lượt ~70 s CPU. Trên cây/máy này một lượt chỉ **42 s CPU**
(python3.12), nên trần của "gộp hai lượt thành một" là **một lượt ≈ 42 s** — không thể tới 60 s nếu
không cắt chính lượt chung hoặc bước **selftest** (bước 21, ~48 s CPU, DoD **giữ 452/452** nên không
đụng). Phần còn lại +2,3 s CPU của lượt gộp so với một lượt `--strict` là **lượt quét token `wake`
thứ hai** — giữ lại **có chủ ý** để hai bảng vẫn là kết quả của **hai khối luật nguyên văn**, không
phải suy diễn "strict ⊇ thường" (đổi lấy ~2 s để không phải chứng minh tương đương bằng lập luận).

Số trong cổng `v2 check` lần này (tải cao, **wall** nên chỉ để tham chiếu):
`# step-time wake paths + --strict … 265.377 ms`, `# step-time-total … TỔNG check 316.019 ms`;
hai bước cũ trong log lịch sử máy rảnh là 27,5 + 27,5 s **wall** ⇒ không so trực tiếp được.

---

## 2. B — Gom phiếu/báo cáo cũ ở gốc repo

- **Hiện trạng đo được**: gốc repo có **20** tệp thuộc nhóm cần gom — **19** phiếu/báo cáo `.md`
  (5 `BAO-CAO-*` + 9 `PHIEU-GUI-AGENT0-*` + 1 `PHIEU-NGAN-API-*` + 3 `PHIEU-TRA-LOI-*` +
  1 `PHIEU-TRINH-*`) + **1** `ViFastOs-architecture-summary.txt`. **Không** có tệp
  `REVIEW-AGENT1-*`/`BAN-GIAO-*` nào ở gốc. (Kế hoạch ghi "22 phiếu": số **thực đo** là 20; chênh do
  Kế hoạch lấy số ở thời điểm khác — ghi rõ ở đây thay vì sửa số cho khớp.)
- **Khác biệt so với mô tả task**: các tệp này **đã được theo dõi Git** (commit từ các vòng trước,
  vd `f87cb8d5`), nên `git status --porcelain | grep '^??'` **rỗng** ngay từ đầu — dùng `git mv`
  (không phải di chuyển tệp chưa theo dõi). Cây làm việc khởi đầu **sạch**.
- **Đã chuyển** (`git mv`, tên tệp **giữ nguyên**) → `v2/agent/archive/reports/`:
  `BAO-CAO-CUOI-TUAN-KT242-2026-11-20.md`, `BAO-CAO-DEM-2026-11-19-VONG2.md`,
  `BAO-CAO-DEM-2026-11-20.md`, `BAO-CAO-QUA-DEM-2026-11-19.md`,
  `BAO-CAO-THI-HANH-LUOT4-2026-11-18.md`, `PHIEU-GUI-AGENT0-2026-11-19.md`,
  `PHIEU-GUI-AGENT0-2026-11-19-VONG2..VONG7.md` (6), `PHIEU-GUI-AGENT0-2026-11-20-VONG8.md`,
  `PHIEU-GUI-AGENT0-2026-11-21-KT241.md`, `PHIEU-NGAN-API-2026-11-20.md`,
  `PHIEU-TRA-LOI-AGENT0-2026-11-18-{C1-C7,LUOT2,LUOT3}.md`,
  `PHIEU-TRINH-AGENT0-2026-11-18-C1-C7.md`, `ViFastOs-architecture-summary.txt`.
- **Giữ nguyên ở gốc** (đúng yêu cầu): `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md` (báo cáo đang dùng của
  lead), `KE-HOACH-*` (3), `AGENTS.md`, `README.md`, `THIRD-PARTY.md`, `TRADEMARKS.md`, `VISCRIPT.md`.
- **Tham chiếu**: thêm `v2/agent/archive/reports/README.md` (nguồn, ngày, danh mục, cách khôi phục).
  - **Liên kết markdown**: 1 liên kết ở `README.md` gốc (`BAO-CAO-QUA-DEM-2026-11-19.md`) đã trỏ
    đường dẫn mới; **8** tệp đã chuyển có **12** liên kết tương đối kiểu `v2/agent/…` được sửa tiền tố
    `../../`; 3 liên kết giữa các tệp cùng thư mục vẫn đúng (đã kiểm `[ -e ]` từng đích: **0 thiếu**).
  - **Tham chiếu dạng chữ** trong `v2/agent/{QUEUE,DECISIONS}.md` + `JOURNAL/` (bản ghi lịch sử,
    **cấm sửa** theo luật task) vẫn truy được vì **tên tệp không đổi** và nay nằm ở
    `v2/agent/archive/reports/` — đã ghi rõ trong README của kho lưu trữ.
- **`[[legacy]]`**: `numbers-policy.toml` có **19** tệp ghim sha256, **0** tệp ở gốc repo
  (toàn bộ dưới `v2/`), nên **không** mục nào bị đụng; `verify-numbers.py` chỉ đọc
  `v2/agent/reports/*.md` + `v2/docs/k6-*.md` (không đệ quy) nên thư mục `archive/reports/` **không**
  vào bảng `scope=`.
- **Kết quả**: gốc repo không còn tệp phiếu/báo cáo cũ; `git status --porcelain` **không** còn mục
  `??` ở gốc (chỉ còn `v2/agent/archive/reports/README.md` là tệp mới **có chủ ý** của task).

---

## 3. C — Tách raw `.log` Chu kỳ 1/2 ngoài khối `scope=`

### 3.1 Khảo sát trước (đếm chính xác, không đụng khối `scope=`)

| Chỉ số | Trước |
|---|---:|
| Tệp theo dõi Git toàn repo | **10.373** |
| `.log` dưới `v2/agent/JOURNAL/` | **7.768** (theo dõi **100 %**; 0 tệp ignored) |
| Khối `scope=`/`scope-red=` trong 43 tài liệu đích | **159** (`scope` **145** + `red` **14**) |
| Tên `scope` phân biệt | **124** |
| Tệp nguồn phân biệt của các khối | **124** = **119** `.log` + 5 tệp khác (`kt76-criteria.txt`, `kt76-l1-timing.txt`, 2 `summary.txt`, `verify-script-regressions.txt`) |

Kế hoạch ghi "165 khối `scope=`": **số thực đo tại cây này là 159 khối** (124 tên phân biệt).
Đã **giữ 100 %** các nguồn: sau khi xoá, **0** tên `scope` không phân giải được và **119/119** tệp
`.log` nguồn vẫn tồn tại.

### 3.2 Tập giữ lại (222 `.log`) — không chỉ `scope=`

| Nhóm | Số | Lý do |
|---|---:|---|
| Nguồn `scope=`/`scope-red=` | 119 | `verify-numbers.py` đọc nội dung để đối chiếu số trong tài liệu |
| Đích liên kết markdown của **tài liệu đang dùng** (`v2/agent/reports/*.md`, `v2/docs/**/*.md`) | 99 | không để liên kết chết trong báo cáo hiện hành |
| `positive_log`/`negative_log` của `v2/test/rules/counters.toml` | 2 | luật 3 (`kt78-ktest-matrix.log`, `kt78-guard/selftest-20261117T015708Z.log`) |
| `[[negative_log]]`/tên log của `numbers-policy.toml` (`kt75-qemu-b09/b10`, `kt75-persist-run`, 3 bản `b02.log`) | 6 | luật 1–2 (`scope-red` + ngoại lệ lịch sử) |

(Giao của các nhóm = 0 nên tổng **221**; cộng **1** tệp `v2/agent/JOURNAL/kt142-logs/kt142-quet-sau.log` — là đích liên kết của báo cáo **đã lưu trữ** `v2/agent/archive/reports/PHIEU-TRA-LOI-AGENT0-2026-11-18-C1-C7.md`, phát hiện khi kiểm liên kết sau khi chuyển (mục B) ⇒ tổng **222**.)

### 3.3 Thực thi + bằng chứng

- **Tag `cycle1-raw-logs`** (annotated) → commit **`948fb3724f5310784ea723b40147c65ee14627b9`**,
  tree `be7bce8dcf4a3adcb16101c41376edf2d8ae17a0`, **root commit** (cây riêng, không nối lịch sử
  `main`), chứa **đúng 7.546** tệp raw `.log` bị tách — đã kiểm **bằng nhau từng tên** với danh sách
  xoá (`diff` rỗng), và **0** tệp trong tập giữ/`scope=` nằm trong tag (`comm -12` rỗng).
- **Xoá khỏi cây**: `git rm` **7.546** tệp (chỉ `.log`; kiểm: `git status | grep '^D'` **0** tệp khác
  đuôi `.log`). Tệp theo dõi Git **10.373 → 2.827** (−**72,8 %**, Kế hoạch ước −73,5 %).
- **Khôi phục** (ghi trong tag + README kho lưu trữ):
  `git checkout cycle1-raw-logs -- v2/agent/JOURNAL/` (hoặc từng tệp
  `git checkout cycle1-raw-logs -- <đường-dẫn>`). Đã kiểm `git cat-file -e cycle1-raw-logs:v2/agent/JOURNAL/kt00-d2124-sections.log`.
- **Cổng sau khi xoá** (chạy lại trên cây đã xoá):
  `verify-numbers.py --root .` ⇒ `ok=5 fail=0` (trước: `ok=5 fail=0`; `docs=42`, `legacy=19`,
  `pairs=127`, `counters=2 used=2`, `law8 cases=21` — **không đổi**);
  `check-scope-table.py --root .` ⇒ **PASS** (số câu bỏ qua 1 = mốc 1, không tăng).
- **Rủi ro đã biết (ghi để lead quyết định)**: 751 đích liên kết markdown `.log` trong
  `v2/agent/JOURNAL/**/*.md` (bản ghi lịch sử) và ~1.286 tham chiếu dạng chữ trỏ tới tệp nay nằm
  trong tag — **không** phải liên kết của tài liệu đang dùng (nhóm 99 đã giữ). Đây đúng phạm vi
  Kế hoạch cho phép (chỉ bảo vệ 100 % khối `scope=`/`scope-red=`, `*.md`, `summary.txt`); khôi phục
  bằng một lệnh ở trên. **Không** có tệp `scope=` nào bị mất.

---

## 4. Cổng + commit

| Bước | Lệnh | Kết quả |
|---|---|---|
| Kiểm máy checker | `python3 v2/tools/check-wake-paths.py --selftest` | **461/461 ok** (452 cũ + 9 mới) |
| `v2 check` | `./v2/bin/v2 check` | **PASS** — `gate-steps registered=44 ran=44 skipped=0 failed=0`, `TỔNG check 316.019 ms` (wall, tải 4 agent) |
| `check-rules` | `./v2/bin/v2 check-rules` | **PASS** — `verify-numbers: ok=5 fail=0` |
| Số ↔ `scope=` | `python3 v2/tools/check-scope-table.py --root .` | **PASS** (1 câu bỏ qua = mốc) |
| T1 | `./v2/bin/v2 gate --dev` | xem §4.1 |

### 4.1 T1 (`gate --dev`)

- Lượt 1: **FAIL có lý do hạ tầng** — `measure-lock: ĐANG GIỮ pid=35962 … nhãn="gate --fast (T2)"`
  ⇒ T1 **nhường ngay** theo D2-248 (không chồng lên T2 của lead), `v2 gate --dev: FAIL`.
  Đây **không** phải lỗi mã; chờ T2 xong rồi chạy lại.
- Lượt 2 (sau khi lead nhả khoá `measure-lock` — `TRỐNG`): chạy trên **cây sạch** tại commit của
  task; kết quả ghi ở dòng dưới (cập nhật bằng `--amend` chỉ chạm tệp journal này; cây mã của commit
  **không đổi**, đúng AGENTS §6.3 — sửa tài liệu không ảnh hưởng phép kiểm):
  `KẾT QUẢ T1: <điền>`

---

## 5. Hạn chế / nợ mở (không giấu)

1. **−38,8 s CPU/cổng**, không đạt −60 s: trần lý thuyết của phép gộp là một lượt (~42 s CPU) trên
   interpreter/cây này; xem §1.4. Muốn hơn nữa phải cắt bước **selftest** (47–49 s CPU, đang giữ
   452/452) hoặc cắt chính lượt chung — **ngoài** DoD của task này.
2. Bước 20 gộp 20+22 ⇒ `check` 45 → 44 bước, T2 76 → 75. Đã cập nhật `CHECK_STEPS_EXPECTED=44`,
   ghi chú ở `consts.sh`/`check.sh`/`lane.sh`; **không** bỏ luật nào (§1.3).
3. Tag `cycle1-raw-logs` là **ref cục bộ** (chưa push). Lead cần `git push origin cycle1-raw-logs`
   cùng lúc push `main` nếu muốn lưu trữ lên remote; nếu không, chỉ còn trên máy này.
4. `E_LEX_UNKNOWN` vẫn không có ca âm: nhánh **không thể tới** (`punct` nuốt mọi ký tự), đã ghi lý do
   tại chỗ trong `selftest()`.
