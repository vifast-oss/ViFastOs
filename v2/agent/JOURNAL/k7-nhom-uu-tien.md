# K7 — nhóm ưu tiên (toàn vẹn dữ liệu) + `write` thiếu nội dung

**Việc Agent 0 giao (phiếu lượt 4, mục "Thứ tự", việc 4, nguyên văn):**

> 4. KT-81 → K7 (write thiếu nội dung + nhóm ưu tiên trước, KT-137 cd/pwd cuối)

Và trong phiếu lượt 4, mục K7:

> `write` có đường dẫn, thiếu nội dung = **ĐỔI THÀNH BÁO LỖI** (tệp **không** bị sửa).
> Nhóm ưu tiên (toàn vẹn dữ liệu) làm trước: `write` lên thư mục, `truncate` lên thư mục,
> `truncate` thiếu cỡ, `write` với đường dẫn chỉ có dấu cách. Mỗi ca **ghim hành vi thật trước**;
> hành vi nào làm hỏng/mất dữ liệu thì **báo Agent 0 trước khi sửa**.

## 1. `write <path>` thiếu nội dung — ĐÃ ĐỔI

**Trước.** `fs_dispatch` khớp `starts_with(line, b"write ")` ⇒ `split_space` cho `path=/a`,
`text=` **rỗng** ⇒ `cmd_write` bỏ vòng ghi, gọi `OP_TRUNC | 0` ⇒ **cắt tệp về 0 byte** rồi in
`# shell: write /a bytes=0 sync=ok`. Gõ thiếu một chữ là mất nội dung tệp, và shell **nói là
thành công**.

**Nay.** Thêm nhánh `text.is_empty()` ⇒ in `# shell: thieu noi dung`, tệp **không** bị đụng.
Muốn cắt rỗng thì dùng `truncate <path> 0` — tường minh.

**Vì sao đây là lỗi chứ không phải "tính năng ghi tệp rỗng".** Một lệnh gõ thiếu **không** được
làm mất dữ liệu đã có. `truncate` đã có sẵn để cắt rỗng; `write` không có nội dung thì không có
việc gì để làm, và "không có việc gì" không được biến thành "xoá nội dung".

## 2. Nhóm ưu tiên — ghim hành vi thật (KHÔNG sửa gì)

Agent 0 dặn "ghim hành vi thật trước; hành vi nào làm hỏng/mất dữ liệu thì báo trước khi sửa".
Kết quả: **cả bốn ca đều đã an toàn** — không ca nào làm hỏng/mất dữ liệu. Vì vậy task này **chỉ
ghim**, không sửa mã. Đây là bằng chứng, không phải suy đoán: bốn ca chạy trên ảnh thật.

| Ca | Hành vi thật | Vì sao an toàn |
|---|---|---|
| `write / abc` | `# shell: fs loi code=8` (`IsDir`) | `Fs::write` chặn `INODE_DIR` **trước** khi ghi byte nào |
| `truncate / 5` | `# shell: fs loi code=8` (`IsDir`) | `Fs::truncate` chặn `INODE_DIR` trước khi `free_inode_blocks` |
| `truncate /a` (thiếu cỡ) | `# shell: thieu kich thuoc` | `fs_dispatch` không gọi `cmd_truncate` khi `parse_u64` trượt |
| `write    abc` (đường dẫn rỗng) | `# shell: thieu duong dan` | `split_space` cho `path` rỗng ⇒ không gọi `cmd_write` |

Cộng một ca **toàn vẹn**: sau cả bốn lệnh, `/a` vẫn `size=8` và `cat /a` vẫn đọc đúng `xin chao`.
Bốn ca riêng lẻ chỉ nói "có in lỗi"; ca này mới nói "dữ liệu còn nguyên".

**Ghi chú về `truncate` lên thư mục.** `v2/lib/vifast-fs/src/dir.rs` có một bình luận `r299` ghi
rằng **trước** đây `truncate` **không** chặn thư mục, và hậu quả là "mọi entry biến mất trong khi
inode con vẫn giữ nlink cũ (fsck Corrupt, dữ liệu mất qua namespace)". Nghĩa là lỗi ấy **đã từng
có thật** và đã được vá ở tầng FS. Ca `truncate / 5` ở đây là **ca hồi quy** cho bản vá đó — nó
đỏ nếu ai bỏ phép chặn `INODE_DIR` trong `Fs::truncate`.

## 3. Bằng chứng

Log đã commit: [`k7-logs/k7-shell-edge-xanh.log`](k7-logs/k7-shell-edge-xanh.log) (ảnh có bản vá)
và [`k7-logs/k7-shell-edge-dot-bien.log`](k7-logs/k7-shell-edge-dot-bien.log) (ảnh đã đột biến).

```text scope=v2/agent/JOURNAL/k7-logs/k7-shell-edge-xanh.log
# shell: write /a bytes=8 sync=ok
# shell: thieu noi dung
# shell: stat path=/a kind=1 size=8 ino=3
# shell: fs loi code=8
# shell: fs loi code=8
# shell: thieu kich thuoc
# shell: thieu duong dan
# shell: stat path=/a kind=1 size=8 ino=3
# shell: cat ok bytes=8 first=xin chao
```

```text scope=v2/agent/JOURNAL/k7-logs/k7-shell-edge-dot-bien.log
# shell: write /a bytes=0 sync=ok
# shell: stat path=/a kind=1 size=0 ino=3
```

## 4. Ba vế của luật đóng lỗi

| Vế | Cách cài | Kết quả |
|---|---|---|
| **Đỏ trên mã cũ** | log SAI trong selftest giữ nguyên hành vi cũ (`bytes=0`, `size=0`) | `selftest 3g new-cmds-edge3 (sai): ĐÚNG (2 PASS + 3 FAIL)` — đỏ **ba** lệnh, không phải hai |
| **Xanh trên mã mới** | ảnh thật, `shell-edge.sh --image` | `8/8 ca + … + k7-uu-tien PASS` |
| **Đột biến bỏ nhánh ⇒ đỏ** | `else if false && text.is_empty()` rồi build lại ảnh | `new-cmds-edge3[write-thieu-noi-dung]: FAIL (… thấy 0)` |

Đột biến **không** làm `k7-uu-tien` đỏ — đúng như mong đợi: bốn ca ưu tiên không đi qua nhánh
`text.is_empty()`. Nếu chúng đỏ theo, tức là ca ưu tiên đang phụ thuộc nhầm vào bản vá này.

## 5. Cái **chưa** làm (theo đúng thứ tự Agent 0 đặt)

- `cat`/`stat` lên thư mục, `ls` đường dẫn tương đối (gộp KT-137), `time` hai tham số — **sau**.
- **KT-137** (`cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối) — **cuối cùng**.
- Bảy lỗi v1 (N1/N2/N3/N7 mở) — thứ tự **N2 → N3 → N7 → N1**, mỗi lỗi đủ ba vế.
