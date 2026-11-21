# KT-312 — thu hồi **10.240 B** trong `Machine` (thu hẹp kiểu `Meta`), pin D2-84 **không** nới

Trạng thái: **DONE** (nhánh `task/kt312-machinesize`, base `main` @ `81b8e210`), 2026-11-22.
Sub-agent `machinesize`. Quyết định: **D2-84** (pin `size_of::<Machine>()` ≤ **196.608 B**, vượt
⇒ **tối ưu** chứ **không** nới), **D2-85c** (bảng phân bố phải **thật**, `padding ≤ 8`), **D2-05**
(test đỏ trước), **D2-198** (uỷ quyền commit trên nhánh task), **D2-04/D2-22** (`spec/` đóng
băng), **D2-17** (không `unsafe`).

**Ghi chú đọc trước**: `v2/agent/DECISIONS.md` trong worktree dừng ở **D2-220** — **không có**
mục **D2-221** mà phiếu nhắc. Đã đọc D2-84, D2-85 (a..d), D2-220. Không sửa tệp DECISIONS
(ngoài quyền của sub-agent).

**Không** tuyên bố K6 đạt lại. **Không** đổi `MAX_*` nào của sản phẩm.

## 1. Đo **trước** (bắt buộc) — bảng phân bố nguyên văn

```text
$ cargo test -p vifast-sim --test kt60_core_parity kt60_pin_dau_chan_d2_84 -- --nocapture
regions[16]                       384
objects[256]                    22528
caps[512]                       28672
slot_mem                        16384
harness_region                     24
tcbs[64]                        52224
tcb_free[64]                      128
scs[32]                          2048
sc_free[32]                        64
pair_hits[121]                    484
forbidden_hits[121]               484
finalise_log[256]               12288
sched[8]                        16832
mail[8]                         15744
published[8]                       16
endpoints[16]                    1280
endpoint_free[16]                  32
replies[32]                       256
reply_free[32]                     64
notifications[16]                 512
notification_free[16]              32
ipc_msgs[24]                    20352
ipc_msg_free[24]                   48
addr_words[64]                   1536
wait_nodes[64]                   1024
wait_free[64]                     128
wait_buckets[64]                  128
wait_buckets_tail[64]             128
tlb_pending[8]                    384
scalars                           311
padding                             1
TOTAL                          194520
```

Pin **196.608** ⇒ headroom **2.088 B** (đúng như phiếu).

## 2. Tìm lãng phí — **số đo** từng ứng viên

Đo `size_of` từng kiểu con bằng một test tạm (đã xoá trước commit):

| Kiểu | Byte | Ghi chú |
|---|---:|---|
| `Meta` | **88** | 3 × `Option<usize>` = **48 B** chỉ để chứa chỉ số < 256 |
| `Cap` | 56 | `Option<SlotRef>` 24 + `Option<u64>` 16 + `Option<CapId>` 8 + 4 + 3 |
| `TcbData` | 816 | `IpcArea` **544** (4 reg + 60 word buffer = ABI MR0..MR63) |
| `Message` | 816 | `mr[64]` **512** (ABI) + `snaps[4]` 128 + `extra_snaps[4]` 128 |
| `QueuedMsg` | 848 | `Message` 816 + `Option<FaultKind>` 24 |
| `HartSched` | 2104 | 3 × `[u16;256]` (mức ưu tiên) + `next[257]` + bitmap |
| `HartMailbox` | 1968 | hàng 8 × 48 + **nhật ký 32 × 48** |
| `FinaliseEntry` | 48 | `TcbState` **32** + `ObjId` 4 + `Option<ObjId>` 8 |
| `SlotMem` | 16384 | `MAX_SLOT_WORDS` 4096 × `u32` |

### 2.1 `padding` — **không** phải nguồn ≥ 8 KiB

`padding` tổng của `Machine` = **1 B** (đuôi `repr(Rust)`). Đệm đuôi từng struct: `Meta` 2,
`Cap` 1, `TcbData` 6, `Message` 5, `QueuedMsg` 1, `HartSched` 4 — **đều là đệm căn lề cuối**,
sắp xếp lại trường **không** bỏ được (đó là bội số của `align_of`). ⇒ loại ứng viên này.

