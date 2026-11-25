# KT-413b — KT-413 phần 2: hợp nhất bộ ca SMP 3 arch + PCID/ASID + đo kế hoạch gỡ `sched.rs`

- **Task**: KT-413b (P0, Chặng 2 / W5) — phụ thuộc KT-413 (DONE `e6e9ac76`).
- **Nhánh**: `task/kt413b-sched-pcid` · **Worktree**: `/private/tmp/vifast-wt/1`
- **Base**: `main` @ `88d7473b` (đã có KT-411b/412/413/415/418).
- **Ngân sách**: **2 h** (lead rút từ 3 h). Bắt đầu 2026-11-26 06:20 JST (2026-11-25 21:20 UTC,
  Thấp điểm chính).
- **Căn cứ**: `QUEUE.md` dòng 466, `KE-HOACH-KIEN-TRUC.md` dòng KT-413b, `JOURNAL/kt413-smp-x64.md`
  §5 (nợ của phần 1), `KT-340c` §6.2 (đổi kỳ vọng 32 ca âm `# sched:` cần D2 duyệt).

## 0. Kế hoạch ngắn

1. **Đo hiện trạng**: `smp_suite.rs` 366 dòng (rv/a64, 3 TAP 71..73) + `smp_suite_x64.rs` 379 dòng
   (x64, 2 TAP 48/49) = **hai** bộ song song; `lib.rs` có **hai** trait `SmpBridge` khác nhau
   (`vifast_ktest::SmpBridge` cho x64 + `smp_suite::SmpBridge` cho rv/a64).
2. **Đỏ-trước**: gộp test host về **một** tệp `tests/smp_suite.rs` (giữ **đủ 9 ca** của hai tệp),
   gọi `smp_suite::run_x64` (chưa có) ⇒ **không biên dịch được** (đỏ đúng lý do "API chưa có").
3. **Ưu tiên 1 — hợp nhất**: một mô-đun `smp_suite` cho **3 arch**, **một** trait `SmpBridge`
   (`with_machine`/`boot_harts`/`untyped`/`set_harts`), **hai** cửa vào:
   - `run(plat, bridge, first)` — profile rv/a64, **3 TAP**, chữ ký **giữ nguyên** (0 chạm
     `rv`/`a64`: `ktest_core.rs` và `smpcore.rs` không phải sửa một dòng);
   - `run_x64(plat, bridge, harts)` — profile x64, **2 TAP 48/49**, nhãn + dòng `# core smp:`
     **giữ nguyên**.
   Vì sao **hai** cửa vào chứ không một `run(cfg)`: hợp đồng TAP của **ba runner** đã đóng băng
   (`expected.txt` `ktest 74 74`; `run-x86_64.sh` `EXPECT_TESTS=50` + `ok 48/49`), mà hai bộ ca
   **khác nhau về ngữ nghĩa** (rv/a64: bootinfo/preemptive/published; x64: four-harts/sleep-migrate)
   ⇒ gộp tập ca là **đổi số TAP** = đổi kỳ vọng harness (AGENTS §4.2). Hợp nhất **cấu trúc**
   (một tệp, một trait, dùng chung `Tap`/`SmpReport`/helper), **không** đổi hành vi.
4. **Ưu tiên 2 — PCID/ASID**: cấp **PCID riêng cho từng `VSpace`** (bộ cấp phát 12 bit, host-test)
   + `VSpace::activate` ghi `CR3[11:0]` khi `CR4.PCIDE=1` + `enable_pcid()` đo `CPUID` rồi bật.
   **Mặc định TẮT** (`PCID_ENABLED=false`) ⇒ runner vẫn thấy `cr4_pcid=0`; ghi rõ cách bật + câu
   hỏi cho Lead về đổi kỳ vọng `# ipi: pcid`.
5. **Ưu tiên 3 — gỡ `sched.rs`**: **KHÔNG làm** (đổi 32 ca âm `# sched:` = đổi kỳ vọng harness,
   AGENTS §4.2). Chỉ **đo** và ghi kế hoạch + danh sách kỳ vọng phải đổi.
6. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `rv`/`a64`; 0 sửa `spec/`; không nới ngưỡng; không `#[ignore]`/
`#[should_panic]`; không sửa `expected.txt`/`run-x86_64.sh`/`accept()`; không `git add/commit`;
chỉ T0 + T1; tối đa 1 QEMU của mình.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ wc -l v2/kernel/vifast-ktest/src/smp_suite.rs v2/kernel/vifast-ktest/src/smp_suite_x64.rs
     366 v2/kernel/vifast-ktest/src/smp_suite.rs
     379 v2/kernel/vifast-ktest/src/smp_suite_x64.rs
$ grep -c 'sched:' v2/test/boot/run-x86_64.sh
69
$ wc -l v2/kernel/vifast-kbin-x86_64/src/sched.rs v2/kernel/vifast-plat-x86_64/src/sched.rs
    1376 v2/kernel/vifast-kbin-x86_64/src/sched.rs
     786 v2/kernel/vifast-plat-x86_64/src/sched.rs
    2162 total
$ grep -n 'EXPECT_TESTS=' v2/test/boot/run-x86_64.sh
237:EXPECT_TESTS="${EXPECT_TESTS:-50}"
$ grep -n 'ktest 74' v2/test/boot/expected.txt
ktest 74 74
```

Hai trait `SmpBridge` **khác nhau** đang cùng tồn tại:

| Trait | Method | Ai cài |
|---|---|---|
| `vifast_ktest::SmpBridge` (lib.rs) | `with_machine`, `link` | blanket impl cho `CoreBridge` (x64 dùng) |
| `vifast_ktest::smp_suite::SmpBridge` | `with_machine`, `boot_harts`, `untyped`, `set_harts` | `Riscv64Virt`, `SmpBridgeImpl` (a64) |

## 2. Đỏ-trước (bằng chứng, D2-05)

### 2.1 Hợp nhất bộ ca SMP — `run_x64` chưa tồn tại

Gộp **đủ 9 ca** của hai tệp test cũ về `tests/smp_suite.rs` (không bỏ ca nào — AGENTS §4.3) và
thêm ca 10 (hợp nhất), **trước** khi hiện thực:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest --test smp_suite
error[E0425]: cannot find function `run_x64` in module `smp_suite`
   --> vifast-ktest/tests/smp_suite.rs:368:30
    |
368 |     let outcome = smp_suite::run_x64(&plat, &bridge, SMP_HARTS);
    |                              ^^^^^^^ not found in `smp_suite`
...
error: could not compile `vifast-ktest` (test "smp_suite") due to 5 previous errors
```

Đỏ **đúng lý do** ("API chưa có"), không phải đỏ vì assert. Sau khi hiện thực: **10/10** ca xanh.

## 3. Việc đã làm

### 3.1 Ưu tiên 1 — hợp nhất `smp_suite_x64` + `smp_suite` thành **một** bộ 3 arch

**Trước** (hai bộ song song, hai trait khác nhau):

| Tệp | Dòng | Trait | TAP | Nhãn |
|---|---:|---|---|---|
| `vifast-ktest/src/smp_suite.rs` | 366 | `smp_suite::SmpBridge` (`with_machine`/`boot_harts`/`untyped`/`set_harts`) | 3 (`first..first+2`) | `smp harts bootinfo`, `smp 4 hart preemptive`, `smp published rule` |
| `vifast-ktest/src/smp_suite_x64.rs` | 379 | `vifast_ktest::SmpBridge` (`with_machine`/`link`) + blanket impl cho `CoreBridge` | 2 (**cố định** 48/49) | `core smp 4 hart`, `core smp sleep migrate` |

**Sau** (một mô-đun, một trait, hai **profile**):

- `v2/kernel/vifast-ktest/src/smp_suite.rs` (**một** tệp, 1.0xx dòng): **một** trait `SmpBridge`
  (`with_machine` + `boot_harts` + `untyped` + `set_harts` mặc định) — **đúng** ba method mà
  `Riscv64Virt`/`SmpBridgeImpl` đã cài (0 chạm `rv`/`a64`), và `ImageBridge` của x64 cài lại
  `link()` → `boot_harts()`/`untyped()` (hẹp hơn, đúng thứ ảnh **có**).
