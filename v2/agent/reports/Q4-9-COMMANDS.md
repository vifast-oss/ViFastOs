<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Q4 — chín lệnh shell: bảng ca đúng / ca biên trên **ảnh sản phẩm**

Người ghi: **Agent 2 dòng OS** · Ngày: **2026-11-18** · HEAD khi làm: `f1077036`
· Tệp được sửa: [`v2/test/boot/shell-edge.sh`](../test/boot/shell-edge.sh)

> **Vì sao có báo cáo này.** Agent 0 yêu cầu bảng **đủ chín dòng** cho chín lệnh shell cần có ca kiểm
> trên ảnh sản phẩm, kèm **tên ca** và **tệp:dòng khẳng định**. Lượt trước bảng còn thiếu ca biên
> cho `uptime`/`jobs` và phán quyết còn gộp; lượt này đã bổ sung và tách.

---

## 1. Bảng chín dòng

Cột cuối ghi **dòng bắt đầu hàm khẳng định** (không ghi dòng `grep` cụ thể, vì dòng đó trôi mỗi lần
sửa tệp; tên hàm thì không trôi).

| lệnh | ca **đúng** | ca **biên** | tệp:dòng (hàm khẳng định) |
|---|---|---|---|
| `ls` | `new-cmds-ok` | `new-cmds-edge`, `new-cmds-edge3` | `v2/test/boot/shell-edge.sh:397` |
| `cat` | `write-cat-bytes`, `write-de-co-lai` | `cat-khong-tham-so`, `cat-khong-co` | `v2/test/boot/shell-edge.sh:817` |
| `write` | `write-cat-bytes`, `write-de-co-lai` | `write-khong-tham-so`, `new-cmds-edge3` (hai ca) | `v2/test/boot/shell-edge.sh:828` |
| `time` | `new-cmds-ok` | `new-cmds-edge`, `new-cmds-edge3` | `v2/test/boot/shell-edge.sh:350` |
| `uptime` | `new-cmds-ok` | `new-cmds-edge2` | `v2/test/boot/shell-edge.sh:362` |
| `sync` | `new-cmds-ok` | `new-cmds-edge`, `new-cmds-edge3` | `v2/test/boot/shell-edge.sh:373` |
| `stat` | `new-cmds-ok`, `truncate-jobs` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:384` |
| `truncate` | `truncate-jobs` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:260` |
| `jobs` | `truncate-jobs` | `new-cmds-edge2` | `v2/test/boot/shell-edge.sh:579` |

## 2. Năm ca biên mới của lượt này

| lệnh | hành vi **thật** (đọc từ mã) | khẳng định đã viết |
|---|---|---|
| `time` **có tham số** | `fs_dispatch` chỉ khớp **nguyên dòng** `time`; không có nhánh khớp tiền tố ⇒ rơi xuống `run_line` ⇒ in dấu hỏi | dấu hỏi là **dòng kết quả đầu** của ca; **không** dòng `time ns=` |
| `write` **có đường dẫn, thiếu nội dung** | `split_space` cho path hợp lệ, text **rỗng**; `cmd_write` bỏ vòng ghi, `OP_TRUNC` ⇒ cắt tệp về **rỗng**, `OP_SYNC`, in `bytes` bằng rỗng | đúng một dòng `bytes` bằng rỗng **và** `stat` thấy `size` bằng rỗng |
| `write` **vào thư mục không có** | `fs_set_path` chỉ nạp đường dẫn; `OP_WRITE` ⇒ `lookup` trượt ⇒ `create` trượt (thư mục cha không có) ⇒ `FsError::NotFound` | đúng một dòng lỗi FS "không tìm thấy", đứng sau `stat` |
| `ls` **lên một tệp** | `fs.lookup` thấy tệp; `read_dir_entry_full` gặp `kind` khác `INODE_DIR` ⇒ `FsError::NotDir`; `cmd_ls` trả **trước** khi in `ls n=` | đúng một dòng lỗi FS "không phải thư mục"; **không** dòng `ls n=…` |
| `sync` **có tham số** | như `time` có tham số — chỉ khớp nguyên dòng `sync` ⇒ in dấu hỏi | dấu hỏi là **dòng kết quả cuối**; **không** dòng `sync ok` |

## 3. Phán quyết gộp → từng lệnh

