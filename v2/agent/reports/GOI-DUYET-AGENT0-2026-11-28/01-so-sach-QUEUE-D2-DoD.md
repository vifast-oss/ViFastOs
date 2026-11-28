# BÁO CÁO SỔ SÁCH — Agent 0 dán (sub-agent `prep-bookkeeping`, CHỈ ĐỌC)

**Mốc**: `main` = `82502f7d` (2026-11-28 20:28:20 +0900) · soạn 2026-11-28 20:5x JST (`11:5x UTC`) · 0 sửa repo, 0 build/test/QEMU/cargo
**Bằng chứng gốc**: `git log/show` · `v2/agent/{QUEUE,STATE,DECISIONS}.md` · `v2/agent/JOURNAL/**` · `/tmp/t2-kt417c-hotfix-82502f7d.log` · `/tmp/t1-kt417c-hotfix-c73eb77a.log` · `/tmp/audits/{docs-audit,hotfix-review,test-audit,harness-audit}.md` · `v2/test/baseline/b1.1-ratchet.toml` · `spec/abi.toml`

## §0 Checklist (6 dòng) — dán hay phải quyết

| # | Mục | Việc Agent 0 làm | Phải quyết? |
|---|---|---|---|
| 1 | §1.A — **6 dòng QUEUE** `READY`/`BLOCKED` → `DONE` (dòng 456, 457, 478, 480, 483, 486) | thay nguyên dòng bằng khối dán tương ứng | **Không** — chỉ dán (trừ `KT-417c` đi kèm §2) |
| 2 | §1.B — dòng 479 `KT-417d`: giữ `BLOCKED`, ghi rõ **3/5 DONE** + chờ duyệt tách | thay nguyên dòng | **Không** — chỉ dán |
| 3 | §1.C — dòng 578 (danh sách `READY` hiện tại + đếm `BLOCKED`) | thay nguyên dòng | **Không** — chỉ dán |
| 4 | §2 — dòng `APPROVED 2026-11-28 Agent 0` + mục **`D2-264`** (+ sửa header `DECISIONS.md:28`) | dán 2 khối vào `QUEUE.md` và `DECISIONS.md` | **CÓ**: `KT-417c` = `DONE` **có điều kiện** (khuyến nghị) hay để mở tới khi đóng 3 khoảng trống bằng chứng |
| 5 | §3 — **DoD W9** viết lại cho `KT-421/422/423` (+ 2 dòng `KT-423b`/`KT-424`) | thay 3–5 dòng | **CÓ**: chọn **Phương án B** (phân loại `*_svc` → khoá ratchet mới, khuyến nghị) hay **A** (mở `KT-423c`, +2 h) |
| 6 | §4 — khối `STATE.md` (giữa 2 dòng mốc) | Lead/Agent 2 dán; Lead **điền 1 chỗ** = kết quả `mutants --all` | **Không** — chỉ dán |

---

## §1 QUEUE sync — 7 dòng cần đổi (6 `DONE` + 1 `BLOCKED` ghi chú) + 1 dòng ghi chú vận hành

Đối chiếu `git log --oneline main` ↔ `STATE.md`: **6 task đã merge + xanh cổng** mà QUEUE còn in `READY`/`BLOCKED` — đúng con số `/tmp/audits/docs-audit.md` F3. Đã kiểm lại **từng cái** bằng máy (merge commit + nhánh + nhật ký + log cổng); **không** có task thứ 7 nào lệch.

| Dòng | Task | QUEUE hiện tại | Đề xuất | Commit / bằng chứng cổng |
|---:|---|---|---|---|
| 456 | `KT-408d` | `READY` | **`DONE`** | mã `9a962a7c` · nhật ký `717f57a1` · merge `1d15b081` · T1 `82/68/14/0` · T2 81/81 @`82502f7d` |
| 457 | `KT-408e` | `READY` | **`DONE`** | mã `857bbd19` · nhật ký `5087f283` · merge `fff6cf91` · T1 PASS 1.015.981 ms · T2 @`82502f7d` |
| 478 | `KT-417b` | `READY` | **`DONE`** | mã `0a247faa` · nhật ký `41dc3308` · merge `bb3c144e` · T1 `82/68/14/0` · T2 @`82502f7d` |
| 480 | `KT-417c` | `BLOCKED` | **`DONE`** (kèm điều kiện §2) | mã `a5541601` + hotfix `c73eb77a` · merge `075b0639` + `82502f7d` · T1 `82/68/14/0` 668,8 s · **T2 81/81 727,1 s** |
| 483 | `KT-418c` | `READY` | **`DONE`** | mã `f237cd34` · nhật ký `6a4a986a` · merge `b76e3bc8` · T1 `82/68/14/0` · T2 @`82502f7d` |
| 486 | `KT-420` | `BLOCKED` | **`DONE`** | mã `0cfc23bb` · nhật ký `8a749e28` · merge `b8bcbd74` · T1 `82/68/14/0` · T2 @`82502f7d` |
| 479 | `KT-417d` | `BLOCKED` | **giữ `BLOCKED`** + ghi `3/5 DONE` (QUEUE chỉ có 4 trạng thái, không có `PARTIAL`) | p1 `2fc15e0e`/`60aba561` · hotfix `e65e42a4`/`a775affe` · p2/2 chờ duyệt tách |
| 578 | ghi chú vận hành | “4 task `READY` … 11 `BLOCKED`” | thay bằng khối §1.C | sau khi dán §1.A/§1.B: **0 `READY` · 9 `BLOCKED`** (8 thuần DAG + `KT-417d`) · **380 `DONE`**/389 |

> **Số học kiểm được**: `grep -c "| DONE |"` 374 → **380**; `| READY |` 4 → **0**; `| BLOCKED |` 11 → **9**; tổng vẫn **389** dòng `^| KT-`. Khớp `STATE.md:28` (DONE 380) trừ 1 điểm: `STATE` ghi `PARTIAL 1` cho `KT-417d` mà QUEUE không có trạng thái đó — xử lý ở §1.B.

> **Một lưu ý trung thực về bằng chứng T2 (đừng đọc quá)**: T2 duy nhất hiện có là **@`82502f7d`** — tức khoá đầu vào là `82502f7d`, không phải từng merge commit của 4 task cũ (`bb3c144e`/`b76e3bc8`/`1d15b081`/`fff6cf91`/`b8bcbd74`). Vì `82502f7d` là **hậu duệ** của tất cả chúng, T2 này **bao trùm** cây của chúng, nên câu "T2 PASS @`82502f7d` bao trùm commit này" là đúng — nhưng **không** được dẫn nó như "T2 riêng của task X". Đây cũng chính là lỗ `harness-audit` **F12** (T2 không sinh khoá đo `commit=`/`build-inputs=`/`dirty=`/`seed=`), nên nếu Agent 0 muốn sổ sách chặt hơn thì nên chốt luôn F12 trong cùng đợt (mở 1 task `KT-00x` hợp lệ để T2 in khoá đo).

### §1.A — 6 khối dán (thay **nguyên dòng**, giữ đúng 6 cột)

**`QUEUE.md:456`** — `KT-408d` · thay nguyên dòng:

```text
| KT-408d | **P1** **Chặng 2 (W7b) — Cắt điện tất định cho `persist.sh --crash` (`MODE_CRASH_MID_WRITE`, đóng `Q7`, 0 % chồng tệp)** | KT-408c | 1,5 h | DONE | **DONE 2026-11-26 (merge `1d15b081`; mã `9a962a7c`; nhật ký `717f57a1` — 5 lượt `--crash` tất định, sha ảnh đĩa giống hệt, đỏ-trước `records=13`; T1 PASS `82/68/14/0`; T2 PASS 81/81 727,1 s @`82502f7d` bao trùm commit này)**: **(APPROVED 2026-11-26 Agent 0 `D2-262` — MỞ `READY` NGAY ở W7b Slot 3, chỉ chạm `kbin-riscv64/src/ktest_persist.rs`, `v2/test/boot/persist.sh`, `v2/test/boot/persist_host.py`)** Thay cơ chế poll dòng log `sleep 0.05` (gây flake `records=13` dưới tải T2 hoặc cắt quá sớm trước khi tạo `/p7.bin`) bằng điểm cắt I/O tất định: (1) mở rộng kịch bản 32 B `PL75` tại LBA 0 (dùng byte dự phòng `17..20`, tương thích ngược `mode=0..2`) thêm `MODE_CRASH_MID_WRITE = 3` + `crash_after_writes: u32`; (2) khi ghi pha 7 đạt đúng mốc `N` sector đẩy xuống thiết bị (vd 72/144 block của `/p7.bin`, trong lúc `CACHE_DATA` còn block bẩn chưa flush), `ktest_persist` in `# persist: crash_cut writes=<N> pending=<P>` rồi gọi `PLATFORM.halt(77)` KHÔNG flush cache; (3) `persist.sh --crash` luôn chứng minh `partial=1` + journal replay `fsck` + `dọn tệp dở` + phục hồi đủ `b07r → b08r → b01r`; (4) ca đỏ-trước: tắt `crash_cut` ⇒ `persist_host.py` đỏ `records=13`; 0 sửa `spec/`, 0 nới ngưỡng; T2 PASS. |
```

**`QUEUE.md:457`** — `KT-408e` · thay nguyên dòng:

```text
| KT-408e | **P1** **Chặng 2 (W7b) — Chống lệch SSOT vĩnh viễn cho `soak-{aarch64,x86_64}.sh` (`46`/`47`→`50`, `lib/tap-parity.sh`, probe fail-fast `a64`, `D2-263`)** | KT-412, KT-413c, KT-418 | 1,5 h | DONE | **DONE 2026-11-26 (merge `fff6cf91`; mã `857bbd19`; nhật ký `5087f283`; T1 PASS `step-time-total 1015981 ms`, selftest `soak-aarch64` 62/62 + `soak-x86_64` 72/72, `TAP_TESTS` 46/47→**50** + `lib/tap-parity.sh` khoá `soak==run==kbin`; T2 PASS 81/81 727,1 s @`82502f7d`)**: **(APPROVED 2026-11-26 Agent 0 `D2-263` — MỞ `READY` NGAY ở W7b Slot 4, 0 % chồng tệp `KT-417b`/`418c`/`408d`; CHỈ chạm `soak-aarch64.sh`, `soak-x86_64.sh` và tệp mới `v2/test/boot/lib/tap-parity.sh`, KHÔNG sửa `run-*.sh`)** (1) **Đồng bộ & SIẾT bất biến ngang `run-*.sh`**: `soak-aarch64.sh` nâng `TAP_TESTS` `46→50`, kiểm `# smp core:` (TAP 47..49 `KT-412`) + `# pan:` & `ok 50 - pan: el1 khong doc trang el0` (`KT-418`), cập nhật fixture `--selftest` + ≥ 2 ca âm; `soak-x86_64.sh` nâng `TAP_TESTS` `47→50`, kiểm `ok 48 - core smp 4 hart`, `ok 49 - core smp sleep migrate` (`KT-413`) + `# smep/smap: … hardened=1` & `ok 50 - smep/smap: bat va ca am` (`KT-418`), cập nhật fixture `--selftest` + ≥ 2 ca âm; (2) **Khóa SSOT trong `--selftest` (chạy mọi T2, 0 bước cổng mới)**: dựng `v2/test/boot/lib/tap-parity.sh` (đọc-only, fail-closed) khẳng định `soak.TAP_TESTS == run.EXPECT_TESTS == kbin.TAP_TESTS` (`kbin-{aarch64,x86_64}/src/main.rs`) + ca âm lệch ±1 ⇒ ĐỎ; (3) **Probe fail-fast cho `soak-aarch64.sh`**: chạy `monitor-sendkey.py` + `check_boot_log "$PROBE_LOG" "$WANT_BUILD" "$WANT_TEXT"` trên boot dò (đồng khuôn `soak-x86_64.sh:1403-1408`) ⇒ `exit 3` sau ~2 s thay vì đốt 971 s; kèm bằng chứng đỏ-trước (`--smoke 1 1` ĐỎ trước vá, `check_boot_log` XANH sau vá); 0 sửa `spec/`; T2 PASS. |
```

**`QUEUE.md:478`** — `KT-417b` · thay nguyên dòng:

```text
| KT-417b | **P0** **Chặng 2 (W7b) — Hợp nhất 1 Cây Cap/MDB bước 1: Đóng gói `Cap` (`56→32 B`, −12 KiB) + Dời `plat-common::obj` (2.424 LOC) → `vifast-core/src/vm/` + Đóng `Q6`** | KT-417 | 2,5 h | DONE | **DONE 2026-11-26 (merge `bb3c144e`; mã `0a247faa`; nhật ký `41dc3308` — T1 lượt 2 trên **cây sạch** PASS `82/68/14/0`; `size_of::<Cap>() == 32 B`, thu hồi 12.288 B, `vifast-core` 139 host test; T2 PASS 81/81 727,1 s @`82502f7d`)**: **(APPROVED 2026-11-26 Agent 0 `D2-262` — MỞ `READY` NGAY ở W7b Slot 1, 0 % chồng tệp `KT-418c`/`KT-408d`; tách bước 2 sang `KT-417d` giữ ≤ 3 h)** (1) **Đóng gói `Cap` (`objmeta.rs`, khuôn `KT-312` `D2-84`)**: thay `parent: Option<CapId>` bằng `parent_raw: u32` (`0==None`), `slot: Option<SlotRef>` bằng `slot_cnode: u32` + `slot_idx: u16` (`≤ MAX_SLOT_WORDS=4096`), `badge: Option<u64>` bằng `badge_val: u64` + `flags: u8` ⇒ `size_of::<Cap>() == 32 B`, thu hồi **12.288 B** trên `StaticStore` (**24 KiB** trên `UntypedStore`), pin `size_of::<Cap>() == 32` + ca biên `slot=4095`/`badge=Some(0)/Some(u64::MAX)/None` trong `footprint.rs`; hạ ghim cỡ chính xác xuống mức mới (khuôn `KT-405`: `budget-common.sh:54,303`, `ktest.sh:600,613,645,829`, `run-{aarch64,x86_64}.sh`, **KHÔNG nới trần `184.832 B` KT-312 hay pin `196.608 B` D2-84**); (2) **Dời `plat-common/src/obj/{mod,vspace,check,arena}.rs` (2.424 LOC) → `vifast-core/src/vm/`** (thêm dep thuần acyclic `vifast-pte` + `vifast-alloc` vào `vifast-core/Cargo.toml`, giữ `#![forbid(unsafe_code)]`, tránh trùng tên `MAX_OBJECTS` với `machine.rs`, giữ shim re-export mỏng ở `plat-common::obj` & `aobj` cho 33 caller không đổi hành vi, cập nhật tạm `source` của `[limits.plat]` trong `spec/kernel.toml`); (3) **Đóng `Q6`**: assert `delta.mutated == 0 && delta.rotated == 0 && delta.bound == 0 && delta.unbound == 0` trong `ktest_inv.rs::ok18` + thêm 2 đột biến `core-host` trong `v2/test/mutants.toml` cho `Mutate`/`Rotate` & `Bind`/`Unbind` ⇒ KILLED; T2 PASS. |
```

**`QUEUE.md:480`** — `KT-417c` · thay nguyên dòng:

```text
| KT-417c | **P1** **Chặng 2 (W8b) — Bật `x86_64` `syscall`/`sysret` (`swapgs` + GDT `planned`, giữ `int 0x80` & `7C-r13`) + Phục hồi phủ preempt CPL3** | KT-417, KT-418c | 3 h | DONE | **DONE 2026-11-28 (merge `075b0639` + hotfix merge `82502f7d`; mã `a5541601` + hotfix `c73eb77a`; nhật ký `aca01bda`, `6705c1e4` → `JOURNAL/kt417c-hotfix.md`; **T1 PASS** `82/68/14/0` 668,8 s (11:16:29–11:27:39 UTC) @`c73eb77a`; **T2 PASS 81/81** 727,1 s (11:28:25–11:40:33 UTC) @`82502f7d`; X1/X2/X3 vá theo `D2-264`; audit đối kháng **0 BLOCKER** — 3 MAJOR còn lại là khoảng trống *bằng chứng chạy end-to-end*, không phải mã chưa vá)**: **(D2-261/D2-262 — chạy song song `KT-419` ở W8b, chỉ chạm `spec/kernel.toml` không đụng `spec/abi.toml`)** (1) Đổi GDT `plat-x86_64/src/gdt.rs` sang thứ tự `planned` (`user_data=0x18`, `user_code=0x20`, `STAR[63:48]=0x10`) + cài đường `syscall`/`sysret` (`swapgs` + `LSTAR`/`STAR`/`SFMASK` trên `asm/syscall.rs`, giữ `7C-r13` `return_message_info = r13`); (2) **giữ song song cổng `int 0x80`** để giữ `min_compat = 1` (`mut-no-int80` còn ĐỎ) + thêm kiểm TAP/ca âm cho `syscall`/`sysret`; (3) **đóng nợ `KT-413c` §6.1**: phục hồi bằng chứng & TAP preempt thật khi đang thực thi ở ring-3 (`# sched: cpl3 preempted=1 resumed=1 cs=0x23`); (4) lật `sysret_ready = true` trong `spec/kernel.toml:126` + cập nhật `check_kernel.py:398-399` & `--selftest` + cập nhật selector user trong `run-x86_64.sh`/`soak-x86_64.sh`; 0 chạm `asm/fpu.rs`; T2 PASS. |
```

**`QUEUE.md:483`** — `KT-418c` · thay nguyên dòng:

```text
| KT-418c | **P1** **Chặng 2 (W7b) — Điều tra & vá lỗi thật `-cpu max` `VSpace` isolation (`rflags=0x10202 [TF=1]`) + kịch bản hardening `-cpu max` (`SMEP`/`SMAP`/`PAN`) + di động host `a64 fpu.rs`** | KT-413c, KT-418b | 2 h | DONE | **DONE 2026-11-26 (merge `b76e3bc8`; mã `f237cd34`; nhật ký `6a4a986a` — T1 trên **cây sạch** PASS `82/68/14/0`; vá `#PF` cô lập `VSpace` trên `-cpu max` + `hardening-max.sh` 4 chân + `a64 fpu.rs` di động host; T2 PASS 81/81 727,1 s @`82502f7d`)**: **(APPROVED 2026-11-26 Agent 0 `D2-261`/`D2-262` — MỞ `READY` NGAY ở W7b Slot 2, 0 % chồng tệp `KT-417b`/`KT-408d`; TỪ CHỐI nới ratchet `B1.1`)** (1) Điều tra & vá lỗi có sẵn lộ ở `KT-418b` §6.4: ảnh ktest `x86_64` trên `-cpu max` dừng ở phép kiểm cô lập `VSpace` (`# exc: vec=14 err=0x6 rflags=0x10202 [TF=1]`) thay vì khớp hook `# vspace: cross fault`; viết test đỏ-trước + vá nguyên nhân gốc; (2) dựng kịch bản độc lập `v2/test/boot/hardening-max.sh` kiểm ca âm phần cứng thật `SMEP`/`SMAP` (`x86_64 -cpu max`) & `PAN` (`aarch64 -cpu max`) giữ nguyên 100 % baseline `run-{x86_64,aarch64}.sh` (KHÔNG sửa 2 runner cũ để 0 chồng tệp với `KT-417b`); (3) bọc `#[cfg(target_os = "none")]` cho inline asm trong `vifast-plat-aarch64/src/asm/fpu.rs` để host test biên dịch di động cả trên host `x86_64` lẫn `aarch64` (`D2-262`); 0 sửa `spec/`, 0 nới `b1.1-ratchet.toml`; T2 PASS. |
```

**`QUEUE.md:486`** — `KT-420` · thay nguyên dòng:

```text
| KT-420 | **P0** **Chặng 3 (W8a) — Kiến trúc User Space phần 1: 6 `.vidl` + `vifast-idl` + dịch vụ `namesrv` + trait `PageSource` `vifast-rt`** | KT-417b, KT-418b | 3 h | DONE | **DONE 2026-11-27 (merge `b8bcbd74`; mã `0cfc23bb`; nhật ký `8a749e28` — T1 trên **cây sạch** PASS `82/68/14/0`; 6 `.vidl` + `vifast-idl` typed + `namesrv` + trait `PageSource`, **0 chạm** `v2/kernel/` và `spec/{abi,kernel}.toml`; T2 PASS 81/81 727,1 s @`82502f7d`)**: **(D2-262 — tách phần `v2/kernel/` & cửa sổ `spec/abi.toml` sang `KT-420b` để giữ ≤ 3 h & chạy song song 100 % khác cây thư mục với `KT-417d` ở W8a!)** (1) Hoàn thiện đủ 6 tệp `.vidl` chuẩn trong `spec/protocols/` (`ns`, `proc`, `fs`, `blk`, `socket`, `term`) và sinh stub Rust typed `vifast-idl` (`vifast-idl --check` PASS); (2) dựng crate dịch vụ `namesrv` trong `v2/user/` (đăng ký/tra cứu endpoint theo tên qua `ns.vidl` + host test đầy đủ); (3) tách sạch trait cấp trang `PageSource` cho allocator `vifast-rt` (sẵn sàng cắm `Frame`/`Untyped` thật ở `KT-420b`) + host test; **0 chạm `v2/kernel/` lẫn `spec/{abi,kernel}.toml`**; T2 PASS. |
```

### §1.B — dòng 479 `KT-417d` (giữ `BLOCKED`, ghi `3/5 DONE`)

Vì sao giữ `BLOCKED`: `QUEUE.md:5` + `AGENTS.md` §10 chỉ cho **4** trạng thái, **không có** `PARTIAL`; đổi sang `READY` sẽ khiến `vifast-agentctl next` trả một task **đã dùng hết cửa sổ `spec/`** và đang chờ quyết định tách. Ghi tiến độ trong ô tiêu chí là cách duy nhất không phá DAG (`KT-419/419b/420b` vẫn đúng là chưa mở).

**`QUEUE.md:479`** — `KT-417d` · thay nguyên dòng:

```text
| KT-417d | **P0** **Chặng 2 (W8a) — Hợp nhất 1 Cây Cap/MDB bước 2: Bảng VM vào `KernelStore` + `ArchVmOps` + `PageTable.{Map,Unmap}` + Xoá `[limits.plat]` & `core_foreign.rs` + Đóng `Q4` F6** | KT-417b | 3 h | BLOCKED | **PARTIAL 3/5 — phần 1/2 DONE `60aba561` (mã `2fc15e0e`) + hotfix F1/F2/F3 DONE `a775affe` (mã `e65e42a4`), T1 `82/68/14/0` cả 2 lượt; phần 2/2 (`ArchVmOps`/xoá `core_foreign.rs`/`Retype` VM ở core + chứng minh `Revoke` 4 loại) CHƯA làm — chờ Agent 0 duyệt tách `KT-417e/f/g/h` (`/tmp/briefs/approval-packet-kt417e.md`, 10,5 h `[ƯL]`); giữ `BLOCKED` vì QUEUE không có trạng thái `PARTIAL`** — **(D2-262 — ĐƯỜNG GĂNG mở khoá Chặng 3; chạy song song `KT-420`, chủ sở hữu duy nhất cửa sổ `spec/{abi,kernel}.toml` ở W8a)** (1) **Đưa bảng VM làm kiểu liên kết hạng nhất của `KernelStore` (`store.rs`)** — `VmMappings`/`VmFlushes`/`VmUntypeds`(`FrameArena`)/`VmObjMeta` đa hình cho cả `StaticStore` (`64/16/4`) lẫn `UntypedStore` (`256/32/16`) — **CẤM global `static` `.corespace`**, giữ 100 % `Copy` + `forbid(unsafe_code)` + `size_of::<Machine<StaticStore, TestObserver>>()` ≤ `184.832 B` (~176 KiB nhờ 12 KiB thu hồi từ `KT-417b`, pin `footprint.rs` đầy đủ padding ≤ 8 B); (2) hợp nhất `Frame`/`PageTable`/`VSpace` vào 1 cây Capability/MDB duy nhất của `Machine` qua trait `ArchVmOps`, **xoá bảng thứ hai + shim `plat-common::obj` + `core_foreign.rs`**, cập nhật toàn bộ caller (`mem_bridge`, `user_run`, `ktest_*`, `x86_64` `shellpath`/`coretest`); (3) cài `PageTable.{Map,Unmap}` thật (gỡ `unsupported` trong `spec/abi.toml` + `check_abi.py`), **xoá `[limits.plat]` khỏi `spec/kernel.toml` + sửa `check_kernel.py:277` & `--selftest`**; (4) **đóng `Q4` F6**: bổ sung `errors = [...]` tường minh (tập con hợp lệ của `[error]`) cho 13 `[method_signature]` mới ở `spec/abi.toml:348-411` + kiểm máy & ca âm trong `check_abi.py`; (5) chứng minh `CNode.Revoke` 1 `Untyped` thu hồi đồng thời `TCB` + `VSpace` + `Frame` + `PageTable` (`mut-state-before-cap` còn KILLED); T2 PASS. |
```

### §1.C — dòng 578 (ghi chú vận hành, danh sách task)

**`QUEUE.md:578`** — thay nguyên dòng:

```text
- Task **`READY`** hiện tại (**0** — W7b + W8a + W8b đã đóng sạch theo `D2-264`): không còn slot mở. Còn lại **9** task `BLOCKED`: 8 thuần theo DAG (`KT-419`, `KT-419b`, `KT-420b`, `KT-421`, `KT-422`, `KT-423`, `KT-423b`, `KT-424`) + **`KT-417d` phần 2/2** đang chờ Agent 0 duyệt tách thành `KT-417e/f/g/h`. Đếm: 389 task = **380 `DONE`** + 9 `BLOCKED`/`PARTIAL` (`KT-417d` = `3/5`).
```

---

## §2 `KT-417c` chốt `DONE` + mục `D2-264`

`grep -c "D2-264" v2/agent/DECISIONS.md` = **0** ⇒ **264 còn trống**, dùng được (mục cao nhất hiện có là `D2-263`, `DECISIONS.md:673`).

### §2.A — dòng `APPROVED` (dán **sau dòng 492**, tức sau dòng cuối bảng `KT-424`, và **trước** khối `APPROVED … D2-263` ở dòng 494 — đúng thứ tự mới-nhất-trước đang dùng)

```text
**APPROVED 2026-11-28 Agent 0 (D2-264 — chốt `KT-417c` = `DONE` sau hotfix X1/X2/X3, đóng trọn W7b/W8a/W8b, viết lại DoD W9 thành đo được)**:
(1) **`KT-417c` = `DONE`** trên `main` @`82502f7d` (merge `task/kt417c-fix`): mã `a5541601` (`075b0639`) + hotfix `c73eb77a` (`6705c1e4`, merge `82502f7d`); **T1 PASS** `82/68/14/0` 668,8 s (11:16:29–11:27:39 UTC) @`c73eb77a`; **T2 PASS 81/81** 727,1 s (11:28:25–11:40:33 UTC) @`82502f7d` (`# gate-wall 727102 ms`). X1 (frame 136 B của cửa `syscall`), X2 (khép cặp `swapgs` trước `iretq`), X3 (đo CPL3 bằng `RIP` thay `cs` hằng) đã vá ở tầng mã; audit đối kháng `/tmp/audits/hotfix-review.md`: **0 BLOCKER**, 3 MAJOR còn lại là **khoảng trống bằng chứng chạy**, không phải mã chưa vá.
(2) **Phạm vi `DONE` (ghi rõ để không ai đọc quá)**: `DONE` nghĩa là "đã vá + T2 xanh", **KHÔNG** nghĩa "đã có bằng chứng chạy end-to-end". Ba khoảng trống của `hotfix-review.md` **giữ nguyên trạng thái nợ**: **MAJOR-1** (`ug=` là hằng 0 theo cấu trúc và in **trước** lần `Exit` đầu tiên ⇒ không phát hiện được X2); **MAJOR-2** (nhánh `Switched` từ **cửa `syscall`** chưa từng chạy trong boot nào — pha core chỉ dùng `int 0x80`); **MAJOR-3** ("chuẩn hoá trước **mọi** `iretq`" sai: còn `asm/syscall.rs:84`, `asm/trap.rs:299`, và `gs_is_kernel` là dead code).
(3) **Chủ sở hữu 3 khoảng trống trên**: gắn vào **`KT-424` mục #0** (kiểm toán chốt chặng, đã phụ thuộc `KT-417c`), **không** cấp ID mới; điều kiện đóng **đo được**: 1 boot `x86_64` in `# core resume … via=syscall` + `gs_in=`/`gs_out=` ở **cả hai** cửa, có ca âm cho dòng `via=syscall`.
(4) **Đồng bộ sổ (Agent 0 dán §1)**: `KT-417b`, `KT-418c`, `KT-408d`, `KT-408e`, `KT-420`, `KT-417c` → `DONE`; `KT-417d` giữ `BLOCKED` + ghi `PARTIAL 3/5`; dòng 578 → `0 READY / 9 BLOCKED / 380 DONE`.
(5) **DoD W9 viết lại (đo được)**: `driver_files` **23→15→10→0** theo từng task + khoá mới `kernel_service_files` = **2 mục đóng băng** `{loop_svc.rs, mem_svc.rs}` (hợp hai khoá luôn **⊆ 23 mục ban đầu** — mục "mới" = 0 ở mọi bước ⇒ không nới ratchet); bỏ nhãn "0 % chồng tệp", thay bằng **"chồng tệp có chủ, đo bằng `git diff --name-only`"**; `Frame.{MapIo,UnmapIo}`/`IrqControl`/`IrqHandler`/`IOSpace` là **tiền đề cứng** từ `KT-419`/`KT-419b`, không phải DoD của W9.
(6) **Sửa header `DECISIONS.md:28`**: `Phần II — D2-01..D2-127` → `Phần II — D2-01..D2-264` (docs-audit F10).
```

### §2.B — mục `D2-264` (thêm vào **cuối `DECISIONS.md`**, ngay sau khối `D2-263` kết thúc ở dòng 684)

```text
- **D2-264** **Agent 0 — Chốt `KT-417c` = `DONE` (W8b, `syscall`/`sysret` x86_64) sau hotfix X1/X2/X3, chốt phạm vi bằng chứng end-to-end còn nợ, đồng bộ QUEUE 6 task, viết lại DoD W9 thành đo được** (2026-11-28 21:00 JST / `12:00 UTC`). Căn cứ trực tiếp: `v2/agent/JOURNAL/kt417c-hotfix.md`, `JOURNAL/kt417c-fix/{t0-hosttests,t0-clippy-check,boot-1-hotfix,red-frame-gs,red-check-syscall,red-neg-check-x3}.log`, `/tmp/t1-kt417c-hotfix-c73eb77a.log`, `/tmp/t2-kt417c-hotfix-82502f7d.log`, `/tmp/audits/hotfix-review.md`, `/tmp/audits/docs-audit.md` F3/F6:
  1. **Chốt `KT-417c` = `DONE`** (không phải `BLOCKED`): mã `a5541601` + hotfix `c73eb77a`, merge `075b0639` + `82502f7d`; T1 PASS `registered=82 ran=68 skipped=14 failed=0` 668,8 s; **T2 PASS 81/81** 727,1 s (`11:28:25–11:40:33 UTC`). Ba lỗi MAJOR X1/X2/X3 (audit mã W8b) đã vá ở tầng mã và qua audit đối kháng với **0 BLOCKER**; việc còn lại là **đo**, không phải sửa.
  2. **Phạm vi `DONE` — NÊU RÕ ĐỂ CHỐNG ĐỌC QUÁ**: `DONE` = "X1/X2/X3 đã vá + T2 trên ứng viên tích hợp đã xanh". `DONE` **KHÔNG** bao gồm 3 khoảng trống bằng chứng của `hotfix-review.md`: (a) **MAJOR-1** — bằng chứng `ug=` là **hằng 0 theo cấu trúc** (`enter_user` chỉ ghi 0 hoặc halt) và được in **trước** lần `Exit` đầu tiên (`boot-1-hotfix.log:69` vs `:72`) ⇒ boot này vẫn xanh y hệt nếu X2 quay lại; (b) **MAJOR-2** — nhánh `resumed_rip_from` chỉ chạy khi `Switched` đến từ **cửa `syscall`**; boot duy nhất có `syscall` **không** `Switched`, pha core (`coretest.rs:956-1046`) dùng `int 0x80` ⇒ **nhánh X1 chưa từng được thực thi**, và **không test host nào** import `vifast-kbin-x86_64`; (c) **MAJOR-3** — tuyên bố "nơi **duy nhất** mọi `iretq` về CPL3" là sai: còn `asm/syscall.rs:84` (`int 0x80`) và `asm/trap.rs:299` (`vifast_isr_common`), cộng `sysretq` `asm/syscall.rs:237`; `syscall_fast.rs:177` `gs_is_kernel` là **dead code**.
  3. **Chủ sở hữu 3 khoảng trống (không cấp ID mới, giữ regex `^KT-[0-9]+[a-z]?$`)**: gắn vào **`KT-424` mục #0** — trước khi kiểm toán chốt chặng, phải có **1 boot `x86_64` chứng minh nhánh đã chạy**: `# core resume … via=syscall`, `gs_in=` đo **tại cửa vào** cho cả hai cửa, `gs_out=` **sau** một `Exit`, và **≥1 ca âm** trên runner đột biến đúng dòng `via=syscall` (đóng MAJOR-1/2); cộng gọi `gs_is_kernel(hart, gs_base_in)` ở đầu `syscall_dispatch` (đóng MAJOR-3). Nếu Agent 0 muốn ID riêng, mã hợp lệ còn trống là **`KT-417i`** (1 cái, 2 h) — khi đó phải thêm **dòng QUEUE trước** rồi mới nhắc trong STATE/JOURNAL (docs-audit F12).
  4. **DoD W9 (`KT-421/422/423/423b/424`) — VIẾT LẠI THÀNH ĐO ĐƯỢC** (thay văn bản bất khả thi ở `KE-HOACH-KIEN-TRUC.md:141,144,185-189`): (a) **số đếm máy thay số mơ hồ**: `driver_files` **23→15** (`KT-421`) →**10** (`KT-422`) →**2** (`KT-423`), rồi 2 mục `{loop_svc.rs, mem_svc.rs}` (dịch vụ kernel, **không phải driver**) chuyển sang khoá ratchet mới **`kernel_service_files`** đóng băng đúng 2 mục ⇒ `driver_files = 0`, `kernel_service_files = 2`, **hợp hai khoá luôn ⊆ 23 mục ban đầu** (mục "mới" = 0 ở mọi bước ⇒ đây là **phân loại**, không phải nới ratchet; `KT-423b` giữ cứng `driver_files = 0`); mỗi bước nghiệm thu bằng **đúng dòng** `# B1.1 ratchet: dep_edges=<e> (đóng băng <b>, mới 0) driver_files=<n> (đóng băng <c>, mới 0) stale=0` do `v2/tools/check-api-boundary.py:1200` in (hôm nay: `dep_edges=3 … driver_files=23 (đóng băng 23, mới 0) stale=0`). (b) **bỏ nhãn "0 % chồng tệp"** (tự phủ định vì `KE-HOACH:141` ghi chính Lead dọn `Cargo.toml`): thay bằng **"chồng tệp có chủ, đo bằng `git diff --name-only`"** — mỗi slot khai báo tập tệp trong JOURNAL, DoD = `git diff --name-only <base>..<task>` ⊆ tập khai báo và **0** dòng khớp `^(v2/user/Cargo\.(toml|lock)|v2/test/baseline/b1\.1-ratchet\.toml)$` (3 tệp dùng chung do **Lead** sở hữu duy nhất lúc merge). (c) **tiền đề cứng**: `Frame.{MapIo,UnmapIo}` (`spec/abi.toml:242`) + `IrqControl`/`IrqHandler`/`IOSpace` (`spec/abi.toml:245-247`) hôm nay vẫn `unsupported = true` ⇒ W9 **không thể** bắt đầu trước `KT-419`/`KT-419b`; DoD W9 **chỉ tiêu thụ**, không cài lại ABI. (d) **bổ sung ràng buộc `unsafe`**: driver U-mode phải giữ `#![forbid(unsafe_code)]` trong crate driver (khuôn `v2/user/namesrv/lib.rs:4`, `v2/user/rt/lib.rs:4`) và mọi truy cập MMIO đi qua **một** API an toàn của `vifast-rt` ⇒ đo bằng `grep -rc "unsafe" v2/user/<crate-driver>/src == 0`; nếu chọn API cần `unsafe` thì phải ghi rõ + `D2` riêng, **không** im lặng nới `forbid`.
  5. **Đồng bộ QUEUE (6 task + 1 ghi chú)**: lật `KT-417b` (`bb3c144e`), `KT-418c` (`b76e3bc8`), `KT-408d` (`1d15b081`), `KT-408e` (`fff6cf91`), `KT-420` (`b8bcbd74`), `KT-417c` (`075b0639`+`82502f7d`) sang `DONE`; `KT-417d` **giữ `BLOCKED`** + ghi `PARTIAL 3/5` (p1 `60aba561`/`2fc15e0e`, hotfix `a775affe`/`e65e42a4`; p2/2 chờ duyệt tách `KT-417e/f/g/h`) — **lý do**: `QUEUE.md:5` + `AGENTS.md` §10 chỉ có 4 trạng thái, **không có `PARTIAL`**, và đổi sang `READY` sẽ khiến `agentctl next` trả task đã dùng hết cửa sổ `spec/`. Sau khi dán: **389 = 380 `DONE` + 9 `BLOCKED`**, `READY` = **0**.
  6. **Chưa cấp `KT-417e/f/g/h`** (đề xuất ở `/tmp/briefs/approval-packet-kt417e.md`, 10,5 h `[ƯL]`): phải có **dòng QUEUE trước**, rồi mới được nhắc trong `STATE.md`/`JOURNAL` (docs-audit F12). Kèm sửa header `DECISIONS.md:28` thành `D2-01..D2-264` (docs-audit F10).
