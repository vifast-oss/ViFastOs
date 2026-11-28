<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VIỆC CỦA AGENT 2 (ViScript) — BÁO CÁO ĐỐI CHIẾU + ĐỀ NGHỊ DUYỆT CHO **AGENT 0**

**Ngày**: 2026-11-16 (18:45 JST) · **Từ**: Agent 1 · **Gửi**: Agent 0 (người duyệt, **không** truy cập repo)
**HEAD khi viết**: `a9f80d83` (`[ST-138] R24 chốt VS2 (kỹ thuật) + đính chính R23 + ST-135 DONE; soak 10/10 = 1352 s`)
· `origin/main` = HEAD (`rev-list origin/main..HEAD` = 0) · cây `viscript/` **sạch** (0 tệp chưa commit)
**Tài liệu này tự chứa**: không cần mở repo. Mọi con số đều có **đường dẫn tệp bằng chứng** trong repo
(nếu có) hoặc ghi rõ **do tôi đọc trực tiếp trong code**.

> **Một câu tóm tắt.** Agent 2 dòng ViScript đã làm xong **9 hàng của đợt 23f/24** (ST-130b, ST-132b, ST-134,
> ST-135, ST-136, ST-137, R24 + R23) và **không còn hàng BLOCKED**. Nó đang **chờ đúng 9 chữ ký duyệt** của
> anh, **4 quyết định kỹ thuật**, **3 quyền môi trường (chỉ Windows)** và **2 việc dọn dẹp**. Tôi đã đối chiếu
> từng hàng với tiêu chí của chính nó (đọc code + đọc log; **không** chạy cargo/clippy/L0 — Agent 1 bị VD-137
> cấm chạy cổng) và **tất cả 9 hàng ĐẠT về nội dung kỹ thuật**. Có **7 phát hiện phụ**: **1 cần sửa** (log soak
> bị ghi đè), **1 cần một lần chạy cổng** (R24 chưa được cổng sau khi tồn tại), còn lại là ghi chú/minh bạch số
> liệu — nằm ở §4. Việc của anh ở **§5 (D1..D9)**.

---

## §0. Cách đọc (3 phút)

| Mục | Nội dung | Anh cần |
|---|---|---|
| §1 | Ảnh chụp hiện trạng (HEAD, số đo 3 nền, 8 hạng mục VS2) | đọc |
| §2 | **Agent 2 đang chờ gì** (9 chữ ký + 4 quyết định + 3 quyền môi trường + 2 dọn dẹp) | **thuộc** |
| §3 | Tôi đã đối chiếu gì, kết quả từng hàng (ĐẠT/ĐẠT-có-ghi-chú) | đọc |
| §4 | 7 phát hiện khi review (1 cần sửa, 6 ghi chú) | đọc |
| **§5** | **9 quyết định D1..D9 — có văn bản sẵn để anh ký** | **LÀM** |
| §6 | Mẫu trả lời ngắn (9 dòng) | dùng |
| §7 | Phụ lục: chỉ mục bằng chứng, lệnh tái lập, thuật ngữ, giới hạn của báo cáo này | tra |

**Bối cảnh 6 dòng** (cho người đọc lạnh): ViFastOs có 2 dòng sản phẩm độc lập trong 1 repo — dòng OS
(`KT-*`, thư mục `v2/`) và dòng **ViScript** (`ST-*`, thư mục `viscript/`). ViScript là ngôn ngữ hướng shell
(lexer → CST lossless → kiểm kiểu → HIR → interpreter + VM `VBC1` + bộ kiểm vi sai), kèm công cụ
`vis check/test/fmt/cache/explain/pkg/lsp/run`. "VS2" là mốc phát hành thứ 2 gồm **8 hạng mục** (async, kênh,
net/HTTP, quyền, LSP, gói, đa nền tảng, hợp chuẩn host). Luật cứng: Agent 2 **không** sửa `spec/`, `docs/`,
`DECISIONS.md`, `QUEUE.md`; không nới ngưỡng; không `#[ignore]`; không retry để qua cổng; mỗi task ≤ 3 h.

---

## §1. Ảnh chụp hiện trạng

**Repo**: `main` = `origin/main` = `a9f80d83`; `viscript/` không có tệp chưa commit. (Ở gốc repo có thay đổi
chưa commit của **dòng OS** — không liên quan báo cáo này.)

**Sổ**: `viscript/agent/QUEUE.md` **156 hàng** = **155 DONE + 1 READY** (ST-138, chưa lật theo VD-143);
**24 báo cáo** `R1..R24`; **VD mới nhất = VD-143**; `viscript/agent/BLOCKED.md` **không còn mục chặn hiện tại**
(các mục Q1..Q5 trong đó là di sản vòng 1).

