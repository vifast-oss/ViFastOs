# KT-94 — ảnh canonical mới sau KT-89/92/93 + kiểm nhanh

**Ngày**: 2026-11-17 · **Ngân sách**: 30 phút · **Dùng**: ~15 phút.

## Vì sao

Ảnh phát hành `291502d2…` (dùng cho toàn bộ phép đo KT-88) build **trước** ba thay đổi làm đổi byte ảnh:

- **KT-89**: danh tính nhúng đổi từ `rev=<12 hex>` sang `inputs=<16 hex>` (dấu vân tay build-inputs);
- **KT-92**: `build.rs` truyền `--remap-path-prefix` cho ba chỗ gọi `$RUSTC` trực tiếp;
- **KT-93**: build-inputs mở rộng (thêm ba tệp script) ⇒ dấu vân tay đổi.

Build lại từ HEAD hôm nay ra sha **khác** — đúng như cảnh báo trong `K6-EVIDENCE-INDEX.md` §1.

## Ảnh mới

```text
REPRO sha256=dd9f7050c1a0a6f63edca611544ff144b40246cc3ec29289fc08b25f5f2feadc
      size=205440  inputs=3caf5e7d5f0a3321  dirty=0  rev=e678efa682b5
      build=907912139bffcff0  text=130062
```

Đã **ghim** vào `v2/artifacts/dd9f7050….bin` bằng `v2/tools/pin-artifact.sh` (script kiểm dấu vân
tay ảnh = cây hiện tại trước khi ghim — xem §"Cải tiến" dưới).

## Kiểm nhanh (không phải phép đo đầy đủ)

```text
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/dd9f7050….bin
# parity: total=28 ok=6 deferred=22 active=6 (active_da_chay=6)
# parity: not_ok=0 timeout=0 khong_co_shell=0
# parity: KET_LUAN=ok (exit 0)
```

⇒ Ảnh mới **boot được**, shell lên, sáu lệnh `active` chạy đúng. **Chưa** chạy lại ma trận/soak/persist
trên ảnh này — đó là việc phải làm **trước khi** dùng nó cho bất kỳ tuyên bố K6 nào.

## Cải tiến `pin-artifact.sh` (cùng lượt)

Trước: chỉ kiểm ảnh **có** dòng `# build:`. Nay kiểm thêm:

- **dấu vân tay ảnh = dấu vân tay cây hiện tại** — lệch ⇒ **từ chối** (mã thoát năm), trừ `--allow-stale`;
- **ảnh tự khai `dirty=1`** ⇒ **từ chối** (mã thoát sáu), trừ `--allow-dirty`.

Lý do: ghim một ảnh build từ cây khác **chính là** lớp lỗi D2-108A (đo ảnh A, trích số từ ảnh B).
Đã kiểm cả hai nhánh: ảnh cũ `dirty=1` ⇒ từ chối; ảnh mới `dirty=0` + khớp ⇒ ghim được.

## Việc còn lại

- Ma trận + soak + persist trên `dd9f7050…` nếu Agent 0 muốn dùng ảnh này cho K6 (dài, để ban ngày).
- `291502d2…` vẫn là ảnh của **phép đo KT-88**; không trộn số giữa hai ảnh.
