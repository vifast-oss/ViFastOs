# KT-84 — ghim ảnh để đo (D2-122.6)

**Trạng thái**: xong phần công cụ; phần "mọi phép đo trỏ `artifacts/<sha256>.bin`" là **quy ước**
cho các task sau, chưa cưỡng chế bằng cổng.

## Vấn đề

Trước KT-84, phép đo trỏ vào `v2/kernel/target/<target>/release/vifast-kbin-riscv64`. Đó là tệp
**bị ghi đè** bởi lần build sau. Hệ quả đã xảy ra nhiều lần (D2-108A): đo xong, build lại, rồi báo
cáo một sha **không còn tồn tại** — hoặc tệ hơn, đo ảnh A nhưng trích số từ ảnh B.

## Cách làm

### 1. Mỗi task một `CARGO_TARGET_DIR`

```bash
export CARGO_TARGET_DIR="$(bash v2/tools/task-target.sh KT-84)"
```

- Thư mục: `<repo>/target-task/<mã>` (đã có trong `.gitignore`).
- Hai task build song song **không** giẫm lên nhau.
- `--list` liệt kê, `--clean <mã>` xoá đúng thư mục của một task.
- Mã task được kiểm (`[A-Za-z0-9._-]`), `--clean` chỉ xoá dưới `target-task/`.

**Không** dùng cho build canonical: `v2/tools/repro-build.sh` **cấm** `CARGO_TARGET_DIR` nằm trong
cây nguồn (A1-175) và tự tạo thư mục tạm. Hai đường khác nhau, có chủ ý.

### 2. Ghim ảnh vào `artifacts/<sha256>.bin`

```bash
bash v2/tools/pin-artifact.sh <ảnh-đã-build>
# PIN sha=<sha256> size=<byte> path=v2/artifacts/<sha256>.bin # build: inputs=<16hex> dirty=<0|1> state=moi
```

- **Tên tệp là sha256** ⇒ không thể ghim nhầm ảnh: đọc tên là biết nội dung.
- Ảnh **phải** có dòng `# build: inputs=… dirty=…` (KT-86/KT-89); thiếu ⇒ **không ghim**, mã thoát ba.
- Ghim lại cùng ảnh ⇒ `state=da-co`, không chép lại.
- Tệp đích đã tồn tại mà **khác** byte ⇒ **không ghi đè**, mã thoát bốn (fail-closed).
- Sau khi chép, script **đọc lại** và so sha; lệch ⇒ không dùng.

### 3. Quy ước cho phép đo

Mọi phép đo (soak, ma trận, persist, parity) **phải** trỏ vào `v2/artifacts/<sha256>.bin`, không
trỏ vào `target/…`. Báo cáo ghi **sha256 đầy đủ** + `size` + dòng `# build:` của ảnh đã ghim.

## Ca kiểm (đã chạy)

| Ca | Kỳ vọng | Kết quả |
|---|---|---|
| ghim ảnh hợp lệ | `state=moi`, tệp có trong `artifacts/` | đạt |
| ghim lại cùng ảnh | `state=da-co`, không chép lại | đạt |
| ảnh không có dòng `# build:` | mã thoát **ba**, không ghim | đạt |
| thiếu tham số | mã thoát **hai** | đạt |
| mã task không hợp lệ (`../etc`) | mã thoát **hai** | đạt |
| `--clean` xoá đúng thư mục task | xoá, không chạm chỗ khác | đạt |

## Chưa làm (cần Agent 0)

- **Cưỡng chế** bằng cổng: hiện chỉ là quy ước + công cụ. Muốn cổng từ chối khi phép đo trỏ
  `target/…` thì phải sửa bốn runner (`ktest.sh`, `soak.sh`, `persist.sh`, `run.sh`) — đổi phương
  pháp đo, cần A0 duyệt.
- **Ca âm "ghi đè target"** (D2-122 mục sáu): cần dựng hai task build song song rồi chứng minh ảnh
  của task này không bị task kia ghi đè. Chưa làm vì cần hai build song song (tốn CPU) — để task riêng.
