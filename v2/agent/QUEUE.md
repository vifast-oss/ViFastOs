# QUEUE — ViFastOs v2, vòng 1

Người ghi: **Agent 0** (qua người điều phối). Người đọc/thi hành: **Agent 2** dòng OS.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §4, §5.1, §5.2, §10.4 và `review/PATCH-BRIEF.md` §1.
Trạng thái hợp lệ: `READY | BLOCKED | REVIEW | DONE`.
**Tệp này tự chứa**: đọc hết 5 khối dưới đây là đủ để chạy một task mà không cần mở tài liệu khác.

---

## Khối A — Luật chơi (bắt buộc)

**Vòng lặp 8 bước cho mỗi task**

1. **Đọc** `STATE.md`, `QUEUE.md`, `DECISIONS.md`, phần spec liên quan. Không đọc lại toàn repo.
2. **Lập kế hoạch nhỏ** trong `STATE.md`: các bước, tệp sẽ chạm, test sẽ thêm.
3. **Test đỏ trước**: viết test/kịch bản thể hiện yêu cầu; chạy, xác nhận đỏ **đúng lý do**.
4. **Cài đặt** theo bước nhỏ; sau mỗi bước chạy `./v2/bin/v2 check`.
5. **Cổng**: `./v2/bin/v2 gate --fast` (luôn); `./v2/bin/v2 gate` nếu chạm platform/user.
6. **Commit**: `[KT-##] <tóm tắt>` + kết quả cổng + `BUILD_ID`.
7. **Cập nhật** `STATE.md`, `JOURNAL/YYYY-MM-DD.md`; `vifast-agentctl done <id>`.
8. **Nén ngữ cảnh** nếu lớn, rồi quay lại bước 1 với task kế.

**Cấm (9 điều)**

1. Sửa `spec/` — trừ `KT-04a` (được ghi `spec/abi.toml`, `spec/kernel.toml`) và `KT-04b`
   (một lần theo D2-32: chỉ chép draft reserved đã sửa theo D2-32 + D2-29).
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. `#[ignore]`, `#[should_panic]` để né, tắt test, bỏ case khỏi danh sách chạy.
4. Nới ngưỡng (thời gian, seed, số ca đỏ, `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`).
5. Retry để qua cổng; flake là `T-INFRA`, ghi log, chạy lại tối đa **một** lần chỉ để phân loại.
6. Thêm `unsafe` ngoài danh sách crate đóng; thêm `cfg(target_arch)` vào kernel/runtime.
7. Đưa số đo viết tay vào tài liệu (số đo do `./v2/bin/v2 nightly` sinh).
8. Sửa `DECISIONS.md` của mình hoặc của dòng ViScript.
9. `pkill -f qemu-system` toàn cục — chỉ `kill <pid>` do chính task tạo.

**Bắt buộc**: hồi quy ⇒ replicate rồi **revert trước**, điều tra sau; lỗi logic phải có `.scn` đỏ trước; `cargo fmt` trước commit; SPDX **theo loại tệp** (`//` Rust/asm; `#` .vis/.vish/.vidl/.toml; sidecar `.license` cho tệp không chú thích); mọi số liệu mang nhãn `[V1:…]`/`[TK]`/`[GT]`/`[ƯL]`.

## Khối B — Ngân sách và điều kiện dừng (3 giờ/task)

| Tình huống | Hành động |
|---|---|
| Task vượt **3 giờ** | Dừng, ghi tiến độ + lý do vào `STATE.md`/`JOURNAL`, chuyển task kế không phụ thuộc |
| Kẹt **hai lần** cùng một ý | `BLOCKED` + câu hỏi cho Agent 0, chuyển task kế (quy tắc 2.5: đổi nhánh, không thử lần ba) |
| Spec mâu thuẫn/thiếu | `BLOCKED` kèm đề xuất cụ thể, **không** tự đổi spec |
| Cổng đỏ do lỗi không thuộc task | Mở mục lỗi trong `JOURNAL` (+ `.scn` nếu có), **không** sửa lan |
| Hết task khả dụng | Chạy `./v2/bin/v2 nightly`, viết báo cáo, dừng |

**Mỗi task ≤ 3 giờ. Task lớn hơn phải tách** (ví dụ KT-04 tách a/b/c/d). Không gộp hai task vào một commit.

## Khối C — Định nghĩa "xong" (DoD)

1. Tiêu chí trong bảng dưới đạt, **có bằng chứng** (đầu ra lệnh, số seed, TAP).
2. Test mới **đã từng đỏ** (dán log đỏ vào `JOURNAL`).
3. Cổng tương ứng **xanh, không retry**.
4. `STATE.md` + `JOURNAL/` cập nhật; không còn TODO không có task.

## Khối D — Lệnh cổng

```bash
./v2/bin/v2 check          # fmt + clippy + luật cấu trúc (nhanh, không sim)
./v2/bin/v2 gate --fast    # BẮT BUỘC cuối mỗi task: fmt, clippy, luật cấu trúc, L0
                    # + gen-check (từ khi có KT-05) + L1 1.000 seed (từ khi có sim)
./v2/bin/v2 nightly        # L1 100.000 seed, mutation, Miri, soak — không chạy trong vòng task
```

- Giai đoạn đầu (trước KT-03): cổng **chưa có** ⇒ ghi "cổng chưa có" vào `STATE.md`, không tự tạo cổng ngoài task.
- **L1 chưa bật tới KT-08** (chưa có sim). Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant.
- `./v2/bin/v2 nightly` và soak **không** chạy song song với build/cổng khác; tối đa 3 QEMU song song; sim dùng tối đa `số core − 1`.

## Khối E — Định dạng báo cáo

`reports/R<n>-v2.md` — 6 mục: (1) tóm tắt 5 dòng; (2) bảng task `ID | trạng thái | commit | cổng`; (3) lỗi tìm thấy (`.scn`); (4) số đo thay đổi; (5) đột biến sống; (6) `BLOCKED` + câu hỏi + đề xuất task kế.

`reports/R<n>-v2.json` — 8 khoá, **có `project`**:

```json
{
  "project": "v2",
  "round": 1,
  "head": "ca2eaf5",
  "build_id": "—",
  "tasks": [{"id": "KT-01", "status": "DONE", "commits": ["…"], "gates": ["fast"]}],
  "gates": {"fast": "PASS", "full": "—", "nightly": "—"},
  "sim": {"seeds": 0, "failures": 0},
  "coverage": {"transitions": "—", "error_paths": "—"},
  "mutants": {"killed": 0, "alive": 0},
  "perf": {},
  "blocked": []
}
```

Agent 0 đọc **JSON trước**, MD sau. JSON thiếu `project` bị trả lại.

---

## Bảng task vòng 1

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` (Q10) | — | 1 h | DONE | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục QD §6, mỗi mục có ca kiểm v2 |
| KT-02 | khung `v2/` + `v2/agent/` + `v2/bin/` (không ghi đè tệp đã có), LICENSE tham chiếu root; port **tối đa** `vifast-fs` + `vifast-fsutil` + `vifast-elf` (D2-28), build xanh, mỗi tệp ≤1.500 dòng | — | 3 h | DONE | `cargo build` workspace xanh; có `v2/bin/`; mọi tệp ≤1.500 dòng |
| KT-03 | wrapper `v2/bin/v2` + khung `check`/`gate --fast`: fmt, clippy, luật cấu trúc, L0, SPDX theo loại tệp; **chưa** gen-check/L1 | KT-02 | 3 h | DONE | `./v2/bin/v2 check` chạy từ repo root; 5 luật có 5 case âm |
| KT-04a | bảng ánh xạ v1→v2 (25 crate, 58 syscall→16+method, 22 KError→19) + `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B) | KT-02 | 3 h | DONE | abi.toml parse; bảng ánh xạ đủ; `abi_version`/`min_compat` |
| KT-04b | object reserved theo **D2-32**: SchedControl=8 lên **lõi**; 8 object reserved 9..16; VSpace khách = cờ `GUEST` trong `Untyped.Retype`, **không** method; chỉ số + `NotSupported` | KT-04a | 2 h | DONE | method reserved trả NotSupported; số không trùng; `check_reserved` xanh trên `spec/` |
| KT-04c | **21 kịch bản** `.scn` + kiểm tĩnh (D2-27) + **arity theo draft D2-33**; ca 18 hết blocked khi SchedControl lên lõi; **21/21, 0 blocked** | KT-04a | 3 h | DONE | 21 tệp; `check_scn` 21/21 0 blocked; draft chữ ký REVIEW |
| KT-04d | gửi Agent 0 duyệt: checklist 5 mục/method + 21 kịch bản + draft chữ ký + **nội dung D2-26..28** | KT-04a,b,c | 1 h | DONE | **APPROVED 2026-11-12 Agent 0

