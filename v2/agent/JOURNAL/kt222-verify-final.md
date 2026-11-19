# KT-222 — reviewer độc lập cuối: xác nhận/bác các tuyên bố đêm 2026-11-20 trên HEAD `be3a0cd8`

**Vai**: L51 — **reviewer độc lập cuối** (không phải tác giả, không phải auditor).
**Nhánh**: `night3/L51-kt222`, tạo từ `be3a0cd8`. **Worktree**: `/private/tmp/night3-L51-kt222`.
**Nguyên tắc**: không tin tuyên bố — **tự chạy lại** trên HEAD cuối. Mỗi kết luận phải có log
trong `kt222-logs/`. Không sửa `spec/`, `QUEUE.md`, `DECISIONS.md`, `docs/**`, `AGENTS.md`,
journal của agent khác. Không push. Không chạy `gate --fast` (chỉ `v2 check`).

---

## 0. KẾT LUẬN NGẮN

**ĐÓNG ĐƯỢC — không tìm thấy đường lọt SỐNG, không tìm thấy báo oan.** Sáu trong bảy đường lọt
tự dựng lại **ĐỎ ở cả hai lớp**; đường thứ bảy (`src/target/evil.rs`) **ĐỎ lớp 1, XANH lớp 2** —
đúng **lỗ đã biết của lớp 2** (bộ lọc `"target" in relp.parts`), được lớp 1 che, **không** phải
đường lọt sống. Tám đối chứng dương **XANH cả hai lớp**. Một tuyên bố có **lệch vị trí bằng
chứng**: log `gate --fast` **có thật và nội dung khớp**, nhưng được commit ở `83275d94` (sau
`be3a0cd8`), **không** nằm trong `be3a0cd8`.

| Hạng mục | Kết quả |
|---|---|
| Gate log `kt221-gate-pass-be3a0cd8.log` | **CÓ THẬT**, 3439 dòng, **52** bước `--- …: OK`, dòng cuối `v2 gate --fast: PASS`; **không** bước nào bị bỏ qua |
| Vị trí log | **LỆCH**: log ở `83275d94`, **không** ở `be3a0cd8` (worktree `be3a0cd8` không có tệp) |
| `v2 check` trên `be3a0cd8` | **PASS** — RC=0, 38 bước OK, dòng cuối `v2 check: PASS` |
| Lớp 1 `--selftest` | **246/246 ok** |
| Lớp 2 `--selftest` | **20/20 ok** |
| Lớp 1 cây thật (thường / `--strict`) | **errors=0 / errors=0** |
| Lớp 2 cây thật | **errors=0** |
| Bảy đường lọt tự dựng lại | **6/7 ĐỎ cả hai lớp**; `src/target/evil.rs` ĐỎ L1 / XANH L2 (lỗ đã biết) |
| Tám đối chứng dương | **8/8 XANH cả hai lớp** |
| K6 ảnh + dấu vân tay | **KHỚP** — ảnh `639ca6d1…` có trong `v2/artifacts/` (checkout chính), `v2 status` in `43df126a4a6702ec` |
| K6 ba con số | **KHỚP** — `soak: ok=1000 fail=0`, `data+bss=224288`, `shell_prompt_lines=2` |
| Đường lọt SỐNG | **KHÔNG** |
| Báo oan | **KHÔNG** |

---

## 1. GATE (tuyên bố A)

### 1.1 `gate --fast` PASS trên `be3a0cd8` — **ĐÚNG nội dung, LỆCH vị trí bằng chứng**

Log `v2/agent/JOURNAL/kt221-logs/kt221-gate-pass-be3a0cd8.log`:

- **Có thật**: 3439 dòng; bản sao trong `kt222-logs/kt222-gate-pass-be3a0cd8-copy.log`.
- **52 dòng `--- …: OK`** — khớp **chính xác** 52 bước của `gate --fast` trong `v2/bin/v2`
  (38 bước `do_check` + 14 bước `do_gate_fast`), **cùng thứ tự**, **không** bước nào bị bỏ qua.
  Đối chiếu máy: trích `run_step "…"` + `echo "--- …: OK"` từ `v2/bin/v2` (bỏ `Miri $ten` — không
  thuộc `gate --fast`) ⇒ dãy 52 tên **trùng khớp** dãy 52 dòng OK của log.
- **Dòng cuối**: `v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)`.
- **Bất biến**: khối cuối log ghi `refs/heads/main : be3a0cd8… (trước: be3a0cd8…)`,
  `HEAD : be3a0cd8…`, `fingerprint index` không đổi, `số worktree 66` — tức cổng chạy **khi**
  main ở `be3a0cd8` và không làm nhiễm checkout chính.