```

### §2.C — 5 câu hỏi Agent 0 phải quyết (đừng để trôi)

| # | Câu hỏi | Khuyến nghị của sub-agent |
|---:|---|---|
| 1 | `KT-417c` = `DONE` **có điều kiện** (3 khoảng trống bằng chứng gắn `KT-424` #0) hay **để mở** tới khi đóng MAJOR-1/2/3? | `DONE` có điều kiện — mã đã vá, T2 xanh; giữ mở chỉ làm `main` mang trạng thái "đã merge nhưng chưa DONE" lần thứ hai |
| 2 | Tách `KT-417d` p2/2 thành `KT-417e/f/g/h` (`/tmp/briefs/approval-packet-kt417e.md`) + `VmObjMeta` = **bảng rời**? | Duyệt — 10,5 h `[ƯL]`, 0 chạm `spec/`, chạy được `e` ∥ `c` |
| 3 | Duyệt **DoD W9 viết lại** (§3) theo **Phương án B** (khoá `kernel_service_files`) hay **A** (`KT-423c` +2 h)? | **B** — 0 task mới, union 23 mục bất biến, `KT-424` mới đạt `driver_files = 0` một cách đo được |
| 4 | Cập nhật `KE-HOACH-KIEN-TRUC.md` (docs-audit F4: đóng băng ở `D2-263`; thực tế 38/47 DONE, 0 READY, 8 BLOCKED + 1 PARTIAL)? | Có — chỉ Agent 0 được ban hành kế hoạch (`D2-259`) |
| 5 | Sửa tiêu đề `DECISIONS.md:28` `D2-01..D2-127` → `..D2-264`? (docs-audit F10) | Có — 1 dòng, cùng lần dán `D2-264` |

---

## §3 DoD W9 (`KT-421..KT-423`) — viết lại thành **đo được**

### §3.0 Ba điểm hỏng của DoD hiện tại (`/tmp/audits/docs-audit.md` F6) + 2 điểm em bổ sung

| # | Văn bản hiện tại | Vì sao không đo được / bất khả thi | Thay bằng |
|---:|---|---|---|
| 1 | `KT-423`: “siết ratchet `driver_files` về **0** vĩnh viễn” (`KE-HOACH-KIEN-TRUC.md:144`) | `driver_files` có **23** mục (`b1.1-ratchet.toml:26-50`), trong đó **2 mục không task W9 nào phủ**: `vifast-kbin-riscv64/src/{loop_svc,mem_svc}.rs` (dịch vụ kernel, khớp mẫu `*_svc`); mục thứ 3 `vifast-plat-riscv64/src/virtio_console.rs` chỉ lọt nếu đọc “gỡ sạch `virtio_*`” theo nghĩa rộng ⇒ đạt 0 là **bất khả thi** trừ khi nới ratchet (cấm) | **số đếm máy theo từng bước**: `driver_files` 23→15→10→2, rồi **phân loại** 2 mục `*_svc` sang khoá mới `kernel_service_files` ⇒ 0; nghiệm thu bằng đúng dòng `# B1.1 ratchet: … (đóng băng <b>, mới 0) … stale=0` |
| 2 | W9 “**0 % chồng tệp**” (`KE-HOACH-KIEN-TRUC.md:141` — cùng dòng lại ghi “Lead dọn `Cargo.toml` & ratchet lúc merge”) | **Tự phủ định**: 3 slot song song cùng ghi `v2/user/Cargo.toml`, `v2/user/Cargo.lock`, `v2/test/baseline/b1.1-ratchet.toml` ⇒ chồng tệp **thật**; nhãn 0 % không kiểm được bằng máy | **“chồng tệp có chủ, đo bằng `git diff --name-only`”**: mỗi slot khai báo tập tệp trong JOURNAL; DoD = `git diff --name-only <base>..<task>` ⊆ tập khai báo **và** 0 dòng khớp 3 tệp dùng chung (Lead sở hữu duy nhất lúc merge) |
| 3 | `KT-421/422/423/423b` đòi `Frame.MapIo` + `IrqHandler` + `IOSpace` như **DoD của chính task** (`KE-HOACH:185-188`) | Hạ tầng đó **chưa tồn tại**: `spec/abi.toml:242` `Frame.{MapIo,UnmapIo}` và `:245-247` `IrqControl`/`IrqHandler`/`IOSpace` vẫn `unsupported = true`; nó thuộc `KT-419`/`KT-419b` (đang `BLOCKED`) ⇒ đặt làm DoD W9 là **đo cái chưa có** | ghi thành **TIỀN ĐỀ CỨNG fail-closed** (thiếu ⇒ task ở `BLOCKED`), DoD W9 chỉ **tiêu thụ** hạ tầng đó |
| 4 *(bổ sung)* | chưa có ràng buộc `unsafe` cho driver U-mode | driver cần MMIO ⇒ **không thể** vừa `#![forbid(unsafe_code)]` vừa tự nhúng `asm`/đọc thanh ghi; im lặng nới `forbid` là mất bất biến (khuôn hiện có: `v2/user/namesrv/lib.rs:4`, `v2/user/rt/lib.rs:4`) | driver U-mode **giữ** `#![forbid(unsafe_code)]`; mọi truy cập MMIO qua **một** API an toàn của `vifast-rt`; đo `grep -rc "unsafe" v2/user/<crate>/src` = **0** (nếu cần `unsafe` thì phải có `D2` riêng, không im lặng) |
| 5 *(bổ sung)* | “siết ratchet” ở mỗi task nhưng `b1.1-ratchet.toml` là **một** tệp | 3 slot song song không thể cùng sửa 1 tệp mà vẫn “0 chồng tệp” | tách 2 mốc đo: **trên nhánh** (`stale` = đúng số mục của mình), **sau merge** (`stale=0`, do Lead hạ danh sách) — cả hai đều in ra từ cùng một bước cổng |

