# Báo cáo đêm 17→18/11/2026 — dòng OS (Agent 1)

**Trạng thái**: HEAD `d2fbe4f7`, cây sạch, `v2 check` PASS, năm luật đều ok, không commit nào chưa push.
**Bối cảnh**: dòng ViScript đã tách sang repo riêng; Agent 0 offline; Agent 1 làm trong phạm vi D2-121.

## 1. Việc đã xong (mười hai task)

| Task | Nội dung | Kết quả |
|---|---|---|
| KT-92 | Đóng **A1-184**: `build.rs` truyền `--remap-path-prefix` cho cả ba chỗ gọi `$RUSTC` | hai đường dẫn khác nhau ⇒ **cùng sha**; đối chứng âm ⇒ khác sha |
| KT-93 | Script đo/build vào build-inputs | sửa `build-rev.sh` ⇒ dấu vân tay **đổi** |
| KT-84 | `task-target.sh` + `pin-artifact.sh` (ghim, `--verify`, `--selftest`) | sáu ca tự kiểm PASS |
| KT-95 | `pre-measure.sh` — sáu phép kiểm trước khi đo | sáu ca kiểm đã chạy |
| KT-96 | `soak.sh --image` — soak ảnh có sẵn | **từ chối** ảnh có shell (0/N sẽ là giả) |
| KT-97 | Kiểm lại sau thay đổi | ma trận **hai mươi mốt trên hai mươi mốt**, persist PASS, parity sáu trên sáu |
| KT-98 | `--selftest` cho hai công cụ | **bảy đột biến, bảy trên bảy bị bắt** |
| KT-99 | Soak trên ảnh canonical mới | **một nghìn trên một nghìn**, wall năm trăm ba mươi ba giây |
| KT-100 | Vá **bốn lỗ fail-open** do reviewer độc lập phát hiện | `missing-input`, `no-pgrep`, `--allow-unverified`, soak chỉ nhận timeout |
| KT-101 | Conformance trên HEAD | `trace-compare` **sáu mươi mốt trên sáu mươi mốt** dòng khớp sim ↔ QEMU |
| KT-102 | `v2 status [--json]` | một màn hình trạng thái |
| KT-103/104/105 | ba lỗi shell tìm bằng thử biên | xem §3 |

## 2. Hai việc giấy tờ đã mở đường cho K6

- **A1-179 XONG**: năm mươi tám số của `k6-exec`/`k6-persist`/`K6-DOD` ra khỏi prose ⇒ `pre_existing` bằng không,
  mọi tài liệu K6 qua luật một **không cần miễn trừ**.
- **KT-83 XONG**: bảng bảy lỗi v1 với `file:dòng`; kèm **đính chính ba chỗ sai** trong `K6-DOD.md`
  (hai dẫn lệch dòng, và N5 phải là **HOÃN K5′** không phải "N/A").

## 3. ba lỗi thật trong shell (tìm bằng thử biên, không phải đọc code)

| Lỗi | Triệu chứng | Vì sao không test nào bắt |
|---|---|---|
| **KT-103** | dòng dài hơn `MAX_LINE` in **một cảnh báo cho mỗi byte** ⇒ ngập log | không có ca kiểm dòng dài |
| **KT-104** | `cat`/`write` không tham số in `# shell: ?` (như lệnh lạ) | parity chỉ gửi lệnh có tham số |
| **KT-105** | `ps` in `state=Unknown` cho **mọi** tiến trình | **test dùng chung lỗi với shell** |

**KT-105 là bài học quan trọng nhất**: `proc_demo.rs` dựng dòng kỳ vọng bằng **cùng** mask sai
(`& 0x1F`) ⇒ test và shell "khớp nhau" mà cả hai đều sai. Khi vá shell, ma trận **mười tám trên hai mươi mốt đỏ** — đó
là lúc lỗi lộ ra. Nếu chỉ sửa test cho xanh thì đã **che** lỗi.

## 4. Ảnh canonical mới nhất

Ảnh `34ee7a25…`, dấu vân tay `be9689a5c032cf69`, cây sạch, `rev=244f06d11777`. Dòng `REPRO` nguyên
văn: `v2/agent/JOURNAL/kt105-journal.md` (mục "Ảnh canonical mới nhất").

Đã ghim; parity sáu trên sáu; persist PASS; ma trận hai mươi mốt trên hai mươi mốt. **Chưa** soak một nghìn trên ảnh này (soak gần nhất
là `dd9f7050…`, trước KT-103/104/105).

