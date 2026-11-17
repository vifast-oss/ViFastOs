# KT-121 — ảnh canonical sau vòng phát triển + review toàn dự án

**Ngày**: 2026-11-18 · **Ngân sách**: 20 phút · **Dùng**: ~10 phút.

## Vì sao build lại

Vòng này đổi **kernel**: `proc_fs.rs` (KT-118 vá PANIC), `proc_demo.rs` (KT-119 TAP 4 dùng trạng thái
thật), `ktest_con.rs` + `ktest_core.rs` (KT-116 hai test xanh giả), `shell_user.rs` (KT-114 bốn lệnh).

## Kết quả

```text
REPRO sha256=5e2ec2193c28b44f89d86093420a2b1beebb9d0ac498dd60cf1b914bff1b53a5
      size=205440  inputs=685a7fe12d2cd26d  dirty=0  rev=c5071d32677f
      build=25072e009196ae4a  text=130014
# đã ghim: v2/artifacts/5e2ec219….bin

bash v2/test/boot/ktest.sh 1 matrix  → # KTEST: 21/21 boot sạch · ktest.sh: PASS
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/5e2ec219….bin → KET_LUAN=ok
bash v2/test/boot/shell-edge.sh --image v2/artifacts/5e2ec219….bin → 8/8 ca PASS
bash v2/test/boot/persist.sh → persist.sh: PASS (7 pha, distinct_process=1)
./v2/bin/v2 check → PASS
```

## Ghi chú

`text=130014` (trước là `130062`) — nhỏ hơn 48 B vì KT-119 bỏ đoạn giải mã `entry` trong test và
KT-118 thêm một phép `min`. Ảnh vẫn 205.440 B (cùng bậc).
