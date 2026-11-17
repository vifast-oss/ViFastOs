# R29 — ViScript, vòng 29 (đợt 29, 2026-11-17)

Báo cáo chốt **đợt 29** — phần **sau R28**. Nguồn quyết định: [`DECISIONS.md`](../DECISIONS.md)
**VD-175** (ST-166/landlock/#146) · **VD-176** (vá #146 phương án A + chứng minh tất định) ·
**VD-177** (ST-167, nợ #147, cây ps, khoá máy dùng chung) · **VD-178** (duyệt 6 tài liệu VS3 + `st169` +
`vs-ergo.md`, mở khoá ST-171) · **VD-179** (cây ps, R29 clone sạch, phạm vi ST-171) · **VD-180** (mô hình
gói phi tập trung kiểu Go) · **VD-181** (tên gói `vifast.org/vis/<tên>`). Luật còn hiệu lực: **VD-131**
(5 câu) · **VD-142** (báo cáo đã commit không sửa) · **VD-143** (hàng phụ thuộc) · **VD-145/VD-146**
(macOS / Windows **chỉ đo**) · **VD-147(3)(4)(5)** (sổ nợ duy nhất · hash đo từng nền · không ghi đè log).
Sổ nợ duy nhất: [`SO-NO.md`](../SO-NO.md) §8.

**HEAD đo của R29: `39041b3f1bdda8aa786bf785a1835293de20cf19`** (`39041b3f`) — kiểm lại lúc bắt đầu
(`git rev-parse --short HEAD` = `39041b3f`, **không nhích** trong suốt đợt). Cây `viscript` của HEAD:
`a13c37f220eec09aff822f5b0442cb2fab61e033` — **trùng** giữa mac và clone sạch trên `ps`.
`git diff 39041b3f..HEAD -- viscript crates Cargo.toml Cargo.lock` = **rỗng** (HEAD không đổi trong đợt).

Báo cáo **không** sửa R1..R28; mọi điểm cần nói lại về R28 ghi ở **§7**.

## 0. Ba hàng phụ thuộc + chính báo cáo — trạng thái tại thời điểm chốt (VD-143)

| Hàng | Nội dung | Trạng thái | Commit | Bằng chứng chính |
|---|---|---|---|---|
| **ST-174** | Vá nợ **#146** (phương án A) + chứng minh **tất định** | **DONE** | `7adcbf6e` | đỏ-trước **100/100** trên mã cũ; sau vá **100/100 xanh** trên **mac và ps**; ca B đối chứng 100/100; ca âm báo “không chạy được điểm chèn”; **6 000 000 vòng 0 đỏ**; gate mac **3 lần PASS** L0 **1184/0** — log `st174-*` |
| **ST-175** | Điều tra cây `ps` bẩn + `vis-remote` từ chối cây bẩn | **DONE** | `c4c07808` | `viscript/` **0 tệp bị xoá**; 93 xoá ở `crates/` là **dòng OS**; 3 tệp sửa có mốc **sau** gate ⇒ ViScript **sạch theo kiểm hồi tố**; `vis-remote` exit **6** khi cây bẩn — log `st175-*` |
| **ST-176** | Nợ **#147** — mac chậm hơn ps | **DONE** | `39041b3f` | soak có khoá: mac **1489 s** vs ps **261 s** = **5,70×**; `vis-lock` (macOS không có `flock`); đo từng suite 124 binary — log `st176-*` |
| **ST-170** | **R29** (chốt đợt 29) | **READY tại `head`** | — (commit báo cáo) | chính báo cáo này; `head` = `39041b3f1bdda8aa786bf785a1835293de20cf19` |

**ST-170 (chính báo cáo này): `READY` tại `head` = `39041b3f`** — QUEUE tại `head` ghi `READY`; hàng lật
`DONE` ở **chính commit chứa báo cáo**, sau khi `check_report --all` PASS (**29** báo cáo, **197** hàng) và
gate `--fast` PASS trên **cả ba nền** (đúng **VD-143**: 3/3 hàng phụ thuộc **ST-174/ST-175/ST-176** đã
`DONE` trước khi lật). QUEUE còn **1** hàng `BLOCKED` (**ST-178**, chờ ST-171) ⇒ `blocked` trong JSON có
**1** mục; **ST-171** `READY` (đã mở khoá theo VD-178) nhưng chưa chạy.

## 1. Tóm tắt

1. **Ba nền, cùng một HEAD `39041b3f`**: **mac** gate `--fast` **PASS 10/10**, L0 **1184/0** (**131** suite),
   **326,2 s**, `lock=yes`; **ps** gate `--fast` **PASS 10/10**, L0 **1188/0** (**131** suite), **39,2 s**,
   `lock=yes`, chạy trên **clone sạch** `/root/viscript-sach`; **psvn** gate `--fast` **9/10 bước**, dừng ở
   **L0** với **152/4** trên 16 suite (suite `cap_proc` 2/4: `/bin/echo` + `/bin/sh` — **VS3 mục 5 nhóm (a)**, không phải hồi quy).
2. **`#146` ĐÓNG ở ST-174.** Trước khi vá có **đúng 2 lần gate mac đỏ do #146** — `st161-gate-macos-lan2-t146.log`
   (`ro_ri=1/50 000`) và `st166-gate-macos.log` (`ro_ri=1/50 000`); mỗi lần chạy lại **một lần** để phân loại
   theo **D2-07** ⇒ PASS 1182/0. Sau vá: **100/100 tất định** trên mac **và** ps, **6 000 000 vòng 0 đỏ**,
   gate mac **3 lần liên tiếp PASS**.
3. **L0 `1184/0` = `1182`** (số của ST-173) **+ `2` ca mới của ST-174** trong
   `crates/vis-cli/src/quyen_st174.rs`: **ca A** (điểm chèn đổi `root/tep` thành symlink ra ngoài ⇒ bị chặn
   tất định) và **ca B** (điểm chèn đổi symlink về tệp thật ⇒ đọc đúng trong root). Trên `ps` con số là
   **1188/0** = **1186** (ST-173) **+ 2** — chênh **+4** so với mac là **4 ca `cfg` nền tảng có sẵn**, không
   phải ca mới.
4. **`#147` CHƯA ĐÓNG**: mac **1489 s** (số chuẩn, `lock=yes`, load 1,32→1,81) vs ps **261 s** (clone sạch,
   `lock=yes`) = **5,70×** > ngưỡng **3×** của VD-177 §2b. Nguyên nhân **không phải CPU** mà là **tạo tiến
   trình + syscall của macOS**: `check` **15,5×**, `report-check` **22,8×**, `rules` **8,3×**, spawn trần
   **9,6×**, ca `#146` 50 000 vòng **20,8×**; riêng `cargo test` chỉ **4,42×**.
5. **Cây `ps` cũ đã đổi tên** `/root/vifast-linux` → `/root/vifast-linux-BAN-CU-KHONG-DUNG` (chỉ đổi tên,
   **không** xoá/khôi phục): HEAD **trước = sau = `898c6de8`**, 4575 tệp tracked + 2338 mục bẩn giữ nguyên.
6. **Khe hở bằng chứng cũ đã đóng bằng lời**: 3 gate `ps` của ST-161/166/167 chạy trên **cây bẩn** của dòng
   OS; phần ViScript **sạch theo kiểm hồi tố** ⇒ giữ làm **tham chiếu**, **không** dùng làm số chuẩn. Số `ps`
   chuẩn của R29 là gate **một lần trên clone sạch** (mục 1).
7. **Quyết định mới**: **VD-178** (duyệt **có sửa** 6 tài liệu VS3 + `st169` + `vs-ergo.md`; **mở khoá
   ST-171**), **VD-179** (cây ps · R29 clone sạch · phạm vi ST-171), **VD-180** (mô hình gói **phi tập trung
   kiểu Go**, MVS, `sum.vifast.org`), **VD-181** (tên gói `vifast.org/vis/<tên>`).
8. **Không có lần gate đỏ nào do mã mới trong đợt 29**: 2 lần đỏ duy nhất là **#146** (đã phân loại (b) và
   đóng ở ST-174). Ba lần gate của R29 (mac/ps/psvn) **không retry** (D2-07).

