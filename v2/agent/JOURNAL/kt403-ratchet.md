<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-403 — Khoá ratchet `B1.1` + `v2 mutants` + telemetry `key=value` + trace 32 B + `vifast-bench`

Nhánh `task/kt403-ratchet`, base `67b4ee2a` (KT-400 đã merge). Agent `kt403-ratchet`, worktree
`/private/tmp/vifast-wt/3`. Ngân sách 2 h (W1, D2-259).

## 1. Vì sao (nợ kiến trúc)

1. **`B1`**: 3.567 LOC dịch vụ FS/Net/Exec nằm trong kernel + cả 3 `plat-*` phụ thuộc
   `vifast-virtio`/`vifast-net`. Trước KT-403 **không có luật** nào chặn thêm dependency/driver mới
   vào kernel ⇒ mỗi task sau có thể nới `B1.2` mà cổng vẫn xanh.
2. **44 feature đột biến ad-hoc** (43 khai báo `mut-*` + `selfbreak`) rải trong 7 `Cargo.toml`,
   không manifest, không lệnh: đột biến nào còn sống thì không ai biết (vi phạm tinh thần D2-113F).
3. **Telemetry** đang so bằng regex **dán liền** ⇒ phụ thuộc thứ tự trường (KT-347b đã phải sửa tay
   khi D2-246 thêm `bad=<n>`).
4. **Trace 32 B** (D2-09/D2-40) đã có crate + golden nhưng **chưa có** công cụ giải mã dump
   `pmemsave`, và sim chưa xuất được dump nhị phân.
5. **`vifast-bench`** (§9.10) chưa tồn tại; chưa có đường ống `-icount` → history.

## 2. Đã làm

### 2.1 Luật ratchet `B1.1` — `v2/tools/check-api-boundary.py`

- Thêm phần `B1.1` (thuần đọc tệp) chạy **trước** phần compile-fail trong chế độ `--root` thường
  ⇒ `v2 check`/T1/T2 đều thi hành.
- Baseline **`v2/test/baseline/b1.1-ratchet.toml`**: 3 cạnh
  `vifast-plat-{aarch64,riscv64,x86_64} -> vifast-net` + **23 tệp driver** (`virtio_*`, `*tcp*`,
  `*_svc`) trong `v2/kernel/`.
- Quy ước **một chiều**: tệp/cạnh mới ⇒ `B1.1-DEP`/`B1.1-DRIVER` ĐỎ; xoá mục khỏi cây ⇒ chỉ in
  `stale` (ratchet siết thêm vẫn xanh); baseline thiếu/hỏng ⇒ `B1.1-BASELINE` ĐỎ **fail-closed**.
- Cờ: `--ratchet` (chỉ luật) và `--ratchet-selftest` (đỏ-trước, 5 ca trên cây mini trong `/tmp`).

Bằng chứng:
```
# ca dương (cây thật):
python3.12 v2/tools/check-api-boundary.py --root "$PWD" --ratchet
# B1.1 ratchet: dep_edges=3 (đóng băng 3, mới 0) driver_files=23 (đóng băng 23, mới 0) stale=0
# B1.1: XANH (0 vi phạm, ratchet một chiều)          rc=0
# selftest đỏ-trước: 5/5 OK
# ca âm trên BẢN SAO cây kernel thật (thêm dep vifast-fs + virtio_ssd.rs):
# B1.1-DEP vifast-plat-aarch64 -> vifast-fs
# B1.1-DRIVER v2/kernel/vifast-plat-aarch64/src/virtio_ssd.rs      rc=1
```

### 2.2 `v2/test/mutants.toml` + `v2 mutants` — `v2/tools/mutants.py`

- Manifest 24 mục: `selfbreak` + 23 đột biến phân biệt phủ **43 khai báo** `mut-*` (lint đối chiếu
  `declarations = 43`, khai báo nào chưa phủ ⇒ ĐỎ).
- Cơ chế **không sửa cây thật** (D2-113F): `kind = "feature"` chép bản sao (`v2/ spec/ docs/`) rồi
  lật cfg `not(feature="X") → any()` / `feature="X" → all()`; 0 điểm cfg ⇒ `E_STALE`.
  `kind = "script"` chạy thẳng lệnh (selfbreak). `kind = "build"` (4 đột biến do `build.rs` cấp
  `--cfg`) khai báo + `wired = false` (nối `RUSTFLAGS` khi Lead gỡ feature).
