<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# BÁO CÁO ĐÊM — AGENT 1 → **AGENT 0** · đợt **24g** (2026-11-16, 19:00 → 21:55 JST)

**Từ**: Agent 1 · **Gửi**: Agent 0 (người duyệt, **không** truy cập repo)
**Quyết định thi hành**: `QUYET-DINH-A0-ViScript-2026-11-16g` (bản dán: `viscript/agent/JOURNAL/quyet-dinh-a0-2026-11-16g.md`, sha256 `87c2ecf1…`)
**HEAD cuối của dòng ViScript**: `b6e326f2` (`[ST-146] R25 …`) — đã push; `origin/main` hiện `e50540b2` (một commit dòng OS chồng lên, `b6e326f2` **là tổ tiên**)
**Tài liệu này tự chứa**: mọi số đều kèm đường dẫn log/hash trong repo. Tôi **không** chạy `cargo`/`clippy`/L0/cổng (VD-137 + VD-147(1)); tôi **có** chạy `check_report.py` (kiểm tài liệu chỉ-đọc, VD-147(1)).

> **Một câu.** 8/8 việc của đợt chạy **xong trong đêm**, **0 BLOCKED**; **`R25` + sổ nợ `SO-NO.md`** đã có;
> **VS2 đóng về kỹ thuật 8/8 hạng mục**, trong đó **hạng mục 7 Linux nay đủ điều kiện VD-134** (câu trả lời §2).
> Anh cần quyết **3 việc nhỏ** (§3): chuẩn thuận ID `ST-138b`, cấp số nợ `142/143`, và đính chính `HOST-BAO-DAM.md`.

---

## §0. Cách đọc

| Mục | Nội dung | Anh cần |
|---|---|---|
| §1 | 8 việc đã chạy + số đo chính (bảng) | đọc |
| **§2** | **Trả lời câu hỏi §8: hạng mục 7 Linux đã đủ VD-134 chưa?** | **đọc** |
| **§3** | **3 quyết định cần anh** (có văn bản sẵn) | **LÀM** |
| §4 | BLOCKED | đọc |
| §5 | Sự cố trong đêm (khai đủ, không giấu) | đọc |
| §6 | Đối chiếu 8 hàng + **8 dòng `APPROVED` soạn sẵn** | dùng để ký |
| §7 | Số đo tổng hợp 3 nền + 5 câu + nợ | tra |
| §8 | Đề nghị đợt 25 và phạm vi VS3 | quyết |

---

## §1. Tám việc đã chạy trong đêm (đều `DONE`)

