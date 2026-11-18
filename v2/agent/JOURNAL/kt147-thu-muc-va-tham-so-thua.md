# KT-147 — `cat`/`stat` lên thư mục, `time` hai tham số (Agent 0 phiếu lượt 5 §B3.4c)

**Ngày**: 2026-11-18 · **Agent 1** · HEAD trước task: `cc60bd22`

## 0. Việc Agent 0 giao (nguyên văn §B3.4c)

> 4. Nhận 4c: cat/stat lên thư mục, time hai tham số (ghim hành vi thật trước; hành vi
>    làm hỏng/mất dữ liệu ⇒ hàng chờ Agent 0, không tự sửa).

## 1. Hành vi **thật** — đo trên ảnh sản phẩm, không đoán từ mã

Ảnh: `sha256=a4df1ce5dfb77866839aca24d724b087eac6e42528ba93ae65dcd98a8b21556c`,
`# build: algo=kt134-v4 inputs=7e8bef66f416d40a dirty=0`, `rev=cc60bd2274d1`.

```text
$ ( sleep 4; printf 'cat /\n'; sleep 1; printf 'stat /\n'; sleep 1; printf 'time 5 6\n'; \
    sleep 1; printf 'ls /\n'; sleep 1; printf 'cat /README\n'; sleep 1; printf 'exit\n'; sleep 3 ) \
  | qemu-system-riscv64 -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMG" \
      -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/truncate/jobs/exit
# shell: input=ring va=0x43000000
shell>
# shell: fs loi code=8
shell>
# shell: stat path=/ kind=2 size=512 ino=1
shell>
# shell: ?
shell>
# shell: ls n=1 path=/
shell>
# shell: cat ok bytes=79 first=ViFastOs
shell>
# shell: exit code=0
```

| lệnh | hành vi thật | đánh giá |
|---|---|---|
| `cat /` | `# shell: fs loi code=8` (`FsError::IsDir`) | **an toàn** — không in byte nào của thư mục, không đổi gì |
| `stat /` | `# shell: stat path=/ kind=2 size=512 ino=1` | **đúng** — `kind=2` là thư mục, `size` là cỡ thư mục |
| `time 5 6` | `# shell: ?` (lệnh lạ) | **an toàn** — `time` chỉ nhận **không** tham số; không có đường nào để hai tham số làm gì |

**Kết luận**: cả ba **không** làm hỏng/mất dữ liệu ⇒ **ghim**, **không** phải hàng chờ Agent 0.

## 2. Sửa gì

`v2/test/boot/shell-edge.sh` — thêm ca **15 `k7-thu-muc`** (bốn phép kiểm, dùng chung khung
`case_begin`/`case_end`):

| phép kiểm | đòi gì |
|---|---|
| `cat-thu-muc` | đúng **một** `# shell: fs loi code=8`, và **không** có `# shell: cat ok bytes=512 …` (nếu `cat /` in nội dung thì thư mục 512 B sẽ hiện ra) |
| `stat-thu-muc` | đúng **một** `# shell: stat path=/ kind=2 size=<n> ino=<n>` |
| `time-hai-tham-so` | đúng **một** `# shell: ?`, và **không** có `# shell: time ns=…` |
| `khong-mat-du-lieu` | sau ba lệnh, `cat /README` vẫn `bytes=79 first=ViFastOs` |

Kịch bản gửi: `cat /` · `stat /` · `time 5 6` · `cat /README` (cách nhau 0,15 s — FIFO 16 B của
16550 làm rơi byte nếu gửi liền).

## 3. Bằng chứng — chạy thật trên ảnh sản phẩm

```text
$ SHELL_EDGE_LOGDIR=/tmp/kt146-thu-muc bash v2/test/boot/shell-edge.sh --image "$IMG"
# k7-thu-muc[cat-thu-muc]: PASS
# k7-thu-muc[stat-thu-muc]: PASS
# k7-thu-muc[time-hai-tham-so]: PASS
# k7-thu-muc[khong-mat-du-lieu]: PASS
# k7-thu-muc: PASS
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 + k7-uu-tien + k7-thu-muc PASS
# shell-edge.sh: PASS
```

## 4. Ca âm — selftest của harness

```text
$ bash v2/test/boot/shell-edge.sh --selftest
# selftest 15 k7-thu-muc (sai): ĐÚNG (1 PASS + 3 FAIL, mã thoát 1)
# selftest 15 k7-thu-muc (dung): ĐÚNG (4 PASS + 0 FAIL, mã thoát 0)
# shell-edge-selftest: PASS (8/8 ca, 36/36 kiểm: 20 sai→ĐỎ + 16 đúng→XANH)
```

Log SAI cố ý tái hiện **đúng ba** hồi quy cần bắt: `cat /` in nội dung (`cat ok bytes=512`),
`stat /` báo `kind=1` (tệp), `time 5 6` chạy như `time` hợp lệ (`time ns=…`).

## 5. Việc **KHÔNG** làm

- **Không** sửa shell: cả ba hành vi đã đúng và an toàn. Sửa chỉ để "cho đẹp" là đổi bề mặt sản
  phẩm — ngoài phạm vi Agent 1 được quyết (B1).
- **Không** thêm `cd`/`pwd`: đó là **KT-137**, Agent 0 xếp **cuối** K7.
- **Không** đụng `ls` tương đối: Agent 0 gộp vào KT-137.