APPROVED 2026-11-13 Agent 0 — K1b (D2-49, 1 hart + hoãn push migration K1c)**; D2-35..39; KT-05 mở; `check_scn` 21/21 0 blocked |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md`; sinh từ `abi.toml` | KT-04d | 3 h | DONE | bước 0: draft chữ ký D2-33 vào `spec/[method_signature]` (`stable-draft`) + D2-35; code sinh biên dịch; sửa tay bị bắt; gen-check bật |
| KT-06 | `vifast-agentctl` `--project v2\|viscript`: next/start/done/block/report, bỏ qua REVIEW | KT-02 | 2 h | DONE | dùng được cho chính vòng này; JSON có `project` |
| KT-07 | K0.5: boot riscv64 QEMU qua OpenSBI nạp trực tiếp; DebugPutStr/DebugHalt; TAP 1 test; 50 boot 0 lỗi | KT-02, KT-03 | 3 h | DONE | 50 boot 0 lỗi, có log |
| KT-08 | sim harness + store giả + scheduler tối thiểu 1 hart; kịch bản rỗng tất định theo seed (chưa cần ABI) | KT-02 | 3 h | DONE | harness + store + scheduler chạy; cùng seed → cùng trace |
| KT-09 | Untyped trên sim (watermark, Retype, Revoke qua CNode) + end-to-end Retype→Configure→Call/Reply | KT-08, KT-05, KT-12b | 3 h | DONE | 29 test (gồm 11 TCB state, gap căn lề, Untyped lồng nhau); invariant tổng byte; preempt; end-to-end; fixture âm kiểm **thông điệp**; C5 D2-41; sửa review Agent 0 ở `da7d30f` |
| KT-10 | CNode/MDB test-first, mô hình tối giản **trong test** | KT-09 | 3 h | DONE | 20 test mới (13 CNode + 5 property + 2 unit); property 128×48=6144; Revoke/finalise; reviewer APPROVE; F1/F2 đã sửa |
| KT-11 | `vifast-model` Untyped/CNode; chạy các ca Untyped/CNode của KT-04c | KT-10 | 2 h | DONE | 24 tệp; 184 test; parity 10.000 seed/240.000 thao tác/0 lệch; .scn 9/9; reviewer APPROVE; D2-44..46 |
| KT-12 | `vifast-traceformat` + ring (trước khi sim dump) | KT-02 | 2 h | DONE | giải mã dump; D2-38/D2-40: text = scenario log, KT-12 sở hữu bản ghi 32 B **cùng định dạng sim/QEMU** + golden nhị phân |
| KT-12b | `vifast-traceformat` **no_std** + `FixedRing<const N>` (kernel); `alloc` feature cho `decode_all`; `Ring`/`Vec` chỉ sau `std`; cross-check byte với golden D2-40 | KT-12 | 2 h | DONE | `#![no_std]` biên dịch; `FixedRing` không cấp phát; test host vẫn xanh |
| KT-T26 | host test `vifast-fs` + block giả + cache; MAX_IO=256; đọc RAW kiểm read-your-own-write; theo D-03 "ai chen giữa" | — | 3 h | DONE | đèn host DONE `02e0a96` theo D2-36; T26 **OPEN** tới K3.5 (VFS/read_dir + smp); không chạy lại 1.000 seed |
| KT-13 | Bootstrap **Miri** (nightly + `miri` + `rust-src`) và lệnh `v2 miri`; test đỏ: lệnh chưa có/component thiếu | — | 1,5 h | DONE | `v2 miri` + PoC sim 13/13 PASS (33 s); thiếu toolchain fail rõ; gate 184 test; reviewer APPROVE |
| KT-14 | `vifast-objmem` core: `ObjHeader` 16 B, `ObjRef`, `ObjStore`, arena thô căn lề, `place/with/with_mut/destroy`, `no_std` + Miri từ commit đầu | KT-13 | 3 h | DONE | 15 test; Miri 15/15; build bare-metal; gate 199 test; reviewer DRIFT đã sửa; YC-18 chốt D2-48 |
| KT-15 | `with_many_mut` N≤8 + kiểm alias từng cặp + fuzz 10.000 chuỗi (200 chuỗi dưới Miri) | KT-14 | 2 h | DONE | 34 test; Miri alias 8/8 + fuzz 200; fuzz 10k; gate 218 test; guard `chua_header`/TypeId; reviewer DRIFT đã sửa |
| KT-16 | Tích hợp objmem vào sim: thay `Store` **và** ruột `kobj::Machine`, giữ API; bước 0 gom helper ≤45′; cho phép tách KT-16a/b | KT-14, KT-15 | 3 h | DONE | 23 tệp; test cũ 0 dòng sửa; body trong objmem, Cap `Vec`; sim 0 unsafe; gate 227 test/L1 1000; parity 10k; Miri 39+4+8+13; reviewer APPROVE |
| KT-17 | Cổng Miri tổng `nightly --only-miri` + KT-04c trên objmem + chốt DoD K1a' | KT-13, KT-14, KT-15, KT-16 | 2 h | DONE | `nightly --only-miri` PASS 365 s; 231 test/41 suite; parity 10k; `.scn` 9/9; 2 follow-up đóng; reviewer DRIFT đã sửa; K1a' DONE |
| KT-18 | TCB đầy đủ + `suspended` + bảng `ALLOWED` 11×11 + `Suspend/Resume/ReadState` + finalise | — | 3 h | DONE | 48 cặp (46 reachable + 2 Stopping reserved); 121 cặp; `ReadState` 4 giá trị; Abort gỡ hàng; 18 test; Miri 18/18; gate 251 test; reviewer DRIFT đã sửa |
| KT-19 | Scheduler 1 hart: 256 mức + bitmap, `Ready/Running`, preempt mọi bước, affinity, không đói, finalise khỏi queue | KT-18 | 3 h | DONE | 263 test; Miri 10/10 (5 seed/64 mức cfg); 256 mức; preempt 20/20; 1k seed/64k op; reviewer APPROVE |
| KT-20 | MCS budget: timer ảo, consumed/refill một đoạn, `Sleeping{Budget}`, invariant theo chu kỳ | KT-19 | 3 h | DONE | 284 test; Miri 12/12; budget 0 vi phạm; no SC/unconfigured **không chạy** (D2-50); refill/Sleeping; 1k seed; reviewer DRIFT đã sửa |
| KT-21 | Donation Call/Reply + passive server + bind Reply/Notification + MCP + budget khi donation | KT-20 | 3 h | DONE | 13 test; Miri 13/13; 14.609 bước donation/64.000; SC-less chỉ donate; Reply đúng donor; MCP clamp D2-51; gate 298; reviewer APPROVE |
| KT-22 | Hộp thư mô hình FIFO, đầy → `Busy`/backpressure; **không** lock-free/loom | KT-19 | 2 h | DONE | 10 test; Miri 10/10; FIFO cap 8; Busy/backpressure; invariant 1k seed; gate 311; reviewer APPROVE; Migrate NotSupported |
| KT-23 | Tích hợp scenario + L1 K1b 1.000 seed + coverage 100% + Miri + parity 10k + bảng DoD K1b | KT-18, KT-19, KT-20, KT-21, KT-22 | 2 h | DONE | 321 test; 46/46+73/73; fuzz 256k/0; Miri 12/12/2608s; parity 10k; `.scn` 21/11; fixture D2-52; DoD 3/4 + push K1c; reviewer APPROVE |
| KT-R1 | báo cáo `R1-v2` phủ task đã mở + `blocked[]` | các task READY | 1 h | DONE | MD + JSON theo Khối E, có `project` |
| KT-24 | IPC core 1: `MessageInfo` 35 bit + Message 64 MR/IPC buffer + `ipc::transfer` + Send/NBSend/Recv/NBRecv + `no_reply` hai chiều | KT-23 | 3 h | DONE | 336 test; Miri 13/13; MessageInfo 35 bit/64 MR; no_reply 4 ca; Call non-Ep NotSupported; invariant IPC; reviewer APPROVE |
| KT-25 | IPC core 2: Call/Reply/ReplyRecv/NBSendRecv/NBSendWait + 4 cap (GRANT/unwrap badge/receive window) + vòng đời Reply, donation không rò SC | KT-24 | 3 h | DONE | 361 test; Miri 24/24; 4 cap mềm Q6; Reply lifecycle/Cancelled Q8; NB* Q15; donation sạch; `.scn` 05/07/08/15/16 0 lệch; reviewer OK |
| KT-26 | Fault + Exit: `FaultKind` VM/unknown/user model-only, fault qua Call+Reply, resume có thanh ghi, procsrv `{code,how}` | KT-25 | 3 h | DONE | 386 test; Miri 24/24; 4 FaultKind; resume regs trước Ready; Exit D2-25; procsrv 4 nhánh; `.scn` 13/14/20; reviewer APPROVE |
| KT-27 | Notification: word 64 + badge, Signal OR + 1 waiter + xoá word, Wait/Poll, bound TCB + `SchedContext.Bind` | KT-24 | 2,5 h | DONE | 413 test; Miri 26/26; word64/OR/badge; 1 waiter; bind 1:1; bound BlockedOnRecv; recv reset error; reviewer APPROVE |
| KT-28 | WaitOnAddress: bảng băm `(frame,offset)`, đọc + xếp hàng một lần, `WakeAddr(n)`, không timeout | KT-23 | 2 h | DONE | 427 test; Miri 12/12; bucket 64; expected lệch trả ngay; WakeAddr FIFO n=0/1/2/MAX; không timeout; reviewer APPROVE |
| KT-29 | SMP 1/2/4/8 + push migration: `with_harts`, `published.idle`/`cur_prio`, `Migrate`, tất định, không work stealing | KT-28 | 3 h | DONE | 442 test; Miri 13/13; 1/2/4/8; push 1/hart nguồn; FIFO drain; ≤1 Migrate/TCB; guard n>1; reviewer CONFIRM 5/5 |
| KT-30 | `vifast-sync`: MCS lock + Mailbox MPSC cố định, ack thế hệ, đầy backpressure; stress+Miri (không loom) | KT-24 | 3 h | DONE | 461 test; Miri 16/16; MCS FIFO/không reentrant; 7 loại/đầy Busy; stress 200/0; acked≠delivered; reviewer CONFIRM |
| KT-31 | Cổng cuối K1c: 100.000 seed × smp 1/2/4/8 + mutation + Miri toàn bộ + bảng DoD K1c (không sửa semantics) | KT-30 | 3 h | DONE | 400k lượt; 6 mutation; Miri 276; gate 461; report C1/C3 **chưa đạt**; D2-61; K1c REVIEW |
| KT-32 | Đóng nợ C1: driver `l1_k1c` trộn IPC/Notification/WaitAddr + 100.000 seed × smp 1/2/4/8; `check_ipc` thật, cùng seed cùng trace | KT-31 | 2 h | DONE | 400k lượt; ipc_sent 5.786.297; delivered 1.123.586; dropped 3.748.540; 0 vi phạm; reviewer APPROVE |
| KT-33 | Đóng nợ C3+Miri: đủ **12/12** mutation §7.2 + thêm `kt25_ipc2` vào Miri all + chạy lại + cập nhật K1C-DOD (C1/C3 ĐẠT) | KT-32 | 2 h | DONE | 12/12 §7.2; Miri 300/kt25 24; driver 10k; gate 461; C2 nợ → KT-34 |
| KT-34 | Đóng C2: reject cap Reply đang `caller=Some` (test), bất biến `BlockedOnReply ⇒ Reply.caller==tcb`, fuzz Signal/WakeAddr sau xếp hàng; 10k×4 rồi 100k×4 | KT-33 | 3 h | DONE | 468 test; C2 ĐẠT; 100k sent 5.786.296/delivered 1.123.598/pending 914.525/dropped 3.748.173; Miri 8 tập/123 test; reviewer CONFIRM |
| KT-35 | `vifast-pte` (safe, forbid unsafe) + `v2/docs/k2-vm.md` hằng số 3 cấp | — | 2,5 h | DONE | 477 test; Miri 9/9; shape 9/9/9/12; PteArray 4096 B; no_std/forbid; reviewer CONFIRM |
| KT-36 | Frame 3 cỡ + ownership/Untyped + `GetAddress` | KT-35 | 3 h | DONE | 494 test; Miri 12+5; retype 12/21/30; PA window 1G; frame chết sạch; reviewer APPROVE |
| KT-37 | PageTable 3 cấp map/unmap + rights/attrs + 7 mã lỗi | KT-35, KT-36 | 3 h | DONE | 512 test; Miri 10+7; 7 mã lỗi/3 cỡ; walk 10k; 0 rò; reviewer APPROVE |
| KT-38 | VSpace + ASID + generation + `active_on` | KT-37 | 2,5 h | DONE | 526 test; Miri 7+5+2+2; 257 ASID⇒gen+1; active_on; 10k; reviewer APPROVE |
| KT-39 | W^X counters + protect/unmap + bất biến | KT-37 | 2,5 h | DONE | 536 test; Miri 8/8; W+X atomic; protect; shared RO; reviewer APPROVE sau DRIFT |
| KT-40 | TLB model + `TlbFlush` + ack thế hệ + chống deadlock | KT-38, KT-39 | 3 h | DONE | 552 test; Miri 10/10; 5 mutation; ack đủ mới Ok; deadlock 2 hart; oracle active_on |
| KT-41 | VM fault + `FaultKind::Vm` model-only | KT-38 | 2 h | DONE | 562/71; Miri 9/9; 3 reason; không fault_ep⇒InvalidArgument; C2; W+NX⇒Wx |
| KT-42 | 4 `.scn` + `check_vm` + shared read-only frame | KT-39, KT-40, KT-41 | 2,5 h | DONE | 569/73; Miri 2/2; 5 .scn (22..26); check_vm 8 checker; shared RO; reviewer APPROVE |
| KT-43 | Cổng cuối K2: 100k×4 + 16 mutation + Miri + DoD | KT-42 | 3 h | DONE | 100k×4 400k lượt/51,2 M op; 17/17 mutation (V5 = AP→BSP); Miri 146/0; gate 569; K2-DOD D1..D11 |
| KT-44 | `plat-common` + tách ảnh sản phẩm/ktest + đo budget | KT-43 | 2,5 h | DONE | efe128fb; 2 ảnh feature ktest; product 0 chuỗi; budget 0/262.144 + probe; TAP 1 boot; reviewer APPROVE |
| KT-45 | Trap khung: `stvec`, trapframe, decode, panic/trace | KT-44 | 3 h | DONE | c6037b24; TAP 5 (bp thật/ecall-SBI/illegal tiêm/nested); 50 boot; 569 gate; reviewer CONFIRM |
| KT-46 | Sv39 thật + nửa cao + physmap + `satp` + guard | KT-45 | 3 h | DONE | d5e7565d; TAP 8+3 (guard 0xd/0xf thật); satp 2 bước; 50 boot; gate 569; reviewer APPROVE |
| KT-47 | Timer SBI: 100 Hz, monotonic, deadline/hart | KT-45 | 2,5 h | DONE | 32b8ca78; TAP 4/4 + 12/12; Δ 10,0000–10,0192 ms; 1000 tick late=missed=0; 50 boot; reviewer APPROVE |
| KT-48 | PLIC + UART IRQ: enable/claim/complete | KT-46 | 3 h | DONE | aec27f73; TAP 15/15; 8 IRQ thật (IIR 0x2); delta thật; 50 boot; gate 569; reviewer DRIFT→vá |
| KT-49 | SMP: SBI HSM, stack/arena/PerHart | KT-46 | 3 h | DONE | 1cc67908; TAP 20; 300/300 boot (100 mỗi N); guard boot-hart=3; 5 ms thật; gate 569; reviewer DRIFT→vá |
| KT-50 | IPI thật + drain hộp thư + `VmKick`/`TlbFlush` | KT-49 | 2,5 h | DONE | 74c22d4b; TAP 25/25; 300/300 boot; coalesce 6→2; sip.SSIP (D2-76); post offline Invalid (D2-77); gate 569 |
| KT-51 | ASID thật + `sfence.vma` + shootdown ack + timeout | KT-46 | 3 h | DONE | 6e6fa4ee; TAP 30/30; stale dương tính; 300/300 boot; 3 mutation; shootdown tự active_on (D2-78); gate 569 |
| KT-52 | Object layer `no_std` + Retype nội bộ (VSpace/PT/Frame) | KT-46 | 3 h | DONE | 320daa59; TAP 37/37; byte invariant; map 4K phần cứng; W^X 2 ca; 300/300 boot; gate 569 |
| KT-53 | U-mode + 3 syscall + nạp `init` ELF | KT-46 | 3 h | DONE | 26adaa33; TAP U-mode 5/5; ecall U=8, U đọc kernel=13; 19 RangeError (D2-79); 450/450 boot; gate 569 |
| KT-54 | ktest TAP + runner QEMU smp 1/2/4 + `expected.txt` | KT-45 | 2,5 h | DONE | d6526279; expected.txt nguồn duy nhất; 450/450 boot (30m53s); 17 ca harness (15 đỏ); 0-boot die; gate 569 |
| KT-55 | Conformance 5 `.scn` × sim/QEMU, chuẩn hoá trace | KT-51 | 3 h | DONE | b3b8ed01; 61/61 dòng khớp smp2/4; TAP 37; gate 573; D2-80 hash ảnh; bắt lỗi W|X thật |
| KT-56 | `FaultKind` thật + W^X + `spec/abi.toml` **một lần** (D2-72 Q1) | KT-46 | 3 h | DONE | 25251719; FaultKind khớp K2; spec 1 hunk + gen tất định; 61/61 trace; 450/450 boot; gate 574 |
| KT-57 | Cổng K3: soak 0/1000, matrix, 100k L1, perf, K3-DOD | KT-54 | 3 h | DONE | ce943707; soak 0/1000 (219 s, 3 luồng); TAP 43/3/7 ×3; L1 400k; gate 574; K3-DOD |
| KT-58 | **K3b** port core `no_std`: CNode/MDB/Untyped | KT-57 | 3 h | DONE | 430b206c; parity 14 ca + 2016 bước; 3 mutation độc lập; gate 579; VM reserved (D2-83) |
| KT-59 | **K3b** port TCB/transition/sched + hộp thư | KT-58 | 3 h | DONE | 5374920a; parity 8 ca + differential; footprint 167.896 (D2-84); gate 587; F1 257-finalise atomic |
| KT-60 | **K3b** port IPC/Reply/Notification/WaitAddr/fault | KT-59 | 3 h | DONE | L1 driver chạy `no_std`; C1/C2 đạt |
| KT-61 | **K3b** cổng: TAP + soak + conformance trên nhân thật | KT-60 | 3 h | DONE | 1e2455c8; core TAP 43..52 trên QEMU; 53/53, 9/9; soak 0/1000; conf 61/61; gate 598; .bss 180.168 |
| KT-62 | **K3.5** syscall 0..15 đi vào `Machine` thật + `init` U-mode dùng core | KT-61 | 3 h | DONE | 58e0cd59; 16 syscall vào core; SYS_* sinh từ spec; notif payload 3/4/7; TAP user 17; gate 607; num_harts thật (D2-88) |
| KT-63 | **K3.5** chuyển cap qua IPC + finalise IPC khi Revoke | KT-62 | 3 h | DONE | 489b0883; cap transfer + finalise IPC; 3 vá nguyên tử (F1/F2/F3); gate 625; TAP 56; soak 0/1000 |
| KT-64 | **K3.5** `TlbFlush` thật + `SlotPool`→arena + `Bind` + `resume_with_regs` | KT-63 | 3 h | DONE | 194a09f5; 4/4 mảng; slot (region,offset) chống alias; Busy reachable; gate 642; TAP 60; soak 0/1000 |
| KT-64a | **K3.5** đồng bộ sim theo D2-89: finalise có waiter ⇒ `Cancelled`+`Ready` | KT-64 | 2,5 h | DONE | 519d2e55; sim anti-hang D2-89/90; `.scn` 27; parity 18/18 + 4/4; TAP 55 sender trên QEMU; gate 647 |
| KT-65 | **K3.5** procsrv (U-mode, Rust) + shell tối thiểu | KT-64a | 3 h | DONE | c607d0a9; 3 ELF U-mode that (procsrv/shell/con); wait=42, kill=Suspend(Abort), ps snapshot tu Machine; TAP 24; gate 649 |
| KT-66 | **K3.5** cổng: TAP + soak + L1 400k + `K35-DOD` | KT-65 | 3 h | DONE | 97903b15+324d1da9; tach anh ktest-proc; stack per-hart canary fail-closed; L1 400k; soak 0/1000; conf 61/61; gate 649 |
| KT-67 | **K6** memsrv U-mode + `vifast-alloc`: Frame thật cấp qua IPC | KT-66 | 3 h | DONE | e59f301d; vifast-alloc 4K/2M/1G mot nguon byte; memory bridge D2-83; TAP 60..64; gate 661/87; soak 0/1000; memsrv = facade (D2-96) |
| KT-67a | **K6** invocation hẹp cho U-mode (`Untyped.Retype`) để memsrv thật | KT-67 | 3 h | DONE | f56b0255; Retype 1.0 + Frame.Map/Unmap 11.0/11.1 (D2-99); memsrv U-mode tu retype; client map that; M1/M2 vá quyen; gate 661/87; soak 0/1000 |
| KT-67b | **K6** header IPC buffer U-mode + receive window ⇒ chuyển cap giữa 2 tiến trình | KT-67a | 3 h | DONE | b8f61c99; header IPC buffer + window; cap giua 2 tien trinh that (2 ELF/2 cspace); spec mo 1 lan +15-3; TAP 19; gate 661/87 |
| KT-68 | **K6** capalloc: cấp/thu hồi capability theo cây | KT-67b | 3 h | DONE | 76b6e22e; capalloc cay MDB (id sinh tu spec); TAP 24; gate 661/87; soak 0/1000; run.sh --boot vá |
| KT-69 | **K6** IDL + ring buffer chia sẻ (shared Frame) | KT-68 | 3 h | DONE | 3aac2ed5; vifast-idl (sinh stub) + vifast-ring SPSC; 2 tien trinh cung PA; TAP 29; gate 675/92; soak 1000/1000 |
| KT-70 | **K6** VFS tầng 1: pager + page cache | KT-69 | 3 h | DONE | e50540b2; pager+page cache LRU; 2 client cung Frame; TAP 35; gate 689/95; soak sha khong doi |
| KT-71 | **K6** VFS tầng 2: journal + rename/truncate/double-indirect + fsck | KT-70 | 3 h | DONE | 36f59041; journal write-ahead + ca am; fsck 4+1 loai; L2 that; gate 703/96; soak 0/1000 |
| KT-72 | **K6** virtio-blk thật (QEMU) | KT-71 | 3 h | DONE | 4e1fe995; PCI ECAM + BAR fail-closed + FLUSH dam phan; FS tren dia that; matrix 15/15; gate 718/99 |
| KT-73 | **K6** virtio-console + UART IRQ → user | KT-72 | 3 h | DONE | 19f16f1a; UART IRQ->ring->U-mode; shell doc ring that; console 0x1003; matrix 18/18; gate 726/99 |
| KT-74 | **K6** SetTls + exec chain | KT-73 | 3 h | DONE | 0155bafa; SetTls 15 (sinh tu spec) + exec atomic; va bug tp=hart-id; matrix 21/21; gate 726/99 |
| KT-75 | **K6** `test-persist` 7 pha trên virtio-blk | KT-74 | 3 h | DONE | 1ea24908; 7/7 pha reboot QEMU that (10 PID); ca am flush dung tang; host doc lap; gate 729/100 |
| KT-76 | **K6** cổng: `K6-DOD` "tương đương v1" + soak + L1 400k | KT-75 | 3 h | DONE | 2f34b09c; K6-DOD trung thuc: 8/8 tieu chi do duoc DAT, S1 shell anh san pham KHONG DAT => K6 KHONG DAT; L1 400k 4 cau hinh song song |

