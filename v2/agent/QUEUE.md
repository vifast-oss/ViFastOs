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

1. Sửa `spec/` — trừ `KT-04a` (được ghi `spec/abi.toml`, `spec/kernel.toml`).
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
| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` (Q10) | — | 1 h | READY | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục QD §6, mỗi mục có ca kiểm v2 |
| KT-02 | khung `v2/` + `v2/agent/` + `v2/bin/` (không ghi đè tệp đã có), LICENSE tham chiếu root, thêm `vifast-fsutil` (**không** port `vifast-fs` — để KT-T26/K2) | — | 3 h | READY | `cargo build` workspace rỗng xanh; `ls v2/agent` đủ tệp; có `v2/bin/` |
| KT-03 | wrapper `v2/bin/v2` + khung `check`/`gate --fast`: fmt, clippy, luật cấu trúc, L0, SPDX theo loại tệp; **chưa** gen-check/L1 | KT-02 | 3 h | BLOCKED | `./v2/bin/v2 check` chạy từ repo root; 5 luật có 5 case âm |
| KT-04a | bảng ánh xạ v1→v2 (25 crate, 58 syscall→15+method, 22 KError→19) + `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B) | KT-02 | 3 h | READY | abi.toml parse; bảng ánh xạ đủ; `abi_version`/`min_compat` |
| KT-04b | object reserved: chỉ số + `NotSupported` (VCpu.Configure, IOSpace.Attach, VSpace khách, PowerControl…) | KT-04a | 2 h | BLOCKED | method reserved trả NotSupported; số không trùng |
| KT-04c | **21 kịch bản** `.scn` viết chỉ bằng ABI + **kiểm tĩnh**: mọi bước tham chiếu syscall/method/mã lỗi có thật trong `abi.toml`, đủ tham số (gồm ca **"Exit khi procsrv chưa Recv"**) | KT-04a | 3 h | BLOCKED | 21 tệp; kiểm tĩnh pass; ca không viết được → BLOCKED "thiếu ABI" |
| KT-04d | gửi Agent 0 duyệt: checklist 5 mục/method + 21 kịch bản | KT-04a,b,c | 1 h | REVIEW | dòng `APPROVED <ngày> Agent 0` trong QUEUE |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md`; sinh từ `abi.toml` | KT-04d | 3 h | BLOCKED | code sinh biên dịch; sửa tay bị bắt; gen-check bật sau APPROVED |
| KT-06 | `vifast-agentctl` `--project v2\|viscript`: next/start/done/block/report, bỏ qua REVIEW | KT-02 | 2 h | BLOCKED | dùng được cho chính vòng này; JSON có `project` |
| KT-07 | K0.5: boot riscv64 QEMU qua OpenSBI nạp trực tiếp; DebugPutStr/DebugHalt; TAP 1 test; 50 boot 0 lỗi | KT-02, KT-03 | 3 h | BLOCKED | 50 boot 0 lỗi, có log |
| KT-08 | sim harness + store giả + scheduler tối thiểu 1 hart; kịch bản rỗng tất định theo seed (chưa cần ABI) | KT-02 | 3 h | BLOCKED | harness + store + scheduler chạy; cùng seed → cùng trace |
| KT-09 | Untyped trên sim (watermark, Retype, Revoke qua CNode) + end-to-end Retype→Configure→Call/Reply | KT-08, KT-05 | 3 h | BLOCKED | invariant tổng byte; preempt; kịch bản end-to-end chạy |
| KT-10 | CNode/MDB test-first, mô hình tối giản **trong test** | KT-09 | 3 h | BLOCKED | property test cây; Revoke |
| KT-11 | `vifast-model` Untyped/CNode; chạy các ca Untyped/CNode của KT-04c | KT-10 | 2 h | BLOCKED | khớp sim 10.000 seed; chạy các ca Untyped/CNode của KT-04c |
| KT-12 | `vifast-traceformat` + ring (trước khi sim dump) | KT-02 | 2 h | BLOCKED | giải mã dump từ sim; cùng định dạng QEMU |
| KT-T26 | host test `vifast-fs` + block giả + cache; MAX_IO=256; đọc RAW kiểm read-your-own-write; theo D-03 "ai chen giữa" | — | 3 h | READY | test đỏ trước; chạy seed |
| KT-R1 | báo cáo `R1-v2` phủ task đã mở + `blocked[]` | các task READY | 1 h | READY | MD + JSON theo Khối E, có `project` |

## Ghi chú vận hành

- `vifast-agentctl next` **bỏ qua** task `REVIEW` (KT-04d) — không tự làm, không tự duyệt. `next` chỉ trả task `READY` có phụ thuộc `DONE`.
- **Mỗi task ≤ 3 h; task lớn tách** (KT-04 đã tách a/b/c/d; KT-01 tách khỏi soak v1 — v1 **không** soak lại).
- `KT-04d` REVIEW chờ đúng dòng `APPROVED <YYYY-MM-DD> Agent 0`; **APPROVED mở KT-05**. Duyệt không bằng chat.
- **KT-04d chỉ duyệt khi có sản phẩm KT-04a/b/c**; duyệt tài liệu thiết kế ghi bằng **D2-25** ở `DECISIONS.md`, không ghi ở đây.
- Câu hỏi O1 (**Exit** vào ABI) đã chốt trong QD §1 Q-A: syscall **14 `Exit(code)`**.
- Nhánh chạy song song được ngay: (KT-01, KT-02, KT-04a, KT-T26) và dòng ViScript `viscript/agent/QUEUE.md` (`ST-01`, `ST-02b`, `ST-10`).
- L1 **chưa bật** tới KT-08; ghi rõ trong `STATE.md`.
- Mọi số trong bảng này lấy từ QD/brief; số nào chưa có nguồn thì ghi `[TK]`/`[ƯL]`, không tự đặt.
