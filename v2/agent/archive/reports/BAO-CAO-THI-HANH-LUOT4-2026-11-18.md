# BÁO CÁO THI HÀNH — PHIẾU LƯỢT BỐN CỦA AGENT 0

**Ngày**: 2026-11-18 · **Người thực hiện**: lead (Agent 2 dòng OS) · **Repo**: `vifast-oss/ViFastOs`
**Phiếu gốc**: `PHIEU-QUYET-DINH-GUI-AGENT0-2026-11-18.md` (lượt bốn)

Tệp này **tự đủ**: không cần đọc mã nguồn, không cần mở repo. Mọi con số đều kèm **lệnh đã chạy**
và **nguyên văn dòng log** (luật 1 của phiếu lượt bốn).

---

## 0. Tóm tắt một khối

| Việc trong phiếu lượt bốn | Trạng thái | Commit |
|---|---|---|
| 1. Ba câu trả lời bắt buộc + quét QUALIFIERS + R5 | **XONG** | `6d397766` (việc) · `80601274` (bằng chứng lượt 3) |
| 2. KT-136 — soak đĩa thật + ngắt giữa lúc ghi | **XONG** | `1dc25ae4` |
| 3. Nhúng `VBREV_ALGO` vào dòng `# build:` của ảnh và log | **XONG** | `1f5978aa` |
| 4a. KT-81 — thiết bị giả nghiêm ngặt + bảng tài nguyên | **XONG** | `5aee1ac7` |
| 4b. K7 — `write` thiếu nội dung + nhóm ưu tiên | **XONG** | `a112a214` |
| 4c. K7 — `cat`/`stat` lên thư mục, `ls` tương đối, `time` hai tham số | **CHƯA** | — |
| 4d. KT-137 — `cd`/`pwd` (cuối cùng) | **CHƯA** | — |
| 4e. Bảy lỗi v1 — N2 → N3 → N7 → N1 | **CHƯA** | — |

**Cổng**: `./v2/bin/v2 gate --fast` **PASS** sau **mỗi** commit ở trên, kể cả `a112a214`
(`v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)`, `EXIT=0`).

**Trạng thái cuối** (`./v2/bin/v2 status`, cùng một lần chạy):

```text
HEAD        a112a214d9a0 (main)  chưa push: 0
build-inputs 212 mục  dấu vân tay: 6514ef5cb0fc13a9  dirty: 0
ảnh mới nhất e34334acada7c6f6fd7eb080c963d9b5674731abda362ab6c1f1d1c8ed299ea5.bin
             sha256=e34334acada7c6f6fd7eb080c963d9b5674731abda362ab6c1f1d1c8ed299ea5 size=205440
             # build: algo=kt134-v4 inputs=6514ef5cb0fc13a9 dirty=0
task        DONE=132 READY=1 BLOCKED=0 REVIEW=0
```

---

## 1. Việc 1 — ba câu trả lời bắt buộc

### 1.1 Câu 1 — "Agent 0 có yêu cầu `write` thiếu nội dung phải báo lỗi không?"

**Không.** Phiếu lượt hai chỉ yêu cầu **thêm một ca biên** cho `write` có đường dẫn nhưng thiếu nội
dung, **không** nói hành vi phải là gì. Vì vậy ca ấy (khi đó) **ghim hành vi thật** — cắt tệp về
0 byte — chứ không phải một yêu cầu.

Đến **phiếu lượt bốn** ngài mới quyết định đổi: "`write` có đường dẫn, thiếu nội dung = **ĐỔI
THÀNH BÁO LỖI**". Việc 4b dưới đây thi hành đúng quyết định ấy.

**Đính chính đã ghi**: `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md` §3.2 và
`v2/agent/reports/Q4-9-COMMANDS.md` §5 đều có khối **ĐÍNH CHÍNH**, giữ nguyên văn bản sai ở dưới.

### 1.2 Câu 2 — "R1 nói `gate --fast` đỏ trên cây sạch; phiếu lượt hai nói xanh. Bên nào đúng?"

**Phiếu lượt hai đúng. R1 sai.**

Bằng chứng: chạy `gate --fast` trên cây **sạch** tại `f50c6dea` ⇒ **PASS, EXIT=0**, bước
`check-scope-table` báo `19 ca, 4/19 bỏ qua, PASS`.

R1 đo **cây làm việc** tại thời điểm ấy, nơi `E_AMBIGUOUS_FIELD` đã được thêm vào nhưng **chưa
commit** — nên nó đo một trạng thái không tồn tại trong lịch sử.