| KT-77a | **K6** `spec/v1-parity.toml` + script kịch bản serial | KT-76 | 3 h | DONE | de55b786; 28 mục (4 active/24 deferred-K7) mỗi mục có `source=file:dòng`; runner dùng nguyên `qemu_guard.sh`; 3 ca âm (exit 3/4/5); doc tuân luật 1 (không ngoại lệ) |
| KT-77b | **K6** user mode + shell vào **ảnh sản phẩm** | KT-78, KT-77a | 3 h | DONE | 24ac5e65; ảnh `07c34fc6`/176.768/`build=6e90c73c` (tái lập từ HEAD); shell 1/2/4 hart; parity 4/4 active; 3 ca âm; marker 0/11; `39ca3cc6` gắn nhãn "trước commit, KHÔNG dùng" |
| KT-78 | **K6** harness QEMU: process group + timeout + dọn con | KT-76 | 3 h | DONE | 8664d69f; process group thật (perl setpgrp, kill -- -PGID); guard_selftest cases=10 ok=10 gồm ca âm iso-bad + override từ ngoài; gate 729/100; matrix 21/21; soak 0/1000 |
| KT-79 | **K6** build tái lập byte (P20) | KT-76 | 3 h | DONE | a6cdcdcd; **OD-K6-3 CÔNG NHẬN ĐẠT ĐỦ** (D2-122.1): macOS + Linux `ps@ps` × 2 đường dẫn = `c1072d72`; ca âm đỏ; nguyên nhân bằng diff byte |
| KT-80 | **K6** 8 luật máy vào cổng (OD-K6-5) | KT-76 | 3 h | DONE | 56935bbc; 5 luật trong `v2 check`/`gate --fast` (wall 12,5 s); **17 ca hồi quy**; `key=value` bắt theo **hình dạng giá trị** (`build=729` đỏ); `expires_task` cho `pre_existing` (quá hạn tự `E1_EXPIRED`); selfbreak 5/5 |
| KT-77c | **K6** FS + vblk vào ảnh sản phẩm (tiêu chí 2, D2-123.2) | KT-77b | 3 h | DONE | 8b2a3e3a; ảnh `64a18ebf`/205.440/`build=1e84e92dc17e58ab text=130062`; 2 build 2 target dir cùng sha; `ls`/`cat` → active (6/6 ok); ghi + đọc lại sau reboot (2 boot, cùng tệp đĩa, smp 1/2/4); 3 ca âm; marker 0 vs 25; dùng 1 h 10 m / 3 h |
| KT-87 | **K6** `v2 check` kiểm cả tệp chưa `git add` (lỗ đã xảy ra hai lần) | KT-86 | 1 h | DONE | 4b9567e5; tập đích = index ∪ untracked; nhãn `[tracked]/[staged]/[untracked]`; `--staged-only`/`V2_CHECK_UNTRACKED` để chẩn đoán; **21** ca hồi quy; wall 12,6–15,8 s |
| KT-88 | **K6** LẦN ĐO CUỐI: ma trận 24 boot + soak 1.000 + 6 tiêu chí trên ảnh build từ HEAD | KT-87 | 3 h | DONE | `f82a8560`; HEAD đóng băng `cf96eb9b`; ảnh `291502d2`/205.440/`text=130062`; **6/6 tiêu chí ĐẠT**; ma trận 21/21; soak 0/1000; persist 7/7; 3 hạn chế ghi thẳng (soak trên ảnh **không** feature) |
| KT-90 | **K6** soak 1.000 trên **ảnh phát hành** (có shell+FS) — bằng chứng **thêm** | KT-88 | 1,5 h | DONE | `a6ed9b37`; **1000/1000 ok, fail=0**, wall 355 s, mỗi vòng có `shell>`+`help`+`exit` sạch, `qemu_con_sot 0/0`; FS dùng volume RAM (soak không gắn `-drive`) |
| KT-89 | **K6** cổng theo **nội dung build-inputs** (D2-127) | KT-88 | 2 h | DONE | `1a9173d8`+`399f24c6`; ảnh in `# build: inputs=<16hex> dirty=<0|1>`; danh sách 260 mục sinh từ `cargo metadata`; **5/5 ca âm + 1 ca dương** (`kt89-neg.log`); trừ `v2/agent`/`v2/docs` là quyết định tạm A1-188 chờ A0 (B6) |
| KT-85 | **K6** chặn commit bằng máy: worktree mỗi task + hook `VIFAST_ROLE` (D2-123.4) | KT-80 | 1 h | DONE | 1d98835b; hook ở `.git/hooks` (common dir ⇒ chặn mọi worktree), fail-closed; 3 ca âm PASS; lead tự thử: `env -u VIFAST_ROLE git commit` ⇒ DENY |
| KT-85b | **K6** hook `reference-transaction` + `commit-msg` (D2-125.2) | KT-85 | 1 h | DONE | a6e216ec; 4 hook ở git-common-dir; ca âm agent2 cherry-pick/merge/reset ⇒ DENY, `refs/heads/main` không đổi; cổng git ≥ 2.28; `install.sh --check` PASS |
| KT-86 | **K6** nhúng `git rev` + dirty vào ảnh, harness từ chối đo nếu lệch (D2-125.3) | KT-85 | 2 h | DONE | 19b36457; `# build:` là `const` **trong byte ảnh**; `vbrev_gate` ở 4 runner, từ chối đo ⇒ mã thoát riêng; 5 ca âm; `repro-build.sh` dừng nếu byte ảnh không chứa danh tính; nợ A1-184 (remap `$RUSTC` lồng) giữ nguyên |
| KT-84 | **K6** đo trên ảnh ĐÓNG BĂNG (D2-122.6) | KT-86 | 3 h | DONE | `ae5f2048`: `task-target.sh` + `pin-artifact.sh` (fail-closed) + docs; **chưa** cưỡng chế bằng cổng (cần A0) |
| KT-201 | **P0** review đối kháng vòng 4 luật `wake`/`include!`/`#[path]` | KT-194 | 3 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L40 — **KHÔNG ĐẠT**; đường lọt **SỐNG** `macro_rules!` passthrough `$i:item` + `include!` trong **lời gọi macro** (`cargo check` RC=0, checker `errors=0` hai chế độ); 2 lọt tiềm ẩn (L2/L3); 5 báo oan chưa ghi; 7 khẳng định hợp đồng sai/thiếu; selftest 118/118 xác nhận; journal `JOURNAL/kt201-review-vong4.md` |
| KT-202 | **P0** vá đường lọt lời gọi macro + `:item/:expr/:stmt/:block/:meta` | KT-201 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: vá lời gọi macro (`_scan_macro_calls`) + `$…:<frag>` `{tt,item,stmt,block,meta}`; repro L40 **ĐỎ** hai chế độ; selftest 128→**144**; **nhưng** reviewer L44 (KT-208) tìm tiếp đường lọt #10 ⇒ **luật chưa đóng**; xem KT-211/KT-216 |
| KT-203 | **P1** review độc lập KT-182 (bản đồ trợ giúp cùng tệp) | KT-182 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L41 — **ĐẠT CÓ ĐIỀU KIỆN**; số đo tác giả **xác nhận** bằng parser độc lập (1019/996/23/0/0/155/0); báo oan 0; F1 ca 35/36 không răng, F2 `cfg(test)` trên item bị loại oan, F3/F5 nhỏ; journal `JOURNAL/kt203-review182.md` |
| KT-204 | **P1** review độc lập KT-183 (`cd` thư mục sâu) | KT-183 | 2,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L42 — **ĐẠT CÓ ĐIỀU KIỆN**; ảnh tái lập **trùng** `639ca6d1`; 18/18 khối + 8/8 ca; **2 lỗ hổng ca kiểm** (M2 `/` cuối, M4 biên `MAX_PATH`) đi qua **toàn harness**; 2 lỗi journal tác giả; `v2 check` T-INFRA (SIGTRAP) — không quy cho KT-183 |
| KT-205 | **P1** vá hai lỗ hổng ca kiểm KT-183 (M2 `/` cuối, M4 biên `MAX_PATH`) | KT-204 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: đóng M2 (`cd /docs/sub/` có `/` cuối) + M4 (biên `MAX_PATH` 48 B) — ca mới `kt183-bien-path`; đột biến M2/M4 **ĐỎ**; hoàn nguyên **19/19** khối PASS, 78 PASS/0 FAIL; ảnh `639ca6d1` **không đổi** (chỉ sửa harness) |
| KT-206 | **P1** vá F1/F2 của KT-182 (ca selftest có răng + `cfg(test)` trên item) | KT-203 | 1,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: fixture ca 35/36 thêm hàm sản phẩm cùng tên **sau** mod + định nghĩa không răng ở `tests/khac.rs` ⇒ xoá bản đồ cùng tệp thì **cả hai ĐỎ**; `is_cfg_test_attr`/`parse_cfg_impls`/`in_test_scope` nhận `cfg(test)`/`cfg(all(test,…))` trên **item**; selftest 36→**40/40**; cây thật 0 đổi |
| KT-207 | **P0** T-INFRA: `rules` chạy trong tiến trình bash **mới** (SIGTRAP bash 3.2) | KT-201 | 1 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: subshell `( … )` là fork ⇒ **thừa hưởng** heap hỏng sau `clippy`; nay `bash v2/bin/v2 rules --root` (tiến trình mới); 3/3 lượt `v2 check` xanh dưới tải; journal `JOURNAL/kt207-sigtrap-rules.md` |
| KT-208 | **P0** review độc lập KT-202 (vá đường lọt lời gọi macro) | KT-202 | 2,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L44 — **KHÔNG ĐẠT**; đường lọt **SỐNG thứ mười**: `include!` ở **vị trí biểu thức** trong nhóm `( … )`/`[ … ]` (`cargo check` RC=0, checker `errors=0` hai chế độ, 6 biến thể); `_walk_items` chỉ đệ quy `{…}`, `_scan_macro_calls` chỉ kiểm nhóm sau `!`; 6 khẳng định hợp đồng sai/thiếu; selftest 144/144 + 16/16 ca có răng; journal `JOURNAL/kt208-review202.md` |
| KT-209 | **P1** đóng L2 (phạm vi `wake` theo mẫu + `build.rs`) và L3 (`include!` trong `v2/lib`) | KT-201 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: phạm vi `wake` theo **mẫu** (`v2/kernel/<crate>/src/**` + `build.rs`, `v2/sim/<crate>/src/**` + `build.rs`); phân giải `include!`/`#[path]` trong `v2/lib`; 5/5 + 4/4 đột biến **ĐỎ**; 4/4 đối chứng dương XANH; selftest 118→**131** |
| KT-211 | **P0** quét `include!` **toàn cục theo token** (mọi tầng nhóm, mọi vị trí) | KT-208 | 2,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: quét **token toàn cục** cho `include!`/`#[path]` (mọi tầng nhóm `{`/`(`/`[`); repro L44 6 biến thể **ĐỎ** cả hai chế độ; repro L40 **ĐỎ**; 5/5 đối chứng dương XANH; selftest 157→**175**; `v2 check` PASS |
| KT-213 | **P0** vá **gốc phân giải** `#[path]` cho tệp non-mod-rs (đường lọt #11) | KT-212 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-172)**: vá **gốc phân giải** `#[path]` (`_walk_items` nhận `mod_dir`, `_module_graph()`/`module_dirs()` dùng chung, `done` theo cặp `(tệp, mod_dir)`); repro #11 trước XANH/XANH → sau **ĐỎ/ĐỎ**; 4/4 đối chứng dương XANH; 0 báo oan; ma trận 28 biến thể, 16/16 dạng `#[path]` ĐỎ; selftest 157→**165** |
| KT-215 | **P0** vá bộ lọc `target` khớp nhầm `src/target/` (đường lọt #12) | KT-212 | 1,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: `_is_build_dir` thay **năm** chỗ `"target" in p.parts`; `MAILBOX_DEFS` thay lọc theo tên tệp; **đóng F2** (chế độ thường quét mọi tệp sản phẩm); repro #12 trước XANH → sau **ĐỎ** hai chế độ; đối chứng `v2/kernel/target/` vẫn bỏ qua; selftest 165→**176** |
| KT-212 | **P0** tấn công `#[path]`/`mod` (tìm đường lọt #11/#12) | KT-211 | 2,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L47 — luật **CÒN LỌT**; **#11** gốc phân giải `#[path]` sai với tệp non-mod-rs (rustc nạp tệp thật chứa `wake`, checker XANH hai chế độ); **#12** thư mục `src/target/`; ma trận 28 biến thể; khuyến nghị **không** quét toàn cục cho `#[path]`/`mod` (chúng là mục Rust) — chấp nhận |
| KT-214 | **P1** nguyên mẫu kiểm `wake` theo `--emit=dep-info` | KT-208 | 2,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: `check-wake-depinfo.py` bắt **11/11** đường lọt; stable 1.99.1, không `-Z`; 6,3 s ấm / 10,9 s lạnh; 20/20 selftest; khuyến nghị **bổ sung lớp 2**. **KT-224 (L53) review độc lập: KHÔNG ĐẠT** — 5 đường lọt cả hai lớp, 8 đường lọt riêng lớp 2, fail-open `.d` rỗng, parse `\ ` sai. **KT-227 (L53) vá P0-1..P0-4**; **đính chính**: "checker tĩnh 5/11" là số đo ở `dc6e01ae`, trên HEAD tĩnh bắt **11/11** |
| KT-216 | **P0** giải xung đột merge KT-211 + KT-213 + KT-215 | KT-215 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: giữ `_walk_all` **và** `_walk_items(mod_dir)`; `include!`/`#[path]` chỉ do `_walk_all` sinh cạnh; selftest **194/194**; 4 repro **ĐỎ** hai chế độ; 6/6 đối chứng dương XANH; `v2 check` PASS |
| KT-217 | **P0** nối lớp 2 (`dep-info`) vào `v2 check` | KT-214 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-173)**: thêm 2 `run_step` sau `wake paths --strict`; xoá `NGOAI_LE` của KT-214; lớp 2 **26,92 s** lạnh / **8,26 s** ấm; `v2 check` PASS. **KT-227 đính chính**: luận cứ "tĩnh 5/11" cũ — trên HEAD tĩnh bắt **11/11**; lớp 2 giữ vai trò **cơ chế độc lập** + phủ `v2/lib` |
| KT-218 | **P0** tấn công cuối hai lớp (tìm đường lọt #13) | KT-217 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L49 — **ĐÓNG ĐƯỢC sau KT-219**; tìm đường lọt **#13** (`#[cfg(test)]` trên nhánh `match`, lọt **cả hai** lớp); 13 đường lọt lớp 2 (đều bị lớp 1 bắt); **0** đường lọt lớp 1 sống; **0** báo oan mới; journal `JOURNAL/kt218-attack-final.md` |
| KT-219 | **P0** vá vùng test quá rộng trên nhánh `match` (đường lọt #13, lọt **cả hai** lớp) | KT-218 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-174)**: vá `_test_char_ranges` — vùng test kết thúc ở cuối **mục** (`,`/`;` cùng mức), không phải tới `{` kế tiếp; repro #13 trước XANH → sau **ĐỎ** cả hai lớp; 15 dạng test thật + 7 ca ngoài cây **XANH**; selftest 194→**246**; `v2 check` PASS |
| KT-221 | **P0** T-INFRA: `rules` bỏ process substitution (SIGSEGV bash 3.2) | KT-220 | 1 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: `gate --fast` đỏ `Segmentation fault: 11` ở `luật cấu trúc`; tách `_rules_impl`, ghi `find`/`grep` ra tệp tạm; output **giống hệt**; 10/10 lượt xanh; `run.sh` 6/6 |
| KT-222 | **P0** reviewer độc lập cuối trên HEAD | KT-221 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-177)**: reviewer L51 — **ĐÓNG ĐƯỢC** trong phạm vi 7 đường lọt đã chọn (7/7 **ĐỎ**, 8/8 đối chứng dương XANH, gate 52/52 bước, K6 khớp); **nhưng** D2-178 rút tuyên bố này vì KT-224 tìm 5 đường lọt khác |
| KT-223 | **P1** reviewer độc lập KT-211 | KT-216 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-176)**: reviewer L52 — **ĐẠT CÓ ĐIỀU KIỆN**; 6/6 biến thể L44 **ĐỎ**; repro L40 **ĐỎ**; **4 báo oan** (`stringify!(include!)`, `include != 0`, `self.include != 0`, trailing comma); **1 đường lọt một phần** (`.txt`); 2 ca selftest không răng ⇒ KT-225 |
| KT-224 | **P0** reviewer độc lập KT-214 (lớp 2) | KT-217 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-178)**: reviewer L53 — **KHÔNG ĐẠT**; **5 đường lọt lọt CẢ HAI lớp** (B3b `ktest`, B12–B15 `v2/lib`); 8 đường lọt riêng lớp 2; **fail-open** `.d` rỗng; parse `\ ` sai; luận cứ tĩnh 5/11 đã cũ ⇒ KT-227 |
| KT-225 | **P0** vá 4 báo oan + đường lọt `.txt` + răng B29/B30 | KT-223 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-176)**: 4/4 báo oan **XANH**; `.txt` chọn (a) — **ĐỎ** cả hai lớp; B29/B30 có răng; thêm ca SHADOW (`stringify!` bị che ⇒ vẫn ĐỎ); selftest 246→**259**; `v2 check` PASS |
| KT-226 | **P1** lớp 2: bộ lọc `target` + đồng nhất allowlist producer | KT-222 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-177)**: lớp 2 dùng chung `CWP._is_build_dir`; `src/target/evil.rs` trước XANH → sau **ĐỎ**; đối chứng `v2/kernel/target/` XANH; producer chọn (a) — chỉ miễn định nghĩa; selftest lớp 2 20→**24** |
| KT-227 | **P0** vá 5 điều kiện của KT-224 (fail-open, parse, target, B3b, v2/lib) | KT-224 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-178)**: P0-1 fail-open `.d` rỗng/thiếu ⇒ **ĐỎ**; P0-2 parse `\ ` đúng; P0-3 `target` + producer theo định nghĩa; P0-4 **B3b** `#[cfg(feature="ktest")] mod` ⇒ **ĐỎ**, báo oan **0**; P1-5 `v2/lib` **không nguy hiểm** (DEBT-KT227-1); P1-6 sửa luận cứ cũ; **5 → 0** đường lọt cả hai lớp; selftest lớp 2 24→**27** |
| KT-228 | **P0** giải xung đột merge KT-225 + KT-226 + KT-227 | KT-227 | 1 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: 11 khối trong `check-wake-depinfo.py` — giữ KT-227 làm bản chính, giữ ca selftest KT-226; **không** xoá ca nào; selftest lớp 1 **259**, lớp 2 **31**; 25 ca KT-224: **0** đường lọt cả hai lớp, **0** báo oan; `v2 check` PASS |
| KT-229 | **P0** tấn công **theo lớp** (không theo danh sách) | KT-228 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-179)**: reviewer L54 — tìm **#14** `macro_rules!` lồng che tên macro an toàn (`matches!(m, wake, t)` ⇒ `m.wake(t)`), lọt **cả hai** lớp; `cargo check` RC=0; đối chứng âm **ĐỎ** |
| KT-230 | **P0** vá #14 (macro lồng che tên an toàn) | KT-229 | 45 ph | DONE | **Agent 1 quyết thay A0 (D2-180)**: `_collect_safe_macro_defs` đệ quy **mọi** tầng nhóm; `_shadowed_safe_macros` nhận `extra_files`; đích `v2/lib` vào `extra` (đóng luôn B12–B15 ở lớp 1); repro **ĐỎ** cả ba; 5 đối chứng dương XANH; selftest lớp 1 259→**281**, lớp 2 31→**34** |
| KT-231 | **P0** vá #15 (alias kiểu che `TcbState::Ready`) | KT-230 | 25 ph | DONE | **Agent 1 quyết thay A0 (D2-181)**: `_transition_calls` (token-tree, **dùng chung cả hai lớp**); alias ⇒ **ĐỎ**; `TcbState::<BiếnThểKhác>` ⇒ XANH; biến gán từ `Ready` ⇒ ĐỎ; `block_current(cur, to)` tham số ⇒ XANH (không báo oan); selftest lớp 1 281→**295**, lớp 2 34→**37** |
| KT-232 | **P0** tấn công fn-pointer `transition_tcb` | KT-231 | 30 ph | DONE | **Agent 1 quyết thay A0 (D2-183)**: reviewer L55 — **7/8 biến thể SỐNG** (fn-pointer, generic `F: Fn`, trait object, UFCS `M::transition_tcb`, `<Machine>::transition_tcb`); **2 lỗi gốc độc lập**: (a) `_transition_calls` chỉ khớp ident theo sau bởi `(`; (b) UFCS đọc sai đối số (`args[1]=t` thay vì state); báo oan **0** |
| KT-233 | **P0** tấn công `build.rs`/`$OUT_DIR` | KT-231 | 30 ph | DONE | **Agent 1 quyết thay A0 (D2-184)**: reviewer L56 — **5 dạng đề đều bị lớp 1 bắt** (nhờ lệnh cấm toàn phần `include!` không literal); **1 đường SỐNG**: crate thành viên workspace **ngoài ba gốc quét** (`v2/extra/evil` + `$OUT_DIR`); **1 báo oan** (`include!` `$OUT_DIR` tệp lành vẫn ĐỎ) |
| KT-234 | **P0** tấn công `Ready` qua tham số liên hàm | KT-231 | 30 ph | DONE | **Agent 1 quyết thay A0 (D2-182)**: reviewer L57 — **6/10 dạng SỐNG** (helper cùng tệp, helper tệp khác, hàm trả về `Ready`, `const`, `static`, closure); 4 dạng bị bắt đúng (`if/else`, `[Ready][0]`, `*&Ready`, comment); báo oan **0** |
| KT-235 | **P1** verify cuối trạng thái đêm | KT-231 | 30 ph | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: reviewer L58 — **13/15 mục khớp**; **2 lệch tài liệu** (STATE + báo cáo + phiếu ghi HEAD/DONE/selftest cũ) — **đã sửa**; gate log **52** bước khớp từng tên, không bước nào bỏ qua; K6 + ảnh khớp |
| KT-236 | **P0** sửa sổ theo D2-193 + lập registry đường lọt | — | 1 h | DONE | **Agent 0 (D2-193)**: khối trạng thái hiện tại duy nhất trong `STATE.md`; **WB-16/17/18 là P0 chặn nghiệm thu**, không phải nợ không chặn; `BYPASS-REGISTRY.md` mã ổn định `WB-NN` + ánh xạ tên cũ; tách task xong khỏi finding đóng; giữ N1 ở K5' |
| KT-237 | **P0** luồng **A** — inventory cửa ghi trạng thái + đề xuất API loại trừ `Ready` | KT-236 | 1,5 h | DONE | **Agent 0 (D2-192)**: reviewer A — **216** call-site (62 sản phẩm + 154 test), phân loại **216/216**; đề xuất `NonReadyTarget` (10 biến thể, không `Ready`) + 5 cửa `Ready` ngữ nghĩa; **WB-16 6/6 + WB-17 7/7 chặn ở biên dịch** (`E0308`/`E0631`); primitive thô private ⇒ `E0624` cả core+sim; chi phí **229 call-site/55 tệp/2 module mới**; **không đổi spec/**; **8 câu hỏi cho A0** (enum vs newtype, giữ tên, 2 cửa pub, block_current, task sửa checker, ca test, reply_with, Running) |
| KT-238 | **P0** luồng **B** — phạm vi `cargo metadata`/dep-info, đóng WB-18 | KT-236 | 3 h | DONE | **Agent 0 (D2-188)**: reviewer B — phạm vi sinh từ `cargo metadata` **22 crate bảo vệ / 1 từ chối rõ** (`vifast-agentctl`); **WB-18 ĐÓNG** (lớp 2 trước XANH → sau **ĐỎ** `E_DEPINFO_WAKE` tại `out/evil.rs:3`); fail-open **0** (bỏ `W_DEPINFO_SKIPPED`, thay bằng `E_DEPINFO_OUTSIDE`/`E_DEPINFO_SCOPE`/`E_DEPINFO_STALE`); `OUT_DIR` đúng lần build (3 ràng buộc); selftest lớp 2 **37 → 52**; `v2 check` PASS |
| KT-239 | **P0** luồng **C** — đóng gói repro WB-16/WB-17 + compile-fail có kiểm nguyên nhân | KT-236 | 2 h | DONE | **Agent 0 (D2-190 §2/§5)**: reviewer E (tiếp quản C) — **UFCS đã sửa** (đọc `args[2]` khi có tiền tố `::`, thiếu ⇒ fail-closed); **36 ca repro** (18 kịch bản × 2 chế độ) cho 6 dạng WB-16 + 6 biến thể WB-17; test âm **4/4 kiểm đúng nguyên nhân** (mã lỗi + thông điệp + đúng `relp:dòng (hàm)` + phân loại); **3 đột biến chứng minh assert có răng**; selftest lớp 1 **295 → 331**; `v2 check` PASS |
| KT-240 | **P0** luồng **D** — audit hợp đồng, thành phần dùng chung, ma trận review | KT-236 | 2 h | DONE | **Agent 0 (D2-189 §1, D2-190 §5)**: reviewer D — **10 thành phần dùng chung** (5 chỗ thiếu selftest chéo); hợp đồng **63/68 khớp**, 2 lệch, 3 không nguồn; **ma trận 4 nhóm/20 ô**; phát hiện tham số `tr` chết trong `_check_wake_text`; **3 lệch sổ đã sửa**; selftest 295/295 + 37/37, `v2 check` PASS |

**APPROVED 2026-11-17 Agent 0** — **D2-122**: (1) **KT-79 CÔNG NHẬN đạt đủ OD-K6-3** (2 máy × 2 đường dẫn = `c1072d72`), đóng KT-79; (2) **B1 chọn (B)**: ảnh sản phẩm = pipeline canonical, áp dụng ngay, xuất **2 tệp cùng lần build** (ảnh phát hành strip + ELF đủ symbol), `fd202276` chỉ giữ làm log “trước canonical, không dùng làm bằng chứng v2.0”; (3) `check-repro` vào **mỗi** gate (2 đường dẫn) + định kỳ `ps@ps`; (4) host Windows → nợ **v2.1**; (5) công nhận A1-173/A1-174/A1-175; (6) **sửa cấu trúc**: `CARGO_TARGET_DIR=<repo>/target-task/<mã>` mỗi task + đo trên `artifacts/<sha256>.bin` (⇒ **KT-84**); (7) thứ tự KT-77b → soak/matrix/persist trên ảnh canonical có shell → KT-82.
| KT-92 | **K6** đóng A1-184: `build.rs` truyền `--remap-path-prefix` cho cả ba chỗ gọi `$RUSTC` | KT-89 | 1 h | DONE | `4040e26d`; hai đường dẫn khác nhau ⇒ **cùng sha** `dd9f7050`; đối chứng âm (không vá) ⇒ khác sha; ảnh dương **không** còn chuỗi đường dẫn |
| KT-93 | **K6** đóng khe tin-cậy: script đo/build vào build-inputs | KT-89 | 45 m | DONE | `5ac0765b`; 260→263 mục; sửa `build-rev.sh` ⇒ dấu vân tay **đổi** |
| KT-94 | **K6** ảnh canonical mới sau KT-89/92/93 + kiểm nhanh | KT-92 | 30 m | DONE | `823b357e`; `dd9f7050…` 205.440 B, `inputs=3caf5e7d5f0a3321 dirty=0`; parity 6/6 |
| KT-95 | **K6** `pre-measure.sh` — kiểm trước khi đo | KT-84 | 1 h | DONE | `11e7e789`; 6 phép kiểm; 6 ca kiểm đã chạy; **chưa** nối vào runner (cần A0) |
| KT-96 | **K6** `soak.sh --image` — soak ảnh có sẵn | KT-95 | 45 m | DONE | `bffd0cd4`; **từ chối** ảnh có shell (0/N sẽ là giả); mặc định không đổi |
| KT-97 | **K6** kiểm lại sau KT-92/93/96 | KT-96 | 30 m | DONE | `cb3d406a`; ma trận **21/21**, persist PASS, parity 6/6 |
| KT-98 | **K6** `--selftest` cho `task-target.sh` + `pre-measure.sh` | KT-95 | 1,5 h | DONE | `f7527899`; 6/6 mỗi cái; **7 đột biến 7/7 bị bắt**; hành vi mặc định không đổi |
| KT-99 | **K6** soak 1.000 trên ảnh canonical mới | KT-94 | 30 m | DONE | `ef39b39e`; **1000/1000**, wall 533 s, `changed=0`, `qemu_con_sot 0/0` |
| KT-100 | **K6** vá 4 lỗ fail-open do review đêm | KT-93 | 1,5 h | DONE | `8d3310af`, `baf3b029`, `295f4414`; `missing-input`, `no-pgrep`, `--allow-unverified`, soak chỉ nhận timeout; ảnh mới `9643311d…` |
| KT-101 | **K6** conformance trên HEAD | KT-100 | 30 m | DONE | `aa7803e7`; `trace-compare` **61/61** dòng khớp sim ↔ QEMU; `scn-check` PASS |
| KT-114 | **K6** bốn lệnh shell mới (`time`/`uptime`/`sync`/`stat`) | KT-109 | 2 h | DONE | `11550366`; chỉ nhánh `cfg(shell_fs)` ⇒ ảnh test vẫn sáu lệnh; `CMDS` chín→mười ba |
| KT-115 | **K6** review **toàn dự án** | — | 3 h | DONE | `fd22dffc`; mười chín phát hiện (hai nặng, tám vừa, chín nhẹ) |
| KT-116 | **K6** vá hai test xanh giả (`ktest_con` seq, `ktest_core` nhãn WaitAddr) | KT-107 | 2,5 h | DONE | `c5071d32` (patch); mỗi mục có đột biến phía code làm test đỏ |
| KT-117 | **K6** `kt18_tcb`: bảng `ALLOWED` hết tự chứng minh vòng tròn | KT-107 | 1 h | DONE | `9ddffa0f`; so với bảng markdown trong tài liệu + tám bất biến suy từ thiết kế |
| KT-118 | **K6** vá PANIC kernel từ request U-mode (`proc_fs` tràn mảng) | KT-115 | 45 m | DONE | `82788a06`; dựng lại được `PANIC: index out of bounds`, vá một dòng |
| KT-119 | **K6** TAP 4 `ps` dùng trạng thái **thật** thay vì giải mã lại | KT-115 | 45 m | DONE | `5d271bc0`; đột biến chỗ đóng gói giờ làm TAP 4 đỏ |
| KT-120 | **K6** vá bản KT-112 của chính tôi (kiểm rỗng) + áp patch KT-116 | KT-115 | 30 m | DONE | `c5071d32`; thêm sàn độc lập `PS_MIN_ROWS` và ca selftest 3b |
| KT-121 | **K6** ảnh canonical sau vòng phát triển | KT-114 | 20 m | DONE | `fd22dffc`; `5e2ec219…` |
| KT-122 | **K6** bất biến pager thành TAP 36 + đính chính `k6-parity.md` | KT-115 | 45 m | DONE | `08d02c9b`, `40e2648d`; `sectors_per_page` viết literal |
| KT-123 | **K6** ba đính chính từ review chéo lô hai | KT-114 | 45 m | DONE | `484b6f98`; `ST_NOFS` trả lời ngay thay vì đứng năm mươi tư giây |
| KT-124 | **K6** ảnh canonical sau KT-122/123 | KT-123 | 20 m | DONE | `c2fdd297`; `64db17ea…` |
| KT-125 | **K6** hai lệnh nữa: `truncate`, `jobs` | KT-114 | 45 m | DONE | `6d6dd698`, `feb45790`; `CMDS` mười ba→mười lăm; ảnh `503be0f9…` |
| KT-81 | **K6** thiết bị giả nghiêm ngặt + bảng tài nguyên | KT-80 | 3 h | DONE | bắt lại 2 lỗi driver cũ ở mô hình; **Agent 0 mở 2026-11-18 (Q7)**. **ĐÓNG 2026-11-18 (phiếu lượt năm §A5)**: (a) `build_request` trong `vifast-virtio` là **mã dựng request của driver thật** (driver gọi đúng hàm ấy), 11 ca host phủ đọc/ghi/flush/nhiều sector/sector cuối, hai đột biến tái tạo lỗi v1 #1/#2 ⇒ `FlushNotNegotiated`/`LengthMismatch`; (b) quét toàn repo: 5 hằng viết tay của 6 vùng nay **dẫn xuất** từ `REGIONS` + 3 `const _` kiểm khớp lúc biên dịch; 15 chỗ còn lại là chú thích hoặc **VA user trùng giá trị nhưng khác khái niệm** (ghi lý do trong JOURNAL) |
| KT-82 | **K6** văn bản đóng K6 (a hoặc b) + caveat P20 | KT-89 | 3 h | DONE | **Agent 0 chốt Q1 = (B), Q2 = (A) 2026-11-18**; văn bản chính thức: [`reports/K6-CLOSURE.md`](reports/K6-CLOSURE.md) — K6 **ĐẠT** gắn ảnh `a0287e4b…`, ba hạn chế + caveat P20 nằm trong văn bản |
| KT-83 | **K6** mở K7 "Đóng lỗi v1" + chốt bảng 7 lỗi | KT-82 | 3 h | DONE | **Agent 0 chốt Q7 = (A) 2026-11-18**: mở K7, thứ tự **N2 → N3 → N7 → N1**; N5 gắn K5'; luật đóng lỗi ba vế (đỏ-trên-mã-cũ / xanh-trên-mã-mới / đột-biến-làm-đỏ-lại) |
| KT-127 | **K6** giải thích `fault_seen` + checker soak fail-closed + ghi soak vào sổ | KT-126 | 1 h | DONE | `07b244a7`; `fault_seen` = `Exit` của shell qua kênh fault (D2-25); đột biến `+=2` ⇒ `fault-seen=2(want=1)`; soak 1000/1000 ảnh `a0287e4b…` vào `STATE.md` |
| KT-128 | **K6** ca hồi quy **âm** cho cổng build-inputs (Q3) | KT-127 | 1 h | DONE | `25138df7`; sửa một byte mã kernel ⇒ dấu vân tay đổi **và** cổng REFUSE `inputs-mismatch`, kèm đối chứng dương; phát hiện selftest **đã đỏ từ trước** (không cổng nào chạy) ⇒ nối vào `v2 check` |
| KT-129 | **K6** đo lại sáu tiêu chí trên **đúng ảnh** Agent 0 đã chốt | KT-128 | 1 h | DONE | log `v2/agent/JOURNAL/kt129-logs/`; boot ba cấu hình hart, parity mười hai lệnh `active`, ma trận, persist, ngưỡng, harness scan |
| KT-130 | **K6** `spec/v1-parity.toml` sáu mục `deferred-K7` → `active` + ca kiểm chín lệnh trên ảnh sản phẩm (Q4) | KT-129 | 2 h | DONE | sáu mục `time`/`uptime`/`sync`/`stat`/`truncate`/`jobs`; `shell-edge` thêm ca `new-cmds-ok` + `new-cmds-edge` (mỗi lệnh một ca đúng + một ca biên) |
| KT-131 | **K6** đưa năng lực "soak ảnh phát hành" vào harness chính thức | KT-127 | 2 h | DONE | `be0e5ab8`+ (KT-135); `soak.sh --features` build bằng **pipeline canonical** ⇒ sha trùng ảnh phát hành; mỗi vòng đòi `shell>` + `help` + `exit` sạch, thiếu một dấu hiệu ⇒ đỏ (đột biến đã chứng minh) |
| KT-136 | **K6** soak **đĩa thật** 100 boot + biến thể ngắt giữa lúc ghi (C5) | KT-131 | 3 h | DONE | **Agent 0 chốt C5 = (A) 2026-11-18**: mỗi vòng ghi → reboot → đọc lại → fsck sạch; thêm ≥ 1 ca kill QEMU giữa lúc ghi ⇒ boot sau phải phục hồi nhất quán |
| KT-137 | **K7** `cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối (C3) | KT-136 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-133): điều kiện bắt đầu B4 đã đóng bởi KT-132 ⇒ mở ngay, xếp sóng hai sau N2** (chạm phân giải đường dẫn). **Agent 0 chốt C3 = (A), cuối K7**: ca kiểm `cd` không tham số, `cd` đường không có, `cd ..`, `cd` vào tệp, `pwd` sau `cd`; **mọi** lệnh có tham số đường dẫn phải chạy lại ca cũ, không đỏ — **XONG 2026-11-19** (L5, merge `f09c89f9`): `cd`/`pwd` + cwd + đường dẫn tương đối, `CMDS` 15→17; 11 phép kiểm `kt137-cdpwd` + selftest 42/42; ba vế đủ (ảnh cũ `# shell: ?`; ảnh mới 11/11; đột biến bỏ `kind=2` ⇒ 3 phép kiểm đỏ); `spec/v1-parity.toml` chỉ đổi `status` + `expect` của hai mục (D2-138). **Hạn chế**: FS sản phẩm chỉ có `/` nên `cd` vào thư mục con chưa kiểm được |
| KT-133 | **K6** bảng đối chiếu số ↔ khối `scope=` cho ba tài liệu K6 (Q6) | KT-129 | 1 h | DONE | **Agent 0 Q6 = (A) 2026-11-18**: mọi trường đo của `k6-exec`/`k6-persist`/`K6-DOD` nay có bảng chỉ ra khối `scope=` chứa số nguyên văn; prose vẫn dùng chữ nên luật số một không cần miễn trừ |
| KT-132 | **K6** kiểm chứng `cd`/`pwd` trên v1 tại tag đóng băng (Q5) | KT-129 | 1 h | DONE | `647a04bc`; build + boot v1 tại tag, chạy thật `cd /bin` rồi `pwd` ⇒ in `/bin` (log `JOURNAL/kt132-v1-cdpwd.log`) ⇒ **(A) đưa vào danh sách**; giữ `deferred-K7` vì shell v2 chưa có builtin |
| KT-146 | **K7** dọn tệp dở sau cắt điện + tài liệu FS + nợ v2.1 (A6 lượt 5) | KT-136 | 2 h | DONE | **Agent 0 phiếu lượt năm §A6**: giữ nguyên ngữ nghĩa `fs.create`; `persist.sh --crash` thêm bước **dọn tệp dở** rồi đòi pha 7 **xanh** + chạy tiếp tới cuối (`phases=7 ok=7`); đột biến bỏ bước dọn ⇒ 3 dòng đỏ; `k6-product-fs.md` §6b ghi journal theo từng thao tác + nợ v2.1 |
| KT-147 | **K7** `cat`/`stat` lên thư mục, `time` hai tham số (4c lượt 5) | KT-136 | 1 h | DONE | **Agent 0 phiếu lượt năm §B3.4c**: đo hành vi thật trên ảnh sản phẩm (`cat /` ⇒ `fs loi code=8`, `stat /` ⇒ `kind=2 size=512`, `time 5 6` ⇒ `# shell: ?`) — cả ba **an toàn** nên **ghim**, không phải hàng chờ; ca 15 `k7-thu-muc` 4 phép kiểm + ca toàn vẹn `/README`; selftest 36/36 |
| KT-145 | **K7** siết `algo=` ở **đường đo** (A4 lượt 5) | KT-143 | 2 h | DONE | **Agent 0 phiếu lượt năm §A4**: chỉ `status.sh`/`pin-artifact.sh` được nhận dạng cũ; `pre-measure.sh`/`soak.sh`/`shell-edge.sh`/`repro-build.sh` TỪ CHỐI ảnh thiếu `algo=` hoặc `algo` khác hiện hành; bỏ hẳn `--allow-legacy`; `build-identity.sh` (5 ca l1..l5) + `algo-strict.sh` (15 ca: 9 hành vi + 6 cấu trúc); hai đột biến nới ⇒ đỏ |
| KT-144 | **K7** `write` thiếu nội dung báo lỗi + ghim nhóm ưu tiên toàn vẹn dữ liệu | KT-136 | 2 h | DONE | **Agent 0 phiếu lượt bốn**: `write <path>` thiếu nội dung nay in `# shell: thieu noi dung`, tệp **không** bị đổi (trước: cắt về 0 byte); bốn ca ưu tiên **ghim** hành vi thật (`write`/`truncate` lên thư mục ⇒ `code=8`, `truncate` thiếu cỡ, `write` path rỗng) + ca toàn vẹn; ba vế đủ (log cũ đỏ, ảnh mới xanh, đột biến bỏ nhánh ⇒ đỏ) |
| KT-148 | **K7** B7: sửa test `smp` + đưa L0 workspace kernel vào cổng | KT-81 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-128)**: chọn (A) sửa **test + chú thích** `smp.rs` cho khớp `HART_STRIDE` (linker đã cấp syscall stack từ KT-49/KT-66; sửa mã sẽ hẹp vùng per-hart ⇒ `mmu::init` panic); thêm bước `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common` vào `gate --fast`; **đột biến** khôi phục công thức cũ ⇒ bước mới đỏ. `vifast-virtio` **không** thuộc workspace kernel nên không thêm lần hai — **XONG 2026-11-19** (L4, merge `bb798c3d`): test `smp` dùng đủ bốn số hạng + chú thích `smp.rs`; bước `L0 kernel workspace` đã nằm trong `gate --fast`; đột biến khôi phục công thức cũ ⇒ đỏ (`left: 86016 right: 118784`); reviewer L6: **ĐẠT** |
| KT-149 | **K7** N2 (T26) — read-your-own-write: vế 1 trên ảnh v1, 50 boot v2, đột biến | KT-81 | 4 h | DONE | **Agent 1 quyết thay A0 (D2-129)**: tiêu chí gồm **bốn** điều — (i) ảnh v1 tại `v1-archive` với `MAX_IO=256` trong worktree nháp, 50 boot `-smp 2`, đếm đỏ theo đèn T26 của v1; (ii) 50 boot ảnh sản phẩm v2, 0 đỏ; (iii) test host `t26_read_your_own_write` 1000 seed xanh; (iv) bỏ `flush()` trong `commit()` ⇒ (iii) đỏ. Harness `v2/test/boot/t26.sh` có `--check-image` + `--selftest` ca âm — **XONG 2026-11-19** (L1, merge `4e6dcea1`): `v2/test/boot/t26.sh` (check-image + boots + selftest g1..g4); 50 boot v2 ⇒ `boots=50 green=50 red=0`; 50 boot v1 **trước vá** `5fc7b3ff` ⇒ 5/50 T-FS đỏ (tag đã có r296b nên 0/50 — giữ làm đối chứng dương, D2-135); host 1000 seed xanh; đột biến bỏ `flush()` ⇒ 2 test đỏ; reviewer L6: **ĐẠT CÓ ĐIỀU KIỆN** (vế 1 trên mã trước vá, không phải tag) |
| KT-150 | **K7** N3 (T31) — pager giao đúng một lần + 4 client + đột biến phát lại | KT-81 | 3,5 h | DONE | **Agent 1 quyết thay A0 (D2-130)**: checker đếm `USER FAULT … addr=<X>` đòi **đúng 1** mỗi fault; ca `clients=4`; đột biến "phát lại Call khi Resume" ⇒ đỏ; vế 1 trên mã v1 (ảnh nếu build được, không thì test host, không thì trích log v1 kèm `file:dòng` và **không** tuyên bố đóng) — **XONG 2026-11-19** (L2, merge `acf607b3`): TAP 37 `pager giao dung mot lan` (3 vế, đếm theo từng địa chỉ) + `PagerSession`/`Ledger` host + ca 4 client mức host; đột biến phát lại Call ⇒ TAP 37 đỏ, TAP 30..36 vẫn xanh; vế 1 = test host trên mã v1 trước r309 (`left: 2 right: 1`), **không phải ảnh** (D2-136); reviewer L6: **ĐẠT CÓ ĐIỀU KIỆN** (checker host tự đối chiếu sổ của chính harness; 4 client mức kernel là nợ) |
| KT-151 | **K7** N7 (Audit Q1-A) — một cơ chế wake chung cho mọi đường | KT-81 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-131)**: quét **mọi** đường đánh thức TCB, đòi tất cả đi qua `vifast-sync::Mailbox`; test chạy đánh thức chéo hart đúng một lần; đột biến thêm đường wake vòng qua mailbox ⇒ đỏ; vế 1 = checker chạy trên mã v1 **đỏ** vì > 1 cơ chế, kèm `file:dòng` — **XONG 2026-11-19** (L3, merge `a9460f0c`): `v2/tools/check-wake-paths.py` (7 ca selftest, nối `v2 check`) + `tests/kt151_wake.rs` (5 ca) + đột biến; checker đỏ trên v1 với **3** cơ chế (aarch64 GIC SGI / riscv64 IPI / x86_64 LAPIC). **N7 KHÔNG ĐÓNG** (D2-137): `vifast-sync::Mailbox` không được kernel dùng, **16 đường** wake trực tiếp, và reviewer L6 phá được cổng (thêm đường wake thứ hai ⇒ cổng vẫn xanh, chỉ `--strict` đỏ) ⇒ mở **KT-154** |
| KT-152 | **K7** N1 (T33a) — đơn điệu sau migrate + conformance chéo hart | KT-81 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-132)**: v2.0 chỉ riscv64 nên tiêu chí "600 boot x86_64" không áp dụng; làm kịch bản sim "đơn điệu sau migrate" + conformance chéo hart + đột biến; vế 1 = trích nguyên văn `[V1:ĐO]` + đường mã v1 kèm `file:dòng`, nhãn "chưa đo lại"; **không** đột biến nào làm đỏ ⇒ **không đóng N1**, gắn K5' — **XONG 2026-11-19** (L4, merge `bb798c3d`): `v2/sim/vifast-sim/src/clock.rs` + `tests/kt152_n1.rs` (4 ca: đơn điệu sau migrate, reset hart muộn, conformance chéo hart, ca âm API); **hai** đột biến đỏ (đồng hồ theo hart; `reset_hart` về 0); vế 1 = trích nguyên văn `[V1:ĐO]` + `file:dòng` v1, nhãn chưa-đo-lại; reviewer L6: **ĐẠT CÓ ĐIỀU KIỆN** (lưới chắn chỉ ở mức mô hình) |
| KT-153 | **K7** review độc lập năm task đêm (đột biến độc lập + kiểm số + kiểm vế 1) | KT-152 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-134)**: reviewer L6 phá bằng chứng của KT-148/149/150/151/152; kết luận KT-148 **ĐẠT**, bốn task còn lại **ĐẠT CÓ ĐIỀU KIỆN**; phát hiện nặng nhất: cổng `check-wake-paths.py` **không** bắt được đột biến thêm-đường-wake-thứ-hai ⇒ mở KT-154 |
| KT-154 | **K7** vá N7-2: mọi đường wake tôn trọng hart `home` + đưa `--strict` vào cổng | KT-151 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-137)**: 16 đường `blocked/sleeping → Ready` trực tiếp không qua `wake_step`; vá để TCB có `home` khác hart hiện tại nhận `MailboxMsg::Wake` về hộp thư `home`; test đỏ-trước + đột biến; `--strict` về 0 thì đưa vào `v2 check`. **Không xong trước hạn ⇒ N7 ở lại K7, không tuyên bố đóng** |
| KT-155 | **K7** đóng nốt N7: 13 đường sim + allowlist tường minh + `--strict` vào cổng + N7-4 | KT-154 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-140)**: vá 13 đường `v2/sim` qua `wake_from` (đối xứng kernel); thay heuristic bằng **allowlist tường minh** (17 chỗ có lý do + 6 cửa wake, fail-closed hai chiều); đột biến `reply_with` **bị bắt** (`E_STRICT_TRANSITION … errors=2 EXIT=1`); `--strict` **2 → 0** và **đã vào `v2 check`**; `wake_from_unlink` cho `wake_addr` (N7-4 xong); 107 suite xanh |
| KT-156 | **K7** đo lại sáu tiêu chí K6 trên ảnh sản phẩm cuối (bằng chứng đóng K7) | KT-155 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-134)**: ảnh `e7149448…` (`inputs=0b8016e6cb6038d5`, khớp dấu vân tay cây); **F1–F6 ĐẠT** — `shell>` ở 1/2/4 hart; parity 2×14 + reboot-read + persist 10 boot; ca âm ảnh tắt shell `1287611c…` (82.552 B, 0 dòng `shell>`) ⇒ parity đỏ `exit 5`; ma trận **21/21** + 3 boot F1 = **24**; soak **1000/1000**; `file=209536 data+bss=223696 budget=262144`; quét harness 0 marker. Tổng **40 m 58 s**. Năm hạn chế ghi trong `JOURNAL/kt156-k7-do-lai.md` §6 |
| KT-157 | **v2.1** N3 nợ: bốn client **mức kernel** cho pager (CSpace + bảng trang riêng) | KT-150 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-145)**: dựng bốn client U-mode trong ảnh `ktest-user`, mỗi client fault trên VA riêng; checker `check_once` mở rộng sổ theo `(client, va)`; đột biến phát lại Call ⇒ TAP 37 đỏ. **Luật cứng**: không dựng được 4 client trong ngân sách thì **không** tuyên bố đóng nợ — bàn giao phần đã làm + ghi rõ còn thiếu |
| KT-158 | **v2.1** KT-137 nợ: seed thư mục `/docs` + `/docs/note.txt` vào FS sản phẩm | KT-137 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-146)**: `proc_fs::seed` thêm thư mục + tệp; ca `shell-edge.sh` 16 mở rộng (`cd /docs` ⇒ `pwd` `/docs`, `ls` thấy `note.txt`, `cat /docs/note.txt`, `cd ..` ⇒ `/`, `cd /docs/..` ⇒ `/`); **đột biến** "`pwd` in hằng `/`" **phải** bị bắt (điều D2-138 ghi là chưa phân biệt được); parity `ls` chỉ khớp `n=`/`path=` nên không phá |
| KT-159 | **v2.1** N7 nợ: `wake()` chỉ-dùng-cho-test + luật checker + ghim hai ngữ nghĩa | KT-155 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-144)**: `wake()` không có call site sản phẩm (chỉ ảnh test + test sim) ⇒ giữ post-only, doc rõ, thêm luật `--strict` cấm `wake(` trong `vifast-core/src`/`vifast-sim/src` ngoài `#[cfg(test)]`; test ghim `wake()` chéo hart **không** đổi trạng thái ngay + đúng một `Wake`, `wake_from()` **đổi ngay** + đúng một `Wake` |
| KT-160 | **K4'** bước 1: `vifast-plat-aarch64` — `PageTableFormat` 4 mức 4 KiB host-testable | KT-156 | 3 h | DONE | **Agent 1 quyết thay A0 (D2-147)**: crate mới `v2/kernel/vifast-plat-aarch64` với `PageTableFormat` 39-bit VA/48-bit PA + `Mmu` **thuần dữ liệu** (không asm), test host map/unmap/quyền/không rò rỉ frame; **cấm** `cfg(target_arch)` trong `vifast-core`/runtime; **không** boot aarch64, **không** tuyên bố K4' đạt |
| KT-161 | **v2.1** review đối kháng sóng một (phá bằng chứng KT-157..160) | KT-157, KT-158, KT-159, KT-160 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-148)**: reviewer độc lập thử **phá** từng bằng chứng — đột biến riêng, kiểm số dương, kiểm vế 1; kết luận ĐẠT / ĐẠT CÓ ĐIỀU KIỆN / KHÔNG ĐẠT kèm `file:dòng` |
| KT-162 | **v2.1** báo cáo đêm vòng hai + chốt nợ v2.1 | KT-161 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-148)**: `BAO-CAO-DEM-2026-11-19-VONG2.md` — quyết định D2-144..149, bốn sub-agent, bằng chứng từng task, hạn chế, nợ còn lại, trạng thái cuối |
| KT-163 | **v2.1** máy kiểm "test có răng" — chống lớp lỗi xanh-giả | KT-155 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-150)**: `v2/tools/check-test-teeth.py` bắt bốn lớp chắc chắn (`assert!(true)`/hằng-đúng, `#[ignore]`, test không có khẳng định nào, `let _ =` nuốt `Result`), allowlist tường minh, `--selftest` log giả; chỉ vào `v2 check` nếu PASS trên cây hiện tại |
| KT-164 | **K4'** bước 1b: trait `PageTableFormat`/`Mmu` lên crate dùng chung | KT-160 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: trait + kiểu đi kèm sang `v2/lib/vifast-pte/src/mmu.rs`; `vifast-plat-aarch64` chỉ còn impl; **Sv39 cũng impl** + **một** thân test generic `kiem_tung_dinh_dang<F>()` chạy cả hai định dạng; đột biến `AP[2]` (aarch64) và `PTE_VALID` (Sv39) đều đỏ; L0 118 passed |
| KT-165 | **v2.1** vá 6 lỗ hổng luật `wake()` do reviewer L6 phát hiện | KT-159 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: phạm vi thêm crate ảnh sản phẩm + **mọi** crate `vifast-plat-*`; bắt alias/fn-pointer, block comment, cắt dòng; sửa `test_line_ranges` nuốt `mod` sản phẩm; selftest **26/26**; checker **cũ** trên cùng cây đột biến cho `errors=0` ⇒ sáu lỗ hổng là thật |
| KT-166 | **v2.1** vá lỗ hổng bằng chứng KT-158 + KT-160 do reviewer L6 phát hiện | KT-158, KT-160 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: ca 16 thêm `cat note.txt`/`stat note.txt`/`ls .` **tương đối** trong `/docs` (14→15 phép kiểm); đột biến `resolve_path` bỏ `Cwd` ⇒ **đỏ**; `host.rs` thêm bit literal ARMv8-A viết tay + `translate` từ chối bảng-mức-lá (22→24 test); L0 thêm `-p vifast-plat-aarch64` |
| KT-167 | **K4'** bước 2: `vifast-plat-aarch64` thanh ghi hệ thống + asm nguyên thủy | KT-164 | 2,5 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: `TCR_EL1`/`MAIR_EL1`/`SCTLR_EL1`/`TTBR*`/`VBAR_EL1`/`ESR_EL1`/`DAIF` encoding host-testable + `TLBI`/barrier asm có stub host; test khẳng định **giá trị tuyệt đối** viết tay từ ARM DDI 0487; ba đột biến đỏ; **không** boot, **không** tuyên bố K4' đạt |
| KT-168 | **v2.1** review đối kháng KT-165 + KT-166 (lách thứ 7, bit descriptor thứ 5) | KT-165, KT-166 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: thử lách luật wake bằng trait method/macro/`include!`/`init/`; thử phá ca 16 bằng `./note.txt`/`../docs/note.txt`; đột biến `DESC_AF`/`DESC_PXN`/`DESC_UXN`/`DESC_SH`; kết luận ĐẠT / ĐẠT CÓ ĐIỀU KIỆN / KHÔNG ĐẠT |
| KT-169 | **K5'** audit sẵn sàng x86_64 (không viết mã) | KT-164 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: `v2/agent/reports/K5-X86-READINESS.md` — đã có / còn thiếu (kèm `file:dòng`) / thứ tự thi công / rủi ro; tuân luật số liệu `verify-numbers.py`; **không** tuyên bố K5' đạt |
| KT-170 | **v2.1** mở rộng máy kiểm test có răng: `#[should_panic]` trần + trợ giúp tệp khác | KT-163 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-154)**: lớp (e) `E_SHOULD_PANIC_BARE` (`#[should_panic]` không `expected` ⇒ đỏ); bản đồ trợ giúp toàn cây; selftest thêm ca; đột biến `#[should_panic]` trần ⇒ `v2 check` đỏ |
| KT-171 | **v2.1** vá vòng 3 luật `wake` (6 lớp lọt L11 phát hiện) | KT-165 | 2 h | DONE | **Agent 1 quyết thay A0 (D2-155)**: `test_only_modules` dựng **đồ thị module** thay miễn-theo-tên/đường-dẫn; phân giải `include!`/`#[path]` (đích ngoài phạm vi ⇒ đỏ fail-closed); quét thân `macro_rules!` trong `v2/lib/**`; miễn local-binding theo **khối** + token `wake` làm đối số macro không miễn; selftest **39/39**; sáu đột biến thật đều đỏ, checker KT-165 trên cùng đột biến cho `errors=0` |
| KT-172 | **v2.1** đóng nốt lỗ hổng `is_test_file_heuristic` cho luật 1–5 | KT-171 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-157)**: đổi 4 call site sang đồ thị module; xử lý `E_STRICT_STALE` của `proc_demo.rs`; đột biến `ktest_evil.rs` + `src/tests/evil.rs` ⇒ `E_STRICT_TRANSITION` đỏ; selftest giữ 39/39 |
| KT-174 | **v2.1** review đối kháng vòng 3 luật `wake` (cách lách thứ 7) | KT-171 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-157)**: thử đồ thị module hai đường, `#[cfg(feature="ktest")]` trong ảnh sản phẩm, `#[path]` mồ côi, `include!` lồng, macro sinh `wake` bằng `concat_idents!`, trait object; kết luận ĐẠT / ĐẠT CÓ ĐIỀU KIỆN / KHÔNG ĐẠT |
| KT-175 | **K4'** review đối kháng KT-167 (thanh ghi + asm aarch64) | KT-167 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-157)**: đột biến từng trường `TCR_EL1`/slot `MAIR_EL1`; kiểm `SCTLR_EL1` có `UXN`/`PXN` hay không; đối chiếu 6 mã `TLBI` bằng `llvm-objdump`; kiểm stub host panic; **không** sửa mã |
| KT-176 | **v2.1** audit báo cáo đêm + cập nhật `STATE.md` | KT-171 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-157)**: mọi con số trong `BAO-CAO-DEM-2026-11-19-VONG2.md` phải truy được về log; bảng ≥ 20 khẳng định; cập nhật `STATE.md` (HEAD, dấu vân tay, hai ảnh, task) |
| KT-177 | **v2.1** H6/H7 — macro `v2/lib` che `concat!` + macro chứa `include!` (repro dựng lại được) | KT-174 | 2 h | DONE | **Agent 0 (D2-160)**: H6/H7 **chưa** đóng; phải dựng lại **repro** hai ca (mã biên dịch được, `--strict` `errors=0` trên checker cũ) rồi commit repro **trước khi đóng phiếu sáng nay**; vá hoặc ghi nợ có tên |
| KT-178 | **v2.1** review đối kháng vòng 4 (phá H5/H1/H2/H3) | KT-174 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-159)**: `include!` từ hai nơi, raw string `r#"…"#`, `include!` trong tệp test-only, `#[cfg_attr(path)]`, `build.rs`, `paste`; kết luận ĐẠT / ĐẠT CÓ ĐIỀU KIỆN / KHÔNG ĐẠT |
| KT-179 | **v2.1** audit nhật ký mười bốn sub-agent | KT-176 | 1,5 h | DONE | **Agent 1 quyết thay A0 (D2-159)**: mỗi nhật ký — con số có log, khẳng định "đóng" có đột biến đỏ, phán quyết nhất quán với reviewer; bảng ≥ 40 khẳng định |
| KT-180 | **v2.1** phiếu gửi Agent 0 (bản giao đêm vòng hai) | KT-176 | 1 h | DONE | **Agent 1 quyết thay A0 (D2-159)**: `PHIEU-GUI-AGENT0-2026-11-19-VONG2.md` — trạng thái một khối, bảng D2-144..158, **bốn câu hỏi** (vòng vá 5; KT-167 C1–C5; K5' M1 + `[calling.x86_64]`; luật cứng `include!`/`#[path]`), ba việc tiếp, nợ |
| KT-181 | **v2.1** quét thân `macro_rules!` ngoài `v2/lib` và `v2/kernel/*/src` (ví dụ `init/`) | KT-177 | 1,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: quét thân `macro_rules!` ở **cả ba** gốc `v2/lib`+`v2/kernel`+`v2/sim` (token `wake` + `include!`/`#[path]`); 10 ca mới; **kết luận trung thực**: không dựng được đường lọt thật cho `wake` ngoài phạm vi cũ ⇒ đây là **phòng ngừa**, không phải lỗ hổng đã khai thác; selftest 118→**128** |
| KT-182 | **v2.1** đo tác động bản đồ trợ giúp KT-163 nhận hàm không phải `#[test]` | KT-170 | 1,5 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: đo 1019 test / **0** test chỉ dựa hàm sản phẩm cùng tệp / 155 gọi nhưng đều có token riêng ⇒ siết **miễn phí**; review L41 (KT-203) **ĐẠT CÓ ĐIỀU KIỆN** (F1 ca 35/36 không răng, F2 `cfg(test)` trên item bị loại oan); vá KT-206 ⇒ selftest 36→**40/40**; `v2 check` PASS |
| KT-183 | **v2.1** `cd` vào thư mục con **sâu** (cần FS sản phẩm nhiều cấp) | KT-158 | 2 h | DONE | **Agent 1 quyết thay A0 (2026-11-20)**: seed `/docs/sub` + `/docs/sub/deep.txt`; ca `kt183-cd-sau` 8 phép kiểm; đột biến `..`/`pwd` hằng **ĐỎ**; ảnh mới `639ca6d1…` (inputs `43df126a`); reviewer L42 **ĐẠT CÓ ĐIỀU KIỆN** (M2/M4) → vá KT-205; K6 đo lại KT-210 **6/6 ĐẠT** |
| KT-184 | **P0** D2-160 + D2-161: H4 fixpoint + luật cứng `include!`/`#[path]` mọi chế độ | KT-174 | 2 h | REVIEW | **Agent 0 (D2-160/161)**: repro H4 **hai lớp** + **ba lớp** đều ĐỎ; selftest tăng đúng số ca; checker cũ cùng cây `errors=0`; `include!`/`#[path]` kiểm ở **cả ba** chế độ; đột biến không-literal và ra-ngoài-cây ĐỎ ở chế độ **thường**; soạn **nháp** dòng `AGENTS.md` (không tự sửa) |
| KT-185 | **P0** D2-162: C1/C2/C4/C5 cho `vifast-plat-aarch64` | KT-167 | 1,5 h | DONE | **Agent 0 (D2-162)**: C1 14/14 đột biến mask/shift ĐỎ so với ARM ARM; C2 thay tautology `sctlr_builder` bằng hằng viết tay, 3/3 hoán vị ĐỎ; C4 sửa tài liệu, bỏ claim "đủ 39 trường"; C5 log build có `Compiling`; C3 để bước 3 |
| KT-186 | **P1** D2-163: nháp `[calling.x86_64]` + M2 (cargo test, không QEMU) | KT-169 | 3,5 h | DONE | **Agent 0 (D2-163)**: nháp **ngoài `spec/`** ở `v2/agent/reports/`; phạm vi K5' "như K3"; boot tạm QEMU `-kernel`; M2 đo bằng `cargo test`; **không** mở `spec/` |
| KT-194 | **P0** D2-166/D2-167: lexer + token-tree + bộ đọc cấu trúc thay regex | KT-184 | 3 h | REVIEW | **Agent 1 quyết thay A0 (2026-11-19, A0 offline)**: review vòng 3 (KT-200) **ĐẠT CÓ ĐIỀU KIỆN**; F1/F2/F2b/F3/F4/F4b/F4c **đóng**; C1/C2/C3 hợp đồng đã sửa; `--selftest` 118/118; gate PASS trên `c8bef672` |

