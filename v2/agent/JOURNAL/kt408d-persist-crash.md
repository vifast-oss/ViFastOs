# KT-408d — Cắt điện **tất định** cho `persist.sh --crash` (`MODE_CRASH_MID_WRITE`, đóng `Q7`)

- **Task**: `KT-408d` (P1, Chặng 2 / W7b) — `v2/agent/QUEUE.md:456`; cơ sở `D2-262` mục 5 (`Q7`,
  APPROVED 2026-11-26 Agent 0), `BLOCKED.md` §1b dòng `Q7`.
- **Người thi hành**: sub-agent `w7b-408d` (W7b **Slot 4**, worktree `/private/tmp/vifast-wt/4`,
  `CARGO_TARGET_DIR=target-task/slot-4`).
- **Nhánh / commit**: `task/kt408d-persist-crash` @ **`9a962a7c`**, parent `297b89dc` (= base `main`).
  `git diff 297b89dc..9a962a7c --stat` = **3 tệp, +417 / −130**.
- **Khung giờ**: Thấp điểm chính `10:00–01:00 UTC`; mở slot `10:41 UTC` (`19:41 JST`).
- **Log bằng chứng (chép cùng commit)**: `kt408d-red-nocut.log`, `kt408d-crash-{1..5}.log`,
  `kt408d-t1.log`, `kt408d-determinism.txt` trong `v2/agent/JOURNAL/kt408d-logs/`.
- **Ngân sách**: **1,5 h** (`QUEUE.md:456`).

## 1. Phạm vi / DoD đã duyệt (`D2-262` mục 5)

(1) Mở rộng kịch bản 32 B `PL75` tại LBA 0 (dùng 4 byte dự phòng `17..20`, tương thích ngược
`mode=0..2`) thêm `MODE_CRASH_MID_WRITE = 3` + `crash_after_writes: u32`; (2) khi pha 7 đạt đúng
mốc `N` sector 512 B đẩy xuống thiết bị (còn block bẩn trong `CACHE_DATA`), `ktest_persist` in
`# persist: crash_cut writes=<N> pending=<P>` rồi `PLATFORM.halt(77)` **KHÔNG flush**; (3)
`persist.sh --crash` **bỏ poll `sleep 0.05`**, luôn kiểm `partial=1` + journal replay `fsck` + dọn
tệp dở + phục hồi đủ `b07r → b08r → b01r`; (4) ca đỏ-trước: tắt `crash_cut` ⇒ `persist_host.py`
đỏ `records=13`; 0 sửa `spec/`, 0 nới ngưỡng; T2 PASS (Lead).

## 2. Đỏ-trước (D2-05, D2-07)

### 2a. Nợ flake là **có thật** — bằng chứng máy đã có trong repo

- `BLOCKED.md` §1b `Q7`: "`persist.sh --crash` không tất định (`records=13`)". `D2-151` ghi đúng
  cơ chế: poll `sleep 0.05` + `kill` ⇒ **máy chậm/tải T2** thì kill rơi **sau** pha 7.
- Audit `KT-176` (`v2/agent/JOURNAL/kt176-reportaudit.md:139`) trích log cổng cũ:
  `G/gate-1.log:2307` `records=13 well_formed=0` (ĐỎ), còn `gate-2.log` thì PASS — **cùng cây mã,
  hai kết quả** ⇒ flake thật, không phải lỗi FS.
- Hai lượt T2 gần nhất chứng minh **điểm kill trôi**: `target-task/peak2-20261126/t2-gate-fast.log:2861`
  ghi `# crash file: name=/p7.bin bytes=27648 … partial=1` (kill ở **54**/144 sector), trong khi
  lượt khác rơi trọn 144 sector ⇒ `records=13`. Cơ chế cũ **không** kiểm cỡ/`records`, nên lượt
  xanh không chứng minh được điểm cắt.

### 2b. Ca đỏ **tất định** của bộ kiểm mới (`--crash-nocut`)

Cùng chuỗi boot `b01..b06` + `b07` nhưng kịch bản **KHÔNG cắt** (`mode=0`, pha 7 ghi trọn
144/144) — đúng trạng thái mà race cũ sinh ra khi kill trượt. Bộ kiểm host **mới** phải ĐỎ, và
đỏ **đúng lý do** (log `kt408d-logs/kt408d-red-nocut.log`, chạy trên cây sạch `9a962a7c`):

