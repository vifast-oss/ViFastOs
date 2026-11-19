# KT-235 — xác nhận trạng thái cuối đêm 2026-11-20

**Vai**: sub-agent L58 của Agent 1 (lead). **Worktree**: `/private/tmp/night3-L58-kt235`,
nhánh `night3/L58-kt235`, cắt từ `cbc2c2af`. **Chế độ**: chỉ đọc + chạy; **không** sửa tệp nguồn,
**không** push, **không** `gate --fast`. Chỉ `./v2/bin/v2 check`.
**Thời điểm chạy**: 2026-11-20 08:21–08:35 JST (2026-11-19 23:21–23:35Z).

---

## KẾT LUẬN

```text
KET LUAN: CO LECH
Commit:   cbc2c2af
Mục khớp: 13/15
Mục lệch: #14 (STATE.md header), #15 (BAO-CAO + PHIEU số liệu cũ)
v2 check: PASS (3 m 07 s)
```

**Bản chất lệch**: cả hai mục lệch đều là **tài liệu cũ**, **không** phải cổng, mã, ảnh hay
sổ QUEUE/DECISIONS. Trạng thái máy-kiểm-được (HEAD, cây sạch, `v2 status`, `v2 check`, hai lớp
kiểm, gate log, K6, ảnh, QUEUE, DECISIONS) **khớp 13/13**.

---

## A. Trạng thái

| # | Kiểm | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 1 | `git log --oneline -1` | `cbc2c2af` | `cbc2c2af [KT-231] so: KT-229/230/231 DONE — DONE=202 READY=0 REVIEW=2` | **KHỚP** |
| 2 | `git status --porcelain` | sạch | rỗng (0 dòng) — đo **trước** khi tạo tệp sản phẩm KT-235 | **KHỚP** |
| 3 | `./v2/bin/v2 status` | `DONE=202 READY=0 BLOCKED=0 REVIEW=2`, `dirty: 0`, fp `43df126a4a6702ec` | đúng cả bốn | **KHỚP** |
| 4 | `./v2/bin/v2 check` | PASS | `v2 check: PASS (L1 chạy trong gate --fast)`, exit 0 | **KHỚP** |

Nguyên văn `v2 status`:

```text
== dòng OS ViFastOs ==
HEAD        cbc2c2af5fc0 (night3/L58-kt235)  chưa push: 0
build-inputs 233 mục  dấu vân tay: 43df126a4a6702ec  dirty: 0
ảnh mới nhất (không có trong v2/artifacts/)
task        DONE=202 READY=0 BLOCKED=0 REVIEW=2
cổng        ./v2/bin/v2 check | ./v2/bin/v2 check-rules | ./v2/bin/v2 gate --fast
```

- Dòng `ảnh mới nhất (không có trong v2/artifacts/)` là **đúng cho worktree**: `v2/artifacts/`
  là gitignored nên worktree phụ không có thư mục này. Ảnh nằm ở checkout chính — xem §D.
- `v2 check`: **START 2026-11-19T23:21:27Z → END 2026-11-19T23:24:34Z = 3 m 07 s**, exit 0.
  Log: `v2/agent/JOURNAL/kt235-logs/kt235-check.log`.

---

## B. Hai lớp kiểm

| # | Lệnh | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 5 | `check-wake-paths.py --selftest` | 295/295 | `# selftest: 295/295 ok`, exit 0 | **KHỚP** |
| 6 | `check-wake-depinfo.py --selftest` | 37/37 | `# selftest: 37/37 ok`, exit 0 | **KHỚP** |
| 7 | `check-wake-paths.py --strict` | `errors=0` | `mode=v2 strict=1 errors=0 warnings=0`, exit 0 | **KHỚP** |
| 8 | `check-wake-depinfo.py --root "$PWD"` | `errors=0` | `errors=0 warnings=0 (18.45 s)`, exit 0 | **KHỚP** |