## 2. Số đo ba nền cùng HEAD `39041b3f`

### 2.1 Bảng tổng

| Nền | Cây đo | Gate `--fast` | L0 passed/failed | Suite | Thời gian | `lock=` | Load trước → sau | Log |
|---|---|---|---:|---:|---:|---|---|---|
| **mac** (mps, macOS 27.0.1 arm64, 12 core) | cây làm việc mac (chỉ log `st170-*` chưa track) | **PASS 10/10** `[exit=0]` | **1184 / 0** | **131** | **326,2 s** | **yes** | `1.79 3.31 3.82` → `4.83 4.12 3.99` | `st170-gate-macos.log` |
| **ps** (Ubuntu 26.04.1, Linux 7.0.0-43) | **clone sạch** `/root/viscript-sach` (`git status --porcelain` **rỗng**) | **PASS 10/10** `[exit=0]` | **1188 / 0** | **131** | **39,2 s** | **yes** | `0.08 0.19 0.40` → `0.12 0.14 0.25` | `st170-gate-ps.log` |
| **psvn** (Windows, MINGW64) | repo `/c/Users/ps/vifast-windows/repo` sau `git pull --ff-only` | **9/10 bước**, dừng ở **L0** `[exit=1]` | **152 / 4** (fail-fast, 16 suite) | 16 | — | — | — | `st170-gate-psvn.log` |

* **Cùng HEAD**: `git rev-parse HEAD` = `39041b3f1bdda8aa786bf785a1835293de20cf19` trên **cả ba** máy;
  cây `viscript` = `a13c37f220eec09aff822f5b0442cb2fab61e033` (mac và ps **trùng**).
* **mac** đo trên cây làm việc: `git status --porcelain -- viscript crates Cargo.toml Cargo.lock` có **3**
  mục, **toàn bộ** là log `st170-*` do chính task này tạo (chưa track) — **không** tệp mã nào bị sửa.