Trước: **một** dòng PASS/FAIL cho **năm** lệnh, và chỉ in lý do của lệnh hỏng **đầu tiên**.
Nay: hàm `emit_verdicts` chạy **hết** mọi lệnh rồi mới kết luận, in **một dòng cho mỗi lệnh**, và khi
hỏng thì in lý do của **mọi** lệnh hỏng. Mã thoát giữ nguyên (không = xanh, khác không = đỏ) nên
khung gọi không phải đổi. Áp cho cả bốn ca: `new-cmds-ok`, `new-cmds-edge`, `new-cmds-edge2`,
`new-cmds-edge3`.

## 4. Bằng chứng chạy thật

Bộ tự kiểm của chính harness (mỗi ca có log sai phải đỏ và log đúng phải xanh):

```text scope=q4-vong-ba/shell-edge-selftest.log
# shell-edge-selftest: PASS (8/8 ca, 32/32 kiểm: 18 sai→ĐỎ + 14 đúng→XANH)
```

Chạy trên **ảnh sản phẩm** `af01e670…` (build từ HEAD `f1077036`, pipeline canonical):

```text scope=q4-vong-ba/shell-edge-image.log
# new-cmds-edge3[time-tham-so]: PASS
# new-cmds-edge3[write-thieu-noi-dung]: PASS
# new-cmds-edge3[write-thu-muc-khong-co]: PASS
# new-cmds-edge3[ls-tep]: PASS
# new-cmds-edge3[sync-tham-so]: PASS
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 PASS
# shell-edge.sh: PASS
```

## 5. Một chỗ hành vi **lệch** kỳ vọng của Agent 0 — nói thẳng

> **ĐÍNH CHÍNH (Agent 0, phiếu lượt bốn, câu đầu tiên).** Tiêu đề và câu đầu của mục này **sai**:
> Agent 0 **không** đặt kỳ vọng "phải báo lỗi". Quyết định lượt hai chỉ yêu cầu **thêm ca biên**.
> Kỳ vọng ấy do tôi tự gán. Đoạn dưới giữ nguyên văn để thấy cái đã sai; kết luận đúng: hành vi
> thật là **cắt tệp về rỗng**, và Agent 0 đã quyết (lượt bốn, câu đầu tiên) **đổi thành báo lỗi**
> — việc đó vào K7.

Agent 0 yêu cầu: "`write` có đường dẫn nhưng thiếu nội dung ⇒ phải báo lỗi rõ ràng, không được ghi
rỗng". **Hành vi thật ngược lại**: shell **không báo lỗi**, nó **cắt tệp về rỗng** và in `bytes` bằng rỗng.

Tôi ghim **hành vi thật**, không ghim kỳ vọng — luật repo: code là nguồn sự thật số một. Nếu Agent 0
muốn đổi thành lỗi thì phải thêm nhánh `text.is_empty()` trong `cmd_write`/`fs_dispatch`; khi đó ca
`new-cmds-edge3[write-thieu-noi-dung]` sẽ **đỏ** và phải đổi theo. **Đây là quyết định của Agent 0,
không phải của tôi.**

## 6. Còn chỗ nào mỏng — trung thực

Năm chỗ Agent 0 chỉ ra **đã hết mỏng**. Vẫn còn tám chỗ mỏng, **không chỗ nào chặn K6**:

- `cat` lên **thư mục** (chưa có ca).
- `write` lên **thư mục** (chưa có ca).
- `truncate` lên **thư mục** (chưa có ca).
- `stat` lên **thư mục** (chưa có ca).
- `ls` với **đường dẫn tương đối** (chưa có ca).
- `write` với đường dẫn rỗng nhưng **có dấu cách** — khác ca "thiếu nội dung"; chưa có ca.
- `time` với **hai** tham số — chưa có ca.
- `truncate` **thiếu** kích thước — chưa có ca.

## 7. Đính chính con số tôi đã khai sai ở phiếu trước

Phiếu trước tôi viết "hai mươi bảy phép kiểm, trong đó mười chín ca âm". **Sai.** Số thật lúc đó là
**mười lăm âm + mười hai dương**. Con số mười chín là tôi chép nguyên từ dòng tổng kết **bị lỗi**
của chính script. Đã sửa script để in đúng hai vế; nay là **mười tám âm + mười bốn dương** trên
**ba mươi hai** phép kiểm.

Bài học: chép số của công cụ mà không kiểm lại công cụ — đúng dạng "số viết tay" mà QD cấm.