- Phân loại trung thực: `KILLED` / `SURVIVED` (đột biến sống) / `E_WRONG-REASON` (đỏ sai lý do) /
  `E_STALE` / `E_UNWIRED`. `--all` còn mục chưa nối ⇒ **exit 3** (không giả xanh).
- Lệnh: `./v2/bin/v2 mutants [--list|--lint|--all|--selftest|<id>]`.
- Bằng chứng:
```
./v2/bin/v2 mutants --lint
# mutants lint: mutants=24 feature=23 declarations=43 covered=43 suites=5 problems=0   rc=0
python3.12 v2/tools/mutants.py --root "$PWD" --selftest
== SELFTEST mutants: OK — 7/7 ca (đỏ-trước cho chính công cụ) ==                     rc=0
python3.12 v2/tools/mutants.py --root "$PWD" selfbreak
ok - selfbreak: KILLED (47.0 s) ... marker=ok                                          rc=0
```

### 2.3 Telemetry `key=value` không nhạy thứ tự — `v2/tools/telemetry.py`

- Chuẩn hoá: `[# ]<tiền tố>: k=v …`; khoá `[a-z][a-z0-9_]*` (cho phép `const:`), giá trị không
  rỗng/không khoảng trắng; **trùng khoá ⇒ `E_DUP_KEY`**; token hỏng ⇒ `E_TOKEN`; thiếu tiền tố ⇒
  `E_NO_PREFIX`.
- `parse_line` → bản đồ; `fields_equal`/`has_fields` so **theo tên**; `format_line` sinh dòng tất
  định (sắp khoá). `--selftest` 14 ca gồm **24/24 hoán vị** của 4 trường cho cùng bản đồ.
- Người dùng thật: `vifast-bench` đọc evidence bằng `telemetry.py` (không regex dán liền).
```
python3.12 v2/tools/telemetry.py --selftest
== SELFTEST telemetry: OK — 14/14 =="                                                 rc=0
```

### 2.4 Trace nhị phân 32 B — `v2/bin/vifast-trace` + `v2/tools/vifast-trace.py` + sim

- `vifast-trace` giải mã: dump **có header** `VFT1` (`magic|version|count|FNV-1a64`) **và** dump
  thô/golden/pmsave; kiểm hash/count/version/kind, lỗi có tên (`E_HASH`/`E_COUNT`/`E_VERSION`/
  `E_TRUNC`/`E_KIND`); in dòng thời gian hoặc `--json`, lọc `--kind`/`--limit`.
- Sim `vifast-sim --trace-bin <tệp>` ghi đúng định dạng (header+records) từ `Event` — nối đầu
  cuối sim → dump; QEMU `pmemsave` cho cùng bản ghi (D2-09/D2-40).
- `--selftest` 9 ca (header/thô/round-trip/4 lỗi).
- **Bằng chứng đầu-cuối** (byte-level, không tự quy chiếu):
```
cargo run -q -p vifast-sim -- --seed 7 --steps 200 --tasks 4 --cap 4096 --trace-bin /tmp/kt403-seed7.vft
# trace-bin=/tmp/kt403-seed7.vft records=200 header=VFT1v1
v2/bin/vifast-trace /tmp/kt403-seed7.vft --limit 1
t=0 hart=0 kind=run tcb=3 a=0 b=0
# vifast-trace: header=VFT1v1 count=200 records=200 in=6420 shown=1
# so byte với golden đã commit: 200/200 bản ghi, body 6400 B == v2/test/trace/golden-sim-seed7.bin
```

### 2.5 `vifast-bench` — smoke dưới `-icount`

- `v2/bin/vifast-bench --smoke`: build ảnh riscv64 → boot **1 lần** qua `qemu_guard.sh` với
  `-icount shift=auto,align=off,sleep=off` → đòi `ok 1 - k3 boot`/`# k3 halt`/`# build:` → đọc
  evidence bằng `telemetry.py` → ghi `v2/test/perf/history.csv` (cột `measure` = `smoke`, **không**
  phải số đo hiệu năng). `--selftest` 5 ca (không QEMU).

## 3. Tệp thêm/sửa