Chi tiết lớp 2: 2 đơn vị cargo (`v2/Cargo.toml` RC=0 4.25 s; `v2/kernel/Cargo.toml`
`--target riscv64gc-unknown-none-elf` RC=0 5.28 s), 145 tệp `.rs` nạp, 2 tệp cửa wake,
59 tệp ngoài phạm vi luật wake tĩnh.

---

## C. Gate

| # | Kiểm | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 9a | Dòng cuối `kt231-gate-pass.log` | `v2 gate --fast: PASS` | `v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)` | **KHỚP** |
| 9b | Số dòng `--- …: OK` | **52** | **52** | **KHỚP** |
| 9c | Bước bị bỏ qua | không | không | **KHỚP** |

**Đối chiếu 52 dòng OK với mã nguồn `v2/bin/v2`** (không tin con số khai):

- `do_check` (dòng 281–467): **25** `run_step` + **13** bước inline = **38**
  - 25 `run_step`: fmt, clippy, self-test luật, luật máy K6, selftest build-inputs, selftest
    danh tính ảnh, selftest t26, ghim thuật toán, selftest ghim, selftest shell-edge, selftest
    budget, selftest counter-evidence, selftest task-target, bảng đối chiếu, selftest bảng đối
    chiếu, selftest wiring, cổng selftest, wake paths, selftest wake paths, wake paths
    `--strict`, wake theo dep-info, selftest wake dep-info, test có răng, selftest test có răng,
    build (L0 compile).
  - 13 inline: luật cấu trúc, abi spec, golden trace, scn, scn fixtures, báo cáo, báo cáo
    fixtures, abi reserved, abi fixtures, gen-check, gen fixtures, idl-gen-check, idl fixtures.
- `do_gate_fast` (dòng 469–535): **13** `run_step` + **1** inline (L1) = **14**
  - 13 `run_step`: L0 test, L0 kernel workspace, selftest runner ktest, selftest qemu guard,
    selftest pre-measure, selftest pin-artifact, `algo=` nghiêm ngặt, selftest persist
    đỏ-trước, cắt điện giữa lúc ghi, selfbreak 5 luật K6, ca âm git-guard, ca âm git-guard
    song song, ca âm git-guard b.
- **38 + 14 = 52** — khớp đúng từng tên bước với 52 dòng OK trong log. **Không bước nào bị bỏ qua.**
  (Khớp cả kết luận độc lập của D2-177: "52/52 bước `--- …: OK` đúng thứ tự (38 `do_check` +
  14 `do_gate_fast`)".)

**Lưu ý phạm vi**: gate PASS chạy trên `a08bf042` (dòng bất biến của log:
`refs/heads/main : a08bf042…`, `HEAD : a08bf042…`). HEAD hiện tại `cbc2c2af` = `a08bf042` + **3
commit chỉ-tài-liệu** (`a42a3357`, `1825a52b`, `cbc2c2af`); `git diff --name-only a08bf042..HEAD`
**không** chạm tệp mã nào ⇒ kết quả gate vẫn hiệu lực cho cây mã hiện tại.

---

## D. K6

| # | Kiểm | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 10a | `soak: ok=1000 fail=0` | có | `# soak: xong wall=297s (3 boot/s); ok=1000 fail=0 tổng=1000/1000` (dòng 1513) | **KHỚP** |
| 10b | `data+bss=224288` | có | `# image=product … file=213632 data+bss=224288 budget=262144` (dòng 56, 498) | **KHỚP** |
| 10c | `shell_prompt_lines=2` | có | `### smp=1/2/4 shell_prompt_lines=2` (dòng 13, 20, 27) | **KHỚP** |
| 11a | Ảnh `639ca6d1…` trong `v2/artifacts/` checkout chính | có | `639ca6d16eb7…dc81.bin`, 213 632 B, 2026-11-20 01:12 | **KHỚP** |
| 11b | sha256 khớp tên tệp | khớp | `shasum -a 256` = `639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81` = tên tệp | **KHỚP** |
| 11c | Kích thước | 213 632 | `stat -f %z` = 213632 | **KHỚP** |

