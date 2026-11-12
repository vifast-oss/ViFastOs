# STATE — ViFastOs v2 (vòng 1)

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.

```
HEAD          : 329177e
BUILD_ID      : —
Task          : KT-02 (bắt đầu)
Bước          : 1/8 — đọc + lập kế hoạch nhỏ (KT-01 vừa DONE)
Nhánh         : main
Cổng gần nhất : cổng chưa có (KT-03 chưa xong; ./v2/bin/v2 chưa tồn tại)
Việc dở       : —
```

## Ghi chú

- **L1 chưa bật** tới KT-08 (chưa có sim). Từ giờ tới đó `./v2/bin/v2 gate --fast` chỉ gồm fmt, clippy, luật cấu trúc, L0.
- `./v2/bin/v2 check` / `./v2/bin/v2 gate --fast` **chưa tồn tại** cho tới khi KT-03 xong ⇒ ô `Cổng gần nhất` ghi "cổng chưa có"; **không** tự tạo cổng ngoài task.
- Task `REVIEW` (KT-04d) **không** được `agentctl next` trả về; chỉ mở khi có dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `QUEUE.md`. **Không** ghi APPROVED.
- Tag v1: `v1-archive` trên `ca2eaf5` (KT-01). **Không** có `v1-final`; v1 không soak lại.
- Nhánh làm việc: `main`. Mỗi commit `[KT-##]`; chạm `kernel/`, `lib/` phải xanh `--fast` (D2-05).

### Hiện trạng đo được lúc nhận bàn giao (2026-11-12)

| Hạng mục | Hiện trạng | Dẫn chứng |
|---|---|---|
| `HEAD` | `329177e` `[KT-00] chốt lệnh v2/bin/v2 + D2-26 copy thư viện v1` | `git log -1 --oneline` |
| Tag v1 | **chưa có** `v1-archive` (chỉ `overnight-start`, `v0.1.0`) | `git tag -l` |
| `docs/v1/` | đủ: 101 `agent2-report-*.md` + `overnight-log.md` + `decisions/` + `KNOWN-ISSUES.md` (7 mục) + `README.md` | `ls docs/v1/` |
| `spec/` | **chưa tồn tại** (KT-04a tạo `spec/abi.toml`) | `ls spec/` rỗng |
| `v2/` | chỉ có `agent/` (QUEUE/STATE/BLOCKED/DECISIONS/AGENTS.md + `JOURNAL/`, `reports/`); **chưa có** `bin/`, `lib/`, `Cargo.toml` | `find v2 -type f` |
| Toolchain | `cargo 1.99.1`, `rustc 1.99.1`; **`~/.cargo/bin` không nằm trên PATH của shell** ⇒ mọi lệnh phải `export PATH="$HOME/.cargo/bin:$PATH"` | `cargo --version` khi chưa export ⇒ `command not found` |
| Tệp chưa track ở root | `ViFastOs-dossier.md`, `review/GUI-AGENT0.md`, `review/work/*` — **không phải của Agent 2**, không commit | `git status --porcelain` |

### Vòng 1 — thứ tự theo `docs/v2/AGENT2-PROMPT.md` §6

`KT-01` → `KT-02` → `KT-04a` → `KT-T26` → `KT-R1`; sau `KT-02` mở `KT-03`, `KT-06`, `KT-08`, `KT-12`.
Không làm `KT-04b/c/d`, `KT-05` cho tới khi `KT-04a` xong.

## Việc đang làm — KT-02 (khung `v2/` + `v2/bin/` + port `vifast-fsutil`)

Ngân sách 3 h. DoD (QUEUE:100): `cargo build` workspace xanh; `ls v2/agent` đủ tệp; có `v2/bin/`.

| Bước | Nội dung | Tệp chạm | Test/kiểm |
|---|---|---|---|
| 1 | Workspace `v2/Cargo.toml` (resolver 2, `license.workspace = true`, `rust-version` 1.99.1), `v2/README.md` + `v2/LICENSE` tham chiếu root (chỉ một bản LICENSE-MIT/APACHE ở gốc — QD §9.3) | `v2/Cargo.toml`, `v2/LICENSE`, `v2/README.md` | `cargo build` từ `v2/` xanh |
| 2 | Port `vifast-fsutil` (bin, 328 dòng) + `vifast-fs` (lib `no_std`, 4.543 dòng) vào `v2/lib/`, **không** path-dep sang `crates/` (D2-26): giữ SPDX `//` + `PORTED-FROM: crates/<tên>@ca2eaf5` | `v2/lib/vifast-fs/**`, `v2/lib/vifast-fsutil/**` | build + `cargo clippy` sạch |
| 3 | Tách `vifast-fs/src/lib.rs` thành tệp ≤ **1.500 dòng** [TK] (D2-06 cổng cứng) — impl `Fs` và `mod tests` chia theo nhóm chức năng | `v2/lib/vifast-fs/src/*.rs` | `cargo test -p vifast-fs` xanh; `wc -l` mọi tệp ≤ 1500 |
| 4 | `v2/bin/` (thư mục cho KT-03) + `v2/agent/` đủ tệp | `v2/bin/.gitkeep` | `ls v2/agent` đủ 7 mục |

- Kế hoạch nhỏ ghi ở đây **trước** khi sửa tệp (bước 2/8 của vòng lặp).
- Cổng: chưa có `./v2/bin/v2` (KT-03) ⇒ cuối task chạy `cargo build`/`test`/`clippy` của workspace `v2/` và ghi rõ "cổng chưa có"; **không** tự tạo wrapper (việc của KT-03).

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-12 | KT-02 | 1/8 | `329177e` | — | cổng chưa có | lập kế hoạch nhỏ 4 bước; chờ review đã ON-TRACK |
| 2026-11-12 | **KT-01** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: tag annotated `v1-archive` trên `ca2eaf5` (subject "T33a mở"); 7/7 mục KNOWN-ISSUES khớp QD §6; test đỏ **N/A** (task tài liệu) |
| 2026-11-12 | KT-01 | 3–7/8 | `329177e` | — | cổng chưa có | đỏ trước: `git tag -l v1-archive` rỗng; tạo tag; dán review vào `JOURNAL/review-2026-11-12-1.md`; sửa câu chữ Exit + mở Q-01 ở `BLOCKED.md` |
| 2026-11-12 | KT-01 | 1/8 | `329177e` | — | cổng chưa có | mở vòng 1: đọc 7 tệp + STATE + review subagent trước task đầu |
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 17 dòng task, 5 READY (KT-01, KT-02, KT-04a, KT-T26, KT-R1), KT-04d REVIEW |
