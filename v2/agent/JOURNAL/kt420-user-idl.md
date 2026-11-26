# KT-420 — 6 `.vidl` chuẩn + stub `vifast-idl` typed + dịch vụ `namesrv` + trait `PageSource` `vifast-rt`

- **Task**: `KT-420` (P0, Chặng 3 / Đợt `W8a` **Slot 2** — P0 User IDL) — `v2/agent/QUEUE.md`
  dòng `KT-420`; cơ sở `D2-262` (tách `KT-420`/`KT-420b`), `KE-HOACH-KIEN-TRUC.md` §4 hàng
  `KT-420`. Nguồn sự thật phụ: `v2/docs/u1-uefi-readiness.md` §2 (hợp đồng `.vidl`), `D2-99`/
  `D2-102c`/`D2-104` (method id sinh tuần tự, arity chính xác).
- **Nhánh / worktree / base**: `task/kt420-user-idl` @ `/private/tmp/vifast-wt/2` (slot 2), base
  `main` @ **`b76e3bc8`**; **commit của task `0cfc23bb`** (Lead commit hộ — hook `vifast-guard`
  đòi `VIFAST_ROLE=lead`, AGENTS §7); `CARGO_TARGET_DIR=…/target-task/slot-2` (ấm).
- **Ngân sách**: 3 h; khung **Thấp điểm chính** (`10:00–01:00 UTC`, 0,5×) — bắt đầu `00:45 JST`
  (`15:45 UTC` hôm trước), mã+T0 xong `01:21 JST`, T1 xong `01:34 JST` ⇒ **~49 phút** trong hạn.
- **Phạm vi (0 % chồng tệp với `KT-417d` đang chạy song song)**: `spec/protocols/**` (chỉ
  `.vidl`), `v2/lib/vifast-idl/**`, `v2/tools/vifast-gen/gen_idl.py`, `v2/test/gen/**`,
  `v2/user/**`, `v2/docs/u1-uefi-readiness.md`. **0 chạm** `v2/kernel/**`, `spec/abi.toml`,
  `spec/kernel.toml`, `v2/bin/**`, `STATE/QUEUE/DECISIONS/AGENTS`, `.github/**` — chứng minh
  bằng `git diff --stat` (rỗng) ở §5.1.

## 0. Kết luận ngắn (đọc trước)

**T1 trên cây sạch @ `0cfc23bb`: PASS** — `gate-steps registered=82 ran=68 skipped=14 failed=0`,
`gate-wall 750595 ms (750.6 s)`, `exit=0`. Log: `JOURNAL/kt420-user-idl/t1-clean.log`.

1. **Đủ 6 `.vidl` chuẩn** trong `spec/protocols/` (`ns`, `proc`, `fs`, `blk`, `socket`, `term` —
   `socket` đã có từ `D2-242` nhóm 6A) + **stub typed** sinh cho 5 tệp mới; `./v2/bin/vifast-idl
   --check` **PASS**. Ca kiểm của cổng (`v2 check` bước 39) mở từ **2 → 15 ca**, gồm **4 ca âm
   mới** cho cú pháp mở rộng — cả 15 xanh trong T1.
2. **Cú pháp mở rộng TỐI THIỂU, thuần cộng thêm**: `param bytes8/16/24/32` (nội tuyến N/8 word
   MR, `MessageInfo.length` = **số word MR**), cap `endpoint`/`notification`, `reply cap` (cap
   trong reply — cần cho `ns.lookup`). Stub của 5 interface cũ (`ring`, `pager`, `socket`,
   `meminfo`, `fsstat`) sinh lại **byte-identical** (0 diff) ⇒ 0 rủi ro cho ảnh/ABI.
3. **`vifast-namesrv`** (`v2/user/namesrv/`): bảng đăng ký **biên 8 ô, không cấp phát**, mã trạng
   thái **lấy từ bảng `spec/abi.toml [error]`** (test đối chiếu `vifast_abi::ERRORS` — không bịa
   số), handler typed qua `ns::Request/Response`; **10 host test**.
