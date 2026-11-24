<!-- SINH TỰ ĐỘNG bởi `./v2/bin/v2 nightly` — KHÔNG SỬA TAY (QD Q12). -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Trạng thái nightly — ViFastOs v2

- kết quả: PASS
- nguồn số đo: khối `scope=` bên dưới, trích nguyên văn từ log tóm tắt của chính lần chạy
  (`v2/agent/JOURNAL/nightly/<run-id>/summary.log`); commit log cùng tệp này.

```text scope=nightly/nightly-20261124-032448-4d4cbb62/summary.log
# nightly run=nightly-20261124-032448-4d4cbb62 head=4d4cbb62 dirty=1
# flags=--soak (profile=soak-x86_64-max; N=1000/1000/600/600; runs=3)
# started=2026-11-24T03:24:49Z ended=2026-11-24T03:53:38Z
# step=soak rc=0 wall_ms=1729000 label=Soak x86_64 -cpu max ảnh sản phẩm (N=600 × 3 luồng) log=step-1-soak.log
# result=PASS
```
