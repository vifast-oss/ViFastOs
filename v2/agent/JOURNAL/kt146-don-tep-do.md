# KT-146 — dọn tệp dở sau cắt điện + tài liệu FS + nợ v2.1 (Agent 0 phiếu lượt 5 §A6)

**Ngày**: 2026-11-18 · **Agent 1** · HEAD trước task: `cc60bd22`

## 0. Việc Agent 0 giao (nguyên văn §A6)

> - Không đổi ngữ nghĩa FS trong K7. FS nhất quán, không mất dữ liệu đã commit ⇒ đạt C5.
> - persist.sh: boot sau crash gặp tệp dở thì xoá rồi tạo lại; ca: crash ⇒ boot ⇒ chạy tiếp
>   các phase tới cuối, không kẹt. Kèm đột biến bỏ bước dọn ⇒ đỏ.
> - Tài liệu FS: journal theo từng thao tác, không có ghi tệp kiểu giao dịch; sau crash có thể còn
>   tệp dở.
> - Nợ v2.1: ghi tệp an toàn (ghi tạm + đổi tên nguyên tử nếu FS có rename) + quyết định ngữ nghĩa
>   create.

## 1. Sửa gì

| tệp | sửa |
|---|---|
| `v2/test/boot/persist.sh` | `crash_run`: thêm bước **(1b) dọn tệp dở** (`fsutil ls` tìm `p7.bin` ⇒ `fsutil rm`), rồi boot lại và đòi pha 7 **xanh** (trước: đòi pha 7 **đỏ** và ghim hành vi ấy). Thêm bước **(2b) chạy tiếp tới cuối**: boot `b08r` đọc lại pha 7 + đòi `# persist: phases=7 ok=7`. |
| `v2/docs/k6-product-fs.md` | thêm §6b: journal **theo từng thao tác**, không theo giao dịch; sau cắt điện có thể còn **tệp dở** (tiền tố đúng, `fsck` sạch, không mất dữ liệu đã commit); `fs.create` độc quyền ⇒ chính sách "xoá rồi tạo lại"; **nợ v2.1** (ghi tạm + `rename` nguyên tử; quyết định ngữ nghĩa `create`). |

**Không** đổi ngữ nghĩa FS: `fs.create` vẫn trả `Exists` khi tên đã tồn tại. Bước dọn là **chính
sách của harness**, không phải thay đổi FS.

## 2. Bằng chứng — chạy thật

```text
$ bash v2/test/boot/persist.sh --crash
# crash: dọn — đã xoá /p7.bin DỞ (create là tạo độc quyền; xoá rồi để kernel tạo lại)
# crash: boot sau cắt điện ĐỌC được dữ liệu đã commit (verify pha 6 xanh)
# crash: boot sau cắt điện GHI LẠI được pha 7 sau khi dọn tệp dở (không kẹt)
# crash: boot sau cắt điện ĐỌC LẠI được pha 7 vừa ghi (verify pha 7 xanh)
# crash: chạy tiếp tới CUỐI — tổng kết phases=7 ok=7
# crash: boot sau cắt điện vẫn GHI được (tạo /p1.txt mới, TAP xanh)
# crash: fsck lần cuối sạch
…
# persist.sh: PASS
```

Bảng boot của cùng lần chạy:

```text
# boot=b01  kết quả=OK       pid=61497  0.1s=1     log=/tmp/kt75-persist/crash/b01.log
# boot=b02  kết quả=OK       pid=61537  0.1s=2     log=/tmp/kt75-persist/crash/b02.log
# boot=b03  kết quả=OK       pid=61580  0.1s=1     log=/tmp/kt75-persist/crash/b03.log
# boot=b04  kết quả=OK       pid=61620  0.1s=2     log=/tmp/kt75-persist/crash/b04.log
# boot=b05  kết quả=OK       pid=61666  0.1s=2     log=/tmp/kt75-persist/crash/b05.log
# boot=b06  kết quả=OK       pid=61715  0.1s=2     log=/tmp/kt75-persist/crash/b06.log
# boot=b07r kết quả=OK       pid=61816  0.1s=3     log=/tmp/kt75-persist/crash/b07r.log
# boot=b08r kết quả=OK       pid=61846  0.1s=2     log=/tmp/kt75-persist/crash/b08r.log
# boot=b01r kết quả=OK       pid=61885  0.1s=1     log=/tmp/kt75-persist/crash/b01r.log
```

## 3. Đột biến — bỏ bước dọn ⇒ ĐỎ (chạy thật)

```text
# crash: dọn — ĐỘT BIẾN: bỏ bước dọn
# crash: boot sau cắt điện ĐỌC được dữ liệu đã commit (verify pha 6 xanh)
not ok - crash: boot sau cắt điện KHÔNG ghi lại được pha 7 sau khi dọn tệp dở
not ok - crash: boot sau cắt điện KHÔNG đọc lại được pha 7 vừa ghi (verify pha 7 đỏ)
not ok - crash: KHÔNG chạy tiếp được tới cuối (thiếu '# persist: phases=7 ok=7')
# crash: boot sau cắt điện vẫn GHI được (tạo /p1.txt mới, TAP xanh)
# persist.sh: FAIL (3 lỗi)
```

Khôi phục ⇒ `persist.sh: PASS`.

## 4. Việc **KHÔNG** làm

- **Không** đổi `fs.create` sang "tạo hoặc cắt rỗng": Agent 0 chốt giữ nguyên ngữ nghĩa trong K7.
- **Không** thêm ghi-tệp-an-toàn (ghi tạm + `rename`): đó là **nợ v2.1**, đã ghi trong
  `v2/docs/k6-product-fs.md` §6b.
- **Không** sửa `v2/lib/vifast-fs`: không có lỗi FS nào ở đây; hành vi tệp dở là **hệ quả đúng** của
  journal theo từng thao tác.