### 1.3 Câu 3 — "HEAD trong bằng chứng lệch với HEAD lúc chạy `v2 status`"

Đúng, và đã sửa cách làm. Nguyên nhân: `v2 status` chạy ở `3b132dcd`, rồi commit bằng chứng
`80601274` được tạo, rồi dòng HEAD trong tệp được lấy từ `git log` **sau** commit ấy.

**Luật mới (đang áp dụng)**: dòng HEAD phải được dán từ **cùng một lần chạy** `v2 status` với phần
còn lại của bằng chứng, không lấy lại sau.

### 1.4 Quét QUALIFIERS

> **ĐÍNH CHÍNH 2026-11-18 (Agent 0 phiếu lượt 5 §A2)** — bảng §0 ghi `80601274` cho mục này là
> **sai**: `80601274` là commit **bằng chứng của lượt 3** (`[KT-134] bang chung nguyen van cho phieu
> luot 3`). Commit **thật** của cả quét QUALIFIERS **và** R5 là `6d397766`
> (`[KT-142] ba cho Agent 0 bat tra loi + quet toan bo QUALIFIERS + R5 ten nhanh duy nhat + R4/R2 vao
> JOURNAL`). Bằng chứng nguyên văn:
>
> ```text
> $ git show --stat --oneline 6d397766 | head -3
> 6d397766 [KT-142] ba cho Agent 0 bat tra loi + quet toan bo QUALIFIERS + R5 ten nhanh duy nhat + R4/R2 vao JOURNAL
>  PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md           |   10 +
>  v2/agent/JOURNAL/kt139-journal.md                  |   30 +-
> ```
>
> ```text
> $ git show --stat --oneline 6d397766 | grep -E 'qualifiers|neg-test|scope-table'
>  v2/agent/JOURNAL/kt142-quet-qualifiers.md          |  142 ++
>  v2/agent/JOURNAL/scope-table-skip-count.txt        |    6 +-
>  v2/tools/check-scope-table.py                      |  264 ++-
>  v2/tools/git-guard/neg-test.sh                     |   66 +-
> ```
>
> Loại cây: **worktree sạch tại `6d397766`** (hai lệnh trên chạy trên cây hiện tại, `git status
> --porcelain` rỗng tại thời điểm chạy).

`v2/tools/check-scope-table.py`: **14 → 8** từ khoá; số ca bị bỏ qua **4 → 1**; **3** khẳng định
thật trước đây được miễn nay được kiểm thật và **đều xanh**. Selftest **31 → 39 ca**.

Ba đột biến (chạy thật):

| Đột biến | Kết quả |
|---|---|
| `resolve_claim_block` → `None` | `FAIL (32/39)` |
| `_unique_value` → `values[0]` | `FAIL (38/39)` |
| bỏ luật 1 | `FAIL (37/39)` |

#### 1.4b Câu **còn** bị bỏ qua (1/19) — vị trí, từ khoá, vì sao loại đúng

Agent 0 phiếu lượt 5 §A2 đòi nêu rõ câu còn lại. Nguyên văn từ
[`v2/agent/JOURNAL/kt142-logs/kt142-quet-sau.log`](../../JOURNAL/kt142-logs/kt142-quet-sau.log):

```text
TONG cau bi bo qua: 1
So tu khoa dang dung: 1 / 8

### 'chưa có' -> 1 cau
    v2/docs/k6-persist.md:78
      nhánh `durable` bằng không chưa có ca |
```

| mục | giá trị |
|---|---|
| **vị trí** | `v2/docs/k6-persist.md:78` |
| **từ khoá loại nó** | `chưa có` (từ khoá **duy nhất** còn dùng trong 8) |
| **câu nguyên văn** | ``nhánh `durable` bằng không chưa có ca \|`` |
| **vì sao loại đúng** | câu **không** khẳng định một số đo. Nó nói nhánh `durable` **chưa có ca kiểm** — tức mô tả một **chỗ thiếu**, không phải một giá trị đo được. Không có khối `scope=` nào để so, nên checker **không thể** kiểm nó; nếu bắt checker kiểm thì phải bịa một khối, đúng thứ luật 1 cấm. Đây là câu duy nhất trong 19 câu từng bị loại mà việc loại là **đúng bản chất**, không phải loại oan. |

Mốc ghi trong [`v2/agent/JOURNAL/scope-table-skip-count.txt`](../../JOURNAL/scope-table-skip-count.txt)
(`v2/docs/k6-persist.md 1`) khớp đúng câu này — không phải mốc "còn sót" nào khác.


