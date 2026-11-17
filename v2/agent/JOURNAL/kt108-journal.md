# KT-108 — harness ca biên cho shell (`v2/test/boot/shell-edge.sh`)

**Ngày**: 2026-11-18 · **Ngân sách**: 2 h · **Dùng**: ~35 phút.
**Tệp mới duy nhất**: `v2/test/boot/shell-edge.sh` (855 dòng). Không sửa tệp nào khác, không
`git add`, không commit.

## Vì sao

Ba lỗi thật của shell đêm 2026-11-18 được tìm ra **bằng tay** (thử biên), không phải bằng test tự
động:

| Lỗi | Trước | Sau |
|---|---|---|
| KT-103 | dòng dài hơn `MAX_LINE` in **một cảnh báo mỗi byte** | một lần mỗi dòng |
| KT-104 | `cat`/`write` không tham số in `# shell: ?` | `# shell: thieu duong dan` |
| KT-105 | `ps` in `state=Unknown` cho **mọi** tiến trình | trạng thái thật |

Không lỗi nào có ca kiểm tự động. Harness này biến đúng ba thử-biên đó (cộng bốn ca biên khác của
shell/FS) thành ca kiểm hồi quy, chạy trên **ảnh đã ghim** (không build).

## Giao diện

```bash
bash v2/test/boot/shell-edge.sh --image <ảnh.bin>             # 8 ca trên ảnh đã ghim
bash v2/test/boot/shell-edge.sh --selftest                    # ca âm (không cần QEMU/ảnh)
bash v2/test/boot/shell-edge.sh --image <ảnh.bin> --selftest  # cả hai (selftest trước)
bash v2/test/boot/shell-edge.sh --help
```

Exit: `0` = mọi ca PASS (và selftest PASS nếu chạy); `1` = có ca FAIL / selftest FAIL;
`2` = lỗi cấu hình (thiếu ảnh/QEMU/OpenSBI, ảnh không có shell hoặc thiếu bản vá KT-103/104).

## Cách làm

- **Một** tiến trình QEMU trên ảnh cho trước; mỗi ca gửi lệnh qua **stdin của QEMU** (UART → ring →
  shell U-mode), chờ đúng chuỗi trong log, rồi khẳng định **đúng chuỗi** trên **lát log của riêng
  ca đó** (không quét cả log: `# shell: ?` của ca khác không làm nhiễu ca này).
- Mỗi ca kết thúc bằng lệnh đánh dấu `echo __END_<n>__`; khi dòng đánh dấu xuất hiện thì mọi output
  của ca đã nằm trong log (shell xử lý lệnh **tuần tự**) ⇒ không cần `sleep`.
- QEMU đi qua `qemu_guard.sh` (KT-78/OD-K6-6): process group riêng, timeout kill **cả group**,
  trap dọn sạch, watchdog dọn khi cha chết. Chỉ kill **group do chính script tạo** (đọc registry
  của guard rồi `qg_kill_group`); **không** `pkill -f qemu-system` (AGENTS §9).
- `qg_boot` chạy QEMU ở nền nên stdin mặc định là `/dev/null`; wrapper `/bin/bash -c 'exec "$@"
  <"$SE_FIFO"'` mở lại FIFO làm stdin cho QEMU **trước** `exec` — guard vẫn là tiến trình cha
  trong group riêng.

## Tám ca

| # | Tên | Lệnh | Khẳng định |
|---|---|---|---|
| 1 | `cat-khong-tham-so` | `cat` | đúng 1 `# shell: thieu duong dan`, **0** `# shell: ?` |
| 2 | `write-khong-tham-so` | `write` | đúng 1 `# shell: thieu duong dan`, **0** `# shell: ?` |
| 3 | `ps-khong-unknown` | `ps` | có `# shell: ps n=…` + ≥1 dòng `  pid=… state=…`, **0** `state=Unknown` |
| 4 | `dong-dai-mot-canh-bao` | dòng 200 ký tự | **đúng 1** `# shell: dong qua dai` |
| 5 | `cat-khong-co` | `cat /khongco` | đúng 1 `# shell: fs loi code=5` |
| 6 | `write-cat-bytes` | `write /a xin chao` rồi `cat /a` | `# shell: write /a bytes=8 sync=ok` + `# shell: cat ok bytes=8 first=xin chao` |
| 7 | `write-de-co-lai` | `write /g <40 B>` rồi `write /g ab` rồi `cat /g` | `bytes=40` → `bytes=2` → `# shell: cat ok bytes=2 first=ab` |
| 8 | `wait-khong-treo` | `wait 13` rồi `echo sau-wait` | `# shell: wait pid=13 code=<n>` trong ≤ 5 s + `# shell: echo sau-wait` chạy |

Ca 4 dùng `echo ` + 195 chữ `A` (tổng 200 ký tự) để dòng bị cắt ở `MAX_LINE` (64 B) vẫn là lệnh
`echo` hợp lệ — không sinh `# shell: ?` làm nhiễu ca 1/2.

## `--selftest` (ca âm bắt buộc)