> **Đính chính so với bản giao việc**: `/tmp/audits/docs-audit.md` **không** nói “còn 9 driver” và **không** nói `unsafe` — con số là **23** mục, và điểm thứ ba là **tiền đề `MapIo`/`Irq*`/`IOSpace`**. Hai mục 4–5 ở trên là **bổ sung của sub-agent**, đã ghi rõ nguồn.

### §3.1 Số học ratchet (đo được, không phải ước lượng)

Hôm nay, đúng dòng trong log T2 `82502f7d` (`/tmp/t2-kt417c-hotfix-82502f7d.log:1039-1040`):

```text
# B1.1 ratchet: dep_edges=3 (đóng băng 3, mới 0) driver_files=23 (đóng băng 23, mới 0) stale=0
# B1.1: XANH (0 vi phạm, ratchet một chiều)
```

| Bước | `dep_edges` | `driver_files` | `kernel_service_files` | Xoá đúng các mục |
|---|---:|---:|---:|---|
| hôm nay (`82502f7d`) | 3 | 23 | — | — |
| `KT-421` | **0** | **15** | — | `kbin-riscv64/src/net_svc.rs`; `plat-{a64,rv}/src/tcp.rs`; `plat-{a64,rv,x64}/src/virtio_net.rs`; `plat-a64/tests/{tcp,virtio_net}.rs` (**8**) |
| `KT-422` | 0 | **10** | — | `plat-{a64,rv,x64}/src/virtio_blk.rs`; `plat-{a64,x64}/tests/virtio_blk.rs` (**5**) |
| `KT-423` | 0 | **2** | — | `plat-{a64,rv,x64}/src/virtio_gpu.rs`; `plat-{a64,rv}/src/virtio_input.rs`; `plat-rv/src/virtio_console.rs`; `plat-a64/tests/{virtio_gpu,virtio_input}.rs` (**8**) |
| `KT-423` (phân loại, Phương án B) | 0 | **0** | **2** | chuyển `kbin-riscv64/src/{loop_svc,mem_svc}.rs` sang khoá mới (vẫn đóng băng, **mục “mới” = 0**) |
| `KT-423b` / `KT-424` | 0 | **0** | **2** | không được tăng (ca âm bắt buộc) |