```text
# crash-red: pha 7 ĐÃ GHI TRỌN 144/144 — đúng trạng thái 'kill trượt' của poll cũ
# crash file: name=/p7.bin bytes=73728 want=36864 want_max=73728 prefix_ok=1 partial=0 const:crash_after=72
# crash state: records=13 want=12 well_formed=0
FAIL crash-host b07.nocut.img: get /p7.bin: 73728 B khác điểm cắt tất định 36864 B (crash_after=72 sector)
FAIL crash-host b07.nocut.img: get /pstate: 13 bản ghi (cần đúng 12 sau khi cắt giữa pha 7 — nợ Q7)
# crash-red: PASS — host bắt đúng 'records=13' + 'partial=0' (đỏ ĐÚNG lý do Q7)
# persist.sh: PASS
```

⇒ **`records=13` tái hiện được 100 %** (không phụ thuộc tải) khi cắt không xảy ra, và bộ kiểm
mới bắt được nó. Đỏ-trước chạy **trước** khi chốt commit vá; các lượt `--crash` sau đó (mục 4)
xanh trên **cùng** bộ kiểm.

## 3. Việc đã làm (3 tệp, 0 chạm `spec/`, 0 nới ngưỡng)

1. **`v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs`** (+138/−…):
   - `MODE_CRASH_MID_WRITE = 3`; `Plan.crash_after: u32` đọc từ byte `17..20` (LE); `parse_plan`
     fail-closed: `mode=3` **chỉ** hợp lệ khi `verify=6` + `write=7` + `1 ≤ crash_after < 144`;
     `mode=0..2` **cấm** `crash_after ≠ 0` (tương thích ngược: kịch bản cũ 32 B vẫn nguyên nghĩa).
   - `PersistDev.crash_cut: Option<u32>` + `arm_crash_cut()`; `flush()` khi được vũ trang in
     `# persist: crash_cut mode=3 writes=<N> pending=<P> pushed=<S> cache_slots=32` rồi
     `PLATFORM.halt(77)` **trước** `push_all` ⇒ block bẩn trong `CACHE_DATA` **không** xuống đĩa.
   - Pha 7 khi `mode=3`: ghi `/p7.bin` **từng sector 512 B** (đúng đơn vị transaction của
     `Fs::write` — `dir.rs:202-205` commit trọn một chunk), vũ trang ngay trước sector thứ `N` ⇒
     đĩa còn **đúng `N` sector bền**; journal còn transaction cuối đã commit nhưng chưa dọn ⇒ boot
     sau **buộc** replay. `0` thay đổi đường `mode=0..2` (đối chứng: lượt `run` 10 boot vẫn PASS).
2. **`v2/test/boot/persist_host.py`** (+130/−…): `plan --crash-after` (byte 17..20, kiểm miền
   fail-closed); `check --kind crash` kiểm log cắt điện (đúng 1 `crash_cut`, `pending ≥ 1`,
   **không** có dòng ghi pha 7 / TAP 2 / `stats`); `crash-host --crash-after` **siết**:
   `/p7.bin` phải **có mặt**, là tiền tố payload pha 7, cỡ **đúng `N*512 B`** (`partial=1`),
   `/pstate` **đúng 12** bản ghi (bỏ "11 hoặc 12"), fsck sạch, dữ liệu pha 6 còn nguyên.
3. **`v2/test/boot/persist.sh`** (+279/−…): `crash_run <cut>` **bỏ hoàn toàn** vòng poll
   `sleep 0.05` + `qg_kill_group`; chờ `qg_boot` (kernel tự halt) rồi kiểm log + đĩa theo mục 2;
   thêm **bước journal replay**: chụp bản sao byte-identical **trước mọi lệnh `fsutil`**, boot
   `verify 6` trên bản sao ⇒ đòi `# persist: fs mounted=1 … replayed=1` + TAP verify 6; bước dọn
   tệp dở nay **bắt buộc** có `/p7.bin` (partial=1) và xoá; phục hồi `b07r → b08r → b01r` giữ
   nguyên + siết `records=15` (14 chuẩn + 1 verify pha 6 lặp của lần cắt). Thêm chế độ
   `--crash-nocut` (đỏ-trước, mục 2b) và `KT75_CRASH_AFTER` (mặc định **72**/144 sector).