**Ghi chú định dạng (không phải lệch số)**: `kt210-k6-remeasure.md` bảng F4 viết gọn
`soak: ok=1000 fail=0 wall=297s`; log gốc đặt `wall=297s` **trước** `ok=1000 fail=0`. Ba con số
đều có trong log; chỉ thứ tự chuỗi khác. Không có số nào bị sai.

---

## E. Sổ

| # | Kiểm | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 12 | QUEUE đếm `\| DONE \|`… khớp `v2 status` | 202/0/0/2 | 202/0/0/2 | **KHỚP** |
| 13 | DECISIONS có D2-169..D2-181 | đủ | đủ **13** mục (tổng 181 mục D2) | **KHỚP** |
| 14 | STATE header khớp HEAD `cbc2c2af` | khớp | **`a08bf042`** (và `DONE=199`) | **LỆCH** |

**#12 — cách đếm**: dùng đúng regex của `v2/tools/status.sh` (dòng 56–59):
`^\| [A-Z]+-[0-9]+[a-z]? \|.*\| <TRẠNG_THÁI> \|` ⇒ **DONE=202, READY=0, BLOCKED=0, REVIEW=2**,
khớp `v2 status`. Đếm thô "chứa `| DONE |` bất kỳ" cho 204/0/1/3 vì cộng thêm **dòng 5** (legend
`READY | BLOCKED | REVIEW | DONE`) và **hai hàng ID không chuẩn** `KT-T26` (dòng 118), `KT-R1`
(dòng 130) — hai hàng này `status.sh` **cố ý** không đếm (ID không phải `KT-<số>`). Tổng hàng
bảng = 206 = 202 + 2 REVIEW + 2 hàng đặc biệt. **Không có mâu thuẫn.**

**#14 — nguyên văn lệch**:

```text
STATE.md:7   HEAD          : `a08bf042` (`main`, đã push — `chưa push: 0`). Commit cuối `[KT-231]`.
STATE.md:19  TASK          : `./v2/bin/v2 status` ⇒ **DONE=199 READY=0 BLOCKED=0 REVIEW=2**.
```

Thực tế: HEAD `cbc2c2af`, `DONE=202`. Nguyên nhân: commit `cbc2c2af` ("so") **chỉ** sửa
`v2/agent/QUEUE.md` (3 dòng thêm) — **không** cập nhật `STATE.md`. `STATE.md` được cập nhật lần
cuối ở `a42a3357` (lúc đó ghi HEAD `a08bf042`, DONE=199 — đúng thời điểm), nên nay cũ **3 commit**
và **3 task DONE**.

---

## F. Báo cáo

| # | Kiểm | Kỳ vọng | Thực tế | Kết |
|---|---|---|---|---|
| 15a | `BAO-CAO-DEM-2026-11-20.md` tồn tại | có | có (14 021 B) | **KHỚP** |
| 15b | `PHIEU-GUI-AGENT0-2026-11-20-VONG8.md` tồn tại | có | có (7 946 B) | **KHỚP** |
| 15c | Số liệu khớp thực tế | khớp | **không khớp** (xem dưới) | **LỆCH** |

**Ba con số kiểm ở `BAO-CAO-DEM-2026-11-20.md`**:

| Số | Tệp khai | Thực tế | Kết |
|---|---|---|---|
| HEAD | `af1ba3c9` (dòng 12) | `cbc2c2af` | **LỆCH** — `af1ba3c9` là commit `[KT-219]`, **37 commit** sau HEAD |
| DONE/READY/REVIEW | `DONE=189 READY=1 BLOCKED=0 REVIEW=3` (dòng 17, lặp ở dòng 154) | `202/0/0/2` | **LỆCH** |
| selftest lớp 1 / lớp 2 | `246 ca / 20 ca` (dòng 19) | `295 / 37` | **LỆCH** — nhưng **§2.3 cùng tệp (dòng 58) ghi đúng `295` / `37`** |