### 2.2 Sức chứa `MAX_*` — **không** có hằng nào chứng minh được là dư

| Vùng | Byte | Vì sao **không** hạ |
|---|---:|---|
| `tcbs[64]` | 52224 | `MAX_TCBS = 64` là biên thiết kế; `TcbData` 816 B chứa `IpcArea` 544 B = **ABI** MR0..MR63 |
| `caps[512]` | 28672 | `MAX_CAPS = 512`; mỗi cap 56 B, **không** đệm trong |
| `slot_mem` | 16384 | `MAX_SLOT_WORDS` 4096 word × 4 B; `HARNESS_SLOT_WORDS` 1024 + arena 3072 |
| `ipc_msgs[24]` | 20352 | `Message.mr[64]` 512 B là **ABI**; pool 24 đã là biên **nhỏ hơn** 8 × 16 Endpoint |
| `sched[8]` | 16832 | 256 mức ưu tiên × (head+tail+len_at) là thiết kế §4.8 |
| `mail[8]` | 15744 | `MAX_MAIL_LOG` **đã** bị D2-85(b) hạ 64 → 32; hạ nữa là **đổi ngữ nghĩa** nhật ký |
| `finalise_log[256]` | 12288 | `MAX_FINALISE = MAX_OBJECTS`; **đạt được** (256 lượt finalise TCB trong một `Machine`) |

⇒ **Không** hạ `MAX_*` nào (đúng luật phiếu: *"không tự hạ sức chứa không có bằng chứng"*).
Lãng phí **có bằng chứng** nằm ở **kiểu trường**, không ở sức chứa.

### 2.3 Ứng viên **thắng**: `Meta` — 3 trường chỉ số dùng `Option<usize>`

`Meta` = **88 B**; offset đo được: `untyped` 0 (16 B), `region` 16 (16 B), `slots_meta` 32
(16 B), `offset` 48, `len` 56, `slot_len` 64, `slot_base` 72, `body` 80, `alive` 84, `ty` 85.
**48 B trong 88 B** là ba `Option<usize>` — mà miền giá trị **chứng minh được**:

| Trường | Nghĩa | Miền giá trị **thật** | Kiểu hẹp |
|---|---|---|---|
| `untyped` | chỉ số **object** của Untyped chứa object | `< MAX_OBJECTS = 256` | `Option<u8>` (2 B) |
| `region` | chỉ số arena trong `Machine::regions` | `< MAX_UNTYPEDS = 16` | `Option<u8>` (2 B) |
| `slots_meta` | số slot CNode | `≤ MAX_SLOT_WORDS = 4096` (harness ≤ 1024, arena ≤ 192) | `Option<u16>` (4 B) |