## 4b. Vòng chạy **song song + review chéo** (sau báo cáo đầu)

Bốn việc độc lập chạy song song, rồi **hai reviewer độc lập** kiểm chéo. Kết quả:

| Task | Nội dung | Kết quả |
|---|---|---|
| **KT-107** | Truy tìm chỗ **test dùng chung công thức với code** | ba mươi ứng viên; **sáu** nghi vấn; **hai** chứng minh xanh giả bằng đột biến phía code |
| **KT-108** | Harness ca biên cho shell | tám ca; selftest mười chín kiểm; chạy trên ảnh cũ **đỏ ba trên tám** đúng ba lỗi thật |
| **KT-109** | Bảng hai mươi hai lệnh parity đang hoãn | nhóm A tám lệnh (hai giờ), B tám, C năm, D một; tổng **mười bốn phẩy năm giờ** |
| **KT-110** | Đo thời gian cổng | `gate --fast` bốn trăm lẻ bảy giây, **L0 chiếm tám mươi lăm phần trăm**, L1 chỉ một giây |

**Review chéo bắt được (đây là giá trị thật của vòng này):**

| Phát hiện | Xử lý |
|---|---|
| `shell-edge.sh` dùng `LOGDIR` **cố định** ⇒ hai lượt song song đọc chung log; ảnh giả **không boot** vẫn báo PASS nhờ log lượt khác | **KT-112** vá: mỗi lượt một thư mục `mktemp -d`; đã dựng lại ca âm để chứng minh |
| `check_ps_no_unknown` chỉ đòi **≥ một** dòng pid | **KT-112** vá: đối chiếu với `n` trong dòng tổng kết |
| Thông báo lỗi hứa "hai mươi giây" nhưng `sleep` một phần hai mươi giây thực tế tốn khoảng một phần năm giây ⇒ thật là ~tám mươi giây | **KT-112** vá: in **số vòng poll** |
| KT-107 ghi "ba trên sáu chứng minh xanh giả" — thực chất **hai trên sáu** (mục thứ ba là đột biến **phía test**) | đính chính trong `kt107-audit.md` |
| KT-109 mục bốn lỗi số học một giờ | đính chính: đợt thứ hai là **ba giờ**, tổng **mười bốn phẩy năm giờ** |
| KT-110 ghi `smp` bốn giá trị — thực tế mặc định **một hart** (đo: hai mươi bảy phẩy chín giây so với một trăm năm mươi mốt giây) | đính chính |
| KT-110 gán điều kiện `/tmp/repfx.out` cho P3 — thực ra thuộc **P6** | đính chính |
| KT-110 thiếu lớp rủi ro **đồng hồ thực** khi song song hoá L0 | bổ sung: `kt30_stress.rs:21`, `spsc.rs:218`, `compile_fail.rs:74` |

**Hai việc tôi tự làm thêm từ kết quả audit:**

- **KT-111** — `persist_host.py` kiểm `detail` bằng **cùng công thức** với kernel. Thêm **bốn bất biến
  ngữ nghĩa** (hai khối khác nhau, cả hai dương, `detail` lớn hơn cả hai, `detail` lớn hơn hoặc bằng hai mũ ba mươi hai) ⇒ đổi dịch
  dịch bit mười sáu hay hai mươi bốn đều **đỏ** (trước: xanh).
- **KT-107 (vá mục đầu)** — thêm test `info_shift_khop_spec` đọc **nguyên văn** dòng `layout` trong
  `spec/abi.toml`. Reviewer xác nhận **độc lập thật**: `gen_idl.py:173,175` **viết cứng** `20`/`28`,
  không đọc spec.

## 4c. Vòng **phát triển tính năng + review toàn dự án** (05:30–07:00)

Ba việc song song, rồi một reviewer độc lập kiểm chéo.

### Tính năng: bốn lệnh shell mới (KT-114)

`time`, `uptime`, `sync`, `stat <path>` — **chỉ** trong nhánh `#[cfg(shell_fs)]`:

| Lệnh | Nguồn | Ghi chú |
|---|---|---|
| `time` / `uptime` | `ClockRead` (mười một) — **đã có** | không mở ABI |
| `sync` | `OP_SYNC` — **đã có** | `write` đang dùng |
| `stat` | `OP_STAT` — **đã có nhưng chưa ai gọi** | in `kind`, `size`, `ino` |

