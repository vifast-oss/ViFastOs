# PATCH-BRIEF — vòng vá sau review (Agent 1 → 4 subagent)

Nguồn sự thật: `docs/QUYET-DINH-sau-review.md` (gọi tắt QD). Nếu QD khác tài liệu thiết kế thì **QD thắng**.
Tài liệu nguồn đã copy sang đích; sửa trực tiếp trên đích.

## 0. Không gian tên (bắt buộc, dùng đúng)

| Loại | Cũ | Mới |
|---|---|---|
| Quyết định v1 | D-01..D-08, D11, D18 | giữ nguyên, chỉ áp cho v1 |
| Quyết định v2 | D-09..D-27 | **D2-01..D2-24** (đúng 24 mục, xem QD §8) |
| Quyết định ViScript | VD-01..VD-10 | giữ, thêm VD-11.. nếu cần cho QD §7 |
| Lỗi gốc v1 | G1..G6 | **R1..R6** (G chỉ dùng cho mốc desktop G1..G5) |
| Task OS | T01..T16 | **KT-01..KT-12, KT-T26, KT-R1** |
| Task ViScript | VT01..VT10 | **ST-01..ST-10** |
| Commit tag | `[T##]` / `[VT##]` | `[KT-##]` / `[ST-##]` |
| Báo cáo | R1 | `R1-v2`, `R1-viscript`; JSON có `project` |

Nhãn trong tài liệu: `[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` (dossier, giữ độ tin cậy gốc), `[TK]` (thiết kế mới — **mặc định** cho dòng không nhãn), `[GT]`, `[ƯL]`. Hằng số thiết kế là `[TK]`, không phải `[ƯL]`.