Tổng kiểm: **8 + 5 + 8 + 2 = 23** ✔ (khớp `b1.1-ratchet.toml:26-50`, đếm bằng máy).

### §3.2 Phương án B (khuyến nghị) vs A — chỉ khác nhau ở 2 mục `*_svc`

- **B (khuyến nghị, 0 h thêm)**: 2 tệp `loop_svc.rs`/`mem_svc.rs` là **dịch vụ kernel** (tự khai trong header: “dịch vụ `loop <n>` của **kernel**”, “`MemSvc`: dịch vụ bộ nhớ của **kernel**”), **không phải driver** ⇒ tách khoá `kernel_service_files` (đóng băng đúng 2 mục). Cần: `check-api-boundary.py` thêm 1 `RATCHET_KEYS` + 1 nhánh in; **Agent 0 duyệt 1 lần** trong `D2-264` mục 4.
- **A (thay thế, +2 h)**: mở **`KT-423c`** (mã hợp lệ, `README` regex) tách `loop_svc`/`mem_svc` ra User Space theo `loop.vidl`/`meminfo.vidl` — khi đó `driver_files = 0` với **đúng 23 mục ban đầu**, không cần khoá mới. Nếu chọn A, thay trong 3 dòng dán: `kernel_service_files` → bỏ, và `KT-423` giữ `driver_files=2` thay vì `0`; thêm 1 dòng `KT-423c` (`BLOCKED`, 2 h, phụ thuộc `KT-420b`).

