<!-- SINH TỰ ĐỘNG bởi `./v2/bin/v2 nightly` — KHÔNG SỬA TAY (QD Q12). -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Trạng thái nightly — ViFastOs v2

- kết quả: FAIL
- nguồn số đo: khối `scope=` bên dưới, trích nguyên văn từ log tóm tắt của chính lần chạy
  (`v2/agent/JOURNAL/nightly/<run-id>/summary.log`); commit log cùng tệp này.

```text scope=nightly/nightly-20261122-135115-d5f84596/summary.log
# nightly run=nightly-20261122-135115-d5f84596 head=d5f84596 dirty=1
# flags=--soak (3 arch; N=1000/1000/600; runs=3)
# started=2026-11-22T13:51:15Z ended=2026-11-22T14:42:36Z
# step=soak rc=0 wall_ms=251000 label=Soak riscv64 ảnh sản phẩm (N=1000 × 3 luồng) log=step-1-soak.log
# step=soak rc=0 wall_ms=993000 label=Soak aarch64 ảnh sản phẩm (N=1000 × 3 luồng) log=step-2-soak.log
# step=soak rc=1 wall_ms=1837000 label=Soak x86_64 ảnh sản phẩm (N=600 × 3 luồng) log=step-3-soak.log
# result=FAIL
```
