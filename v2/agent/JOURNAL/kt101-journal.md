# KT-101 — conformance trên HEAD sau các thay đổi đêm

**Ngày**: 2026-11-18 · **Mục đích**: chạy lại bộ conformance (chưa chạy từ trước KT-92).

## Kết quả

```text
bash v2/test/conformance/run.sh --boot
# sim trace: 61 dòng → /tmp/k3-kt55/sim-trace.log
# trace sim: OK (61 dòng, 5 ca — 22:16 23:19 24:7 25:7 26:12)
scn-check: PASS (27 tệp, 0 blocked, 16 nhóm)
# qemu: ktest.sh 1 matrix (4 ảnh × smp 1/2/4)
ok - trace-compare: 61/61 dòng khớp, 5/5 ca (22:16 23:19 24:7 25:7 26:12), build=7304a40491224327
# trace-compare PASS (/tmp/k3-kt55/sim-trace.log ↔ /tmp/k3-ktest/ktest-smp2-1.log)
```

## Đọc kết quả

- **`trace-compare` 61/61 dòng khớp** giữa trace của **sim** và log QEMU thật ⇒ mô hình và kernel
  vẫn khớp nhau sau khi `build.rs` đổi cách gọi `$RUSTC` (KT-92) và sau khi build-inputs mở rộng.
- **`scn-check` PASS** 27 tệp, 0 blocked.
- Đây là phép kiểm **độc lập** với ma trận: ma trận chỉ đếm boot sạch; `trace-compare` so **nội dung**
  từng dòng trace giữa hai môi trường.

## Ghi chú

`build=7304a40491224327` là BUILD_ID của ảnh test dùng cho trace-compare — **khác** `build=907912139bffcff0`
của ảnh phát hành (khác feature set). Không phải mâu thuẫn.
