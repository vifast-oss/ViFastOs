# KT-408e — Chống lệch SSOT TAP cho `soak-{aarch64,x86_64}.sh` (`lib/tap-parity.sh` + probe fail-fast)

- **Task**: `KT-408e` (`P1`, Đợt `W7b` Slot 4), quyết định **`D2-263`**; phụ thuộc `KT-412`,
  `KT-413c`, `KT-418`.
- **Nhánh**: `task/kt408e-tap-parity`, gốc `main` @ `297b89dc` (worktree `/tmp/vifast-wt/3`,
  `CARGO_TARGET_DIR=target-task/slot-3`).
- **Phạm vi đã chạm** (0 % chồng tệp 3 slot còn lại — `KT-417b`/`KT-418c`/`KT-408d`):
  `v2/test/boot/soak-aarch64.sh`, `v2/test/boot/soak-x86_64.sh`,
  **mới** `v2/test/boot/lib/tap-parity.sh`. **0 chạm** `run-*.sh`, `v2/bin/**`, `spec/**`,
  `v2/kernel/**`, `v2/user/**`.
- **Ngân sách**: 1,5 h. Không chạy `gate --fast`, `nightly`, soak đầy đủ (1.000×3), `l1-tier`,
  `mutants`.

## 1. Nguyên nhân gốc — hai lỗ hổng thật, không phải "một con số sai"

Cùng một ảnh kiểm mà **ba** nơi khai số ca TAP tách rời, không nơi nào đối chiếu:

| Nguồn | Trước `KT-408e` | Sau |
|---|---:|---:|
| `kbin-aarch64/src/main.rs::TAP_TESTS` | 50 | 50 |
| `run-aarch64.sh::EXPECT_TESTS` | 50 | 50 |
| **`soak-aarch64.sh::TAP_TESTS`** | **46** | **50** |
| `kbin-x86_64/src/main.rs::TAP_TESTS` | 50 (+`gpu`) | 50 (+`gpu`) |
| `run-x86_64.sh::EXPECT_TESTS` | 50 | 50 |
| **`soak-x86_64.sh::TAP_TESTS`** | **47** | **50** |

- **(i) Nhân bản SSOT**: `--selftest` của soak chỉ chấm trên **fixture do nó tự dựng**, nên nó
  vẫn XANH trong khi soak thật lệch 3–4 TAP. Bằng chứng Peak 2 (`becc1756`), bản sao **đã commit**
  trong nhánh: `v2/agent/JOURNAL/kt408e-tap-parity/peak2-soak.log` (dòng
  `# soak-aarch64.sh: FAIL (1000/1000 boot đỏ)` + `#   fail … plan,'1..46',…,cần,đúng,1;…cần,46`)
  và `v2/agent/JOURNAL/kt408e-tap-parity/peak2-nightly-step-2-soak.log` (log từng boot:
  `boot 2/1000 FAIL: # plan '1..46' xuất hiện 0 lần, cần đúng 1` / `# có 50 dòng 'ok', cần 46`).
  Nguồn gốc (do Lead giữ, KHÔNG chép đè): `target-task/peak2-20261126/soak.log`,
  `target-task/nightly/nightly-20261126-060807-becc1756/step-2-soak.log`, manifest
  `v2/agent/JOURNAL/nightly/nightly-20261126-060807-becc1756/` — 971 s cho 0/N.
- **(ii) Thiếu cổng probe fail-fast ở `soak-aarch64.sh`**: probe cũ chỉ kiểm `# conf image:`
  khớp ELF, nên lệch TAP chỉ lộ **sau** khi đã đốt 1.000 boot. `soak-x86_64.sh` đã có cổng này
  (`check_boot_log` trên boot dò ⇒ `exit 3`) từ trước.

## 2. Thay đổi

### 2.1 `v2/test/boot/lib/tap-parity.sh` — **mới**, SSOT read-only, fail-closed

