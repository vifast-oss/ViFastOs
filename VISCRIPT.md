# ViScript đã chuyển sang repo riêng

**ST-171 (VD-165 + VD-179 §4)** — mã nguồn ViScript **không còn** trong repo này.

| Trước (repo này) | Sau (repo `ViScript`) |
|---|---|
| `viscript/` — workspace 8 crate + `bin/` + `test/` + `agent/` + `infra/` | gốc repo `ViScript`: `crates/`, `bin/`, `test/`, `agent/`, `infra/`, `Cargo.toml` |
| `docs/viscript/` — `SPEC-lex.md`, `ViScript-thiet-ke.md`, `doc-examples-allow.txt` | `ViScript/docs/viscript/` (giữ nguyên tên thư mục) |
| `vis-host-linux/`, `vis-host-wasm/` — workspace riêng, path dep của `vis-cli` | `ViScript/vis-host-linux/`, `ViScript/vis-host-wasm/` |
| `vis-dev`, `vis` — shim gốc repo gọi `viscript/bin/…` | `ViScript/vis-dev`, `ViScript/vis` (gọi `bin/…`) |

- **Đường dẫn repo mới (local)**: `/Users/mps/dvchd/ViScript`
- **Lịch sử**: **459 commit** giữ lại (lọc từ 1791 commit của `ViFastOs`), gồm **mọi** commit `[ST-…]`
  của dòng ViScript. Ánh xạ SHA cũ → mới: `ViScript/agent/JOURNAL/st171-commit-map.txt`.
- **Sổ nợ / QUEUE / DECISIONS / JOURNAL / báo cáo `R1..R29`** đã chuyển sang `ViScript/agent/`;
  số nợ và số task **giữ nguyên** (#146, #140, ST-171…), không đánh lại.
- **Cổng của dòng ViScript nay chạy trong repo `ViScript`**: `./vis-dev gate --fast`.
- **Remote**: **chưa có** — BLOCKED-CHO-A0 (cần URL + quyền tạo `vifast-oss/ViScript`).

Không sửa mã ViScript trong repo này nữa; mọi thay đổi làm ở repo `ViScript`.
