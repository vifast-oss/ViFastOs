<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# BÁO CÁO ĐÊM — AGENT 1 → **AGENT 0** · đêm 17/11/2026 (02:40 → 09:05 JST)

**Từ**: Agent 1 (đêm nay **đóng vai Agent 0** theo yêu cầu người điều phối: ban hành quyết định 17, 17b, 17c, 17d)
**Gửi**: Agent 0 (người duyệt) — bản này để anh đọc lúc **10:00** khi online
**HEAD cuối**: `cc88ca23` (`[ST-159] R28`) — đã push; `viscript/` sạch
**Tài liệu tự chứa**: mọi số kèm log/hash trong repo. Tôi **không** chạy `cargo`/cổng/soak; số là của Agent 2, tôi **đọc log + đối chiếu sha256 + tự chạy `check_report.py`**.

> **Một câu.** Đêm nay chạy **12 hàng** (10 việc + 3 báo cáo `R26/R27/R28`), **0 BLOCKED**:
> VS2 đóng **8/8** (VD-151); Windows **self-test 12/12**, cổng **tới L0** (9/10 bước);
> đóng **6 nợ** (#136/#133/#135/#144/#110/#129 + ca 7 ST-147), cấp **#145**, chuyển **#143 → VS3**;
> có **bộ đầu vào thiết kế VS3** (222 dòng, 57 dẫn chứng). Sổ **146 = 89 mở / 34 đóng / 21 VS3**;
> `check_report` **PASS (28 báo cáo, 177 hàng)**; L0 macOS **1179/0**.
> **Anh cần**: duyệt 13 dòng `APPROVED` (§3.1) và **trả lời 5 câu hỏi VS3** (§3.2) để tôi soạn 6 quyết định thiết kế VS3.

---

## §1. Mười ba hàng đã chạy (tất cả `DONE`)

| # | Hàng | Commit | Kết quả chính |
|---|---|---|---|
| 1 | **ST-148** — sổ nợ theo VD-149 | `cfce6b86`, `ac514fe7` | `#142/#143` thêm + ánh xạ `#113b/#114b`; nguồn cho #89/#130/#139; trùng số 0 |
| 2 | **ST-147** — Windows: UTF-8 mọi lần gọi python (VD-150) | `fd16d7d1`, `c2692ae2` | Windows **7/12 → 11/12**; macOS gate PASS 10/10 L0 1159/0; ca 7 tách sang ST-153 |
| 3 | **ST-149** — nợ #136 (biến trùng tên lệnh trong `cmd`) | `6ce972ef`, `8abb0c81` | Đỏ-trước: in bảng `ls` thay vì `3`; `khai.rs` **+37/−6**; 4 ca; **#136 đóng** |
| 4 | **ST-150** — nợ #133 (ca âm GHI) | `9eeb936b`, `fdf69023` | Con ghi thư mục chỉ-đọc ⇒ **EACCES**; đối chứng ghi được; `ps` **14/14**; **#133 đóng** (vế 1) |
| 5 | **ST-153** — VD-152 (`check.rs` tên gốc `\`) | `4a805115`, `1263f7ea`, `32b79d4a` | `ten_goc` + 3 ca; ca 7 ST-147 đóng; Linux **1169/0**; psvn **12/12**, cổng **9/10 bước → L0** |
| 6 | **ST-151** — nợ #135 (sync `vis-linux`) | `9bd7d022`, `b1c07802` | `SYNC_DIRS`/`SYNC_FILES` + `--check-sync` + 9 ca; **#135 đóng**; L0 **1175/0** |
| 7 | **ST-152** — `R26` | `a9888091` | 26 báo cáo/171 task; `head=b1c07802`; `blocked []` |
| 8 | **ST-154** — nợ #144 + việc sổ | `d203b664`, `704ea55f`, `bca7370e` | `read_dir` thư mục có quyền ghi; `ps` **15/15**; **#143→VS3**, **#145**, **#144 đóng** |
| 9 | **ST-155** — điều tra VS3 (chỉ đọc) | `7970432e` | `st155-vs3-dieu-tra.md` **222 dòng**, 6 mục, **57 dẫn chứng**, 19–28 task ≈ 57–84 h, **Top 5 câu hỏi** |
| 10 | **ST-156** — `R27` | `c9249b79` | 27 báo cáo/174 task; 6 điểm đính chính |
| 11 | **ST-157** — nợ #110 (parity argv) | `567350e1`, `4bdede6c` | VM `()` → **`[a, b]`**; cùng byte với interp; `vm.rs` **+40/−6**; **#110 đóng**; L0 **1179/0** |
| 12 | **ST-158** — nợ #129 (một nguồn khối) | `8036b0a2`, `fdb21d15` | `test/doc_excluded.rs` (7 số) + 4 tệp `include!`; trước/sau **13/13, 13/13, 3/3, 10/10**; **#129 đóng** |
| 13 | **ST-159** — `R28` | `cc88ca23` | 28 báo cáo/**177** task; `head=fdb21d15`; `blocked []`; gate **L0 1179/0** |

**Commit quyết định (Agent 0 → Agent 1 dán)**: `b855caa3`, `456e8a4a`, `93e1d418`, `b73650fb`, `dd088322`, `1f522686`.
**Sổ**: `QUEUE` **177 hàng, tất cả `DONE`**; không còn `READY/BLOCKED/REVIEW`.

---

## §2. Kết quả then chốt

1. **VS2 đóng kỹ thuật 8/8** (VD-151); 3 câu giới hạn giữ nguyên; Linux Landlock **15 ca**; Windows "chỉ đo", wasm "chỉ check".
2. **Windows tiến lớn**: self-test **7/12 → 12/12**; cổng **bước 5 → 9/10 bước**, dừng **L0** (`cap_proc` 150/4 vì `/bin/*`) ⇒ phần còn lại thuộc **VS3 mục 5**.
3. **Parity interp↔VM** (#110) đóng: `main(args: list[str])` chạy `--vm` nay ra **cùng byte** với interp — ca đỏ-trước 3/4 đỏ (`()` vs `[a, b]`).
4. **Nợ**: **146 mục = 89 mở / 34 đóng / 21 VS3 / 1 không làm / 1 điều hướng** (0 trùng, dải 1..145). Đóng đêm nay: **#136, #133, #135, #144, #110, #129** (+ ca 7 ST-147). Thêm: **#142/#143** (VD-149), **#145** (VD-154c). Chuyển: **#143 → VS3**.
5. **Đính chính của tôi**: hiến chương VS3 ghi `#142` cho nợ đường GHI — **sai**, đúng là **`#140/#109b`** (VD-154a); sổ `144/91/20` của R26 đúng **tại head của nó**, hiện **146/89/34/21** (VD-155). R27/R28 đã ghi.
6. **Không nới ngưỡng**: `R1 max 1498`, `FUNCS 52`, `all_codes 33`, `VBC1 53`, doc-examples 24 khối — không đổi. L0 tăng **chỉ vì thêm ca** (1159→1163→1166→1175→1179).

---

## §3. Việc cần anh quyết

### §3.1 — **Duyệt 13 hàng + 3 báo cáo** (soạn sẵn, dán ngay dưới từng hàng trong `QUEUE.md`)
```
APPROVED 2026-11-17 Agent 0 · 17 §6 · Agent 1 đối chiếu: SO-NO #142/#143 thêm, ánh xạ #113b/#114b, nguồn #89/#130/#139; trùng số 0; check_report PASS
APPROVED 2026-11-17 Agent 0 · 17b §1 · ST-147 DONE theo phạm vi VD-150 (Windows 7/12→11/12; 6 ca encoding xanh; macOS gate PASS 10/10 L0 1159/0); ca 7 chuyển ST-153
APPROVED 2026-11-17 Agent 0 · 17 §6 · đỏ-trước in bảng ls thay vì 3; khai.rs +37/−6 (chỉ biến che tên lệnh; cmd/fn/type giữ nguyên); 4 ca; #136 đóng; gate mac PASS L0 1163/0
APPROVED 2026-11-17 Agent 0 · 17 §6 · ca 14 (helper ghi thư mục chỉ-đọc ⇒ EACCES, tệp không tạo; đối chứng dương ghi được); ps 14/14; #133 đóng
APPROVED 2026-11-17 Agent 0 · 17b §3 · ten_goc tách cả \ và / (3 ca unit); ca 7 ST-147 đóng; mac 1166/0, Linux gate PASS 1169/0; psvn self-test 12/12, cổng qua 9/10 bước dừng L0
APPROVED 2026-11-17 Agent 0 · 17 §6 · SYNC_DIRS/SYNC_FILES một nguồn + --check-sync (0/2) + 9 ca (bỏ 1 mục ⇒ 4 ca đỏ); #135 đóng; gate mac PASS L0 1175/0
APPROVED 2026-11-17 Agent 0 · 17 §6 · R26 26 báo cáo/171 task/11 khoá/head b1c07802/blocked []; 5 câu VD-131 đủ; 8 hạng mục; check_report PASS; gate mac PASS L0 1175/0
APPROVED 2026-11-17 Agent 0 · 17c §5 · ca 15 (read_dir thư mục có quyền ghi vẫn liệt kê; ngoài phạm vi EACCES); ps 15/15; #144 đóng, #143→VS3, #145 thêm; sổ 146
APPROVED 2026-11-17 Agent 0 · 17c §5 · ST-155 chỉ đọc (222 dòng, 57 dẫn chứng, 0 dòng mã); gate mac PASS L0 1175/0
APPROVED 2026-11-17 Agent 0 · VD-155 · R27 27 báo cáo/174 task/11 khoá/head dd088322/blocked []; 6 điểm đính chính; check_report PASS; gate mac PASS L0 1175/0
APPROVED 2026-11-17 Agent 0 · 17d §1 (VD-156) · đỏ-trước vm "()" vs interp "[a, b]"; vm.rs +40/−6; 4 ca cùng byte; #110 đóng; L0 1179/0; ca cap_khong_lo đổi kỳ vọng là BẮT BUỘC (ghim sự lệch cũ)
APPROVED 2026-11-17 Agent 0 · 17d §1 (VD-156) · test/doc_excluded.rs 7 số một nguồn + 4 include!; trước/sau 13/13,13/13,3/3,10/10; #129 đóng; L0 1179/0 không đổi
APPROVED 2026-11-17 Agent 0 · VD-156 · R28 28 báo cáo/177 task/11 khoá/head fdb21d15/blocked []; 4 đính chính; check_report PASS; gate mac PASS L0 1179/0
```

### §3.2 — **VS3: trả lời 5 câu hỏi** (chặn cả 6 mục; đọc `JOURNAL/st155-vs3-dieu-tra.md` §8)
1. **Mô hình bộ nhớ/GC**: heap riêng mỗi luồng + bộ đếm không atomic (đúng thiết kế §7.4), hay `Arc`/`Mutex` toàn cục? — quyết định `Send`/`Sync` của `Value`/`Host`/`Task`/`Chan`.
2. **Chính sách crate**: mở ngoại lệ giấy phép (webpki-roots MPL/CDLA, ring OpenSSL) và **ngoại lệ `unsafe`** (D2-17) cho TLS/`openat`? "Không" ⇒ mục 1/3/4 đổi thiết kế.
3. **Windows "hỗ trợ thật"** nghĩa là gì (đóng gói POSIX / shim native / giữ Git Bash) và có được **sửa kỳ vọng** 15 ca host + ~100 ca test theo nền (mở rộng VD-132)?
4. **`select` kênh**: cú pháp mới hay hàm std? (tên `select` đã là lệnh chiếu cột — `builtin.rs:333-337`).
5. **Nợ đường GHI**: xác nhận **`#140/#109b`** (không phải `#142` — VD-154a) + ma trận phụ thuộc (1 → 2/3/4; 3 → 6).

Sau khi anh trả lời, tôi soạn **6 tài liệu quyết định VS3 riêng** (mục tiêu đo được + API + ca đỏ-trước + ngân sách ≤3 h/task + cổng). **Trước đó không mở hàng VS3** (VD-148).

### §3.3 — **Windows thuộc VS3** (xác nhận)
Cổng Windows dừng **L0** (`cap_proc` 150/4), `cargo test` **1053/100**, host **39/54** — do thiếu môi trường POSIX. Đề nghị giữ "chỉ đo", xử ở VS3 mục 5; **không** sửa kỳ vọng ca. Lưu ý: **không có CI** trong repo ⇒ mọi số Windows phụ thuộc SSH `psvn` (R28 §8 đã ghi).

### §3.4 — **Chốt `#145`, `#142`, `#140`**
`#145` (chú thích `rt.rs` nói sai đường chặn `chan(0)`) **mở**, xử khi VS3 kênh; `#142` (`pkg update` ghim) **mở** (VS3 gói); đường GHI = **`#140/#109b`** (VS3).

### §3.5 — **Đợt sau nếu chưa mở VS3**: chỉ làm nợ nhỏ không cần thiết kế — **#138** (`/proc`), **#132** (crt-static); giữ **#140/#142/#145** cho VS3.

---

## §4. BLOCKED

**Không còn hàng nào `BLOCKED`** (177/177 `DONE`). Hai giới hạn có văn bản: Windows ~100 ca + `/bin/*` (VS3), `#145` (nợ mới, chờ VS3 kênh).

---

## §5. Sự cố trong đêm (khai đủ)

| # | Sự cố | Loại | Xử lý |
|---|---|---|---|
| 1 | ST-153 gate mac **lần 1 đỏ** ca **đua** `dua_doi_symlink_lap_co_gioi_han` (**1/50 000**) | **T-INFRA** | Chạy riêng **5/5 xanh**, chạy lại cổng **một** lần; log đỏ giữ nguyên |
| 2 | ST-151 gate mac **lần 1 đỏ** ca **thời gian** `hieu_nang_nap_module_5_tep` (**116 ms ≥ 100 ms**, tải 7.39) | **T-INFRA** | Như trên; **không** nới ngưỡng |
| 3 | ST-154 gate **lần 1 đỏ `fmt`** (rustfmt đòi xuống dòng một `assert!`) | lỗi định dạng thật | Sửa khoảng trắng, chạy lại lần 2 PASS |
| 4 | ST-159 **lần chạy cổng đầu** hỏng do **script bọc** (PATH thiếu `~/.cargo/bin`, đường log tương đối) | lỗi của Agent 2 | Cổng **chưa chạy**; sửa script bọc, lần 2 là cổng thật; log lần 1 giữ nguyên |
| 5 | `psvn`: `echo EXIT=$?` bị **PowerShell** nội suy ⇒ mất mã thoát | thao tác | Suy từ `--- L0 test: FAIL`; ghi rõ |
| 6 | `psvn`: log untracked **chặn `git pull`** | thao tác | Chuyển sang `C:\Users\ps\backup-20261117\` (không xoá) rồi pull |
| 7 | `ps`: log L0 **xen dòng** ⇒ đếm thô sai | đo | Đếm theo `<n> passed` + `cargo test --list` ⇒ **1169** |
| 8 | ST-154 **tự-trích hash vòng tròn** (SO-NO trích sha log, log băm SO-NO) | quy trình | Bỏ tự-trích; hash log ghi ở QUEUE/JOURNAL |
| 9 | **1 ca cũ đổi kỳ vọng** (`cap_khong_lo::doi_so_dong_lenh_la_str`) | **chủ đích** (VD-156) | Ca đó từ ST-124 **ghim chính sự lệch #110**; đổi để khẳng định parity, giữ bất biến ST-124 |
| 10 | Soak macOS **1352 s** ở HEAD cũ `3079dede` | truy vết | Giữ caveat, không tuyên bố mới |

---

## §6. Đối chiếu của tôi (Agent 1)

Tôi tự kiểm **từng hàng**: diff (`khai.rs` +37/−6; `check.rs` +30/−1; `vis-linux` +44/−10; `landlock.rs` +137; `vm.rs` +40/−6; `doc_excluded.rs` mới + 4 `include!`), log (self-test 11/12 → **12/12**, `ps` **14/14 → 15/15**, gate mac 10/10, L0 1163/1166/1175/**1179**, `ps` 1169), sha256 hai phía, sổ (146 dòng, 0 trùng) và **tự chạy** `check_report.py --all` ⇒ **PASS (28 báo cáo, 177 hàng)**. Không thấy chỗ nào nới ngưỡng, `#[ignore]`, `unsafe`, crate mới hay log ghi đè.

---

## §7. Số đo (dùng cho tệp duyệt đợt 29)

| Phép đo | macOS | Linux `ps` | Windows `psvn` (chỉ đo) |
|---|---|---|---|
| `gate --fast` | **PASS** 10/10, L0 **1179/0** (131 suite) | **PASS** 10/10, L0 **1169/0** (129 suite, **trước** +13 ca ST-151/157) | **9/10 bước**, dừng **L0** (`cap_proc` 150/4) |
| `cargo test` | **1179/0** | 1169/0 (chưa đo lại) | **1053/100** @ `3dc13342` |
| host suite / Landlock | 54/54 | **54/54** · `vis-host-linux` **15/15** | **39/54** (15 ca `/bin/*`) |
| targets (check) | 18/18 | 18/18 | **18/18** |
| `vis test` / diff / cache | 447/447 ×2 · lech=0 · 0 lệch @`ee58a518` | như macOS (40,9 s / 6,33 s) | — |
| soak 10 vòng | 10/10 = 1352 s (**HEAD cũ** `3079dede`) | **10/10 = 250 s** @`2866f8c2` | — |
| TOCTOU đường đọc | 0/50 000 | 0/50 000 | — |

**Khoá khác**: R1 max **1498** · `FUNCS` 52 · `all_codes` 33 · `VBC1` 53 · doc-examples 24 khối.
**Báo cáo**: `R26` (171 task) · `R27` (174) · `R28` (177) · `check_report` **PASS (28/177)**.

---

## §8. Đề nghị đợt 29 (sau 10:00)

1. Anh ký **§3.1** ⇒ đêm 17/11 khép hoàn toàn.
2. Anh trả lời **5 câu hỏi VS3** ⇒ tôi soạn **6 quyết định thiết kế VS3**; sau đó mở hàng VS3 đầu tiên (đề nghị **mục 1 async song song**, tách 3–4 task ≤3 h).
3. Nếu chưa mở VS3: **#138** (`/proc`) + **#132** (crt-static) — nhỏ, không cần thiết kế; **#140/#142/#145** để VS3.
4. Windows: giữ "chỉ đo"; cân nhắc **runner CI** trước khi hứa "hỗ trợ thật".

---

## §9. Phụ lục — nơi tra

| Việc | Đường dẫn |
|---|---|
| Báo cáo chốt | `viscript/agent/reports/R26-viscript.*`, `R27-*`, `R28-*` |
| Quyết định đêm | `viscript/agent/JOURNAL/quyet-dinh-a0-2026-11-17{,b,c,d}.md` |
| Điều tra VS3 | `viscript/agent/JOURNAL/st155-vs3-dieu-tra.md` |
| Sổ nợ | `viscript/agent/SO-NO.md` (146 mục) |
| Sổ task | `viscript/agent/QUEUE.md` (177 hàng, tất cả DONE) |
| Nhật ký | `viscript/agent/JOURNAL/2026-11-17.md` |
| Log chính | `st147-*` … `st159-*` |

**Giới hạn**: tôi không chạy `cargo`/cổng/soak; mọi số do Agent 2 đo, tôi đọc log + sha256 + tự chạy `check_report`.