* **ps** đo trên **clone sạch**: `git status --porcelain` = **0 dòng** *trước khi* chạy gate (điều kiện
  VD-179 §3); `vis-remote` in `# ST-175: cay SACH (git status --porcelain rong) — duoc phep chay/do`.
* **Thời gian** đo dưới `viscript/bin/vis-lock` (VD-177 §3) trên **cả hai** máy ⇒ `lock=yes` (mac pid 3002,
  ps pid 834320). Target đã **làm nóng** trên cả hai (mac ấm từ các đợt trước; ps ấm từ soak ST-176) nên
  phép so là **cùng điều kiện biên dịch**.

### 2.2 mac — `st170-gate-macos.log`

```
# HEAD=39041b3f1bdda8aa786bf785a1835293de20cf19
# tree viscript=a13c37f220eec09aff822f5b0442cb2fab61e033
# load truoc: { 1.79 3.31 3.82 }
vis-lock: lock=yes pid=3002 tep=/Users/mps/.vifast-measure.lock cho=0.0s
--- fmt: OK / --- clippy: OK / --- luật R1..R4: OK / --- self-test case âm: OK
--- vis explain: OK / report-check: PASS (28 báo cáo, 197 hàng QUEUE phủ đủ)
--- báo cáo fixtures: OK / --- build: OK
--- doc-examples: OK (24 khối tài liệu: 18 xanh + 6 ngoại lệ đỏ đúng; 12 fixture QD §7.1..7.3)
--- L0 test: OK
vis-dev gate --fast: PASS (L1 chưa có tới khi có sim ViScript)
load sau: { 4.83 4.12 3.99 }
[exit=0]
vis-lock: lock=no pid=3002 ma_ra=0 giay=326.2s
```

**10/10 bước `--- …: OK`**, `[exit=0]`, **không retry**. L0 = **1184 passed / 0 failed** trên **131** dòng
`test result`.

### 2.3 ps — `st170-gate-ps.log` (**số ps CHUẨN của R29**, VD-179 §3)

Dựng clone (đã có từ ST-176b, cập nhật lên HEAD đo):

```sh
git -C /root/viscript-sach fetch https://github.com/vifast-oss/ViFastOs.git main   # fetch_exit=0
git -C /root/viscript-sach checkout 39041b3f1bdda8aa786bf785a1835293de20cf19       # co_exit=0
git -C /root/viscript-sach status --porcelain | wc -l                              # = 0  ⇒ SẠCH
git -C /root/viscript-sach rev-parse HEAD:viscript                                 # a13c37f2…  (trùng mac)
```

Chạy (qua `bin/vis-remote ps`, `VIS_REMOTE_PS_REPO=/root/viscript-sach`, dưới `viscript/bin/vis-lock`):

```
# ST-175: cay SACH (git status --porcelain rong) — duoc phep chay/do
vis-lock: lock=yes pid=834320 tep=/root/.vifast-measure.lock cho=0.0s
--- fmt: OK / --- clippy: OK / --- luật R1..R4: OK / --- self-test case âm: OK
--- vis explain: OK / report-check: PASS (28 báo cáo, 197 hàng QUEUE phủ đủ)
--- báo cáo fixtures: OK / --- build: OK / --- doc-examples: OK (24 khối …)
--- L0 test: OK
vis-dev gate --fast: PASS (L1 chưa có tới khi có sim ViScript)
vis-lock: lock=no pid=834320 ma_ra=0 giay=39.2s
[exit=0]
```

**10/10 bước**, `[exit=0]`, **không retry**. L0 = **1188 passed / 0 failed** trên **131** dòng `test result`.
`sha256` của log trên máy đích: `2fdf5970d396c20a8b173425ed05a682dfbf04bce36ba4331b35dc2c2f9b5a18`.

**Load** (header VD-136 của `vis-remote` **không** in load, nên lấy bằng `uptime` trên `ps`):
**trước** `0.08 0.19 0.40` (20:16:58 JST, ~90 s trước khi gate bắt đầu) → **sau** `0.12 0.14 0.25`
(20:30:39 JST). Gate chạy **20:18:29 → 20:19:09 JST** (mtime log trên máy đích
`2026-11-17 20:19:09.650`). Máy **rảnh** trong suốt phép đo.

### 2.4 psvn (Windows) — **chỉ đo** (VD-146)

`psvn` **truy cập được**; repo đã `git pull --ff-only` từ `0df923c2` lên **`39041b3f`** (log
`st170-psvn-pull.log`, `[exit=0]`), `git status` **rỗng** trước khi chạy. Cổng `--fast` trên `39041b3f`:

| Bước | Kết quả |
|---|---|
| 1–9 (`fmt` · `clippy` · `luật R1..R4` · `self-test case âm` · `vis explain` · `báo cáo` · `báo cáo fixtures` · `build` · `doc-examples`) | **OK** (self-test **12/12**; doc-examples 24 khối) |
| **10 — L0 test** | **FAIL** `[exit=1]` — fail-fast tại suite **`cap_proc`**: 15 suite OK = **150 passed**, suite `cap_proc` = **2 passed / 4 failed** ⇒ **152 passed / 4 failed** trên **16** suite đã chạy |