### §3.3 Khối dán — thay **nguyên dòng** (giữ 6 cột; trạng thái giữ `BLOCKED`, chỉ đổi ô tiêu chí)

> Mọi lệnh trong ô tiêu chí đều viết **không có ký tự `|`** để dán vào bảng không vỡ cột (dùng pathspec `-- <tệp>` và nhiều `-e` thay cho pipe).

**`QUEUE.md:488`** — `KT-421` · thay nguyên dòng:

```text
| KT-421 | **P0** **Chặng 3 (W9) — `B1.2` bước 1: Tách `netsrv` + `virtio-net` 100 % ra User Space (`v2/user/`) & Gỡ `vifast-net` khỏi kernel** | KT-419b, KT-420b, KT-406 | 3 h | BLOCKED | (D2-259/D2-261/D2-262 + **`D2-264` mục 4 — DoD ĐO ĐƯỢC**) **TIỀN ĐỀ CỨNG (fail-closed — thiếu thì giữ `BLOCKED`, không đốt 3 h)**: `Frame.{MapIo,UnmapIo}` phải đã gỡ `unsupported` (hôm nay `spec/abi.toml:242` còn `unsupported = true, install_at = "Chu-ky-3"`) và `IrqControl`(12)/`IrqHandler`(13)/`IOSpace`(14) phải hết `unsupported` (`spec/abi.toml:245-247`) — do **`KT-419`/`KT-419b`** cài; `KT-421` chỉ TIÊU THỤ, không mở lại ABI. Chạy driver `virtio-net` (`Frame.MapIo` + `IrqHandler` + `IOSpace`) + ngăn xếp `vifast-net` (DHCP/ARP/ICMP/UDP/DNS/TCP đầy đủ `socket.vidl`, `ping`/`ifconfig`) 100 % ở User Space; xoá sạch `net_svc.rs`, `proc_net.rs`, `virtio_net.rs`, `tcp.rs` (kèm nhánh chết `product-net` ở `riscv64/build.rs:206`) và gỡ dep `vifast-net` khỏi cả 3 `plat-*`/`kbin-*`. **DoD đo được (thay nhãn "0 % chồng tệp" và số mơ hồ)**: (1) **trên nhánh** `v2 check` bước `step_api_boundary` (`v2/bin/lib/steps_check.sh:179`) in **đúng** dòng `# B1.1 ratchet: dep_edges=0 (đóng băng 3, mới 0) driver_files=15 (đóng băng 23, mới 0) stale=8` — `driver_files` **23→15**, xoá đúng 8 mục: `vifast-kbin-riscv64/src/net_svc.rs`, `vifast-plat-{aarch64,riscv64}/src/tcp.rs`, `vifast-plat-{aarch64,riscv64,x86_64}/src/virtio_net.rs`, `vifast-plat-aarch64/tests/{tcp,virtio_net}.rs`; `dep_edges` **3→0**; dán nguyên dòng log vào `JOURNAL/`; (2) **sau merge** (Lead hạ đúng 8 mục + 3 cạnh khỏi `b1.1-ratchet.toml`): `# B1.1 ratchet: dep_edges=0 (đóng băng 0, mới 0) driver_files=15 (đóng băng 15, mới 0) stale=0` ⇒ **`stale=0` là điều kiện nghiệm thu**; (3) **ca âm**: tạo lại `vifast-plat-x86_64/src/virtio_net.rs` ⇒ cổng ĐỎ `B1.1-DRIVER` (fail-closed, 0 xanh oan); (4) `test ! -e` đủ 8 tệp; (5) **chồng tệp đo bằng máy**: `git diff --name-only <base>..<task>` ⊆ tập tệp khai báo trong JOURNAL, và `git diff --name-only <base>..<task> -- v2/user/Cargo.toml v2/user/Cargo.lock v2/test/baseline/b1.1-ratchet.toml` = **0 dòng** (3 tệp đó do **Lead** sở hữu duy nhất lúc merge). T2 PASS. |
```

**`QUEUE.md:489`** — `KT-422` · thay nguyên dòng:

```text
| KT-422 | **P0** **Chặng 3 (W9) — `B1.2` bước 2: Tách `blksrv` + `fssrv` (`virtio-blk` + `vifast-fs`/`vfs`) 100 % ra User Space & Gỡ khỏi kernel** | KT-419b, KT-420b | 3 h | BLOCKED | (D2-259/D2-261/D2-262 + **`D2-264` mục 4 — DoD ĐO ĐƯỢC**) **TIỀN ĐỀ CỨNG (fail-closed)**: như `KT-421` — `Frame.{MapIo,UnmapIo}` + `IrqHandler` + `IOSpace` phải do `KT-419`/`KT-419b` cài xong trước. Chạy `virtio-blk` (`blksrv`, qua `Frame.MapIo` + `IrqHandler` + `IOSpace`) và `vifast-fs`/`vifast-vfs` (`fssrv`, qua `blk.vidl`/`fs.vidl`) 100 % ở User Space (giữ đủ lệnh FS `ls`/`cat`/`write`/`mkdir`/`rm`/`mv`/`ln`/`fsck`/`df` & bền đĩa persist); xoá sạch `proc_fs.rs`, `virtio_blk.rs` và gỡ `vifast-fs`/`vifast-vfs` khỏi kernel. **DoD đo được**: (1) **trên nhánh**: `# B1.1 ratchet: dep_edges=0 (đóng băng 0, mới 0) driver_files=10 (đóng băng 15, mới 0) stale=5` — `driver_files` **15→10**, xoá đúng 5 mục: `vifast-plat-{aarch64,riscv64,x86_64}/src/virtio_blk.rs`, `vifast-plat-{aarch64,x86_64}/tests/virtio_blk.rs`; (2) **sau merge**: `driver_files=10 (đóng băng 10, mới 0) stale=0` — `stale=0` bắt buộc; (3) **ca âm**: thêm lại `virtio_blk.rs` ⇒ ĐỎ `B1.1-DRIVER`; (4) `test ! -e` đủ 5 tệp; (5) **kernel sạch FS/VFS**: chính dòng `dep_edges=0` đã phủ 3 `targets` (`vifast-net`/`vifast-fs`/`vifast-vfs`, `b1.1-ratchet.toml:17`) ⇒ 0 cạnh từ `plat-*`; (6) chồng tệp đo như `KT-421` mục (5) (0 dòng trên 3 tệp dùng chung). T2 PASS. |
```

**`QUEUE.md:490`** — `KT-423` · thay nguyên dòng:

```text
| KT-423 | **P1** **Chặng 3 (W9) — `B1.2` bước 3: Tách `gpusrv` + `inputsrv` + `rngsrv` (`virtio-rng`) ra User Space — Đích cuối `plat-*` 0 driver!** | KT-419b, KT-420b | 3 h | BLOCKED | (D2-259/D2-261/D2-262 + **`D2-264` mục 4 — DoD ĐO ĐƯỢC, thay "`driver_files → 0` vĩnh viễn" bằng số đếm máy**) **TIỀN ĐỀ CỨNG (fail-closed)**: như `KT-421`. Chạy `virtio-gpu` (`gpusrv`, giữ PPM 23/23), `virtio-input` (`inputsrv`) và **`virtio-rng` (`rngsrv`, từ `vifast-virtio::{rng,rngdev}` của `KT-418b`)** 100 % ở User Space qua `gpu.vidl`/`input.vidl`; gỡ sạch `virtio_*` & `vifast-virtio` khỏi `vifast-plat-*` ⇒ đích cuối **`vifast-plat-*` chỉ còn `boot / trap / MMU / IRQ / timer / SMP / FPU`**. **DoD đo được**: (1) **trên nhánh**: `# B1.1 ratchet: dep_edges=0 (đóng băng 0, mới 0) driver_files=2 (đóng băng 10, mới 0) stale=8` — `driver_files` **10→2**, xoá đúng 8 mục: `vifast-plat-{aarch64,riscv64,x86_64}/src/virtio_gpu.rs`, `vifast-plat-{aarch64,riscv64}/src/virtio_input.rs`, `vifast-plat-riscv64/src/virtio_console.rs`, `vifast-plat-aarch64/tests/{virtio_gpu,virtio_input}.rs`; (2) **phân loại 2 mục còn lại** (dịch vụ kernel, **không phải driver**) `{vifast-kbin-riscv64/src/loop_svc.rs, vifast-kbin-riscv64/src/mem_svc.rs}` sang khoá ratchet mới `kernel_service_files` (đóng băng đúng 2 mục; `check-api-boundary.py` thêm khoá vào `RATCHET_KEYS` + kiểm "mục mới" = 0) ⇒ `driver_files=0`, `kernel_service_files=2`, **hợp hai khoá luôn ⊆ 23 mục ban đầu** (mục "mới" = 0 ở mọi bước ⇒ phân loại, **không** nới ratchet); (3) **ca âm**: thêm lại bất kỳ `virtio_*.rs` trong `vifast-plat-*` ⇒ ĐỎ `B1.1-DRIVER`; (4) `test ! -e` đủ 8 tệp; (5) `grep -rn "vifast-virtio" v2/kernel/*/Cargo.toml` = **0 dòng**; (6) chồng tệp đo như `KT-421` mục (5). T2 PASS. |
```

**`QUEUE.md:491`** — `KT-423b` · thay nguyên dòng:

```text
| KT-423b | **P1** **Chặng 3 (W9b) — Driver Mở rộng Thuần User Space: PCIe MSI-X + NVMe (`nvmesrv`, `-device nvme`, 0 dòng mã thêm vào kernel)** | KT-419b, KT-420b, KT-423 | 2,5 h | BLOCKED | **(D2-262 — bằng chứng sống cho tính dễ mở rộng của Kiến trúc 5 tầng)** **TIỀN ĐỀ CỨNG**: như `KT-421` (`IrqControl.GetMsi` + `IrqHandler` + `Frame.MapIo` + `IOSpace` do `KT-419`/`KT-419b` cài). Cài driver PCIe MSI-X + NVMe (`nvmesrv`, triển khai `blk.vidl` trên `-device nvme`) **100 % trong `v2/user/`** với **0 dòng mã thêm vào `v2/kernel/`**. **DoD đo được**: (1) `git diff --name-only <base>..<task> -- v2/kernel/` = **0 dòng** (đo bằng pathspec, không đếm cảm tính); (2) `# B1.1 ratchet: … driver_files=0 (đóng băng 0, mới 0) … stale=0` + `kernel_service_files=2 (đóng băng 2, mới 0)` giữ nguyên; (3) đọc/ghi/verify khối thật trên QEMU NVMe (TAP + dòng `# nvme:` trong `JOURNAL/`); (4) ca âm DMA ngoài `IOSpace` bị chặn **có tên lỗi**. T2 PASS. |
```

**`QUEUE.md:492`** — `KT-424` · thay nguyên dòng:

```text
| KT-424 | **P1** **Chặng 3 (W10) — Hệ sinh thái Thực tế (`ViShell`/`ViScript` + PoC Rust `std` PAL `U1b`) & Kiểm toán Kiến trúc Chốt chặng** | KT-414, KT-414b, KT-417c, KT-421, KT-422, KT-423, KT-423b | 3 h | BLOCKED | **(D2-259/D2-261/D2-262)** (1) Port & chạy tương tác thật `ViShell` (`ViScript`) trên `vifast-rt` + `.vidl` + kiểm chéo cục bộ `viscript-vifast.sh` (`D2-241`, 0 GitHub Actions); (2) chạy PoC ứng dụng Rust `std` (`U1b`: `thread`/`Mutex` trên `WaitAddr`, vDSO clock, `fs`, `stdout`); (3) **mục #0 — đóng 3 khoảng trống bằng chứng của `KT-417c`** (`D2-264` mục 2–3): 1 boot `x86_64` in `# core resume … via=syscall` + `gs_in=`/`gs_out=` ở **cả hai** cửa + ≥1 ca âm cho dòng `via=syscall` + gọi `gs_is_kernel(hart, gs_base_in)` ở đầu `syscall_dispatch` (đóng `hotfix-review.md` MAJOR-1/2/3); (4) **Kiểm toán kiến trúc sạch (KHÔNG soak phát hành)**: T2 PASS + `L1-medium` 40k (cả `StaticStore` & `UntypedStore`) + Miri + `v2 mutants --all` (100 % KILLED) + `vifast-bench --bkl` + **bất biến đo được**: `# B1.1 ratchet: … dep_edges=0 … driver_files=0 … stale=0` + `kernel_service_files=2 (đóng băng 2, mới 0)` (khoá mới theo `D2-264` mục 4) + `grep -c "\[limits.plat\]" spec/kernel.toml` = **0** + `v2/user/` là Cargo workspace thật (`cargo metadata --no-deps` đọc được) + `plat-*` chỉ còn `boot/trap/MMU/IRQ/timer/SMP/FPU`. |
```

---

## §4 `STATE.md` — khối trạng thái hiện tại (Lead/Agent 2 dán, **giữ nguyên 2 dòng mốc**)

Thay **dòng 11 → 44** của `v2/agent/STATE.md` bằng đúng khối dưới (hai dấu ``` ở dòng 10 và 45 giữ nguyên). Cập nhật theo `AGENTS.md` §6.3 + docs-audit F7: **0 câu hỏi trong nguồn sự thật** — `PUSH` ghi bằng dữ kiện (`origin/main` = `a0f1f2e7`, 5 commit chưa push), không ghi “đã push?”.

