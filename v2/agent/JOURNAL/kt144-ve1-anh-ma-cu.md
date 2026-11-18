# KT-144 vế 1 — "đỏ trên **ảnh** build từ mã cũ" (Agent 0 phiếu lượt 5 §A3)

**Ngày**: 2026-11-18 · **Agent 1**

## 0. Việc Agent 0 giao (nguyên văn §A3)

> Build ảnh tại commit cha của a112a214, chạy shell-edge.sh --image với ca
> new-cmds-edge3[write-thieu-noi-dung] ⇒ phải FAIL (thấy bytes=0, size=0). Dán dòng log + sha ảnh.
> Log SAI trong selftest giữ lại, nhưng nó chứng minh checker, không thay được vế này.
> Luật 4 (mới): mọi lỗi K7, vế 1 phải chạy trên ẢNH build từ mã cũ.

## 1. Vì sao phải làm lại

Báo cáo lượt 4 §4b ghi vế 1 là "log cũ đỏ (3 lệnh)". Nhưng "log cũ" ấy là **log QEMU của lần chạy
trước**, không phải **ảnh build từ mã cũ** — và log SAI trong `shell-edge.sh --selftest` chỉ chứng
minh **checker** bắt được chuỗi sai, không chứng minh **mã cũ** thật sự cho ra `bytes=0`/`size=0`.
Agent 0 đúng: thiếu vế ấy.

## 2. Cách làm

1. `git worktree add --detach /tmp/vifast-old-a112 5aee1ac7` — `5aee1ac7` là **commit cha** của
   `a112a214` (KT-144).
2. Build ảnh **canonical** tại worktree ấy (không chép mã, không sửa gì):

   ```text
   $ VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
       bash v2/tools/repro-build.sh --root /tmp/vifast-old-a112 --target-dir /tmp/vifast-old-a112-target
   REPRO machine=Darwin-arm64 root=/private/tmp/vifast-old-a112 sha256=96cc1202c0bfbfe911273798b77cbcada9cc5c42159c77bd20b18c98b1d2b984 size=205440 wall_s=15 algo=kt134-v4 inputs=f636bf7d956cc6b8 dirty=0 rev=5aee1ac7d1f2 build=8f390bafdc1bc984 text=130062 image=/private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
   ```

3. Chạy **chính** `shell-edge.sh` của cây hiện tại trên ảnh ấy:

   ```text
   $ SHELL_EDGE_LOGDIR=/tmp/kt145-old-image bash v2/test/boot/shell-edge.sh \
       --image /private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
   ```

## 3. Kết quả — ĐỎ đúng ca, đúng lý do

```text
# new-cmds-edge3[time-tham-so]: PASS
# new-cmds-edge3[write-thieu-noi-dung]: FAIL (mong đúng 1 dòng '# shell: thieu noi dung' (write /a khong noi dung phai BAO LOI), thấy 0)
# new-cmds-edge3[write-thu-muc-khong-co]: PASS
# new-cmds-edge3[ls-tep]: PASS
# new-cmds-edge3[sync-tham-so]: PASS
# new-cmds-edge3: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
```

Và **hành vi thật của mã cũ** trong log QEMU (`/tmp/kt145-old-image/qemu.log`) — đúng `bytes=0`,
`size=0` như Agent 0 đòi:

```text
192:# shell: write /a bytes=0 sync=ok
194:# shell: stat path=/a kind=1 size=0 ino=3
```

(dòng 190 là `write /a xin chao` ⇒ `bytes=8`; dòng 204/214 là các ca sau ghi lại `/a`.)

## 4. Danh tính ảnh dùng cho vế 1

```text
$ grep -a -o -m1 '# build: algo=[0-9A-Za-z._-]* inputs=[0-9a-f]* dirty=[01]' \
    /private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
# build: algo=kt134-v4 inputs=f636bf7d956cc6b8 dirty=0

$ shasum -a 256 /private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
96cc1202c0bfbfe911273798b77cbcada9cc5c42159c77bd20b18c98b1d2b984  /private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

| mục | giá trị |
|---|---|
| commit build | `5aee1ac7d1f2` (cha của `a112a214`) |
| sha256 ảnh | `96cc1202c0bfbfe911273798b77cbcada9cc5c42159c77bd20b18c98b1d2b984` |
| size | `205440` |
| dòng danh tính | `# build: algo=kt134-v4 inputs=f636bf7d956cc6b8 dirty=0` |
| loại cây | **worktree sạch** tại `5aee1ac7` (`git status --porcelain` rỗng) |

## 5. Vế 2 và vế 3 (đã có từ lượt 4, nhắc lại để đủ bảng ba vế)

| vế | ảnh | kết quả |
|---|---|---|
| 1 — đỏ trên mã cũ | `96cc1202…` (build từ `5aee1ac7`) | `new-cmds-edge3[write-thieu-noi-dung]: FAIL`, log có `bytes=0` + `size=0` |
| 2 — xanh trên mã mới | `e34334ac…` (build từ `a112a214`) | `new-cmds-edge3[write-thieu-noi-dung]: PASS`, `# shell: thieu noi dung` |
| 3 — đột biến làm đỏ lại | ảnh đột biến (bỏ nhánh `text.is_empty()`) | `# shell: write /a bytes=0 sync=ok` + `size=0`, ca `FAIL` |

## 6. Ghi chú

- Worktree `/tmp/vifast-old-a112` và target dir `/tmp/vifast-old-a112-target` **để nguyên** cho tới
  khi hết phiên (dùng lại được cho các vế 1 khác của K7 — luật 4).
- Ảnh cũ mang `algo=kt134-v4` **trùng** định nghĩa hiện hành, nên `shell-edge.sh` (sau KT-145) vẫn
  nhận — đúng: vế 1 cần đo **mã cũ**, không phải đo một định nghĩa khác.