4 ca đỏ: `khong_strict_khong_cap_van_chay` · `strict_co_cap_proc_thi_chay_hai_duong` ·
`strict_cap_proc_sh_exit3` · `strict_cap_proc_pipeline_hai_stage` — **cùng một lý do**:
`VS0501: không chạy được chương trình ngoài /bin/echo` (và `/bin/sh`): *The system cannot find the path
specified. (os error 3)*. Đây là **nhóm (a)** của VS3 mục 5 (48 ca phụ thuộc `/bin/*`, bảng
`st169-bang-phan-loai-windows.md`) — **KHÔNG** phải hồi quy của đợt 29, và **không** sửa ca (VD-146/VD-162).

**Không** dùng làm số chuẩn (VD-146: Windows chỉ đo, không tuyên bố hỗ trợ). Số ca `cargo test` trên Windows
**không** so trực tiếp với mac/ps vì ~100 ca đỏ thuộc **VS3 mục 5** (bảng phân loại `st169`: a=48 / b=15 /
c=42 trên 105 ca). **Load**: Windows **không có** load average ⇒ **không ghi** (không bịa số); `nproc` = 6.

### 2.5 Vì sao L0 = `1184/0` (và `1188/0` trên ps)

| Bước | mac | ps | Nguồn |
|---|---:|---:|---|
| ST-173 (landlock chỉ Linux) | **1182** | **1186** | `st173-gate-macos.log`, `st173-gate-ps.log` |
| **+ 2 ca ST-174** (`quyen_st174.rs`) | **+2** | **+2** | `st174-gate-macos-{1,2,3}.log` |
| **R29 (`39041b3f`)** | **1184** | **1188** | `st170-gate-macos.log`, `st170-gate-ps.log` |

Hai ca mới của ST-174 (`crates/vis-cli/src/quyen_st174.rs`, 324 dòng, `#![cfg(unix)]`, chỉ test):

* **ca A** — `ca_a_diem_chen_doi_sang_symlink_ra_ngoai_bi_chan_tat_dinh`: điểm chèn đổi `root/tep` thành
  symlink ra ngoài ⇒ **bị chặn tất định** (`ro_ri=0 chan=100 chan_thay_doi=100`).
* **ca B** — `ca_b_diem_chen_doi_symlink_ve_tep_that_doc_dung_trong_root`: điểm chèn đổi symlink (trong root)
  về tệp thật ⇒ **đọc đúng `TRONG`** (`ok_trong=100`) — **đối chứng** rằng bản vá **không** “chặn tất”.

Chênh **+4** giữa ps (1188) và mac (1184) là **4 ca `cfg` nền tảng có sẵn** (ST-176 §4.1 đã ghi: binary
unittest `vis-cli` có 68 test trên mac, 72 trên ps) — **không** phải ca mới của đợt 29.

### 2.6 Tỉ lệ cổng mac/ps (dữ kiện mới cho #147)

| Phép đo | mac | ps | Tỉ lệ |
|---|---:|---:|---:|
| **Gate `--fast` (R29)** | **326,2 s** | **39,2 s** | **8,32×** |
| Soak 10 vòng (ST-176) | 1489 s | 261 s | 5,70× |
| `vis-dev check` (ST-176) | 190,0 s | 12,3 s | 15,5× |
| `cargo test --workspace` (ST-176) | 111,2 s | 25,1 s | 4,42× |

Tỉ lệ **cổng** 8,32× nằm **giữa** `check` (15,5×) và `cargo test` (4,42×) — đúng như phân rã của ST-176
(`check` chiếm ~59 % thời gian cổng mac). Đây là **dữ kiện**, không phải ngưỡng mới.

## 3. Tình trạng task tại `QUEUE.md` @ `head` `39041b3f`

`QUEUE.md` tại HEAD có **197 hàng** = **194 `DONE`** + **2 `READY`** (**ST-170**, **ST-171**) +
**1 `BLOCKED`** (**ST-178**). **0** hàng `REVIEW`. JSON `tasks` phủ **đủ 197** hàng, `status` **khớp**
từng hàng (máy kiểm `check_report.py` đối chiếu `git show <head>:viscript/agent/QUEUE.md`).

**20 hàng mới so với R28** (R28 có 177 hàng):

