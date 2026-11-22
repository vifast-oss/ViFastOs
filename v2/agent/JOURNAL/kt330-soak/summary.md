# KT-330 — soak ứng viên cuối vòng 25 (tóm tắt)

Theo **D2-230**: chỉ commit **tóm tắt**; raw `boot-*.log` không vào Git.
Ứng viên: `5e635474` (KT-328 + KT-329), nền Chu kỳ 2 `ee8f2ca2`.

| Đợt | Kết quả | `BUILD_ID` | wall | `qemu_foreign` trước/sau |
|---|---|---|---|---|
| riscv64 | **0/1000** lỗi | `c5122d22419c6d05` | 240 s | 0/0 |
| x86_64 (qemu64) | **0/600** lỗi | `601d108c11649265` | 1027 s | 0/0 |
| x86_64 (-cpu max) | **0/600** lỗi | `601d108c11649265` | 1025 s | 0/0 |

## Bằng chứng nguyên văn (dòng tổng kết mỗi đợt)

```text
[riscv64] # infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=6.64 loadavg_5m=5.25 loadavg_15m=5.36 ncpu=12 boot_done=0 verdict=OK
[riscv64] # infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.55 loadavg_5m=5.21 loadavg_15m=5.34 ncpu=12 boot_done=1 verdict=OK
[riscv64] # soak: BUILD_ID=c5122d22419c6d05 sha256=f32eb11cc329f0b60002c4a953f47ba2d358f2845e8ddbe5c8aef9f5f00f83df git=5e635474f51757142a3c4673d87fea94cc898c6b
[riscv64] # soak: 0/1000 lỗi (PASS)
[x86_64 (qemu64)] # infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.08 loadavg_5m=5.08 loadavg_15m=5.30 ncpu=12 boot_done=0 verdict=OK
[x86_64 (qemu64)] # infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=3.78 loadavg_5m=5.18 loadavg_15m=5.30 ncpu=12 boot_done=1 verdict=OK
[x86_64 (qemu64)] # infra: cpu=qemu64 invalid=0 isolation_bad=0 guard_log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k5-x86_64-soak/qemu-guard.log
[x86_64 (qemu64)] # soak: BUILD_ID=601d108c11649265 sha256=fafbd8310a3e161b33fe163f4edc516e75c9cf6d3258ce8a26f2c7e5ac257507 git=5e635474f51757142a3c4673d87fea94cc898c6b
[x86_64 (qemu64)] # soak: 0/600 lỗi (PASS)
[x86_64 (-cpu max)] # infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=3.43 loadavg_5m=5.06 loadavg_15m=5.26 ncpu=12 boot_done=0 verdict=OK
[x86_64 (-cpu max)] # infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=5.08 loadavg_5m=4.51 loadavg_15m=4.80 ncpu=12 boot_done=1 verdict=OK
[x86_64 (-cpu max)] # infra: cpu=max invalid=0 isolation_bad=0 guard_log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k5-x86_64-soak/qemu-guard.log
[x86_64 (-cpu max)] # soak: BUILD_ID=601d108c11649265 sha256=fafbd8310a3e161b33fe163f4edc516e75c9cf6d3258ce8a26f2c7e5ac257507 git=5e635474f51757142a3c4673d87fea94cc898c6b
[x86_64 (-cpu max)] # soak: 0/600 lỗi (PASS)
```

## Cổng

- `rc_124=0` cả ba đợt (không boot nào bị treo).
- `qemu_foreign=0` **hai mẫu** mỗi đợt ⇒ không có `MEASUREMENT_INVALID`.
- **Không retry** (D2-07).