### 1.5 R5 — ca âm git-guard **song song**

`neg-test.sh --selftest-parallel`: **PASS** trong 13,7 s; hai nhánh
`task/kt85neg-21205-1794982281` và `task/kt85neg-21206-1794982281` **khác tên**.
Đột biến (đặt tên cố định) ⇒ exit 1 với `not ok - selftest-parallel: lần 2 exit 2` và
`hai lần chạy dùng CÙNG tên nhánh [task/kt85neg]`.

---

## 2. Việc 2 — KT-136: soak **đĩa thật** + ngắt giữa lúc ghi

**Soak** (`persist.sh --soak 10`, mỗi vòng 10 boot ⇒ 100 boot):

```text
cycles=10 boots=100
sha_all_equal=1 bad_cycles=0
```

Cả 10 vòng cho **cùng** một sha256 đĩa:
`f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389`. Thời gian ~2m51s.

**Ngắt giữa lúc ghi** (`persist.sh --crash`): giết QEMU khi log hiện `# persist: phase=6
action=verify `, rồi boot lại và kiểm.

```text
fsck=1 fail=0
/p5.bin bytes=1536 match=1
/p1.txt absent=1
/p7.bin bytes=23040 want_max=73728 prefix_ok=1 partial=1
records=12 well_formed=1
```

**Phát hiện (đã ghim, CHƯA sửa — chờ ngài quyết)**: sau khi ngắt giữa lúc ghi, `/p7.bin` tồn tại
**một phần**, nên lần boot sau **không** tạo lại được vì `fs.create` là **độc quyền**. Đây **không**
phải hỏng FS: journal của FS là **theo từng thao tác**, không theo giao dịch. **Không** mất dữ liệu
đã commit ⇒ báo cáo, không tự sửa.

**Ba đột biến**: `fresh_disk` không xoá ⇒ ĐỎ (`format hỏng`, `mount: Invalid`); chỉ vòng 1 xoá ⇒ ĐỎ
(cùng lỗi); sai mốc sha ⇒ ĐỎ (`bad_cycles=2`, exit 1).

---

## 3. Việc 3 — nhúng `VBREV_ALGO` vào dòng danh tính

Dòng `# build:` nay là (ảnh hiện hành):

```text
# build: algo=kt134-v4 inputs=6514ef5cb0fc13a9 dirty=0
```

(Ảnh của riêng KT-143 là `algo=kt134-v4 inputs=f2ad758e76dd2140 dirty=0`; KT-144 sửa mã kernel nên
`inputs` đổi tiếp — đúng như thiết kế.)

**Vì sao**: đổi **định nghĩa** dấu vân tay (thêm/bớt thư mục khỏi phạm vi) làm `inputs` đổi mà
**không** đổi một byte mã sản phẩm nào. Thiếu `algo=` thì hai log khác số mà người đọc không có
cách nào biết chúng có so được với nhau hay không — và tệ hơn, một log cũ có thể "tình cờ" khớp số
với định nghĩa mới.

**Đã làm**:

- `build.rs` đọc `VIFAST_BUILD_ALGO` (do `repro-build.sh` đặt) hoặc đọc thẳng `$VBREV_ALGO` bằng
  chính `build-rev.sh` — **một** nguồn sự thật, không chép lại tên.
- `vbrev_parse` **bắt buộc** `algo=` và phải **khớp** `VBREV_ALGO` hiện hành; `unknown` cũng bị từ
  chối. `vbrev_image_has` nhận thêm `algo` ⇒ ảnh build bằng định nghĩa khác **không** phải ảnh đang
  đo, dù `inputs` tình cờ trùng.
- Sáu nơi khác đọc dòng này (`pre-measure.sh`, `pin-artifact.sh`, `status.sh`, `soak.sh`,
  `shell-edge.sh`, `repro-build.sh`) nhận **cả** dạng mới và dạng cũ (ảnh lịch sử vẫn ghim/hiện được).
- Selftest thêm ba ca: `(k1)` thiếu `algo=` ⇒ `REFUSE malformed`; `(k2)` `algo=` khác ⇒
  `REFUSE malformed`; `(k3)` đối chứng dương `algo=` đúng ⇒ `ACCEPT`.
  Kết quả: `# vbrev-selftest: PASS (a/b/c/d/e/f/g/h/i/j/k)`.
  Đột biến bỏ phép so `algo` ⇒ `(k2) FAIL`.

