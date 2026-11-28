<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# BÀN GIAO AGENT 1 → AGENT 0 · Tài liệu VS3 + công thái học (17/11/2026)

**Mục đích**: một tệp duy nhất để Agent 0 duyệt. Mọi nội dung dưới đây **đã có trong repo** (`viscript/agent/JOURNAL/`), đều **chỉ đọc, 0 dòng mã** — chưa có thay đổi mã nào cho R1–R24.

## Cần Anh duyệt (để mở ST-171 tách repo)
| # | Tài liệu | Commit nguồn | Trạng thái |
|---|---|---|---|
| 1 | 6 tài liệu quyết định VS3: `vs3-qd-1…6.md` (Phần A dưới) | `d2dbb0cc` (ST-168) | **CHỜ DUYỆT** |
| 2 | Bảng phân loại 105 ca Windows theo suite (Phần B) | `33d8b52f` (ST-169) | **CHỜ DUYỆT** (A0 đã nhận qua ST-172a, chưa duyệt riêng) |
| 3 | `vs-ergo.md` — R1–R24 + bảng 2 pha + 3 script (Phần C) | `14bc4e0e`, `63eb6681` (ST-172e/f) | **CHỜ DUYỆT** |

**Sau khi duyệt**: theo VD-173/VD-175/VD-177, chỉ khi đủ 3 mục trên mới chạy **ST-171** (tách repo ViScript); nếu chưa duyệt thì **dừng ở ST-170 (R29)**, không tách repo.