- **Hai cửa vào** cùng mô-đun:
  - `run(plat, bridge, first)` — profile rv/a64, **3 TAP**, chữ ký **giữ nguyên** ⇒
    `vifast-kbin-riscv64/src/ktest_core.rs:448` và `vifast-kbin-aarch64/src/smpcore.rs:105`
    **không sửa một dòng**;
  - `run_x64(plat, bridge, harts)` — profile x64, **2 TAP 48/49**, nhãn + dòng `# core smp:`
    **giữ nguyên**.
- Xoá `smp_suite_x64.rs` + `tests/smp_suite_x64.rs`; xoá trait `vifast_ktest::SmpBridge` +
  blanket impl trong `lib.rs` (chỉ x64 dùng, đã thay bằng trait chung).
- `coresmp.rs` (x64): `smp_suite_x64::run` → `smp_suite::run_x64`; `SMP_HARTS` → `X64_HARTS`;
  bỏ `link()`/`LinkReport` + hai static `UNTYPED_BASE`/`UNTYPED_BYTES` (chỉ phục vụ `link()`).

**Vì sao hai cửa vào chứ không một `run(cfg)`** — trung thực, không giấu:

1. Hợp đồng TAP của **ba runner** đã đóng băng: `expected.txt` `ktest 74 74` (rv/a64 3 TAP
   71..73) và `run-x86_64.sh` `EXPECT_TESTS=50` + `ok 48`/`ok 49` (x64 2 TAP). Gộp **tập ca**
   (union 5 ca) là **đổi số TAP** của ít nhất một arch ⇒ đổi kỳ vọng harness ⇒ **cần D2**
   (AGENTS §4.2) — đúng thứ task này **cấm** làm.
2. Hai tập ca **khác nhau về ngữ nghĩa**, không phải bản sao: rv/a64 kiểm `set_harts` từ
   `BootInfo` + 8 TCB preemptive + luật `published`; x64 kiểm 5 TCB + priority tĩnh +
   `notification_wait`/`signal` + migration. Gộp cơ học sẽ **mất** phép kiểm (rv/a64 không có
   sleep/migrate; x64 không có `published` rule) hoặc **thêm** TAP.
3. Vì vậy hợp nhất ở **cấu trúc** (một tệp, một trait, dùng chung `Tap`/`SmpReport`/helper/
   `Outcome`), **không** đổi hành vi: **0** TAP đổi số/nhãn, **0** dòng bằng chứng đổi, **0**
   ngưỡng đổi. Việc gộp **tập ca** (nếu Lead muốn) là một D2 riêng — xem §6 câu hỏi 1.

**Bằng chứng boot thật** (QEMU 11.1.3, `-M q35 -cpu qemu64 -smp 4`, ảnh `--features ktest`):

```text
# core smp: online mask=0xf harts=4 expect=4
# core smp: init addr=0x240000 size=184712 pin=196608 harts=4 untyped=1 base=0x213000 bytes=16384 healthy=1
# core smp: harts=4 picks=[1, 1, 1, 1] first=[t4, t1, t2, t3] distinct=4 double=0 prio_ok=1 sleep_ok=1 migrate=0 invariants_ok=1
ok 48 - core smp 4 hart
ok 49 - core smp sleep migrate
# core smp: taps=2 fails=0 ok=1
```

Dòng `# core smp:` **giống hệt** bằng chứng KT-413 (`JOURNAL/kt413-smp-x64.md` §3.2) — hợp nhất
không đổi một giá trị đo. (Boot dừng ở `not ok 27 - blk: sector0` vì đĩa tạm tôi tạo là **zero**,
không có magic sector — runner thật ghi magic trước khi boot; không liên quan thay đổi này.)

### 3.2 Ưu tiên 2 — PCID/ASID: cấp PCID riêng/VSpace + bật khi CPU hỗ trợ, **mặc định TẮT**