```text
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-28 21:05 JST / 12:05 UTC (Lead: T2 PASS @`82502f7d`; hotfix `KT-417c` merged; `mutants --all` ĐANG CHẠY) ===
HEAD          : `82502f7d` — **T2 PASS 81/81** (`# gate-steps registered=81 ran=81 skipped=0 failed=0`,
                `# gate-wall 727102 ms`; 2026-11-28 **11:28:25–11:40:33 UTC**; log `/tmp/t2-kt417c-hotfix-82502f7d.log`).
                Gồm hotfix `KT-417c` X1/X2/X3 (`c73eb77a`; T1 PASS `82/68/14/0` 668,8 s, 11:16:29–11:27:39 UTC;
                merge `82502f7d`) + vá công cụ `mutants` (`f1c59c5d`, merge `c3922c75`).
PUSH          : `origin/main` = **`a0f1f2e7`** ⇒ **5 commit chưa push** (`f1c59c5d`, `c3922c75`, `c73eb77a`,
                `6705c1e4`, `82502f7d`); push sau khi `mutants --all` xong (AGENTS §7, không push khi cổng đỏ/WIP).
KẾ HOẠCH      : [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`D2-259`..`D2-264`). W7b/W8a/W8b **đóng sạch**
                (6 task `DONE`; khối dán đồng bộ QUEUE: `/tmp/briefs/agent0-so-sach-2026-11-28.md` §1);
                `KT-417d` p2/2 (`ArchVmOps`/`core_foreign.rs`/`Retype` VM) **chờ Agent 0 duyệt tách**
                `KT-417e/f/g/h` (`/tmp/briefs/approval-packet-kt417e.md`, 10,5 h `[ƯL]`); DoD W9 (`KT-421..423`)
                đã soạn lại **đo được** (cùng báo cáo §3) — chờ Agent 0 chốt Phương án B.