**APPROVED 2026-11-17 Agent 0** — quyết định cổng K6 `OD-K6-1..8` tại [`reports/QUYET-DINH-A0-K6-2026-11-17.md`](reports/QUYET-DINH-A0-K6-2026-11-17.md) (sha256 `0361fb7e…`), ghi thành **D2-119**. Kèm: (i) `spec/v1-parity.toml` **tạo mới** được phép ở KT-77a (không đổi ABI); (ii) đính chính của lead: số lỗi v1 đã đóng là **2/7** (N4, N6), **1/7 hoãn K5'** (N5), **4/7 mở** (N1/N2/N3/N7) — KT-83 chốt lại trước khi văn bản đóng K6 dùng con số.
| KT-77 | ~~**K6** đưa U-mode + shell vào ảnh sản phẩm~~ → **tách thành KT-77a/77b** (D2-119) | — | — | DONE | superseded |


APPROVED 2026-11-12 Agent 0

> KT-04d DONE: duyệt gói bản 2 sau C1/C2; D2-35..39. **APPROVED mở KT-05.**

## Khối F — chỉ thị hiện hành (2026-11-12 19:45, sau APPROVED KT-04d)

> Nguồn: D2-29..D2-46 + `APPROVED 2026-11-12 Agent 0` ở trên. Khối này thay mọi ghi chú cũ trái với nó.