4. **`PageSource` cho allocator `vifast-rt`**: trait + `StaticPages` + `PageArena` (0 `unsafe`),
   `heap` chuyển sang `PageArena<StaticPages>` giữ `HEAP_BYTES = 8192`; **12 host test**; tái xuất
   `vifast_rt::{PageSource, PageArena, StaticPages, ArenaState, PAGE_SIZE}` để `KT-420b` chỉ thay
   **nguồn**, không sửa `GlobalAlloc`/`heap_stats`.
5. **Rủi ro lớn nhất đã biết, có tên**: `v2 check` **không** chạy host test của `namesrv`/`page`
   (bước 83 chỉ chạy `bump.rs`; `v2/bin/**` ngoài quyền task) ⇒ bằng chứng T0 nằm trong
   `JOURNAL/kt420-user-idl/t0.log`, **không** tự chặn hồi quy ở cổng (§6.1).

## 1. Bối cảnh & vì sao tách `KT-420` / `KT-420b`

`D2-262` tách `KT-420` khỏi phần `v2/kernel/` + cửa sổ `spec/abi.toml` để (a) giữ ≤ 3 h, (b) chạy
song song 100 % khác cây thư mục với `KT-417d` (chủ sở hữu duy nhất `spec/{abi,kernel}.toml` ở
`W8a`). Vì vậy task này **chỉ** làm tầng Hợp đồng/Userland:

| Hạng mục | Trong `KT-420` | Đẩy sang `KT-420b` (W8d) |
|---|---|---|
| `.vidl` chuẩn + stub typed | **đủ 6 tệp + sinh stub + ca âm** | — |
| `namesrv` | lõi dịch vụ + host test | cấp `NS_EP`, vòng `Recv`/`Reply`, badge |
| `PageSource` | trait + arena + host test, allocator dùng trait | nguồn `Frame`/`Untyped` thật, `exec(path)` |
| `spec/abi.toml` | **0 chạm** | `boot_abi = 2`, gỡ `unsupported` `Tcb.Exec` |

Hiện trạng xuất phát (đã đọc trước, không làm lại): `spec/protocols/{fsstat,meminfo,socket}.vidl`;
`v2/lib/vifast-idl` + `v2/bin/vifast-idl` + `v2/tools/vifast-gen/gen_idl.py` (KT-69, giới hạn 4 MR
+ 1 cap/method); `v2/user/` là workspace Cargo thật (`vifast-urt`/`vifast-rt`/`vifast-programs`,
`Cargo.lock` B3); allocator PoC = `runtime/lib.rs` `mod heap` trên arena tĩnh 8 KiB, lõi
`runtime/bump.rs` (KT-347/KT-404).

## 2. Thiết kế & quyết định (phần mở rộng `.vidl` là trọng tâm)

### 2.1 Vì sao **phải** mở rộng cú pháp (và mở đúng 3 thứ)

Cú pháp KT-69 chỉ có `u32`/`u64` + `frame`/`buffer` + `returns u32|u64|unit`. Bộ 6 giao thức cần:

| Nhu cầu | Không mở rộng thì hỏng ở đâu | Cách mở **tối thiểu** |
|---|---|---|
| **Tên** (`ns`) | tên dịch vụ là chuỗi ASCII; nhét tay `u8` vào 4 MR thì mất kiểu, mất `len`, dễ cắt ngầm | `param <tên> bytes16` (2 word) + `param len u32` |
| **Đường dẫn** (`proc.spawn`, `fs.open/mkdir/unlink`) | bố cục dây hiện có là **24 B/lượt** (`OP_PATH`, KT-77c) — phải giữ nguyên byte khi `KT-420b` đổi label → stub | `param <tên> bytes24` (3 word) + `param len u32` = đúng **4 MR** |
| **Cap kernel object** (`ns.register` trao `Endpoint`) | `CAP_TYPES` chỉ có `frame`/`buffer` ⇒ generator từ chối | thêm `endpoint`, `notification` (chỉ kiểm **loại**; cap vẫn đi qua send/receive window) |
| **Cap trong reply** (`ns.lookup` trả `Endpoint`) | reply chỉ mang scalar ⇒ client nhận tên nhưng **không có** cap để gọi dịch vụ | `reply cap <tên> <loại>` (tối đa 1/method) ⇒ `MessageInfo.caps` của reply = 1 |