## Trạng thái hiện tại (17/11 15:06 JST)
- **Đang chạy**: **ST-174** (vá nợ **#146** bằng `symlink_metadata` + điểm chèn `#[cfg(test)]`, chứng minh tất định 100/100, 6 triệu vòng, gate mac 3 lần) — bắt đầu 14:07, dừng mềm 17:10.
- **Đã xong gần nhất**: ST-173 (landlock chỉ Linux, `da3ecfb3`); ST-167 (soak: mac 1696 s **cận trên** vs ps 254 s = 6,68×, nợ **#147** cấp); ST-176/ST-175 mở.
- **Thứ tự còn lại**: ST-174 → ST-175 (cây ps + chặn cây bẩn) → ST-176 (#147, 2 h) → **ST-170 (R29)** → ST-171 (chỉ sau khi Anh duyệt đủ 3 mục trên).

---

# PHẦN A · 6 TÀI LIỆU QUYẾT ĐỊNH VS3



<!-- ===== vs3-qd-1.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-1 · Async **song song thật**
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Thay thế**: — · **Phụ thuộc**: không (gốc) · **Mở đường cho**: QĐ-2, QĐ-3, QĐ-4

## 1. Mục tiêu đo được
1. **Song song thật**: chương trình `N` task độc lập (mỗi task sleep/IO ảo) chạy trên `W` worker; đo **wall-clock** với `W = min(N, nproc)` và `W = 1`; **tỉ lệ ≥ 0,80 × min(nproc, N)** trên máy ≥ 4 core, ghi cả 2 số + `nproc` vào log (ngưỡng này Agent 0 chốt khi duyệt; chưa dùng để "đạt" trước khi có số).
2. **Tất định**: cùng một chương trình **không chia sẻ** chạy `W=1` và `W=4` cho **cùng stdout/exit** (byte-for-byte), ở interp **và** VM.
3. **Không `unsafe`**: `crates/vis-interp`, `crates/vis-vm`, `crates/vis-cli` giữ `#![forbid(unsafe_code)]`; chứng minh `Value: !Send` bằng **doctest `compile_fail`**.
4. **Không rò qua closure**: gửi closure/handle tệp/Task qua `Chan` liên luồng ⇒ lỗi **lúc biên dịch** nếu biết, nếu không ⇒ mã lỗi lúc chạy (đề xuất mã mới, xem §3).

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-interp/src/rt.rs:39` — `struct Task`; `:95` `tao`; `:112` `spawn`; `:176` `await_task` — executor **đơn luồng**, chạy lần lượt.
- `crates/vis-interp/src/interp.rs:221-234` — dựng `argv` và gọi `main` (parity đã sửa ở ST-157).
- 5 câu VD-131 hiện hành: “Async không song song.” — câu này **phải đổi** khi VS3-QĐ-1 xong (Agent 0 duyệt lại câu chữ).

## 3. Thiết kế chốt (theo VD-160)
- **Heap riêng mỗi worker**, bộ đếm tham chiếu **không atomic**; **loại** `Arc`/`Mutex` toàn cục.
- `Send`/`Sync`: `Value` **không/không** · `Task` **không/không** · `Chan` **có/có** · `Host` **có/có**.
- Qua `Chan` giữa 2 worker: **sao chép sâu** (`int/float/bool/str/bytes/list/dict/record` của các kiểu này); closure/handle/Task **không truyền được**.
- Chia sẻ `bytes`/`str` lớn bất biến qua `Arc`: **hoãn**, ghi nợ, chỉ làm khi có số đo.
- Mã lỗi mới (đề xuất, **Agent 0 chốt số**): `VS05xx` “giá trị không gửi được qua kênh giữa hai worker”.

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-1a | Khung scheduler `W` worker + `spawn` phân phối + tắt êm; **chưa** đổi ngữ nghĩa | `W=1` cho **cùng** kết quả hiện tại (toàn bộ L0 xanh) | — |
| VS3-1b | `suspend/resume` + `await_task` liên worker **bị chặn** (báo lỗi rõ) | ca: await task khác worker ⇒ lỗi `VS05xx`, không deadlock | 1a |
| VS3-1c | Wait-set: `await` chỉ trong worker sở hữu; kênh là đường duy nhất | ca đỏ-trước: `Value` chạm worker khác ⇒ không thể biên dịch/lỗi | 1b |
| VS3-1d | Ca song song thật + đo `W=1` vs `W=min(nproc,N)` | 2 số + `nproc` trong log; tỉ lệ ≥ ngưỡng Agent 0 chốt | 1c |
| VS3-1e | `compile_fail` doctest `Value: !Send`; differential `W=1` vs `W=4` | doctest chạy trong L0; differential 0 lệch | 1c |

## 5. Ca đỏ-trước (bắt buộc có trước khi sửa)
- `compile_fail`: hàm nhận `T: Send`, gọi với `Value` ⇒ không biên dịch.
- Gửi closure qua `Chan` ⇒ lỗi kiểu (nếu biết) hoặc lỗi lúc chạy; **không** treo.
- `W=1` vs `W=4`: cùng stdout (chương trình không chia sẻ).

## 6. Cổng
`gate --fast` (10/10, L0 0 đỏ) + `vis test` 447 + differential `W=1`/`W=4` + bảng đo song song (log riêng, không tính vào L0). **Không** nới ngưỡng L0.

## 7. Rủi ro
(1) Bộ nhớ/GC: heap riêng làm `Value` không `Send` — mọi API công khai phải rà lại; (2) tính tất định của differential; (3) `nproc` ảo trên CI; (4) macOS thread stack nhỏ hơn Linux.



<!-- ===== vs3-qd-2.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-2 · Kênh: rendezvous `chan(0)` + `recv_any` + #145
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: QĐ-1 · **Mở đường cho**: —

## 1. Mục tiêu đo được
1. `chan(0)` = **rendezvous**: `send` **chặn** cho tới khi có `recv` (đo: gửi rồi không nhận trong 50 ms ⇒ chưa trả về; nhận ⇒ cả hai trả về).
2. `recv_any(chans, timeout=None)` trả `(chi_so, gia_tri)`; hết giờ trả `none` (khớp quy ước kiểu trả về hiện có).
3. `chan(0)` phải bị chặn **lúc tyck** (exit 2) **hoặc** chú thích `rt.rs` phải sửa cho đúng thực tế (nợ **#145**); **không** để lệch tài liệu–mã.
4. Không thêm cú pháp mới; tên `select` **không** bị chiếm (lệnh chiếu cột).

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-interp/src/builtin.rs:772` — `$kenh(n)` tạo kênh đệm `n`; `:1232` `chan_method`.
- `crates/vis-interp/src/rt.rs:120` — `tao_kenh(host, cap, span)`.
- `rt.rs:128-129` chú thích nói literal `chan(0)` bị **tyck** chặn, nhưng `crates/vis-types/src/tyck/expr/kenh.rs:16-33` **không** kiểm `cap`; runtime chặn (exit 1) — **#145**.

## 3. Thiết kế chốt (theo VD-163)
- Hàm std **`recv_any(chans, timeout=None)`**, không cú pháp mới; **không** nhánh gửi ở VS3.
- `chan(0)`: rendezvous thật (hàng đợi người gửi/nhận, đánh thức đúng cặp).
- #145 xử **trong** QĐ-2: thêm kiểm `cap == 0` ở tyck **hoặc** sửa chú thích; ưu tiên **tyck** (lỗi sớm), mã lỗi giữ `VS0501` trừ khi Agent 0 chốt mã khác.

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-2a | Rendezvous `chan(0)` (hàng đợi + đánh thức) | ca gửi-không-nhận chặn; nhận ⇒ trả; không treo | QĐ-1 xong |
| VS3-2b | `recv_any` + timeout | 3 ca: chọn đúng kênh, timeout ⇒ `none`, nhiều kênh rỗng | 2a |
| VS3-2c | **#145**: tyck kiểm `cap` của `kenh(0)` | ca đỏ-trước `exit 2`, không phải runtime; sửa chú thích `rt.rs` | 2a |
| VS3-2d | Kiểm chéo interp/VM + tài liệu | `vis test` 447, doc-examples; mục kênh trong SPEC/thiết kế cập nhật (Agent 0 duyệt) | 2b, 2c |

## 5. Ca đỏ-trước
- `chan(0)`: gửi không ai nhận trong 50 ms ⇒ **chưa** trả (đo bằng cờ, không bằng ngủ dài).
- `recv_any` với 2 kênh, chọn đúng kênh có dữ liệu; timeout 50 ms ⇒ `none`.
- `kenh(0)` literal ⇒ `VS0501` lúc **biên dịch** (trước khi sửa: runtime exit 1).

## 6. Cổng
`gate --fast` + `vis test` 447 + ca đa nền (mac/Linux). Không đổi ngưỡng.

## 7. Rủi ro
Đánh thức sai thứ tự ⇒ bất định (test phải tất định); đói (starvation); tương tác với QĐ-1 (kênh là đường duy nhất qua worker).



<!-- ===== vs3-qd-3.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-3 · Net: TLS + HTTP/1.1 keep-alive
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: QĐ-1 (worker/IO) · **Mở đường cho**: QĐ-6

## 1. Mục tiêu đo được
1. `https://` chạy được: GET/POST tới **server thử trong fixtures** (chứng chỉ tự ký nhúng repo) ⇒ 200 + body đúng; **không** gọi mạng ngoài trong test.
2. Xác minh chứng chỉ: chứng chỉ tự ký **không** nằm trong kho hệ thống ⇒ **lỗi** rõ ràng (không im lặng bỏ qua).
3. Keep-alive: 2 request liên tiếp **cùng** kết nối (đếm accept ở server thử = 1).
4. Cổng có bước kiểm **licenses + advisories** (`cargo-deny`), không mạng ⇒ dùng cache.

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-cli/src/net.rs:9` — `TcpListener/TcpStream`; `:17` `struct …{ tay: Vec<(String, TcpStream)> }`; `:87` `accept_tren`.
- `crates/vis-cli/src/modules.rs:550` — module chuẩn `str/list/map/path/time/json/fs/net/http` (chưa TLS).
- 5 câu VD-131 hiện hành: “Net không TLS.” — **phải đổi** khi xong.

## 3. Bảng phụ thuộc (theo VD-161, Agent 1 **phải** xác minh lại bằng `cargo metadata` ở phiên bản ghim)
| Crate | Phiên bản ghim | Giấy phép (khai) | Lý do | unsafe bên trong |
|---|---|---|---|---|
| `rustls` | (chốt khi duyệt) | Apache-2.0/MIT/ISC (**kiểm lại**) | TLS thuần Rust | có (được phép, crate ngoài) |
| `rustls-platform-verifier` | " | Apache-2.0/MIT (**kiểm lại**) | kho chứng chỉ HĐH | có |
| `ring` | " | **Apache-2.0 AND ISC** (đính chính B2) | crypto provider | có |
| `rustls-webpki` | **≥ 0.103.16** | ISC | xác minh chuỗi | có |
| `webpki-roots` | chỉ **wasm** | **CDLA-Permissive-2.0** (chỉ dữ liệu chứng chỉ) | roots cho wasm | không |

Quy tắc: ngoài danh sách giấy phép cho phép (MIT/Apache-2.0/ISC/BSD-2/3/Unicode-3.0/Zlib; CDLA chỉ cho dữ liệu chứng chỉ) ⇒ **BLOCKED-CHO-A0**. Crate của ta vẫn `#![forbid(unsafe_code)]`.

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-3a | Client TLS + platform verifier + test server tự ký | test offline 200/body; cert lạ ⇒ lỗi rõ | QĐ-1 |
| VS3-3b | Keep-alive HTTP/1.1 | 2 request = 1 accept | 3a |
| VS3-3c | `cargo-deny` licenses+advisories trong cổng (cache offline) | cổng chạy không mạng; tài liệu cài trên mac/ps/psvn | 3a |
| VS3-3d | Sandbox `net` cap + tài liệu | cap thiếu ⇒ `VS0604`; doc cập nhật “Net có TLS” | 3b |

## 5. Ca đỏ-trước
- HTTPS tới server tự ký **trong fixtures** ⇒ 200 (trước: lỗi “không hỗ trợ TLS”).
- Cert tự ký **không** trong kho hệ thống ⇒ lỗi xác minh.
- 2 request ⇒ 1 accept.

## 6. Cổng
`gate --fast` + bước `cargo-deny` (licenses/advisories) + test offline (không mạng).

## 7. Rủi ro
Kho chứng chỉ HĐH khác nhau (mac/ps/psvn); proxy; wasm roots; `ring` build trên Windows (cần NASM? — phải kiểm).



<!-- ===== vs3-qd-4.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-4 · Quyền: đường GHI nguyên tử (hết TOCTOU)
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: QĐ-1 · **Mở đường cho**: —

## 1. Mục tiêu đo được
1. Trên Linux: ghi qua **fd đã mở** (`openat2`/`O_NOFOLLOW`), **0 rò** trong 500 000 vòng ca đua (như `#146`).
2. Trên macOS: **O_NOFOLLOW** + kiểm `dev/ino` sau mở; ghi rõ giới hạn còn lại trong `HOST-BAO-DAM.md`.
3. Windows: giữ "chưa hỗ trợ" (QĐ-5) — **không** hứa.
4. Không `unsafe` trong crate của ta (rustix API an toàn).

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-cli/src/quyen.rs:546` `struct FsGuard`; `:560` `read` (Linux dùng `/proc/self/fd`); `:568` `write` — **kiểm-rồi-mở** ⇒ nợ **#140/#109b**.
- `HOST-BAO-DAM.md` §4 ghi dòng giới hạn (Agent 0 sở hữu).
- macOS: VD-145 (không fd→path) — #137.

## 3. Thiết kế chốt (theo VD-161/VD-164)
- Linux: `rustix::fs::openat2` (RESOLVE_NO_SYMLINKS|RESOLVE_BENEATH) hoặc `openat` + `O_NOFOLLOW`; ghi qua `File` từ fd.
- macOS: `O_NOFOLLOW` + `fstat` so `dev/ino` với `lstat` trước; còn khe hở ghi rõ.
- `rustix` đã có? **chưa** trong `Cargo.toml` của ta ⇒ thêm mới phải qua bảng phụ thuộc (giấy phép MIT/Apache/ISC?) và **Agent 0 duyệt**.

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-4a | Linux: `openat2`/`openat`+`O_NOFOLLOW` cho `write` | ca đua ghi 500k ⇒ 0 rò; L0 xanh | QĐ-1 |
| VS3-4b | macOS: `O_NOFOLLOW` + `dev/ino`; ghi giới hạn | ca đua 500k ⇒ 0 rò (hoặc ghi rõ số) | 4a |
| VS3-4c | Ca đua ghi (đỏ-trước) + `HOST-BAO-DAM` cập nhật | ca đỏ-trước chứng minh lỗ trước sửa; doc khớp | 4a |
| VS3-4d | Rustix: bảng phụ thuộc + cổng licenses | `cargo-deny` xanh; không `unsafe` | 4a |

## 5. Ca đỏ-trước
Đổi symlink giữa kiểm và mở (như `quyen.rs:704` ca tất định) ⇒ phải **chặn**; ca đua 500k ⇒ 0 rò.

## 6. Cổng
`gate --fast` + ca đua 500k trên mac/ps + `cargo-deny`.

## 7. Rủi ro
Nhân kernel cũ (openat2 < 5.6) ⇒ fallback; macOS vẫn còn khe hở bản chất (không fd→path) — phải nói thẳng; Windows không hứa.
## 4b. Hướng dài hạn (VD-176(4)) — dùng chung cơ chế cho đường đọc và đường ghi
Mở **từng thành phần đường dẫn** bằng **`openat` + `O_NOFOLLOW`** qua **`rustix`** (Linux: `openat2` với `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS`), **bỏ hẳn kiểm theo đường dẫn** (không còn `canonicalize` rồi so). **Đường đọc và đường ghi dùng cùng một cơ chế**; đây là cùng hướng với `#140/#109b` (VS3). **Không** làm ở ST-174 (chỉ vá 1 dòng `symlink_metadata`).



<!-- ===== vs3-qd-5.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-5 · Windows "hỗ trợ thật"
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: **độc lập** (chạy song song được) · **Mở đường cho**: —

## 1. Mục tiêu đo được (theo VD-162)
1. Chạy **native**, không cần Git Bash/MSYS trong `PATH`.
2. Host suite **54/54** trên `psvn`.
3. `cargo test --workspace` **0 đỏ** — hoặc chỉ còn các ca thuộc **danh sách kỳ vọng theo nền** đã được Agent 0 duyệt (mỗi ca 1 dòng lý do).
4. Cổng Windows **10/10** và có **CI runner Windows** (self-hosted `psvn` hoặc runner dịch vụ) chạy tự động.
5. Trước khi đủ 4 điều trên: vẫn tuyên bố **“chỉ đo”**.

## 2. Hiện trạng (dẫn chứng)
- Baseline `0df923c2`: `cargo test` **1068 / 105 đỏ** (131 suite); self-test **12/12**; cổng dừng **L0** (`cap_proc` 150/4 fail-fast) — `JOURNAL/st161-*`.
- Host suite **39/54**; targets **18/18** — `JOURNAL/st144-*`.
- Phân loại 105 ca: **a=48 / b=15 / c=42** — `JOURNAL/st169-bang-phan-loai-windows.md`.

## 3. Kế hoạch (theo VD-162(2)(3))
- **Nhóm a (48)**: thay `/bin/*` + `bash`/`sh` bằng **helper repo-built** (một binary test có `echo/cat/grep/printf/true/false/sleep/sh -c`), dùng **cùng kỳ vọng** cả 3 nền.
- **Nhóm b (15)**: điều tra từng ca; sửa sản phẩm nếu là lỗi thật; nếu là khác biệt bản chất ⇒ chuyển sang **c** và ghi lý do.
- **Nhóm c (42)**: **trước hết** sửa test dùng `/`/đường dẫn tương đối (đề nghị ST-169 §4); nếu còn ⇒ đưa vào **danh sách kỳ vọng theo nền** (một tệp duy nhất) để Agent 0 duyệt.
- **CI**: runner Windows cho `gate --fast` + `vis-host-suite`; log ngoài repo (đã có `vis-remote`).


## 3b. Bảng tóm tắt theo suite (đầy đủ 105 dòng: `agent/JOURNAL/st169-bang-phan-loai-windows.md`)

| Suite | a | b | c | Tổng |
|---|---|---|---|---|
| `cap_proc` | 4 | 0 | 0 | 4 |
| `cap_proc_grant` | 6 | 0 | 0 | 6 |
| `chan_doan_da_tep` | 0 | 0 | 5 | 5 |
| `diag_cmp` | 0 | 0 | 1 | 1 |
| `extern` | 10 | 0 | 0 | 10 |
| `grant_proc_canon` | 5 | 0 | 0 | 5 |
| `json_ra` | 0 | 5 | 1 | 6 |
| `ls_open` | 0 | 0 | 1 | 1 |
| `lsp` | 0 | 5 | 1 | 6 |
| `lsp_da_tep` | 0 | 0 | 4 | 4 |
| `net` | 0 | 1 | 0 | 1 |
| `net_server` | 0 | 3 | 0 | 3 |
| `ngoai_vm` | 3 | 0 | 0 | 3 |
| `pipeline` | 14 | 0 | 0 | 14 |
| `quyen` | 0 | 0 | 8 | 8 |
| `sem` | 0 | 0 | 1 | 1 |
| `siet_caps_pkg` | 0 | 0 | 1 | 1 |
| `soak_script` | 1 | 0 | 0 | 1 |
| `span_tep` | 0 | 0 | 1 | 1 |
| `st83_no_nho` | 0 | 0 | 2 | 2 |
| `std_fs_json` | 0 | 0 | 7 | 7 |
| `strict_caps` | 0 | 0 | 2 | 2 |
| `sync_list` | 4 | 0 | 0 | 4 |
| `vis_test` | 0 | 0 | 3 | 3 |
| `vish_cmd_body` | 0 | 0 | 2 | 2 |
| `vish_main` | 1 | 0 | 0 | 1 |
| `vm_main_argv` | 0 | 0 | 1 | 1 |
| `vsh1b` | 0 | 0 | 1 | 1 |
| **Tổng** | **48** | **14** | **42** | **104** |

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-5a | Helper repo-built (`vis-posix` test binary) thay `/bin/*` | 48 ca (a) xanh trên cả 3 nền; không sửa kỳ vọng | — |
| VS3-5b | Sửa 42 ca (c) dùng `/` + golden tương đối | số ca (c) còn lại ≤ danh sách duyệt | 5a |
| VS3-5c | Điều tra 15 ca (b), phân loại lại | mỗi ca kết luận + sửa hoặc chuyển (c) | 5a |
| VS3-5d | CI runner Windows (psvn) chạy cổng + host | log tự động, `git pull` không chặn | 5a |
| VS3-5e | Tài liệu danh sách kỳ vọng theo nền + tuyên bố mức hỗ trợ | Agent 0 duyệt danh sách; R báo số | 5b, 5c |

## 5. Ca đỏ-trước
`vis-host-suite` trên `psvn` (39/54 → 54/54); `cargo test` 105 → 0 (hoặc = danh sách duyệt).

## 6. Cổng
Trên `psvn`: `gate --fast` 10/10, host 54/54, `cargo test` 0 đỏ; trên mac/ps: L0 **không đổi**.

## 7. Rủi ro
(1) Không có CI ⇒ kết quả phụ thuộc SSH; (2) “native” có thể kéo theo sửa lớn về tiến trình/tín hiệu; (3) danh sách kỳ vọng theo nền dễ bị lạm dụng ⇒ Agent 0 duyệt từng ca.



<!-- ===== vs3-qd-6.md ===== -->

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS3-QĐ-6 · Gói: registry mạng + chữ ký + GC
**Trạng thái**: CHỜ AGENT 0 DUYỆT (VD-148) · **Phụ thuộc**: QĐ-3 (TLS) · **Mở đường cho**: —

## 1. Mục tiêu đo được
1. `pkg add` tải gói từ **registry HTTPS** (server thử trong fixtures) và **xác minh chữ ký** (Ed25519) + sha nội dung.
2. Chữ ký sai/chứng chỉ sai ⇒ **từ chối**, không ghi vào `.vis/pkg/`.
3. `pkg gc` xoá gói không còn trong `vis.lock`, **không** xoá gói đang dùng; chạy 2 lần ⇒ lần 2 không đổi.
4. `vis.lock` ghi **đủ** (tên, phiên bản, sha256, khoá ký) — test round-trip offline.

## 2. Hiện trạng (dẫn chứng)
- `crates/vis-cli/src/pkg.rs` + `crates/vis-cli/src/pkg/resolver.rs`, `semver.rs`.
- `crates/vis-cli/src/modules.rs:5-6` — nạp gói từ `.vis/pkg/` theo `vis.lock`, kiểm **sha** lúc nạp.
- Nợ: **#142** (`pkg update` ghim mọi gói khác), **#143** (NamePolicy::Strict — VS3), **#125** (mạng/chữ ký VS3).

## 3. Bảng phụ thuộc (Agent 1 xác minh bằng `cargo metadata` khi ghim)
| Crate | Giấy phép (khai) | Lý do | Ghi chú |
|---|---|---|---|
| `ed25519-dalek` | BSD-3-Clause (**kiểm lại**) | chữ ký gói | có unsafe bên trong |
| `sha2` | MIT/Apache-2.0 (**kiểm lại**) | sha nội dung | — |
| (TLS từ QĐ-3) | — | tải registry | — |

## 4. Chia task ≤ 3 h
| Task | Nội dung | Điều kiện xong | Phụ thuộc |
|---|---|---|---|
| VS3-6a | Client registry HTTPS + server thử fixtures | tải gói + ghi `.vis/pkg/` offline | QĐ-3 |
| VS3-6b | Chữ ký Ed25519 + `vis.lock` mở rộng | chữ ký sai ⇒ từ chối; lock round-trip | 6a |
| VS3-6c | `pkg gc` + test 2 lần idempotent | ca: xoá đúng, không xoá gói đang dùng | 6a |
| VS3-6d | #142 (chính sách update) + tài liệu | Agent 0 chốt chính sách; doc cập nhật | 6b |

## 5. Ca đỏ-trước
Registry trả gói có chữ ký sai ⇒ `VS0701` (hoặc mã riêng) + **không** ghi; `gc` idempotent.

## 6. Cổng
`gate --fast` + test offline (server thử) + `cargo-deny` licenses/advisories.

## 7. Rủi ro
Khoá ký/rotating; registry giả; offline; supply-chain (phải có `cargo-deny` từ QĐ-3 trước).



---

# PHẦN B · BẢNG PHÂN LOẠI 105 CA WINDOWS (ST-169)

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# ST-169 · Bảng phân loại **105 ca đỏ** `cargo test --workspace` trên Windows (`psvn`)

**Nguồn**: `JOURNAL/st161-cargo-test-psvn-nff.log` @ `0df923c2` (131 suite, 1068 đạt / 105 đỏ).
**Vai trò**: đầu vào cho **ST-168 mục 5** (Windows "hỗ trợ thật", VD-162). **Chỉ đọc log + mã**, không sửa.
**Quy ước đường dẫn (VD-165 §2)**: `file:dòng` tính từ **gốc ViScript** (sau khi tách repo), tức không có tiền tố thư mục hiện tại.

## Cách phân loại (theo VD-162(3))

| Nhóm | Nghĩa | Cách xử lý dự kiến |
|---|---|---|
| **a** | Phụ thuộc chương trình POSIX `/bin/*` (hoặc `bash`, `sh`, `python3` ngoài repo) | Thay bằng **chương trình trợ giúp do repo tự build** (VD-162(2)); **cùng kỳ vọng trên cả 3 nền** |
| **b** | **Nghi lỗi thật** của ViScript trên Windows (hoặc hành vi cần quyết định) | Sửa sản phẩm; nếu là khác biệt bản chất ⇒ chuyển **c**; **cần Agent 0 chốt từng ca** |
| **c** | Khác biệt **bản chất nền** (dấu `\`, tên tệp đặc biệt, TAP/golden có đường dẫn tuyệt đối, POSIX bit, signal, symlink…) | Kỳ vọng **theo nền** trong **MỘT tệp danh sách duy nhất** có lý do từng ca; **Agent 0 duyệt danh sách** |

**Đếm**: **a = 48** · **b = 15** · **c = 42** (tổng **105**). Các ca **b** là **ứng viên**, cần Agent 0 xác nhận từng ca
(đúng theo VD-162(3): danh sách kỳ vọng theo nền phải được duyệt).

## Bảng (105 ca)

| # | Ca | Nhóm | `file:dòng` | Bằng chứng (rút từ log) |
|---|---|---|---|---|
| 1 | `cap_proc::khong_strict_khong_cap_van_chay` | **a** | `crates/vis-cli/tests/cap_proc.rs:98:5` | assertion `left == right` failed: interp stderr: c.vish:1:1: error VS0501: không chạy được |
| 2 | `cap_proc::strict_cap_proc_pipeline_hai_stage` | **a** | `crates/vis-cli/tests/cap_proc.rs:133:5` | assertion `left == right` failed: stderr: e.vish:4:1: error VS0501: không chạy được chương |
| 3 | `cap_proc::strict_cap_proc_sh_exit3` | **a** | `crates/vis-cli/tests/cap_proc.rs:115:5` | assertion `left == right` failed: stderr: d.vish:4:1: error VS0501: không chạy được chương |
| 4 | `cap_proc::strict_co_cap_proc_thi_chay_hai_duong` | **a** | `crates/vis-cli/tests/cap_proc.rs:85:5` | assertion `left == right` failed: interp stderr: main.vish:4:1: error VS0501: không chạy đ |
| 5 | `cap_proc_grant::grant_basename_echo_chay` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:105:5` | assertion `left == right` failed: (2, [], "vis run: `--grant p=echo`: không thấy lệnh `ech |
| 6 | `cap_proc_grant::grant_echo_chan_grep` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:86:5` | vis run: `--grant p=/bin/echo`: không phân giải được `C:/bin/echo`: The system cannot find |
| 7 | `cap_proc_grant::grant_proc_cho_cap_dir_la_loi_kieu` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:156:5` | assertion `left == right` failed: (0, [49, 10], "") |
| 8 | `cap_proc_grant::grant_tuyet_doi_echo_chay` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:69:5` | assertion `left == right` failed: (2, [], "vis run: `--grant p=/bin/echo`: không phân giải |
| 9 | `cap_proc_grant::hai_grant_pipeline_hai_stage` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:141:5` | assertion `left == right` failed: (2, [], "vis run: `--grant p=/bin/echo`: không phân giải |
| 10 | `cap_proc_grant::khong_grant_khong_gioi_han` | **a** | `crates/vis-cli/tests/cap_proc_grant.rs:120:5` | assertion `left == right` failed: (1, [], "d.vish:5:1: error VS0501: không chạy được chươn |
| 11 | `extern::cat_doc_stdin_rong_la_rong` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/cat` — test phải đỏ, không fallback PATH |
| 12 | `extern::duong_dan_hermetic_co_that` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/echo` — test phải đỏ, không fallback PATH |
| 13 | `extern::echo_ra_list_str` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/echo` — test phải đỏ, không fallback PATH |
| 14 | `extern::echo_raw_giu_newline` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/echo` — test phải đỏ, không fallback PATH |
| 15 | `extern::explain_perms_in_nhat_ky` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/echo` — test phải đỏ, không fallback PATH |
| 16 | `extern::false_thoat_ma_1_la_vs0501` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/usr/bin/false` — test phải đỏ, không fallback PATH |
| 17 | `extern::printf_nhieu_dong_bo_newline_cuoi` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/usr/bin/printf` — test phải đỏ, không fallback PATH |
| 18 | `extern::stderr_khong_tron_stdout` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/cat` — test phải đỏ, không fallback PATH |
| 19 | `extern::timeout_la_vs0501` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/bin/sleep` — test phải đỏ, không fallback PATH |
| 20 | `extern::true_khong_in_gi` | **a** | `crates/vis-cli/tests/extern.rs:35:5` | thiếu chương trình hermetic `/usr/bin/true` — test phải đỏ, không fallback PATH |
| 21 | `grant_proc_canon::grant_duong_dan_tuong_doi` | **a** | `crates/vis-cli/tests/grant_proc_canon.rs:227:5` | assertion `left == right` failed: (1, [], "e.vish:5:1: error VS0501: không chạy được chươn |
| 22 | `grant_proc_canon::grant_ten_tran_phan_giai_path` | **a** | `crates/vis-cli/tests/grant_proc_canon.rs:174:5` | assertion `left == right` failed: (2, [], "vis run: `--grant p=echo`: không thấy lệnh `ech |
| 23 | `grant_proc_canon::khong_grant_khong_gioi_han` | **a** | `crates/vis-cli/tests/grant_proc_canon.rs:287:5` | assertion `left == right` failed: (1, [], "g.vish:5:1: error VS0501: không chạy được chươn |
| 24 | `grant_proc_canon::sao_chep_trung_ten_bi_chan` | **a** | `crates/vis-cli/tests/grant_proc_canon.rs:109:5` | vis run: `--grant p=/bin/echo`: không phân giải được `C:/bin/echo`: The system cannot find |
| 25 | `grant_proc_canon::tuyet_doi_khop_thi_chay` | **a** | `crates/vis-cli/tests/grant_proc_canon.rs:91:5` | assertion `left == right` failed: (2, [], "vis run: `--grant p=/bin/echo`: không phân giải |
| 26 | `ngoai_vm::cat_pipe_vm_cung_byte` | **a** | `crates/vis-cli/tests/ngoai_vm.rs:56:5` | assertion `left == right` failed: [67, 58, 92, 85, 115, 101, 114, 115, 92, 112, 115, 92, 6 |
| 27 | `ngoai_vm::complete_ma_thoat_vm_cung_byte` | **a** | `crates/vis-cli/tests/ngoai_vm.rs:64:5` | assertion `left == right` failed: [67, 58, 92, 85, 115, 101, 114, 115, 92, 112, 115, 92, 6 |
| 28 | `ngoai_vm::sh_exit3_vm_cung_byte` | **a** | `crates/vis-cli/tests/ngoai_vm.rs:88:5` | assertion `left == right` failed: [67, 58, 92, 85, 115, 101, 114, 115, 92, 112, 115, 92, 6 |
| 29 | `pipeline::chuong_trinh_ngoai_co_that` | **a** | `crates/vis-cli/tests/pipeline.rs:18:5` | thiếu chương trình ngoài `/bin/echo` (test giả định POSIX có nó) |
| 30 | `pipeline::complete_giu_stderr_va_code_khac_0` | **a** | `crates/vis-cli/tests/pipeline.rs:130:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-cmp3-169 |
| 31 | `pipeline::complete_grep_khong_khop_het_la_loi` | **a** | `crates/vis-cli/tests/pipeline.rs:104:5` | assertion `left == right` failed: complete ⇒ **không** còn VS0501: C:\Users\ps\AppData\Loc |
| 32 | `pipeline::complete_grep_khop_code_0` | **a** | `crates/vis-cli/tests/pipeline.rs:119:5` | assertion `left == right` failed |
| 33 | `pipeline::echo_qua_cat_nhan_stdin` | **a** | `crates/vis-cli/tests/pipeline.rs:72:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-cat-1691 |
| 34 | `pipeline::echo_qua_grep_tim_thay` | **a** | `crates/vis-cli/tests/pipeline.rs:81:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-grep-169 |
| 35 | `pipeline::fixture_vish_trong_test_cmd_pipe` | **a** | `crates/vis-cli/tests/pipeline.rs:255:9` | assertion `left == right` failed: fixture cat: C:\Users\ps\vifast-windows\repo\viscript\cr |
| 36 | `pipeline::grep_khong_khop_van_la_loi_khi_khong_complete` | **a** | `crates/vis-cli/tests/pipeline.rs:91:5` | stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-grep0-16912\grep0.vish:2:1: error VS0501:  |
| 37 | `pipeline::list_nhieu_dong_qua_grep_loc_dung_dong` | **a** | `crates/vis-cli/tests/pipeline.rs:202:5` | assertion `left == right` failed |
| 38 | `pipeline::printf_hai_dong_qua_cat_ra_hai_dong` | **a** | `crates/vis-cli/tests/pipeline.rs:188:5` | assertion `left == right` failed |
| 39 | `pipeline::printf_khong_newline_qua_cat_khong_them_dong_rong` | **a** | `crates/vis-cli/tests/pipeline.rs:179:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-printf-1 |
| 40 | `pipeline::raw_str_di_nguyen_van_khong_them_newline` | **a** | `crates/vis-cli/tests/pipeline.rs:212:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-pipe-raw-1691 |
| 41 | `pipeline::timeout_neu_ro_stage` | **a** | `crates/vis-cli/tests/pipeline.rs:230:5` | thông điệp timeout phải nêu stage: C:\Users\ps\AppData\Local\Temp\vis-pipe-to-16912\to.vis |
| 42 | `pipeline::vm_co_host_chay_cung_interp` | **a** | `crates/vis-cli/tests/pipeline.rs:298:5` | assertion `left == right` failed |
| 43 | `soak_script::vis_soak_hop_le_va_in_du_10_vong` | **a** | `crates/vis-cli/tests/soak_script.rs:49:10` | chạy --dry-run: Os { code: 193, kind: Uncategorized, message: "%1 is not a valid Win32 app |
| 44 | `sync_list::check_sync_du_het_thi_exit_0_trong_cay_gia` | **a** | `crates/vis-cli/tests/sync_list.rs:338:5` | assertion `left == right` failed: đủ mục ⇒ exit 0. stdout: "" |
| 45 | `sync_list::check_sync_tai_repo_that_in_du_8_muc_va_exit_0` | **a** | `crates/vis-cli/tests/sync_list.rs:288:5` | assertion `left == right` failed: stdout: "" |
| 46 | `sync_list::check_sync_thieu_mot_muc_thi_exit_2` | **a** | `crates/vis-cli/tests/sync_list.rs:309:5` | assertion `left == right` failed: thiếu mục ⇒ exit 2. stdout: "" |
| 47 | `sync_list::help_co_check_sync` | **a** | `crates/vis-cli/tests/sync_list.rs:359:5` | assertion `left == right` failed: stderr: <3>WSL (9 - Relay) ERROR: CreateProcessCommon:88 |
| 48 | `vish_main::main_rong_va_pipeline_hop_le` | **a** | `crates/vis-cli/tests/vish_main.rs:61:5` | assertion `left == right` failed: interp stderr: a.vish:4:1: error VS0501: không chạy được |
| 49 | `?::host_conformance_suite` | **b** | `` | ok 1 - fs_read_thuong |
| 50 | `json_ra::check_json_ba_tep_dung_thu_tu` | **b** | `crates/vis-cli/tests/json_ra.rs:97:52` | thiếu f1 |
| 51 | `json_ra::check_json_loi_hai_tep` | **b** | `crates/vis-cli/tests/json_ra.rs:272:5` | assertion `left == right` failed |
| 52 | `json_ra::check_json_loi_trong_tep_phu` | **b** | `crates/vis-cli/tests/json_ra.rs:248:5` | assertion `left == right` failed: {"ok":false,"files":[{"file":"C:\\Users\\ps\\AppData\\Lo |
| 53 | `json_ra::check_json_severity_warning_tab` | **b** | `crates/vis-cli/tests/json_ra.rs:169:5` | python3 lỗi: Traceback (most recent call last): |
| 54 | `json_ra::check_json_tep_sach` | **b** | `crates/vis-cli/tests/json_ra.rs:55:5` | assertion `left == right` failed |
| 55 | `lsp::lsp_completion_thay_ham_module` | **b** | `crates/vis-cli/tests/lsp.rs:875:5` | assertion `left == right` failed: {"jsonrpc":"2.0","id":2,"result":[]} |
| 56 | `lsp::lsp_completion_thay_std_da_use` | **b** | `crates/vis-cli/tests/lsp.rs:911:5` | {"jsonrpc":"2.0","id":2,"result":[]} |
| 57 | `lsp::lsp_hover_ham_module` | **b** | `crates/vis-cli/tests/lsp.rs:895:5` | {"jsonrpc":"2.0","id":2,"result":null} |
| 58 | `lsp::lsp_module_std_sai_giong_vis_check` | **b** | `crates/vis-cli/tests/lsp.rs:753:5` | assertion `left == right` failed: LSP khác vis check |
| 59 | `lsp::lsp_module_thieu_giong_vis_check` | **b** | `crates/vis-cli/tests/lsp.rs:779:5` | LSP thiếu VS0701: [] |
| 60 | `net::tcp_tu_choi` | **b** | `crates/vis-cli/tests/net.rs:153:5` | stdout: "lỗi: quá thời gian chờ nối tới `127.0.0.1:50267`\n" |
| 61 | `net_server::server_hai_ket_noi_lien_tiep` | **b** | `crates/vis-cli/tests/net_server.rs:67:9` | quá 5 s chưa thấy địa chỉ trong C:\Users\ps\AppData\Local\Temp\vis-srv-hai-ket-noi-5124\ad |
| 62 | `net_server::server_hai_tien_trinh` | **b** | `crates/vis-cli/tests/net_server.rs:67:9` | quá 5 s chưa thấy địa chỉ trong C:\Users\ps\AppData\Local\Temp\vis-srv-hai-5124\addr-0.txt |
| 63 | `net_server::server_mot_lan` | **b** | `crates/vis-cli/tests/net_server.rs:67:9` | quá 5 s chưa thấy địa chỉ trong C:\Users\ps\AppData\Local\Temp\vis-srv-mot-5124\addr.txt |
| 64 | `chan_doan_da_tep::b_dai_van_dung_a` | **c** | `crates/vis-cli/tests/chan_doan_da_tep.rs:184:5` | assertion `left == right` failed: C:\Users\ps\AppData\Local\Temp\vis-da-tep-dai-14424-1\a. |
| 65 | `chan_doan_da_tep::loi_ca_hai_tep` | **c** | `crates/vis-cli/tests/chan_doan_da_tep.rs:117:5` | assertion `left == right` failed: C:\Users\ps\AppData\Local\Temp\vis-da-tep-hai-14424-0\a. |
| 66 | `chan_doan_da_tep::loi_cap_tep_a` | **c** | `crates/vis-cli/tests/chan_doan_da_tep.rs:84:5` | assertion `left == right` failed: C:\Users\ps\AppData\Local\Temp\vis-da-tep-a-14424-2\a.vi |
| 67 | `chan_doan_da_tep::loi_trong_b_dong_sau` | **c** | `crates/vis-cli/tests/chan_doan_da_tep.rs:106:5` | assertion `left == right` failed: C:\Users\ps\AppData\Local\Temp\vis-da-tep-b-14424-3\a.vi |
| 68 | `chan_doan_da_tep::loi_trong_fn_a` | **c** | `crates/vis-cli/tests/chan_doan_da_tep.rs:95:5` | assertion `left == right` failed: C:\Users\ps\AppData\Local\Temp\vis-da-tep-fn-14424-4\a.v |
| 69 | `diag_cmp::ca_xanh_ba_duong_khop_stdout` | **c** | `crates/vis-cli/tests/diag_cmp.rs:494:5` | 4 ca lệch: |
| 70 | `json_ra::check_json_ten_tep_dac_biet_parse_duoc` | **c** | `crates/vis-cli/tests/json_ra.rs:24:29` | ghi: Os { code: 123, kind: InvalidFilename, message: "The filename, directory name, or vol |
| 71 | `ls_open::khong_mock_thi_dung_fs_that` | **c** | `crates/vis-cli/tests/ls_open.rs:250:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-ls-real-17292 |
| 72 | `lsp::lsp_ba_tep_bang_vis_check` | **c** | `crates/vis-cli/tests/lsp.rs:820:9` | assertion `left == right` failed: lệch ở C:\Users\ps\AppData\Local\Temp\vis-lsp-so-16892-0 |
| 73 | `lsp_da_tep::a_sach_rong_b_co_loi` | **c** | `crates/vis-cli/tests/lsp_da_tep.rs:154:5` | Content-Length: 199 |
| 74 | `lsp_da_tep::did_change_van_dung_a` | **c** | `crates/vis-cli/tests/lsp_da_tep.rs:182:5` | Content-Length: 199 |
| 75 | `lsp_da_tep::mo_a_chi_chan_doan_a` | **c** | `crates/vis-cli/tests/lsp_da_tep.rs:114:5` | Content-Length: 199 |
| 76 | `lsp_da_tep::mo_b_chan_doan_b` | **c** | `crates/vis-cli/tests/lsp_da_tep.rs:134:5` | Content-Length: 199 |
| 77 | `quyen::doc_ngoai_quyen` | **c** | `crates/vis-cli/tests/quyen.rs:92:5` | stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 78 | `quyen::doc_trong_quyen` | **c** | `crates/vis-cli/tests/quyen.rs:70:5` | assertion `left == right` failed: stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 79 | `quyen::ghi_co_co` | **c** | `crates/vis-cli/tests/quyen.rs:135:5` | assertion `left == right` failed: stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 80 | `quyen::ghi_thieu_co` | **c** | `crates/vis-cli/tests/quyen.rs:114:5` | stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 81 | `quyen::grant_doi_duong_dan` | **c** | `crates/vis-cli/tests/quyen.rs:180:5` | stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 82 | `quyen::hai_cap_dir` | **c** | `crates/vis-cli/tests/quyen.rs:227:5` | stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 83 | `quyen::khong_khai_cap_thi_khong_han_che` | **c** | `crates/vis-cli/tests/quyen.rs:154:5` | assertion `left == right` failed: stderr: f.vis:4:19: error VS0102: escape không hợp lệ |
| 84 | `quyen::net_thieu_cap` | **c** | `crates/vis-cli/tests/quyen.rs:200:5` | stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 85 | `sem::sem_323_ca_qua_binary` | **c** | `crates/vis-cli/tests/sem.rs:230:5` | 4/447 ca sai: |
| 86 | `siet_caps_pkg::co_pkg_va_khai_du_cap_thi_chay` | **c** | `crates/vis-cli/tests/siet_caps_pkg.rs:209:5` | assertion `left == right` failed: stderr: f.vis:4:23: error VS0102: escape không hợp lệ |
| 87 | `span_tep::json_bao_dung_tep_va_dong_cot_moi_ca` | **c** | `crates/vis-cli/tests/span_tep.rs:93:5` | 5 ca sai: |
| 88 | `st83_no_nho::exists_ngoai_quyen_trap` | **c** | `crates/vis-cli/tests/st83_no_nho.rs:72:5` | f.vis:3:23: error VS0102: escape không hợp lệ |
| 89 | `st83_no_nho::exists_trong_quyen_tra_bool` | **c** | `crates/vis-cli/tests/st83_no_nho.rs:98:5` | assertion `left == right` failed: stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 90 | `std_fs_json::differential_fs_va_json_khop_hai_duong` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [diff] interp exit 2: diff.vis:5:20: error VS0102: escap |
| 91 | `std_fs_json::explain_perms_in_fs_nhat_ky` | **c** | `crates/vis-cli/tests/std_fs_json.rs:377:5` | assertion `left == right` failed: stderr: log.vis:4:20: error VS0102: escape không hợp lệ |
| 92 | `std_fs_json::fs_duong_dan_tuyet_doi_duoc_phep` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [abs] interp exit 2: abs.vis:4:20: error VS0102: escape  |
| 93 | `std_fs_json::fs_exists_va_read_thieu_la_err` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [ex] interp exit 2: ex.vis:4:21: error VS0102: escape kh |
| 94 | `std_fs_json::fs_ghi_vao_duong_dan_sai_la_err` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [bad] interp exit 2: bad.vis:4:20: error VS0102: escape  |
| 95 | `std_fs_json::fs_list_sap_xep_khong_de_quy` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [ls] interp exit 2: ls.vis:4:19: error VS0102: escape kh |
| 96 | `std_fs_json::fs_write_read_round_trip` | **c** | `crates/vis-cli/tests/std_fs_json.rs:52:5` | assertion `left == right` failed: [rw] interp exit 2: rw.vis:4:20: error VS0102: escape kh |
| 97 | `strict_caps::strict_cap_dir_ngoai_root` | **c** | `crates/vis-cli/tests/strict_caps.rs:122:5` | f.vis:3:23: error VS0102: escape không hợp lệ |
| 98 | `strict_caps::strict_cap_dir_trong_root` | **c** | `crates/vis-cli/tests/strict_caps.rs:99:5` | assertion `left == right` failed: stderr: f.vis:3:23: error VS0102: escape không hợp lệ |
| 99 | `vis_test::cac_dong_ok_dung_thu_tu_khong_lap` | **c** | `crates/vis-cli/tests/vis_test.rs:62:9` | dòng thứ 423 sai thứ tự: ok 426 - vish\ngoai-thoat-ma.vish |
| 100 | `vis_test::tap_hop_le_va_du_so_ca` | **c** | `crates/vis-cli/tests/vis_test.rs:49:5` | assertion `left == right` failed: exit phải 0 khi toàn xanh: |
| 101 | `vis_test::vm_khop_interp_va_khong_con_skip` | **c** | `crates/vis-cli/tests/vis_test.rs:76:5` | assertion `left == right` failed: VM: exit phải 0 (ca cần host thì SKIP): |
| 102 | `vish_cmd_body::khong_bind_van_la_lenh` | **c** | `crates/vis-cli/tests/vish_cmd_body.rs:356:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-cmdbody-khong |
| 103 | `vish_cmd_body::than_lenh_tren_thu_muc_that` | **c** | `crates/vis-cli/tests/vish_cmd_body.rs:312:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-cmdbody-that- |
| 104 | `vm_main_argv::argv_dau_cap_sau` | **c** | `crates/vis-cli/tests/vm_main_argv.rs:112:5` | assertion `left == right` failed: (2, [], "h.vis:4:40: error VS0102: escape không hợp lệ\n |
| 105 | `vsh1b::long_tren_thu_muc_that` | **c** | `crates/vis-cli/tests/vsh1b.rs:161:5` | assertion `left == right` failed: stderr: C:\Users\ps\AppData\Local\Temp\vis-vsh1b-that-75 |

## Ghi chú

1. **a (48)**: chia 3 nhóm con — `/bin/*` trực tiếp (`cap_proc*`, `grant_proc_canon`, `extern`, `pipeline`, `ngoai_vm`, `vish_main`),
   `bash`/`sh` thiếu (`sync_list`), và `python3`/tệp thực thi Windows (`soak_script`, `json_ra::check_json_severity_warning_tab`).
   Cả 3 đều xử được bằng helper repo-built + biến môi trường, **không** sửa kỳ vọng.
2. **b (15)**: `lsp` 6 (completion/hover rỗng, "LSP khác vis check", thiếu `VS0701`) · `net_server` 3 (không thấy tệp `addr`)
   · `net` 1 (timeout nối) · `host_conformance` 1 (TAP) · `json_ra` 4 (tên tệp đặc biệt/`thiếu f1`/JSON đường dẫn). Cần Agent 0 chốt.
3. **c (42)**: chủ yếu là **dấu `\`** trong đường dẫn đưa vào mã `.vis`/golden ⇒ lỗi `escape không hợp lệ` (quyen 8, std_fs_json 7,
   strict_caps 2, st83 2, siet_caps 1, vm_main_argv 1) hoặc đường dẫn tuyệt đối trong TAP/golden (chan_doan_da_tep 5, vis_test 3,
   vish_cmd_body 2, vsh1b 1, ls_open 1, sem 1, span_tep 1, diag_cmp 1, json_ra 4). Cách xử lý **không** phải "sửa sản phẩm":
   test nên dùng `/` hoặc escape, và golden nên so **tương đối**; khi đó nhiều ca sẽ xanh **mà không cần** kỳ vọng theo nền.
4. **Đề nghị**: trước khi mở danh sách kỳ vọng theo nền, thử **sửa 42 ca (c)** theo hướng "test dùng `/`/tương đối" — nếu xanh hết thì
   danh sách theo nền thu về **0**, đúng tinh thần VD-162(1) "chạy native, cùng kỳ vọng". Việc này thuộc **ST-168 mục 5** (tài liệu), **chưa** làm trong ST-169.
5. Bảng này **không** thay đổi số đo: baseline Windows vẫn **1068/105 @0df923c2**; cổng Windows vẫn dừng **L0** (150/4 fail-fast).



---

# PHẦN C · VS-ERGO — R1–R24 + 2 PHA + 3 SCRIPT (ST-172e/f)

<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# VS-ERGO · Bộ quy tắc công thái học ViScript (R1–R10) — ST-172b
**Trạng thái**: **CHỜ AGENT 0 DUYỆT** (cùng đợt 6 tài liệu VS3) · **ST-172b** · **Chỉ đọc, 0 dòng mã**
**Căn cứ**: **VD-171** (thay mục 7, 8, 10, 11, 13 của VD-170; sửa VD-19/Q2/§1.6) + `vs-ergo-facts.md` (ST-172a, APPROVED) + probe tại HEAD `6d01d8d3`.
**Mỗi quy tắc**: Hiện trạng (probe + `file:dòng`) → Đặc tả → Ca đỏ-trước → Bảng chuyển đổi mã (nếu có) → Ảnh hưởng → Task ≤ 3 h → Phụ thuộc. **Không bàn lại hướng.**

## Bảng tổng (VD-173 — pha, task, giờ, ảnh hưởng)
| R | Nội dung | Pha | Task | Giờ | Test/ví dụ hiện có bị ảnh hưởng |
|---|---|---|---|---|---|
| P0 | newtype báo lỗi · bỏ “VS0” · cheat sheet doc-test | **1** | 3 | 5 | 8 (golden + `API-CODES`) |
| R1 | Chữ hoa enum + cảnh báo pattern thường + xoá `None` | **1** | 3 | 3,5 | 6 (golden, fmt) |
| R2 | Option/Result enum std, bỏ `none`, `T??` | **1** | 3 | 8 | 8 (option/kenh) |
| R3 | `if let` đúng tầng, `let else` khối thoát | **1** | 2 | 2,5 | 6 |
| R4 | `Error` record + `Json` enum + `AnyError` | **1** | 3 | 8 | 30 (toàn bộ test std) |
| R5 | tên đối số, record theo tên, method toàn std | **1** | 4 | 8,5 | 10 (record/gọi hàm) |
| R6 | `p"…"` thô, `str+str`, format spec, `print` nhiều đối số | **1** | 4 | 6,5 | 5 (str/golden) |
| R7 | `/`→float, `//`, `%`, const suy kiểu | **1** | 3 | 4,5 | **4** (run.rs:98,100,112; interp_v2.rs:77) |
| R8 | xác minh `xs[i]?`/`xs.get` | **1** | 1 | 1 | 1 |
| R9 | mỗi mã một nghĩa, gợi ý, bỏ “VS0”, báo tại chỗ | **1** | 3 | 4,5 | 40 (mọi test so mã) |
| R11 | hàm dựng (record/enum payload) | **1** | 2 | 3,5 | 3 |
| R12 | `type X = T`, `type Email(str)`, bỏ `alias`/`newtype` | **1** | 1 | 2,5 | 2 |
| R16 | `let` che tên, `var` không khai lại | **1** | 1 | 1 | 2 |
| R17 | điều kiện bool + **gợi ý** `is_empty()` | **1** | 1 | 0,5 | 2 |
| R19b | tràn i64 **panic** (phá test VS0501 hiện có) | **1** | 1 | 1 | 1 |
| R19c | float literal cần chữ số sau `.`; `5.bit_or(3)` | **1** | 1 | 1 | 1 |
| **Tổng pha 1** | | | **30** | **≈ 62** | **≈ 120** |
| R13 | biến thể không tiền tố + prelude | 2 | 1 | 1,5 | 0 |
| R14 | method người dùng (`impl P:`, `self`, `var self`) — **đầu pha 2** | 2 | 2 | 5 | 0 |
| M1–M3 | Nguyên tắc mô hình (doc) | — | 0 | 1 | 0 |
| R15 | nối dòng `.`/`??`/`and`/`or` (phần `() [] {}` đã chạy) | 2 | 1 | 1,5 | 0 |
| R18 | pattern `as`, record, `while let` | 2 | 2 | 3,5 | 0 |
| R19a | `0o755`, bit method | 2 | 2 | 2,5 | 0 |
| R20 | `assert`, `use as`, `use {}`, list/range | 2 | 2 | 3 | 0 |
| R21 | pipeline chỉ shell/cmd (tài liệu) | 2 | 1 | 0,5 | 0 |
| R10 | đơn vị in chuẩn hoá + trộn đơn vị | 2 | 2 | 3 | 3 (golden đơn vị) |
| R22 | Trait là kiểu (`list[Shape]`, bảng ảo, đóng gói) | 2 | 2 | 9 | 0 |
| R23 | Gán lồng sâu trên `var` (`u.addr.city = x`) | 2 | 3 | 4 | 0 |
| R24 | Iterator lười + `.to_list()/.to_map()/.to_set()` (chốt trước R20) | 2 | 3 | 6 | 0 |
| **Tổng pha 2** | | | **20** | **≈ 38** | **≈ 3** |

## R1 · Chữ hoa — **Pha 1**
**Hiện trạng**: pattern nhận **cả `Some(v)` lẫn `some(v)`** (probe `c6`,`c8`); `None` là **builtin** in `<builtin None>` (probe `r2`); `lower.rs:53` có `None`; `true/false/none` viết thường.
**Đặc tả**: kiểu/trait/biến thể enum **UpperCamel** (`Some/None/Ok/Err/Circle`); biến/hàm/module/trường **snake_case**; hằng **UPPER_SNAKE**; `true/false` thường. **Dựng và pattern viết giống hệt nhau.** Pattern thường ⇒ **cảnh báo** + `vis fmt` **tự sửa**.
**Ca đỏ-trước**: `some(v)` ⇒ cảnh báo + fmt sửa `Some(v)`; `Some(1)`/`none`/`None` xanh; `let Some = 1` ⇒ lỗi.
**Chuyển đổi**: `some`/`none` pattern → `Some`/`None`; xoá `None` khỏi builtin (`lower.rs:53`), thêm **VS0111** (cảnh báo) + fmt.
**Ảnh hưởng**: `tyck/pat.rs`, `lower.rs:53`, fmt, `all_codes`, golden, docs.
**Task**: **172b-R1a** (1,5 h cảnh báo) · **R1b** (1 h fmt sửa) · **R1c** (1 h xoá `None`). **Phụ thuộc**: R9 (mã mới).

## R2 · Option/Result là enum std — **Pha 1**
**Hiện trạng**: `none` là **từ khoá** (`token.rs:185`); `T??` ⇒ **VS0201** (probe `y2`); `Some(1)` dựng được, `some(1)` ⇒ VS0501 (probe `c4`); tự gói `T→T?` **đã có** (probe `n1`).
**Đặc tả**: `enum Option[T]: Some(T), None`; `enum Result[T,E]: Ok(T), Err(E)`; `T?` = `Option[T]`; **bỏ từ khoá `none`** (`None` là biến thể); **cho phép `T??`**; tự gói **đủ số lớp**, `None` **lớp ngoài cùng**, lớp trong `Some(None)`.
**Ca đỏ-trước** (bắt buộc theo A0):
- `fn first[T](xs: list[T]) -> T?` gọi với `T = int?` ⇒ kiểu trả `int??`.
- `map[str, int?].get` phân biệt **không có khoá** ⇒ `None` vs **khoá = None** ⇒ `Some(None)` vs **có giá trị** ⇒ `Some(Some(v))`.
- `let x: int?? = 5` ⇒ `Some(Some(5))`; `let y: int?? = None` ⇒ `None`; `let z: int?? = Some(None)` ⇒ `Some(None)`.
**Chuyển đổi**: bỏ `none` keyword; `None` thành biến thể; sửa VD-19 (A0).
**Ảnh hưởng**: `ty.rs`, parser (`T?` lặp), tyck gói nhiều lớp, interp/VM, `lower.rs`, docs.
**Task**: **172b-R2a** (3 h) enum std + bỏ `none`; **R2b** (3 h) `T??` + gói nhiều lớp; **R2c** (2 h) ca `first`/`map.get`. **Phụ thuộc**: R1.

## R3 · Vắng mặt/lỗi: `?`, `??`, `if let`, `let … else` — **Pha 1**
**Hiện trạng** (probe mới, **khác** ST-172a):
- `??` **chạy**: `o ?? 9` ⇒ `5` (`p5`).
- **`if let` CÓ**: `if let v = o:` ⇒ `5` (`y6`); `if let Some(v) = o:` ⇒ **VS0412** “pattern ctor `Some` không khớp kiểu `int`” (`y4`) — pattern kiểm với **kiểu trong**, không phải `int?`.
- **`let … else:` CÓ**: `let x = r else:` + khối `return` ⇒ chạy (`y7`); `else 0` ⇒ **VS0201** (`y5`) — đúng ý A0: **không** có `f() else 0`.
**Đặc tả**: `?` đẩy lỗi lên; `??` giá trị mặc định; `if let` / `let … else` / `match` xử lý tại chỗ. **`let x = expr else e: <khối>`** — khối **bắt buộc thoát**; **Option không có `e`**; **không** có biểu thức `f() else 0`.
**Ca đỏ-trước**: `if let Some(v) = o:` chạy (sửa kiểm pattern theo `T?`); `let x = r else:` không thoát ⇒ lỗi rõ; `f() else 0` ⇒ VS0201.
**Ảnh hưởng**: `tyck` (pattern `if let`), parser `let else`, docs.
**Task**: **172b-R3a** (1,5 h sửa if-let pattern) · **R3b** (1 h ràng buộc khối thoát). **Phụ thuộc**: R2.

## R4 · Lỗi: `Error` record + `Json` enum + `AnyError` — **Pha 1**
**Hiện trạng**: probe `r4` (copy nguyên): `r4.vis:4:13: error VS0412: `?` trên `Result` chỉ dùng được khi hàm trả `Result[…, …]`, hàm này trả `str?` (§4.4)`; `json.decode(str)->V` (`API-STD.txt:15`); std trả `Result[_, str]`; `AnyError` chỉ là thiết kế (VD-20).
**Đặc tả**: mọi hàm std trả **`Result[T, Error]`** với `Error` là **record cài `trait Error { message, source, span }`** thay `str`; `?` tự chuyển `E: Error → AnyError` (VD-20); `json.decode → Result[Json, Error]`, `Json` là **enum** (`Null/Bool/Num/Str/Arr/Obj`), **bỏ `V`**; `main` có thể trả `Result[(), AnyError]` (in lỗi, **exit 1**).
**Ca chuẩn (2 dòng)**: `let s = fs.read(p)?` · `let v = json.decode(s)?`.
**Ca đỏ-trước**: hàm trả `Result[T, AnyError]` dùng `fs.read()?` + `json.decode()?` chạy; lỗi giữ `message/source/span`; `json.decode` trả `Json`; `main` trả `Err` ⇒ exit **1**.
**Bảng chuyển đổi**: `Result[_, str]` → `Result[_, Error]`; `V` → `Json`.
**Ảnh hưởng**: `tyck/expr.rs:177,194`, `ty.rs`, `stdlib.rs`, mọi test std, docs §4.4. **Phụ thuộc**: R2, R9.

## R5 · Gọi hàm: tên + record-theo-tên + method toàn std — **Pha 1**
**Hiện trạng**: `f(1, b: 5)` ⇒ **VS0201** (`n3`); record **chỉ** `{name: …}` (`exhaustive.rs:151`); `x.f(a)` mới một phần (`z1`: `str.trim`; `s.upper` chạy); `str(5)` chạy (`r3`).
**Đặc tả**: `f(1, b: 5)`; `--b` **chỉ** chế độ lệnh/`cmd`; `User(name: "An", age: 20)`, default **bỏ được**; `{…}` **chỉ map**; `x.f(a) ≡ mod.f(x, a)` khi **tham số đầu của `f` có kiểu của `x`** — **toàn bộ std**; báo **xung đột** `std.str`/`str`/builtin `str(x)`.
**Ca đỏ-trước**: `f(1, b: 5)` chạy; `f(b: 5, 1)` lỗi; `User(name: "a")` với `age: int = 0` ⇒ `age==0`; `{name: "a"}` ⇒ lỗi; `" abc ".trim()` ⇒ `abc`; `str(5)` vẫn `5`.
**Ảnh hưởng**: parser args/record, tyck call/method, `stdlib.rs`, docs, test record cũ.
**Task**: **172b-R5a** (2 h) · **R5b** (2 h) · **R5c** (2 h) · **R5d** (2,5 h). **Phụ thuộc**: R3, R4.

## R6 · Chuỗi — **Pha 1**
**Hiện trạng**: `p"C:\x"` ⇒ **VS0102** (`r1`); `"a"+"b"` ⇒ VS0412 (`t1`); `"{n:.2}"` ⇒ VS0201 (`t10`); `print("a","b")` ⇒ VS0401 (`t4`).
**Đặc tả**: `p"…"` **không escape**, **có** nội suy; `str+str` **nối**; `{expr:spec}` **runtime**; `print(a,b,…)` **cách 1 dấu cách**, `print()` ⇒ `\n`.
**Ca đỏ-trước**: `p"C:\x"` in `C:\x`; `p"a{b}"` nội suy; `"a"+"b"` ⇒ `ab`; `"{3.14159:.2}"` ⇒ `3.14`; `print("a","b")` ⇒ `a b`.
**Ảnh hưởng**: lexer `lit.rs`, `tyck/expr.rs:501,1303`, parser nội suy, interp, golden.
**Task**: **172b-R6a** (1,5 h) · **R6b** (1,5 h) · **R6c** (2,5 h) · **R6d** (1 h). **Phụ thuộc**: —.

## R7 · Số — **Pha 1**
**Hiện trạng**: `(Div, Int, Int) => Type::Int` (`crates/vis-types/src/tyck/num.rs:57`); runtime `builtin.rs:464`; **test dựa vào `int/int=int`**: `crates/vis-interp/tests/run.rs:98` (`7/2`→`3`), `:100` (`-7/2`→`-3`), `:112` (`5mb/5mb`→`1`), `interp_v2.rs:77` (`50pct/50pct`→`1`), `run.rs:120,429` (chia 0 ⇒ VS0502); `const A = 1` ⇒ VS0201 (`t5`); `1+1.5`⇒`2.5`, `let f: float = 1` (`p3`,`p4`).
**Đặc tả**: chỉ `int→float` chuyển ngầm; **`/` luôn `float`**; **`//`** chia lấy phần nguyên; **`%` khớp `//`**; `const` suy kiểu từ literal; sửa **Q2** (A0).
**Ca đỏ-trước**: `7/2` ⇒ `3.5`; `7 // 2` ⇒ `3`; `-7 // 2` ⇒ `-4`; `7 % 2` ⇒ `1`; `5mb/5mb` ⇒ `1.0`; `const A = 1` ⇒ `int`.
**Test phải sửa**: 4 ca trên (run.rs:98,100,112; interp_v2.rs:77) — **đổi kỳ vọng** theo ngữ nghĩa mới, **không** nới ngưỡng; chia 0 giữ.
**Ảnh hưởng**: `num.rs:57-65`, `builtin.rs:434,464`, parser `//`, test, docs Q2.
**Task**: **172b-R7a** (2 h) · **R7b** (1,5 h) · **R7c** (1 h). **Phụ thuộc**: —.

## R8 · Chỉ số (xác minh) — **Pha 1**
**Hiện trạng**: `xs[0]?` ⇒ **VS0412** “`?` chỉ dùng cho `T?`/`Result`, được `int`” (`x2`) ⇒ **CHƯA**; `xs.get(1)` ⇒ **`some(2)`** (`x3`) ⇒ **CÓ**; `xs[5]` ⇒ VS0503 (`b4`); `s[0]` ⇒ VS0412 gợi ý; `s[0:2]` ⇒ VS0201 (`b3`).
**Đặc tả**: giữ Q5/VD-18; chỉ **xác minh**.
**Ca đỏ-trước**: `xs[i]?` chạy; `xs.get(i)`; `xs[i]` ngoài phạm vi ⇒ VS0503.
**Task**: **172b-R8** (1 h). **Phụ thuộc**: R2.

## R9 · Thông báo lỗi: mỗi mã một nghĩa — **Pha 1**
**Hiện trạng**: `VS0412` phủ 4 nhóm (`tyck/mod.rs:1125`, `tyck/expr.rs:501`, `mod.rs:818,1067`); `VS0501` phủ tính năng + tràn i64 (`p2`) + `some(1)` (`t7`); `VS0201` gộp cú pháp; `VS0301` gộp tên/kiểu; `VS0604` gộp `&`/`^`/tiến trình; `VS0701` gộp module/std/pkg; nhiều thông điệp còn **“VS0”**.
**Đặc tả**: mỗi mã **một nghĩa**; lỗi cú pháp hay gặp có **dòng “gợi ý:”**; **bỏ “VS0”**; từ khoá chưa làm (`newtype`…) **báo lỗi tại chỗ dùng**.
**Bảng chuyển đổi đề xuất** (Agent 0 chốt số; `API-CODES.txt` A0 sở hữu):
| Mã cũ | Nghĩa mới | Mã mới |
|---|---|---|
| VS0412 | kiểu không khớp (chung) | **VS0412** |
| | gán ràng buộc bất biến | **VS0413** |
| | toán tử không hỗ trợ cho kiểu | **VS0414** |
| | `async` sai chỗ | **VS0415** |
| | range không `int` | **VS0416** |
| VS0501 | tính năng chưa hỗ trợ | **VS0501** |
| | tràn i64 lúc chạy | **VS0507** |
| VS0201 | cú pháp chung | **VS0201** |
| | `newtype` chưa hạ | **VS0203** |
| | `const` thiếu kiểu | **VS0204** |
| | dựng record bằng `{…}` | **VS0205** |
| | `let…else` khối không thoát | **VS0206** |
| VS0301 | tên không tồn tại | **VS0301** |
| | kiểu không tồn tại | **VS0303** |
| VS0604 | tính năng chung | **VS0604** |
| | chạy nền `&` | **VS0605** |
| | `^`/tiến trình ngoài | **VS0606** |
| VS0701 | tệp module | **VS0701** |
| | `std` chưa hỗ trợ | **VS0705** |
| | `pkg` chưa hỗ trợ | **VS0706** |
**Gợi ý cho 5 mẫu VS0201**: `const A = 1` → “const cần kiểu”; `s[a:b]` → “str không cắt”; `U(n: 1)` → “dựng record bằng tên kiểu”; `"{x:.2}"` → “format spec chưa hỗ trợ”; `|x|` → “closure dùng `x => expr`”.
**Ca đỏ-trước**: mỗi ca mới đúng 1 mã; không thông điệp nào chứa “VS0”; `newtype` báo tại dòng khai.
**Ảnh hưởng**: `vis-diag`, mọi test so mã, `API-CODES.txt`, docs.
**Task**: **172b-R9a** (2 h) · **R9b** (1,5 h) · **R9c** (1 h). **Phụ thuộc**: —.

## R10 · Đơn vị — **Pha 2**
**Hiện trạng**: `150m` in `150m`; `1h+30m` in `90m`; `5kb+5kb` ⇒ `10kb`; **`1kb+1kib` ⇒ `2024b`** (`y1`) — **trộn đơn vị KHÔNG báo lỗi**; `50pct+10pct` ⇒ `60pct` (`p9`).
**Đặc tả**: in **chuẩn hoá** (`1h30m`, `1.5mb`); **xác minh** trộn đơn vị ⇒ theo A0 **báo lỗi** (nếu chốt vậy, đổi auto-convert sang VS0414/VS0412).
**Ca đỏ-trước**: `150m` ⇒ `2h30m`; `1500kb` ⇒ `1.5mb`; `1kb+1kib` ⇒ lỗi (hoặc ghi rõ nếu giữ auto-convert); `45s` ⇒ `45s`.
**Ảnh hưởng**: `Type::Duration/Size` Display, `tyck/num.rs`, golden, docs.
**Task**: **172b-R10a** (2 h) · **R10b** (1 h). **Phụ thuộc**: R7.

## Thứ tự thực thi (A0 chốt)
**P0 của VD-170** (`newtype` báo lỗi tại chỗ dùng, bỏ “VS0”, cheat sheet doc-test) → **R9** → **R6** → **R1 + R2** → **R3 + R4** → **R5** → **R7** → **R10**. Tất cả **TRƯỚC VS3**.

## Scripts theo bộ quy tắc mới (dạng sẽ có sau khi làm; số dòng vs Python)
**Script 1 — log** (`?`, `if let`, `print` nhiều đối số):
```
use std.fs
fn main():
    let noi = fs.read("/tmp/ergo/app.log")?
    let dong = noi.split("\n").filter(l => l.len() > 0)
    print("tong dong:", dong.len())
    print("ERROR:", dong.filter(l => l.contains("ERROR")).len())
    print("WARN :", dong.filter(l => l.contains("WARN")).len())
```
**7 dòng** vs Python **6** — hơn 1 dòng (`use std.fs`).

**Script 2 — JSON** (2 dòng bằng `?`):
```
use std.fs
use std.json
fn main() -> Result[(), AnyError]:
    let ban = {"ten": "An", "lop": "10A"}
    let ra = json.encode_pretty(ban)?
    fs.write("/tmp/ergo/out.json", ra)?
    let s = fs.read("/tmp/ergo/out.json")?
    let v = json.decode(s)?
    print("da ghi", ra.len(), "byte")
```
**8 dòng** vs Python **8** — **ngang** (nhờ `?` + `Result` main).

**Script 3 — CLI** (`cmd` + `--help` sau R5/R12):
```
use std.fs
cmd dem(tep: str, --dem: bool = false) -> ():
    let dong = fs.read(tep)?.split("\n").filter(l => l.len() > 0)
    if --dem:
        print(dong.len(), "dong")
    else:
        for l in dong:
            print(l)
```
**8 dòng** vs Python **8** — **ngang** (sau khi `cmd` thân script + `--help` xong; hiện `cmd` không trộn script/lệnh — `cf` VS0201, tham số không thay được — `c6/c7`).

---

# PHẦN BỔ SUNG · R11–R21 (VD-172)
**Căn cứ**: VD-172 (bổ sung vào phạm vi ST-172b) · probe HEAD `2f4e8302` (probe trong `/tmp/ergo/v2/*.vis`) · **giữ nguyên** các mục R1–R10 ở trên.
**Mỗi mục**: hiện trạng (probe) → đặc tả → ca đỏ-trước → ảnh hưởng/task. **Không bàn lại hướng.**

## R11 · Mọi hàm dựng là hàm — **Pha 1**
**Hiện trạng**: dựng record theo tên `U(n: 1)` ⇒ **VS0201** (`t9`); khai báo biến thể enum **mang dữ liệu** `Circle(float)` ⇒ **VS0201** (`c2`,`c3`); biến thể **đơn vị** gọi `Shape.Empty` **chạy** (`c1` in `Empty`); `Some(1)` prelude chạy.

**Mã probe nguyên văn (VD-173#4)**:
```
# c2.vis — khai báo biến thể mang dữ liệu
enum Shape:
    Circle(float), Empty
# vis run c2.vis ⇒ c2.vis:2:17: error VS0201: mong `:`, thật `)`

# c3.vis — gọi kèm tiền tố
enum Shape:
    Circle(float)
# vis run c3.vis ⇒ c3.vis:2:17: error VS0201: mong `:`, thật `)`

# c1.vis — biến thể đơn vị (CHẠY)
enum Shape:
    Circle
    Empty
# vis run c1.vis ⇒ in `Empty`
```
⇒ **Kết luận**: biến thể **có dữ liệu** chưa khai báo được (VS0201), dù cheat sheet dựng `Circle(2.0)`; biến thể **đơn vị** khai báo + `Shape.Empty` **chạy**.

**Đặc tả**: record, biến thể enum, kiểu bọc đều gọi **như hàm**: `Circle(2.0)`, `Circle(r: 2.0)`; **tên đứng sau vị trí**; cùng luật với `fn`.
**Ca đỏ-trước**: `type C: r: float` + `C(2.0)` và `C(r: 2.0)`; `enum S: Circle(float)` + `Circle(2.0)`; `C(2.0, r: 1.0)` ⇒ lỗi (vị trí sau tên).
**Ảnh hưởng**: parser (biến thể mang dữ liệu), tyck gọi, lower/interp, docs.
**Task**: **R11a** (2 h) enum payload + gọi · **R11b** (1,5 h) record/wrapper ctor. **Phụ thuộc**: R2.

## R12 · Gộp khai báo kiểu (`type X = T`, `type Email(str)`) — **Pha 1**
**Hiện trạng**: `type X = int` ⇒ **VS0201** (`a1`); `type Email(str)` ⇒ **VS0201** (`a2`); **`alias X = int` CHẠY** (`d1`); `newtype Met = float` **bị bỏ qua im lặng** (`n11`) rồi dùng kiểu ⇒ VS0301 (`n8`).
**Đặc tả**: **`type X = T`** thay `alias`; **`type Email(str)`** thay `newtype` (dựng `Email("…")`, mở bằng `let Email(s) = e`); **bỏ từ khoá `alias`, `newtype`**.
**Ca đỏ-trước**: `type X = int`; `type Email(str)` + `Email("a@b")` + `let Email(s) = e`; `alias`/`newtype` ⇒ lỗi (hoặc cảnh báo chuyển).
**Ảnh hưởng**: parser khai báo, tyck, lower, `KEYWORDS` (`token.rs:185`), docs.
**Task**: **R12a** (2,5 h) `type =` + wrapper. **Phụ thuộc**: R9 (mã cho `alias/newtype`).

## R13 · Biến thể enum: prelude + tiền tố khi trùng — **Pha 2**
**Hiện trạng**: `Shape.Empty` (dùng `.`) **chạy** (`c1`); `Some/None/Ok/Err` prelude chạy; biến thể không tiền tố khi duy nhất — **chưa xác minh**; trùng tên giữa 2 enum — **chưa có** (do R11a chưa xong).
**Đặc tả**: không cần tiền tố khi tên **duy nhất**; trùng tên ⇒ **bắt buộc `Shape.Empty`**, lỗi biên dịch **có gợi ý**; `Some/None/Ok/Err` **luôn trong prelude**.
**Ca đỏ-trước**: `enum A: Empty` + `enum B: Empty` + `Empty` ⇒ lỗi + gợi ý `A.Empty`; `Shape.Empty` chạy; `Empty` đơn nhất chạy.
**Task**: **R13** (1,5 h). **Phụ thuộc**: R11a.

## R14 · Method người dùng: `impl P:` + `self`/`var self` — **Pha 2 (đầu)**
**Hiện trạng**: `impl P:` + `fn len(self)` ⇒ **VS0302** “khai báo trùng `len`” rồi **VS0403** “không phân giải được phương thức” (`a5`); `fn f(var self)` ⇒ **VS0403** (`a6`); method khai trong thân `type` ⇒ **VS0201** (`d2`). ⇒ **method người dùng CHƯA có**; hiện chỉ method builtin (`str`, `list`, `map`).
**Thiết kế đầy đủ**:
1. **Khai báo**: `impl P:` (P là kiểu bản ghi/trait? — **chỉ record/enum trước**); trong thân chỉ `fn`, không `type`/`const`.
2. **`self`**: `fn f(self) -> T` là **chỉ đọc** — không được gán `self.x = …` (lỗi biên dịch, gợi ý dùng `var self`).
3. **`var self`**: `fn f(var self) -> ()` **sửa được**; **chỉ gọi được trên biến `var`** (gọi trên `let` ⇒ lỗi), vì đây là thao tác đột biến tại chỗ (không copy).
4. **`Self`**: viết hoa, là bí danh của kiểu đang `impl`; dùng được ở kiểu trả/đối số/thân.
5. **Quan hệ trait**: `impl Trait for P` (nếu có) **không** đụng `impl P`; method trait gọi như method thường; trait có method mặc định dùng `self`.
6. **Quan hệ R5**: method người dùng **thắng** method std cùng tên (inherent trước); `x.f(a)` ≡ `P.f(x, a)` vẫn đúng khi `f` là method người dùng; **xung đột tên** (method impl vs method std cùng tên cùng kiểu) ⇒ **lỗi biên dịch** có gợi ý đổi tên, không im lặng chọn một bên.
**Ca đỏ-trước**: `p.tang()` trên `var p` chạy + đổi `x`; gọi trên `let p` ⇒ lỗi; `self.x = …` trong `fn f(self)` ⇒ lỗi có gợi ý; `Self` dùng được; trùng tên method impl/std ⇒ lỗi.
**Ảnh hưởng**: parser `impl`, tyck method lookup (inherent > std > trait), lower, docs; **0 test hiện có** bị ảnh hưởng (tính năng mới).
**Task**: **R14a** (3 h) `impl` + `self` + `Self` · **R14b** (2 h) `var self` + xung đột tên. **Phụ thuộc**: R11 (hàm dựng), R5 (method std).

### Mô hình ViScript — nguyên tắc nền (VD-174 mục 1–3) — **chỉ tài liệu**
1. **Không class, không kế thừa**; trừu tượng chỉ qua `type` + `trait` + `impl`; dùng lại bằng **method mặc định trong trait** hoặc **record chứa record**.
2. **Không lifetime, con trỏ, tham chiếu, `unsafe`, macro người dùng** (trước 1.0); sinh code chỉ qua **derives có sẵn**.
3. **Không tham chiếu dùng chung sửa được** (`Rc<RefCell>`) ở 1.0: trạng thái chung trong một luồng = **map + ID**; giữa luồng = **`Chan`**. Truyền theo giá trị ⇒ **không vòng tham chiếu**, đếm tham chiếu đủ.
**Việc**: Agent 0 ghi vào thiết kế; **không** task mã. (0 task, ~1 h tài liệu, 0 test.)

## R22 · Trait là kiểu (`list[Shape]`, gọi qua bảng ảo) — **Pha 2 (đầu, cùng R14)**
**Hiện trạng**: `trait Shape:` + `impl Shape for Circle:` **parse được** (probe `t1`: chạy, in `1`); nhưng **tên trait không phải kiểu**: `let xs: list[Shape] = []`, `let s: Shape = 5` ⇒ **VS0301** “kiểu `Shape` không có trong VS0 … và không được khai trong tệp” (`t6`,`t7`,`t8`) ⇒ **trait-as-type CHƯA CÓ**.
**Đặc tả (VD-174#4)**: trait dùng được làm **kiểu**; `list[Shape]` chứa **mọi kiểu cài `Shape`**; ViScript **tự đóng gói**, **không** cú pháp `Box`/`dyn`; gọi method **qua bảng ảo**.
- **Luật an toàn đối tượng**: trait dùng làm kiểu được khi mọi method của trait **không có generic riêng**, **không trả `Self`**, **không nhận tham số `Self`** (nếu vi phạm ⇒ lỗi biên dịch có gợi ý thêm `where Self: Sized` — hoặc tương đương).
- **Đóng gói tự động**: gán `Circle(…)`/`int` vào `Shape` ⇒ gói; gán ngược `Shape → Circle` ⇒ **lỗi tường minh** (không có downcast ở 1.0; dùng `match` nếu cần — ghi nợ).
- **Bảng ảo**: gọi `s.area()` trên `Shape` ⇒ dispatch theo kiểu thật.
**Ca đỏ-trước**: `list[Shape]` chứa **2 kiểu cài** (`Circle`, `Square`) + gọi `area()` qua vòng `for` ⇒ đúng kết quả từng kiểu; trait có method generic ⇒ lỗi an toàn đối tượng có gợi ý; `Shape → Circle` ⇒ lỗi.
**Ảnh hưởng**: `ty.rs` (trait là kiểu), tyck method dispatch, lower/VM (bảng ảo), docs; **0 test hiện có**.
**Task**: **R22a** (3 h) trait-as-type + an toàn đối tượng · **R22b** (3 h) 2 kiểu + bảng ảo + ca. **Phụ thuộc**: R14 (impl), R11.

## R23 · Gán trường lồng sâu trên `var` — **Pha 2 (đầu, cùng R14)**
**Hiện trạng**: `u.addr.city = "B"` ⇒ **VS0412** “gán **lồng** (`x.f[i]`/`x[i][j]`/`x.f.g`) **chưa hỗ trợ ở VS0** — gán lại cả biến `var` bằng `with`/giá trị mới (§4.1)” (`t2`) ⇒ **CHƯA CÓ**, thông điệp đã có sẵn.
**Đặc tả (VD-174#5)**: `u.addr.city = x` và `xs[i].name = x` **hợp lệ** trên biến `var`; **tự sao chép đúng đường dẫn** (copy-on-write từng tầng: sửa bản sao của `addr` rồi gán lại vào `u`), không tạo tham chiếu; trên `let` ⇒ lỗi như gán thường.
**Ca đỏ-trước**: `var u = {addr: {city: "A"}}` + `u.addr.city = "B"` ⇒ `u.addr.city == "B"`; `xs[0].name = "x"`; trên `let u` ⇒ lỗi; đọc trước/sau để chứng minh không alias.
**Ảnh hưởng**: tyck gán (bỏ chặn VS0412 cho đường lồng), lower/interp/VM (copy-on-path), docs §4.1; **0 test hiện có**.
**Task**: **R23a** (2 h) tyck + lower · **R23b** (1,5 h) ca + `xs[i].f` · **R23c** (0,5 h) lỗi trên `let`. **Phụ thuộc**: R5 (record theo tên), R2.

## R24 · Iterator lười + `.to_list()/.to_map()/.to_set()` — **Pha 2 (đầu, cùng R14) — chốt TRƯỚC R20**
**Hiện trạng**: `xs.filter(…)`, `xs.map(…)` **trả list ngay** (eager; `t3`,`t4`: `print(ys)` in list); `.to_list()` ⇒ **VS0403** (`t3`); `.take` ⇒ **VS0403** (`t4`); `for y in ys` chạy (`t5`) ⇒ **chưa có iterator**.
**Đặc tả (VD-174#6)**: chuỗi method trên `list`/`map`/`range` là **lười (iterator)**; tạo collection khi gọi **`.to_list()`/`.to_map()`/`.to_set()`** hoặc khi dùng trong **`for`**.
**Kiểu trả về chốt** (trước R20):
| Method | Trả về |
|---|---|
| `filter(f)` | `Iterator[T]` (lười) |
| `map(f)` | `Iterator[U]` (lười) |
| `take(n)` | `Iterator[T]` |
| `enumerate()` | `Iterator[(int, T)]` |
| `zip(other)` | `Iterator[(T, U)]` |
| `to_list()` / `.to_map()` / `.to_set()` | collection cụ thể, **vật chất hoá** |
**Ca đỏ-trước**: `xs.filter(f).map(g).take(2).to_list()` ⇒ đúng; `for x in xs.map(g):` ⇒ đúng; `xs.filter(f)` **không** in ra list được (phải `.to_list()` hoặc `for`); `zip` + `enumerate` đúng kiểu.
**Ảnh hưởng**: tyck method (kiểu iterator), interp/VM (đánh giá lười), `stdlib.rs` (thêm `to_*`), docs; **0 test hiện có** (nhưng **R20** phải dựng trên kiểu này — **chốt trước R20**).
**Task**: **R24a** (3 h) kiểu iterator + đánh giá lười · **R24b** (2 h) `to_*` + `for` · **R24c** (1 h) `enumerate/zip`. **Phụ thuộc**: R5, R20 chờ R24.

## R15 · Nối dòng + dấu phẩy cuối — **Pha 2**
**Hiện trạng**: gọi hàm nhiều dòng trong `()` **chạy** (`a7`: 3 dòng + dấu phẩy cuối); chuỗi `.method()` trên một dòng chạy (`d7`); nối dòng khi dòng mới bắt đầu `.`/`|`/`??`/`and`/`or` — **chưa xác minh** (riêng `|` đầu dòng trong script ⇒ VS0201, `d7`).
**Đặc tả**: tự nối trong `() [] {}`; nối khi dòng mới bắt đầu `.`, `|`, `??`, `and`, `or`; **dấu phẩy cuối** mọi nơi.
**Ca đỏ-trước**: `xs\n    .filter(...)\n    .len()`; `let x = a\n    ?? b`; `f(\n 1,\n 2,\n)`; `[1, 2,]`.
**Task**: **R15** (1,5 h). **Phụ thuộc**: —.

## R16 · `let` che tên; `var` không khai lại — **Pha 1**
**Hiện trạng**: `let x = 1; let x = "a"` ⇒ **VS0302** (`a8`) — cần **bỏ** cho `let`; `var x = 1; var x = 2` ⇒ **VS0302** (`a9`) — **đúng**.
**Đặc tả**: `let` **được che tên** trong cùng phạm vi; `var` **không** khai lại.
**Ca đỏ-trước**: `let x = 1; let x = "a"` chạy; `var x = 1; var x = 2` ⇒ lỗi.
**Task**: **R16** (1 h). **Phụ thuộc**: —.

## R17 · Điều kiện `if`/`while` chỉ `bool` + gợi ý — **Pha 1**
**Hiện trạng**: `if xs:` ⇒ **VS0412** “điều kiện `if`: cần `bool`, được `list[int]`” (`b1`) — **kiểu đúng**, **thiếu gợi ý** `xs.is_empty()`.
**Đặc tả**: chỉ `bool`; lỗi kèm **gợi ý** `xs.is_empty()` (với list/map/str).
**Ca đỏ-trước**: `if xs:` ⇒ VS0412 + `gợi ý: xs.is_empty()`; `if xs.is_empty():` chạy.
**Task**: **R17** (0,5 h). **Phụ thuộc**: R9 (dòng gợi ý).

## R18 · Pattern mở rộng + `while let` — **Pha 2**
**Hiện trạng**: literal + khoảng `1..=5` **chạy** (`b2`); tuple `(a, b)` **chạy** (`d3`); `as` ⇒ **VS0201** (`d4`); `while let` ⇒ **VS0201** (`d5`); record pattern `{name}` có trong test (`exhaustive.rs:151-163`).
**Đặc tả**: thêm record `User(name, age: a)`, `as`, **`while let`**; pattern đã có: literal, khoảng, tuple, record.
**Ca đỏ-trước**: `User(name, age: a)`; `v as x`; `while let v = o:`; `1..=5`.
**Task**: **R18a** (2 h) as + record pattern · **R18b** (1,5 h) while let. **Phụ thuộc**: R2.

## R19 · `0o755` · tràn i64 **panic** · bit method — **Pha 2 (R19a; R19b/R19c pha 1)**
**Hiện trạng**: `0o755` ⇒ **VS0201** (`b3`); tràn i64 runtime ⇒ **VS0501** “tràn i64 lúc chạy” (`p2`) — cần **panic**; `5.bit_or(3)` ⇒ **VS0104** (`b4`: lexer đọc `5.` là float) — **xung đột cú pháp** cần xử lý; bit method chưa có.
**Đặc tả**: `0o755`; tràn i64 **panic** (mã thoát 101); bit là **method** `bit_or/bit_and/bit_xor/shl/shr`.
**Ca đỏ-trước**: `0o755` ⇒ 493; `i64::MAX+1` ⇒ panic 101; `5.bit_or(3)` ⇒ 7; `5 . bit_or(3)` phải như nhau (không phụ thuộc khoảng trắng).
**Task**: **R19a** (1,5 h) 0o · **R19b** (1 h) panic · **R19c** (1 h) bit method + lexer. **Phụ thuộc**: R7.


### R19c · Float literal chỉ khi có chữ số sau `.` — **Pha 1**
**Đặc tả (VD-173#2)**: float literal **chỉ** khi ngay sau `.` là **chữ số**; `5.bit_or(3)` = **int + gọi method**; `5.0` là float; `1.` và `1.e3` **vẫn bị cấm** (giữ `VS0104`).
**Ca đỏ-trước**: `5.bit_or(3) == 7`; `5.0` là float (`5.0 + 1` ⇒ `6.0`); `1.` ⇒ VS0104; `1.e3` ⇒ VS0104; `5 . bit_or(3)` như nhau.
**Ảnh hưởng**: lexer `lit.rs` (FLOAT), 1 khối test; **không** đổi `VS0104` cho trường hợp cấm.
**Task**: **R19c** (1 h) lexer + ca. **Phụ thuộc**: R7.

## R20 · `assert` · `use as` · `use {}` · list/range bổ sung — **Pha 2**
**Hiện trạng**: `assert true, "m"` ⇒ **VS0201** (`b5`); `use std.fs as f` ⇒ **VS0201** (`b6`); `use std.fs.{exists}` ⇒ **VS0201** (`d6`); `xs.enumerate()` ⇒ **VS0403** (`b7`); `(1..5).step(2)` ⇒ **VS0403** (`b8`).
**Đặc tả**: `assert cond, "thông điệp"`; `use std.fs as f`; `use std.fs.{read, write}`; `list` thêm `enumerate, zip, any, all, find, sort_by, is_empty`; range thêm `.step(n)`.
**Ca đỏ-trước**: 5 dạng trên chạy; `assert false, "m"` ⇒ panic/thoát 101.
**Task**: **R20a** (2 h) assert + use · **R20b** (1 h) list/range. **Phụ thuộc**: R5.

## R21 · Pipeline `|` chỉ shell/cmd (ghi thiết kế) — **Pha 2**
**Hiện trạng**: `1 | 2` trong script ⇒ **VS0201** (`d7`) — **đúng** hướng; `|>` là pipe biểu thức script (`parse/expr.rs:119`).
**Đặc tả**: `|` chỉ trong **chế độ shell** và **khối `cmd`**; code thường dùng chuỗi method; hai bên **dùng chung hàm std**; ghi vào **thiết kế** (Agent 0 sở hữu).
**Ca đỏ-trước**: `1 | 2` ⇒ lỗi + gợi ý; `x |> f` chạy.
**Task**: **R21** (0,5 h, chỉ tài liệu). **Phụ thuộc**: —.

## Kiểm thêm: generic `f[int](x)` vs chỉ số
**Hiện trạng**: `f[int](1)` **chạy** (`b9`) — parser phân biệt được; **không** xung đột với `xs[i]`.
**Ghi chú**: xung đột thật nằm ở **`5.bit_or(3)`** (`b4`: `5.` thành float) — thuộc **R19c**.

## Thứ tự thực thi (VD-172 bổ sung)
… → **R5** → **R11** → **R12** → **R13** → **R14** → **R15** → **R16** → **R17** → **R18** → **R7** → **R19** → **R10** → **R20** → **R21** (chỉ sửa tài liệu).
**Cheat sheet**: cập nhật theo **toàn bộ R1–R21** sau khi thực thi; **mọi ví dụ là doc-test** (VD-170 P0 mục 3).
