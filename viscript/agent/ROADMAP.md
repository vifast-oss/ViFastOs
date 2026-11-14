<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# ROADMAP bàn giao — dòng ViScript (sau R16 / vòng 16)

Tài liệu **một trang** cho người tiếp nhận: 8 hạng mục VS2, **mức thật đang có** (đo được), bằng chứng,
và **điều kiện đóng 100%**. Không có mục nào dưới đây được coi là “xong” nếu chưa đạt cột 4.
Số đo lấy từ `reports/R15-viscript.md` + `R16-viscript.md` (đo lại 2026-11-14, macOS aarch64).

## A. Tám hạng mục VS2

| # | Hạng mục | Mức thật (đo được, **không** vượt mức) | Bằng chứng (task/hash/số) | Điều kiện để đóng 100% |
|---|---|---|---|---|
| 1 | **Async** | `async fn`/`await`/`spawn`/join cuối; scheduler **một luồng, run-to-completion**, **không song song thật**, **không GC**; 10k task interp **316 ms** / VM **168 ms**; drain 100 task đúng thứ tự | ST-60/61/94/98; `async_10k`; `R15`/`R16` §3; `3840d463` | Scheduler có **chạy song song thật**/work-stealing + suspend-resume; test **tăng tốc** trên ≥2 core; `async` method/`cmd`; chính sách bộ nhớ (GC/ownership) đo được |
| 2 | **Kênh** | `chan[T](n≥1)` send/recv/close; **`chan(0)` bị từ chối** `VS0505` ngay khi tạo (**không** rendezvous), exit 1; `recv` sau close `none`; `send` sau close `VS0504`; deadlock `VS0505` không treo; drain `Moi` 100 task thứ tự spawn | ST-61/94; `kenh.rs` + `kenh_cap0.rs`; `fe6d459b`; soak 8/8 | **Wait-set/chuyển task treo** + rendezvous `chan(0)` đúng nghĩa (hoặc tuyên bố từ chối vĩnh viễn có văn bản); `select`/timeout; test hai đường |
| 3 | **Net/HTTP** | `NetHost` TCP **loopback** thật, `http.get`; **không TLS/redirect/keep-alive/chunked**; **một luồng**, phục vụ tuần tự; cap body 8 MiB; 24 ca net/http | ST-70/71/74; `R15` §4 hàng 3; net không nằm trong host suite | **TLS** (rustls/webpki — cần duyệt crate), redirect, chunked/keep-alive, timeout, ≥2 kết nối **đồng thời**; test loopback + Internet thật có kiểm chứng |
| 4 | **Quyền** | `cap[Dir]/File/Net/Proc`; `--strict-caps` cưỡng chế biên host (interp+VM); `--grant` Dir/File/Net = đường dẫn, **Proc = allowlist lệnh** (basename/canonical); `VS0604` exit 2; **chưa siết mặc định**; **chưa Landlock thật**; con **không sandbox** | ST-72b/81/93/97; `quyen.rs` 460 dòng; `cap_proc*` ca; `0397c5c6` | **Siết mặc định**; **Landlock** (crate `landlock` **hoặc** `unsafe` được duyệt) cưỡng chế **kernel** trên Linux; `cap[Proc]` allowlist + **sandbox tiến trình con**; test Linux thật |
| 5 | **LSP** | `vis lsp --stdio`: diagnostics **module-aware** = `vis check`, UTF-16; completion sau `.` theo kiểu tĩnh receiver đơn giản (field kind 5, method kind 2; 50→2 mục); **chưa** hover field/đa tài liệu; full sync | ST-91; `lsp_std*.rs` **39 ca**; `R15` §4 hàng 5 | Hover field + đa tài liệu + generic/`list[P]`/`f().`; bảng `std.str`; publish tệp phụ; incremental sync; test LSP thật |
| 6 | **Gói** | Registry **local**, manifest `files[]`, lock SHA-256, `.vis/store` content-addressed, `use pkg.*`, `VS0701/0702`, lock `entry` + `pkg remove`; **chưa** dải phiên bản/registry HTTP/ký số | ST-64/66/69/80/90; `pkg*.rs`; `R15` §4 hàng 6 | **Semver ranges** + resolver; **registry HTTP**; **chữ ký số**; `pkg update`; migration lock cũ; GC store |
| 7 | **Đa nền tảng** | **Chỉ `cargo check`**: ma trận **18/18 ô PASS** (macOS/Linux/Windows × wasm32/wasm32-wasip1/aarch64/x86_64…); **chưa runner** Windows/Linux/wasm | ST-58/73/86; `vis-targets` 18/18; `R15`/`R16` §3 | **Runner thật**: wasm32-wasip1 + wasmtime; Linux (Landlock) + Windows; CI chạy **54 host** trên ≥2 nền; không chỉ check |
| 8 | **Hợp chuẩn host** | **54 ca chạy thật trên macOS**, **120 lần, 0 lệch**; **VM 54/54**; cache **520×4 0 lệch**; net **không** nằm trong suite | ST-87/88/89/98; `HOST-BAO-DAM.md`; soak 8/8, host 9+7 s | 54 ca × **4 nền thật** (macOS/Linux/Windows/wasm), có net/TLS trong suite; CI lặp lại; đo hiệu năng từng nền |