- **LỆCH**: `git ls-tree be3a0cd8 -- v2/agent/JOURNAL/kt221-logs/` **không có** tệp này. Tệp được
  commit ở `83275d94` (`[KT-221] STATE: gate PASS tren be3a0cd8 …`), là commit **sau**
  `be3a0cd8` và **chỉ** chạm `STATE.md` + log. Trên worktree `be3a0cd8` (đúng HEAD cuối theo
  phiếu) **không có** log ⇒ người kiểm chỉ từ `be3a0cd8` **không** thấy bằng chứng.

⇒ Tuyên bố "gate PASS trên `be3a0cd8`" **đúng về kỹ thuật** (log tự khai bất biến main =
`be3a0cd8`), nhưng câu "log ở đường dẫn X **trên HEAD `be3a0cd8`**" **sai**: log nằm ở commit
kế tiếp. Đây là **lỗi quy trình bằng chứng**, không phải bịa.

### 1.2 `v2 check` PASS trên `be3a0cd8` — **ĐÚNG**

Tự chạy trong worktree `be3a0cd8`:

```text
$ ./v2/bin/v2 check
…
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
RC=0
```

Log: `kt222-logs/kt222-v2check.log` (38 bước `--- …: OK`, không bước FAIL). Lớp 2 dep-info chạy
trong `v2 check`: `errors=0 warnings=0 (12.59 s)`.

---

## 2. HAI LỚP KIỂM `wake` (tuyên bố B)

| # | Lệnh | Kết quả | Log |
|---|---|---|---|
| 3 | `check-wake-paths.py --selftest` | **246/246 ok** | `kt222-wake-paths-selftest.log` |
| 4 | `check-wake-depinfo.py --selftest` | **20/20 ok** | `kt222-wake-depinfo-selftest.log` |
| 5a | `check-wake-paths.py` (cây thật) | `errors=0 warnings=0` | `kt222-wake-paths-tree.log` |
| 5b | `check-wake-paths.py --strict` (cây thật) | `errors=0 warnings=0` | `kt222-wake-paths-tree-strict.log` |
| 5c | `check-wake-depinfo.py` (cây thật, 2 unit) | `errors=0 warnings=0` | `kt222-v2check.log` §wake theo dep-info |

**Cả ba tuyên bố 3/4/5 ĐÚNG.** Lớp 2 nạp 145 tệp `.rs` (gộp host + kernel), quét 143 tệp (miễn 2
tệp cửa `wake`), `tệp CHỈ tới được qua #[path]/include!/macro: 0`.

---

## 3. BẢY ĐƯỜNG LỌT TỰ DỰNG LẠI (tuyên bố C)

Harness: `kt222-logs/harness-7-duong-lot.py` (dựng → `cargo check` → lớp 1 thường → lớp 1
`--strict` → lớp 2 → **hoàn nguyên**). Bảng máy đọc: `kt222-logs/repro/bang-7-duong-lot.json`.
Cây sạch sau mỗi lượt (`git status` chỉ còn `kt222-logs/`).