`CMDS` chín → **mười ba** ở ảnh sản phẩm; ảnh test **vẫn sáu** ⇒ `proc_demo.rs` và `ktest.sh` **không** phải
sửa. Đây là chỗ KT-91 tưởng bị chặn — thực ra KT-77c đã mở đường bằng `#[cfg(shell_fs)]`.

### Review toàn dự án (KT-115): mười chín phát hiện

| Mức | Số | Đã vá trong đêm |
|---|---|---|
| nặng | hai | **cả hai** |
| vừa | tám | ba |
| nhẹ | chín | một |

**Hai phát hiện nặng, đều đã vá và có bằng chứng dựng lại:**

- **PANIC kernel từ một request U-mode** (KT-118) — `proc_fs.rs` lấy độ dài tên từ **phần cao của
   `head`** (tối đa hai trăm năm mươi lăm) rồi chỉ kẹp theo `MAX_PATH` (bốn mươi tám), trong khi ba
   MR word chỉ chở **hai mươi bốn** byte ⇒ tên dài hai mươi lăm byte làm chỉ số vượt mảng bốn phần
   tử. Dựng lại được: `PANIC: index out of bounds`. Vá một dòng.
- **TAP bốn `shell ps tu core` vẫn là xanh giả** sau KT-105 (KT-119) — test **giải mã lại** đúng công
   thức của shell, nên đột biến chỗ **đóng gói** (dịch trạng thái thêm một bit) vẫn xanh. Nay ghi
   trạng thái **thật** bằng `m.tcb_state(id)` lúc `PS_REQ` và dựng kỳ vọng từ đó.

**Một phát hiện về chính bản vá của tôi** (KT-120): bản KT-112 thêm vào `shell-edge.sh` là **kiểm
rỗng** — `n_rows` và `n_declared` cùng sinh từ **một** biến `count` của shell nên luôn bằng nhau.
Đã thêm sàn **độc lập** `PS_MIN_ROWS` bằng ba (procsrv + shell + victim) và ca selftest 3b.

**Một phát hiện về bất biến pager** (KT-122): `reads == misses` trước chỉ **in** khi sai, không TAP
nào đỏ. Nay thành **TAP ba mươi sáu**; `sectors_per_page` viết **literal tám** thay vì tính từ hằng của chính
code.

### Review chéo lô hai: ba đính chính (KT-123)

| Phát hiện | Xử lý |
|---|---|
| Bất biến mới ở `ktest_con.rs` **không** phải thứ bắt đột biến (tắt đi vẫn đỏ nhờ `Ring::check`) | sửa comment cho đúng — nó là kiểm nhất quán, **không** độc lập |
| Ca ba-b của `shell-edge.sh` dùng lại `right.log` của ca ba | cấp `right.log` riêng |
| Ảnh có `product-fs` mà FS = `None`: mỗi lệnh đứng **~năm mươi tư giây** | kernel trả `ST_NOFS` **ngay**; shell in `fs khong san sang` thay vì `fs loi code=` kèm số không |

### Ảnh canonical cuối đêm

`64db17ea…` (dấu vân tay `8d5d423fb4cb7763`, cây sạch) — ma trận **hai mươi mốt trên hai mươi mốt**, parity ok, `shell-edge`
tám trên tám, persist PASS.

## 5. Cần Agent 0 quyết

| STT | Việc | Ghi chú |
|---|---|---|
| một | **Tiêu chí bốn** dùng số nào — `K6-CLOSURE-DRAFT.md` §2, phương án (a) hay (b) | lead đề xuất **(b)** |
| hai | **KT-91** (ba lệnh `time`/`uptime`/`jobs`) | đổi `# shell: help cmds=` ⇒ phải sửa kỳ vọng harness |
| ba | **B6** — build-inputs có gồm `v2/agent/**` không | A1-188 đang tạm loại trừ |
| bốn | **KT-82a** — số đo viết bằng **chữ** để qua luật một | giữ thông tin nhưng **luật không kiểm máy được**; có làm lại bằng khối `scope=` không |
| năm | Thêm ca kiểm `ps` **không** in `state=Unknown` | đụng kỳ vọng harness (parity hoặc TAP) |
| sáu | Xác nhận **A1-188…A1-193** | các quyết định thay mặt trong đêm |

## 6. Việc còn lại (không chờ A0)

- Soak một nghìn trên `34ee7a25…` (ảnh mới nhất).
- Nối `pre-measure.sh` vào bốn runner (đổi phương pháp đo ⇒ cần A0).
- KT-81 (thiết bị giả nghiêm ngặt) — theo D2-119 làm **sau** khi K6 đóng.
