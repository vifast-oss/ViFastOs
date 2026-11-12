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
| KT-04d | gửi Agent 0 duyệt: checklist 5 mục/method + 21 kịch bản + draft chữ ký + **nội dung D2-26..28** | KT-04a,b,c | 1 h | DONE | **APPROVED 2026-11-12 Agent 0**; D2-35..39; KT-05 mở; `check_scn` 21/21 0 blocked |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md`; sinh từ `abi.toml` | KT-04d | 3 h | DONE | bước 0: draft chữ ký D2-33 vào `spec/[method_signature]` (`stable-draft`) + D2-35; code sinh biên dịch; sửa tay bị bắt; gen-check bật |
| KT-06 | `vifast-agentctl` `--project v2\|viscript`: next/start/done/block/report, bỏ qua REVIEW | KT-02 | 2 h | DONE | dùng được cho chính vòng này; JSON có `project` |
| KT-07 | K0.5: boot riscv64 QEMU qua OpenSBI nạp trực tiếp; DebugPutStr/DebugHalt; TAP 1 test; 50 boot 0 lỗi | KT-02, KT-03 | 3 h | DONE | 50 boot 0 lỗi, có log |
| KT-08 | sim harness + store giả + scheduler tối thiểu 1 hart; kịch bản rỗng tất định theo seed (chưa cần ABI) | KT-02 | 3 h | DONE | harness + store + scheduler chạy; cùng seed → cùng trace |
| KT-09 | Untyped trên sim (watermark, Retype, Revoke qua CNode) + end-to-end Retype→Configure→Call/Reply | KT-08, KT-05, KT-12b | 3 h | READY | invariant tổng byte; preempt; kịch bản end-to-end chạy; fixture âm kiểm **thông điệp**; định nghĩa ngữ nghĩa thời điểm `expect_states` (C5) |
| KT-10 | CNode/MDB test-first, mô hình tối giản **trong test** | KT-09 | 3 h | BLOCKED | property test cây; Revoke |
| KT-11 | `vifast-model` Untyped/CNode; chạy các ca Untyped/CNode của KT-04c | KT-10 | 2 h | BLOCKED | khớp sim 10.000 seed; chạy các ca Untyped/CNode của KT-04c |
| KT-12 | `vifast-traceformat` + ring (trước khi sim dump) | KT-02 | 2 h | DONE | giải mã dump; D2-38/D2-40: text = scenario log, KT-12 sở hữu bản ghi 32 B **cùng định dạng sim/QEMU** + golden nhị phân |
| KT-12b | `vifast-traceformat` **no_std** + `FixedRing<const N>` (kernel); `alloc` feature cho `decode_all`; `Ring`/`Vec` chỉ sau `std`; cross-check byte với golden D2-40 | KT-12 | 2 h | DONE | `#![no_std]` biên dịch; `FixedRing` không cấp phát; test host vẫn xanh |
| KT-T26 | host test `vifast-fs` + block giả + cache; MAX_IO=256; đọc RAW kiểm read-your-own-write; theo D-03 "ai chen giữa" | — | 3 h | DONE | đèn host DONE `02e0a96` theo D2-36; T26 **OPEN** tới K3.5 (VFS/read_dir + smp); không chạy lại 1.000 seed |
| KT-R1 | báo cáo `R1-v2` phủ task đã mở + `blocked[]` | các task READY | 1 h | DONE | MD + JSON theo Khối E, có `project` |

APPROVED 2026-11-12 Agent 0

> KT-04d DONE: duyệt gói bản 2 sau C1/C2; D2-35..39. **APPROVED mở KT-05.**

## Khối F — chỉ thị hiện hành (2026-11-12 19:45, sau APPROVED KT-04d)

> Nguồn: D2-29..D2-42 + `APPROVED 2026-11-12 Agent 0` ở trên. Khối này thay mọi ghi chú cũ trái với nó.

**Đã xong — không làm lại:**
- `KT-04a` `1766145`; `KT-04b` `5ba7e80`; `KT-04c` `5454834`+`476d405`; `KT-04d` đã duyệt, gói bản 2 `e6edf0f` + C1.
- `KT-05` `845df91`+`c6487a9`; `KT-12` `e750847`..`d3dfe31` (host/format; no_std sang KT-12b);
- `KT-06` `decc619`+`33dcf8d`; `KT-07` `bf61901`; `KT-08` `cf433b7`+`bcc84bb`; `KT-R1` `0ba7968`.
- `KT-T26` **DONE** theo D2-36: đèn host, **T26 OPEN** tới K3.5; không chạy lại 1.000 seed.

**Thứ tự hiện tại:**
1. **KT-09** (READY): Untyped/Retype/Revoke + end-to-end; KT-12b DONE (D2-42).
2. `KT-10`/`KT-11` theo chuỗi sau KT-09.
3. **KT-05/KT-12 DONE** — không làm lại.
4. OS hết READY thì ViScript: **ST-04 lexer trước ST-03**; `ST-10` DONE.
5. Không mở YC mới cho YC-06/11/12/13 — đã chốt D2-36..39.

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
