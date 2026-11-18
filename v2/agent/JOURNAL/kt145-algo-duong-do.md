# KT-145 — siết `algo=` ở **đường đo** (Agent 0 phiếu lượt 5 §A4)

**Ngày**: 2026-11-18 · **Agent 1** · HEAD trước task: `a112a214`

## 0. Việc Agent 0 giao (nguyên văn §A4)

> - Chỉ status.sh và pin-artifact.sh được nhận dạng cũ (hiển thị / ghim ảnh lịch sử).
> - Đường đo (pre-measure.sh, soak.sh, shell-edge.sh, repro-build.sh khi so sánh) phải từ chối
>   ảnh không có algo= hoặc algo khác hiện hành.
> - Ca kiểm: đưa ảnh dạng cũ vào soak.sh và shell-edge.sh ⇒ REFUSE. Kèm đột biến nới ⇒ đỏ.

## 1. Hiện trạng trước khi sửa (đo, không suy)

Quét **toàn bộ** `v2/` tìm lệnh `grep` còn nhận dạng cũ của dòng `# build:`:

```text
$ grep -rn --include=*.sh -E "grep.*# build: (inputs=|rev=|\(inputs\|rev\)=|\[a-z\])" v2/ | grep -v '^v2/agent/JOURNAL/'
v2/test/boot/build-rev.sh:...:        if LC_ALL=C grep -a -q -E '# build: (inputs|rev)=[0-9a-f]+ dirty=[01]' "$image" 2>/dev/null; then
v2/test/boot/shell-edge.sh:1628:    build_line="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: [a-z]*=[0-9a-f]* dirty=[01]' "$IMAGE" …)"
v2/tools/pin-artifact.sh:120:        if ! LC_ALL=C grep -a -q -m1 -e '# build: algo=' -e '# build: inputs=' -e '# build: rev=' "$f"; then
v2/tools/pin-artifact.sh:160:LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' "$IMAGE" …)"
v2/tools/pre-measure.sh:165:LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' -e '# build: rev=…' "$IMAGE" …)"
v2/tools/status.sh:47:    ART_LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' -e '# build: rev=…' "$ART" …)"
```

Ba chỗ thuộc **đường đo** (`shell-edge.sh`, `pre-measure.sh`, và `soak.sh` chỉ kiểm dòng **log**
chứ không kiểm **tệp ảnh**) ⇒ đúng lỗ §A4 đóng.

## 2. Sửa gì

| tệp | sửa |
|---|---|
| `v2/test/boot/build-identity.sh` | **MỚI**. `vbrev_image_identity <ảnh>` → 0 = ảnh khai **đúng** `algo=` hiện hành; đặt `VBREV_IMG_ALGO/INPUTS/DIRTY` + `VBREV_IMG_REASON ∈ {ok, no-image, no-build-line, no-algo, algo-mismatch}`. Kèm `--selftest` 5 ca (l1..l5) có **đối chứng dương** (l3). |
| `v2/test/boot/shell-edge.sh` | nạp `build-identity.sh`; `preflight_image` TỪ CHỐI ảnh sai định nghĩa; **thêm `--check-image`** (chỉ kiểm danh tính, không mở QEMU) để ca hồi quy chạy trong vài giây; `build_line` chỉ nhận dạng mới. |
| `v2/test/boot/soak.sh` | nạp `build-identity.sh`; TỪ CHỐI **trước khi boot** nếu ảnh sai định nghĩa; **thêm `--check-image`**. |
| `v2/tools/pre-measure.sh` | **bỏ hẳn cờ `--allow-legacy`**; kiểm **định nghĩa** trước mọi phép đọc khác; `LINE` chỉ nhận dạng mới; selftest 6 → 7 ca (thêm (g) `algo-mismatch`). |
| `v2/tools/repro-build.sh` | kiểm `vbrev_image_identity` **trước** `vbrev_image_has` để thông điệp phân biệt "ảnh dạng cũ" với "ảnh khác dấu vân tay". |
| `v2/test/boot/algo-strict.sh` | **MỚI**. 15 ca: 9 ca hành vi (3 script × 3 ảnh) + 6 ca cấu trúc (quét toàn bộ `v2/`). |
| `v2/bin/v2` | `check`: `build-identity.sh --selftest`; `gate --fast`: `algo-strict.sh`. |

**Vì sao `vbrev_image_identity` KHÔNG nằm trong `build-rev.sh`**: `build-rev.sh` là **định nghĩa**
dấu vân tay, bị `v2/tools/check-algo-pin.sh` ghim (đổi tệp ⇒ phải tăng `VBREV_ALGO`, vì hằng số ấy
nằm trong chuỗi băm). Hàm mới là **người tiêu thụ** định nghĩa, không tham gia phép băm. Đặt nó
vào `build-rev.sh` sẽ buộc tăng `VBREV_ALGO` và làm mọi ảnh/bằng chứng đã ghi lệch mốc **trong khi
thuật toán băm không đổi một byte**. Tách tệp giữ đúng cả hai. Bằng chứng: `v2 check` xanh bước
`ghim thuật toán dấu vân tay (KT-134)` mà **không** phải tăng `VBREV_ALGO`.