⇒ Tệp báo cáo **tự mâu thuẫn**: khối "Trạng thái một khối" (đầu tệp) và §4.5 còn ở mốc
`af1ba3c9`, trong khi §2.3 đã cập nhật `295/37`. Commit `a42a3357` chỉ vá §2.3 + thêm D2-179..181
+ L54, **không** làm mới khối đầu tệp.

**Ba con số kiểm ở `PHIEU-GUI-AGENT0-2026-11-20-VONG8.md`**:

| Số | Tệp khai | Thực tế | Kết |
|---|---|---|---|
| HEAD | `a08bf042` (dòng 12) | `cbc2c2af` | **LỆCH** (3 commit chỉ-tài-liệu) |
| DONE | `199` (dòng 17) | `202` | **LỆCH** |
| selftest lớp 1 / lớp 2 | `295 / 37` (dòng 19) | `295 / 37` | **KHỚP** |

---

## Tổng hợp 15 mục

| # | Mục | Kết |
|---|---|---|
| 1 | HEAD `cbc2c2af` | KHỚP |
| 2 | Cây sạch | KHỚP |
| 3 | `v2 status` 202/0/0/2, dirty 0, fp `43df126a4a6702ec` | KHỚP |
| 4 | `v2 check` PASS | KHỚP |
| 5 | Lớp 1 selftest 295/295 | KHỚP |
| 6 | Lớp 2 selftest 37/37 | KHỚP |
| 7 | Lớp 1 `--strict` errors=0 | KHỚP |
| 8 | Lớp 2 `--root` errors=0 | KHỚP |
| 9 | Gate log 52 OK + PASS + không bỏ qua | KHỚP |
| 10 | K6 ba con số | KHỚP |
| 11 | Ảnh `639ca6d1…` sha256 + 213 632 B | KHỚP |
| 12 | QUEUE 202/0/0/2 | KHỚP |
| 13 | DECISIONS D2-169..181 (13) | KHỚP |
| 14 | STATE header khớp HEAD | **LỆCH** |
| 15 | Báo cáo + phiếu số liệu khớp | **LỆCH** |

**13/15 khớp. 2/15 lệch — cả hai là tài liệu cũ, không phải cổng/mã/ảnh/sổ.**

---

## Đề xuất (không tự sửa — ngoài quyền L58)

1. `STATE.md` dòng 7 + 19: cập nhật `a08bf042`→`cbc2c2af`, `DONE=199`→`DONE=202` (hoặc ghi rõ
   "HEAD lúc cổng" tách khỏi "HEAD hiện tại" để khỏi lệch lần sau).
2. `BAO-CAO-DEM-2026-11-20.md` khối đầu + §4.5: làm mới `af1ba3c9`→`cbc2c2af`,
   `DONE=189 READY=1 REVIEW=3`→`202/0/0/2`, `246/20`→`295/37` (đồng bộ với §2.3).
3. `PHIEU-GUI-AGENT0-2026-11-20-VONG8.md` dòng 12 + 17: `a08bf042`→`cbc2c2af`, `199`→`202`.
4. Quy trình: commit "so" cuối đêm nên cập nhật **cả bốn** tệp sổ (QUEUE, STATE, báo cáo, phiếu)
   trong **một** commit, hoặc thêm bước kiểm tự động "STATE HEAD == HEAD" vào `v2 check`.

---

## Tệp

| tệp | nội dung |
|---|---|
| `v2/agent/JOURNAL/kt235-verify-final.md` | tệp này |
| `v2/agent/JOURNAL/kt235-logs/kt235-status.log` | nguyên văn `v2 status` |
| `v2/agent/JOURNAL/kt235-logs/kt235-check.log` | nguyên văn `v2 check` (PASS, 3 m 07 s) |
| `v2/agent/JOURNAL/kt235-logs/kt235-evidence.log` | nguyên văn các lệnh kiểm §B, §C, §D, §E |