| Hàng | Trạng thái | Commit | Nội dung (rút gọn) |
|---|---|---|---|
| ST-160 | DONE | `870746f9` | dán quyết định đợt 29 (13 APPROVED + VD-157..164 + mở #146); sổ 147 mục, 0 trùng |
| ST-161 | DONE | `cf78e887`, `66ef02c3` | đo lại cùng HEAD `0df923c2` ba nền; gate ps **đỏ #146 lần 1** → chạy lại 1 lần PASS 1182/0 |
| ST-162 | DONE | `0df923c2` | `bin/vis-remote` (397 dòng): đường tuyệt đối, `.exit` đọc lại từ tệp, log ngoài repo |
| ST-163 | DONE | `edf382ec` | điều tra #146: mac **4 đỏ / 8,1 triệu vòng**, ps **0 / 4,1 triệu** ⇒ phân loại **(b)** |
| ST-164 | DONE | `36320070`, `b057ac82` | `hieu_nang_nap_module_5_tep` đo **min của 5 lần** (8 ms), ngưỡng 100 ms giữ nguyên |
| ST-165 | DONE | `4cea479c`, `7a718647` | nợ #138: `/proc` đọc được trên Linux; 4 ca mới; gate mac 1182/0, ps 1186/0 |
| ST-166 | DONE | `717dc27a`, `32525831` | nợ #132: helper theo RUSTC/PATH tuyệt đối; gate mac **đỏ #146 lần 2** → chạy lại PASS 1182/0 |
| ST-167 | DONE | `b7005807` | soak 10 vòng cùng HEAD `795df046`: mac 1696 s (cận trên) vs ps 254 s = 6,68× ⇒ mở #147 |
| ST-173 | DONE | `da3ecfb3` | landlock chỉ Linux (`cfg(target_os)`); mac hết lỗi biên dịch; ps 15/15 không đổi |
| ST-174 | DONE | `7adcbf6e` | **vá #146 (A)** + chứng minh tất định; gate mac **3 lần PASS** L0 **1184/0** |
| ST-175 | DONE | `c4c07808` | `vis-remote` từ chối cây bẩn (exit 6); điều tra cây ps ⇒ ViScript **sạch theo kiểm hồi tố** |
| ST-176 | DONE | `39041b3f` | nợ #147: `vis-lock` + đo lại có khoá: mac 1489 s vs ps 261 s = 5,70×; **#147 chưa đóng** |
| ST-168 | DONE | `d2dbb0cc` | 6 tài liệu quyết định VS3 `vs3-qd-1..6.md` |
| ST-169 | DONE | `33d8b52f` | bảng phân loại **105** ca Windows (a=48 / b=15 / c=42) |
| ST-172 | DONE | `88c3c4cd` | vs-ergo: 7 điểm công thái học + VS0412 dùng 4 nhóm lỗi |
| ST-172a | DONE | `f7b17e51` | vs-ergo-facts: dữ kiện 8 nhóm — **chỉ dữ kiện** |
| ST-172b | DONE | `000c97c8`, `b5115a74` | vs-ergo 13 mục P0–P3 theo VD-170/171/172 |
| **ST-170** | **READY** | — | **R29** (báo cáo này) — lật `DONE` ở chính commit chứa báo cáo |
| **ST-171** | **READY** | — | tách ViScript thành repo riêng — **đã mở khoá** (VD-178), chạy **sau R29** |
| **ST-178** | **BLOCKED** | — | sửa tài liệu theo VD-178 — chờ repo ViScript mới (ST-171) |

## 4. Nợ — trích theo số trong `SO-NO.md` (VD-147(3))

Sổ: **147 mục** = **89 mở** / **35 đóng** / **21 VS3** / 1 không làm / 1 điều hướng (dải 1..147, trùng số
**0**). Đóng trong đợt 29: **#146** (ST-174). Mở mới trong đợt 29: **#147** (VD-177 §2).

### 4.1 `#146` — ca đua symlink: **ĐÓNG ở ST-174**

**(a) Trước khi vá — đếm được ĐÚNG 2 lần gate mac đỏ do #146** (theo `st170-dem-gate-do-t146.md`, VD-175 §3):

| Lần | HEAD | Log | Kết quả lần 1 | Chạy lại 1 lần (D2-07) |
|---|---|---|---|---|
| 1 | `0df923c2` | `st161-gate-macos-lan2-t146.log` | **đỏ** `ro_ri=1` (`lap=50000 ok_trong=11878 ro_ri=1 chan=38121 vong_doi=16383`), `test result: FAILED. 62 passed; 1 failed`, `--- L0 test: FAIL` | PASS 1182/0 |
| 2 | `f2d2e72d` | `st166-gate-macos.log` | **đỏ** `ro_ri=1/50 000` (`lap=50000 ok_trong=12038 ro_ri=1 chan=37961 vong_doi=16994`), `test result: FAILED. 65 passed; 1 failed`, `--- L0 test: FAIL` | PASS 1182/0 (`st166-gate-macos-lan2.log`) |

Cả hai lần đều **phân loại (b) — TOCTOU thật, chỉ macOS** (ST-163: mac **4 đỏ / 8,1 triệu vòng**, ps
**0 / 4,1 triệu**), **không** phải lỗi mã mới và **không** phải `T-INFRA`.

**(b) Sau khi vá — trạng thái đóng** (ST-174, VD-176 phương án A):

| Bằng chứng | Kết quả | Log |
|---|---|---|
| Ca **A** đỏ-trước (mã cũ) | **100/100 đỏ** — `ro_ri=100`, đọc được `NGOAI` ngoài root | `st174-do-truoc-100.log` (`[exit=101]`) |
| Ca **A** sau vá | **100/100 bị chặn** trên **mac và ps** (`ro_ri=0 chan=100 chan_thay_doi=100`) | `st174-sau-mac-ps.log` |
| Ca **B** đối chứng | **100/100 đọc đúng `TRONG`** (không “chặn tất”) | `st174-sau-mac-ps.log` |
| Ca **âm** cho điểm chèn | bỏ điểm chèn ⇒ cả 2 ca **FAILED** với “không chạy được điểm chèn” | `st174-ca-am-diem-chen.log` |
| **6 000 000 vòng** ngẫu nhiên (mac) | **0 đỏ** (`ok_trong=1581842 chan=4418158`, 1006,3 s) | `st174-6m-mac.log` |
| Gate mac **3 lần liên tiếp** | **PASS 10/10**, L0 **1184/0** (131 suite) | `st174-gate-macos-{1,2,3}.log` |

Bản vá: **1 dòng logic** `quyen.rs:513` `std::fs::metadata` → **`std::fs::symlink_metadata`** + **điểm chèn
`#[cfg(all(test, unix))]`** tại `quyen.rs:505` (không có trong build thường — `strings`/`nm` trên binary
`vis` đều **0**). Giới hạn còn lại (**hardlink** tệp ngoài vào root được coi là trong root) là **giới hạn
chính sách**, đã ghi `HOST-BAO-DAM.md`, thuộc VS3 (`#140/#109b`).

### 4.2 `#147` — mac chậm hơn ps: **CHƯA ĐÓNG**

| | mac | ps |
|---|---:|---:|
| Soak 10 vòng | **1489 s** | **261 s** |
| `lock=` | **yes** (pid 5907) | **yes** (pid 687011) |
| Load trước → sau | `1.32 1.27 1.42` → `1.81 2.52 2.56` | `1.03 0.40 0.18` → `1.23 1.00 0.51` |
| HEAD | `aa407ce5` | `aa407ce5` |
| **Tỉ lệ** | **5,70×** | |

**5,70× > 3×** (ngưỡng VD-177 §2b) ⇒ **nợ vẫn mở**. Số mac **1489 s** là **số chuẩn** (máy rảnh, có khoá),
thay cho **1696 s** của ST-167 (đo khi mac bận — **cận trên**).

**Nguyên nhân đo được — tạo tiến trình + syscall của macOS, KHÔNG phải CPU:**

| Pha | mac | ps | Tỉ lệ |
|---|---:|---:|---:|
| `vis-dev check` (tổng) | 190,0 s | 12,3 s | **15,5×** |
| ├ `report-check.py --all` (gọi `git` từng báo cáo) | 111,9 s | 4,9 s | **22,8×** |
| ├ `vis-dev rules` (một `wc`/`grep` mỗi tệp, 873 tệp) | 49,3 s | 6,0 s | **8,3×** |
| `cargo test --workspace` (124 binary) | 111,2 s | 25,1 s | **4,42×** |
| Chi phí **tạo tiến trình trần** (20 lần) | 15,4 ms/lần | 1,6 ms/lần | **9,6×** |
| Ca `#146` **50 000 vòng** | 8,51 s | 0,41 s | **20,8×** |

10 suite chênh lớn nhất theo **hiệu tuyệt đối** = **+66,7 s / +86,1 s = 77 %** tổng chênh
(`cache_khong_che` +23,1 s · `vis` +10,1 · `diag_cmp` +10,0 · `host_conformance` +7,2 · `vis_test` +5,1 ·
`sem` +4,5 · `net_server` +2,9 · `diff_100k` +1,6 · `test_timeout` +1,1 · `stdlib` +1,1).

**Đề xuất (chưa làm, cần A0 duyệt)**: gộp `run_rules` thành **một** lượt `find`+`awk`/python thay vì một
tiến trình mỗi tệp; gom lệnh `git` của `check_report.py` thành một lượt. Ước tính theo số đo trên: bỏ được
**~150 s trong 190 s** của `check` trên mac.

### 4.3 Khe hở bằng chứng cũ — 3 gate `ps` của ST-161/166/167

Ba gate `ps` cũ chạy trên **cây bẩn** `/root/vifast-linux` (2338 mục bẩn, **2297 tệp bị xoá** — toàn bộ là
**dòng OS**). Kết luận **theo bằng chứng** (ST-175, VD-179 §2):

* **`viscript/` = 0 tệp bị xoá**; 93 tệp xoá ở `crates/` (gốc) là **dòng OS**, **không** phải 8 crate
  ViScript (lấy bằng `cargo metadata`, tất cả dưới `viscript/crates/`).
* **HEAD khớp reflog**: `0df923c2` (ST-161), `6f8f63a5` (ST-166), `795df046` (ST-167) đều có trong reflog
  `ps` đúng mốc thời gian.
* **3 tệp sửa** trong `viscript/` (`HOST-BAO-DAM.md`, `main.rs`, `quyen.rs`) có mốc **sau** gate (ST-173/174).

⇒ **Giữ 3 gate làm THAM CHIẾU**, nhãn “**sạch theo kiểm hồi tố**”, **KHÔNG** dùng làm số chuẩn (log chỉ ghi
**commit**, không ghi trạng thái cây tại đúng phút chạy). **Mã ViScript khác nhau giữa ba gate**
(`git rev-parse <HEAD>:viscript/crates` = `c8102181…` → `1a576761…` → `df4da818…`) nên **không** thể coi là
cùng một phép đo. **Số `ps` chuẩn của R29 = gate một lần trên clone sạch** (§2.3).

## 5. Quyết định mới (VD-178 … VD-181) + ST-171

| Quyết định | Nội dung | Ảnh hưởng R29 |
|---|---|---|
| **VD-178** | **Duyệt CÓ SỬA** 6 tài liệu VS3 (`vs3-qd-1..6.md`) + bảng Windows `st169` + `vs-ergo.md`; 9 chỗ [SỬA] ghi trong `JOURNAL/quyet-dinh-a0-2026-11-17-vd178.md` (sha256 `ca7f565e…`). Các [SỬA] làm ở **ST-178** (≤3 h, Agent 1, **0 dòng mã**) **trong repo ViScript mới**. **ST-171 (tách repo) được MỞ KHOÁ sau ST-170.** Thứ tự lớn: **pha 1 công thái học (30 task ≈62 h) TRƯỚC VS3**; xen kẽ được VS3-5a/5d. Mọi đổi kỳ vọng test nêu trong VD này **là A0 duyệt**. | ST-171 `READY`; ST-178 `BLOCKED` |
| **VD-179** | (1) **Đóng băng**: chỉ commit khi dòng OS báo KT-88 xong. (2) “`viscript/` = 0 xoá” **chưa đủ** — phải kiểm cả `crates/`; kết luận theo bằng chứng ⇒ **sạch theo kiểm hồi tố**. (3) **R29 chạy lại MỘT lần gate ps trên clone sạch** — **số này là số ps chuẩn**; cây cũ **giữ nguyên, đổi tên** `-BAN-CU-KHONG-DUNG`; `vis-remote` từ chối cây bẩn (**duyệt**, kèm ca âm). (4) Phạm vi tách repo = `viscript/` + `docs/viscript/` + mọi crate ViScript theo `cargo metadata` + đường dẫn phụ thuộc. (5) **L0 1184/0** (R29 dự kiến 1182): ghi rõ 2 ca thêm thuộc task nào. | §2.3, §2.5, §4.3, §6 |
| **VD-180** | **Mô hình gói phi tập trung kiểu Go, KHÔNG có kho tập trung ở 1.0**: tên gói = **đường dẫn**; phiên bản = **tag git semver**; chọn bằng **MVS** (không bộ giải ràng buộc); `pkg update X` chỉ đổi X + phụ thuộc bắt buộc (giữ `#142`); `vifast.org/x/<tên>` = trang tĩnh meta-tag; tin cậy bằng **`vis.lock` sha256 cây tệp** + **`sum.vifast.org`** (sổ hash chỉ ghi thêm); **VS0707** = hash không khớp; **không** lệnh bỏ qua, chỉ `VIS_SUMDB=off`; `VIS_PRIVATE=<mẫu>`; `proxy.vifast.org` **chỉ thiết kế**, `VIS_PROXY=direct` mặc định. **4 ca đỏ-trước bắt buộc** (tag đổi nội dung · MVS v1.2/v1.4 · vanity → GitHub · offline + cache). **ST-178 sửa `vs3-qd-6.md`.** | tài liệu VS3 (ST-178) |
| **VD-181** | **Đường dẫn gói chính thức = `vifast.org/vis/<tên>`** — **path trên một host**, **KHÔNG** subdomain làm tiền tố module; `<tên>` chữ thường `[a-z0-9-]`; **không alias `/x/`**; tài liệu `vifast.org/docs/vis/…`. Dịch vụ ngang hàng **không lồng** dưới `vis.`: `sum.vifast.org`, `proxy.vifast.org`. `vis.vifast.org` **không** làm tiền tố module. `viscript.org` chỉ là tài sản thương hiệu. **Bất biến**: sau gói đầu tiên, tên/đường dẫn **không đổi**. | tài liệu VS3 (ST-178) |

**ST-171 (tách repo) — ĐÃ MỞ KHOÁ** (VD-178), chạy **sau R29**, phạm vi theo **VD-179 §4**: `viscript/` +
`docs/viscript/` + mọi crate ViScript theo `cargo metadata` + mọi đường dẫn mà các crate đó phụ thuộc; kiểm
sau tách: `cargo build` và gate ở repo mới **không cần tệp nào từ repo cũ**. Điều kiện xong (a)–(g) giữ
nguyên theo VD-165 + VD-167(1) (chuyển **cả** `docs/viscript/`).

## 6. Việc kế

1. **ST-171** — tách ViScript thành repo riêng **"ViScript"** (Agent 1 điều phối, Agent 2 thực thi), **3 h**,
   chạy **sau R29**. Điều kiện: `git filter-repo`/`subtree split` giữ đủ lịch sử; chuyển sổ nợ/QUEUE/JOURNAL/
   báo cáo/`check_report` + cập nhật đường dẫn, `check_report --all` **PASS ở repo mới**; giữ nguyên số nợ và
   số task; cổng repo mới trên **mac/ps/psvn cùng số đo với R29** (lệch thì giải thích từng suite); repo cũ
   xoá mã ViScript, giữ **một dòng trỏ** sang; cập nhật `vis-remote` + đường clone trên ps/psvn.
2. **ST-178** — sửa tài liệu theo **VD-178** (mọi [SỬA] của QĐ-1…6, C1 mã thoát, C2 R1–R24, C3) + cập nhật
   cheat sheet; **0 dòng mã**; mỗi [SỬA] ghi “đã sửa: tệp:dòng”; gửi A0 diff. **BLOCKED** tới khi ST-171 xong.
3. **ST-179+** — **pha 1 công thái học** theo thứ tự **C3** của VD-178 (P0 → R9(+C1) → R6 → R17 → R1+R2 →
   R3 → R8 → R4 → R5 → R11 → R12 → R16 → R7 → R19b → R19c), mỗi task ≤3 h, **đỏ-trước**, gate PASS.
4. **`#147`** — chờ A0 quyết định hướng (gộp `run_rules`/`report-check` để không tạo tiến trình mỗi tệp, đo
   lại sau) **hoặc** ghi nhận “macOS phạt syscall là **giới hạn nền**”. **Không** tự sửa mã/ngưỡng.
5. **`#140/#109b`** (đường ghi) + **`#145`** (`rt.rs` chú thích `chan(0)`) — xử trong VS3 cùng `rustix`/
   `openat2`; **không** làm trong VS2.

## 7. Hạn chế bằng chứng (ghi trung thực)

1. **mac đo trên cây làm việc** (không phải clone sạch): `git status --porcelain -- viscript crates
   Cargo.toml Cargo.lock` có **3** mục — **toàn bộ** là log `st170-*` do task này tạo. **Không** tệp mã nào
   bị sửa; `git diff 39041b3f..HEAD -- viscript crates Cargo.toml Cargo.lock` **rỗng**. Đây là thông lệ của
   dòng ViScript (mac là nơi ghi log), khác với `ps` (đo trên clone sạch theo VD-179 §3).
2. **psvn chỉ đo** (VD-146): số ca Windows **không** so trực tiếp với mac/ps; ~100 ca đỏ thuộc **VS3 mục 5**
   (bảng `st169`). Không tuyên bố hỗ trợ Windows.
3. **Thời gian gate phụ thuộc tải máy**: mac load **1,79 → 4,83** trong lúc chạy (tăng do chính phép đo);
   ps load thấp. Tỉ lệ **8,32×** ở §2.6 là **dữ kiện**, không phải ngưỡng; **#147** vẫn dùng số **soak**
   (5,70×) làm căn cứ đóng/mở.
4. **`#146` đóng bằng ca tất định**, không bằng số vòng: 6 000 000 vòng chỉ là **bằng chứng phụ** (0 đỏ).
   Giới hạn **hardlink** vẫn còn (chính sách, không phải lỗi đua) — ghi `HOST-BAO-DAM.md`.
5. **Ba gate `ps` cũ** (ST-161/166/167) **không** chứng minh được cây sạch tại đúng phút chạy — chỉ “sạch
   theo kiểm hồi tố”. R29 **không** dùng chúng làm số chuẩn (§4.3).
6. **Đổi URL origin của clone `ps`** sang GitHub là việc **ngoài phạm vi “chỉ đổi tên”** của QUEUE ST-170
   mục 2 — làm vì origin cũ (`/root/vifast-linux`) trỏ vào đường vừa đổi tên nên `git fetch` sẽ hỏng. Ghi ở
   `st170-doi-ten-cay-cu.log` để Agent 1/A0 đối chiếu; **không** đụng cây cũ.

## 8. Kết luận

1. **R29 có số chuẩn ba nền cùng HEAD `39041b3f`**: mac **PASS 10/10 · L0 1184/0 · 131 suite · 326,2 s ·
   `lock=yes`**; ps **PASS 10/10 · L0 1188/0 · 131 suite · 39,2 s · `lock=yes`** trên **clone sạch**;
   psvn **chỉ đo** trên cùng HEAD.
2. **`#146` đã đóng** (ST-174): **2** lần gate đỏ trước vá (đếm được, có log), sau vá **100/100 tất định**
   trên mac **và** ps + **6 000 000 vòng 0 đỏ** + gate mac **3 lần PASS**.
3. **`#147` vẫn mở**: **5,70×** > 3×; nguyên nhân là **tạo tiến trình/syscall macOS**, không phải CPU.
4. **Cây `ps` cũ đã đổi tên** đúng VD-179 §3, HEAD **trước = sau**, không xoá/khôi phục tệp nào.
5. **Khe hở bằng chứng cũ đã xử lý bằng lời**: 3 gate `ps` cũ = **tham chiếu**, không phải số chuẩn.
6. **ST-171 đã mở khoá** (VD-178) và là việc kế tiếp; **ST-178** `BLOCKED` tới khi có repo mới.
