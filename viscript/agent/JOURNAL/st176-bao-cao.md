# ST-176 — Báo cáo nợ #147 “mac chậm hơn ps”

**Ngày**: 2026-11-17 · **Agent 2** dòng ViScript · **HEAD đo**: `aa407ce5` (cả hai nền)
**Nguồn sự thật**: VD-177 §2 (cấp nợ #147), VD-179 §3 (clone sạch), VD-177 §3 (khoá máy dùng chung).
**Không sửa mã, không đổi ngưỡng, không đổi ca, không `#[ignore]`.** Mọi phép đo giữ `~/.vifast-measure.lock`.

---

## 0. Tóm tắt cho Agent 0

| Câu hỏi | Trả lời |
|---|---|
| Soak mac (đã có, ST-176 a) | **1489 s**, 10/10 OK, `lock=yes`, load 1,32→1,81 |
| Soak ps (mới, clone sạch) | **261 s**, 10/10 OK, `lock=yes`, load 1,03→1,23 |
| **Tỉ lệ mac/ps** | **5,70×** (1489 / 261) |
| Ngưỡng đóng nợ (VD-177 §2b) | ≤ 3× ⇒ đóng · **> 3× ⇒ còn nợ** |
| **Kết luận** | **5,70× > 3× ⇒ nợ #147 CHƯA đóng được.** Số mac 1489 s là số chuẩn (đo khi máy rảnh, có khoá) |
| Nguyên nhân (đo được) | **Chi phí tạo tiến trình + syscall trên macOS**, không phải CPU. `vis-dev check` = 190,0 s / 12,3 s = **15,5×**, trong đó `report-check` 22,8× và `rules` 8,3×; phần chạy test chỉ 4,42× |

---

## 1. Soak ps trên clone sạch (VD-179 §3)

### 1.1 Dựng clone

Lệnh đã dùng (qua `bin/vis-remote ps --allow-dirty --log /tmp/… -- bash -lc '…'`):

```sh
git clone --quiet /root/vifast-linux /root/viscript-sach      # clone cục bộ, origin/main = aa407ce5
git -C /root/viscript-sach checkout --quiet aa407ce5          # CO_EXIT=0
git -C /root/viscript-sach status --porcelain | wc -l         # = 0  ⇒ clone SẠCH
git -C /root/viscript-sach rev-parse HEAD:viscript            # 04e5e77b4d8d51645a69fab9bd8528ebdedd6527
```

* `git fetch origin` **không cần**: `origin/main` trong `/root/vifast-linux` đã là `aa407ce5`, nên clone cục bộ có sẵn commit. Không có lỗi mạng.
* **Cây `viscript` khớp mac**: mac `git rev-parse aa407ce5:viscript` = `04e5e77b4d8d51645a69fab9bd8528ebdedd6527` — **trùng**.
* Chép công cụ khoá: `scp viscript/bin/vis-lock root@ps:/root/viscript-sach/viscript/bin/vis-lock` + `chmod +x`.
  `vis-lock` **không có trong commit `aa407ce5`** (là tệp mới của ST-176), nên sau khi chép cây có **1 mục untracked**.
  Đã khai báo cục bộ bằng `.git/info/exclude` (`viscript/bin/vis-lock`, `viscript/agent/JOURNAL/st176-soak-ps.log`)
  ⇒ `git status --porcelain` **trở lại rỗng**. Không sửa tệp tracked nào.
* **Làm nóng build trước khi soak** (để so sánh công bằng — target mac đã ấm từ trước):
  `cd /root/viscript-sach/viscript && cargo test --workspace --no-run` → `BUILD_EXIT=0`, 127 binary, 13,07 s.
  Không làm nóng thì vòng `gate-1` gánh cả biên dịch và tỉ lệ sẽ sai.

### 1.2 Lệnh soak (nguyên văn)

```sh
cd /root/viscript-sach/viscript
bin/vis-lock --log agent/JOURNAL/st176-soak-ps.log -- \
    env VIS_TASK_ID=176 bin/vis-soak --log agent/JOURNAL/st176-soak-ps.log --append
```

`vis-soak` tự `cd "$(dirname "$0")/.."` ⇒ chạy ở gốc `viscript/`, đúng như soak mac. Không phải điều chỉnh gì.

### 1.3 Kết quả

| | mac (`st176-soak-macos.log`) | ps (`st176-soak-ps.log`) |
|---|---|---|
| HEAD | `aa407ce5` | `aa407ce5` |
| Cây nguồn | cây làm việc mac (bẩn là bình thường) | **clone sạch**, `status --porcelain` = 0 dòng |
| `lock=` | **yes** (pid 5907) | **yes** (pid 687011) |
| Load trước | `1.32 1.27 1.42` | `1.03 0.40 0.18` |
| Load sau | `1.81 2.52 2.56` | `1.23 1.00 0.51` |
| **Tổng** | **1489 s** | **261 s** (`vis-lock` ghi `giay=260.8`) |
| Vòng | 10/10 OK | 10/10 OK |
| Mã thoát | 0 | 0 |

### 1.4 Phân rã 10 vòng

| Vòng | mac (s) | ps (s) | Tỉ lệ |
|---|---:|---:|---:|
| gate-1 | 320 | 46 | 6,96× |
| gate-2 | 317 | 39 | 8,13× |
| gate-3 | 316 | 39 | 8,10× |
| diff-1 | 156 | 41 | 3,80× |
| diff-2 | 156 | 41 | 3,80× |
| diff-3 | 157 | 40 | 3,93× |
| host-1 | 8 | 1 | 8,00× |
| host-2 | 8 | 1 | 8,00× |
| cache-1 | 26 | 6 | 4,33× |
| cache-2 | 25 | 7 | 3,57× |
| **TỔNG** | **1489** | **261** | **5,70×** |

Theo nhóm: **gate 953/124 = 7,69×** · **diff 469/122 = 3,84×** · **host 16/2 = 8,00×** · **cache 51/13 = 3,92×**.

---

## 2. Tỉ lệ và kết luận a/b/c

* **Tỉ lệ = 1489 / 261 = 5,70×** (nếu lấy `giay=260.8` của `vis-lock` thì 5,71×).
* **a** (đo lại mac khi load < 2 + giữ khoá): **XONG** — 1489 s, load 1,32→1,81, `lock=yes`.
* **b** (≤ 3× ⇒ đóng #147): **KHÔNG đạt** — 5,70× > 3×.
* **c** (> 3× ⇒ đo từng suite): **XONG** — mục 3 và 4 dưới đây.

> **Kết luận: nợ #147 vẫn mở.** Con số 1489 s của mac là **số chuẩn** (máy rảnh, có khoá), thay cho 1696 s của ST-167 (đo khi mac bận, là cận trên). Nhưng khoảng cách 5,70× là **thật**, không phải do tải máy.

---

## 3. Đo từng suite (124 test binary)

### 3.1 Cách đo

```sh
cargo test --workspace --no-run --message-format=json   # lọc reason=compiler-artifact + executable
```

* **Chỉ lấy binary trong `target/debug/deps/`** (124 cái). Danh sách thô có 127 mục vì `cargo test` cũng build
  **binary thường** (`target/debug/vis-lex-fuzz`, `vis-parse-fuzz`, `vis`, `fmt`); chạy thẳng chúng là chạy
  **fuzzer vô hạn** — lần đo đầu bị treo ở `vis-lex-fuzz` (đã dừng, lọc lại). Đây là bẫy của cách lấy danh sách,
  không phải lỗi mã.
* Mỗi binary chạy **một lần**, `--test-threads` mặc định, `cwd` = thư mục package (như `cargo test`), `stdin=/dev/null`.
* mac: dưới `bin/vis-lock` (giữ khoá suốt lượt đo). ps: chạy tuần tự trên clone sạch, cũng dưới `vis-lock`.
* **Không retry lấy xanh**; chạy lần 2 chỉ để loại **hiệu ứng lần đầu** (xem 3.2).

### 3.2 Hiệu ứng “lần đầu” trên mac (phải loại bỏ)

| | mac lần 1 | mac lần 2 (ấm) | ps lần 1 | ps lần 2 |
|---|---:|---:|---:|---:|
| Tổng 124 binary | 131,559 s | **111,222 s** | 23,746 s | **25,144 s** |

Chênh 20,3 s ở mac tập trung ở vài suite **có tạo tiến trình** (`prelude` 8,21→0,19 s · `khe_ong` 5,54→0,12 s ·
`where_tinh` 3,15→0,08 s · `trait_cli` 3,21→0,35 s). Đây là **macOS kiểm/ánh xạ binary vừa build ở lần chạy đầu**,
không phải tốc độ CPU. **Số dùng để so sánh là lần 2 (ấm)**: mac **111,222 s** vs ps **25,144 s** = **4,42×**.

### 3.3 Tổng theo crate

| Crate | Số binary | mac (s) | ps (s) | Tỉ lệ |
|---|---:|---:|---:|---:|
| vis-cli | 77 | 102,053 | 20,385 | **5,01×** |
| vis-vm | 5 | 6,224 | 4,461 | 1,40× |
| vis-types | 12 | 0,815 | 0,054 | 15,09× |
| vis-bytecode | 7 | 0,800 | 0,143 | 5,59× |
| vis-syntax | 14 | 0,721 | 0,046 | 15,67× |
| vis-fmt | 3 | 0,331 | 0,035 | 9,46× |
| vis-interp | 4 | 0,178 | 0,013 | 13,69× |
| vis-diag | 2 | 0,100 | 0,007 | 14,29× |
| **Tổng** | **124** | **111,222** | **25,144** | **4,42×** |

`vis-cli` chiếm **102,1 s / 111,2 s = 92 %** thời gian mac — đúng crate có nhiều ca **gọi tiến trình con**.

### 3.4 MƯỜI suite chênh lớn nhất — theo **hiệu tuyệt đối** (cái đáng sửa trước)

| # | Suite | Crate | mac (s) | ps (s) | Tỉ lệ | Chênh (s) |
|---:|---|---|---:|---:|---:|---:|
| 1 | `cache_khong_che` | vis-cli | 27,824 | 4,722 | 5,89× | **+23,102** |
| 2 | `vis` | vis-cli | 12,341 | 2,267 | 5,44× | **+10,074** |
| 3 | `diag_cmp` | vis-cli | 12,242 | 2,219 | 5,52× | **+10,023** |
| 4 | `host_conformance` | vis-cli | 7,745 | 0,565 | 13,71× | **+7,180** |
| 5 | `vis_test` | vis-cli | 6,125 | 1,066 | 5,75× | **+5,059** |
| 6 | `sem` | vis-cli | 5,543 | 1,016 | 5,46× | **+4,527** |
| 7 | `net_server` | vis-cli | 3,232 | 0,365 | 8,85× | **+2,867** |
| 8 | `diff_100k` | vis-vm | 3,324 | 1,719 | 1,93× | **+1,605** |
| 9 | `test_timeout` | vis-cli | 2,316 | 1,168 | 1,98× | **+1,148** |
| 10 | `stdlib` | vis-cli | 1,437 | 0,315 | 4,56× | **+1,122** |

10 suite này = **+66,7 s / +86,1 s tổng chênh = 77 %**. Sửa 3 cái đầu đã bắt được **43 s (50 %)**.

### 3.5 MƯỜI suite chênh lớn nhất — theo **tỉ lệ** (cả hai ≥ 0,05 s)

| # | Suite | Crate | mac (s) | ps (s) | Tỉ lệ |
|---:|---|---|---:|---:|---:|
| 1 | `vm_neg` | vis-vm | 0,221 | 0,001 | **221×** |
| 2 | `vis_interp` | vis-interp | 0,069 | 0,001 | 69× |
| 3 | `vis_types` | vis-types | 0,069 | 0,001 | 69× |
| 4 | `vis-parse-fuzz` | vis-syntax | 0,068 | 0,001 | 68× |
| 5 | `api_dong_bang` | vis-cli | 0,214 | 0,004 | 53,5× |
| 6 | `trait_cli` | vis-cli | 0,352 | 0,008 | 44× |
| 7 | `pkg_lock` | vis-cli | 0,295 | 0,008 | 36,9× |
| 8 | `vis_vm` | vis-vm | 0,064 | 0,002 | 32× |
| 9 | `generic` | vis-types | 0,116 | 0,004 | 29× |
| 10 | `cache_fastpath` | vis-cli | 0,227 | 0,008 | 28,4× |

Các suite này **nhỏ tuyệt đối** (tổng chênh < 1,5 s) nhưng tỉ lệ rất cao: chúng gần như chỉ có
**khởi động tiến trình + đọc tệp**, nên tỉ lệ ở đây là tỉ lệ **chi phí cố định của HĐH**, không phải của mã.

### 3.6 Nửa `check` của gate — chỗ chênh nặng nhất

`vis-dev gate --fast` = `vis-dev check` + `cargo test --workspace`. Đo riêng (cùng khoá, cùng HEAD):

| Pha | mac (s) | ps (s) | Tỉ lệ |
|---|---:|---:|---:|
| **`vis-dev check` (tổng)** | **190,03** | **12,26** | **15,5×** |
| ├ `vis-dev rules` (873 tệp nguồn) | 49,31 | 5,95 | **8,3×** |
| ├ `report-check.py --all` (28 báo cáo) | 111,88 | 4,90 | **22,8×** |
| ├ fixtures luật R1..R4 | 3,07 | 0,74 | 4,1× |
| ├ fixtures máy kiểm báo cáo | 2,56 | 0,25 | 10,2× |
| ├ `doc-examples --gate` (24 khối) | 0,31 | 0,07 | 4,4× |
| ├ `vis explain VS0107` | 0,52 | — | — |
| └ `cargo fmt` + `clippy` + `build` (đã ấm) | ~0,5 | ~0,05 | ~10× |
| *cộng các pha đo được* | *168,2* | *12,0* | *14,0×* |
| `cargo test --workspace` (124 binary) | 111,22 | 25,14 | 4,42× |

(Các pha đo riêng cộng lại **168,2 s** trên tổng `check` **190,0 s**; ~22 s còn lại là `find`/vòng lặp shell
và dao động giữa hai lượt đo — không đủ lớn để đổi kết luận.)

⇒ **`check` mới là nửa đắt của gate trên mac** (190 s / 320 s = **59 %**), không phải phần chạy test.
Vì sao: `run_rules` trong `bin/vis-dev` chạy **một tiến trình `wc`/`grep` cho MỖI tệp** (873 tệp),
và `check_report.py` gọi `git` cho từng báo cáo — toàn là **tạo tiến trình**, đúng chỗ macOS chậm.

---

## 4. Kiểm riêng hai nhóm ca theo yêu cầu

### 4.1 Ca nhiều vòng `#146` — `quyen::tests::dua_doi_symlink_lap_co_gioi_han` (50 000 vòng)

Binary: `target/debug/deps/vis-<hash>` (unittest của `vis-cli`), chạy `--exact`.

| Nền | Lần | Kết quả | Thời gian |
|---|---|---:|---:|
| mac | 1 | 1 passed, 0 failed (67 filtered) | **8,51 s** |
| mac | 2 | 1 passed, 0 failed | 8,51 s |
| ps | 1 | 1 passed, 0 failed (71 filtered) | **0,41 s** |
| ps | 2 | 1 passed, 0 failed | 0,41 s |

**Tỉ lệ 20,8×** — cao hơn hẳn tỉ lệ chung 5,70×. Ca này là **vòng thuần syscall** (50 000 lần `read` qua
`FsGuard` + một luồng phụ đổi symlink liên tục), nên đây là **chi phí syscall/đường dẫn tệp của macOS**,
không phải CPU. Ca **vẫn xanh cả hai nền**, không đổi ngưỡng, không đổi số vòng.

*Ghi chú*: mac có **68** test trong binary này, ps có **72** — 4 ca chênh do `cfg` nền tảng (có sẵn, không do ST-176).

### 4.2 Ca tạo tiến trình

Đo trực tiếp chi phí tạo tiến trình (20 lần, cùng một script trivial `let x = 1`):

| Nền | 20 lần `vis run <tệp trivial>` | Mỗi lần |
|---|---:|---:|
| mac | 0,307 s | **15,4 ms** |
| ps | 0,032 s | **1,6 ms** |

**Tỉ lệ 9,6× mỗi lần tạo tiến trình.** Đây là nguyên nhân gốc của:
`prelude`/`khe_ong`/`trait_cli`/`where_tinh` (mỗi ca gọi `Command::new(env!("CARGO_BIN_EXE_vis"))`),
`host_conformance` (13,7×), `net_server` (8,85×), và cả `report-check` (22,8×) lẫn `rules` (8,3×) ở mục 3.6.

---

## 5. Kết luận

1. **Nợ #147 CHƯA đóng**: tỉ lệ **5,70×** > ngưỡng 3× của VD-177 §2b. Số chuẩn mới: **mac 1489 s · ps 261 s**, cùng HEAD `aa407ce5`, cả hai có `lock=yes`, mac load 1,32→1,81 (rảnh), ps load 1,03→1,23.
2. **Nguyên nhân không phải CPU**: phần chạy test chỉ **4,42×**; phần `check` là **15,5×**. Hai chỗ đắt nhất đều là **tạo tiến trình**:
   * `run_rules` gọi `wc`/`grep` **một tiến trình mỗi tệp** (873 tệp) → 49,3 s / 6,0 s = 8,3×;
   * `check_report.py` gọi `git` cho từng báo cáo → 111,9 s / 4,9 s = 22,8×;
   * chi phí tạo tiến trình trần: **15,4 ms (mac) vs 1,6 ms (ps) = 9,6×**.
3. **Ca `#146` 50 000 vòng: 20,8×** — chi phí syscall tệp của macOS; ca vẫn xanh hai nền.
4. **Không sửa mã, không đổi ngưỡng, không đổi ca** trong task này (đúng luật cứng). Hướng sửa (đề xuất, **chưa làm**, cần Agent 0 duyệt): gộp `run_rules` thành **một** lượt `find`+`awk`/python thay vì một tiến trình mỗi tệp; gom lệnh `git` của `check_report.py` thành một lượt. Ước tính theo số đo trên: bỏ được ~150 s trong 190 s của `check` trên mac.

---

## 6. Danh sách tệp (Agent 1 commit — **Agent 2 KHÔNG tự commit**)

**Tệp mới của ST-176 (cần commit):**

| Tệp | Nội dung |
|---|---|
| `viscript/bin/vis-lock` | công cụ giữ khoá `~/.vifast-measure.lock` (đã tự kiểm PASS ở `st176-kiem-lock.log`) |
| `viscript/agent/JOURNAL/st176-kiem-lock.log` | log tự kiểm `vis-lock` (12 ca, có ca âm) |
| `viscript/agent/JOURNAL/st176-soak-macos.log` | soak mac 10 vòng, **1489 s**, `lock=yes` |
| `viscript/agent/JOURNAL/st176-soak-ps.log` | soak ps 10 vòng trên **clone sạch**, **261 s**, `lock=yes` |
| `viscript/agent/JOURNAL/st176-suites-mac.tsv` | thời gian 124 test binary trên mac (lần 2, ấm) |
| `viscript/agent/JOURNAL/st176-suites-ps.tsv` | thời gian 124 test binary trên ps (lần 2) |
| `viscript/agent/JOURNAL/st176-suites-mac.lock.log` | bằng chứng `vis-lock` cho các lượt đo trên mac |
| `viscript/agent/JOURNAL/st176-bao-cao.md` | báo cáo này |

**Không đụng tới**: `viscript/agent/QUEUE.md`, `DECISIONS.md`, `STATE.md`, `BLOCKED.md` (Agent 0/Agent 2 theo AGENTS §3 — task này không sửa), `spec/`, mã nguồn, ngưỡng.

**Trên ps (không commit, chỉ là bằng chứng cục bộ)**: `/root/viscript-sach` (clone sạch `aa407ce5`),
`/root/vislogs/st176-*.{tsv,log,json}`. Cây cũ `/root/vifast-linux` **giữ nguyên**, không xoá/đổi tên (việc của ST-170).

---

## 7. Ghi chú kỹ thuật để lần sau đo lại

* **Phải lọc `target/debug/deps/`** khi lấy danh sách test binary: danh sách thô có thêm binary thường
  (`vis-lex-fuzz`, `vis-parse-fuzz`, `vis`, `fmt`) — chạy thẳng là **treo vô hạn**.
* **Phải chạy 2 lượt trên mac** và lấy lượt 2: lượt đầu cộng ~20 s do macOS kiểm binary mới build.
* **Phải làm nóng `cargo test --no-run` trên clone ps** trước khi soak, nếu không `gate-1` gánh cả biên dịch.
* `targets` chạy **riêng** trên mac trả `exit=101` vì `cargo` không có trong `PATH` của shell không đăng nhập
  (ca gọi `cargo check`); thêm `$HOME/.cargo/bin` vào `PATH` thì **PASS 1/1 trong 0,20 s**. Đây là lỗi của
  cách đo, **không** phải ca đỏ — trong `gate` thì `cargo` có trong `PATH` nên soak 10/10 vẫn xanh.
* **HEAD mac nhích trong lúc đo** (`d0b814c1` → `74a6dc37`, hai commit `[KT-00]` của dòng OS). Đã kiểm
  `git diff --stat aa407ce5 <HEAD> -- viscript crates Cargo.toml Cargo.lock` = **rỗng** ⇒ mã ViScript trên mac
  **vẫn đúng `aa407ce5`**, số đo không lệch HEAD.