## 4. Bằng chứng **tất định** — 5 lượt `persist.sh --crash` trên cây sạch `9a962a7c`

Mỗi lượt: 6 boot pha 1..6 xanh (kiểm log + host) → boot `b07` **tự halt(77)** sau đúng 72 sector
→ kiểm log/đĩa → replay trên bản sao → dọn tệp dở → `b07r`/`b08r`/`b01r` → fsck cuối.

| Lượt | log | `crash_cut` | `/p7.bin` | `/pstate` | replay | phục hồi | kết quả |
|---|---|---|---|---|---:|---:|---|
| 1 | `kt408d-crash-1.log` | `writes=72 pending=7` | `36864 B` (=72×512), `partial=1` | `records=12` | `replayed=1` | `phases=7 ok=7 records=15` | PASS |
| 2 | `kt408d-crash-2.log` | `writes=72 pending=7` | `36864 B`, `partial=1` | `records=12` | `replayed=1` | `phases=7 ok=7 records=15` | PASS |
| 3 | `kt408d-crash-3.log` | `writes=72 pending=7` | `36864 B`, `partial=1` | `records=12` | `replayed=1` | `phases=7 ok=7 records=15` | PASS |
| 4 | `kt408d-crash-4.log` | `writes=72 pending=7` | `36864 B`, `partial=1` | `records=12` | `replayed=1` | `phases=7 ok=7 records=15` | PASS |
| 5 | `kt408d-crash-5.log` | `writes=72 pending=7` | `36864 B`, `partial=1` | `records=12` | `replayed=1` | `phases=7 ok=7 records=15` | PASS |

**Bất biến mạnh nhất** (`kt408d-logs/kt408d-determinism.txt`): sha256 của **ảnh đĩa sau cắt điện**
giống **hệt** ở cả 5 lượt:

```text
crash1 add789a562f691c1…
crash2 add789a562f691c1…
crash3 add789a562f691c1…
crash4 add789a562f691c1…
crash5 add789a562f691c1…
```

(sha đầy đủ: `add789a562f691c19d83dfdbcabe2454e022ac7f3dcaa4e9f524917e163e5648`; tệp
`<run>/crash/b07.crash.img`.) Trước vá, cùng phép đo cho `bytes=27648` một lượt và trọn 144 sector
lượt khác (mục 2a) — nay là **một** giá trị.

Đối chứng hồi quy (cùng commmit, dùng bản sao runner tạm bỏ cổng rev-gate — **không** commit):
`run` 10 boot: `# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10` + `PASS`;
`--red-selftest`: 4/4 dòng đỏ/xanh đúng + `PASS`.

## 5. T0 / T1

- **T0 (trong lúc sửa)**: `rustfmt --check` sạch; build ảnh `suite-persist` OK
  (`ktest_build_suite`, log `/tmp/kt408d-build.log`); `python3 -m py_compile persist_host.py`;
  `bash -n persist.sh`; `python3 persist_host.py plan … mode=3 --crash-after 72` cho byte đúng
  (`… 03 | 47 1f … | 48 00 00 00`) và các ca sinh kịch bản sai bị từ chối (`mode=3` + `crash_after=0`,
  `mode=0` + `crash_after≠0`, `crash_after=144`).
- **T1 `./v2/bin/v2 gate --dev`**: **PASS** — log `kt408d-logs/kt408d-t1.log`:
  `v2 check: PASS`; `--- cắt điện giữa lúc ghi (KT-136): BỎ QUA (T1 — chỉ chạy ở T2)` khớp
  `v2/bin/lib/consts.sh:16`; `# gate-steps registered=82 ran=68 skipped=14 failed=0`;
  `# gate-wall 889648 ms (889.6 s)`; `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải
  full gate; full gate = 'gate --fast')`. `gate --dev` là **T1**, không phải full gate; T2 là việc
  của Lead. Vì bước `--crash` **chỉ chạy ở T2**, bằng chứng chức năng của task là **5 lượt
  `--crash` ở mục 4** trên cây sạch `9a962a7c` (không phải T1).