- `v2/kernel/vifast-plat-x86_64/src/pcid.rs` (**mới**, ~300 dòng):
  - `PcidAlloc` — bộ cấp phát **thuần** (bitmap 4096 bit, `PCID_KERNEL = 0` **không** cấp cho
    VSpace), host-test được;
  - bitmap **toàn ảnh** `PCID_BITS: [AtomicU64; 64]` + `alloc_pcid()`/`free_pcid()`/
    `used_pcids()`/`exhausted()` (CAS, không khoá);
  - `PCID_ENABLED: AtomicBool` (**mặc định `false`**), `enabled()`, `enable()` — đo
    `CPUID.01H:ECX[17]` **trước**, chỉ ghi `CR4.PCIDE` khi CPU có (bit không hỗ trợ ⇒ `#GP`);
  - `cr3_value(root, pcid, enabled)` — **thuần**: `root | pcid` chỉ khi bật, ngược lại **đúng
    bằng** `root` (giữ `CR3[11:0] = 0` khi `PCIDE = 0`).
- `v2/kernel/vifast-plat-x86_64/src/asm.rs`: thêm `write_cr4` (asm + stub host) — `unsafe` vẫn
  **chỉ** trong `asm.rs` (D2-17).
- `v2/kernel/vifast-plat-x86_64/src/vspace.rs`: `VSpace` mang **`pcid` riêng** (cấp lúc `create`);
  `activate()` ghi `cr3_value()`; `restore()` ghi `cr3_value(boot_root, PCID_KERNEL, enabled)`;
  accessor `pcid()`/`cr3_value()`.
- `v2/kernel/vifast-plat-x86_64/tests/pcid.rs` (**mới**, 6 ca host): PCID 0 không cấp · cấp N số
  đôi một khác nhau + trả rồi cấp lại đúng số · hết PCID ⇒ `None` (fail-closed, không panic,
  không trùng) · `CR3` chỉ mang PCID khi bật · bitmap toàn ảnh round-trip · **mặc định TẮT**.

**Vì sao mặc định tắt** (không phải làm dở): `run-x86_64.sh` (dòng 1163-1170) **và**
`soak-x86_64.sh` (dòng 603, khớp **cả dòng** tới `$`) đòi
`# ipi: pcid … cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg` — nhánh đóng lỗi
v1 **N5**. Bật `CR4.PCIDE` là **đổi kỳ vọng harness** ⇒ cần D2. Ngoài ra bật PCID mà **không**
chuyển `flush_va` sang `INVPCID` **theo PCID** thì `INVLPG` chỉ xả entry của PCID **hiện hành**
trên hart gọi ⇒ hart khác vẫn đọc PA cũ (đúng lỗi N5) — nợ riêng, ghi ở đầu `pcid.rs`.

**Cách bật** (khi có D2): gọi `pcid::enable()` một lần trên BSP trước `VSpace::create` đầu tiên;
`activate` tự ghép PCID; đổi `flush_va` sang `INVPCID`; đổi kỳ vọng `# ipi: pcid` + `soak`.

**Bằng chứng**: `cargo test -p vifast-plat-x86_64 --test pcid` **6/6 xanh**; toàn bộ host test
`vifast-plat-x86_64` xanh (24+14+3+13+14+27+18+9+5+9+6+7+6+8+10+10+21+3+7+9 ca); ảnh
`product-shell` **có** symbol `pcid::PCID_BITS` (mã **thật** trong ảnh sản phẩm, không bị DCE);
`.data+.bss` product-shell `8368 + 43672 = 52040 ≤ 262144`; ảnh tối giản `43424 ≤ 65536`
(không link `VSpace` — pre-existing).

### 3.3 Ưu tiên 3 — gỡ 2.162 dòng `sched.rs`: **KHÔNG làm**, chỉ đo + kế hoạch

Xem §5.1 (đo chính xác + danh sách kỳ vọng phải đổi + đề xuất task riêng có D2).

## 4. Cổng đã chạy (T0)