## 1. Task OS vòng 1 (chép vào v2/agent/QUEUE.md và §13 tài liệu)

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| KT-01 | tag `v1-archive` trên `ca2eaf5`; `docs/v1/KNOWN-ISSUES.md`; chuyển tài liệu v1 vào `docs/v1/` (Q10) | — | 1 h | READY | tag có message "T33a mở"; KNOWN-ISSUES đủ 7 mục QD §6, mỗi mục có ca kiểm v2 |
| KT-02 | khung `v2/` + `v2/agent/` (không ghi đè tệp đã có), LICENSE tham chiếu root, thêm `vifast-fsutil` | — | 3 h | READY | `cargo build` workspace rỗng xanh; `ls v2/agent` đủ tệp |
| KT-03 | `v2 check` + `v2 gate --fast` khung: fmt, clippy, luật cấu trúc, L0, SPDX theo loại tệp; **chưa** gen-check/L1 | KT-02 | 3 h | BLOCKED | cổng chạy; 5 luật có 5 case âm |
| KT-04a | bảng ánh xạ v1→v2 (25 crate, 58 syscall→15+method, 22 KError→19) + `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B) | KT-02 | 3 h | READY | abi.toml parse; bảng ánh xạ đủ; `abi_version`/`min_compat` |
| KT-04b | object reserved: chỉ số + `NotSupported` (VCpu.Configure, IOSpace.Attach, VSpace khách, PowerControl…) | KT-04a | 2 h | BLOCKED | method reserved trả NotSupported; số không trùng |
| KT-04c | 20 kịch bản `.scn` viết **chỉ bằng ABI** + bảng kỳ vọng (cap copy/revoke, call/reply, reply bị xoá, suspend khi chờ, kill khi chờ, exit, 4 cap, no_reply, donation) | KT-04a | 3 h | BLOCKED | 20 tệp + 5 ca chạy trên mock CNode/Untyped tối giản trong test |
| KT-04d | gửi Agent 0 duyệt: checklist 5 mục/method + 20 kịch bản | KT-04a,b,c | 1 h | REVIEW | dòng `APPROVED <ngày> Agent 0` trong QUEUE |
| KT-05 | `vifast-gen` + gen-check + `docs/abi.md`; sinh từ `abi.toml` | KT-04a | 3 h | BLOCKED | code sinh biên dịch; sửa tay bị bắt; gen-check bật sau APPROVED |
| KT-06 | `vifast-agentctl` `--project v2\|viscript`: next/start/done/block/report, bỏ qua REVIEW | KT-02 | 2 h | BLOCKED | dùng được cho chính vòng này; JSON có `project` |
| KT-07 | K0.5: boot riscv64 QEMU qua OpenSBI nạp trực tiếp; DebugPutStr/DebugHalt; TAP 1 test; 50 boot 0 lỗi | KT-02, KT-03 | 3 h | BLOCKED | 50 boot 0 lỗi, có log |
| KT-08 | sim harness + store giả + scheduler tối thiểu 1 hart | KT-07 | 3 h | BLOCKED | 1 kịch bản end-to-end (Retype→Configure→Call/Reply) chạy |
| KT-09 | Untyped trên sim (watermark, Retype, Revoke qua CNode) | KT-08 | 3 h | BLOCKED | invariant tổng byte; preempt |
| KT-10 | CNode/MDB test-first, mô hình tối giản **trong test** | KT-09 | 3 h | BLOCKED | property test cây; Revoke |
| KT-11 | `vifast-model` Untyped/CNode | KT-10 | 2 h | BLOCKED | khớp sim 10.000 seed; chạy lại 20 ca KT-04c |
| KT-12 | `vifast-traceformat` + ring (trước khi sim dump) | KT-08 | 2 h | BLOCKED | giải mã dump từ sim; cùng định dạng QEMU |
| KT-T26 | host test `vifast-fs` + block giả + cache; MAX_IO=256; đọc RAW kiểm read-your-own-write; theo D-03 "ai chen giữa" | — | 3 h | READY | test đỏ trước; chạy seed |
| KT-R1 | báo cáo `R1-v2` phủ task đã mở + `blocked[]` | các task READY | 1 h | READY | MD + JSON theo §11.7, có `project` |

Ghi chú: trạng thái `READY|BLOCKED|REVIEW|DONE`; `agentctl next` bỏ qua REVIEW.

## 2. Task ViScript vòng 1

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| ST-01 | workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX theo loại tệp + cổng 1.500 dòng/unsafe có case âm + `vis-host-vifast` **không** nằm trong `viscript/` | — | 3 h | READY | cổng chạy; 3 case âm |
| ST-02a | `docs/viscript/SPEC-lex.md` + **EBNF tự chứa** + bảng ưu tiên + 60 golden token + 30 golden biểu thức | ST-01 | 3 h (2 phiên) | REVIEW | mọi khối mã trong tài liệu parse được (ngoại lệ khai báo); Agent 0 duyệt |
| ST-02b | sổ mã lỗi VS + 4 chữ số + quy tắc cấp | ST-01 | 2 h | READY | bảng mã; `vis explain` được |
| ST-03 | `vis-diag`: cột theo Unicode scalar, render, bảng vi/en/ja | ST-01 | 3 h | BLOCKED | snapshot 20 ca, có tiếng Việt/emoji |
| ST-04 | lexer theo SPEC-lex (sau duyệt) | ST-02a | 3 h | BLOCKED | bảng token vàng + fuzz 10 phút + NFC/NFD |
| ST-05 | parser + CST lossless | ST-04 | 3 h | BLOCKED | 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
| ST-06 | CmdCST + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | BLOCKED | golden AST/CST |
| ST-07 | `vis-fmt` lossless, idempotent, sửa tab | ST-05 | 3 h | BLOCKED | vòng tròn parse→fmt→parse |
| ST-08 | runner `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | BLOCKED | TAP; chạy fixture |
| ST-09 | fixture theo QD §7.4 (1/khối mã + 1/quyết định; cấm `...`) | ST-06,07,08 | 2 h | BLOCKED | xanh |
| ST-10 | báo cáo `R1-viscript` | các task | 1 h | READY | MD + JSON có `project` |

## 3. Sản phẩm và người phụ trách

| Subagent | Sản phẩm | Đường dẫn |
|---|---|---|
| SA-A | ViFastOs v2 đã vá | `docs/v2/ViFastOs-v2-thiet-ke.md` |
| SA-B | ViScript đã vá (kèm Phụ lục A mới) | `docs/viscript/ViScript-thiet-ke.md` |
| SA-C | SPEC-lex | `docs/viscript/SPEC-lex.md` |
| SA-D | Điều khiển + KNOWN-ISSUES + archive v1 | `AGENTS.md`, `v2/AGENTS.md`, `viscript/AGENTS.md`, `v2/agent/*`, `viscript/agent/*`, `docs/v1/*` |

## 4. Luật chung

- Giữ nguyên cấu trúc/mục không bị QD đụng; không viết lại văn phong; không xoá mục.
- Mỗi tài liệu thêm mục đầu: `## Thay đổi sau review (2026-11-12)` liệt kê ngắn các nhóm đã vá + trỏ `docs/QUYET-DINH-sau-review.md`.
- Không tự chọn khi QD mâu thuẫn code/dossier: ghi `review/work/PATCH-NOTES-<area>.md` kèm dẫn chứng, **không tự quyết**.
- Mọi con số mới phải có nhãn (`[TK]`/`[GT]`/`[ƯL]`/`[V1:…]`).
- Không sửa `docs/QUYET-DINH-sau-review.md`.
- Kết thúc: trả về tóm tắt ≤25 dòng: tệp đã ghi, mục đã sửa, chỗ chưa làm được, câu hỏi cho Agent 0.