CỔNG          : **T2 PASS 81/81 @`82502f7d`** (mới nhất, 727,1 s). Trước: `b97c33c1` PASS 81/81 (27/11 00:20–00:31 UTC);
                `4f6acb02` PASS; W7b có 1 lượt đỏ 1/81 do DNS slirp (T-INFRA, không tính xanh `D2-07`).
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417` + `KT-417b` + `KT-417d` p1/2 + hotfix F1/F2/F3; (2) Lõi Kép DONE;
                (3) User Space `KT-420` **DONE**, `KT-420b` chưa; (4) **W8b `KT-417c` = DONE có điều kiện** (`D2-264`):
                3 khoảng trống **bằng chứng chạy** (`hotfix-review.md` MAJOR-1 `ug=` hằng 0, MAJOR-2 nhánh
                `Switched` từ cửa `syscall` chưa từng chạy, MAJOR-3 `iretq` thứ 2/3 + `gs_is_kernel` dead code)
                gắn **`KT-424` mục #0**; (5) **W9 DoD phải viết lại**: `driver_files` = **23** mục (không phải 0),
                `Frame.{MapIo,UnmapIo}`/`IrqControl`/`IrqHandler`/`IOSpace` còn `unsupported` (`spec/abi.toml:242,245-247`)
                ⇒ **tiền đề cứng** `KT-419`/`KT-419b`; nhãn "0 % chồng tệp" → **"chồng tệp có chủ"**;
                (6) `vifast-ktest` 3 arch DONE. **Audit**: `code` X1/X2/X3 **đã vá** (0 BLOCKER, 3 MAJOR = nợ **đo**);
                `test` 12 gap — **mở**: `G7` (sàn số ca selftest, `check_abi.py` tự khai), `G9` (`page.rs`
                `align > page_size` tiêu trang rồi trả `None`); `harness` 15 — L1 0-seed **đã vá** (`e6af3f76`), **mở**:
                `F7` (`INFRA_INVALID` chỉ in, không chặn), `F8` (bước thiếu `.rc` không tính đỏ), `F12` (T2 không
                sinh khoá đo + preflight bỏ qua tệp `??`); `docs` 15 — `QUEUE` lệch 6 task (**có khối dán**),
                `DECISIONS.md:28` header lệch, `D2-264` chưa có (đã soạn).
TASK          : **389** — **380 DONE / 0 READY / 9 BLOCKED** (`KT-417d` = `PARTIAL 3/5`) **sau khi Agent 0 dán** §1.
                Hiện `QUEUE.md` còn in `READY`/`BLOCKED` cho 6 task **đã DONE**: `KT-417b`, `KT-418c`, `KT-408d`,
                `KT-408e`, `KT-420`, `KT-417c` — chờ dán (Lead **không** sửa QUEUE, AGENTS §3).
ĐANG CHẠY    : **`mutants --all`** (PID 60806; bắt đầu **11:41 UTC** 28/11; log `/tmp/mutants-all-82502f7d.log`)
                — **kết quả: `<Lead điền>`**. Lượt trước (`53eaddb7`) **FAIL** vì `mutants.py:480 TypeError str/bytes`
                + `mut-info-x1` `E_WRONG-REASON` oan; đã vá ở `f1c59c5d` (selftest 12/12) nên lượt này là phép đo
                **đầu tiên** của `mutants --all` trên cây đã vá. **Dở dang cần lượt sau**: (a) Agent 0 trả lời
                **5 câu hỏi** (§2.C báo cáo sổ sách); (b) đóng `MAJOR-1/2/3` bằng **1 boot `via=syscall`** + `gs_in=`
                (giao `KT-424` #0); (c) **chạy nặng bù**: `soak-x86_64 600`, `hardening-max.sh`, product 3 arch,
                `l1-tier --tier full` — bắt buộc vì `c73eb77a` chạm `v2/kernel/vifast-{plat,kbin}-x86_64` + `run-x86_64.sh`;
                (d) bằng chứng nặng cũ (`JOURNAL/heavy-20261127/`, đạt trên `44e60e67`, build-inputs ≡ `b97c33c1`)
                **không tái dùng được** cho HEAD (§6.3) — đây là nợ bằng chứng, không phải nợ mã.
MÁY (sim)     : `size_of::<Machine<StaticStore, TestObserver>>()` = **172.424** B [V1:ĐO @`b97c33c1`] / trần
                **184.832** / pin **196.608**. Hotfix `KT-417c` chỉ chạm `plat-x86_64`/`kbin-x86_64`/runner ⇒
                kỳ vọng **không đổi**, nhưng **chưa đo lại** trên `82502f7d` ⇒ không được coi là đã kiểm.
CHẠY NẶNG     : (lịch sử, **đã lạc hậu so với HEAD**) 27/11 00:44–02:10 UTC trên `44e60e67`: soak x86_64 **600/600
                lỗi=0, aarch64 smoke 2/2, hardening-max **PASS 4 chân**, product 3 arch **PASS** (10/10, 11/11),
                `l1-tier full` **400k PASS**; Miri **PASS** 7261 s (`a0f1f2e7`). `mutants --all` lần trước FAIL (công cụ).
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```

---

## §5 Ba rủi ro lớn nhất khi dán (đọc trước khi dán)

1. **Bằng chứng T2/T1 của `KT-417c` chỉ nằm ở `/tmp`** (`/tmp/t1-kt417c-hotfix-c73eb77a.log`, `/tmp/t2-kt417c-hotfix-82502f7d.log`) — `/tmp` **không bền** và docs-audit F13 đã nêu đúng lỗi này một lần. Nếu `D2-264` dẫn `/tmp/...` mà không commit log/nhật ký vào `v2/agent/JOURNAL/`, mục `DONE` mất dấu vết theo §6.3. **Đề nghị**: Lead `cp` 2 log + `meta.txt` vào `JOURNAL/kt417c-fix/` **trước** khi dán `D2-264` — hoặc ghi rõ `D2-264` là “dẫn chứng ngoài repo, chưa bền”.
2. **Dán `DONE` mà không chạy lại cổng sau khi hạ `b1.1-ratchet.toml` (W9)**: dòng `stale=0` chỉ đúng nếu danh sách được hạ **cùng lúc với việc xoá tệp**. Nếu hạ danh sách trước/sau, cổng vẫn XANH (stale chỉ là cảnh báo) ⇒ ratchet trôi âm thầm đúng kiểu `KT-408e` vừa vá. DoD §3 đã bắt `stale=0` làm điều kiện nghiệm thu — đừng bỏ qua khi merge.
3. **`D2-264` mục 3 gắn nợ `MAJOR-1/2/3` vào `KT-424`** trong khi `KT-424` lại là task **chốt chặng** cuối: nếu `KT-421..423b` trượt, nợ đo của một tính năng **bảo mật** (syscall/CPL3) sẽ nằm ở task cuối chuỗi. Nếu Agent 0 không muốn rủi ro đó, dùng `KT-417i` (mã hợp lệ, 2 h) và mở ngay sau W8b thay vì đợi `KT-424`.