- `tap_parity_check [gốc-repo]`: đọc **mã thật** (grep + parser chặt), khẳng định
  `soak-<arch>.sh:TAP_TESTS == run-<arch>.sh:EXPECT_TESTS == kbin-<arch>/src/main.rs:TAP_TESTS`
  cho **cả** `aarch64` lẫn `x86_64`; kiểm thêm mỗi soak **có gọi** `tap_parity_cases` trong
  `--selftest` (mất lời gọi = tháo khoá im lặng ⇒ ĐỎ). Chỉ đọc **đúng 6 tệp** (2 cặp + 2
  `kbin/*/src/main.rs`), không quét `gpu-*.sh`/`product-*.sh` (kỳ vọng TAP khác do feature `gpu`).
- **Fail-closed**: thiếu tệp / không có khai báo / khai báo trùng / parse hỏng (`abc`, `46+1`,
  ngoài `[1,999]`) ⇒ ĐỎ kèm lý do; lệch (kể cả ±1) ⇒ ĐỎ kèm **đủ ba giá trị**.
- Tên hàm là `tap_parity_cases` **không** phải `*selftest`: `v2/tools/check-selftest-wiring.py`
  (KT-134) coi mọi tệp có hàm `*selftest()` là "script tự kiểm độc lập" và đòi một **lời gọi
  cổng** riêng — lib này không phải script độc lập (ca của nó chạy **bên trong** `--selftest`
  của 2 soak, vốn đã có cổng id 66/71). Đặt tên `*selftest` sẽ làm `v2 check` đỏ `E_ORPHAN`
  (đã đo thật ở lượt T1 đầu).
- `tap_parity_cases <số-ca-đầu>`: 1 ca dương trên cây thật + **10 ca âm** trên bản sao tạm
  (mktemp, xoá sau): soak ±1, run ±1, kbin ±1, thiếu tệp, parse hỏng, khai báo trùng,
  mất call-site; đặt `TAP_PARITY_CASES`/`TAP_PARITY_FAILS` để caller cộng vào tổng ca của nó.
- Ý nghĩa: từ nay mọi task tăng TAP (`KT-417c`/`419`/`419b`/`420b`…) mà quên đồng bộ soak sẽ bị
  T2 bắt trong **~0,3 s** thay vì đốt 971 s.

### 2.2 `soak-aarch64.sh`

- `TAP_TESTS` **46 → 50** (khớp `1..50` của ảnh và `EXPECT_TESTS` của `run-aarch64.sh`); comment
  ghi rõ nguồn `KT-412` (47..49) + `KT-418` (50) và việc số này bị `lib/tap-parity.sh` khoá chéo.
- `check_boot_log` thêm khối **(5d)** kiểm **đích danh**: `# smp core: taps=3 fails=0
  boot_harts=4`, `ok 47 - smp harts bootinfo`, `ok 48 - smp 4 hart preemptive`,
  `ok 49 - smp published rule`, `ok 50 - pan: el1 khong doc trang el0`, và dòng `# pan:` đúng
  một lần + kết luận `ok=1` (không đòi giá trị CPU cụ thể: `cortex-a72` = ARMv8.0 ⇒
  `pan_supported=0`, `-cpu max` ⇒ `pan_supported=1`, cả hai đều phải `ok=1`).
- `--selftest`: fixture `1..46` → `1..50` + 4 dòng TAP + `# smp core:` + `# pan:`; **3 ca âm mới**
  (mất `# smp core:`, mất `# pan:`, sai nhãn TAP 50); gọi `tap_parity_cases` (11 ca) trước tổng kết.
- **Probe fail-fast**: boot dò chuyển xuống sau `boot_keys()` và chạy bằng **cùng** đường bơm
  phím `lib/monitor-sendkey.py` (ảnh aarch64 có TAP 32 `input: key` phụ thuộc `sendkey a` — probe
  `qg_boot` trần in `not ok 32`), rồi chấm bằng **chính** `check_boot_log`; lệch TAP/bất biến ⇒
  `exit 3` (`TỪ CHỐI ĐO`) tại **boot 0**, không đốt 1.000 boot.