| Cổng | Kết quả |
|---|---|
| `cargo test -p vifast-ktest` (host) | **10/10** ca xanh (5 ca cũ của rv/a64 + 4 ca cũ của x64 + 1 ca **hợp nhất**) |
| `cargo test -p vifast-plat-x86_64` (host) | xanh **toàn bộ** (gồm **6/6** ca `pcid` mới) |
| `./v2/bin/v2 t0 vifast-ktest` | **PASS** (fmt + clippy + test) |
| `cargo clippy -p vifast-ktest --all-targets -- -D warnings` | sạch |
| `cargo clippy -p vifast-plat-x86_64 --all-targets -- -D warnings` | sạch |
| `cargo build -p vifast-kbin-x86_64 --target x86_64-unknown-none --release --features ktest` | xanh |
| `cargo build -p vifast-kbin-aarch64 --target aarch64-unknown-none --release --features ktest` | xanh (**0 chạm** a64) |
| `cargo build -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --release --features ktest` | xanh (**0 chạm** rv) |
| `python3 v2/tools/check-wake-paths.py --combined` | `strict=1 errors=0 warnings=0` |
| `bash v2/test/boot/budget-x86_64.sh` (product / product-shell / ktest) | `43424 ≤ 65536` · `52040 ≤ 262144` · `102472 ≤ 262144` |
| Boot thật QEMU `-smp 4` (ảnh ktest) | `ok 48`/`ok 49` + dòng `# core smp:` **giống hệt** KT-413 |

**Ghi chú T0**: `./v2/bin/v2 t0 vifast-kbin-x86_64` **đỏ ở bước `cargo test`** vì crate là
`no_std` bin có `#[panic_handler]` — `cargo test` trên host báo
`error[E0152]: found duplicate lang item panic_impl` (std của harness test + panic handler của
ảnh). Đây là **giới hạn có sẵn** của crate (không phải hồi quy của task này: không thêm
`panic_handler` nào); T0 của crate ấy là **clippy + build 3 feature** ở bảng trên, và T1/T2 mới
là cổng thật cho ảnh.

## 5. Việc **chưa** làm (và vì sao)

### 5.1 Ưu tiên 3 — gỡ 2.162 dòng harness `sched.rs` (`KT-340e`): **KHÔNG** làm

**Đo chính xác** (không suy đoán):

```text
$ wc -l v2/kernel/vifast-kbin-x86_64/src/sched.rs v2/kernel/vifast-plat-x86_64/src/sched.rs
    1376 v2/kernel/vifast-kbin-x86_64/src/sched.rs
     786 v2/kernel/vifast-plat-x86_64/src/sched.rs
    2162 total
$ grep -c 'sched:' v2/test/boot/run-x86_64.sh   # 69 dòng (kể cả chú thích)
$ grep -c 'sched:' v2/test/boot/soak-x86_64.sh  # 64 dòng
$ grep -c '^# sched: ' <log boot thật>          # 16 dòng bằng chứng
```

**Nội dung 2.162 dòng** (không phải "mã chết" — nó đang **đo** thật):

- `plat-x86_64/src/sched.rs` (786): **thuần logic** — `TASK_SEQUENCE` (9 lượt chuyển), layout
  frame ngữ cảnh (`init_frame_words`/`switch_frame_ok`), `next_task`, hằng `MH_*` (hàng đợi đa
  hart), `task_name`, `mh_all_picked`/`mh_end`/`mh_hold`/`mh_harts_picked`.
- `kbin-x86_64/src/sched.rs` (1376): harness **thật** — 4 stack tác vụ **16 KiB** (64 KiB `.bss`),
  hàng đợi toàn cục + **ownership bằng CAS** (`MH_OWNER`), priority/sleep tự tính, bộ đếm
  preempt/umode, `prio_report` (TAP 29/30), `run` (TAP 22..25).

**Vì sao KHÔNG gỡ trong khung này**: 6 TAP (22..25, 29, 30) và **16 dòng** `# sched:` bám
**nguyên văn** mô hình tự viết (`switch a→b`, `ticks=10 tasks=4`, `runs a=… b=…`,
`hart=N picked=<a|b|c|u>`, `multi-hart … first=[a, u, c, b]`, `prio first=b … picks=[2, 2, 2, 2]`,
`sleep wakes=4 …`). Thay "ai chạy tiếp" bằng `Machine::dispatch(hart)` **đổi giá trị** các dòng
ấy (scheduler của core chọn theo **priority thật** trên TCB thật, không theo `TASK_SEQUENCE`) ⇒
phải sửa **kỳ vọng** runner — đúng việc `KT-340c` §6.2 ghi là **cần D2 duyệt**, và đúng luật
tuyệt đối của task này ("KHÔNG sửa kỳ vọng harness").

