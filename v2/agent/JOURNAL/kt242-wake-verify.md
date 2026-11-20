# KT-242 — lead xác minh bản vá luật `wake` trên HEAD mới

**Người xác minh**: lead (Agent 1) — **không** phải người viết bản vá (bản vá do sub-agent
`wake-review` viết ở `c0b48f3b`). **HEAD lúc xác minh**: `0018f5ab`.
**Vì sao**: reviewer ghi rõ "bản vá khép N1–N6 nhưng **không** tự đóng KT-184/KT-194 — cần một
vòng review độc lập trên HEAD mới". Đây là vòng đó, ở mức **chạy lại toàn bộ repro đã biết**.

---

## 1. Lớp fixture (không cần cargo) — `fixture-probes.py`

| Ca | Kỳ vọng | Kết quả `[thuong]` | `[strict]` | Đạt |
|---|---|---|---|---|
| **N1** `impl` lồng trong `wake_step` | ĐỎ | `E_STRICT_STATE_WRITE` | `E_STRICT_STATE_WRITE` | ✅ |
| **N2** `fn` lồng trong `wake_step` | ĐỎ | `E_STRICT_STATE_WRITE` | `E_STRICT_STATE_WRITE` | ✅ |
| **N5a** luật 3 cùng dòng `#[cfg(test)]` | ĐỎ | `E_ARCH_DOORBELL` | `E_ARCH_DOORBELL` | ✅ |
| **N5b** luật 1 cùng dòng | ĐỎ | `E_CHANNEL_PRODUCER` | `E_CHANNEL_PRODUCER` | ✅ |
| **N5c** luật 2 cùng dòng | ĐỎ | `E_CONSUMER` | `E_CONSUMER` | ✅ |
| **N6** `include!("\0")` | ĐỎ, **không** crash | `E_STRICT_WAKE_INCLUDE` | `E_STRICT_WAKE_INCLUDE` | ✅ |
| **ĐC+** R1 cùng dòng `#[cfg(test)]` | ĐỎ | `E_STRICT_TEST_ONLY_WAKE` | `E_STRICT_TEST_ONLY_WAKE` | ✅ |
| **ĐC+** R1 trong `#[cfg(test)] mod` | **XANH** | `errors=0` | `errors=0` | ✅ |
| **N3b** `cfg_attr` lồng trỏ ra ngoài `v2/lib` | ĐỎ | `E_STRICT_WAKE_INCLUDE` | `E_STRICT_WAKE_INCLUDE` | ✅ |
| **N3** `cfg_attr` lồng, đích **trong** `v2/lib` | XANH (latent) | `errors=0` | `errors=0` | ✅ (đúng như ghi) |
| **DEBT-KT227-1** token `wake` trong `v2/lib` | XANH (latent, ngoài phạm vi) | `errors=0` | `errors=0` | ✅ (đúng như ghi) |

## 2. Lớp cây thật (cargo + 3 lớp) — `run-repro.sh`

| Ca | `cargo` | lớp 1 thường | lớp 1 `--strict` | lớp 2 dep-info | Đạt |
|---|---|---|---|---|---|
| **r3a** (N1, cửa thứ bảy) | **RC=0** (biên dịch được) | `errors=1` | `errors=1` | `errors=1` | ✅ |
| **binpath** (N3, `[[bin]] path` ngoài `src/`) | RC=0 | `errors=1` | `errors=1` | `errors=1` | ✅ |
| **rs-case** (N4, tệp `.RS` hoa) | RC=0 | `errors=1` | `errors=1` | `errors=1` | ✅ |

Điểm quan trọng: **`cargo` vẫn RC=0** — tức mã vẫn biên dịch được, đúng bản chất "đường lọt";
lớp kiểm mới là thứ bắt nó. Cả **ba** lớp đều ĐỎ.

## 3. Cây sạch sau xác minh

`git status --porcelain` sau ba repro: **rỗng** (không tệp nào dưới `v2/kernel`/`v2/sim`/`v2/lib`
bị sửa). `run-repro.sh` chỉ khôi phục cây sản phẩm, không đụng bản vá checker.

## 4. Kết luận

- **N1–N6 đã đóng** và **xác minh độc lập** trên `0018f5ab` ⇒ **WB-19 → CLOSED**.
- **KT-184/KT-194 vẫn giữ `REVIEW`**: vòng này chỉ **chạy lại repro đã biết**; để đóng cần một
  vòng **tấn công mới** (nghĩ đường lọt chưa ai thử), không phải chạy lại danh sách cũ. Ghi rõ để
  không ai suy "đã xác minh" thành "đã đóng".
- Nợ còn lại **không đổi**: **DEBT-KT227-1** (latent), **L2** (ba mẫu phạm vi vênh, không có
  đường lọt sống), closure trong thân cửa (không thoát được vì cửa trả `Result`).