| Phép đo | macOS (máy dev) | Linux (`ps`, qua SSH) | Windows (`psvn`) — **chỉ đo** |
|---|---|---|---|
| `cargo test --workspace` | **1159 / 0** (129 suite) | **1161 / 1** (129 suite; tổng 1162) | **1051 / 102** (129 suite) |
| `vis test test/sem` (interp / `--vm`) | 447 ok / 447 ok · 0 SKIP | chưa đo (nợ #130) | — |
| differential 100k seed `20261115` | **lech = 0** | chưa đo | — |
| host suite (54 ca) | **54/54** | **54/54** | **39/54** |
| `bin/vis-targets` (mức `check`) | **18/18** | **18/18** (sau `viscript/infra/linux/setup.sh`) | **8/18** |
| `gate --fast` | **PASS** | **10/11 bước**; L0 đỏ **đúng 1 ca môi trường** | **FAIL ở `clippy`** (ca `sandbox.rs`, hệ quả ST-134, chỉ hiện trên Windows) |
| soak 10 vòng | **10/10 = 1352 s** | — | — |
| Landlock (`vis-host-linux`) | — | **13/13** ca (3 ca âm tiến trình con) | — |
| TOCTOU đường đọc | **0/50 000** (`ok_trong=13 852`) | **0/50 000** (2 lần: `ok_trong=29 244` và `34 142`) | — |

Số khoá khác: R1 max **1498** dòng/tệp (luật ≤1.500, VD-128) · `FUNCS` **52** · `all_codes` **33** ·
`VBC1` **53** · allowlist doc-examples **6** · doc-examples **24 khối = 18 xanh + 6 ngoại lệ**.

**Tám hạng mục VS2 (R24 §4)**: (1) Async **chưa có trong VS2 → VS3**; (2) Kênh **chưa có → VS3**;
(3) Net/HTTP **chưa có TLS/HTTP2 → VS3**; (4) **Quyền: ĐÓNG về kỹ thuật, chờ chữ ký ST-137**; (5) LSP **ĐÓNG**;
(6) Gói **ĐÓNG** (trong phạm vi VS2); (7) Đa nền tảng **một phần** (Linux chạy thật, Windows chỉ đo, wasm chỉ
`check`); (8) Hợp chuẩn host **ĐÓNG** (54/54 trên cả macOS và Linux).

**5 câu bắt buộc (VD-131) có nguyên văn trong R24 §3**: "Async không song song." / "Kênh không rendezvous." /
"Net không TLS." / "Quyền chỉ siết mặc định khi dùng gói." / "LSP chỉ publish tài liệu đang mở."

---

## §2. Agent 2 đang chờ đúng những gì

### 2.1. Chín hàng "CHỜ DUYỆT" (xếp theo thứ tự nên ký)

| # | Hàng | Trạng thái QUEUE | Ai đối chiếu (theo dòng CHỜ DUYỆT) | Anh cần |
|---|---|---|---|---|
| 1 | **ST-130 + ST-130b** | DONE | Agent 1 | ký |
| 2 | **ST-132** (Landlock thật) | DONE | Agent 0 (đã XEM LẠI đòi ca âm thứ 3 → ST-132b đã bù) | ký (gộp với #3) |
| 3 | **ST-132b** (ca âm thứ 3 `read_dir`) | DONE | Agent 0 | ký |
| 4 | **ST-134** (vá runner + 2 ca theo VD-132 + ca iv) | DONE | Agent 0 | ký |
| 5 | **ST-135** (chạy lại đầy đủ trên Linux) | DONE | Agent 1 | ký |
| 6 | **ST-136** (đo trên Windows, chỉ đo) | DONE | Agent 1 | ký |
| 7 | **ST-137** (sửa rò rỉ TOCTOU Linux) | DONE | Agent 1 | ký |
| 8 | **ST-133** (R23 — báo cáo đợt 23) | DONE | Agent 0 | ký (kèm ghi "R23 **không** chốt VS2") |
| 9 | **ST-138** (R24 — chốt VS2) | **READY** (chưa lật, VD-143) | Agent 1 | ký **sau** #1..#8 |

Chi tiết chín dòng này nằm ở `viscript/agent/QUEUE.md` dòng ~255–281 (mỗi hàng có một dòng `> CHỜ DUYỆT …`
ngay dưới, ghi commit + số đo). Tôi đã **không** sửa dòng nào.

### 2.2. Bốn quyết định kỹ thuật Agent 2 không được tự quyết (nó đã ghi đúng)

| Ký hiệu | Việc | Nợ/nguồn | Vì sao cần anh |
|---|---|---|---|
| Q-A | **Ca `symlink_canonicalize_hai_phia` đỏ trên Linux** vì `/usr/bin/echo` là **coreutils multicall** (binary từ chối chạy khi `argv[0]` khác tên) | nợ **#139** (R24 §5) | Đây là ca **thứ ba** chạm nền; VD-132 mới cho **2** ca. Sửa ca = đụng kỳ vọng test ⇒ phải có VD |
| Q-B | **macOS chưa có fd→path**: giữ bước xác nhận hai-syscall của ST-124; đo 0/50 000 nhưng **không bảo đảm theo cơ chế** | nợ **#137** | Muốn bảo đảm cần `libc`/`unsafe` (cần duyệt crate + unsafe) hoặc chuyển VS3 |
| Q-C | **Đường GHI vẫn kiểm-rồi-mở** (đường đọc đã đóng bằng `/proc/self/fd`); cần văn bản giới hạn trong `HOST-BAO-DAM.md` | nợ **#140** (nợ cũ #109) | `HOST-BAO-DAM.md` là tài liệu cam kết, do anh sở hữu nội dung |
| Q-D | **Windows**: giữ "chỉ đo" (VD-121/VD-136) hay mở VS3 cho Windows | R24 §2.3 | Quyết định phạm vi |

### 2.3. Ba "quyền môi trường" chỉ trên Windows (BLOCKED-ENV, Agent 2 đã dừng đúng luật)

| # | Việc | Bằng chứng | Ghi chú |
|---|---|---|---|
| W-1 | `rustup target add wasm32-unknown-unknown` (+ tuỳ chọn `x86_64-unknown-linux-gnu`) trên `psvn` để `vis-targets` 8/18 → 18/18 | R24 §2.3; `JOURNAL/st136-windows.md` | Chỉ **thành phần toolchain** (VD-136/VD-140 cho phép), không cài phần mềm hệ thống |
| W-2 | Sửa **1 dòng** `encoding="utf-8"` trong `viscript/test/doc-examples/doc_examples.py` (hiện đọc ống bằng `cp1252` ⇒ `UnicodeDecodeError` trên Windows) | `JOURNAL/st136-windows.md`; log `st136-lan1-gate-fast.log` | Lỗi **script kiểm**, không phải sản phẩm |
| W-3 | Xử lý biến không dùng ở `crates/vis-cli/tests/sandbox.rs` khi **cả hai** nhánh `cfg(target_os)` bị loại trên Windows (clippy đỏ) | `JOURNAL/st136-windows.md`; dòng 61 của tệp | Test-only; cần anh cho phép vì VD-136 cấm sửa cho Windows |

### 2.4. Bốn việc Agent 2 tự khai trong `STATE.md` ("Cần Agent 0/1")

1. Dán `APPROVED` cho ST-137 (và ST-130/132b/134/136) để hạng mục **Quyền** chính thức đóng.
2. VD thứ 3 cho ca coreutils (**Q-A** / nợ #139).
3. Một dòng giới hạn **đường GHI** trong `HOST-BAO-DAM.md` (**Q-C** / nợ #140).
4. Quyết định **Windows**: `VD-121` "chỉ đo" hay VS3 (**Q-D**).

**Không còn hàng BLOCKED.** `viscript/agent/BLOCKED.md` còn các mục Q1..Q5 **từ vòng 1** (đã xử lý hoặc
quá cũ) — tệp này **không** được cập nhật từ lâu, đừng đọc nó như trạng thái hiện tại; nguồn đúng là
`QUEUE.md` + `STATE.md` header.

---

## §3. Tôi đã đối chiếu gì (và kết quả)

**Phạm vi tôi làm**: đọc code + log trong repo, đối chiếu từng dòng `> CHỜ DUYỆT` với **tiêu chí của chính
hàng đó** trong QUEUE; chạy **đúng một** bước đọc-only của cổng là `check_report.py --all` (xem §4-F7).
Tôi **không** sửa tệp nào, **không** chạy `cargo`, `clippy`, L0, gate, soak.

| Hàng | Tiêu chí (tóm) | Bằng chứng tôi **tự** kiểm | Kết luận |
|---|---|---|---|
| **ST-130b** | Thêm **ca thứ 8** vào `crates/vis-cli/tests/siet_caps_pkg.rs`: `use` module nội bộ + `use std.fs`, **không** `pkg.*`, **không** `cap` ⇒ chạy được, không `VS0604`, cả interp và `--vm --no-cache` | Tệp có **đúng 8 `#[test]`**; ca thứ 8 `use_module_noi_bo_va_std_nhung_khong_pkg_giu_hanh_vi_cu` chạy cả 2 đường, đòi `exit=0`, `stdout="x\n7\n"` (chứng minh module thật được nạp), và **đối chứng nhúng trong ca**: cùng chương trình + `--strict-caps` ⇒ `exit=2` + `VS0604` + nêu `cap[Dir]` + không in gì | **ĐẠT** |
| **ST-132** | Landlock thật: ≥8 ca `cfg(linux)` xanh trên runner, **≥3 ca âm ở tiến trình con** chứng minh **kernel** chặn | `vis-host-linux/tests/landlock.rs` có **13 `#[test]`**; ca âm thứ 3 ở dòng 258 (`con_read_dir_ngoai_pham_vi_bi_kernel_chan`); 2 ca âm cũ (đọc ngoài, ghi ngoài) + ca đối chứng; macOS `--sandbox` exit 3 | **ĐẠT** (sau ST-132b) |
| **ST-132b** | Con `read_dir` **ngoài** phạm vi ⇒ `PermissionDenied`/`EACCES`; có **đối chứng dương** cùng ca; có đỏ-trước | Đọc trực tiếp: trước ruleset ⇒ `exit=0` + thấy `bi-mat.txt` (đỏ-trước); sau ruleset ⇒ `exit=7` + stderr `Permission denied`; cùng helper `read_dir` **trong** phạm vi ⇒ `exit=0` + thấy `duoc.txt` | **ĐẠT** |
| **ST-134** | (2a) toolchain ghi vào **script trong repo**; (2b) **2 ca** theo VD-132, không xoá/ignore/cfg-bỏ, mỗi nhánh nền khẳng định **giá trị cụ thể**; (2c) ca (iv) kết luận 1 trong 3 loại | (2a) `viscript/infra/linux/setup.sh`: `TARGETS=(wasm32-unknown-unknown x86_64-pc-windows-msvc)`, idempotent, `VIS_SETUP_CHECK=1` chỉ kiểm, exit 3 khi thiếu rustup, log môi trường + `digest-toolchain` (`JOURNAL/st134-setup-ps.log`). (2b) `sandbox.rs:66/74` hai nhánh nền; ca grep tiền tố ở `pipeline.rs` (nhật ký `st134-runner-ca-nen.md` §2b). (2c) Kết luận **(b) lỗi sản phẩm**, sửa **+65/−3 = 68 dòng** (`cmdsig.rs` +15, `lower/khai.rs` +47, `lower.rs` 6), `vish_cmd_body` 16/16 **cả hai nền** | **ĐẠT** |
| **ST-135** | Chạy lại đầy đủ trên Linux, **chỉ đo**, không sửa | `JOURNAL/st135-linux-lai.md`: cây đo HEAD `7881eb30`; `cargo test` **1161/1**, host **54/54**, targets **18/18**, gate **10/11**; ca đỏ duy nhất phân loại **môi trường** (coreutils multicall) | **ĐẠT** |
| **ST-136** | Windows **chỉ đo**; log môi trường đủ mục VD-136; mỗi ca lệch có tên + hai đầu ra + nhóm **phỏng đoán**; không sửa mã/kỳ vọng/cfg | `JOURNAL/st136-env.log` đủ: máy, OS build, rustc `1.99.1 (58cf8f9c3)` **khớp `ps`**, `autocrlf=false`, repo path, targets, Build Tools 17.14.8/MSVC 14.44.35207, hash `632e1fba`. `JOURNAL/st136-windows.md` (45 KB) có bảng 102 ca đỏ + nhóm nguyên nhân | **ĐẠT** (một số liệu cần ghi rõ hơn — §4-F6) |
| **ST-137** | Bước 0 chỉ **cửa sổ thật**; sửa không `unsafe`, không crate mới; đỏ-trước có log; sau: **0/50 000** trên `ps` **và** macOS; ca `/proc` không đọc được ⇒ từ chối; host 54/54 hai nền; diff 100k `lech=0` | (a) `e47cfb2f` chạm `quyen.rs` +269, `ngoai.rs` +70, `builtin.rs` +36, `main.rs` +8 — **không** chạm `Cargo.toml/lock` ⇒ **không crate mới**; `grep unsafe` trong code mới: **0** (chỉ có chữ "unsafe" trong chú thích). (b) `quyen.rs` có **6 `#[test]`** = 2 cũ + **4 mới** đúng như dòng CHỜ DUYỆT (`handle_ngoai_root_bi_chan_tat_dinh`, `doc_lay_tu_handle_khi_ten_da_bi_doi`, `proc_khong_doc_duoc_thi_tu_choi`, `doc_theo_handle_bao_dung_duong_dan_cua_proc`). (c) Log: đỏ-trước `ro_ri=92` /50 000 (`st137-red-ps.log`); sau **0/50 000** với `ok_trong=29 244` (`st137-sau-ps.log`) và `34 142` (`st137-sau-ps-sachtree.log`), macOS `ok_trong=13 852` (`st137-sau-macos.log`). (d) host **54/54** hai nền (`st137-host-suite-ps.log`, `…-macos.log`); diff 100k `lech=0` (`st137-diff-100k.log`) | **ĐẠT** |
| **ST-133 (R23)** | Có đủ số khoá, 5 câu bắt buộc, 8 hạng mục theo 3 loại, soak 10/10 | R23 tồn tại; 2 câu **sai** (nguồn gốc ST-132b; nợ #126) đã được **ĐÍNH CHÍNH** trong QUEUE (2026-11-16) và R24 §1 đính chính lại theo VD-142 | **ĐẠT để duyệt như báo cáo đợt 23** (không phải chốt VS2) |
| **ST-138 (R24)** | Đủ 6 hàng phụ thuộc + trạng thái ở đầu; mục "Đính chính R23"; 5 câu VD-131; 8 hạng mục 3 loại; Linux và Windows **mỗi nền một mục số đo**; soak 10/10; `report-check` PASS | R24 §0 đủ 6 hàng; §1 đủ 8 điểm đính chính; §3 đủ 5 câu; §4 đủ 8 hạng mục; §2.2 Linux, §2.3 Windows; soak log 10/10 (xem §4-F1 về **tên tệp** log); JSON `round=24`, `head=3079dede`, 156 task, 11 khoá. **`check_report.py --all` tại HEAD: PASS (24 báo cáo, 156 hàng QUEUE) — tôi vừa chạy, exit 0** | **ĐẠT**, còn **1 điều kiện thủ tục** (§4-F3) |

**Kiểm tra "luật cấm" trên 5 commit của đợt** (`eab366d7`, `a47a1ffe`, `6e6673bd`, `e47cfb2f`, `a9f80d83`):
không thấy `#[ignore]`/`#[should_panic]` mới, không nới `MAX_IO/MAX_TCBS/MAX_OBJECTS/1.500`, không thêm
`unsafe`, không thêm crate ngoài. Soak được ghi **không retry**: lần 1 dừng vì `T-INFRA` (ca `net` flake, chạy
lại **1 lần** để phân loại, 1 passed), lần 2 dừng vì **vi phạm R3 do chính báo cáo viên** (thiếu sidecar
`.license`), lần 3 10/10 = 1352 s — cả 3 ghi trong R24 §6, đúng D2-07.

---

## §4. Bảy phát hiện khi review

### F1 — **[CẦN SỬA]** Log soak ghi đè bằng chứng lịch sử (đúng format, sai nơi ở)

`viscript/bin/vis-soak` ghi cứng `LOG=agent/JOURNAL/st113-soak.log`. Hệ quả: soak của **đợt 24** đã **ghi đè**
log soak của **vòng 20 (ST-113)**; và trước đó **ST-133** cũng đã ghi đè một lần. Bằng chứng:
`git log -- agent/JOURNAL/st113-soak.log` có **3 commit** (`36ed5922` ST-113 tạo, `739f6e69` ST-133 đè,
`a9f80d83` ST-138 đè lần nữa — 2364 dòng đổi).
Nội dung **hiện tại** của tệp là soak đợt 24 (`=== TỔNG | 1352s | 10/10 vòng OK | load sau: …`), tức **R24 §2.1
có bằng chứng thật**, nhưng nằm trong một tệp **mang tên đợt 20** và **bản ghi đợt 20 đã mất khỏi cây**.
Không ảnh hưởng số đo; ảnh hưởng **tính tái lập** (đúng tinh thần VD-142: log/báo cáo là bản ghi lịch sử).
**Đề nghị** (D8): cho `VIS_SOAK_LOG`/`--log`, mỗi đợt ghi tệp riêng `st<ID>-soak.log`, khôi phục
`st113-soak.log` từ git, **không** xoá log cũ.

### F2 — [GHI CHÚ] JSON của R24 và QUEUE hiện tại lệch trạng thái **ST-135** — nhưng đúng ngữ nghĩa

`R24-viscript.json` ghi ST-135 = `READY`; `QUEUE.md` (sau cùng một commit) ghi **DONE**. Lý do: máy kiểm báo
cáo đối chiếu với **QUEUE tại `head` của báo cáo** (`queue_at(3079dede)`), thời điểm ST-135 còn READY. Tôi đã
đọc `viscript/test/report/check_report.py` (hàm `main`) xác nhận đây là **thiết kế**, không phải lỗi. Không cần
sửa R24 (VD-142 cấm); chỉ cần biết khi đọc JSON. (Nội bộ R24 MD §0 ghi DONE — chênh với JSON — nên
đính chính một dòng khi có R25.)

### F3 — [CẦN MỘT LẦN CHẠY] Cổng của chính commit R24 chưa được chứng minh

Soak (10/10, 1352 s) chạy **trước** khi R24 được viết: log soak ghi `report-check: PASS (23 báo cáo, 156 hàng)`,
tức lúc đó **chưa có** báo cáo thứ 24. Nghĩa là dòng "gate PASS" của R24 là cổng **trước khi R24 tồn tại** —
bình thường, nhưng commit cuối (`a9f80d83`) **thêm** R24 + lật ST-135 DONE mà **chưa** có lần cổng nào sau đó.
Bù lại: tôi vừa chạy riêng **bước báo cáo** tại HEAD ⇒ **PASS (24 báo cáo, 156 hàng)**. Đề nghị Agent 2 chạy
**một** `gate --fast` trên HEAD rồi dán dòng cuối vào QUEUE (rẻ, và làm dòng duyệt của anh chắc).

### F4 — [GHI CHÚ] `bin/vis-host-suite` in cứng "chạy thật trên macOS" cả khi chạy trên Linux

`st137-host-suite-ps.log` dòng cuối: `vis-host-suite: 54/54 ca xanh (chạy thật trên macOS; 3 nền còn lại chưa đo)`
— chạy trên `ps`. Câu này lấy từ `bin/vis-host-suite:36` (chuỗi cứng). Không sai số (54/54 thật), nhưng **dòng
bằng chứng gây hiểu nhầm** cho người đọc sau. Sửa 1 dòng: in nền thật (`uname -sm`).

### F5 — [GHI CHÚ] Số nợ bị **trùng số**: `#109` mang hai nghĩa

ST-128 ghi "nợ **#109**" = `next_block()` = 801; R24 §5 dòng 140 ghi "nợ cũ **#109**" = **đường ghi kiểm-rồi-mở**.
Hai nợ khác nhau cùng số. R24 cũng đánh lại dải 132..141 trong khi các nợ cũ (#115, #124…) còn được nhắc ở
nơi khác. Đề nghị (D9): một **sổ nợ duy nhất** (mỗi số một nghĩa, có cột "đóng ở task nào") để anh theo dõi.

### F6 — [GHI CHÚ] R24 **không ghi hash commit đo** cho macOS và Linux

R24 §2.1 chỉ ghi "máy dev arm64", §2.2 ghi máy `ps` nhưng **không** hash; chỉ §2.3 (Windows) có `632e1fba`.
`STATE.md` ghi số macOS đo ở `7881eb30`; JSON `head=3079dede` là commit **dòng OS** (không chạm `viscript/`).
Không sai số, nhưng thiếu **truy vết**. Đề nghị: mỗi nền một dòng hash trong R25 (hoặc một dòng đính chính
trong QUEUE).

### F7 — [GHI CHÚ QUY TRÌNH] Tôi đã chạy `check_report.py --all` — xin anh xác nhận có được phép

VD-137 cấm Agent 1 "chạy cổng". `check_report.py` là **bước nằm trong** `gate --fast`, nhưng **chỉ đọc**
(không build, không đụng mã). Tôi chạy **một lần** để trả lời câu "R24 có làm đỏ cổng không". Nếu anh muốn
siết: ghi một câu trong VD kế tiếp ("Agent 1 được chạy các bước kiểm tài liệu chỉ-đọc: `check_report.py`,
`doc_examples.py --gate`") — hoặc cấm hẳn, tôi sẽ bỏ.

---

## §5. Chín quyết định cần anh ký (văn bản sẵn để dán)

> Cách anh trả lời: viết **một** tệp `QUYET-DINH-A0-2026-11-16f.md` (mẫu ở §6) với các mục §4.1..§4.9
> tương ứng D1..D9. Agent 1 dán **nguyên văn** vào `DECISIONS.md`/`QUEUE.md`/`HOST-BAO-DAM.md` theo VD-137,
> rồi Agent 2 commit `[ST-00]`. Các dòng `APPROVED` dưới đây là **văn bản mẫu**, anh sửa được tuỳ ý.

### D1 — Duyệt 8 hàng đã CHỜ DUYỆT (nội dung kỹ thuật đã đủ)

**Đề nghị**: duyệt, mỗi hàng một dòng, đặt **ngay dưới** dòng `> CHỜ DUYỆT` tương ứng trong `QUEUE.md`:

```
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: 8/8 #[test], ca thứ 8 có đối chứng --strict-caps ⇒ VS0604, sem 447/22 không đổi
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: Landlock 13/13 + 3 ca âm tiến trình con + đối chứng dương (ST-132 + ST-132b)
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: setup.sh 2 target trong repo; 2 ca theo VD-132 có nhánh nền; ca (iv) lỗi sản phẩm 68 dòng; 16/16 hai nền; targets 18/18 trên ps
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: Linux 1161/1 (129 suite), host 54/54, targets 18/18, gate 10/11 (1 ca môi trường)
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: chỉ đo, log môi trường đủ VD-136, 102 ca lệch có tên + hai đầu ra + nhóm phỏng đoán, không sửa code/kỳ vọng
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · Agent 1 đối chiếu: cửa sổ thật ở xác nhận-sau-khi-mở; đỏ-trước 92/50 000 → 0/50 000 trên ps và macOS; 4 ca mới; không unsafe, không crate mới; host 54/54 hai nền; diff 100k lech=0
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · R23 duyệt như BÁO CÁO ĐỢT 23, KHÔNG chốt VS2 (VD-143); 2 câu sai đã đính chính tại QUEUE 2026-11-16 và R24 §1 (VD-142)
```

**Nếu anh muốn ký gọn**: một dòng duy nhất
`APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.1 · duyệt ST-130/130b, ST-132/132b, ST-134, ST-135, ST-136, ST-137, ST-133; Agent 1 đã đối chiếu từng hàng`
— hợp luật, nhưng mất chi tiết bằng chứng; tôi khuyên dòng riêng từng hàng.

### D2 — Q-A / nợ #139: ca coreutils multicall (khuyến nghị **(a)**)

| Lựa chọn | Nội dung | Ưu / nhược |
|---|---|---|
| **(a) khuyến nghị** | Cho sửa **chính ca đó**: exec `/bin/sh` với `-c 'echo a'` (hoặc chương trình không-multicall) trên **cả hai nền**; giữ nguyên phần khẳng định "canonicalize hai phía"; **không** xoá ca, **không** `#[ignore]`, **không** cfg-bỏ | Repo-only, tất định, ca vẫn kiểm đúng điều cần kiểm. Chi phí ~10 dòng + 1 lần cổng trên `ps` |
| (b) | Sửa **máy** `ps`: bỏ uutils coreutils khỏi `/usr/bin` (hoặc cài `echo` thật) | Ngoài repo, cần người vận hành, dễ tái diễn khi image đổi |
| (c) | Cho ca thứ 3 "kỳ vọng theo nền" như VD-132 | Không phân biệt được bằng `cfg` (cùng Linux, khác image) ⇒ phải dò lúc chạy, phức tạp hơn (a) |

**Văn bản VD mẫu (nếu (a))**:

```
### VD-144 · Ca coreutils multicall: đổi chương trình exec, giữ nguyên điều kiểm (2026-11-16)
Trên runner `ps`, `/usr/bin/echo` là symlink vào coreutils multicall (`/usr/lib/cargo/bin/coreutils/echo`)
và binary này từ chối chạy khi `argv[0]` khác tên của nó ⇒ ca `symlink_canonicalize_hai_phia` đỏ vì MÔI TRƯỜNG,
không phải lỗi quyền (sản phẩm canonicalize rồi exec đúng đường dẫn thật). Cho phép sửa CHÍNH CA: dùng
chương trình không-multicall (ví dụ `/bin/sh -c 'echo a'`) trên CẢ HAI nền; giữ nguyên hai chiều khẳng định
canonicalize; không xoá ca, không `#[ignore]`, không `cfg`-bỏ, không nới ngưỡng; ghi lý do trong chú thích ca.
Không áp cho ca nào khác.
```

### D3 — Q-B / nợ #137: macOS chưa có fd→path (khuyến nghị **(a)**)

| Lựa chọn | Nội dung | Đánh giá |
|---|---|---|
| **(a) khuyến nghị** | **Chấp nhận là giới hạn có văn bản**: macOS giữ bước xác nhận hai-syscall của ST-124; đo 0/50 000 nhưng **không** bảo đảm theo cơ chế; sửa bằng `libc`/`unsafe` = VS3 | Không mở `unsafe`, không thêm crate; trung thực với mức thật (đúng VD-131) |
| (b) | Duyệt `libc` + `unsafe` đóng cho macOS `fcntl(F_GETPATH)` ngay trong VS2 | Mở 2 cánh cửa đang đóng (D2-17, allowlist); rủi ro; không cần cho mục tiêu VS2 |
| (c) | Chuyển nguyên hạng mục Quyền sang VS3 | Quá tay: Linux đã đóng, macOS đóng ở mức runtime — chỉ còn 1 giới hạn |

**Văn bản mẫu (nếu (a))**:

```
### VD-145 · macOS không có fd→path trong VS2 (2026-11-16)
Trên macOS, std an toàn không có cách lấy đường dẫn thật của handle đã mở (`/dev/fd` readlink trả EINVAL;
`fcntl(F_GETPATH)` cần `libc`/`unsafe`). VS2 giữ bước xác nhận hai-syscall của ST-124 trên macOS; số đo
0/50 000 nhưng KHÔNG bảo đảm theo cơ chế. Giới hạn này ghi trong HOST-BAO-DAM.md. Sửa cần `libc` hoặc
`unsafe` ⇒ việc VS3, phải có VD riêng.
```

### D4 — Q-C / nợ #140: văn bản giới hạn đường ĐỌC (đã đóng) và đường GHI (còn) trong `HOST-BAO-DAM.md`

Hiện `HOST-BAO-DAM.md` còn đoạn **tạm thời** ("VD-141, tới khi ST-137 APPROVED"): nói bất biến đường đọc
**chưa đạt trên Linux**. Sau ST-137 câu đó **hết đúng** — phải thay. Văn bản đề nghị (thay nguyên đoạn gạch
đầu dòng "Đường đọc" và dòng tạm thời):

```
- **Đường đọc**: mở handle trước rồi **quyết định quyền trên đường dẫn THẬT của chính handle**; nội dung trả về
  đọc từ **chính handle đó**. Trên Linux: đường dẫn thật lấy qua `/proc/self/fd/<fd>`; `/proc` không đọc được
  ⇒ từ chối bằng `VS0604` (không quay về kiểm theo tên). Trên macOS: không có fd→path trong std an toàn ⇒ giữ
  bước xác nhận (dev, ino) trước/sau canonicalize như ST-124; đo 0/50 000 nhưng bảo đảm theo cơ chế chỉ có
  trên Linux (giới hạn có văn bản, VD-145).
  Bất biến: nội dung trả về là nội dung của chính tệp mà quyết định quyền đã áp.
- **Đường ghi: giới hạn đã biết, còn nguyên trong VS2.** FsGuard kiểm đường dẫn rồi mới ghi (mở trước để lấy
  handle là đã tạo/cắt tệp) ⇒ cửa sổ đổi symlink giữa hai bước vẫn tồn tại. Sửa triệt để cần `openat` +
  `O_NOFOLLOW` (cần `unsafe` hoặc crate ngoài) ⇒ VS3. Khi chạy `--sandbox` trên Linux, Landlock chặn ghi ra
  ngoài vùng được cấp ở mức kernel.
```

**Đề nghị**: anh duyệt **nguyên văn** trên; Agent 1 dán (tài liệu không phải `.rs`, không phải mã — đúng
phạm vi VD-137/VD-139); Agent 2 không phải sửa gì.

### D5 — Q-D / Windows: khuyến nghị **giữ "chỉ đo"** + cho phép **2 sửa một dòng** ở hạ tầng cổng

| Lựa chọn | Nội dung | Đánh giá |
|---|---|---|
| **(a) khuyến nghị** | Giữ VD-121/VD-136 ("Windows: chưa hỗ trợ, chỉ đo"). Cho phép (W-2) `encoding="utf-8"` 1 dòng và (W-3) sửa tối thiểu `sandbox.rs` để clippy không đỏ **chỉ trên Windows**; **không** đổi sản phẩm, **không** cfg-bỏ ca. (W-1) tuỳ chọn: cho `rustup target add` trên `psvn` nếu muốn 18/18 | Làm cổng Windows "chạy được để đo" mà không mở phạm vi hỗ trợ |
| (b) | Giữ nguyên tuyệt đối, mọi thứ về Windows để VS3 | Gate Windows tiếp tục đỏ ở clippy ⇒ số đo mãi không đầy đủ |
| (c) | Mở VS3 cho Windows ngay | Cần thiết kế riêng (đường dẫn `\`, `/bin/*`, CRLF, tiến trình) — không phải việc 3 h |

**Văn bản VD mẫu (nếu (a))**:

```
### VD-147 · Windows trong VS2: giữ "chỉ đo" + 3 việc hạ tầng cổng (2026-11-16)
(1) Giữ VD-121/VD-136: Windows chỉ ĐO, không tuyên bố hỗ trợ, không sửa sản phẩm cho Windows trong VS2.
(2) Cho phép hai sửa CHỈ-TEST, mỗi chỗ tối thiểu: `viscript/test/doc-examples/doc_examples.py` đọc ống bằng
    UTF-8 (`encoding="utf-8"`), và `crates/vis-cli/tests/sandbox.rs` xử lý biến không dùng khi cả hai nhánh
    `cfg(target_os)` bị loại. Không đổi kỳ vọng, không xoá ca, không `#[ignore]`, không `cfg`-bỏ ca.
(3) Cho phép cài THÀNH PHẦN TOOLCHAIN trên `psvn` bằng `rustup target add wasm32-unknown-unknown`
    (+ `x86_64-unknown-linux-gnu` nếu cần) theo VD-136/VD-140; không cài phần mềm hệ thống, không reboot.
Các nhóm ca `/bin/*`, escape `\`, CRLF, tiến trình trên Windows thuộc VS3.
```

### D6 — ST-133 (R23): duyệt như **báo cáo đợt 23**, không chốt VS2

Đã có `ĐÍNH CHÍNH 2026-11-16 Agent 0` (ST-132b không phải "Agent 2 tự thêm"; nợ #126 đóng) trong QUEUE, và
R24 §1 đính chính 8 điểm. **Đề nghị**: dòng `APPROVED` ở D1 (#7) là đủ; **không** mở lại R23 (VD-142).

### D7 — ST-138 (R24): duyệt **sau** D1 và **sau một lần `gate --fast` tại HEAD**

Lý do (F3): cổng hiện có chạy **trước** khi R24 tồn tại (log soak: "23 báo cáo"). Bước báo cáo tại HEAD thì
tôi đã kiểm và **PASS (24 báo cáo, 156 hàng)**; còn `fmt/clippy/L0` tại HEAD chưa có lần chạy nào **sau** R24.
**Đề nghị**: Agent 2 chạy `./vis-dev gate --fast` trên `a9f80d83` (hoặc HEAD lúc đó), dán dòng cuối + số
`L0 …/0` vào dòng `> CHỜ DUYỆT` của ST-138, rồi anh ký:

```
APPROVED 2026-11-16 Agent 0 · theo QUYET-DINH-A0-2026-11-16f §4.7 · Agent 1 đối chiếu 6 hàng phụ thuộc (đều APPROVED sau D1) + 8 hạng mục theo VD-131 + gate --fast trên HEAD sau R24: PASS <L0 x/0>
DONE Agent 0 2026-11-16 · ST-138: R24 là báo cáo chốt VS2 (kỹ thuật) theo VD-131/VD-143
```

Khi đó Agent 2 lật ST-138 `READY → DONE`, và **VS2 đóng về kỹ thuật** với 5 câu bắt buộc + trạng thái 8 hạng
mục (4 đóng / 3 chưa có → VS3 / 1 một phần).

### D8 — Đợt 25: 5 việc đề nghị (mỗi việc ≤ 3 h)

| # | Việc | Ngân sách | Vì sao |
|---|---|---|---|
| 1 | **Dọn log soak** (F1): `VIS_SOAK_LOG`/`--log`, tách `st138-soak.log`, khôi phục `st113-soak.log` từ git; sửa câu cứng ở `bin/vis-host-suite` (F4) | 1 h | Bằng chứng lịch sử, tái lập được |
| 2 | **Số Linux đầy đủ**: `vis test` 447 (interp + `--vm`), differential 100k, cache — trên `ps` (nợ #130) | 1,5 h | Hiện Linux thiếu 3 bộ số so với macOS |
| 3 | **D2 (a)**: sửa ca coreutils theo VD-144, chạy lại L0 trên `ps` cho **0 đỏ** | 1,5 h | Đóng ca đỏ cuối trên Linux |
| 4 | **D4**: dán văn bản `HOST-BAO-DAM.md` + (nếu D3 (a)) ghi giới hạn macOS | 1 h | Tài liệu cam kết phải khớp mã |
| 5 | **R25 + sổ nợ duy nhất** (F5, F6): một số một nghĩa, mỗi nền một dòng hash, `report-check` PASS (25 báo cáo) | 1,5 h | Bàn giao sạch |

Việc **không** làm trong đợt 25 (thuộc VS3, cần tài liệu thiết kế của anh): async song song + wait-set;
rendezvous/select/timeout; TLS/HTTP2/keep-alive; Landlock mở rộng; Windows; resolver qua mạng/chữ ký;
`openat`/`O_NOFOLLOW` cho đường ghi; macOS fd→path.

### D9 — Quy trình: chốt lại 3 điểm nhỏ

1. **Ai chạy bước kiểm tài liệu** (F7): Agent 1 được chạy `check_report.py`/`doc_examples.py --gate` (chỉ đọc)
   hay không.
2. **Dòng `APPROVED` phải ghi người đối chiếu** (VD-137 đã nói; đợt này các dòng mới nên ghi "Agent 1 đối
   chiếu" hoặc "Agent 0 đối chiếu" cho đúng).
3. **Sổ nợ duy nhất** (F5) — nếu đồng ý, Agent 2 làm ở việc #5 (D8).

---

## §6. Mẫu trả lời ngắn của anh (dán vào `QUYET-DINH-A0-2026-11-16f.md`)

```
# QUYẾT ĐỊNH AGENT 0 · đợt 24 (2026-11-16)
## §4.1 Duyệt 8 hàng: ĐỒNG Ý (dùng 7 dòng APPROVED như REVIEW §5-D1, hoặc một dòng gộp)
## §4.2 Nợ #139: chọn (a) / (b) / (c)  — nếu (a) thì ban hành VD-144
## §4.3 Nợ #137: chọn (a) / (b) / (c)  — nếu (a) thì ban hành VD-145
## §4.4 HOST-BAO-DAM: ĐỒNG Ý nguyên văn / sửa: <câu>
## §4.5 Windows: chọn (a) / (b) / (c)  — nếu (a) thì ban hành VD-147; W-1: cho phép / không
## §4.6 R23: duyệt như báo cáo đợt 23, không chốt VS2: ĐỒNG Ý
## §4.7 R24: duyệt SAU khi có gate --fast tại HEAD: ĐỒNG Ý / duyệt ngay kèm ghi chú
## §4.8 Đợt 25: chạy 5 việc như bảng / cắt bớt: <số>
## §4.9 Quy trình: Agent 1 được chạy bước kiểm tài liệu chỉ-đọc: CÓ / KHÔNG; sổ nợ duy nhất: CÓ / KHÔNG
```

Chỉ cần **9 dòng** là Agent 2 chạy tiếp được ngay, không hỏi lại.

---

## §7. Phụ lục

### 7.1. Chỉ mục bằng chứng (đường dẫn trong repo, để anh đối chiếu khi cần)

| Việc | Bằng chứng chính |
|---|---|
| Trạng thái + việc kế | `viscript/agent/STATE.md` (header, 35–45 dòng đầu) |
| Sổ task + dòng `CHỜ DUYỆT` | `viscript/agent/QUEUE.md` dòng ~255–281 |
| Quyết định | `viscript/agent/DECISIONS.md` (VD-131..VD-143 mới nhất) |
| Báo cáo chốt | `viscript/agent/reports/R24-viscript.md` + `.json` |
| Kế hoạch đợt đã chốt | `KE-HOACH-CHOT-23f.md` (gốc repo) |
| ST-130b | `viscript/crates/vis-cli/tests/siet_caps_pkg.rs` (8 `#[test]`) |
| ST-132/132b | `vis-host-linux/tests/landlock.rs` (13 `#[test]`, ca mới dòng 258); `viscript/agent/JOURNAL/st132b-read-dir.md` |
| ST-134 | `viscript/infra/linux/setup.sh`; `viscript/agent/JOURNAL/st134-runner-ca-nen.md` (§2b hai ca, §3 sửa 68 dòng, §4 sáu ca đỏ Linux) |
| ST-135 | `viscript/agent/JOURNAL/st135-linux-lai.md` |
| ST-136 | `viscript/agent/JOURNAL/st136-env.log` + `st136-windows.md` (45 KB, bảng 102 ca) |
| ST-137 | `viscript/agent/JOURNAL/st137-phantich.md` (cửa sổ thật), `st137-red-ps.log`, `st137-sau-ps.log`, `st137-sau-macos.log`, `st137-host-suite-*.log`, `st137-diff-100k.log` |
| Soak đợt 24 | `viscript/agent/JOURNAL/st113-soak.log` (**tên tệp sai** — F1) |
| Cam kết host/quyền | `viscript/agent/HOST-BAO-DAM.md` (mục FsGuard, dòng ~99–105) |
| Máy kiểm báo cáo | `viscript/test/report/check_report.py` |

### 7.2. Lệnh tái lập (cho Agent 2 hoặc người điều phối, **không** cho Agent 0)

```bash
cd /Users/mps/dvchd/ViFastOs
git log -1 --oneline                 # a9f80d83
git status --short -- viscript       # rỗng
python3 viscript/test/report/check_report.py --all   # PASS (24 báo cáo, 156 hàng)
cd viscript && ../vis-dev gate --fast                # cổng đầy đủ (Agent 2 chạy, không phải Agent 1)
grep -c '#\[test\]' crates/vis-cli/tests/siet_caps_pkg.rs                    # 8
grep -c '#\[test\]' ../vis-host-linux/tests/landlock.rs                      # 13
git show e47cfb2f --stat                                                      # ST-137: 11 tệp, không Cargo.toml
```

### 7.3. Thuật ngữ

| Từ | Nghĩa |
|---|---|
| **Hàng / CHỜ DUYỆT** | một dòng task trong `QUEUE.md`; dòng `> CHỜ DUYỆT` ngay dưới = tuyên bố "đã xong, chờ duyệt" |
| **VD-###** | quyết định của Agent 0 (nguồn sự thật về luật/quy trình) |
| **D2-###** | quyết định dòng OS |
| **Red-first / đỏ-trước** | bắt buộc có test/log đỏ **trước** khi sửa, để chứng minh test có tác dụng |
| **T-INFRA** | flake do hạ tầng; được chạy lại **1 lần** để phân loại, **không** để lấy xanh |
| **L0** | bộ test nhanh trong `gate --fast` (129 suite) |
| **landlock** | LSM của Linux dùng cưỡng chế quyền hệ tệp ở tầng kernel |
| **TOCTOU** | lỗi "kiểm-rồi-dùng": kẻ tấn công đổi đường dẫn giữa lúc kiểm và lúc dùng |
| **fd→path** | lấy đường dẫn thật của một handle đã mở (Linux: `/proc/self/fd/<fd>`) |
| **multicall** | một binary đóng nhiều lệnh, chọn lệnh theo `argv[0]` (uutils coreutils) |
| **soak** | chạy lặp 10 vòng tuần tự (gate/diff/host/cache) để tìm flake |

### 7.4. Giới hạn của báo cáo này (nói thẳng)

1. Tôi **không** chạy `cargo test`, `clippy`, L0, gate, soak — Agent 1 bị VD-137 cấm chạy cổng. Mọi số
   `cargo test/clippy/L0` trong tài liệu này là **số Agent 2 đo**, tôi **đọc** từ log + báo cáo, không tự sinh.
2. Tôi **đã chạy** `check_report.py --all` (chỉ đọc) — xem F7; kết quả PASS là số **của tôi**.
3. Số Windows là **số đo**, không phải tuyên bố hỗ trợ (VD-121).
4. Tôi không kiểm được máy `ps`/`psvn` (không có SSH trong phiên này); mọi số Linux/Windows là **đọc log**.
5. Ba tệp `TRA-LOI-AGENT1-CHO-AGENT0-2026-11-16{,d,e}.md` và `BAN-GIAO-ViScript-CHO-AGENT0.md` ở gốc repo là
   báo cáo cũ của tôi; tài liệu **này** thay thế chúng cho đợt 23f/24.