**Danh sách kỳ vọng phải đổi** (để Lead xin D2 — đo trên `run-x86_64.sh`):

| # | Vị trí | Kỳ vọng hiện tại | Phải đổi thành |
|---|---|---|---|
| 1 | `check_sched()` dòng 962 | `# sched: switch a→b n=<k>` | dòng bằng chứng của `dispatch` (vd `# sched: tcb=<ObjId> hart=<n> prio=<p>`) |
| 2 | dòng 967 | `# sched: switch b→a n=<k>` | như trên (một dòng/lượt chuyển) |
| 3 | dòng 972 | `# sched: ticks=<t> tasks=4` | `# sched: ticks=<t> tcbs=<n>` (số TCB thật) |
| 4 | dòng 985 | `# sched: umode task ran n=… ring=3 preempted=… resumed=…` | `# sched: umode tcb=<ObjId> cs=0x1b preempted=…` |
| 5 | dòng 992 | `# sched: umode counts … cs=0x1b kernel_stack=0x…` | giữ `cs=0x1b`, bỏ `kernel_stack` (do harness tự viết) |
| 6 | dòng 1000 | `# sched: umode progress first=… last=… delta=…` | bộ đếm của core (`TICK_COUNT`/`dispatch`) |
| 7 | dòng 1032 | `# sched: hart=<n> picked=<a|b|c|u> n=<k>` | `# sched: hart=<n> picked=<ObjId> n=<k>` |
| 8 | dòng 1038 | `# sched: multi-hart harts=4 … first=[a, b, u, c] diff=… double=…` | **đã có** trong `smp_suite::run_x64` (`# core smp: … picks=[…] first=[t0..t4] double=0`) — dùng lại, **không** viết bản thứ hai |
| 9 | dòng 1183 | `# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]` | `# sched: prio first=<ObjId> viol=0 ticks=<t> picks=[…]` (priority thật của core) |
| 10 | dòng 1188 | `# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]` | `# sched: sleep wakes=<n> viol=0 ticks=<t>` (đường `notification_wait`/`signal`) |
| 11 | ca âm (n) dòng 1421 | xoá **cả** `# sched:` ⇒ ĐỎ | xoá dòng bằng chứng mới ⇒ ĐỎ |
| 12 | ca âm (p) dòng 1453 | `resumed=0` | `resumed=0` trên dòng mới |
| 13 | ca âm (q) dòng 1466 | `cs=0x1b` → `cs=0x8` | giữ (nếu dòng mới còn `cs=`) |
| 14 | ca âm (r) dòng 1478 | `delta=-50` | `delta=-50` trên bộ đếm mới |
| 15 | ca âm (s) dòng 1491 | `n=0` | `n=0` trên dòng mới |
| 16 | ca âm (t) dòng 1505 | xoá `hart=1|2|3` + `multi-hart harts=1` | xoá `# core smp:`/`# sched: hart=` mới |
| 17 | ca âm (u) dòng 1521 | xoá `hart=2|3` + `harts=2` | như trên |
| 18 | ca âm (v) dòng 1536 | `diff=0` | `diff=0` trên dòng mới |
| 19 | ca âm (w) dòng 1550 | ghép `# sched: ticks=10 tasks=3` | `# sched: ticks=10 tcbs=3` |
| 20 | ca âm (x) dòng 1564 | `prio … first=b` → `first=a` | `first=<ObjId>` mới |
| 21 | ca âm (y) dòng 1577 | `sleep wakes=3 …` | dòng `sleep` mới |
| 22 | ca âm (z) dòng 1591 | `prio … ticks=48 hard=1` | dòng `prio` mới (bỏ `hard` nếu core không có trần cứng) |
| 23 | ca âm (aa) dòng 1708 | `multi-hart … first=[a, a, u, c]` | dòng `# core smp:` mới |
| 24 | `soak-x86_64.sh` | **64 dòng** `# sched:` (10, 10b, 10c, 10d…) | cùng bộ dòng mới |

