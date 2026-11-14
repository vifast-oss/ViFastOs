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
| KT-40 | TLB model + `TlbFlush` + ack thế hệ + chống deadlock | KT-38, KT-39 | 3 h | READY | 16 entry/hart; ack đủ mới Ok; rút hộp thư; 4 mutation; 100k×4 |
| KT-41 | VM fault + `FaultKind::Vm` model-only | KT-38 | 2 h | BLOCKED | NotMapped/Permission/Wx; fault_ep; spec không đổi; gate/Miri |
| KT-42 | 4 `.scn` + `check_vm` + shared read-only frame | KT-39, KT-40, KT-41 | 2,5 h | BLOCKED | 4 scn đỏ trước; self-test check_vm; shared RO; gate |
| KT-43 | Cổng cuối K2: 100k×4 + 16 mutation + Miri + DoD | KT-42 | 3 h | BLOCKED | 0 vi phạm TLB-safety; AP→BSP bắt; Miri; DoD; log |


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
2. **K2**: KT-35..39 DONE; **KT-40 READY** (TLB/shootdown); K1c DONE.
3. Thứ tự K2: 40 ∥ 41 → 42 → 43; mỗi task ≤3 h.
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