| Tệp | Loại |
|---|---|
| `v2/tools/check-api-boundary.py` | sửa: luật `B1.1` + `--ratchet(-selftest)` |
| `v2/test/baseline/b1.1-ratchet.toml` | mới: 3 cạnh dep + 23 tệp driver đóng băng |
| `v2/tools/mutants.py` | mới: runner + lint + selftest |
| `v2/test/mutants.toml` | mới: 24 mục (selfbreak + 23 đột biến / 43 khai báo) |
| `v2/tools/telemetry.py` | mới: quy chuẩn `key=value` |
| `v2/tools/vifast-trace.py`, `v2/bin/vifast-trace` | mới: giải mã dump 32 B |
| `v2/bin/vifast-bench` | mới: smoke `-icount` + history |
| `v2/test/perf/history.csv` (+`.license`) | mới: header lịch sử đo |
| `v2/sim/vifast-sim/src/main.rs`, `v2/sim/vifast-sim/Cargo.toml` | sửa: `--trace-bin` + dep traceformat |
| `v2/bin/v2`, `v2/bin/lib/help.sh` | sửa: dispatch `mutants`, usage |
| `v2/bin/lib/selftests.sh` | sửa: 5 `_t0_tool` cho công cụ KT-403 (có cổng gọi, KT-134) |
| `v2/tools/check-wake-depinfo.py` | sửa 13 dòng: ca (51) selftest **kín môi trường** (`CARGO_TARGET_DIR`), hành vi thật không đổi |

**Không chạm**: `spec/`, `docs/v1/`, `docs/v2/`, QD, `v2/kernel/**`, `v2/user/**`.

## 4. Kiểm + kết quả (T0/T1)

| Lệnh | Kết quả |
|---|---|
| `python3.12 v2/tools/check-api-boundary.py --root "$PWD" --ratchet` | **0** (3 cạnh/23 tệp, 0 vi phạm) |
| `python3.12 v2/tools/check-api-boundary.py --ratchet-selftest` | **0** (5/5 ca) |
| `./v2/bin/v2 mutants --lint` | **0** (24 mục, 43/43 khai báo) |
| `python3.12 v2/tools/mutants.py --selftest` | **0** (7/7 ca) |
| `python3.12 v2/tools/mutants.py selfbreak` | **0** (`KILLED`, 47 s) |
| `python3.12 v2/tools/telemetry.py --selftest` | **0** (14/14) |
| `python3.12 v2/tools/vifast-trace.py --selftest` | **0** (9/9) |
| `bash v2/bin/vifast-bench --selftest` | **0** (5/5) |
| `bash v2/bin/vifast-bench --smoke --no-build` | **0** (`qemu_rc=0 ok=1`, `-icount`, `build_id=eb80ff08b21b4204`) |
| `cargo fmt --all` + `cargo clippy --workspace --all-targets -D warnings` | **0** |
| `./v2/bin/v2 t0` | **0** (T0: fmt+clippy+test crate bị chạm) |
| `./v2/bin/v2 t0 --selftest` | **0** (14/14 công cụ T0, gồm 5 công cụ mới) |
| `./v2/bin/v2 gate --dev` (T1) | **0** — `v2 gate --dev: PASS`, 659 s |
| `./v2/bin/v2 check` (sau khi vá telemetry/bench, cây cuối) | **0** — 73,7 s |

Tái dùng bằng chứng (AGENTS §6.3): T1 chạy trên cây trước 3 thay đổi **chỉ ảnh hưởng T0/không thuộc
bước cổng** (bộ lọc tiền tố `telemetry.py`, chuyển `,`→`+` trong `vifast-bench`, JOURNAL). Sau các
sửa đó `v2 check` (gồm fmt/clippy/luật/checker/wiring) chạy lại **xanh** trên cây cuối; không bước
T1 nào bị ảnh hưởng. Không chạy lại T1 chỉ để tăng số lần PASS (`AGENTS.md` §6.3).

**Ghi chú môi trường (quan trọng cho Lead)**: T1 phải chạy **không** export `CARGO_TARGET_DIR`.
Với `CARGO_TARGET_DIR` (cách các slot chạy để tiết kiệm đĩa), hai **lỗi có sẵn từ trước** xuất hiện:
(i) `run-aarch64.sh`/`run-x86_64.sh` tìm ảnh ở `v2/kernel/target/…` trong khi cargo đặt ảnh ở
`$CARGO_TARGET_DIR` ⇒ "không thấy ảnh" (ĐỎ giả); (ii) `check-wake-depinfo.py --selftest` ca (51)
đọc `CARGO_TARGET_DIR` thay cho target dir giả — **đã vá kín môi trường** trong tệp này
(13 dòng, chỉ đổi lượt gọi selftest; hành vi checker thật không đổi). Mục (i) thuộc phạm vi
`KT-407` (đồng bộ runner), chưa sửa ở đây.