Tổng: **12 mẫu dương** trong `check_sched()` + **12 ca âm** (n,p,q,r,s,t,u,v,w,x,y,z) +
**64 dòng** `soak-x86_64.sh` + **6 TAP** (22..25, 29, 30) phải viết lại. `expected.txt` **không**
đổi (số TAP giữ 50). `mutants.toml` **không** có đột biến `# sched:` (đã kiểm: chỉ có đột biến
của `vifast-core`), nên không phải sửa — nhưng nên **thêm** đột biến mới của `dispatch`
(vd bỏ `published` refresh) để giữ độ phủ.

**Đề xuất**: task riêng (`KT-413c`) với **một D2** duyệt hợp đồng bằng chứng mới
`# sched: tcb=<ObjId> hart=<n> prio=<p> state=<…>` + đổi 24 mục trên + 64 dòng soak; khi ấy
`kbin-x86_64/src/sched.rs` **và** `plat-x86_64/src/sched.rs` xoá được **trọn** 2.162 dòng, đồng
thời trả lại **64 KiB** `.bss` (4 stack 16 KiB) cho ngân sách ảnh.

## 6. Rủi ro / cần Lead quyết

1. **Hai profile thay vì một tập ca duy nhất** (§3.1): hợp nhất **cấu trúc** đã xong (một tệp,
   một trait, một `Outcome`, dùng chung helper); gộp **tập ca** (union 5 ca) **không** làm được
   mà không đổi số TAP của ít nhất một arch ⇒ đổi `expected.txt`/`run-x86_64.sh` = đổi kỳ vọng
   harness (AGENTS §4.2). **Câu hỏi**: Lead có mở D2 cho việc này không? Nếu có, đề xuất hướng
   **thêm** ca (không bớt): rv/a64 3 → 5 TAP (thêm `core smp sleep migrate` + `core smp 4 hart`
   dạng ObjId), x64 2 → 5 TAP (thêm 3 ca `smp …`), `expected.txt` `ktest 74 → 76`,
   `run-x86_64.sh` `EXPECT_TESTS 50 → 53`. **Không** tự làm.
2. **PCID mặc định TẮT** (§3.2): mã cấp PCID riêng/VSpace + `enable()` đã có, nhưng **chưa**
   bật vì `run-x86_64.sh`/`soak-x86_64.sh` đòi `cr4_pcid=0` (nhánh đóng N5). **Câu hỏi**: có mở
   task bật PCID không? Cần thêm: (a) `flush_va` → `INVPCID` **theo PCID** (nếu không, hart khác
   đọc PA cũ — đúng lỗi N5); (b) đổi kỳ vọng `# ipi: pcid` ở **hai** runner + soak; (c) D2.
   Ước lượng 2–3 h. Hiện tại **0** hành vi đổi: `CR3` ghi ra **đúng bằng** `root` như trước.
3. **528 B `.bss`** thêm vào ảnh `product-shell` (`PCID_BITS` 512 B + 2 static) vì `VSpace::create`
   cấp PCID **luôn** (kể cả khi tắt) — để danh tính PCID là dữ kiện **đo được** của VSpace, không
   phải thứ bật sau mới có. Đo được: `data+bss = 8368 + 43672 = 52040 ≤ 262144` (headroom lớn).
   Nếu Lead muốn **0 byte** khi tắt thì phải `#[cfg]`-gate — nhưng khi ấy mã **không** được đo
   trong ảnh (mất bằng chứng "chạy thật"), nên tôi **không** làm.
4. **`pcid::enable()` chưa được gọi ở đâu** — **đúng thiết kế** (mặc định tắt). Nếu ai gọi nó
   trên đường boot, `# ipi: pcid` sẽ in `cr4_pcid=1` ⇒ **hai runner ĐỎ** cho tới khi có D2 đổi
   kỳ vọng. Đã ghi cảnh báo ở đầu `pcid.rs` + test `pcid_is_disabled_by_default`.
5. **`./v2/bin/v2 t0 vifast-kbin-x86_64` đỏ ở `cargo test`** (`E0152 duplicate lang item
   panic_impl`) — **pre-existing** (crate `no_std` bin + harness `std`), không phải hồi quy:
   task này không thêm `panic_handler`. T0 của crate ấy = clippy + 3 build (bảng §4).