### 2.3 `soak-x86_64.sh`

- `TAP_TESTS` **47 → 50** (ảnh `50 + cfg!(feature="gpu")`, soak không bật `gpu` ⇒ 50).
- `check_boot_log` thêm khối **(11b)**: `ok 48 - core smp 4 hart`, `ok 49 - core smp sleep migrate`
  (`KT-413`), `ok 50 - smep/smap: bat va ca am` (`KT-418`), và **đúng 2** dòng `# smep/smap:` với
  cả hai kết luận `hardened=1` (kmain + `sec_probe`; `hardened=0` = CPU có mà chưa bật ⇒ ĐỎ).
- `--selftest`: fixture `1..47` → `1..50` + 3 dòng TAP + 2 dòng `# smep/smap:`; **3 ca âm mới**
  (mất `# smep/smap:`, `hardened=0`, sai nhãn TAP 49); gọi `tap_parity_cases` (11 ca).

## 3. Bằng chứng **đỏ-trước** (máy sinh; log **đã commit** trong nhánh tại
`v2/agent/JOURNAL/kt408e-tap-parity/`; bản chạy gốc ở `target-task/w7b/kt408e/`, trích **nguyên
văn** từ bản đã commit bên dưới)

1. **SSOT lệch thật** — `red-tap-parity-unpatched.log` (chạy `lib/tap-parity.sh` trên cây chưa vá):
   ```text
   # tap-parity: aarch64 LỆCH soak.TAP_TESTS=46 run.EXPECT_TESTS=50 kbin.TAP_TESTS=50
   # tap-parity: x86_64 LỆCH soak.TAP_TESTS=47 run.EXPECT_TESTS=50 kbin.TAP_TESTS=50
   RC=1
   ```
2. **aarch64 soak thật ĐỎ** — `red-soak-a64-smoke.log` (`--smoke 1 1`, cây chưa vá):
   ```text
   # probe: '# conf image:' khớp ELF (build=e79687034717ee3f text=227784)   ← probe cũ KHÔNG chấm bất biến
   boot 1/1 FAIL: # plan '1..46' xuất hiện 0 lần, cần đúng 1
   # có 50 dòng 'ok', cần 46
   # soak: 0/1 lỗi=1 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
   RC=1
   ```
3. **x86_64 soak thật ĐỎ (đã có fail-fast)** — `red-soak-x64-smoke.log`:
   ```text
   # soak-x86_64.sh: TỪ CHỐI ĐO — probe boot không đạt bất biến của một boot sạch:
   #   # plan '1..47' xuất hiện 0 lần, cần đúng 1
   #   # có 50 dòng 'ok', cần 47
   RC=3
   ```
4. **`--selftest` là điểm mù (xanh giả) trước khi vá** — `red-selftest-a64.log`,
   `red-selftest-x64.log`: `48/48 PASS`, `58/58 PASS` **dù** soak thật lệch 46/47 ↔ 50. Đây đúng
   lỗ hổng (i) của `D2-263`: selftest chỉ chấm fixture tự dựng.

## 4. Bằng chứng **xanh-sau** (cùng đường chạy)

1. `green-tap-parity-patched.log`: `# tap-parity: soak==run==kbin (aarch64=50 x86_64=50)` `RC=0`.
2. `green-selftest-a64.log`: `62/62 ca đúng như phải` + `PASS` (trước: 48/48), trong đó
   `ok 41..43` là 3 ca âm mới (thiếu `# smp core:` / thiếu `# pan:` / sai nhãn TAP 50) và
   `ok 52..62` là 11 ca tap-parity.
3. `green-selftest-x64.log`: `72/72 ca đúng như phải` + `PASS` (trước: 58/58), `ok 58..60` là
   3 ca âm mới (`# smep/smap:` / `hardened=0` / nhãn TAP 49), `ok 62..72` là 11 ca tap-parity.
