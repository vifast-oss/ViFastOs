# KT-242 — hồ sơ bằng chứng L1 (digest trace, chia lô, ba mức)

Người ghi: Agent 2 (luồng L1/kế hoạch) · Ngày: 2026-11-20 · Task: **KT-242**
Nhánh: `task/kt242-l1` · HEAD trước task: `21cc75cd`
Máy: macOS 27.0.1 arm64, **12 core** · Rust **1.100.0** · profile **debug** (`cargo test`)

Tài liệu đọc kèm: [`../kt242-l1-profile.md`](../kt242-l1-profile.md) (hồ sơ đo),
[`../kt242-l1-tiers.md`](../kt242-l1-tiers.md) (ba mức), `v2/tools/l1-tier.sh` (công cụ).

## 1. Tệp trong thư mục này

| Tệp | Nội dung |
|---|---|
| `digest-truoc-sau.log` | digest trace **trước** và **sau** tối ưu, 1.000 và 10.000 seed × smp 1/2/4/8, kèm bộ đếm tổng |
| `tier-smoke-lo.log` | mức `smoke` chạy **chia lô** (22 lô): digest từng lô + XOR của chúng |
| `tier-medium-lo.log` | mức `medium` chạy **chia lô** (24 lô): digest từng lô + XOR của chúng |
| `SHA256SUMS` | sha256 của ba tệp trên |

## 2. Kết luận mà ba tệp này chứng minh

**(a) Tối ưu không đổi trace.** Digest **trước** = digest **sau**, tuyệt đối, ở **16/16** ô
(2 mức seed × 4 cấu hình smp). Bộ đếm tổng (`ipc_sent`, `ipc_delivered`, `ipc_dropped`,
`vm map/walk/unmap/fault/shootdown`) cũng trùng **từng số**.

**(b) Chia lô không đổi trace.** XOR digest của **mọi** lô = digest chạy **một** tiến trình:

| Mức | smp | XOR các lô | digest một tiến trình |
|---|---:|---|---|
| smoke | 1 | `237f3fa69d2bf1ea` | `237f3fa69d2bf1ea` |
| medium | 1 | `f11e8bf72f8083fb` | `f11e8bf72f8083fb` |
| medium | 2 | `65d347211408784b` | `65d347211408784b` |
| medium | 4 | `ed6726edf194cca9` | `ed6726edf194cca9` |
| medium | 8 | `744ad4c446c308ef` | `744ad4c446c308ef` |

Đây là bằng chứng ở mức **toàn bộ trace** (không phải mẫu): digest là XOR của
`vifast_sim::trace_hash(trace)` **từng seed**, và XOR giao hoán nên gộp lô được.

## 3. Tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.100.0

# (a) digest một tiến trình — in dòng "# L1 DIGEST: ..."
VIFAST_L1_DIGEST=1 VIFAST_SEEDS=1000 VIFAST_SMP=1 <bin> --nocapture

# (b) chạy chia lô + gộp (in bảng + digest gộp)
bash v2/tools/l1-tier.sh --tier smoke  --jobs 11
bash v2/tools/l1-tier.sh --tier medium --jobs 11
```

`<bin>` = đường dẫn `cargo test --no-run --message-format=json` in ra cho test target `l1_k1c`
(`v2/tools/l1-tier.sh` tự tìm).

## 4. Giới hạn

- **Máy đo không rảnh**: trong lúc đo có agent khác chạy `rustc` (97 % CPU), load average
  **5,6 → 11,3**. Số **wall** vì thế là **cận trên**; số **digest** và **bộ đếm** thì không phụ
  thuộc tải.
- **Chưa đo L1-full** (100k × 4) trong task này — xem `../kt242-l1-profile.md` §5.4 và §6.
- Digest là **bằng chứng**, **không** phải ngưỡng cổng: không `assert` nào dựa trên nó.