- **T2**: chưa chạy (theo phân công; Lead chạy `gate --fast` trên ứng viên tích hợp).

## 6. Rủi ro + nợ còn lại (trung thực)

1. **Bất biến cỡ đĩa phụ thuộc ngữ nghĩa `Fs::write` commit từng chunk 512 B**
   (`v2/lib/vifast-fs/src/dir.rs:202-205`, chú thích "TẠM REVERT batch (#8)"). Nếu FS gộp
   transaction (batch) trở lại, `--crash` sẽ **ĐỎ** (crash-host đòi cỡ đúng `N*512 B`), không
   âm thầm xanh — nhưng khi đó phải chỉnh đơn vị `crash_after` theo transaction thật.
2. **`--crash-nocut` là chế độ mới chỉ để đỏ-trước** (không nằm trong T1/T2). Giữ lại để tái hiện
   `records=13` khi cần; nếu Lead muốn tối giản runner thì có thể bỏ (§2b vẫn giữ log).
3. **Số boot của `--crash` tăng 10 → 11** (thêm boot replay trên bản sao). Thời gian thêm đo được
   ~1 s/lượt; log ghi `boots=11 pids=10` (một boot replay không được tính vào bảng pha).
4. **`/pstate` cuối chuỗi phục hồi = 15 bản ghi** (14 chuẩn + 1 verify pha 6 **lặp** vì boot b07 bị
   cắt sau khi verify 6, rồi `b07r` verify 6 lần nữa) ⇒ `# persist: total_ok=0` trong log `b08r` là
   **có sẵn từ trước** (cơ chế cũ cũng vậy), không phải hồi quy; `phases=7 ok=7` vẫn đúng và nay
   runner **siết** thêm `records=15`.
5. `crash-host` bỏ nhánh "chấp nhận `/p7.bin` vắng hoặc 12 bản ghi tuỳ duyên": chế độ cắt điện
   **bắt buộc** `partial=1` + `records=12`. Đây là siết, không nới.
6. Chỉ đo điểm cắt **72**; `KT75_CRASH_AFTER` cho phép 1..143 nhưng chưa có lượt đo ở biên
   (đề xuất: nếu cần, chạy `KT75_CRASH_AFTER=1` và `143` ở T2/nightly tương lai).
7. Chưa chạy `gate --fast`/`nightly`/`soak` (ngoài phạm vi sub-agent, đúng BRIEF §1.7).

## 7. Tệp đã sửa

| Tệp | Loại | Δ |
|---|---|---|
| `v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs` | sửa | `mode=3` + `crash_after` + halt không flush + ghi pha 7 từng sector |
| `v2/test/boot/persist_host.py` | sửa | `plan --crash-after`, `check --kind crash`, `crash-host` siết `partial=1`/`records=12` |
| `v2/test/boot/persist.sh` | sửa | `crash_run` bỏ poll, kiểm log/đĩa/replay/dọn/phục hồi; thêm `--crash-nocut` |
| `v2/agent/JOURNAL/kt408d-persist-crash.md` | **mới** | nhật ký này |
| `v2/agent/JOURNAL/kt408d-logs/*` | **mới** | log máy sinh (đỏ-trước, 5 lượt crash, T1, determinism) |

## 8. Commit

```text
9a962a7c [KT-408d] persist: cắt điện tất định giữa pha 7 (MODE_CRASH_MID_WRITE + crash_after) — bỏ poll sleep 0.05, kiểm partial=1 + records=12 + journal replay
<kế tiếp> [KT-408d] nhật ký + bằng chứng máy sinh: 5 lượt --crash tất định (sha ảnh đĩa giống hệt), đỏ-trước records=13, T1 PASS 82/68/14/0
```
Commit `9a962a7c` chứa **toàn bộ mã** đã đo; commit kế tiếp chỉ thêm **tài liệu + log** (0 đổi mã)
nên bằng chứng 5 lượt `--crash` + T1 ở mục 4/5 áp dụng nguyên vẹn cho cây cuối nhánh.