Ràng buộc **giữ nguyên**: ≤ 4 tham số/method, ≤ 1 cap request + ≤ 1 cap reply, `returns` vẫn
`u32|u64|unit`, method id sinh tuần tự (D2-102c), `decode` kiểm **arity chính xác** (D2-104) và
từ chối tràn `u32`. `arity()` đổi nghĩa từ "số tham số" thành "**số word MR**" — đúng nghĩa của
`MessageInfo.length`; với mọi interface cũ hai nghĩa **trùng nhau** nên stub cũ byte-identical.

Quyết định **không** làm (có chủ đích, ghi để khỏi tranh luận lại): không thêm struct/string/float;
không thêm kiểu trả phức tạp; không nới trần 4 MR (dữ liệu dài đi **buffer cap của người gọi**,
đúng quy tắc §2.2 của `u1-uefi-readiness.md`).

### 2.2 Nội dung 6 giao thức (và vì sao mỗi lựa chọn)

| Tệp | Interface | Method | Quyết định đáng chú ý |
|---|---|---|---|
| `ns.vidl` | `ns` v1 | `register`(bytes16+len, cap `endpoint`), `lookup`(bytes16+len, **reply cap**), `unregister` | tên ≤ 16 B, **không** cần buffer cap cho mỗi lần tra cứu (lookup là đường nóng của `KT-420b`) |
| `proc.vidl` | `proc` v1 | `spawn`(bytes24+len→pid), `wait`, `kill(pid,how)`, `list(cursor)→u64` | `# caps=none`: cap do kernel cài lúc spawn; `spawn` **không** mang cap Frame (đã dùng trọn 4 MR) — `KT-420b` đọc ELF qua fssrv vào Frame rồi `Tcb.Exec(elf_frame)` |
| `fs.vidl` | `fs` v1 | `open`, `read`, `write`, `close`, `mkdir`, `unlink`, `rename`, `readdir`, `stat`, `sync` | dữ liệu dài + **2 đường dẫn** (`rename`) đi **buffer cap** của người gọi; status = `status \| value<<8` **đúng bảng `ST_OK/ST_END/ST_ERR/ST_NOFS` của KT-77c**; **không** lặp `statfs` (một nguồn: `fsstat.vidl`) |
| `blk.vidl` | `blk` v1 | `capacity`, `sector_size`, `read`, `write`, `flush` | đơn vị **sector** (không hardcode 512/4096); dùng chung cho `virtio-blk` (KT-422) và `nvme` (KT-423b) |
| `term.vidl` | `term` v1 | `put(len) cap text buffer → unit`, `get(len) cap text buffer → u32` | byte qua buffer; `put` trả `unit` (đúng §2.2 doc: lỗi terminal không được treo chương trình gọi) |
| `socket.vidl` | `socket` v1 | **giữ nguyên 6 method** (D2-242 nhóm 6A) | không sửa — tránh churn ngoài phạm vi |

Mã trạng thái **không bịa số**: mọi mã của `ns` là mã **có tên** trong `spec/abi.toml [error]`
(`InvalidCapability` 1, `InvalidArgument` 5, `SlotOccupied` 7, `SlotEmpty` 8, `Busy` 14) và
**test host đối chiếu `vifast_abi::ERRORS`** — sửa spec mà quên crate là ĐỎ.

### 2.3 `namesrv`: vì sao là **thư viện**, không phải `[[bin]]`

Chương trình U-mode cần bố cục CSpace (`NS_EP`, slot reply, badge) do kernel dựng — đúng việc của
`KT-420b` (chủ sở hữu `spec/abi.toml`). Ở `KT-420`, crate là **lõi kiểm được trên host**: bảng
biên + bộ chuyển `ns.vidl`. Bảng **không** giữ cap thật (cap chỉ sống trong CSpace): ô lưu
**CPTR** nơi tầng IPC đã cài cap nhận được (`read_received_caps`), và `KT-420b` đối chiếu badge
trước khi reply. Fail-closed: tên sai/`len` vượt/thiếu cap/bảng đầy/trùng tên ⇒ **mã lỗi có tên**,
bảng **không đổi** (không ghi đè — chống đánh tráo dịch vụ).

### 2.4 `PageSource`: vì sao trait ở shim (`runtime/page.rs`) mà tái xuất ở `vifast-rt`