## B. Cách chạy tiếp

### B1. Lệnh cổng + soak (tuần tự, **không** chạy song song — AGENTS §6)

```bash
cd viscript
cargo test --workspace                 # 1010 test
cargo test -p vis-cli --test sem       # 447 ca / 22 nhóm
target/debug/vis test test/sem         # 447 ok (interp)
target/debug/vis test --vm test/sem    # 447 ok + 0 SKIP (VM)
../vis-dev gate --fast                 # cổng bắt buộc: fmt, clippy, R1..R4, L0, report-check
bin/vis-soak                           # soak 8 vòng tuần tự (~14 phút trên máy này)
VIS_DIFF_N=100000 VIS_DIFF_SEED=20261115 ./bin/vis-diff   # 0 lệch (không chạy song song gate)
target/release/vis-parse-fuzz --seconds 60 --seed 20261115 # 0 vi phạm
./bin/vis-targets                      # 18/18 check (CHỈ check)
```

### B2. Năm việc nên làm trước (theo thứ tự đòn bẩy)

1. **Landlock thật + runner Linux**: cần duyệt crate `landlock` **hoặc** `unsafe` trong danh sách đóng
   (D2-17); nối `vis-host-linux` vào `vis-cli`; chạy 54 ca trên Linux.
2. **Runner wasm**: wasm32-wasip1 + wasmtime (duyệt crate), chạy `vis test` + host suite trên wasm.
3. **TLS**: rustls/webpki (duyệt crate), redirect/keep-alive/chunked, test Internet thật.
4. **Siết mặc định caps**: mặc định `--strict-caps`; `cap[Proc]` allowlist + sandbox tiến trình con;
   cập nhật 6 ca `cap_proc` + host suite.
5. **Async song song thật**: scheduler đa luồng/work-stealing + wait-set kênh; test tăng tốc ≥2 core và
   rendezvous `chan(0)`; quyết định dứt điểm “từ chối vĩnh viễn” nếu không làm.

### B3. Nơi đọc

- [`QUEUE.md`](QUEUE.md) — 112 hàng, trạng thái task.
- [`DECISIONS.md`](DECISIONS.md) — **VD-01..VD-112**.
- `reports/R1..R16` — báo cáo từng vòng (bắt đầu `R15`/`R16`).
- [`HOST-BAO-DAM.md`](HOST-BAO-DAM.md) — hợp chuẩn host 4 nền.
- [`STATE.md`](STATE.md) — trạng thái hiện tại + việc dở.