**Đã xong — không làm lại:**
- `KT-04a` `1766145`; `KT-04b` `5ba7e80`; `KT-04c` `5454834`+`476d405`; `KT-04d` đã duyệt, gói bản 2 `e6edf0f` + C1.
- `KT-05` `845df91`+`c6487a9`; `KT-12` `e750847`..`d3dfe31` (host/format; no_std sang KT-12b);
- `KT-06` `decc619`+`33dcf8d`; `KT-07` `bf61901`; `KT-08` `cf433b7`+`bcc84bb`; `KT-R1` `0ba7968`.
- `KT-T26` **DONE** theo D2-36: đèn host, **T26 OPEN** tới K3.5; không chạy lại 1.000 seed.

**Thứ tự hiện tại — phase K1a' (vòng 2):**
1. **K1b DONE** (KT-18..KT-23; 3/4 + push migration chuyển K1c theo D2-49a).
2. **K3 DONE** (D2-82): KT-44..57 DONE, soak 0/1000, TAP 100 %, gate 574, K3-DOD. **K3b DONE** (D2-86): core chạy trên nhân thật (TAP 43..52), soak 0/1000, gate 598. **K3.5** (D2-87): KT-62/63/64 DONE (gate 642, TAP 60); **KT-64a DONE** (sim đồng bộ, parity 18/18 + 4/4); **KT-65 DONE** (procsrv+shell thật, TAP 24, gate 649); **KT-66 DONE** — **K3.5 DONE** (D2-94). **K6** (D2-95): KT-67..76; **KT-67 DONE** (alloc + bridge, gate 661/87); **KT-67a DONE** (invocation, gate 661/87); **KT-67b DONE** (cap giữa 2 tiến trình thật, gate 661/87); **KT-68 DONE** (capalloc, gate 661/87); **KT-69 DONE** (IDL + ring, gate 675/92); **KT-70 DONE** (pager + page cache, gate 689/95); **KT-71 DONE** (journal + fsck, gate 703/96); **KT-72 DONE** (virtio-blk, gate 718/99); **KT-73 DONE** (console + UART IRQ, gate 726/99); **KT-74 DONE** (SetTls + exec, gate 726/99); **KT-75 DONE** (persist qua reboot thật, gate 729/100); **KT-76 DONE** (cổng K6; gate 729/100; **K6 KHÔNG ĐẠT** vì S1 — D2-118); **Agent 0 chốt cổng K6** (D2-119, `reports/QUYET-DINH-A0-K6-2026-11-17.md`): **OD-K6-1** hướng (a) có giới hạn 2x3h rồi tự rơi về (b); **KT-78/79/80 READY**, **KT-77a READY**, KT-77b/81/82/83 BLOCKED theo thứ tự KT-78 → KT-77a → KT-77b → KT-82.
3. Thứ tự K3: A(44→45→46→{47∥48∥49}→50→51) ∥ B(52→53) ∥ C(54); 55/56; 57 cuối, chạy một mình.
4. Push migration + SMP 1/2/4/8 thuộc K1c (D2-53 Q1/Q2); `nightly --l1` 100k ở KT-29/KT-30.
5. Không đụng `spec/`/`vifast-model`; parity 10k hồi quy; ViScript do tab riêng.
3. Q1–Q8 chốt tại **D2-49**: 1 hart, hoãn push migration K1c; model API; donation tự động; bảng ALLOWED model-only; budget chu kỳ thẳng hàng; không lock-free/loom.
4. Không đụng `spec/`/`vifast-model`/`.scn`; parity 10k chỉ hồi quy.
5. OS hết READY mới lấy ViScript; dòng ViScript do tab riêng quản lý.

