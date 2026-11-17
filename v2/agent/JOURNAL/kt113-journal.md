# KT-113 — ảnh canonical sau vòng song song + review chéo

**Ngày**: 2026-11-18 · **Ngân sách**: 20 phút · **Dùng**: ~10 phút.

## Vì sao phải build lại

Vòng song song đổi `v2/lib/vifast-idl/src/lib.rs` (thêm test `info_shift_khop_spec`) và
`v2/test/boot/persist_host.py`. `v2/lib` nằm trong build-inputs ⇒ dấu vân tay đổi
`be9689a5c032cf69` → `bd4a0db3952459d0`.

## Quan sát đáng ghi: ảnh đổi **byte** dù mã chạy không đổi

```text
ảnh cũ  34ee7a25…  inputs=be9689a5c032cf69
ảnh mới 0b79a652…  inputs=bd4a0db3952459d0
```

Chỉ **mã test** của `vifast-idl` đổi (không vào ảnh kernel), nhưng sha **vẫn** đổi — vì ảnh **tự
khai** dấu vân tay build-inputs trong byte (`# build: inputs=…`). Đây là **hệ quả có chủ ý** của
KT-86/89: ảnh nói đúng nguồn của nó, nên đổi nguồn là đổi ảnh. Không phải hồi quy.

## Kết quả

```text
REPRO sha256=0b79a6525f0d0dba537dbc6bb5d607bb9189114c7ce215c64975a781bdc87e8e
      size=205440  inputs=bd4a0db3952459d0  dirty=0  rev=eb2ecbc299b3
      build=907912139bffcff0  text=130062
# đã ghim: v2/artifacts/0b79a652….bin

bash v2/test/parity/run.sh --mode serial --image v2/artifacts/0b79a652….bin → KET_LUAN=ok
bash v2/test/boot/shell-edge.sh --image v2/artifacts/0b79a652….bin → 8/8 ca PASS
```

## Chưa làm

Ma trận, persist, soak trên ảnh này — hành vi **không** đổi so với `34ee7a25…` (chỉ khác dấu vân tay
nhúng), và ba phép đó đã chạy trên `34ee7a25…`. Nếu Agent 0 muốn bằng chứng đầy đủ trên **đúng** ảnh
này thì chạy lại — tốn khoảng mười lăm phút.