4. `green-soak-a64-smoke.log` (`--smoke 1 1`):
   ```text
   # probe: '# conf image:' khớp ELF (build=e79687034717ee3f text=227784)
   # probe: log đạt mọi bất biến của một boot sạch (TAP 50/50 + marker + halt)
   boot 1/1 ok
   # soak: 1/1 lỗi=0 — 0/N chỉ là trần (QD §5.3)
   RC=1        ← hợp đồng `--smoke` luôn exit 1 (không phải bằng chứng cổng)
   ```
5. `green-soak-x86_64-smoke-retry1.log` (`--smoke 1 1`):
   ```text
   # probe: log đạt mọi bất biến của một boot sạch (TAP 50/50 + marker + halt)
   boot 1/1 ok
   # soak: 1/1 lỗi=0 — 0/N chỉ là trần (QD §5.3)
   RC=1
   ```
6. `check-selftest-wiring.py --root .` ⇒ `PASS (45 script tự kiểm: 43 có cổng, 2 ngoại lệ,
   0 mồ côi)` — lib mới không thành `E_ORPHAN` (hàm đặt tên `tap_parity_cases`, không phải
   `*selftest`, nên không bị coi là script tự kiểm độc lập; 11 ca vẫn chạy **trong** `--selftest`
   của 2 soak vốn đã có cổng gọi, id 66/71).

## 5. Ca âm chứng minh `tap-parity` bắt được lệch (máy sinh)

`green-selftest-a64.log`/`green-selftest-x64.log` (cùng bộ 11 ca, chạy 2 lần ở 2 runner):

```text
ok 53 - tap-parity: ca âm (soak-aarch64 lệch +1) ⇒ ĐỎ (aarch64 LỆCH soak.TAP_TESTS=51 run.EXPECT_TESTS=50 kbin.TAP_TESTS=50)
ok 54 - tap-parity: ca âm (soak-x86_64 lệch -1) ⇒ ĐỎ (x86_64 LỆCH soak.TAP_TESTS=49 run.EXPECT_TESTS=50 kbin.TAP_TESTS=50)
ok 55 - tap-parity: ca âm (run-aarch64 lệch +1) ⇒ ĐỎ (aarch64 LỆCH soak.TAP_TESTS=50 run.EXPECT_TESTS=51 kbin.TAP_TESTS=50)
ok 56 - tap-parity: ca âm (run-x86_64 lệch -1) ⇒ ĐỎ (x86_64 LỆCH soak.TAP_TESTS=50 run.EXPECT_TESTS=49 kbin.TAP_TESTS=50)
ok 57 - tap-parity: ca âm (kbin-aarch64 lệch +1) ⇒ ĐỎ (aarch64 LỆCH … kbin.TAP_TESTS=51)
ok 58 - tap-parity: ca âm (kbin-x86_64 lệch -1) ⇒ ĐỎ (x86_64 LỆCH … kbin.TAP_TESTS=49)
ok 59 - tap-parity: ca âm (thiếu tệp soak-aarch64.sh (fail-closed)) ⇒ ĐỎ (thiếu tệp …)
ok 60 - tap-parity: ca âm (TAP_TESTS không parse được (abc)) ⇒ ĐỎ (… không phải 'TAP_TESTS=<số>[ # …]')
ok 61 - tap-parity: ca âm (TAP_TESTS khai báo trùng) ⇒ ĐỎ (… có 2 dòng 'TAP_TESTS=', cần đúng 1)
ok 62 - tap-parity: ca âm (soak-x86_64 mất lời gọi tap_parity_cases) ⇒ ĐỎ (… KHÔNG gọi … (mất khoá))
```

Đây là **đúng** lớp lỗi của Peak 2 (lệch +4 ở soak-aarch64, +3 ở soak-x86_64): cùng một bản sao
cây thật, chỉ đổi **một** số, parity lập tức ĐỎ và **nêu tên đủ ba giá trị**.