Phần `unsafe` **duy nhất** của đường cấp phát là `unsafe impl GlobalAlloc` trong
`v2/user/runtime/lib.rs` (danh sách đóng D2-17/D2-253); `vifast-rt` là crate `forbid(unsafe_code)`
**và phụ thuộc** shim (`rt → urt`). Nếu đặt trait trong `rt` thì allocator (ở `urt`) **không thấy**
trait ⇒ trái yêu cầu "allocator dùng trait". Vì vậy:

- trait + arena ở `runtime/page.rs` (cùng chỗ với `bump.rs` — lõi cấp phát), **0 `unsafe`**;
- `vifast-rt` **tái xuất** ⇒ chương trình dùng `vifast_rt::PageSource` (API công khai của runtime);
- `PageArena` trả **VA tuyệt đối** (căn lề theo địa chỉ thật, sửa luôn lỗi tiềm ẩn "căn theo
  offset" của bump cũ); trạng thái tuần tự hoá được (`ArenaState`) để `GlobalAlloc` giữ trong
  `AtomicUsize` — **không** `static mut`, không `unsafe` mới.

Giới hạn có chủ ý (ghi trong đầu tệp): một lần cấp **≤ 1 trang**; `KT-420b` thêm `alloc_pages(n)`
khi có nguồn nhiều trang liền. `HEAP_BYTES = 2 × 4096 = 8192` **giữ nguyên** để dòng evidence
`# u1a: heap used=…/8192` của ảnh sản phẩm không đổi (KT-347).

### 2.5 Tài liệu: sửa `u1-uefi-readiness.md`, **không** sửa `k6-idl.md`

`v2/docs/k6-*.md` nằm trong glob của `verify-numbers.py` (mọi số phải là khối `scope=` trích
nguyên văn log) — sửa nó ở task này là tự tạo việc sai công cụ. Cú pháp mở rộng vì vậy ghi ở
`v2/docs/u1-uefi-readiness.md` **§2.2 + §2.2.1** (bảng 6 giao thức + 3 mở rộng + ràng buộc) và
docstring của generator.

## 3. Đỏ-trước (D2-05) — log máy sinh, **trước** khi có tính năng

### 3.1 (a) 6 `.vidl` chuẩn — `JOURNAL/kt420-user-idl/red-vidl.log`

Ca kiểm của cổng (`v2/test/gen/run_idl_fixtures.sh`, bước 39 của `v2 check`) được mở từ 2 → 15 ca
**trước** khi viết gì; chạy trên cây chưa có tính năng ⇒ **11/15 đỏ**:

```text
not ok 3 - thiếu .vidl chuẩn: ns proc fs blk term
not ok 4 - ns.vidl: không có tệp để kiểm method
...
not ok 10 - thiếu stub/module: generated_ns.rs 'pub mod ns;' ... generated_term.rs 'pub mod term;'
ok 11 - gen-check cây thật: stub khớp mọi .vidl          ← (phần đã có từ KT-69, không đỏ giả)
not ok 12 - bytes7 (không phải bội 8) bị từ chối nhưng SAI lý do (mong 'bytes phải là bội số của 8')
...
# idl fixtures: FAIL (11)
exit=1
```

### 3.2 (b) `namesrv` — `JOURNAL/kt420-user-idl/red-namesrv.log`

```text
$ cargo test --manifest-path v2/user/Cargo.toml -p vifast-namesrv
error: package ID specification `vifast-namesrv` did not match any packages
exit=101
```

### 3.3 (c) `PageSource` — `JOURNAL/kt420-user-idl/red-pagesource.log`

```text
$ rustc --edition 2021 --test v2/user/runtime/page.rs
error: couldn't find file `v2/user/runtime/page.rs`
exit=1
```

Sau khi cài: 15/15 (`green-vidl.log`), 10/10 namesrv, 12/12 page — xem §5.

## 4. Thay đổi theo tệp (tóm tắt kỹ thuật)

- **`v2/tools/vifast-gen/gen_idl.py`**: `SCALARS`/`BYTES`/`words()`; `Method.mr_words`,
  `Method.reply_cap`; parse `reply cap`; kiểm `mr_words > 4` ⇒ từ chối; `rust_ty`/`put_bytes`/
  `get_bytes` (sinh **chỉ khi** interface có `bytesN`); `arity()` = word; `reply_cap_count()` sinh
  **chỉ khi** có reply cap; `targets()` thêm 5 tệp. Đường scalar-only giữ **nguyên văn** ⇒ stub cũ
  0 diff.
- **`spec/protocols/{ns,proc,fs,blk,term}.vidl`** (mới): nội dung §2.2, mỗi tệp có header giải
  thích quy ước dây + mã trạng thái + vì sao chọn inline/buffer.
- **`v2/lib/vifast-idl/src/lib.rs`**: 5 `pub mod` (`ns`, `proc`, `fs`, `blk`, `term`) + 4 test mới
  (hợp đồng 6 giao thức, roundtrip `bytesN` + arity word, `reply cap`, "stub mới không
  unsafe/unwrap và đúng nguồn").
- **`v2/test/gen/run_idl_fixtures.sh`**: 2 → **15 ca** (2 ca cũ giữ nguyên; + đủ 6 tệp; + method
  hợp đồng từng interface; + stub/module; + `--check` cây thật; + 4 ca âm: `bytes7`, hai
  `reply cap`, cap lạ, 5 MR) + fail-closed nếu số ca lệch dòng `1..15`.
- **`v2/user/namesrv/{Cargo.toml,lib.rs}`** (mới): bảng biên, `valid_name`, `register/lookup/
  unregister`, `handle(&ns::Request, Option<u32>) -> Response`, mã trạng thái + test đối chiếu
  ABI; 10 host test.
- **`v2/user/runtime/page.rs`** (mới): `PageSource`, `StaticPages`, `PageArena`, `ArenaState`;
  12 host test. **`v2/user/runtime/lib.rs`**: `#![cfg_attr(not(test), no_std)]`, `pub mod page`
  (gated `test|rt_alloc`), `mod heap` chuyển sang `PageArena<StaticPages>` (state trong 4
  `AtomicUsize`, `HEAP_PAGES = 2`, `HEAP_BYTES = 8192`).
- **`v2/user/rt/lib.rs`**: tái xuất `PageSource`/`PageArena`/`StaticPages`/`ArenaState`/`PAGE_SIZE`
  (+ `HEAP_PAGES`), `cfg_attr(not(test), no_std)`, `#[panic_handler]` gated `not(test)` (để
  `cargo test -p vifast-rt` = 0 test vẫn compile — trước task này T0 của crate này **đỏ sẵn** vì
  lý do đó).
- **`v2/user/Cargo.toml`/`Cargo.lock`**: thêm member `namesrv` (+ path dep `vifast-idl`,
  dev-dep `vifast-abi`); lockfile chỉ **thêm** 3 gói path, đồng bộ B3 xanh.
- **`v2/docs/u1-uefi-readiness.md`**: §2.2 bảng 6 giao thức (bỏ `net.vidl` — đã gộp vào `socket`
  từ D2-242), §2.2.1 cú pháp mở rộng, §2.3 `PageSource`.

## 5. Bằng chứng máy sinh

### 5.1 Ranh giới: 0 chạm `v2/kernel/` & `spec/{abi,kernel}.toml`

```text
$ git diff --stat -- v2/kernel spec/abi.toml spec/kernel.toml
(trống)
$ git diff --cached --stat | tail -1        # trước commit của Lead
28 files changed, 3651 insertions(+), 71 deletions(-)
```

### 5.2 T0 (`JOURNAL/kt420-user-idl/t0.log`)

```text
$ cargo test --manifest-path v2/user/Cargo.toml -p vifast-urt -p vifast-namesrv
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
$ cargo test --manifest-path v2/Cargo.toml -p vifast-idl
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
$ bash v2/test/gen/run_idl_fixtures.sh
# idl fixtures: PASS
exit=0
$ ./v2/bin/v2 t0 vifast-idl vifast-urt vifast-namesrv vifast-rt
v2 t0: PASS (4 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```

Trong lúc sửa có **một** lỗi T0 thật (clippy `identity_op` ở `page.rs`: `Some(0x1008 + 0)`) — sửa
rồi chạy lại, không nới lint.

### 5.3 `v2 check` trước T1 (`JOURNAL/kt420-user-idl/check.log`)

```text
$ ./v2/bin/v2 check
50 bước ": OK"   (CHECK_STEPS_EXPECTED = 50)
v2 check: PASS (L1 chạy trong gate --fast)
exit=0
```

### 5.4 Ảnh sản phẩm riscv64 — `JOURNAL/kt420-user-idl/budget.log`

Build lại ảnh sau thay đổi (0 chạm kernel nên kỳ vọng **không đổi byte**), đo ngân sách:

```text
# image=product … file=619128 arch=riscv64(e_machine=243) data+bss=115344 budget=262144 …
# data+bss-sections: .data=8160 .bss=107184
exit=0
```

`data+bss = 115344 B ≤ 262144 B`; `file=619128` trùng **đúng** ảnh build trước thay đổi ⇒
`u1a.elf`/heap mới không đẩy `.bss` (u1a.elf **không** nhúng vào ảnh — `kt347-u1a.sh` đã canh).

### 5.5 T1 — `JOURNAL/kt420-user-idl/t1-clean.log` (**lượt duy nhất, PASS**)

```text
# KT-420 — T1 (gate --dev) trên cây sạch HEAD 0cfc23bb
# bắt đầu: 2026-11-27 01:22:02 JST
--- idl fixtures
# idl fixtures: PASS
--- idl fixtures: OK
--- L1 (sim 1.000 seed)
# L1 OK: 1000 seed, 999 trace khác nhau
--- L1: OK (900 ms, đo bởi cổng)
--- QEMU aarch64 (K4' bước 3): OK
--- QEMU x86_64 (K5' bước 1): OK
--- QEMU smoke (T1: 7 ảnh × 1 boot): OK
--- workspace v2/user: clippy 3 arch + host test (KT-404): OK      ← 8/8 ca bump, 3 arch clippy -D warnings
# gate-steps registered=82 ran=68 skipped=14 failed=0
# gate-wall 750595 ms (750.6 s)
# gate-cpu user=1038.737 s sys=348.880 s total=1387.617 s
# gate-lanes 6 làn nền (F A D E B C) + main + làn đuôi tuần tự (sau) + 5 làn đuôi song song …
# gate-phase pha1=109444 ms pha2=478313 ms pha3=161038 ms (tổng 748795 ms)
# gate-critical-path 748795 ms (tổng các pha đo được; wall=750595 ms)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
exit=0
# kết thúc: 2026-11-27 01:34:34 JST
```

14 bước **bỏ qua đúng nhãn T1** (in rõ `--- <tên>: BỎ QUA (T1 — chỉ chạy ở T2)`): `selftest runner
ktest`, `selftest qemu guard`, `selftest persist đỏ-trước`, `cắt điện giữa lúc ghi`, `selfbreak 5
luật K6`, 3 ca `git-guard`, 4 ca QEMU virtio (net/input/gpu/gpu-x86_64), `kiểm máy --cfg test`,
`wake dep-info --product-fs` — **không** bỏ âm thầm bước nào. T1 **không** phải full gate.

## 6. Rủi ro / nợ (trung thực) & đề xuất task kế

1. **Nợ P1 — host test của `namesrv`/`page` chưa nằm trong cổng.** `v2 check` bước 83 chỉ chạy
   `rustc --test runtime/bump.rs`; hai bộ test mới (22 ca) chỉ chạy ở T0/tay. Muốn thành hàng rào
   máy thì phải sửa `v2/bin/lib/steps_check.sh` (**ngoài quyền task này**). **Đề xuất**: task nhỏ
   (~20 phút) thêm `cargo test -p vifast-urt -p vifast-namesrv` vào bước 83 (hoặc bước riêng),
   `CHECK_STEPS_EXPECTED` +1. Cho tới lúc đó: bằng chứng phụ thuộc `t0.log` + kỷ luật chạy T0.
2. **`KT-420b` phải nối đúng 3 chỗ** (ghi để không hiểu sai hợp đồng): (a) `lookup` reply **chỉ
   đặt** `MessageInfo.caps = 1`; tầng IPC phải **cài** cap `Endpoint` vào receive window người gọi
   (crate giữ CPTR, không giữ cap — và **chưa** có đường thu hồi cap khi `unregister`, sẽ cần
   `CNode.Delete`/badge policy); (b) `NS_EP` + vòng `Recv/Reply` + badge cho `namesrv`;
   (c) `PageSource` cắm `Frame`/`Untyped` — cần `alloc_pages(n)` cho khối > 1 trang và luật
   "trang nào của ai" (hiện chỉ một nguồn tuần tự).
3. **`fs.vidl` v1 chưa có `truncate/link/fsck/statfs`** — `statfs` ở `fsstat.vidl` (một nguồn);
   `KT-422` cần thêm thì theo quy tắc "đổi bố cục ⇒ tăng `version`, không tái dùng method id"
   (§2.2 doc). `fs.rename` dùng buffer 2 đường dẫn — `KT-422` phải giữ đúng bố cục đó nếu không
   tăng version.
4. **`proc.spawn` không mang cap Frame** (trần 4 MR) ⇒ `KT-420b`/`KT-421` đọc ELF ở User Space
   (fssrv → Frame → `Tcb.Exec(elf_frame)`); nếu chốt thiết kế "procsrv nhận thẳng Frame" thì cần
   method mới (`spawn_elf`) + tăng version, **không** nhét cap thứ hai.
5. **`term.put` trả `unit`** (theo thiết kế §2.2): lỗi ghi terminal bị nuốt — chấp nhận cho mốc
   TUI, nhưng nếu `termsrv` cần backpressure thì phải đổi sang `u32` (version 2).
6. **`#![cfg_attr(not(test), no_std)]` ở `vifast-urt`/`vifast-rt`**: chỉ ảnh hưởng build test
   host; `cfg(test)` **không** bao giờ bật khi build ảnh (T1 `kiểm máy --cfg test` là bước T2 —
   lần này bị bỏ qua theo nhãn T1, nên **Lead nên để T2 xác nhận** điểm này; lý do: hai tệp này
   nay có `cfg(test)` trong mã).
7. **`v2/docs/k6-idl.md` chưa cập nhật** cú pháp mở rộng (cố ý — thuộc glob `verify-numbers`,
   mọi số phải trích log). Nếu muốn tài liệu KT-69 phản ánh `bytesN`/`reply cap` thì làm ở task
   riêng có log cổng tương ứng, hoặc chuyển phần cú pháp sang `u1-uefi-readiness.md` (đã làm).

## 7. Tệp đã đổi

| Tệp | Loại | Nội dung |
|---|---|---|
| `spec/protocols/ns.vidl` | **mới** | `ns` v1: register/lookup(reply cap)/unregister, tên `bytes16` |
| `spec/protocols/proc.vidl` | **mới** | `proc` v1: spawn/wait/kill/list, đường dẫn `bytes24`, `# caps=none` |
| `spec/protocols/fs.vidl` | **mới** | `fs` v1: 10 method; dữ liệu/2 đường dẫn qua buffer cap; status bảng KT-77c |
| `spec/protocols/blk.vidl` | **mới** | `blk` v1: capacity/sector_size/read/write/flush, đơn vị sector |
| `spec/protocols/term.vidl` | **mới** | `term` v1: put(unit)/get(u32), byte qua buffer |
| `v2/tools/vifast-gen/gen_idl.py` | sửa | `bytesN`, cap `endpoint/notification`, `reply cap`, arity=word, 5 target mới |
| `v2/lib/vifast-idl/src/{lib.rs}` | sửa | 5 module + 4 test hợp đồng mới |
| `v2/lib/vifast-idl/src/generated_{ns,proc,fs,blk,term}.rs` | **mới** | stub typed sinh tự động (1475 dòng) |
| `v2/test/gen/run_idl_fixtures.sh` | sửa | 2 → 15 ca (6 giao thức + 4 ca âm cú pháp) |
| `v2/user/namesrv/{Cargo.toml,lib.rs}` | **mới** | lõi `namesrv` no_std + 10 host test |
| `v2/user/runtime/page.rs` | **mới** | `PageSource`/`StaticPages`/`PageArena`/`ArenaState` + 12 host test |
| `v2/user/runtime/lib.rs` | sửa | `mod page`; `mod heap` dùng `PageArena<StaticPages>`; `cfg_attr(not(test), no_std)` |
| `v2/user/rt/lib.rs` | sửa | tái xuất `PageSource`…; `cfg_attr`/`panic_handler` gated `not(test)` |
| `v2/user/Cargo.toml`, `v2/user/Cargo.lock` | sửa | member `namesrv` + 3 gói path (B3 xanh) |
| `v2/docs/u1-uefi-readiness.md` | sửa | §2.2 bảng 6 giao thức, §2.2.1 cú pháp mở rộng, §2.3 `PageSource` |
| `v2/agent/JOURNAL/kt420-user-idl/{red-vidl,red-namesrv,red-pagesource,t0,check,budget,green-vidl,t1-clean}.log` | **mới** | log bằng chứng máy sinh |

**0 tệp** dưới `v2/kernel/`, `spec/abi.toml`, `spec/kernel.toml`, `v2/bin/`,
`STATE/QUEUE/DECISIONS/AGENTS`, `.github/**`.

## 8. Commit

- Commit mã của task: **`0cfc23bb`** — `[KT-420] 6 .vidl chuẩn + stub vifast-idl typed +
  namesrv + trait PageSource vifast-rt` (Lead commit hộ; hook `vifast-guard` đòi
  `VIFAST_ROLE=lead` — Agent **không** tự đặt, AGENTS §7).
- Commit journal (đề xuất, lượt này): `[KT-420] journal + bằng chứng T1 xanh trên cây sạch
  (0cfc23bb)`.
- Không push, không tag, không `--no-verify`.

## 9. T1 (lệnh / kết quả / log)

```bash
cd /private/tmp/vifast-wt/2      # HEAD 0cfc23bb, cây sạch (chỉ kt420-plan.md untracked)
./v2/bin/v2 gate --dev           # ⇒ exit 0
```

- Log: `v2/agent/JOURNAL/kt420-user-idl/t1-clean.log` (3.4 k dòng, có mốc đầu/cuối + `exit=0`).
- Dòng tổng kết: `# gate-steps registered=82 ran=68 skipped=14 failed=0`;
  `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate
  --fast')`.
- Thời gian: `# gate-wall 750595 ms (750.6 s)`; CPU `user=1038.737 s sys=348.880 s
  total=1387.617 s`; critical path `748795 ms`.
- L1: `# L1 OK: 1000 seed, 999 trace khác nhau` (`--- L1: OK (900 ms, đo bởi cổng)`).
- Lượt chạy: **một lượt duy nhất**, xanh, **không** retry (D2-07 không cần dùng tới).
- **T1 không phải full gate**: 14 bước T2-only bị bỏ **có nhãn**; đề nghị Lead chạy T2
  (`gate --fast`) trên ứng viên tích hợp như quy trình.

## 10. Bàn giao cho Lead

1. **Merge + T2**: nhánh `task/kt420-user-idl` @ `0cfc23bb` (+ commit journal này). Chạy
   `./v2/bin/v2 gate --fast` trên ứng viên tích hợp. Điểm cần T2 xác nhận thêm: bước
   `kiểm máy --cfg test` (KT-256) — vì task này thêm `cfg(test)` vào `vifast-urt`/`vifast-rt`
   (chỉ ảnh hưởng build test host, nhưng đây là bước duy nhất chứng minh bằng dòng lệnh rustc
   thật của ảnh).
2. **`KT-420b` (W8d) có thể mở ngay** khi `KT-417d` DONE: hợp đồng đã sẵn — `ns.vidl` +
   `namesrv` (chỉ cần nối `NS_EP`/vòng `Recv`), `PageSource` (chỉ cần thay nguồn), và 6 `.vidl`
   cho các dịch vụ `KT-421`/`KT-422`/`KT-423b`.
3. **Đề xuất task nhỏ (≤ 30 phút, ngoài phạm vi)**: nối host test `vifast-urt`/`vifast-namesrv`
   vào bước 83 của `v2 check` để 22 ca mới thành hàng rào máy thay vì bằng chứng T0 (§6.1).
4. **Không cần hành động**: `v2/docs/k6-idl.md` giữ nguyên (lý do ở §2.5/§6.7);
   `QUEUE.md`/`STATE.md`/`DECISIONS.md` không bị chạm — Agent 0 cập nhật trạng thái `KT-420`
   theo `D2-262`.