**Chống lặp:**
- Đọc bảng trạng thái trước mỗi task; task `DONE` không làm lại.
- Agent 2 **không** ghi QUEUE; `agentctl done` chỉ đề xuất (D2-37). Người điều phối lật sổ.
- Không sửa `spec/` ngoài D2-35 (Agent 0 đã duyệt); không ghi APPROVED.
- Review nói `DRIFT` thì đối chiếu DECISIONS trước; chỉ mở YC mới khi có blocker kỹ thuật mới kèm `file:line` + 2 lựa chọn.

## Ghi chú vận hành

- `vifast-agentctl next` **bỏ qua** task `REVIEW` — không tự làm, không tự duyệt. `next` chỉ trả task `READY` có phụ thuộc `DONE`; agentctl **không** ghi QUEUE (D2-37).
- **Mỗi task ≤ 3 h; task lớn tách** (KT-04 đã tách a/b/c/d; KT-01 tách khỏi soak v1 — v1 **không** soak lại).
- `KT-04d` đã có dòng `APPROVED 2026-11-12 Agent 0`; **APPROVED mở KT-05**. Duyệt không bằng chat.
- **KT-04d đã duyệt** với sản phẩm KT-04a/b/c + C1; D2-35..39 ghi ở `DECISIONS.md`.
- Câu hỏi O1 (**Exit** vào ABI) đã chốt trong QD §1 Q-A: syscall **14 `Exit(code)`**.
- Task READY hiện tại: `KT-05`, `KT-12`; dòng ViScript `ST-02b`, `ST-04`, `ST-10`.
- L1 **đã bật** trong `gate --fast` (KT-08): 1.000 seed; nightly 100.000 seed.
- Mọi số trong bảng này lấy từ QD/brief; số nào chưa có nguồn thì ghi `[TK]`/`[ƯL]`, không tự đặt.