## 6. Probe fail-fast (đo được: `exit 3` ở boot 0, không đốt 1.000 boot)

Bản sao tạm `soak-aarch64.probe-demo.sh` **chỉ** đổi `TAP_TESTS=51` (mô phỏng lệch +1), chạy
`--smoke 1 1` (`probe-failfast-demo.log`):

```text
# probe: '# conf image:' khớp ELF (build=e79687034717ee3f text=227784)
# soak-aarch64.sh: TỪ CHỐI ĐO — probe boot không đạt bất biến của một boot sạch:
#   # plan '1..51' xuất hiện 0 lần, cần đúng 1
#   # có 50 dòng 'ok', cần 51
#   # thiếu 'ok 51 - ...'
RC=3
```

Không có dòng `# soak: bắt đầu`/`boot 1/1` ⇒ **0 boot soak** được đếm; so với 971 s / 1.000 boot
của Peak 2. Bản sao demo đã **xoá** ngay sau phép đo (`git status` chỉ còn 2 tệp sửa + 1 tệp mới).

## 7. T0 / T1

- **T0** (trong lúc sửa): `bash -n` cả 3 tệp; `soak-{aarch64,x86_64}.sh --selftest` (62/62, 72/72);
  `tap_parity_check` trên cây thật (`RC=0`); `check-selftest-wiring.py --root .` (`PASS`);
  `--smoke 1 1` cho cả 2 arch (mục 4).
- **T1**: `./v2/bin/v2 gate --dev` — log **đã commit**
  `v2/agent/JOURNAL/kt408e-tap-parity/t1-gate-dev.log`:
  ```text
  --- selftest soak aarch64 (KT-265) ... # soak-aarch64 --selftest: 62/62 ca đúng như phải → PASS → OK
  --- selftest soak x86_64 (KT-293)  ... # soak-x86_64 --selftest: 72/72 ca đúng như phải → PASS → OK
  # step-time-total  1015981 ms  TỔNG gate --dev (T1)
  v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
  RC=0
  ```
  Trong đó `ok 52` / `ok 62` của hai bước là **ca dương tap-parity** (đọc cây thật ngay trong T1);
  14 bước T2-only in `BỎ QUA (T1 — chỉ chạy ở T2)` (không bỏ âm thầm). Hai bước selftest soak
  (id 66/71) **không** nằm trong `V2_T2_ONLY_STEPS` nên **có chạy** ở T1.
  Ghi chú trung thực: lượt T1 **đầu** bị **dừng chủ động** (chưa chạy hết) sau khi chạy tay
  `check-selftest-wiring.py --root .` phát hiện `E_ORPHAN v2/test/boot/lib/tap-parity.sh` — lỗi
  **thật** của tệp mới (hàm tên `*selftest` khiến nó bị coi là script tự kiểm độc lập mà không
  có cổng gọi). Đã sửa bằng cách đặt tên `tap_parity_cases` (ca vẫn chạy **trong** `--selftest`
  của 2 soak vốn đã có cổng), chạy lại máy kiểm ⇒ `PASS (0 mồ côi)`, rồi chạy T1 **một** lượt.
  **Không** phải retry để lấy xanh: không nới ngưỡng/ca nào, lỗi được sửa rồi mới đo.

## 8. Bất biến đã giữ / không nới

- **Không nới ngưỡng nào**: `TAP_TESTS` tăng theo **ảnh thật** (50), không phải để soak qua cổng;
  `N ≥ 3/p`, `TIMEOUT_DS=200`, `LANES ≤ 3`, `READY_DS` giữ nguyên.
- **Không sửa** `accept()`/kỳ vọng/snapshot của harness, không bỏ ca, không `#[ignore]`, không
  `--no-verify`, không `VIFAST_ROLE`, không push.
- `--smoke` vẫn **luôn** `exit 1` (không thành bằng chứng cổng); soak thật **không retry** (D2-07).
- Tệp mới chỉ **đọc** 6 tệp SSOT và dùng `mktemp -d`; không ghi vào repo, không QEMU.