## 5. Còn lại / bàn giao Lead lúc merge (D2-259)

1. Gỡ 43 khai báo `mut-*` khỏi 7 `Cargo.toml` + nối `v2 mutants --all` vào `v2 nightly --all`
   (hôm nay `wired = true` mới có `selfbreak`; 23 mục còn lại `wired = false`, `--all` exit 3).
2. Với 4 đột biến `kind = "build"` (`mut-info-x1`, `mut-ktest-driver`, `mut-net-one-caller`,
   `mut-no-x0-out`): nối bằng `RUSTFLAGS`/`--cfg` hoặc patch `Cargo.toml` của bản sao.
3. Ring **luôn bật trong kernel** + `pmemsave` (D2-09) chưa nối: W1 mới có đường ống sim +
   bộ giải mã. Nối ring vào `.corespace`/vùng hart khi `KT-405`/`KT-408` có `PerHartState`
   (không được tăng `.data+.bss` — đã kiểm: chưa chạm ảnh).
4. `vifast-bench`: W1 mới smoke riscv64; số đo thật (IPC round-trip, chờ khoá, p50/p99) ở `KT-415`.

## 6. KT-403 r2 — đóng nợ mục 5.1 + 5.2 (nhánh `task/kt403-r2`, base `79af0ab9`)

Worker `kt403-mut`, worktree `/private/tmp/vifast-wt/4`. **Không** đổi QUEUE (Lead trình Agent 0).

### 6.1 Gỡ 43 khai báo `mut-*` (7 `Cargo.toml`)

`vifast-core` 2, `vifast-kbin-aarch64` 8, `vifast-kbin-riscv64` 7, `vifast-kbin-x86_64` 12,
`vifast-plat-aarch64` 3, `vifast-plat-riscv64` 3, `vifast-plat-x86_64` 8 — **tổng 43**. Mỗi tệp
nhận thêm ghi chú "manifest là nguồn duy nhất" + `[lints.rust] unexpected_cfgs = "allow"`:
sau khi gỡ, `#[cfg(feature = "mut-*")]` còn trong mã (móc đột biến, **không** sửa kernel source)
bị rustc coi là "unexpected cfg" ⇒ `-D warnings` thành lỗi. Đã đo: `allow` thắng `-D warnings`
(ca mini) và **không** nới ngưỡng/tắt test nào.

### 6.2 `v2/tools/mutants.py` + `v2/test/mutants.toml`

- `kind = "build"` nay chạy thật: áp `patches` (`file`/`find`/`replace`) lên **bản sao**; `find`
  không khớp ⇒ `E_STALE`. 4 mục: 3 vá `build.rs` để bật `--cfg` (`mut_info_x1`, `mut_no_x0_out`,
  `mut_net_one_caller`), 1 vá `Cargo.toml` của bản sao dựng lại nhãn cũ `net = ["ktest"]`
  (`mut-ktest-driver`, suite `product-graph`, `expect = "pass"`).
- `marker` riêng từng mục (thay marker chung của suite) ⇒ "ĐỎ đúng lý do" không còn nhận bất kỳ
  `not ok` nào (đỏ hạ tầng không bị tính KILLED).
- `--lint`: `declarations = 43` = số khai báo **đã thay thế** (đóng băng `67b4ee2a`);
  còn khai báo `mut-*` trong cây ⇒ `E_LEFTOVER`; `covered != 43` ⇒ đỏ.
- Sửa lỗi manifest W1: `mut-drop-nb-no-waiter`/`mut-state-before-cap` ghi suite `l1-host` nhưng
  sống trong **unit test** `v2/kernel/vifast-core` (`nb_queue_tests.rs`) — `l1-tier --tier smoke`
  chạy `vifast-sim --test l1_k1c`, **không** thấy chúng (đo được: `SURVIVED`, 18,2 s). Nay suite
  `core-host`: `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib nb_queue`.
- Nợ KT-407 với **bản sao**: runner boot tìm ảnh ở `<repo>/v2/kernel/target`, ảnh sản phẩm tìm
  `v2/target/release/vifast-fsutil` (đường cứng). `_copy_tree` trỏ hai đường đó về
  `$CARGO_TARGET_DIR` khi biến này được đặt — chỉ trên **bản sao**, cây thật không đổi.
