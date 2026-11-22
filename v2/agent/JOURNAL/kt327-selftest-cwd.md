# KT-327 — `selftest.sh` từ chối oan khi một worktree khác đang chạy `ktest.sh`

## 1. Triệu chứng (gặp thật ở T2 vòng 24)

```text
--- selftest runner ktest (KT-54)
selftest.sh: đang có 'ktest.sh' chạy — chạy selftest một mình (mutation expected.txt ~1 s)
--- selftest runner ktest (KT-54): FAIL
```

Bước chỉ chạy **240 ms** rồi đỏ. Nguyên nhân: hai sub-agent (`vspace2`, `x64dhcp`) đang chạy
`ktest.sh 1 matrix` trong **worktree riêng** (`/tmp/kt325-s1`, `/tmp/kt325-s2`) đúng lúc lead chạy
T2. Bản cũ dùng:

```bash
pgrep -f 'test/boot/ktest\.sh'
```

⇒ khớp **mọi** worktree trên máy, dù `expected.txt` mà nó đọc là **tệp khác**.

Đây **không** phải flake: nó là **giả định sai** "chỉ có một cây". Cùng lớp với nợ LOGDIR dùng chung
(`$TMPDIR/k3-ktest`) đã ghi ở KT-311 §7.

## 2. Sửa

Chỉ từ chối khi `ktest.sh` đang chạy đọc **đúng** `expected.txt` của cây này:

1. `pgrep -f` lấy PID ứng viên (như cũ).
2. Với mỗi PID: `ps -o command=` lấy **đường dẫn** `…/test/boot/ktest.sh` trong command line;
   tương đối ⇒ ghép với `cwd` của tiến trình (`lsof -a -p PID -d cwd -Fn`).
3. So đường dẫn đã chuẩn hoá với `$KTEST` của chính mình.
4. **Không lấy được** `cwd` ⇒ **bảo thủ**: coi như cùng cây (giữ nguyên hành vi cũ).

Không đổi phạm vi bảo vệ: ca (a) vẫn phá `expected.txt` ~1 s, và vẫn **cấm** chạy song song với
một vòng boot đọc **cùng** tệp đó.

## 3. Kiểm hai chiều (bắt buộc — nếu chỉ kiểm một chiều là nới lỏng âm thầm)

| Chiều | Thiết lập | Kết quả |
|---|---|---|
| **Cùng cây** | `ktest.sh 1 matrix` trong `ViFastOs`, rồi `selftest.sh` | **TỪ CHỐI** `đang có 'ktest.sh' chạy trên **cùng** cây` ✔ |
| **Khác cây** | `ktest.sh 1 matrix` trong `/tmp/kt325-s1`, rồi `selftest.sh` ở `ViFastOs` | **PASS 28/28** ✔ (trước đây: từ chối oan) |

`pgrep` khớp **2** tiến trình ở cả hai chiều ⇒ phép so đường dẫn mới là thứ quyết định, không phải
"không thấy gì để so".

## 4. Cổng

`bash -n v2/test/boot/selftest.sh` OK; `selftest.sh` **28/28**; `v2 check` xanh. Chỉ sửa **một** tệp
harness (`v2/test/boot/selftest.sh`) — không chạm sản phẩm, không nới ngưỡng nào.

## 5. Rủi ro

- Nếu `ps`/`lsof` bị chặn trong môi trường lạ ⇒ rơi về hành vi **bảo thủ** (từ chối) — không bao
  giờ thành "cho chạy song song" ngoài ý muốn.
- `pgrep -f` vẫn có thể khớp tiến trình **không phải** `ktest.sh` có chuỗi đó trong đối số (ví dụ
  `grep`); khi đó `ps` không tách được token kết thúc bằng `test/boot/ktest.sh` ⇒ bỏ qua tiến trình
  đó. Đây là **nới** so với bản cũ ở đúng trường hợp vô hại.