| # | Đường lọt | cargo | Lớp 1 thường | Lớp 1 `--strict` | Lớp 2 dep-info |
|---|---|---|---|---|---|
| r01 | L40 `macro_rules!` `:item` + `include!` trong **đối số lời gọi** | **RC=0** | **ĐỎ** (3) | **ĐỎ** (3) | **ĐỎ** (1) |
| r02 | L44 `include!` vị trí biểu thức trong `( … )` | **RC=0** | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) |
| r03 | L47 `#[path]` trong inline module của tệp **non-mod-rs** | **RC=0** | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) |
| r04 | L47 `src/target/evil.rs` (thư mục tên `target`) | **RC=0** | **ĐỎ** (1) | **ĐỎ** (1) | **XANH** (0) |
| r05 | L49 `#[cfg(test)]` trên nhánh `match` (đường lọt **#13**) | **RC=0** | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) |
| r06 | L40 `mod` trong lời gọi macro (`cfg_if!`) | **RC=0** | **ĐỎ** (5) | **ĐỎ** (5) | **ĐỎ** (1) |
| r07 | L40/L45 `include!` trong `v2/lib` trỏ sang `v2/kernel` | **RC=0** | **ĐỎ** (1) | **ĐỎ** (1) | **ĐỎ** (1) |

**Đọc bảng cho đúng:**

- **6/7 ĐỎ ở cả hai lớp** — khớp tuyên bố "13 đường lọt đã đóng" cho các đường đã dựng lại.
- **r04 là ngoại lệ có chủ đích**: lớp 2 **XANH** vì `check-wake-depinfo.py` vẫn dùng bộ lọc
  `"target" in relp.parts` (dòng 354/837) — **tái hiện đúng lỗ đã ghi ở `kt219-test-range.md` §7
  ("lỗ của lớp 2, lớp 1 che")**. Lớp 1 **ĐỎ** ⇒ **cổng vẫn đỏ** ⇒ **không phải đường lọt sống**.
  Đây là **bất đối xứng đã biết**, không phải hồi quy mới.
- **r05 xác nhận bản vá KT-219 cắn**: cùng hình dạng repro #13 của `kt219-test-range.md` §1.1
  (pattern `A => (),` là **ident**, nhánh sản phẩm `B => { m.wake(t)?; }`) nay **ĐỎ cả ba** chế
  độ. Đường lọt #13 **đã đóng**.
- **r02/r07 bắt bằng fail-closed** (`E_STRICT_WAKE_INCLUDE`: đích `include!` ra ngoài
  `v2/kernel`+`v2/sim`), **không** bằng phân giải đích — vẫn là ĐỎ hợp lệ, nhưng ghi rõ cơ chế.
- **r01/r06 bắt bằng luật macro** (`E_STRICT_MACRO_TT`/`_INCLUDE`/`_MOD`) + lớp 2 bắt bằng
  **đúng tệp chứa `wake`** (`v2/lib/kt222_evil_core.rs`) — hai lớp bắt hai lý do khác nhau.

### 3.1 Đo thêm: nợ có tên `#[cfg(feature = "ktest")] mod` (r08)

`kt219-test-range.md` §7 ghi ba "nợ có tên"; nợ #1 là `#[cfg(feature = "ktest")] mod evil;`
(tệp riêng) — journal khai **XANH cả hai lớp** (ma trận A9). Tôi dựng lại đúng dạng:

| Kiểm | Kết quả |
|---|---|
| `cargo check -p vifast-core` | **RC=0** (module **không** được biên dịch — feature `ktest` không bật) |
| Lớp 1 thường / `--strict` | **ĐỎ** (1) — `E_STRICT_TEST_ONLY_WAKE` trên `kt222_r08_evil.rs:6` |
| Lớp 2 dep-info | **XANH** (0) — tệp không được rustc nạp |

⇒ Trên HEAD `be3a0cd8`, dạng này **không** lọt cả hai lớp như ma trận A9 của KT-219 khai: **lớp 1
bắt**. Nợ #1 **không sống** (và còn được lớp 1 che). Ghi nhận **lệch nhỏ** so với ma trận A9 —
có thể do hình dạng A9 khác (đặt trong crate có feature `ktest` thật) — **không** ảnh hưởng kết
luận "không có đường lọt sống".

---

## 4. TÁM ĐỐI CHỨNG DƯƠNG (tuyên bố D)

Harness: `kt222-logs/harness-8-doi-chung-duong.py`; bảng máy đọc:
`kt222-logs/pos/bang-8-doi-chung.json`. Tất cả dựng **cùng lúc** trong crate `vifast-core`:

| # | Dạng | cargo | Lớp 1 thường | Lớp 1 `--strict` | Lớp 2 |
|---|---|---|---|---|---|
| 1 | `include_str!` | RC=0 | XANH | XANH | XANH |
| 2 | `include_bytes!` | RC=0 | XANH | XANH | XANH |
| 3 | `stringify!(include)` | RC=0 | XANH | XANH | XANH |
| 4 | `#[path]` hợp lệ trong cây | RC=0 | XANH | XANH | XANH |
| 5 | `mod` bình thường | RC=0 | XANH | XANH | XANH |
| 6 | `#[cfg(test)] mod tests` (có `m.wake(t)` bên trong) | RC=0 | XANH | XANH | XANH |
| 7 | `wake` trong tệp allowlist (`fn wake` — `WAKE_PRODUCER`) | RC=0 | XANH | XANH | XANH |
| 8 | tệp trong `v2/kernel/target/` (build dir) | RC=0 | XANH | XANH | XANH |

**8/8 XANH cả hai lớp** — `errors=0` ở cả bốn lượt đo (`pos-static-normal.log`,
`pos-static-strict.log`, `pos-depinfo.log`). **Không có báo oan.**

**Ghi chú kỹ thuật (không phải báo oan trên cây thật)**: lần chạy đầu tôi để tệp allowlist chứa
**lời gọi** `wake(1)` (định nghĩa `fn wake` + gọi nó) ⇒ lớp 1 **ĐỎ**, lớp 2 **XANH**. Đây là
**bất đối xứng allowlist**: lớp 2 miễn **cả tệp** producer, lớp 1 chỉ miễn **định nghĩa** `fn
wake`. Cây thật **không** có dạng này (`core_sched.rs`/`kobj.rs` chỉ có định nghĩa + doc) nên
**không** phải báo oan thực tế; ghi để người sau biết ranh giới. Đối chứng #7 ở trên dùng dạng
**hợp lệ** (chỉ định nghĩa) — XANH.

---

## 5. K6 (tuyên bố E)

### 5.1 Ảnh `639ca6d1…` + dấu vân tay — **KHỚP**

- `v2/artifacts/` **không** có trong worktree vì `.gitignore` dòng 8 ghi `artifacts/` (thư mục
  build, không commit). Trong **checkout chính** `/Users/mps/dvchd/ViFastOs/v2/artifacts/`:
  `639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81.bin`, **213632 B**,
  `shasum -a 256` = đúng tên tệp.
- `./v2/bin/v2 status` (worktree `be3a0cd8`): `build-inputs 233 mục  dấu vân tay:
  43df126a4a6702ec  dirty: 0` — **đúng** dấu vân tay tuyên bố. Log: `kt222-status.log`.
- Dấu vân tay `43df126a4a6702ec` cũng khớp log K6: `kt210-measure.log:9` `inputs=43df126a4a6702ec
  dirty=0`.

### 5.2 Sáu tiêu chí K6 — **KHỚP log**

`kt210-k6-remeasure.md` ánh xạ F1..F6 → §F1..§F6; `kt210-logs/kt210-measure.log` (1521 dòng, 81
dòng `ok`) có **đủ** sáu tiêu đề `===== F1 … F6 =====`. Ba con số kiểm độc lập:

| Con số | Dòng log | Giá trị |
|---|---|---|
| `soak: ok=1000 fail=0` | 1513 | `# soak: xong wall=297s (3 boot/s); ok=1000 fail=0 tổng=1000/1000` |
| `data+bss=224288` | 56, 498 | `# image=product … file=213632 data+bss=224288 budget=262144` |
| `shell_prompt_lines=2` | 13, 20, 27 | `### smp=1/2/4 shell_prompt_lines=2` |

Ba con số **khớp tuyệt đối** với journal. Các số F2/F3/F6 cũng có mặt (`f2_write_exit=0`,
`f2_reboot_read_exit=0`, `f2_dia_khong_doi_giua_hai_boot=1`, `f3_noshell_shell_prompt_lines=0`,
`f3_parity_noshell_exit=5`, `matrix_exit=0`, `harness_scan_exit=0`, `hits=0`/`hits=25`).

---

## 6. TUYÊN BỐ ĐÚNG / SAI

| Tuyên bố | Kết luận |
|---|---|
| A1 `gate --fast` PASS trên `be3a0cd8`, log 52 dòng OK, dòng cuối PASS, không bỏ bước | **ĐÚNG nội dung** — **LỆCH vị trí**: log ở `83275d94`, không ở `be3a0cd8` |
| A2 `v2 check` PASS trên `be3a0cd8` | **ĐÚNG** (RC=0, 38 bước OK) |
| B3 lớp 1 selftest 246/246 | **ĐÚNG** |
| B4 lớp 2 selftest 20/20 | **ĐÚNG** |
| B5 hai lớp `errors=0` trên cây thật (thường + `--strict`) | **ĐÚNG** (thêm lớp 2 `errors=0`) |
| C "13 đường lọt đã đóng" | **ĐÚNG cho 7 đường đã dựng lại** — 6/7 ĐỎ cả hai lớp; `src/target/evil.rs` ĐỎ L1 / XANH L2 (lỗ đã biết của lớp 2, lớp 1 che) |
| D 8 đối chứng dương XANH | **ĐÚNG** — 8/8 XANH cả hai lớp |
| E6 ảnh `639ca6d1…` trong `v2/artifacts/`, `v2 status` in `43df126a4a6702ec` | **ĐÚNG** (ảnh ở checkout chính — `artifacts/` bị gitignore; dấu vân tay khớp) |
| E7 sáu tiêu chí K6 có log, ba con số khớp | **ĐÚNG** |

**Tuyên bố SAI**: không có tuyên bố kỹ thuật nào sai. **Một lệch quy trình**: log gate không nằm
trong HEAD cuối `be3a0cd8` (nằm ở `83275d94`).

---

## 7. ĐÁNH GIÁ: LUẬT `wake` ĐÓNG ĐƯỢC CHƯA?

**Trong phạm vi đã kiểm: ĐÓNG ĐƯỢC.** Không có dạng nào tôi dựng lại cho **cả hai lớp XANH**
trong khi mã **thật sự được biên dịch**. Đường lọt #13 (nguy hiểm nhất — lọt cả hai lớp) **đã
đóng** và tôi tái lập được đúng hình dạng repro của KT-219.

**Phạm vi đã kiểm:**

- Nội dung + tính đầy đủ của log `gate --fast` (52/52 bước, đối chiếu máy với `v2/bin/v2`).
- `v2 check` đầy đủ trên `be3a0cd8` (38 bước).
- Hai lớp: selftest (246/246, 20/20) + cây thật (thường, `--strict`, dep-info).
- **7/13** đường lọt: L40 (`:item`+`include!`, `mod` trong macro), L44 (`include!` trong `( … )`),
  L47 (`#[path]` non-mod-rs, `src/target/`), L49 (#13), L40/L45 (`include!` v2/lib→v2/kernel).
- **8/8** đối chứng dương.
- K6: ảnh, dấu vân tay, sáu tiêu chí có log, **3** con số.
- Nợ có tên #1 (`cfg(feature = "ktest") mod`) — đo thêm.

**Chưa kiểm (nói rõ để không suy rộng):**

- **6 đường lọt còn lại**: 4 biến thể L44 (`[ … ]`, tuple, closure, `Some(…)`), đường #8
  (`#[path]` ra ngoài `src/` qua `..`), đường #10 (`macro_rules!` `:expr` + `include!`).
- **Không chạy lại `gate --fast`** (bị cấm trong phiếu) — chỉ kiểm **log** của lần chạy trước.
- **Không chạy nightly / Miri / soak**; **không đo lại K6** (cần QEMU + ảnh) — chỉ đối chiếu log.
- **Không kiểm** 190 khẳng định của `kt220-audit-night.md`, không kiểm các journal khác.
- **Không kiểm** tính đầy đủ của danh sách "13 đường lọt" (chỉ kiểm 7 đường có trong phiếu).
- **Không kiểm** nhánh `#[cfg]` không bật ngoài dạng `ktest` (nợ #2/#3 của KT-219).
- **Không kiểm** lớp 2 trên cây có `target/` thật trong `src/` (chỉ dựng `src/target/`).

**Rủi ro còn lại (ghi để A0 biết):**

1. **Lớp 2 mù `src/target/`** (bộ lọc `"target" in relp.parts`) — **đã biết**, lớp 1 che. Nếu
   tương lai ai sửa lớp 1 mà không sửa lớp 2, đường #12 **sống lại**. Đề nghị: vá bộ lọc lớp 2
   theo `_is_build_dir` của lớp 1 (chỉ bỏ `target` **trước** `src`).
2. **Bất đối xứng allowlist producer**: lớp 2 miễn cả tệp `fn wake`, lớp 1 chỉ miễn định nghĩa.
   Không ảnh hưởng cây thật; nếu tương lai có tệp producer chứa lời gọi `wake(` thì lớp 1 đỏ
   (fail-closed — chấp nhận được).
3. **Log gate không nằm trong HEAD cuối** — nếu A0 chỉ nhận `be3a0cd8` thì thiếu bằng chứng cổng.
   Đề nghị: commit log vào đúng HEAD cuối (hoặc ghi rõ "log ở commit kế tiếp").

---

## 8. TỆP + LOG

| Tệp | Nội dung |
|---|---|
| `kt222-logs/kt222-v2check.log` | `v2 check` đầy đủ trên `be3a0cd8` (RC=0, 38 bước OK) |
| `kt222-logs/kt222-wake-paths-selftest.log` | lớp 1 `--selftest` 246/246 |
| `kt222-logs/kt222-wake-depinfo-selftest.log` | lớp 2 `--selftest` 20/20 |
| `kt222-logs/kt222-wake-paths-tree.log` / `-strict.log` | lớp 1 cây thật, `errors=0` |
| `kt222-logs/kt222-gate-pass-be3a0cd8-copy.log` | bản sao log gate (3439 dòng, 52 OK) |
| `kt222-logs/kt222-status.log` | `v2 status` — dấu vân tay `43df126a4a6702ec` |
| `kt222-logs/harness-7-duong-lot.py` + `repro/` | 7 đường lọt + bảng JSON + log từng bước |
| `kt222-logs/harness-8-doi-chung-duong.py` + `pos/` | 8 đối chứng dương + bảng JSON + log |
| `kt222-logs/repro/r08-*.log` | nợ có tên `cfg(feature = "ktest") mod` |

**Không** sửa: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`,
`v2/AGENTS.md`, journal của agent khác. **Không** push. **Không** chạy `gate --fast`.