Mỗi ca: một thư mục `mktemp -d` riêng, hai log giả — `wrong.log` chứa **đúng đầu ra của lỗi cũ**
(`# shell: ?`, `state=Unknown`, 100 dòng `dong qua dai`, …) và `right.log` chứa đầu ra đúng — rồi
chạy **chính** hàm khẳng định của harness trên hai log đó. Log sai **phải ĐỎ**, log đúng **phải
XANH**. Ca 8 có thêm một log "đúng nội dung nhưng `wait` trả lời 99 s" ⇒ vẫn phải ĐỎ.

Kết quả: **PASS (8/8 ca, 19/19 kiểm: 11 sai→ĐỎ + 8 đúng→XANH)**.

Dẫn chứng harness **bắt được lỗi giả** (các dòng ĐỎ của selftest):

```text
# selftest 1 cat-khong-tham-so: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: thieu duong dan', thấy 0)
# selftest 1 cat-khong-tham-so (co ca hai): log SAI -> ĐỎ đúng (thấy 1 dòng '# shell: ?' (lỗi KT-104 tái phát))
# selftest 2 write-khong-tham-so: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: thieu duong dan', thấy 0)
# selftest 2 write-khong-tham-so (co ca hai): log SAI -> ĐỎ đúng (thấy 1 dòng '# shell: ?' (lỗi KT-104 tái phát))
# selftest 3 ps-khong-unknown: log SAI -> ĐỎ đúng (thấy 3 dòng 'state=Unknown' (lỗi KT-105 tái phát))
# selftest 4 dong-dai-mot-canh-bao: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: dong qua dai', thấy 100 (KT-103: một lần mỗi dòng))
# selftest 5 cat-khong-co: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: fs loi code=5', thấy 0)
# selftest 6 write-cat-bytes: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: cat ok bytes=8 first=xin chao', thấy 0)
# selftest 7 write-de-co-lai: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: cat ok bytes=2 first=ab', thấy 0 (ghi đè không co lại?))
# selftest 8 wait-khong-treo: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: wait pid=13 code=<n>', thấy 0 (shell treo ở wait?))
# selftest 8 wait-khong-treo (cham): log SAI -> ĐỎ đúng (wait trả lời sau 99s > 5s (quá hạn))
```

## Chạy thật trên ảnh canonical

```text
image: v2/artifacts/34ee7a25da5fa9f325a9c291360333855d7da25472929588b8a714e9ffa01ac3.bin
       sha256=34ee7a25… bytes=205440  # build: inputs=be9689a5c032cf69 dirty=0
qemu:  11.1.3; opensbi: /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin
cấu hình: -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot (không icount)
```

Kết quả: **8/8 ca PASS**, `# shell-edge.sh: PASS`, exit 0, wall ~4 s. Log: `kt108-real.log`.

```text
# cat-khong-tham-so: PASS
# write-khong-tham-so: PASS
# ps-khong-unknown: PASS
#   | 3 dòng pid, state=Unknown=0
# dong-dai-mot-canh-bao: PASS
#   | dòng 200 ký tự -> 1 dòng '# shell: dong qua dai'
# cat-khong-co: PASS
# write-cat-bytes: PASS
# write-de-co-lai: PASS
# wait-khong-treo: PASS
#   wait 13: trả lời sau 0s (hạn 5s); 'echo sau-wait' chạy
# exit: PASS (# shell: exit code=0, # product halt:)
# shell-edge: 8/8 ca PASS
# shell-edge.sh: PASS
```

## Chạy thật trên ảnh **trước** KT-104/KT-105 (chứng minh bắt lỗi thật)

Ảnh `f6f3d0c6…` (KT-103 đã vá, KT-104/KT-105 **chưa**): harness **ĐỎ 3/8** — đúng ba lỗi thật,
không phải "luôn xanh". Log: `kt108-red-prefix.log`.

```text
# cat-khong-tham-so: FAIL (mong đúng 1 dòng '# shell: thieu duong dan', thấy 0)
# write-khong-tham-so: FAIL (mong đúng 1 dòng '# shell: thieu duong dan', thấy 0)
# ps-khong-unknown: FAIL (thấy 3 dòng 'state=Unknown' (lỗi KT-105 tái phát))
# dong-dai-mot-canh-bao: PASS
# cat-khong-co: PASS
# write-cat-bytes: PASS
# write-de-co-lai: PASS
# wait-khong-treo: PASS
# shell-edge.sh: FAIL
```

## Ghi chú

- Không sửa `soak.sh`, `ktest.sh`, `persist.sh`, `spec/`, `v2/kernel/**`, `v2/lib/**`.
- Không `git add`, không commit.
- `qg_infra` có thể báo `verdict=KHONG_HOP_LE` khi máy bận (loadavg > ncpu) — đó là cảnh báo về
  **số thời gian**, không phải kết quả chuỗi; harness vẫn PASS/FAIL theo 8 ca. Ca 8 có hạn 5 s
  (rộng hơn nhiều so với 0 s đo được) nên không flake vì load.
- Log chạy thật: `/tmp/kt108-shell-edge/qemu.log` (mỗi lần chạy ghi đè).