`Option<usize>` **16 B** vì `usize` không có niche; `Option<u8>`/`Option<u16>` dùng niche
⇒ **2 B**/**4 B**, **cùng** ngữ nghĩa `Some`/`None` và **cùng** giá trị.

**Sai lầm đã bắt được nhờ test (ghi lại)**: bản nháp đầu đặt `debug_assert!(u < MAX_UNTYPEDS)`
cho `untyped` — **sai**: `untyped` là chỉ số **object** (`object_index()`), không phải chỉ số
region. Test cực đại §3.2 bắt ngay (`untyped 16 >= MAX_UNTYPEDS` khi dựng 16 Untyped). Đã sửa
thành `< MAX_OBJECTS`.

## 3. Thay đổi

### 3.1 `Meta` 88 → **48 B** (`v2/kernel/vifast-core/src/objmeta.rs`)

```rust
pub(crate) untyped: Option<u8>,     // < MAX_OBJECTS = 256
pub(crate) region: Option<u8>,      // < MAX_UNTYPEDS = 16
pub(crate) slots_meta: Option<u16>, // ≤ MAX_SLOT_WORDS = 4096
```

`push_meta` giữ **nguyên** chữ ký `Option<usize>` (mọi caller không đổi) và thu hẹp **một chỗ**
duy nhất, có `debug_assert` miền giá trị. Sáu chỗ đọc đổi sang `map(usize::from)`/`as usize`.

`BodyRef`/`Meta`/`Cap`/`FinaliseEntry` + mã `BODY_*` **chuyển sang module mới `objmeta.rs`** để
giữ **R1** (≤ 1.500 dòng/tệp — `machine.rs` đang đúng 1.500); `machine.rs` **tái xuất đúng tên
cũ** nên **không** tệp nào phải sửa import. `machine.rs` 1.500 → **1.423** dòng.

### 3.2 Test mới (đỏ trước — D2-05)

| Test | Trước | Sau |
|---|---|---|
| `footprint::tests::kt312_meta_khong_dem_thua` | **ĐỎ** (`Meta` = 88 ≠ 48) | XANH |
| `footprint::tests::kt312_meta_gia_tri_cuc_dai_van_vua_kieu_hep` | — | XANH |

Test thứ hai dựng **16 Untyped** (chỉ số region 15), retype TCB từ Untyped **cuối**
(`untyped = 15`) và CNode harness **1000 slot** (`slots_meta = 1000 > u8::MAX`) rồi đọc lại qua
`untyped_arena`/`object_bytes`/`cnode_slot_count` — chứng minh kiểu hẹp **không cắt** giá trị.

### 3.3 Test pin cập nhật (không nới ngưỡng)

- `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb`: phép tính đổi sang **đại số có dấu**
  (`isize`) và thêm khoản **giảm** `KT312_GIAM_MOI_META = 40 × MAX_OBJECTS = 10.240`;
  thêm `assert_eq!(size_of::<Meta>(), 48)`.
- `kt60_pin_dau_chan_d2_84`: thêm `assert_eq!(f.bytes[1], 256 * 48)` — dòng `objects[256]` của
  bảng được **ghim**, không còn là lời khai. `scalars` **311 không đổi** (không thêm/bớt trường
  vô hướng).
- `v2/test/boot/ktest.sh`: 5 chỗ `194520` → **`184280`** (4 dòng evidence).

## 4. Đo **sau** — bảng phân bố nguyên văn

```text
$ cargo test -p vifast-sim --test kt60_core_parity kt60_pin_dau_chan_d2_84 -- --nocapture
regions[16]                       384
objects[256]                    12288      ← 22528 − 10240
caps[512]                       28672
slot_mem                        16384
harness_region                     24
tcbs[64]                        52224
tcb_free[64]                      128
scs[32]                          2048
sc_free[32]                        64
pair_hits[121]                    484
forbidden_hits[121]               484
finalise_log[256]               12288
sched[8]                        16832
mail[8]                         15744
published[8]                       16
endpoints[16]                    1280
endpoint_free[16]                  32
replies[32]                       256
reply_free[32]                     64
notifications[16]                 512
notification_free[16]              32
ipc_msgs[24]                    20352
ipc_msg_free[24]                   48
addr_words[64]                   1536
wait_nodes[64]                   1024
wait_free[64]                     128
wait_buckets[64]                  128
wait_buckets_tail[64]             128
tlb_pending[8]                    384
scalars                           311
padding                             1
TOTAL                          184280
```

| | Trước | Sau |
|---|---:|---:|
| `size_of::<Machine>()` | 194.520 | **184.280** |
| Pin D2-84 | 196.608 | **196.608** (không đổi) |
| Headroom | 2.088 | **12.328** |
| Thu hồi | — | **10.240 B** (≥ 8 KiB ✔) |

## 5. Đột biến

| # | Đột biến | Kỳ vọng | Kết quả |
|---|---|---|---|
| **M1** | `MAX_ENDPOINTS` 16 → 32 | pin **vẫn xanh**, **bảng đổi** | `endpoints[16] 1280 → 2560`, `endpoint_free[16] 32 → 64`, `TOTAL 184280 → 185592`; pin test **XANH** ⇒ bảng là **thật** |
| **M2** | `MAX_TCBS` 64 → 80 | pin **ĐỎ** (chứng minh pin có răng) | `tcbs[64] 52224 → 65280`, `TOTAL 197368` ⇒ **ĐỎ**: `D2-84: size_of::<Machine>() = 197368 > 196608` |
| **M3** | `push_meta` luôn ghi `untyped: None` (bỏ giá trị trường) | sim **ĐỎ** (chứng minh trường được dùng) | **ĐỎ 3/18**: `kt64a_parity_finalise_recv_waiter`, `kt64a_parity_finalise_notification_waiter`, `kt63_parity_revoke_finalise_caller_va_sender` — `digest lệch` ở bước `revoke cay Untyped 2` |

Cả ba đột biến đã **revert**; cây sạch sau mỗi lần.

## 6. Cổng

| Việc | Kết quả |
|---|---|
| `cargo test -p vifast-core` | **XANH — 63/63** (2 test KT-312 mới) |
| `cargo test -p vifast-sim` | **XANH** (toàn bộ 40 nhóm test, gồm `l1_1000_seed_tat_dinh` + `l1_k1c`) |
| `cargo fmt --all` (cả hai workspace) | đã chạy |
| `./v2/bin/v2 check` | **XANH** (exit 0) |
| `bash v2/test/boot/ktest.sh 1 1` | **XANH — 9/9 boot sạch** (smp=1, HEAD `d19e675e`, cây sạch) |

Log QEMU nguyên văn (bằng chứng `ktest.sh` đã cập nhật đúng số đo **thật**):

```text
# core: link addr=0x80255800 bytes=184280 pin=196608 image_cap=262144 parts=29 sum=184280 scalars=311 padding=1 sum_ok=1 in_image=1
# core: phase fails=0 static=184280 pin=196608 image_cap=262144
# user core: static=184280 pin=196608 image_cap=262144 untyped=65536 harts=1 tcb=11 ep=12 cspace=2 state=Running current=1 check=1
```

```text
# KTEST: 9/9 boot sạch (smp=1, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

## 7. Còn gì chưa xong / rủi ro

1. **Tên trong bảng phân bố là chuỗi hằng**, không sinh từ `MAX_*`: đột biến **M1** cho thấy
   `bytes` đổi đúng nhưng **tên** vẫn in `endpoints[16]`. Đây là khuyết điểm **có trước** KT-312
   (không phải hồi quy); `bytes` mới là thứ pin kiểm. **Đề xuất**: một task nhỏ sinh `names` từ
   hằng (hoặc thêm `assert_eq!(f.names[i], …)` cho từng dòng) — **không** làm trong task này.
2. **Ứng viên thu hồi tiếp** (chưa làm, cần đo thêm): `Cap.badge: Option<u64>` (16 B) →
   `u64` + `bool` ⇒ `Cap` 56 → 48 B (**−4.096 B**); `CapSnapshot.badge` cùng cách ⇒ `Message`
   816 → 752 B (**−1.536 B** trên `ipc_msgs[24]`). Tổng **−5.632 B** nếu làm cả hai; **không**
   làm vì (a) ngoài ngân sách ≥ 8 KiB đã đạt, (b) chạm nhiều điểm đọc `badge` hơn.
3. **`finalise_log[256]` = 12.288 B** là vùng lớn thứ ba còn dư **tiềm năng** (`FinaliseEntry`
   48 B, trong đó `TcbState` **32 B** mà sim **luôn** ghi `Dead`) — nhưng bỏ trường là **bỏ tính
   năng** (đổi API `pub`) ⇒ **không** làm; ghi thành **đề xuất** cho lead.
4. **Chưa chạy T2 `gate --fast`** (phiếu yêu cầu `v2 check` + `cargo test -p vifast-sim`; T2 là
   việc của lead trên ứng viên tích hợp).
5. **Không** tuyên bố K6 đạt lại.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt312-machinesize`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới pin `196.608`, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common`, **không** `unsafe` mới.
- **Không** đổi `MAX_*` nào của sản phẩm.
- Stage theo **đường dẫn cụ thể**, **không** `git add -A`.