**`VBREV_ALGO` `kt134-v3` → `kt134-v4`** — **không** phải vì giá trị dấu vân tay đổi (vẫn
`1324edbf33f315b8` trước và sau), mà vì **script tính** đã đổi, và bản ghim cặp (sha script, tên
định nghĩa) cố ý coi mọi thay đổi script là thay đổi định nghĩa.

**`K6-CLOSURE.md`** đã ghi rõ: định nghĩa đổi sang `kt134-v3` ở `3b132dcd`, nay `kt134-v4` ở
KT-143; **kết luận K6 và phụ lục A giữ nguyên**, gắn với sha ảnh đã ghi; **không** chạy lại soak
1000, **không** làm lại baseline; log cũ **không** sửa/không phân tích lại, truy nguyên bằng commit.

---

## 4. Việc 4a — KT-81: thiết bị giả nghiêm ngặt + bảng tài nguyên

Đây là **hai luật cuối** trong tám luật máy của OD-K6-5 (sáu luật kia đã cài ở KT-80).

### 4.1 Luật 5 — thiết bị giả nghiêm ngặt

**Vấn đề**: hai lỗi driver của v1 sống lâu vì **mô hình** chấp nhận mọi thứ (không feature, không
dung lượng, không cỡ dữ liệu). Mô hình dễ dãi **tệ hơn** không có mô hình, vì nó tạo cảm giác đã
kiểm.

**Đã thêm** `StrictBlk` — mô hình hoá năm điều kiện mà một `virtio-blk` thật luôn kiểm:

| Điều kiện | Lỗi v1 | Mã lỗi |
|---|---|---|
| Gửi `T_FLUSH` khi **chưa** đàm phán `VIRTIO_BLK_F_FLUSH` | **#1** | `FlushNotNegotiated` |
| Độ dài dữ liệu không khớp loại request + số sector | **#2** | `LengthMismatch` |
| Sector/dải sector ngoài dung lượng | — | `OutOfRange` |
| Loại request lạ | — | `Unsupported` |
| Chưa `DRIVER_OK` | — | `NotReady` |

Hai chi tiết **cố ý**: (a) kiểm **cả hai phía** của `FLUSH` — driver khai đã đàm phán **và** thiết
bị báo có bit; (b) **thứ tự kiểm cố định** để cùng một request sai luôn cho cùng một lý do (nếu
không, log đỏ không tái lập được).

**Test**: 10 ca, mỗi lỗi v1 có **ca âm** và **ca dương**. Đột biến: bỏ nhánh `FlushNotNegotiated` ⇒
`FAILED. 7 passed; 3 failed`; bỏ nhánh `LengthMismatch` ⇒ `FAILED. 8 passed; 2 failed`.

### 4.2 Luật 6 — bảng tài nguyên một nguồn + kiểm chồng lấn

**Vấn đề**: địa chỉ UART/PLIC/CLINT/trang test/ECAM nằm rải ở nhiều tệp dưới dạng hằng số viết tay
⇒ (a) **hai nguồn** (một chỗ sửa, chỗ khác không ⇒ MMU map một đằng, driver đọc một nẻo, không lỗi
nào báo); (b) **chồng lấn** (hai vùng đè nhau ⇒ ghi vùng này xoá vùng kia).

**Đã thêm**: `Region { name, base, size }` + `REGIONS` (sáu vùng), `overlaps`, `first_overlap` (trả
**cặp** vùng đè nhau, không chỉ `bool`), `first_invalid` (bắt kích thước 0 và tràn địa chỉ).
`vifast-plat-riscv64` nay **dẫn xuất** `UART0_BASE`/`PLIC_BASE`/`CLINT_BASE`/`TEST_PAGE_BASE` từ
bảng — không còn hằng số viết tay thứ hai để lệch.

**Test**: 7 ca. Đột biến: tăng `PLIC.size` thêm một trang để nó đè `UART0` ⇒ ca "bảng thật không
chồng lấn" `FAILED`; khôi phục ⇒ `7 passed`.

### 4.3 Cái **không** có trong KT-81

- **Không** sửa driver thật. `StrictBlk` là **mô hình để test**, không biên dịch vào ảnh.
- **Không** đổi ABI/`spec/`, không nới ngưỡng, không thay linker script.
- **Không** chứng minh driver thật **tuân** mọi điều kiện của `StrictBlk` — việc đó cần nối mô hình
  vào đường chạy thật, **chưa làm**. Giá trị hiện tại: hai lỗi v1 ấy **giờ có một mô hình bắt
  được**, và bảng tài nguyên có phép kiểm chồng lấn thật.

