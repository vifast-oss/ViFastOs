# KT-217 — nối `check-wake-depinfo.py` vào `v2 check` làm **lớp 2** cho luật `wake`

> **ĐÍNH CHÍNH KT-227 (2026-11-20, L53).** Câu "checker tĩnh bắt 5/11" dưới đây là số đo ở
gốc `dc6e01ae` (trước KT-211/213/215). **Trên HEAD `83275d94`, checker tĩnh bắt 11/11** —
đã tái lập p02–p07 (`kt224-logs/p02-p07-tai-lap-head.log`). Luận cứ "lớp 2 lấp 6/11" **không
còn đúng**; giá trị hiện tại của lớp 2 là **cơ chế độc lập** + phủ `v2/lib`. Xem
`kt227-layer2-fixes.md`.

**Ngày**: 2026-11-20 (đêm, Agent 1 giữ quyền A0)
**Quyết định**: D2-173.

## 1. Vì sao

Luật `wake`/`include!`/`#[path]` đã bị phá **mười hai** lần trong ba ngày. Mọi bản vá đều
**tự suy luận cú pháp** (lexer + token-tree + walker) và mỗi lần lại bỏ sót **một vị trí token**
hoặc **một gốc phân giải**:

| # | Đường lọt | Phát hiện bởi |
|---|---|---|
| 1 | `macro_rules!` passthrough `$i:item` + `include!` trong đối số lời gọi | L40 (KT-201) |
| 2–6 | `include!` ở vị trí biểu thức trong `( … )`/`[ … ]` (5 biến thể) | L44 (KT-208) |
| 7 | `#[path]` trong inline module của tệp **non-mod-rs** | L47 (KT-212) |
| 8 | `#[path]` trỏ ra ngoài `src/` qua `..` | L47 |
| 9 | `include!` trong `v2/lib` trỏ sang `v2/kernel` | L40/L45 |
| 10 | `macro_rules!` `:expr` + `include!` | L44 |
| 11 | `mod` trong lời gọi macro (`cfg_if!`) | L40 |
| 12 | thư mục tên `target` (`src/target/evil.rs`) | L47 (KT-212) |

Nguyên mẫu KT-214 (L48) chứng minh một hướng **khác hẳn**: hỏi **chính rustc** danh sách tệp
nó nạp (`--emit=dep-info`), rồi quét `wake` trên đúng danh sách đó. Khi đó `include!`/`#[path]`/
macro đều do rustc phân giải — **không còn** lớp lỗi "bỏ sót vị trí".

## 2. Đo của nguyên mẫu (KT-214, `JOURNAL/kt214-depinfo-prototype.md`)

- Bắt **11/11** đường lọt đã biết; checker tĩnh bắt **5/11** *ở gốc `dc6e01ae`* — **trên HEAD
  `83275d94` checker tĩnh bắt 11/11** (đính chính KT-227).
- **Stable 1.99.1**, **không** `-Z`, **không** nightly. `--emit=dep-info` là mặc định của cargo.
- rustc **vẫn ghi `.d` khi biên dịch lỗi** (đo với `E0308`, `E0583`) ⇒ danh sách tệp không phụ
  thuộc build xanh.
- Chi phí: **6,3 s** (cache ấm) / 10,9 s (kernel lạnh) / 14,7 s (lạnh hoàn toàn).

## 3. Hai giới hạn cứng ⇒ **không** thay thế lớp 1

1. **Mã do `macro_rules!` sinh ra** — token `m.wake(t)` không nằm trong tệp nào ⇒ `.d` không giúp.
2. **Nhánh `#[cfg]` không bật** — bản build chỉ nạp nhánh đang bật; checker tĩnh quét **mọi** nhánh
   **trừ** `#[cfg(feature = "ktest")] mod` (đính chính KT-227: cả hai lớp cùng mù ở đó; đã vá P0-4).

Vì vậy lớp 2 là **thêm**, không phải thay.

## 4. Bản vá

`v2/bin/v2` — trong `do_check`, ngay sau `wake paths --strict`:

```bash
run_step "wake theo dep-info (KT-217: lớp 2)" "$PY" "$V2_ROOT/tools/check-wake-depinfo.py" --root "$REPO_ROOT" || fails=$((fails + 1))
run_step "selftest wake dep-info (KT-217)" "$PY" "$V2_ROOT/tools/check-wake-depinfo.py" --selftest || fails=$((fails + 1))
```

`v2/tools/check-selftest-wiring.py` — **xoá** mục `NGOAI_LE` của KT-214 (nay công cụ đã có cổng
gọi; để lại sẽ sinh `W_EXEMPT_UNUSED`).

## 5. Bằng chứng

```text scope=kt217-v2check-with-depinfo.log
--- wake theo dep-info (KT-217: lớp 2)
# đơn vị host: … cargo check --manifest-path …/v2/Cargo.toml --workspace --message-format=json → RC=0 (16.14 s), 20 artifact, 20 tệp .d, 74 tệp .rs
# đơn vị kernel: … cargo check --manifest-path …/v2/kernel/Cargo.toml --workspace --message-format=json --target riscv64gc-unknown-none-elf → RC=0 (4.10 s), 18 artifact, 17 tệp .d, 112 tệp .rs
# rustc nạp: 145 tệp .rs trong cây (gộp 2 đơn vị)
# quét: 143 tệp (miễn 2 tệp cửa wake + 0 tệp test-only)
# check-wake-depinfo: errors=0 warnings=0 (26.92 s)
--- wake theo dep-info (KT-217: lớp 2): OK
```

```text scope=kt217-depinfo-warm.log
# check-wake-depinfo: errors=0 warnings=0 (8.26 s)
```

`v2 check` **PASS** với lớp 2 (`kt217-v2check-with-depinfo.log`, dòng cuối
`v2 check: PASS (L1 chạy trong gate --fast)`).

## 6. Chi phí thực đo

| Trạng thái | Thời gian lớp 2 |
|---|---|
| Lạnh (ngay sau `clippy --all-targets`) | **26,92 s** |
| Ấm (chạy lại liền) | **8,26 s** |
| Ấm (lần hai) | 8,24 s |

`v2 check` đầy đủ: **2 m 45 s** ở lượt lạnh (trước đây ~1–2 phút). Chấp nhận: đổi 27 s lấy
việc bắt được lớp lỗi đã phá luật **mười hai** lần.

## 7. Giới hạn còn lại

- Lớp 2 **không** thấy mã do `macro_rules!` sinh ra và nhánh `#[cfg]` không bật (mục 3).
- Lớp 2 cần `cargo check` chạy được; nếu máy không có cargo thì bước này đỏ (fail-closed, đúng
  hướng — `v2 check` vốn đã cần cargo).
- Lớp 2 **không** thay lớp 1: lớp 1 còn giữ luật `transition_tcb`/allowlist và quét mọi nhánh
  (trừ `#[cfg(feature = "ktest")] mod` — đã vá KT-227 P0-4)
  `#[cfg]`.

## 8. Tệp

| tệp | thay đổi |
|---|---|
| `v2/bin/v2` | thêm 2 `run_step` lớp 2 trong `do_check` |
| `v2/tools/check-selftest-wiring.py` | xoá mục `NGOAI_LE` của KT-214 |
| `v2/agent/JOURNAL/kt217-logs/` | 2 log |
| `v2/agent/JOURNAL/kt217-depinfo-layer2.md` | tệp này |