## 9. Rủi ro / nợ

1. **Nợ nhỏ (đã ghi nhận)**: `soak-x86_64.sh` **không** có DNS interpose cục bộ như `KT-300` của
   aarch64 ⇒ đường `# net: dns` phụ thuộc slirp/host resolver. Lượt `--smoke 1 1` đầu tiên của
   slot này ĐỎ **1 boot** vì `# net: dns error=Timeout elapsed_ms=6000` dưới tải cao
   (`qemu_foreign=1`, loadavg 6–7 do slot song song) — **không** liên quan thay đổi của task
   (log `v2/agent/JOURNAL/kt408e-tap-parity/green-soak-x64-smoke.log`);
   retry **phân loại một lần** (D2-07) ⇒ `1/1 ok`. Đề xuất: nối DNS interpose cục bộ cho
   `soak-x86_64.sh` (cùng khuôn KT-300) ở một task sau nếu soak x86_64 thật còn chạm flake này.
2. **Phạm vi cố ý**: `run-aarch64.sh` **không** kiểm đích danh `# smp core:`/`# pan:` (chỉ đếm
   khung TAP) — `D2-263` cấm sửa `run-*.sh` ở W7b; nay soak aarch64 **siết hơn** runner, đó là
   hướng đúng (siết, không nới) nhưng lệch mức phủ giữa soak và runner cần nêu ở task kế tiếp.
3. `TAP_TESTS` vẫn là **ba chỗ khai** (dù nay bị khoá chéo). Hợp nhất thành một hằng sinh tự động
   (vd. `kbin` xuất ra `expected.txt`) là việc của `KT-420b`/sau Đại tu, ngoài phạm vi 1,5 h này.

## 10. Commit

- **`857bbd19`** — `[KT-408e] soak-{aarch64,x86_64}: khoá SSOT TAP soak==run==kbin (46/47→50) +
  probe fail-fast a64` (Lead commit; 4 tệp: 2 sửa + `lib/tap-parity.sh` + journal này).
- **Kế tiếp (chờ Lead commit)** — `[KT-408e] journal: trỏ bằng chứng tới log đã commit`:
  chép **nguyên văn** 15 log máy sinh vào `v2/agent/JOURNAL/kt408e-tap-parity/` (12 log của task +
  `peak2-soak.log` + `peak2-nightly-step-2-soak.log`) và sửa tham chiếu §1/§3/§7 sang đường dẫn
  **đã commit**; **không** đổi/xoá/nới một số liệu nào. `verify-numbers` sau khi sửa:
  `ok=5 fail=0` (như trước). Lý do: log gốc nằm dưới `target-task/` (bị gitignore) ⇒ nếu chỉ trỏ
  vào đó thì bằng chứng sẽ mất khi dọn target, trái §6.3 (bằng chứng phải tái dùng được).
- **Ghi nhận phạm vi K6 (đo được, không phải suy đoán)**: `numbers-policy.toml` khai
  `globs = ["v2/agent/reports/*.md", "v2/docs/k6-*.md"]` ⇒ `check_k6_rules` **không quét**
  `v2/agent/JOURNAL/*.md` (đo: `docs=42` = 22 reports + 21 `k6-*.md` − 1 `[[excluded]]`), nên
  `verify-numbers` xanh **không** chứng minh gì cho journal này. Thử phân loại: chép tạm journal
  vào `v2/agent/reports/` rồi chạy lại ⇒ luật 1 ĐỎ **254** `E1_NUMBER_OUTSIDE`
  (`số đo ngoài khối scope=`) — bản tạm đã xoá ngay. Nếu sau này JOURNAL được đưa vào globs, số
  đo trong journal phải chuyển vào khối `scope=` trích **nguyên văn** log (15 log đã commit sẵn
  ở `v2/agent/JOURNAL/kt408e-tap-parity/` để làm việc đó).
