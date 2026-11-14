# AGENTS.md — luật làm việc trong repo ViFastOs

Cập nhật: 2026-11-12 (vòng vá sau review). Nguồn sự thật của vòng vá:
[`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md) (gọi tắt **QD**).
Tệp này áp cho **mọi** agent làm việc trong repo. Hai dòng sản phẩm có bản rút gọn:
[`v2/AGENTS.md`](v2/AGENTS.md) và [`viscript/AGENTS.md`](viscript/AGENTS.md).

## 1. Thứ tự nguồn sự thật

Khi hai nguồn nói khác nhau, **nguồn đứng trước thắng**:

1. **Code + harness** — thứ chạy được và đo được: `crates/`, `v2/`, `viscript/`, sim, tệp `.scn`, TAP, log cổng.
2. **`spec/`** — `spec/abi.toml`, `spec/kernel.toml`, `docs/viscript/SPEC-lex.md`: một nguồn sự thật duy nhất cho ABI và ngữ nghĩa ngôn ngữ.
3. **`v2/agent/DECISIONS.md`** (D-01..D-08, D2-01..D2-69) và **`viscript/agent/DECISIONS.md`** (VD-01..VD-88).
4. **Tài liệu thiết kế** — [`docs/v2/ViFastOs-v2-thiet-ke.md`](docs/v2/ViFastOs-v2-thiet-ke.md), [`docs/viscript/ViScript-thiet-ke.md`](docs/viscript/ViScript-thiet-ke.md).

QD mạnh hơn hai tài liệu thiết kế. Không có nguồn nào cho phép **nới ngưỡng để qua cổng**.

Nhãn số liệu (bắt buộc, QD §2.2): `[V1:CODE]` `[V1:ĐO]` `[V1:DOC]` (từ dossier, giữ độ tin cậy gốc), `[TK]` thiết kế mới (**mặc định** cho dòng không nhãn), `[GT]` giả thuyết, `[ƯL]` số ước lượng cần đo. Hằng số thiết kế là `[TK]`, **không** phải `[ƯL]`. Số đo chỉ do `./v2/bin/v2 nightly` / `vis-dev nightly` sinh; tài liệu không chứa số đo viết tay (QD §9 Q12).

## 2. Bản đồ tài liệu

| Nhóm | Đường dẫn | Trạng thái |
|---|---|---|
| Quyết định vòng vá | `docs/QUYET-DINH-sau-review.md` | nguồn sự thật, **cấm sửa** |
| Thiết kế v2 | `docs/v2/ViFastOs-v2-thiet-ke.md` | cấm sửa (chỉ Agent 0 / Agent 1 theo QD) |
| Thiết kế ViScript | `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md` | cấm sửa (chỉ Agent 0 / Agent 1 theo QD) |
| Điều khiển OS | `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/` | xem bảng chủ sở hữu §3 |
| Điều khiển ViScript | `viscript/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/` | xem bảng chủ sở hữu §3 |
| Lưu trữ v1 | `docs/v1/` (toàn bộ báo cáo `agent2-report-*`, `overnight-log.md`, `decisions/`, `KNOWN-ISSUES.md`) | **đọc-only** |

## 3. Ai ghi tệp nào

| Tệp | Người ghi | Ghi chú |
|---|---|---|
| `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `viscript/agent/QUEUE.md`, `viscript/agent/DECISIONS.md` | **Agent 0** (qua người điều phối) | Agent 2 **không** sửa; cần đổi thì BLOCKED + đề xuất |
| `v2/agent/STATE.md`, `v2/agent/BLOCKED.md`, `v2/agent/JOURNAL/`, `v2/agent/reports/` | **Agent 2** dòng OS | STATE cập nhật **sau mỗi bước** |
| `viscript/agent/STATE.md`, `viscript/agent/BLOCKED.md`, `viscript/agent/JOURNAL/`, `viscript/agent/reports/` | **Agent 2** dòng ViScript | như trên |
| `spec/` | chỉ task được ghi rõ (KT-04a, KT-04b một lần theo D2-32, **KT-05** một lần theo D2-33/D2-35, ST-02a, ST-02b chỉ bảng mã lỗi) | xem §4 |
| `v2/AGENTS.md`, `viscript/AGENTS.md`, `AGENTS.md` | người điều phối / Agent 0 | Agent 2 không sửa |

Hai QUEUE **độc lập**: một task OS bị BLOCKED **không** chặn dòng ViScript và ngược lại (trừ cặp phụ thuộc ghi trong chính QUEUE).

## 4. Cấm

Cấm tuyệt đối, không có ngoại lệ "tạm thời":

1. **Sửa `spec/`** trừ task được ghi rõ trong QUEUE (`KT-04a`: `spec/abi.toml`, `spec/kernel.toml`; `KT-04b`: một lần theo D2-32, chỉ chép draft reserved đã sửa + D2-29; `KT-05`: một lần, chỉ chép `[method_signature]` + `[debug_info].return_channel` theo D2-33/D2-35; `ST-02a`: `docs/viscript/SPEC-lex.md`; `ST-02b`: chỉ bảng mã lỗi trong SPEC/phụ lục, theo VD-27). Mọi thay đổi ABI/ngữ nghĩa khác phải qua Agent 0 (D2-04, D2-22).
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. Thêm `#[ignore]`, `#[should_panic]` để né, tắt test, hoặc bỏ case khỏi danh sách chạy.
4. **Nới ngưỡng** (thời gian, seed, số ca đỏ, `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`).
5. **Retry** để qua cổng (D2-07). Flake ghi `T-INFRA` kèm log; chạy lại **tối đa một lần** chỉ để phân loại, không để lấy xanh.
6. Sửa `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/viscript/ViScript-thiet-ke.md`, `docs/viscript/SPEC-lex.md`.
7. Sửa bất kỳ tệp nào trong `docs/v1/` (kho lưu trữ v1, tag `v1-archive`).
8. Thêm `unsafe` ngoài danh sách crate đóng (D2-17); thêm `cfg(target_arch)` vào kernel/runtime.
9. Thêm phụ thuộc crate ngoài allowlist giấy phép (D2-21, QD §9.3).
10. Đưa số đo viết tay vào tài liệu.

## 5. v1 đã đóng băng

- Tag **`v1-archive`** trên commit `ca2eaf5`. **Không** có tag `v1-final`.
- `docs/v1/` là **đọc-only**: toàn bộ báo cáo `agent2-report-*` của v1, `overnight-log.md`, `decisions/`, `KNOWN-ISSUES.md`, `README.md`.
- **Không soak lại v1**, không phát triển v1, không mở tính năng trên v1 (D-02 hết hiệu lực từ khi lưu trữ, QD §2.3).
- Sổ lỗi v1 còn mở đã mang sang v2 dưới dạng ca kiểm: [`docs/v1/KNOWN-ISSUES.md`](docs/v1/KNOWN-ISSUES.md).

## 6. Lệnh

```bash
./v2/bin/v2 check                  # nhanh: fmt + clippy + luật cấu trúc, không chạy sim
./v2/bin/v2 gate --fast            # cổng bắt buộc cuối mỗi task: fmt, clippy, luật cấu trúc, L0, gen-check (từ KT-05), L1 1.000 seed (từ khi có sim)
./v2/bin/v2 nightly                # L1 100.000 seed, mutation, Miri, soak (không chạy trong vòng task)

./vis-dev check             # dòng ViScript
./vis-dev gate --fast       # cổng bắt buộc cuối mỗi task ViScript
./vis-dev nightly
```

- Lệnh v2 nằm ở `v2/bin/v2` (không thể là `./v2` vì `v2/` là thư mục). Có thể `export PATH="$PWD/v2/bin:$PATH"` rồi gõ `v2 …`; tài liệu dùng dạng đầy đủ `./v2/bin/v2`.
- `./v2/bin/v2 nightly` và soak **không** chạy song song với build/cổng khác.
- **L1 đã bật** trong `gate --fast` từ KT-08 (1.000 seed; nightly 100.000 seed).
- Chưa có `gate` đầy đủ trong vòng 1; chỉ `gate --fast`.
- Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant (QD §5.2).

## 7. Commit

- Dòng OS: `[KT-##] <tóm tắt>` — ví dụ `[KT-07] boot riscv64 qua OpenSBI, 50 boot 0 lỗi`.
- Dòng ViScript: `[ST-##] <tóm tắt>` — ví dụ `[ST-04] lexer theo SPEC-lex`.
- Hậu tố task con giữ nguyên: `[KT-04a]`.
- Mỗi commit: `cargo fmt` đã chạy, cổng `--fast` xanh, header SPDX đúng loại tệp (QD §9.3: `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`).
- Commit chạm `kernel/` hoặc `lib/` phải xanh `--fast`; lỗi logic phải có `.scn` đỏ trước (D2-05).
- Hồi quy ⇒ **revert trước**, điều tra sau (D2-03).
- **Push sau mỗi commit xanh của task**: `git push origin main`; tag đẩy riêng `git push origin <tag>`. Không push khi cổng đỏ; không push commit WIP.

## 8. Duyệt

- Duyệt **chỉ** bằng một dòng trong tệp, không bằng chat:
  `APPROVED <YYYY-MM-DD> Agent 0` — ghi ngay dưới mục task trong `v2/agent/QUEUE.md` hoặc `viscript/agent/QUEUE.md`, kèm một mục D2/VD mới nếu quyết định thay đổi nội dung.
- `KT-04d` = **APPROVED 2026-11-12 Agent 0** (gói bản 2 + D2-35..43); **KT-05 mở**. `ST-02a` = **DONE** (APPROVED 2026-11-12). **Dòng ViScript: VÒNG 1 + VÒNG 2 (VS0) + VÒNG 3 (khung VS1) + VÒNG 4 (VS1b) + VÒNG 5 (VS1c) + VÒNG 6 (VSH1a) + VÒNG 7 (VSH1b) + VÒNG 8 (VSH1c) + VÒNG 9 (VSH2) đều DONE** — vòng 5 (`R5`): **507 test**, **346 ca**, parity **320/320**, `json.decode`/`std.fs`, pipe→stdin + `| complete`, generic thật. Vòng 6 (`R6`): **606 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity **344/344**, differential **100k 0 lệch**, **khe ống hợp nhất**, closure-arg + `where/each/first/count` + sugar `it`, `sort_by --desc`, prelude `FileEntry`/`ProcEntry` + **`VS0605`**, `get`/`select`, **`Render`** (bảng/khối), `ls`/`open` + **`--fs-mock`** ⇒ **§9.7 khối 672 chạy thật**; **allowlist doc-examples 8 → 6**. Còn **VSH1b** (cú pháp khai kiểu trong `.vish`, `where` kiểm tĩnh đủ, `--long`, `( … )` làm đối số) . Vòng 7 (`R7`): **651 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity **344/344**, differential **100k 0 lệch**, `.vish` **khai báo kiểu** (`type`/`let x: T`/`cmd … -> T`), `where` kiểm tĩnh đầy đủ, `ls --long` thật, `( … )` làm đối số, chẩn đoán **tuyệt đối** trong ngoặc; **allowlist 6**. Còn **VSH1b-2** (thân `cmd` pipeline, nhóm-lệnh có cờ, nhóm nhiều dòng), **14 nợ** trong `R7` §5, và **VS2** (async/kênh/net/Landlock thật/LSP/gói/**registry**/4 nền tảng) . Vòng 8 (`R8`): **704 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity **344/344**, differential **100k 0 lệch**, **thân `cmd` bằng pipeline lệnh**, **nhóm-lệnh có cờ + nhiều dòng + một đường hạ nhóm**, **`VS0405`** (một mã field-access, đúng **sổ kiểu**), `where ( x => x )` kiểm tĩnh, **stage đầu là biểu thức**; **allowlist 6**. Còn **9 nợ lõi** (`select`→`to json`, hai `sort_by`, `CODEGEN_TIN_TYCK`, `Ty::Any`, `&self`, mồ côi đa tệp, siêu trait, derives sinh mã, `Hop[Hop[int]]`) + **VS2** . Vòng 9 (`R9`): **730 test**, **370 ca/18 nhóm**, `vis test` **370 ok**, parity **344/344**, differential **100k 0 lệch**, **`to json`** + **một comparator** cho hai `sort_by`, **`CODEGEN_TIN_TYCK` 21 → 0** (codegen **từ chối** HIR có chẩn đoán — chặn bằng chẩn đoán, không bằng soi kiểu); **allowlist 6**. Còn **10 nợ có chủ đích** + **VS2** (bảng 8 hạng mục **có tiêu chí đo** trong `R9` §4) — **ViScript chưa hoàn thiện toàn bộ**
- Agent 2 soạn nháp spec ở trạng thái REVIEW; sau duyệt chỉ Agent 0 sửa (D2-22).
- Không tự duyệt, không tự suy ra "coi như đã duyệt".

## 9. QEMU và tiến trình nền

- **Cấm** `pkill -f qemu-system` toàn cục: chỉ giết **PID do chính task tạo** (`kill <pid>`, hoặc `pkill -P <pid>`).
- Tối đa 3 QEMU song song; sim dùng tối đa `số core − 1`.
- Máy bận/QEMU treo: giết đúng PID đã ghi, ghi `T-INFRA`, tiếp.

## 10. Trạng thái task

`QUEUE.md` dùng đúng 4 trạng thái: `READY | BLOCKED | REVIEW | DONE`.
`vifast-agentctl --project v2|viscript` : `next` (task kế có phụ thuộc đã DONE) **bỏ qua REVIEW**, `start <id>`, `done <id>`, `block <id> <lý do>`, `report`.
Mỗi task ≤ 3 giờ; task lớn hơn phải tách (QD §5.1, §10.4).

## 11. Định nghĩa "bàn giao xong"

Người mới mở máy, đọc **một** thư mục `agent/` + **hai** tài liệu là chạy được vòng lặp mà không hỏi ai: mọi task có (i) tiêu chí đo được, (ii) ngân sách, (iii) lệnh cổng, (iv) trạng thái.
