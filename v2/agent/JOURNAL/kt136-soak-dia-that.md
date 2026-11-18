# KT-136 — soak **đĩa thật** 100 boot + cắt điện giữa lúc ghi

**Việc Agent 0 giao (phiếu lượt 4, mục "Thứ tự", việc 2, nguyên văn):**

> 2. KT-136 soak đĩa thật

Và trong phiếu lượt 3 (C5), Agent 0 đã chốt nội dung:

> mỗi vòng ghi → reboot → đọc lại → fsck sạch; thêm ≥ 1 ca kill QEMU giữa lúc ghi ⇒ boot sau phải
> phục hồi nhất quán

Đây là **nợ K6 số hai**: soak 1.000 vòng của KT-57 chạy trên **RAM** (không chạm đĩa thật), còn
`persist.sh` chạm đĩa thật nhưng chỉ **10 boot**. Tệp này ghi kết quả.

## 1. Đã thêm gì

| Tệp | Thay đổi |
|---|---|
| `v2/test/boot/persist.sh` | hai chế độ mới: `--soak N` (N vòng × 10 boot) và `--crash` (cắt điện giữa lúc ghi) |
| `v2/test/boot/persist_host.py` | lệnh con `crash-host`: kiểm ảnh đĩa **sau** khi bị cắt điện |
| `v2/test/boot/qemu_guard.sh` | tách `qg_start` khỏi `qg_boot`, thêm `qg_boot_kill` (giết cả process group ở thời điểm chọn trước) |
| `v2/bin/v2` | `gate --fast` gọi thêm `persist.sh --crash` (~17 s) |

**Vì sao tách `qg_start`:** `qg_boot` luôn chờ tới khi QEMU tự thoát (hoặc hết ngân sách), nên không
dùng được cho ca "giết ở một thời điểm chọn trước". Hai hàm dùng **cùng một** đường khởi động ⇒
không có đường thứ hai lệch nhau về `setpgrp`/đo pgid. `guard_selftest.sh` vẫn `cases=10 ok=10`.

## 2. Soak 100 boot — bất biến **mạnh hơn** "chạy không lỗi"

Mỗi vòng: xoá đĩa → 10 boot (7 pha ghi/đọc lại + 2 ca âm) → sau **mỗi** boot kiểm host độc lập
(`fsutil fsck` + `ls` + `stat` + `get`, so **từng byte** với mô hình sinh lại trên host).

Bất biến thêm: **mọi vòng phải cho cùng một sha256 ảnh đĩa**. Payload là hàm của **số pha**, không
của nonce ⇒ hai vòng cùng dãy thao tác phải cho cùng byte trên đĩa. Lệch ⇒ có **tích luỹ** giữa các
vòng (block/inode không được trả lại, journal không được dọn, bitmap rò). Đây là bất biến **chặt
hơn** "100 lần không lỗi": một FS rò tài nguyên vẫn có thể xanh 100 lần.

Lệnh:

```bash
KT75_LOGDIR=/tmp/kt136-soak bash v2/test/boot/persist.sh --soak 10
```

Kết quả — dán nguyên văn từ `soak-100-boot.log`:

```text
# soak: cycles=10 boots_per_cycle=10 tổng_boot=100
# soak: cycle=1 boots=10 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=2 boots=20 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=3 boots=30 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=4 boots=40 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=5 boots=50 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=6 boots=60 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=7 boots=70 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=8 boots=80 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=9 boots=90 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycle=10 boots=100 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389
# soak: cycles=10 sha_all_equal=1 bad_cycles=0
```

```text
# persist.sh: PASS
```

**Đọc kết quả:** 100 boot, 10 vòng, **một** sha256 duy nhất cho cả 10 vòng ⇒ không có tích luỹ.
Mỗi boot đều qua `fsutil fsck` + so từng byte phía host (không tin lời khai của kernel).

## 3. Cắt điện giữa lúc ghi

Kịch bản: chạy b01..b06 cho `/p5.bin` (payload pha 6) **đã commit**; rồi khởi động b07 (đọc lại pha
6 + ghi pha 7 = 144 block) và **giết cả process group** ngay khi log vừa in dòng
`phase=6 action=verify` — tức là đang ở **giữa** pha 7, pha ghi lớn nhất.

Lệnh:

```bash
KT75_LOGDIR=/tmp/kt136-crash bash v2/test/boot/persist.sh --crash
```

Kết quả — dán nguyên văn từ `crash-giua-luc-ghi.log`:

```text
# crash: đã giết group pid=30015 ngay sau dòng verify pha 6 (đang ghi pha 7)
# crash file: name=/p5.bin bytes=1536 want=1536 match=1 const:committed=1
# crash file: name=/p1.txt absent=1 const:want_absent=1
# crash file: name=/p7.bin bytes=23040 want_max=73728 prefix_ok=1 partial=1
# crash state: records=12 well_formed=1
# crash host: fsck=1 fail=0 img_sha256=60523e761ce4dfa2b67131255305f8d429aae26918c4fca756190412e2480c4f fsutil_mutated=0
# crash: ảnh đĩa sau cắt điện — fsck sạch, dữ liệu đã commit còn nguyên
# crash: boot sau cắt điện ĐỌC được dữ liệu đã commit (verify pha 6 xanh)
# crash: ghim — ghi pha 7 sau cắt điện HỎNG vì /p7.bin đã tồn tại dở (create là tạo độc quyền)
# crash: ghim — journal của FS là theo TỪNG THAO TÁC, không theo giao dịch; cắt giữa lúc ghi để lại tệp dở
# crash: boot sau cắt điện vẫn GHI được (tạo /p1.txt mới, TAP xanh)
# crash: fsck lần cuối sạch
```

```text
# persist.sh: PASS
```

**Bốn điều được kiểm, và điều gì đã học được:**

1. **`fsck` sạch** sau cắt điện — không có cấu trúc hỏng.
2. **Dữ liệu đã commit còn nguyên từng byte** — `/p5.bin` khớp mô hình, `/p1.txt` vẫn vắng (đã bị
   rename đi ở pha 5, không "sống lại").
3. **Tệp ghi dở là một TIỀN TỐ của nội dung đúng** — `/p7.bin` dừng ở 23040 B trên tổng 73728 B,
   `prefix_ok=1`. Không có byte rác. (Ba lần chạy cho ba cỡ khác nhau — 22528, 29696, 44544,
   23040 — vì thời điểm giết phụ thuộc tốc độ máy; cả bốn lần đều là tiền tố hợp lệ.)
4. **FS còn đọc VÀ ghi được** sau cắt điện — boot sau đọc được pha 6, và tạo được `/p1.txt` mới.

**Phát hiện — ghim lại, không sửa:** sau khi cắt giữa lúc ghi, `/p7.bin` **tồn tại dở**, nên lần
boot sau **không tạo lại được** nó: `fs.create` là **tạo độc quyền** (trả lỗi khi tệp đã có), đúng
ngữ nghĩa `create`. Đây **không** phải hỏng FS — nó là hệ quả của việc journal của FS là **theo
từng thao tác**, không phải theo giao dịch: một lần cắt giữa lúc ghi để lại tệp dở, và ứng dụng
muốn ghi lại thì phải `truncate` trước (hoặc dùng cờ mở tương đương `O_TRUNC`).

Điều này **không** làm mất dữ liệu đã commit, nên theo luật Agent 0 đặt cho nhóm ưu tiên K7
("hành vi nào làm hỏng/mất dữ liệu thì báo trước khi sửa"), tôi **không** sửa gì — chỉ ghim hành vi
thật và báo ở đây. Nếu Agent 0 muốn `create` ghi đè được (hoặc muốn thêm `truncate` vào đường ghi
của ktest), đó là quyết định của Agent 0.

## 4. Đột biến — chứng minh bộ kiểm không rỗng

| Đột biến | Kết quả |
|---|---|
| `fresh_disk` **không** xoá đĩa (vòng sau thừa hưởng trạng thái vòng trước) | **ĐỎ**: `not ok 1 - persist format: format hỏng`, `fsutil ls /: mount: Invalid` — bắt ở boot đầu của vòng 2, **trước** khi tới phép so sha |
| chỉ vòng 1 xoá đĩa (vòng 2 chạy trên đĩa đã có FS) | **ĐỎ**: cùng lý do — `format hỏng` |
| mốc sha bị đặt sai (`sha1` = toàn số 0) | **ĐỎ**: `not ok - soak: cycle=1 sha ảnh đĩa KHÁC cycle=1 …`, `bad_cycles=2`, exit 1 |

Hai đột biến đầu bị bắt **sớm hơn** phép so sha (ở `format`), nên chúng chứng minh soak bắt được
"trạng thái vòng trước rò sang vòng sau", **không** chứng minh riêng phép so sha có tác dụng. Đột
biến thứ ba chứng minh riêng nhánh so sha: nó in đúng dòng `not ok` và trả exit 1.

## 5. Cái **không** có trong bằng chứng này

- **Không** phải soak 1.000 vòng. Agent 0 yêu cầu 100 boot; đây là 100 boot. Soak 1.000 vòng của
  KT-57 vẫn là phép đo **RAM**, không thay thế nhau.
- **Không** đo hiệu năng (không có ngưỡng thời gian cho soak này). `--soak 10` mất khoảng ba phút
  trên máy này; con số đó **không** phải ngưỡng, chỉ là thời gian chạy.
- **Không** chứng minh FS chịu được **mất điện thật** (rút điện, đĩa mất cache). QEMU ghi thẳng
  xuống file trên host, nên "cắt điện" ở đây = **giết tiến trình QEMU** giữa lúc ghi. Mô hình đĩa
  bền-theo-thứ-tự đầy đủ vẫn chỉ có trong `vifast-fs/src/crash.rs` (test host).
- **Không** chạy `--soak` trong `gate --fast` (ba phút là quá lâu cho cổng). `--crash` **có** trong
  `gate --fast` (~17 s).