---

## 5. Việc 4b — K7: `write` thiếu nội dung + nhóm ưu tiên

### 5.1 `write <path>` thiếu nội dung — **ĐÃ ĐỔI**

**Trước**: `write /a` (không nội dung) ⇒ `cmd_write` gọi `OP_TRUNC | 0` ⇒ **cắt tệp về 0 byte** rồi
in `# shell: write /a bytes=0 sync=ok`. Gõ thiếu một chữ là mất nội dung tệp, và shell **nói là
thành công**.

**Nay**: in `# shell: thieu noi dung`, tệp **không** bị đụng. Muốn cắt rỗng thì dùng
`truncate <path> 0` — tường minh.

### 5.2 Nhóm ưu tiên — **ghim**, không sửa (cả bốn đã an toàn)

Ngài dặn "ghim hành vi thật trước; hành vi nào làm hỏng/mất dữ liệu thì báo trước khi sửa". Kết quả
chạy trên **ảnh thật**:

| Ca | Hành vi thật | Vì sao an toàn |
|---|---|---|
| `write / abc` | `# shell: fs loi code=8` (`IsDir`) | `Fs::write` chặn `INODE_DIR` **trước** khi ghi byte nào |
| `truncate / 5` | `# shell: fs loi code=8` (`IsDir`) | `Fs::truncate` chặn trước `free_inode_blocks` |
| `truncate /a` (thiếu cỡ) | `# shell: thieu kich thuoc` | `fs_dispatch` không gọi `cmd_truncate` |
| `write    abc` (path rỗng) | `# shell: thieu duong dan` | `split_space` cho `path` rỗng |

Cộng một ca **toàn vẹn**: sau cả bốn lệnh, `/a` vẫn `size=8` và `cat /a` vẫn đọc đúng `xin chao`.

**Một ghi chú đáng chú ý**: bình luận `r299` trong `v2/lib/vifast-fs/src/dir.rs` ghi rằng
`truncate` **từng** không chặn thư mục, và hậu quả là "mọi entry biến mất trong khi inode con vẫn
giữ nlink cũ (fsck Corrupt, dữ liệu mất qua namespace)". Nghĩa là lỗi ấy **đã từng có thật** và đã
được vá ở tầng FS. Ca `truncate / 5` ở đây là **ca hồi quy** cho bản vá đó.

### 5.3 Ba vế của luật đóng lỗi

| Vế | Cách cài | Kết quả |
|---|---|---|
| **Đỏ trên mã cũ** | log SAI trong selftest giữ nguyên hành vi cũ (`bytes=0`, `size=0`) | `selftest 3g new-cmds-edge3 (sai): ĐÚNG (2 PASS + 3 FAIL)` — đỏ **ba** lệnh |
| **Xanh trên mã mới** | ảnh thật, `shell-edge.sh --image` | `8/8 ca + … + k7-uu-tien PASS` |
| **Đột biến bỏ nhánh ⇒ đỏ** | `else if false && text.is_empty()` rồi build lại ảnh | `new-cmds-edge3[write-thieu-noi-dung]: FAIL (… thấy 0)` |

Đột biến **không** làm `k7-uu-tien` đỏ — đúng như mong đợi (bốn ca ưu tiên không đi qua nhánh ấy).

---

## 6. Còn lại — thứ tự ngài đặt

1. `cat`/`stat` lên thư mục, `ls` đường dẫn tương đối (gộp KT-137), `time` hai tham số.
2. **KT-137** (`cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối) — **cuối cùng**.
3. Bảy lỗi v1: N4/N6 **đã đóng**, N5 **hoãn K5'**, **N1/N2/N3/N7 mở** — thứ tự **N2 → N3 → N7 → N1**,
   mỗi lỗi đủ ba vế.

---

## 7. Cần ngài quyết (nếu có)

**Không có câu hỏi chặn.** Hai điểm ghi nhận để ngài biết, không cần trả lời ngay:

1. **`/p7.bin` một phần sau khi ngắt giữa lúc ghi** (§2) — đã ghim, chưa sửa. Nếu ngài muốn sửa thì
   cần một quyết định về ngữ nghĩa (`fs.create` độc quyền vs. cho phép ghi tiếp tệp dở).
2. **`StrictBlk` chưa nối vào đường chạy thật** (§4.3) — mô hình bắt được lỗi, nhưng chưa chứng minh
   driver thật tuân nó. Nếu ngài muốn vế ấy thì cần thêm một task.
