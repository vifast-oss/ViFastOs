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
| KT-81 | **K6** thiết bị giả nghiêm ngặt + bảng tài nguyên | KT-80 | 3 h | BLOCKED | bắt lại 2 lỗi driver cũ ở mô hình |
| KT-82 | **K6** văn bản đóng K6 (a hoặc b) + caveat P20 | KT-89 | 3 h | BLOCKED | ba trạng thái đóng/hoãn/mở; số từ bảng KT-83; **điều kiện tiên quyết (A1-179)**: 4 tài liệu `pre_existing` (`K6-DOD`, `k6-exec`, `k6-persist`, `k6-parity`) phải chuyển số vào khối `scope=` trước |
| KT-83 | **K6** mở K7 "Đóng lỗi v1" + chốt bảng 7 lỗi | KT-82 | 3 h | BLOCKED | 2 đóng / 1 hoãn K5' / 4 mở; `file:dòng` từng lỗi; K7 **không** nhận N5 |

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
