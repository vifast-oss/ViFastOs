# AGENTS.md — luật làm việc trong repo ViFastOs

Cập nhật: 2026-11-24 (D2-256: lịch vận hành theo khung giờ DeepSeek; KT-242: ba tầng kiểm, tầng L1, tái dùng bằng chứng). Nguồn sự thật của vòng vá:
[`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md) (gọi tắt **QD**).
Tệp này áp cho **mọi** agent làm việc trong repo. Hai dòng sản phẩm có bản rút gọn:
[`v2/AGENTS.md`](v2/AGENTS.md). **Dòng ViScript đã tách sang repo riêng** — xem [`VISCRIPT.md`](VISCRIPT.md).

## 1. Thứ tự nguồn sự thật

Khi hai nguồn nói khác nhau, **nguồn đứng trước thắng**:

1. **Code + harness** — thứ chạy được và đo được: `crates/`, `v2/`, sim, tệp `.scn`, TAP, log cổng.
2. **`spec/`** — `spec/abi.toml`, `spec/kernel.toml`: một nguồn sự thật duy nhất cho ABI. (Ngữ nghĩa ViScript: `docs/SPEC-lex.md` trong repo **ViScript**.)
3. **`v2/agent/DECISIONS.md`** (D-01..D-08, D2-01..D2-95). (Quyết định ViScript VD-01…: `agent/DECISIONS.md` trong repo **ViScript**.)
4. **Tài liệu thiết kế** — [`docs/v2/ViFastOs-v2-thiet-ke.md`](docs/v2/ViFastOs-v2-thiet-ke.md). (Thiết kế ViScript: `docs/ViScript-thiet-ke.md` trong repo **ViScript**.)

QD mạnh hơn hai tài liệu thiết kế. Không có nguồn nào cho phép **nới ngưỡng để qua cổng**.

Nhãn số liệu (bắt buộc, QD §2.2): `[V1:CODE]` `[V1:ĐO]` `[V1:DOC]` (từ dossier, giữ độ tin cậy gốc), `[TK]` thiết kế mới (**mặc định** cho dòng không nhãn), `[GT]` giả thuyết, `[ƯL]` số ước lượng cần đo. Hằng số thiết kế là `[TK]`, **không** phải `[ƯL]`. Số đo chỉ do `./v2/bin/v2 nightly` / `vis-dev nightly` sinh; tài liệu không chứa số đo viết tay (QD §9 Q12).

## 2. Bản đồ tài liệu

| Nhóm | Đường dẫn | Trạng thái |
|---|---|---|
| Quyết định vòng vá | `docs/QUYET-DINH-sau-review.md` | nguồn sự thật, **cấm sửa** |
| Thiết kế v2 | `docs/v2/ViFastOs-v2-thiet-ke.md` | cấm sửa (chỉ Agent 0 / Agent 1 theo QD) |
| Điều khiển OS | `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`, `JOURNAL/`, `reports/` | xem bảng chủ sở hữu §3 |
| Lưu trữ v1 | `docs/v1/` (toàn bộ báo cáo `agent2-report-*`, `overnight-log.md`, `decisions/`, `KNOWN-ISSUES.md`) | **đọc-only** |

## 3. Ai ghi tệp nào

| Tệp | Người ghi | Ghi chú |
|---|---|---|
| `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md` | **Agent 0** (qua người điều phối) | Agent 2 **không** sửa; cần đổi thì BLOCKED + đề xuất |
| `v2/agent/STATE.md`, `v2/agent/BLOCKED.md`, `v2/agent/JOURNAL/`, `v2/agent/reports/` | **Agent 2** dòng OS | STATE cập nhật **sau mỗi bước** |
| `spec/` | chỉ task được ghi rõ (KT-04a, KT-04b một lần theo D2-32, **KT-05** một lần theo D2-33/D2-35, ST-02a, ST-02b chỉ bảng mã lỗi) | xem §4 |
| `v2/AGENTS.md`, `AGENTS.md` | người điều phối / Agent 0 | Agent 2 không sửa |

Hai QUEUE **độc lập**: một task OS bị BLOCKED **không** chặn dòng ViScript và ngược lại (trừ cặp phụ thuộc ghi trong chính QUEUE).

## 4. Cấm

Cấm tuyệt đối, không có ngoại lệ "tạm thời":

1. **Sửa `spec/`** trừ task được ghi rõ trong QUEUE (`KT-04a`: `spec/abi.toml`, `spec/kernel.toml`; `KT-04b`: một lần theo D2-32, chỉ chép draft reserved đã sửa + D2-29; `KT-05`: một lần, chỉ chép `[method_signature]` + `[debug_info].return_channel` theo D2-33/D2-35; `ST-02a`: `docs/SPEC-lex.md` (nay ở repo ViScript); `ST-02b`: chỉ bảng mã lỗi trong SPEC/phụ lục, theo VD-27). Mọi thay đổi ABI/ngữ nghĩa khác phải qua Agent 0 (D2-04, D2-22).
2. Sửa `accept()` / kỳ vọng / snapshot của harness để test xanh.
3. Thêm `#[ignore]`, `#[should_panic]` để né, tắt test, hoặc bỏ case khỏi danh sách chạy.
4. **Nới ngưỡng** (thời gian, seed, số ca đỏ, `MAX_IO`, `MAX_TCBS`, `MAX_OBJECTS`, `KERNEL_STATIC_BUDGET`).
5. **Retry** để qua cổng (D2-07). Flake ghi `T-INFRA` kèm log; chạy lại **tối đa một lần** chỉ để phân loại, không để lấy xanh.
6. Sửa `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`.
7. Sửa bất kỳ tệp nào trong `docs/v1/` (kho lưu trữ v1, tag `v1-archive`).
8. Thêm `unsafe` ngoài danh sách crate đóng (D2-17); thêm `cfg(target_arch)` vào kernel/runtime.
9. Thêm phụ thuộc crate ngoài allowlist giấy phép (D2-21, QD §9.3).
10. Đưa số đo viết tay vào tài liệu.
11. **Dùng GitHub Actions / CI trên GitHub** (D2-241, chỉ đạo trực tiếp của người điều phối 2026-11-22).
    Không tạo `.github/workflows/**`, không workflow nào khác của GitHub. Cổng chạy **trên máy**
    (`./v2/bin/v2 check|gate --dev|gate --fast|nightly`); nếu cần chạy định kỳ thì dùng script +
    bộ hẹn giờ trên máy, **không** dùng dịch vụ CI ngoài. Lý do: cổng có bước QEMU nhiều phút và
    ngưỡng thời gian/đo đạc chỉ đúng trên máy đo — CI ngoài sẽ sinh bằng chứng **không hợp lệ**
    mà vẫn xanh, tức là bỏ âm thầm phép kiểm (trái §4.2/§4.3).

## 5. v1 đã đóng băng

- Tag **`v1-archive`** trên commit `ca2eaf5`. **Không** có tag `v1-final`.
- `docs/v1/` là **đọc-only**: toàn bộ báo cáo `agent2-report-*` của v1, `overnight-log.md`, `decisions/`, `KNOWN-ISSUES.md`, `README.md`.
- **Không soak lại v1**, không phát triển v1, không mở tính năng trên v1 (D-02 hết hiệu lực từ khi lưu trữ, QD §2.3).
- Sổ lỗi v1 còn mở đã mang sang v2 dưới dạng ca kiểm: [`docs/v1/KNOWN-ISSUES.md`](docs/v1/KNOWN-ISSUES.md).

## 6. Lệnh

```bash
./v2/bin/v2 check                  # nhanh: fmt + clippy + luật cấu trúc, không chạy sim
./v2/bin/v2 gate --dev             # T1: kiểm tích hợp nhanh (§6.1) — KHÔNG phải full gate
./v2/bin/v2 gate --fast            # T2: cổng đầy đủ, bắt buộc trên ứng viên tích hợp
./v2/bin/v2 nightly                # L1 100.000 seed, mutation, Miri, soak (không chạy trong vòng task)

./vis-dev check             # dòng ViScript
./vis-dev gate --fast       # cổng bắt buộc cuối mỗi task ViScript
./vis-dev nightly
```

- Lệnh v2 nằm ở `v2/bin/v2` (không thể là `./v2` vì `v2/` là thư mục). Có thể `export PATH="$PWD/v2/bin:$PATH"` rồi gõ `v2 …`; tài liệu dùng dạng đầy đủ `./v2/bin/v2`.
- `./v2/bin/v2 nightly` và soak **không** chạy song song với build/cổng khác.
- **L1 đã bật** trong `gate --fast` từ KT-08 (1.000 seed; nightly 100.000 seed).
- Chưa có `gate` đầy đủ trong vòng 1; `gate --fast` là **T2** (đầy đủ nhất hiện có).
- Đo thời gian L1 ngay khi sim chạy; vượt 60 s thì giảm seed của `--fast`, **không** giảm invariant (QD §5.2).

### 6.1 Ba tầng kiểm (KT-242, A0 2026-11-20)

| Tầng | Lệnh | Nội dung | Khi chạy |
|---|---|---|---|
| **T0** | agent tự chạy | `cargo test -p <crate>`, test đỏ-trước, checker liên quan | trong lúc sửa |
| **T1** | `./v2/bin/v2 gate --dev` | fmt/lint + luật + checker + compile-fail + L0 + L1 1.000 + smoke QEMU; **bỏ 8 bước selftest harness** | trước khi tích hợp nhóm thay đổi |
| **T2** | `./v2/bin/v2 gate --fast` | đầy đủ mọi bước | trên ứng viên tích hợp / bàn giao |

- **T1 không phải full gate.** Không được gọi kết quả T1 là "full gate PASS"; output T1 in rõ nhãn.
- Không bỏ âm thầm bước nào: mỗi bước bị bỏ in `--- <tên>: BỎ QUA (T1 — chỉ chạy ở T2)`.
- **T2 bắt buộc ngay** (không trì hoãn) khi thay đổi chạm: đồng bộ SMP/memory ordering/vòng đời
  đối tượng; page table/quyền truy cập/revoke/allocator; boot/linker/layout ảnh/toolchain; persist
  và toàn vẹn dữ liệu; bản sửa cho lỗi chỉ tái hiện dưới stress.
- `main` có thể ở trạng thái "đã kiểm T1, chờ T2" — phải ghi rõ; **không** gọi là ứng viên nghiệm thu.

### 6.2 Tầng L1

| Mức | Seed | Dùng khi |
|---|---:|---|
| **L1-smoke** | 1.000 (mặc định của `gate --fast`/`--dev`) | mọi lần T1/T2 |
| **L1-medium** | 10.000 | thay đổi chạm sim/scheduler/IPC |
| **L1-full** | 100.000 × 4 cấu hình = 400k (hợp đồng K6) | ứng viên nghiệm thu |

```bash
bash v2/tools/l1-tier.sh --tier smoke|medium|full   # tự chia lô, gộp bảng, đỏ nếu thiếu seed
bash v2/tools/l1-tier.sh --selftest                 # 27 ca, không chạy sim
```

Không gọi smoke/medium là "tương đương full". Chia lô **không** đổi seed/`SO_OP`/cấu hình/ngưỡng.

### 6.3 Tái dùng bằng chứng (chống chạy lặp vô ích)

Mỗi phép đo lưu **khóa đầu vào**: `commit` + `build-inputs` + `toolchain` + `config/seed` +
`binary.sha256` (+ dòng `# build:` của ảnh nếu dùng ảnh) + lệnh + kết quả + log.

- Chỉ sửa tài liệu/runner **không** ảnh hưởng phép kiểm ⇒ **giữ bằng chứng cũ**, ghi lý do.
- **Không** chỉ nhìn SHA ảnh để quyết định tái dùng L1: L1 còn phụ thuộc source mô phỏng, harness,
  cấu hình, seed.
- **Nợ P20**: sha256 ảnh **phụ thuộc thư mục build**; danh tính bền là dòng
  `# build: algo=… inputs=… dirty=…`.
- Thiếu dữ liệu / không chắc ⇒ **chạy**, không PASS mặc định.
- Mặc định **≤ 1 đợt T2 theo lịch / 24 giờ**, cộng 1 đợt trên ứng viên bàn giao nếu đầu vào đổi;
  vượt phải có lý do kỹ thuật và ghi lại.
- Không chạy lại L1-full/soak trên cùng fingerprint chỉ để tăng số lần PASS.

### 6.4 Lịch vận hành theo khung giờ DeepSeek (D2-256)

Tối ưu chi phí token và tận dụng tối đa máy đo cục bộ theo biểu giá giờ của DeepSeek API
(nguồn chuẩn **`UTC`**; thấp điểm giảm **50 %** mọi loại token, cao điểm **2×** chỉ áp dụng
Thứ 2–Thứ 6 trừ lễ công Trung Quốc). Kiểm tra khung hiện tại trước khi mở phiên bằng:
`date -u '+%u %H:%M UTC'` (`1..5` = T2–T6, `6..7` = cuối tuần).

| Khung giờ | `UTC` (chuẩn) | `ICT` (`UTC+7`, VN) | `JST` (`UTC+9`, máy đo) | Biểu giá | Chế độ bắt buộc |
|---|---|---|---|---|---|
| **Cao điểm 1** *(3 h, T2–T6)* | `01:00–04:00` | `08:00–11:00` | `10:00–13:00` | **2×** | **CHẠY NẶNG**: T2 gộp lô (`gate --fast`), `repro-check.sh --arch`, `l1-tier.sh --tier medium`, `vifast-bench`, bảo trì máy |
| **Khe thấp điểm trưa** *(2 h, T2–T6)* | `04:00–06:00` | `11:00–13:00` | `13:00–15:00` | **0,5×** | **PHÁT TRIỂN NGẮN (≤ 2 h)**: triage log Cao điểm 1, bugfix gọn, journal/RFC, chốt ứng viên sạch trước `05:50 UTC` |
| **Cao điểm 2** *(4 h, T2–T6)* | `06:00–10:00` | `13:00–17:00` | `15:00–19:00` | **2×** | **CHẠY NẶNG DÀI NHẤT**: `v2 nightly --all` (L1 400k + Miri + `mutants`), `nightly --soak` (3 arch + đĩa thật), `l1-tier.sh --tier full` |
| **Thấp điểm chính** *(15 h liên tục, T2–T6)* | `10:00–01:00` (+1) | `17:00–08:00` (+1) | `19:00–10:00` (+1) | **0,5×** | **PHÁT TRIỂN TÍNH NĂNG & REFACTOR**: mở tối đa 4 slot worktree (`wt.sh`), viết mã/test đỏ-trước/RFC, sub-agent chạy T0+T1 |
| **Cuối tuần & lễ công TQ** *(63 h liên tục)* | T6 `10:00` → T2 `01:00` | T6 `17:00` → T2 `08:00` | T6 `19:00` → T2 `10:00` | **0,5×** | **TỰ DO**: chạy đầy đủ cả phát triển 4 slot song song lẫn các đợt T2/nightly giữa chặng |

**Quy tắc thi hành:**

1. **Biên an toàn 10 phút (`00:50 UTC` & `05:50 UTC`, T2–T6)**: ngừng mở task code mới và ngừng
   spawn sub-agent mới; chốt commit + T1 đang dở để bàn giao nhánh cho Lead chạy cổng nặng đúng
   lúc bước sang Cao điểm.
2. **Giờ Cao điểm — 0/tối thiểu token LLM**:
   - **Cấm** spawn nhiều sub-agent phát triển tính năng mới trong khung `01:00–04:00` và
     `06:00–10:00 UTC` (T2–T6).
   - Toàn bộ việc chạy nặng chạy độc quyền trên máy đo (`qemu_foreign=0`, `dirty=0`) bằng script /
     bộ hẹn giờ cục bộ (**không** GitHub Actions theo §4.11) và ghi log ra tệp.
   - Nếu cổng nặng hoặc soak báo **ĐỎ** trong giờ Cao điểm: chỉ lưu log + phân loại `T-INFRA`
     (tối đa 1 lượt ngắn của Lead); **dồn vòng điều tra/vá mã nhiều lượt LLM sang khung Thấp điểm
     kế tiếp** (`04:00 UTC` hoặc `10:00 UTC`).
3. **Giờ Thấp điểm — tối đa hoá năng suất code (giá 0,5×) + Prompt Cache**:
   - Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; Lead chỉ chạy **T2 (`v2 gate --fast`,
     ~10 phút)** trong khối 15 h đêm khi cần mở khoá đường găng cho đợt kế — còn toàn bộ `nightly --all`,
     `soak`, `L1-full`, `repro-check` 3 arch gom chạy ở giờ Cao điểm.
   - Giữ đầu ngữ cảnh ổn định (`AGENTS.md`, `STATE.md`, `QUEUE.md` gọn), tránh chèn timestamp biến đổi
     ở đầu prompt, giữ đúng 1 tệp `JOURNAL/kt<n>-<ten>.md`/task để tối đa hoá DeepSeek prompt cache-hit.
4. **Không nới luật vì lịch giờ**: lịch giờ chỉ đổi **thời điểm chạy**, không bỏ T2 trên ứng viên
   tích hợp (§6.1), không retry lấy xanh (§4.5), và tuân thủ tái dùng bằng chứng (§6.3 — cùng
   fingerprint không chạy lại `L1-full`/soak vô ích chỉ vì đang là giờ Cao điểm).

## 7. Commit

- Dòng OS: `[KT-##] <tóm tắt>` — ví dụ `[KT-07] boot riscv64 qua OpenSBI, 50 boot 0 lỗi`.
- Dòng ViScript: `[ST-##] <tóm tắt>` — ví dụ `[ST-04] lexer theo SPEC-lex`.
- Hậu tố task con giữ nguyên: `[KT-04a]`.
- Mỗi commit: `cargo fmt` đã chạy, cổng `--fast` xanh, header SPDX đúng loại tệp (QD §9.3: `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`).
- Commit chạm `kernel/` hoặc `lib/` phải xanh `--fast`; lỗi logic phải có `.scn` đỏ trước (D2-05).
- Hồi quy ⇒ **revert trước**, điều tra sau (D2-03).
- **Push sau mỗi commit xanh của task**: `git push origin main`; tag đẩy riêng `git push origin <tag>`. Không push khi cổng đỏ; không push commit WIP.
- Hook `vifast-guard` đòi `VIFAST_ROLE=lead` khi commit/push lên nhánh bảo vệ. Agent **không** tự
  đặt biến này và **không** dùng `--no-verify`: stage xong thì báo lead commit.

## 8. Duyệt

- Duyệt **chỉ** bằng một dòng trong tệp, không bằng chat:
  `APPROVED <YYYY-MM-DD> Agent 0` — ghi ngay dưới mục task trong `v2/agent/QUEUE.md` (ViScript: `agent/QUEUE.md` trong repo ViScript), kèm một mục D2/VD mới nếu quyết định thay đổi nội dung.
- `KT-04d` = **APPROVED 2026-11-12 Agent 0** (gói bản 2 + D2-35..43); **KT-05 mở**. `ST-02a` = **DONE** (APPROVED 2026-11-12). **Dòng ViScript đã tách sang repo riêng** (`VISCRIPT.md`): đọc `agent/ROADMAP.md`, báo cáo mới nhất `ls agent/reports/ | sort -V | tail -1`, rồi `agent/QUEUE.md` **trong repo ViScript**. Tệp luật không chứa số đếm để không lỗi thời.
**Quy trình hiện hành**: VD-119 (Agent 0 soạn, Agent 2 dán nguyên văn, commit `[ST-00]`).
- Agent 2 soạn nháp spec ở trạng thái REVIEW; sau duyệt chỉ Agent 0 sửa (D2-22).
- Không tự duyệt, không tự suy ra "coi như đã duyệt".

## 9. QEMU và tiến trình nền

- **Cấm** `pkill -f qemu-system` toàn cục: chỉ giết **PID do chính task tạo** (`kill <pid>`, hoặc `pkill -P <pid>`).
- Tối đa 3 QEMU song song; sim dùng tối đa `số core − 1`.
- Máy bận/QEMU treo: giết đúng PID đã ghi, ghi `T-INFRA`, tiếp.

## 10. Trạng thái task

`QUEUE.md` dùng đúng 4 trạng thái: `READY | BLOCKED | REVIEW | DONE`.
`vifast-agentctl --project v2` : `next` (task kế có phụ thuộc đã DONE) **bỏ qua REVIEW**, `start <id>`, `done <id>`, `block <id> <lý do>`, `report`. (ViScript: dùng `agent/` trong repo ViScript.)
Mỗi task ≤ 3 giờ; task lớn hơn phải tách (QD §5.1, §10.4).

## 11. Định nghĩa "bàn giao xong"

Người mới mở máy, đọc **một** thư mục `agent/` + **hai** tài liệu là chạy được vòng lặp mà không hỏi ai: mọi task có (i) tiêu chí đo được, (ii) ngân sách, (iii) lệnh cổng, (iv) trạng thái.