6. **Chưa chạy full `run-x86_64.sh`** (39 mục + 33 ca âm, ~10+ phút) — theo phân tầng, đó là
   **T2 của Lead**. Bằng chứng thay thế: **một** boot thật `-smp 4` đã xác nhận `ok 48`/`ok 49`
   + dòng `# core smp:` **giống hệt** KT-413; T1 (dưới) chạy smoke QEMU.
7. **`migrate=0`** trong dòng `# core smp:` — **số đo thật**, không đổi so với KT-413 (5 TCB đều
   `home` cố định, không TCB nào `Ready` ở hart khác home). Bộ ca chỉ đòi `migrate=[0-9]+`.
8. **`vifast_ktest::SmpBridge` (crate root) đã xoá** — chỉ `coresmp.rs` và test x64 dùng; cả hai
   đã chuyển sang `smp_suite::SmpBridge`. Không còn tham chiếu nào khác (đã grep toàn repo).

## 4b. T1 — `gate --dev` (qua khoá máy)

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
v2 check: PASS (L1 chạy trong gate --fast)
--- fmt: OK
--- clippy: OK
--- L0 kernel workspace (KT-148): OK
--- L1: OK (1541 ms, đo bởi cổng)          # 1000 seed, 999 trace khác nhau
--- selftest pre-measure (KT-95): FAIL
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
# gate-steps registered=80 ran=66 skipped=14 failed=1
# step-time-total   597992 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**Vì sao T1 còn đúng một bước đỏ (không phải hồi quy)**: `v2/tools/pre-measure.sh` **từ chối** đo
trên cây bẩn (`refuse 7 tree-dirty`); worktree này có **12** tệp build-input chưa commit (đúng
phần KT-413b) ⇒ ca tự kiểm (a) — dựng ảnh giả `dirty=0` rồi đòi `PRE-MEASURE: OK` — **phải** đỏ.
Đây là **thiết kế** (chống đo ảnh không khớp cây), **cùng** hiện tượng đã ghi ở
`JOURNAL/kt413-smp-x64.md` §4.1 và `JOURNAL/kt411-ktest-generic.md` §T1: bước này chỉ xanh **sau
khi lead commit**. Mọi bước T1 khác đều **OK** (fmt, clippy, luật cấu trúc, L0 kernel workspace,
L1 1000 seed, wake paths, budget, …).

**Lượt T1 đầu** (trước khi sửa fmt): đỏ ở `--- fmt: FAIL` — `tests/pcid.rs` để `use` nhiều dòng
(tôi tạo tệp **sau** lượt `cargo fmt` đầu). Đã `cargo fmt --manifest-path v2/Cargo.toml --all`
⇒ `FMT OK`; lượt T1 sau chỉ còn bước pre-measure ở trên.

## 7. Tổng kết

| Hạng mục | Trạng thái |
|---|---|
| (1) Hợp nhất bộ ca SMP 3 arch | **XONG** — một mô-đun, một trait, hai profile; **0** TAP đổi số/nhãn; `expected.txt`/`run-x86_64.sh`/`soak` **0 chạm**; 0 chạm `rv`/`a64` |
| (2) PCID/ASID | **XONG phần cài đặt** — cấp PCID riêng/VSpace + `enable()` đo `CPUID`; **mặc định TẮT** (runner vẫn `cr4_pcid=0`); cách bật + câu hỏi ở §6.2 |
| (3) Gỡ 2.162 dòng `sched.rs` | **KHÔNG làm** (đúng luật task) — đo đầy đủ + 24 mục kỳ vọng phải đổi + đề xuất `KT-413c` có D2 (§5.1) |
| T0 | PASS (`vifast-ktest` 10/10, `vifast-plat-x86_64` toàn bộ + 6/6 `pcid`, clippy sạch, 3 ảnh build xanh) |
| `v2 check` | **PASS** (`TỔNG check 87289 ms`) |
| T1 | 65/66 bước OK; **1** bước đỏ = `selftest pre-measure` (cây bẩn — chỉ xanh sau khi lead commit) |
| T2 | **Lead chạy** (task cấm) |
