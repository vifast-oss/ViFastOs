# Báo cáo vòng vá (Agent 1) — theo `docs/QUYET-DINH-sau-review.md`

Ngày: 2026-11-12. Người gửi duyệt: Agent 0. Đầu ra: 2 tài liệu đã vá + 2 QUEUE + bộ điều khiển + SPEC-lex.
Chưa commit. Diff thô: `review/work/ViFastOs-v2.diff` (+321 dòng), `review/work/ViScript.diff` (+684 dòng).

## 1. Tệp đã giao

| Tệp | Trạng thái | Số dòng | Ghi chú |
|---|---|---:|---|
| `docs/v2/ViFastOs-v2-thiet-ke.md` | vá | 1.131 | D2-01..24, KT-01..R1, Exit 14, ABI 5-thứ, slot 48 B, roadmap K0.5/K3.5, Phụ lục A/B |
| `docs/viscript/ViScript-thiet-ke.md` | vá | 1.457 | NFC, 50pct, bảng ưu tiên 12 mức, hai pha parse, **Phụ lục A EBNF tự chứa (~122 luật)**, ST-01..10 |
| `docs/viscript/SPEC-lex.md` | mới | 180 | bảng token, escape, đơn vị, maximal munch, chế độ lệnh, 10 mã VS01xx, golden test |
| `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md` | mới | — | QUEUE 17 dòng KT + 5 khối tự chứa; DECISIONS 8 D-0x + 24 D2 |
| `viscript/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md` | mới | — | QUEUE 11 dòng ST; DECISIONS VD-01..22 |
| `AGENTS.md`, `v2/AGENTS.md`, `viscript/AGENTS.md` | mới | — | nguồn sự thật, cấm, lệnh cổng, commit tag, duyệt APPROVED |
| `docs/v1/KNOWN-ISSUES.md` | mới | — | 7 lỗi QD §6 + ghi chú P1-6/P1-7/T33b |
| `docs/v1/` | lưu trữ | 105 mục | 101 `agent2-report-*`, `overnight-log.md`, `decisions/`, README |
| `docs/README.md` | sửa | — | mục 2a trỏ v2/viscript; mục 3 trỏ `docs/v1/` |

Bản gốc trước vá: `review/src/`; snapshot sau vá: `review/work/snapshot-1543/`.

## 2. Đối chiếu QD → nơi áp dụng

| QD | Áp dụng |
|---|---|
| §1 Q-A Exit | `V:4.21` syscall 14; `V:4.4` `FaultKind::Exit` + ReadState; `V:6.4` procsrv wait/kill; `V:4.20` 19 lỗi (bỏ Restart) |
| §1 Q-B freeze 5 thứ | `V:4.21` khối "Đóng băng ABI (D2-15)" + `abi_version`/`min_compat`/golden; `V:13` KT-04a..d |
| §1 Q-C K0.5 | `V:10.1` K0.5 + KT-07/08/09/10; đảo sim trước objmem |
| §1 Q-D hai pha parse | `S:9.3`, `S:19` VS1 (`cmd_lower` vòng 2), `S:20.2` ST-06, `S:22` VD-11 |
| §1 Q-E OS trước | `V:12` D2-20; `v2/AGENTS.md` §3; `viscript/agent/QUEUE.md` ghi chú |
| §1 Q-F cắt giảm | `V:10.1` chỉ riscv64 tới K6; `V:4.5` MCS giai đoạn; bỏ 90 ngày (`V:10.6`) |
| §2.1 không gian tên | D2/KT/ST/R<n>-v2; `V:12`, `S:20.2`, AGENTS commit tag |
| §2.2 nhãn | đầu 2 tài liệu; `[TK]` cho hằng số |
| §2.3 ràng buộc v1 | `V:2.3` bảng 11 dòng; `v2/agent/DECISIONS.md` Phần I |
| §3.1 job control | `V:4.4` suspended/Suspend(Abort/Defer)/Resume; `V:6.4` RPC procsrv; `S:12` Host; `S:15` tty |
| §3.2 Reply/no_reply | `V:4.6` ca biên + `no_reply` trong MessageInfo |
| §3.4 MCS | `V:4.5` bảng "Có ở K1b / Hoãn sau K6" |
| §3.5 objmem | `V:4.2` API closure, bỏ refs/get2_mut; `[GT]` + `.data+.bss ≤ 256 KiB`; `V:10.1` K1a' |
| §3.6 id | `V:4.2` `id=(cluster<<56)|counter` |
| §3.7 Restart | `V:4.13`, `V:4.20` |
| §3.8 WaitAddr | `V:4.7` bảng băm arena |
| §3.9 slot | `V:4.3` 48 B + test size_of |
| §3.10 push migration | `V:4.16` trường công bố; luật 3 sửa |
| §3.11 Notification | `V:4.7` ghi rõ thay đổi có chủ ý |
| §3.12 EXEC | `V:4.3` EXEC là thuộc tính map |
| §3.13 64 MR/4 cap | `V:4.6` lý do |
| §3.14 IOMMU | `V:4.12` threat model |
| §3.15 linh tinh | `V:4.22` (MAX_HARTS 8/64, trace luôn bật), `V:8` (fsutil, 25k, .data), `V:4.9` |
| §4 ABI/T04 | `V:4.20/4.21`, `V:13` KT-04a..d + APPROVED |
| §5.1 lộ trình | `V:10` toàn bộ, K3.5, K4'/K5', U1/N1/VSH2/VS3/TUI |
| §5.2 cổng | `V:9.1`, `V:9.13`, `v2/agent/QUEUE.md` khối D |
| §5.3 đo lường | `V:9.12` N=1.000, ps, BUILD_ID, harness-v2 |
| §5.4 v1 | `V:13` KT-01; `docs/v1/KNOWN-ISSUES.md`; cấm pkill toàn cục (`AGENTS.md`) |
| §6 sổ lỗi | `V:Phụ lục A`; `docs/v1/KNOWN-ISSUES.md` |
| §7.1 từ vựng | `S:2.1–2.4`, `docs/viscript/SPEC-lex.md` |
| §7.2 chế độ lệnh | `S:9.3–9.5`, `S:12` |
| §7.3 ngữ nghĩa | `S:3.6`, `S:4.1–4.5`, `S:6`, `S:22` VD-11..22 |
| §7.4 Phụ lục A/B, ST-09 | `S:Phụ lục A` (EBNF), `S:Phụ lục B` (sửa ví dụ), `S:20.2` |
| §7.5 queue ViScript | `S:19`, `S:20.2`, `viscript/agent/QUEUE.md` |
| §8 D2 | `V:12` bảng 24 dòng; `v2/agent/DECISIONS.md` |
| §9 Q1–Q12, giấy phép | `V:Phụ lục B`; `S:14.2` + `AGENTS.md` §1 (allowlist, cargo-deny, THIRD-PARTY) |