- Khi chạy trong `v2 nightly` (cha đã giữ khoá máy đo + uỷ quyền), `V2_MEASURE_LOCK=off` cho tiến
  trình con: nightly chạy tuần tự, các lần chạy đột biến **không** phải phép đo.

### 6.3 `v2 nightly --all`

Bước `mutation` (giữ 7 bước, không đổi số bước) nay chạy `python3.12 v2/tools/mutants.py --root .
--all`; nhánh `mutants)` trong `steps_soak.sh` dùng `find_python` fail-closed. `--dry-run` in
đúng lệnh; `v2 nightly --selftest` **30/30 PASS**.

### 6.4 Bằng chứng

```
./v2/bin/v2 mutants --lint
# mutants lint: mutants=24 feature=23 declarations=43 covered=43 leftover=0 suites=8 problems=0   rc=0
python3.12 v2/tools/mutants.py --root "$PWD" --selftest
== SELFTEST mutants: OK — 9/9 ca ==                                                              rc=0
./v2/bin/v2 mutants --all --dry-run   # 24 đột biến, 0 chưa nối                                  rc=0

# đột biến FEATURE (suite core-host, unit test thật):
./v2/bin/v2 mutants mut-state-before-cap   # KILLED (5,6 s) rc=101 marker=ok
./v2/bin/v2 mutants mut-drop-nb-no-waiter  # KILLED (6,1 s) rc=101 marker=ok
# đột biến BUILD (nhãn đồ thị):
./v2/bin/v2 mutants mut-ktest-driver       # KILLED (1,2 s) rc=0 marker=ok — cargo tree in
                                           # `vifast-plat-riscv64 feature "ktest"`
# đột biến BUILD (`--cfg` từ build.rs, chạy full runner aarch64 + QEMU):
./v2/bin/v2 mutants mut-info-x1            # KILLED (85,4 s) rc=1 marker=ok
                                           # `not ok 8 - boot 1: có 2 dòng 'not ok'` (TAP 43/44)
```

Cổng trên cây cuối: `cargo fmt --all --check` **0**; `cargo clippy --workspace --all-targets --
-D warnings` (workspace `v2/Cargo.toml`) **0**; `./v2/bin/v2 check` **0** (117 s);
`./v2/bin/v2 t0 --selftest` **11/11** (tooling 14/14); `v2 nightly --selftest` **30/30**.

### 6.5 Nợ / cảnh báo còn lại

1. **Chưa chạy `v2 mutants --all` đầy đủ** (23 đột biến × runner boot/QEMU ≈ 2 h; ước lượng bước
   nightly đặt 5400 s). Đã chứng minh 2 feature + 2 build end-to-end; 19 mục còn lại `wired=true`
   và lint xác nhận, nhưng chưa có log KILLED/SURVIVED từng mục.
2. `v2 t0` (không `--selftest`) **đỏ vì nợ có sẵn**: clippy kernel crates có **17 lỗi** (vd
   `clippy::needless_range_loop` trong `vifast-core/src/check.rs`). Đã đo **cùng 17 lỗi** trên cây
   base `79af0ab9` (`git archive` → `/tmp`) trong khi T0 chỉ chọn kernel crate vì diff chạm
   `Cargo.toml` của chúng ⇒ **không** do lần gỡ feature; sửa được thì phải sửa kernel source
   (ngoài phạm vi). Thêm một **flake** `resume_tests::resume_inactive_chuyen_running_va_giao_entry`
   (`left: 3, right: 2`) — chạy lại 2 lần 104/104 PASS trên **cùng** cây ⇒ `T-INFRA` (D2-07),
   không sửa gì.
3. `v2 gate --dev` (T1): **727,4 s**, `registered=75 ran=61 skipped=14 failed=1`; đỏ **duy nhất**
   `selftest pre-measure (KT-95)` = exit **7 "cây nguồn bẩn"** — worker bị cấm commit nên cây còn
   diff; đây là **tiền đề sạch của T1**, không phải lỗi mã. Lead commit rồi chạy lại T1 là được.
4. `mut-net-one-caller` (suite `product-riscv64`) cần `vifast-fsutil` + đĩa/mạng QEMU; bản sao đã
   có symlink target dir (KT-407) nhưng **chưa chạy** mục này, nên chưa có log.