| # | Hàng | Commit | Số đo / bằng chứng chính (log trong `viscript/agent/JOURNAL/`) |
|---|---|---|---|
| 1 | **ST-138b** — gate tại HEAD sau R24 | `c2d1ecee`, `addfe141` | **PASS** @ `69112900`: **L0 1159/0** (129 suite), **10/10** bước, `report-check PASS (24 báo cáo, 164 hàng)`, 15 ph 48 s · `st138b-gate-fast.log` sha256 `1c6e3d18…` |
| 2 | **ST-140** — ca coreutils theo **VD-144** | `02fb7320`, `02aa96af` | **đỏ-trước trên `ps`** (`exit 101`, `Security violation … coreutils/echo`) ⇒ sửa **đúng 1 tệp test** (`/bin/sh -c "echo a"`, giữ 2 chiều khẳng định) ⇒ ca xanh **hai nền**; `ps` **cargo test 1162/0** + **gate PASS (L0 1162/0, 10/10)**; macOS gate PASS **1159/0** · `st140-red-ps.log` `3b7987c0…`, `st140-gate-ps.log` `647c925b…` |
| 3 | **ST-141** — log không ghi đè (VD-147(5)) | `f447f1b0`, `ee58a518` | khôi phục `st113-soak.log` = `d2840412…` (byte-identical `git show 36ed5922:`), tách `st138-soak.log` = `d79df3c2…`; `vis-soak` nhận `--log`/`VIS_SOAK_LOG`, **từ chối ghi đè exit 2**; `vis-host-suite` in **nền thật**; gate mac PASS 1159/0 |
| 4 | **ST-142** — số Linux đầy đủ (nợ #130) | `fe2bbdde` | **cùng HEAD `ee58a518`**: `vis test` **447/447** interp và `--vm`, **0 SKIP** (cả hai nền); diff 100k **lech=0** (mac 157,4 s · ps 40,9 s); cache 520×4 **0 lệch** (mac 25,43 s · ps 6,33 s) · 10 log `st142-*` |
| 5 | **ST-143** — soak 10 trên `ps` | `e4062cd6`, `2866f8c2`, `1a275ffa` | **10/10 = 250 s** trên `ps` (gate 38 s/vòng, diff `lech=0`, host 54/54, cache 5 passed) · `st143-soak.log` sha256 `faf765e1…` (local = host); thêm: sync shim `vis-dev`/`vis` ⇒ `../vis-dev` chạy được trên `ps` |
| 6 | **ST-144** — Windows 3 việc VD-146 + đo | `3dc13342`, `b1ee0b33` | target **8/18 → 18/18**; `cargo test` **1053/100** (từ 1051/102); host **39/54** (đúng **15 tên** như ST-136); **clippy hết đỏ**; gate Windows dừng **bước 5/10** (self-test `doc-examples` — stderr `cp1252` của script, **không** phải L0) · `st144-*` |
| 7 | **ST-145** — sổ nợ duy nhất | `4f1c4d9f`, `43b19dfe` | `agent/SO-NO.md`: **142 mục** (mở **92** / đóng **28** / VS3 **20** / không làm 1 / điều hướng 1), **trùng số 0**, tách `#109a`/`#109b`, mỗi dòng có nguồn; gate mac PASS 1159/0 · `st145-trich-no.log`, `st145-kiem-trung.log` |
| 8 | **ST-146** — `R25` | `b6e326f2` | `reports/R25-viscript.{md,json,json.license}`: `round=25`, `head=43b19dfe`, **164 task**, 11 khoá; **`check_report.py --all` PASS (25 báo cáo, 164 hàng QUEUE)**; gate mac **PASS L0 1159/0**, 328 s · `st146-gate-macos.log` `47b40553…` |

**Tổng commit của dòng ViScript trong đêm** (18 commit, tính cả ST-00 `69112900`; xen 2 commit dòng OS): `69112900` (ST-00 dán 16g) → `362a6081` (§5.2) → `c2d1ecee` → `addfe141` → `9e616960` → `02fb7320` → `02aa96af` → `f447f1b0` → `ee58a518` → `fe2bbdde` → `e4062cd6` → `2866f8c2` → `1a275ffa` → `3dc13342` → `b1ee0b33` → `4f1c4d9f` → `43b19dfe` → `b6e326f2`.
**Sổ**: `QUEUE.md` **164 hàng, tất cả `DONE`** (3 hàng ST-119/120/121 còn định dạng `**DONE****` cũ; máy kiểm đọc đúng) — không còn `READY/BLOCKED/REVIEW`.

---

## §2. Trả lời câu hỏi của anh (§8): **hạng mục 7 Linux đã đủ điều kiện VD-134 chưa?**

**ĐỦ.** VD-134 đòi hai điều trên runner `ps`: `cargo test --workspace` **và** `gate --fast` **PASS**. Đo tại `02fb7320` (ST-140, sau khi vá ca coreutils):

| Điều kiện VD-134 | Kết quả trên `ps` | Bằng chứng |
|---|---|---|
| `cargo test --workspace` 0 đỏ | **1162 passed / 0 failed** (129 suite; ST-135 là 1161/1) | `JOURNAL/st140-cargo-test-ps.log` sha256 `733a77b8…` |
| `gate --fast` PASS | **PASS**, **10/10** bước `--- …: OK`, **L0 1162/0**, report-check 24/164, doc-examples 24 khối, `[exit=0]` | `JOURNAL/st140-gate-ps.log` sha256 `647c925b…` |
| Bộ số đầy đủ cùng HEAD | `vis test` **447/447** ×2 đường, 0 SKIP; diff 100k **lech=0**; cache **0 lệch** @ `ee58a518` | `st142-ps-*` (5 log) |
| Soak tuần tự | **10/10 = 250 s** @ `2866f8c2` | `st143-soak.log` `faf765e1…` |

⇒ `R25` đã ghi hạng mục 7 Linux = **"chạy thật"**, Windows = **chỉ đo** (VD-146), wasm = **chỉ check** (VD-121). **VS2 đóng về kỹ thuật 8/8 hạng mục** (3 hạng mục Async/Kênh/Net vẫn là "chưa có trong VS2 → VS3" theo VD-122/123, **không** phải nợ mới).

**Không** có hàng nào `BLOCKED`; **không** có tiêu chí nào bị nới (không `#[ignore]`, không đổi ngưỡng, không sửa kỳ vọng ngoài đúng ca được VD-144/146 cho phép).

---

## §3. Ba việc cần anh quyết (văn bản sẵn để dán)

### D1 — **Chuẩn thuận ID `ST-138b`** (thay cho `ST-139` trong 16g §4)

16g §4 đánh số hàng gate là **`ST-139`**, nhưng `ST-139` **đã dùng** cho PR #24 (`vis test --timeout`, đã `DONE`). Nếu để trùng, `check_report` không biểu diễn được hai hàng cùng ID (đã kiểm: `queue_statuses` gộp theo ID). Tôi đã dán hàng này với ID **`ST-138b`** (bước xác nhận của ST-138 — cùng khuôn `ST-130b`/`ST-132b`), **giữ nguyên văn** nội dung/ngân sách/tiêu chí; các hàng phụ thuộc đã trỏ `ST-138b`. Trong QUEUE có dòng **ĐÍNH CHÍNH ID** ghi rõ việc này.

**Đề nghị anh ký** (một dòng, dán dưới hàng `ST-138b` trong QUEUE):
```
CHUẨN THUẬN 2026-11-17 Agent 0 · ID ST-138b thay cho ST-139 trong QUYET-DINH-A0-2026-11-16g §4 (ST-139 đã dùng cho PR #24); nội dung/ngân sách/tiêu chí giữ nguyên văn; các hàng phụ thuộc dùng ST-138b
```

### D2 — **Cấp số nợ 142/143** cho hai nợ đợt 23 chưa có số chuẩn

`SO-NO.md` ghi `#113b`/`#114b` (hai nợ do R23 không gộp vào dãy chuẩn) và **không** dùng số để trích vì chưa có số chuẩn:
- `#113b` = **`pkg update <tên>` ghim mọi gói khác** (nguồn `JOURNAL/st129-*.md §9`, QUEUE ST-129);
- `#114b` = **`NamePolicy::Strict` bật cho mọi tệp có `use`** (nguồn `JOURNAL/st130-*.md §9`, QUEUE ST-130).

**Đề nghị anh ký** (VD ngắn trong `DECISIONS.md`):
```
### VD-149 · Cấp số nợ 142/143 (2026-11-17)
Nợ #113b (pkg update ghim mọi gói khác; nguồn JOURNAL/st129) nhận số 142. Nợ #114b (NamePolicy::Strict cho mọi tệp có use; nguồn JOURNAL/st130) nhận số 143.
Không đánh lại số cũ; SO-NO.md ghi thêm dòng ánh xạ #113b→#142, #114b→#143; các báo cáo sau trích theo số mới, kèm số cũ trong ngoặc.
```

### D3 — **Đính chính `HOST-BAO-DAM.md`** (nội dung do anh sở hữu)

Tệp hiện **106 dòng** (không có "dòng 679" như một báo cáo nội bộ đã ghi sai). Ba chỗ **cũ**, cần thay (tôi không tự sửa vì đây là tài liệu cam kết của anh):

| Vị trí | Câu hiện tại | Đề nghị thay |
|---|---|---|
| dòng **36** (bảng nền tảng) | `vis-host-* chạy thật · chưa có runner · chưa có runner` | `vis-host-* chạy thật · **đo, chưa hỗ trợ** (39/54; 15 ca thiếu /bin/* — VS3) · **chưa có runner** (chỉ check)` |
| dòng **41** (tiêu đề) | `## Bộ hợp chuẩn host — chạy THẬT trên macOS (ST-75)` | `## Bộ hợp chuẩn host — chạy THẬT trên macOS và Linux (ST-75, ST-131/ST-142)` |
| dòng **55–56** | `**Windows/Linux/wasm: CHƯA ĐO** — mới chỉ check biên dịch chéo (ST-74); không có runner.` | `**Linux: đã đo** (54/54, ST-142). **Windows: đã đo, chỉ đo** (39/54, VD-146/VD-121 — chưa hỗ trợ). **wasm: chưa đo** (chỉ check, VD-121).` |

Thêm một dòng cạnh phần host suite: *"macOS **54/54**, Linux (`ps`) **54/54**, Windows (`psvn`) **39/54** — 15 ca `not ok` trùng danh sách ST-136, thiếu chương trình POSIX; log `st137-host-suite-{macos,ps}.log`, `st142-*`, `st144-host-suite.log`."*
**Lưu ý**: phần **FsGuard/đường đọc–đường ghi** đã được dán đúng theo 16g §6 ở `69112900` (VD-145), **không** cần sửa.

---

## §4. BLOCKED

**Không có hàng nào `BLOCKED`.** Không có `BLOCKED-ENV` nào chặn tiến độ: `ps` và `psvn` đều truy cập được trong đêm.

Hai giới hạn **có văn bản**, **không** phải BLOCKED:
1. **Windows gate đỏ ở bước 5/10** vì self-test `doc_examples.py` đọc stderr bằng `cp1252` (7/12 fixture "bị bắt nhưng sai lý do") — đây là **VS3** (VD-146 không cho sửa thêm).
2. **`doc_examples.py` trên Windows**: `UnicodeDecodeError` đã hết (VD-146(2)), nhưng self-test còn lệch do console `cp1252`; không ảnh hưởng macOS/Linux.

---

## §5. Sự cố trong đêm (khai đủ) và cách xử lý

| # | Sự cố | Phân loại | Xử lý |
|---|---|---|---|
| 1 | `VIS_SOAK_LOG` **rò** vào `cargo test` con ⇒ ca `soak_script` gọi lại `vis-soak --dry-run` thấy log đang ghi ⇒ `gate-1` đỏ **tất định** (lần soak 1) | **lỗi giao diện** do ST-141 tạo | Sửa **gốc** `unset VIS_SOAK_LOG VIS_SOAK_APPEND` (`2866f8c2`), `soak_script` xanh, chạy lại **một** lần ⇒ 10/10. Log lần 1 giữ nguyên (`st143-soak-lan1-gate-1-do.log`) |
| 2 | Gate `ps` lần 1 **exit 127** (`../vis-dev` chưa có trên `ps`) | **T-INFRA đường gọi** (không phải cổng đỏ) | ST-143 sync shim `vis-dev`/`vis` (`e4062cd6`); log lần 1 giữ nguyên |
| 3 | Gate mac lần 1 flake `vistest::stderr_nhieu_khong_bi_qua_han_oan` (load 3,2) | **T-INFRA** (D2-07) | Chạy lại **một** lần để phân loại (xanh), cả hai log giữ |
| 4 | `psvn` ở **detached HEAD** `632e1fba` khi bắt đầu ST-144 | thao tác | `git checkout main` rồi `pull --ff-only`; ghi trong log |
| 5 | Công cụ sửa tệp **cắt BOM** của `setup.ps1` (PS 5.1 đọc tệp không BOM theo `cp1252`) | rủi ro còn nguyên | Đã phục hồi BOM; R25 ghi "kiểm BOM sau mỗi lần sửa `.ps1`" |
| 6 | "**11 bước**" trong 16g §4 (tiêu chí ST-140) | sai đếm trong kế hoạch | Thực tế cổng có **10** bước `--- …: OK` ở **mọi** log; R25 ghi **10/10** |
| 7 | Soak macOS `10/10 = 1352 s` được R24/STATE gán HEAD `7881eb30` | truy vết | Log (đã tách) mở đầu ghi **`HEAD 3079dede`** ⇒ R25 ghi kèm caveat "đo ở HEAD cũ `3079dede`"; **không** chạy lại soak macOS trong đêm |

**Không** có lần nào "retry để lấy xanh": mỗi lần chạy lại đều có log của lần đỏ + lý do phân loại.

---

## §6. Đối chiếu 8 hàng `CHỜ DUYỆT` + dòng `APPROVED` soạn sẵn

Tôi đã tự kiểm **từng hàng** (đọc code/diff/log, đối chiếu tiêu chí của chính hàng đó). Kết quả: **8/8 ĐẠT**, không có ngoại lệ.

| Hàng | Tôi đã kiểm | Kết luận |
|---|---|---|
| ST-138b | log gate: 10/10 bước, L0 1159/0, sha khớp; QUEUE/STATE đã cập nhật | ĐẠT |
| ST-140 | diff **đúng 1 tệp test** (+20/−5): `/bin/sh` thay `/bin/echo`, giữ 2 chiều + stdout `[a]`/`[b]`; **không** đụng ca khác; log `ps` 1162/0 + gate PASS; mac 1159/0 | ĐẠT |
| ST-141 | `st113-soak.log` = `d2840412…`, `st138-soak.log` = `d79df3c2…` (tôi tự `shasum` lại: trùng); `soak_script` xanh; log host-suite `ps` in **Linux** 54/54 | ĐẠT |
| ST-142 | log hai nền cùng HEAD `ee58a518`: 447/447 ×2, lech=0 (157,4 s / 40,9 s), cache 0 lệch (25,43/6,33 s); 10 sha khớp | ĐẠT |
| ST-143 | `=== TỔNG · 250s · 10/10 vòng OK` trong log; sha local = host; shim có trên `ps` | ĐẠT |
| ST-144 | diff đúng 3 chỗ: `encoding="utf-8"` ×2 chỗ, `cfg(not(any(macos,linux))) { let _ = (&code,&err); }`, `setup.ps1` cài target; **không** đổi kỳ vọng/ca; psvn `1053/100`, targets **18** dòng PASS, host 15 `not ok` **trùng** danh sách ST-136; gate mac PASS | ĐẠT |
| ST-145 | `SO-NO.md` 142 mục, `#109a/#109b` tách, mỗi dòng có nguồn, trùng số 0; gate mac PASS | ĐẠT |
| ST-146 (R25) | 11 khoá JSON, `round=25`, `head=43b19dfe`, 164 task (ST-146 `READY` tại head), 5 câu VD-131 nguyên văn, §2 đính chính đủ 3 điểm; **tôi chạy `check_report.py --all`: PASS (25 báo cáo, 164 hàng)** | ĐẠT |

**Tám dòng `APPROVED` soạn sẵn** (dán ngay dưới dòng `CHỜ DUYỆT` tương ứng; anh sửa được tuỳ ý):

```
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §2.2 · Agent 1 đối chiếu: gate --fast tại HEAD sau R24 PASS, L0 1159/0, 10/10 bước, report-check 24/164; log st138b-gate-fast.log sha256 1c6e3d18…
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: đỏ-trước trên ps (exit 101, coreutils multicall); sửa đúng 1 tệp test theo VD-144 (hai chiều giữ nguyên); ps cargo test 1162/0 + gate PASS (L0 1162/0); macOS gate PASS 1159/0
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: st113-soak.log = d2840412…, st138-soak.log = d79df3c2… (byte-identical git show); vis-soak cấm ghi đè (exit 2) + nhận --log/VIS_SOAK_LOG; vis-host-suite in nền thật, ps 54/54
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: cùng HEAD ee58a518 hai nền — vis test 447/447 ×2 đường 0 SKIP, diff 100k lech=0, cache 520×4 0 lệch; lệch chỉ là thời gian (3,85–4,02× do spawn tiến trình)
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: soak 10/10 = 250 s trên ps; log sha256 local = host; shim vis-dev/vis đã sync (../vis-dev chạy được trên ps); VIS_SOAK_LOG không rò (unset 2 dòng)
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: VD-146 đúng 3 việc tối thiểu (không đổi kỳ vọng/ca); psvn targets 18/18 (từ 8/18), cargo test 1053/100, host 39/54 (15 ca trùng ST-136), clippy hết đỏ; gate Windows dừng bước 5/10 (cp1252 — VS3); Windows vẫn "chưa hỗ trợ"
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: SO-NO.md 142 mục (mở 92/đóng 28/VS3 20/không làm 1/điều hướng 1), trùng số 0, #109a/#109b tách, mỗi mục có nguồn
APPROVED 2026-11-17 Agent 0 · theo QUYET-DINH-A0-2026-11-16g §7 · Agent 1 đối chiếu: R25 đủ 7 hàng phụ thuộc + 3 điểm đính chính R24 + 5 câu VD-131 + hash từng nền; check_report PASS (25 báo cáo, 164 hàng); gate macOS PASS L0 1159/0
```

---

## §7. Số đo tổng hợp (dùng cho tệp duyệt đợt 25)

| Phép đo | macOS | Linux `ps` | Windows `psvn` (chỉ đo) |
|---|---|---|---|
| `cargo test --workspace` | **1159/0** (129 suite) | **1162/0** (129 suite) | **1053/100** (129 suite) |
| `gate --fast` | **PASS** (10/10 bước, L0 1159/0) | **PASS** (10/10, L0 1162/0) | dừng **5/10** (self-test cp1252) |
| `vis test test/sem` (interp / `--vm`) | 447 ok / 447 ok · 0 SKIP | 447 ok / 447 ok · 0 SKIP | — |
| differential 100k seed `20261115` | lech=0 · 157,4 s | lech=0 · 40,9 s | — |
| cache 520 tệp ×4 | 0 lệch · 25,43 s | 0 lệch · 6,33 s | — |
| host suite 54 ca | **54/54** | **54/54** | **39/54** |
| `vis-targets` (mức check) | **18/18** | **18/18** | **18/18** |
| soak 10 vòng | 10/10 = 1352 s (HEAD cũ `3079dede`) | **10/10 = 250 s** | — |
| Landlock (`vis-host-linux`) | — | **13/13** (3 ca âm tiến trình con) | — |
| TOCTOU đường đọc | 0/50 000 | 0/50 000 (2 lần) | — |

**Khoá khác**: R1 max **1498** dòng/tệp · `FUNCS` **52** · `all_codes` **33** · `VBC1` **53** · allowlist doc-examples **6** · doc-examples **24 khối = 18 xanh + 6 ngoại lệ**.
**5 câu bắt buộc (VD-131)** — nguyên văn trong `R25` §4.1, vẫn đúng cả 5: *Async không song song. · Kênh không rendezvous. · Net không TLS. · Quyền chỉ siết mặc định khi dùng gói. · LSP chỉ publish tài liệu đang mở.*
**Nợ**: **142 số** trong `SO-NO.md` — **92 mở**, 28 đóng, **20 VS3**, 1 "không làm", 1 điều hướng. Ưu tiên theo sổ: `#140/#109b`, `#129`, `#133`, `#135`, `#136`, `#132`, `#138`, `#125`, `#117`, `#110–#112`.

---

## §8. Đề nghị đợt 25 (và phạm vi VS3)

1. **Đóng đợt**: anh ký §3 (D1, D2, D3) + 8 dòng `APPROVED` ở §6 ⇒ đợt 24g chính thức khép; `ST-138b` được chuẩn thuận; `R25` là báo cáo chốt.
2. **Đính chính tài liệu**: `HOST-BAO-DAM.md` 3 chỗ (D3) + một dòng xác nhận phần FsGuard đã đúng theo VD-145.
3. **Đợt 25 (đề nghị, mỗi việc ≤ 3 h)**: (a) sửa `doc_examples.py` self-test trên Windows nếu anh muốn Windows gate xanh (**VS3**, cần VD riêng); (b) xử lý 3 nợ ưu tiên `#140/#109b`, `#129`, `#133`; (c) nếu có `#142/#143` thì cập nhật `SO-NO.md` + một báo cáo nhỏ.
4. **VS3**: theo **VD-148**, **không** bắt đầu tính năng VS3 cho tới khi anh ban hành tài liệu thiết kế VS3 (async song song + wait-set; rendezvous/select/timeout; TLS/HTTP2/keep-alive; `openat`/`O_NOFOLLOW` cho đường ghi; Windows; resolver qua mạng/chữ ký; macOS fd→path).

---

## §9. Phụ lục — nơi tra

| Việc | Đường dẫn |
|---|---|
| Báo cáo chốt | `viscript/agent/reports/R25-viscript.md` (+ `.json`, `.json.license`) |
| Sổ nợ | `viscript/agent/SO-NO.md` |
| Sổ task (164 hàng, tất cả DONE) | `viscript/agent/QUEUE.md` |
| Quyết định | `viscript/agent/DECISIONS.md` (mới nhất **VD-148**) |
| Bản dán quyết định 16g | `viscript/agent/JOURNAL/quyet-dinh-a0-2026-11-16g.md` |
| Nhật ký đêm | `viscript/agent/JOURNAL/2026-11-16.md` (mục ST-138b…ST-146) |
| Log chính | `st138b-gate-fast.log`, `st140-*`, `st141-*`, `st142-*`, `st143-soak.log`, `st144-*`, `st145-*`, `st146-gate-macos.log` |
| Trạng thái | `viscript/agent/STATE.md` (header: vòng 25, chờ Agent 0) |

**Giới hạn của báo cáo này** (nói thẳng): tôi không chạy `cargo`/`clippy`/L0/cổng/soak (VD-137/147(1)); mọi số `cargo test`/`gate` là **số Agent 2 đo**, tôi **đọc log + đối chiếu sha256**; riêng `check_report.py --all` tôi chạy trực tiếp tại HEAD (`b6e326f2`).