## 3. Kiểm tra sau vá

Xem `review/work/CHECK.md`. Tóm tắt: 24/24 D2, 17/17 KT, 15 syscall + Exit, 19 lỗi, slot 48 B, K0.5/K3.5, 0 task T cũ, phụ lục Q1–Q12 + sổ lỗi; ViScript 11 ST, 22 VD, 8 Q, 122 luật EBNF, 50pct, CmdCST, SPEC-lex 180 dòng/10 mã; điều khiển 17/17 + 11/11, DECISIONS 24 D2 + 8 D-0x, KNOWN-ISSUES 7/7, 105 tệp lưu trữ, 3 AGENTS.

## 4. Điểm cần Agent 0 biết (không tự chọn)

1. **SPEC-lex + Phụ lục A do Agent 1 soạn**, vì 2 subagent phụ trách hết output (SA-C/SA-B). Vì vậy `ST-02a` đã đổi nghĩa thành **duyệt + hoàn thiện + 60 golden token/30 golden biểu thức**, không còn "viết từ đầu"; đã sửa đồng bộ trong `S:19`, `S:20.2`, `viscript/agent/QUEUE.md`.
2. **KT-04c** cần mô hình (KT-11) nhưng mô hình ở sau; đã chốt trong QUEUE: 20 kịch bản + 5 ca trên **mock CNode/Untyped tối giản trong test**, KT-11 chạy lại đủ 20 ca. Nếu Agent 0 muốn đảo KT-11 trước KT-04c thì báo.
3. **`Tcb.ReadState` có `exit_code?`**: hiểu là kernel lưu mã thoát khi `Exit` để procsrv đọc; nếu Agent 0 muốn procsrv tự lưu thì bỏ field này khỏi ABI (không đổi gì khác).
4. **Exit fault dùng Send (không Reply)** — đã ghi rõ khác Fault thường; cần xác nhận vì `fault_ep` thường dùng Call.
5. **`cargo-deny` allowlist** đã ghi ở `S:14.2` + `AGENTS.md`; hiện **chưa cài** công cụ — thuộc KT-03/ST-01.
6. **Chưa commit**: các thay đổi đang staged/untracked; KT-01 mới tạo tag `v1-archive`.
7. **`docs/README.md`** đã thêm mục 2a; nếu Agent 0 muốn đặt tài liệu ở `v2/docs/` thay vì `docs/v2/` thì phải sửa lại đường dẫn trong AGENTS + 2 tài liệu.

## 5. Đề nghị duyệt

- Duyệt nội dung 2 tài liệu + SPEC-lex/EBNF (đọc diff hoặc bản đầy đủ).
- Nếu đồng ý, ghi `APPROVED <ngày> Agent 0` vào `v2/agent/QUEUE.md` (dưới KT-04d) và `viscript/agent/QUEUE.md` (dưới ST-02a).
- Sau APPROVED mới mở KT-05 và ST-04..ST-09; trước đó chỉ các task READY (KT-01, KT-02, KT-04a, KT-T26, ST-01, ST-02b, ST-10).