## 3. Bằng chứng — ca kiểm chạy thật

```text
$ bash v2/test/boot/algo-strict.sh
# algo-strict: định nghĩa hiện hành algo=kt134-v4, dấu vân tay cây=6514ef5cb0fc13a9
# --- A. hành vi: ba script đường đo ---
# ok a1 - shell-edge.sh TỪ CHỐI đúng reason=no-algo
# ok a2 - shell-edge.sh TỪ CHỐI đúng reason=algo-mismatch
# ok a3 - shell-edge.sh NHẬN ảnh algo đúng
# ok b1 - soak.sh TỪ CHỐI đúng reason=no-algo
# ok b2 - soak.sh TỪ CHỐI đúng reason=algo-mismatch
# ok b3 - soak.sh NHẬN ảnh algo đúng
# ok c1 - pre-measure.sh TỪ CHỐI đúng reason=legacy-format
# ok c2 - pre-measure.sh TỪ CHỐI đúng reason=algo-mismatch
# ok c3 - pre-measure.sh KHÔNG từ chối ảnh algo đúng vì lý do định nghĩa (đối chứng dương)
# --- B. cấu trúc: quét toàn bộ v2/ tìm chỗ còn nhận dạng cũ ---
# ok d1 - không tệp nào NGOÀI danh sách được phép còn nhận dạng cũ (3 tệp trong danh sách)
# ok d2 - mọi mục trong danh sách được phép đều còn thật sự khớp mẫu (danh sách không mục rữa)
# ok e:v2/test/boot/shell-edge.sh - v2/test/boot/shell-edge.sh không nằm trong danh sách được phép nhận dạng cũ
# ok e:v2/test/boot/soak.sh - v2/test/boot/soak.sh không nằm trong danh sách được phép nhận dạng cũ
# ok e:v2/tools/pre-measure.sh - v2/tools/pre-measure.sh không nằm trong danh sách được phép nhận dạng cũ
# ok e:v2/tools/repro-build.sh - v2/tools/repro-build.sh không nằm trong danh sách được phép nhận dạng cũ
# algo-strict: 15 PASS, 0 FAIL
# algo-strict.sh: PASS
```

```text
$ bash v2/test/boot/build-identity.sh --selftest
# build-identity-selftest: PASS (l1/l2/l3/l4/l5)
```

```text
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest: PASS (a/b/c/d/e/f/g)
```

## 4. Đột biến — nới ⇒ ĐỎ (chạy thật)

### Đột biến 1 — bỏ phép so `algo` trong `vbrev_image_identity`

```text
# not ok a2 - shell-edge.sh NHẬN ảnh phải bị từ chối (reason=algo-mismatch) — [REV-GATE: ACCEPT image=…/badalgo.bin algo=kt000-khong-co inputs=6514ef5cb0fc13a9 dirty=0]
# not ok b2 - soak.sh NHẬN ảnh phải bị từ chối (reason=algo-mismatch) — [REV-GATE: ACCEPT image=…/badalgo.bin algo=kt000-khong-co inputs=6514ef5cb0fc13a9 dirty=0]
# not ok c2 - pre-measure.sh từ chối nhưng SAI reason (mong algo-mismatch) — [# ảnh: sha256=92f8d0cc… size=4157
# algo-strict: 12 PASS, 3 FAIL
# algo-strict.sh: FAIL
```

```text
$ bash v2/test/boot/build-rev.sh --selftest   # (khi đột biến nằm trong build-rev.sh)
# vbrev-selftest (l2) FAIL: ảnh algo KHÁC định nghĩa mà hàm nhận — reason=ok
# vbrev-selftest: FAIL
```

### Đột biến 2 — thêm lại `grep` nhận dạng cũ vào `pre-measure.sh`

```text
# not ok d1 - tệp còn nhận dạng cũ mà KHÔNG được phép: v2/tools/pre-measure.sh
# algo-strict: 14 PASS, 1 FAIL
# algo-strict.sh: FAIL
```

Khôi phục cả hai ⇒ `15 PASS, 0 FAIL`.

## 5. Việc **KHÔNG** làm (và vì sao)

- **Không** tăng `VBREV_ALGO`. Thuật toán băm không đổi; tăng sẽ làm mọi ảnh/bằng chứng đã ghi
  lệch mốc mà không đổi một byte mã nguồn nào — đúng thứ §A4 đang chống.
- **Không** sửa `status.sh` / `pin-artifact.sh`: Agent 0 cho phép hai tệp ấy nhận dạng cũ. Ca
  `d1`/`d2` ghim đúng ranh giới ấy (thêm tệp thứ tư ⇒ đỏ).
- **Không** đụng `v2/test/boot/build-rev.sh`: xem §2.
