# STATE — ViFastOs v2 (vòng 1)

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.

```
HEAD          : 19659ee
BUILD_ID      : —
Task          : KT-03 (bắt đầu)
Bước          : 1/8 — đọc + lập kế hoạch nhỏ (KT-02 vừa DONE)
Nhánh         : main
Cổng gần nhất : cổng chưa có (./v2/bin/v2 chưa tồn tại — chính là KT-03)
Việc dở       : —
```

## Ghi chú

- **L1 chưa bật** tới KT-08 (chưa có sim). Từ giờ tới đó `./v2/bin/v2 gate --fast` chỉ gồm fmt, clippy, luật cấu trúc, L0.
- `./v2/bin/v2 check` / `./v2/bin/v2 gate --fast` **chưa tồn tại** cho tới khi **KT-03** xong ⇒ ô `Cổng gần nhất` ghi "cổng chưa có"; **không** tự tạo cổng ngoài task.
- Task `REVIEW` (KT-04d) **không** được `agentctl next` trả về; chỉ mở khi có dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `QUEUE.md`. **Không** ghi APPROVED.
- Tag v1: `v1-archive` trên `ca2eaf5` (KT-01). **Không** có `v1-final`; v1 không soak lại.
- Nhánh làm việc: `main`. Mỗi commit `[KT-##]`; chạm `kernel/`, `lib/` phải xanh `--fast` (D2-05). **Push sau mỗi commit xanh**: `git push origin main`.
- Toolchain: `cargo`/`rustc` 1.99.1 nằm ở `~/.cargo/bin`, **không có trên PATH** của shell mặc định ⇒ mọi lệnh phải `export PATH="$HOME/.cargo/bin:$PATH"`.
- **D2-27** (Agent 0, 2026-11-12): KT-04c/KT-04d = **21** kịch bản (20 theo QD §4 + ca "Exit khi procsrv chưa Recv"). Q-01 đóng.
- **D2-28** (Agent 0, 2026-11-12): KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`, điều kiện `cargo build` xanh + mỗi tệp ≤ 1.500 dòng.
- **Thứ tự điều phối chốt sau KT-02**: `KT-03` → **`KT-07`** (K0.5 boot riscv64, tạo `v2/kernel/`, `v2/boot/`, `v2/platform/`) → `KT-04a`. KT-07 chỉ phụ thuộc KT-02/03, không cần ABI freeze.

## Hiện trạng đo được (sau KT-02, 2026-11-12)

| Hạng mục | Hiện trạng | Dẫn chứng |
|---|---|---|
| `HEAD` | `19659ee` `[KT-01]` (trước commit KT-02) | `git log -1 --oneline` |
| Tag v1 | `v1-archive` (annotated) trên `ca2eaf5`, subject `T33a mở` | `git for-each-ref refs/tags/v1-archive` |
| `v2/` | workspace riêng: `Cargo.toml`, `LICENSE` (+`.license`), `README.md`, `bin/`, `lib/{vifast-elf,vifast-fs,vifast-fsutil}`, `tools/port/`, `agent/` | `find v2 -maxdepth 2` |
| Build `v2/` | `cargo build` **xanh** (3 crate); `cargo test -p vifast-fs` **52/52**; `cargo clippy --all-targets -- -D warnings` **0 warning**; `cargo fmt --all --check` sạch | `JOURNAL/2026-11-12.md` mục KT-02 |
| Luật 1.500 dòng | tệp lớn nhất `vifast-fs/src/fs.rs` = **1.248** dòng | `wc -l v2/lib/*/src/*.rs` |
| `spec/` | **chưa tồn tại** (KT-04a tạo `spec/abi.toml`) | `ls spec/` rỗng |
| `v2/bin/` | có `.gitkeep`; **chưa có** wrapper `v2` (KT-03) | `ls v2/bin` |
| PATH cargo | `~/.cargo/bin` ngoài PATH mặc định | `cargo --version` ⇒ `command not found` nếu chưa export |

## Việc đang làm — KT-03 (wrapper `v2/bin/v2` + `check`/`gate --fast` khung)

Ngân sách 3 h. DoD (QUEUE): `./v2/bin/v2 check` chạy từ repo root; 5 luật có 5 case âm. **Chưa** gen-check, **chưa** L1.

_Kế hoạch nhỏ điền ở bước 2/8 sau khi review vòng này trả kết quả._

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-12 | KT-03 | 1/8 | `19659ee` | — | cổng chưa có | bắt đầu KT-03 sau khi KT-02 DONE |
| 2026-11-12 | **KT-02** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: workspace `v2/` + `bin/` + port `vifast-fs`(tách 10 tệp, max 1.248 dòng)/`vifast-fsutil`/`vifast-elf`; build xanh, 52/52 test, clippy 0 warning |
| 2026-11-12 | KT-02 | 2–7/8 | `329177e` | — | cổng chưa có | tách 4.543 dòng v1 → 10 tệp; `cargo fix` gỡ 13 import; D2-27/D2-28 nhận trong lúc làm |
| 2026-11-12 | KT-02 | 1/8 | `329177e` | — | cổng chưa có | lập kế hoạch nhỏ 4 bước; review ON-TRACK |
| 2026-11-12 | **KT-01** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: tag annotated `v1-archive` trên `ca2eaf5` (subject "T33a mở"); 7/7 mục KNOWN-ISSUES khớp QD §6; test đỏ **N/A** (task tài liệu) — commit `19659ee` |
| 2026-11-12 | KT-01 | 3–7/8 | `329177e` | — | cổng chưa có | đỏ trước: `git tag -l v1-archive` rỗng; tạo tag; dán review vào `JOURNAL/review-2026-11-12-1.md`; sửa câu chữ Exit + mở Q-01 ở `BLOCKED.md` |
| 2026-11-12 | KT-01 | 1/8 | `329177e` | — | cổng chưa có | mở vòng 1: đọc 7 tệp + STATE + review subagent trước task đầu |
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 17 dòng task, 5 READY (KT-01, KT-02, KT-04a, KT-T26, KT-R1), KT-04d REVIEW |
