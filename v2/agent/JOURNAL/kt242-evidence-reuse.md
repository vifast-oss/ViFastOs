# KT-242 — thiết kế "tái dùng bằng chứng theo khóa đầu vào" (A0 §3.A)

**Người viết**: lead (Agent 1). **Ngày**: 2026-11-20. **Trạng thái**: thiết kế, **chưa** hiện thực.
**Vì sao**: A0 yêu cầu không chạy lại L1-full/soak khi bằng chứng còn hiệu lực, nhưng **không**
được PASS mặc định khi thiếu dữ liệu.

---

## 1. Khóa đầu vào của một phép đo

Mỗi phép đo (L1, soak, ktest matrix, persist, conformance, gate) lưu **manifest**:

```json
{
  "kind": "l1-full",
  "commit": "21cc75cd75237c36e8cc41a959a92c23a187eba5",
  "build_inputs": "54f3b59fc08f3221",
  "toolchain": "1.100.0",
  "config": {"smp": [1,2,4,8], "seeds": 100000, "op": 64},
  "binary": {"path": "…/l1_k1c-…", "sha256": "…"},
  "image": {"sha256": "d915b977…", "build_line": "algo=kt134-v4 inputs=54f3b59fc08f3221 dirty=0"},
  "command": "…",
  "result": "PASS",
  "log": "v2/agent/JOURNAL/kt242-logs/l1-smp1.log",
  "log_sha256": "…",
  "measured_at": "2026-11-20T18:00:00+09:00"
}
```

**Khóa so khớp** = `kind` + `commit` + `build_inputs` + `toolchain` + `config` + `binary.sha256`
(+ `image.build_line` nếu phép đo dùng ảnh).

## 2. Khi nào được tái dùng

| Thay đổi | Tái dùng? | Lý do |
|---|---|---|
| Chỉ tài liệu (`v2/docs/**`, `v2/agent/**`, `*.md` ngoài build-inputs) | **CÓ** | không nằm trong build-inputs, không ảnh hưởng binary |
| Runner/harness **không** nằm trong đường đo (ví dụ sửa `gate --dev`) | **CÓ**, ghi lý do | binary kiểm không đổi |
| Sửa mã sản phẩm (bất kỳ tệp trong build-inputs) | **KHÔNG** | `build_inputs` đổi |
| Đổi toolchain | **KHÔNG** | binary đổi |
| Đổi seed/config/op | **KHÔNG** | khóa `config` đổi |
| Đổi binary kiểm (kể cả rebuild cùng nguồn) | **KHÔNG** nếu sha khác | khóa `binary.sha256` đổi |
| Đổi ảnh (sha khác) nhưng `build_line` giống | **CÓ** cho phép đo **không** phụ thuộc byte ảnh; **KHÔNG** cho soak/ktest | P20: sha phụ thuộc thư mục build |

**Không** chỉ nhìn SHA ảnh để quyết định tái dùng L1 — L1 phụ thuộc source mô phỏng, harness,
cấu hình, seed.

## 3. Quy tắc fail-closed

- Thiếu manifest / thiếu trường / log không còn ⇒ **chạy lại**, không PASS.
- Manifest có nhưng `log_sha256` không khớp ⇒ **chạy lại**.
- `result` khác `PASS` ⇒ không tái dùng.
- Không chắc ⇒ chạy.

## 4. Test invalidation (bắt buộc khi hiện thực)

1. Manifest khớp hoàn toàn ⇒ `reuse`.
2. Đổi `commit` ⇒ `miss`.
3. Đổi `build_inputs` ⇒ `miss`.
4. Đổi `config.seeds` ⇒ `miss`.
5. Đổi `binary.sha256` ⇒ `miss`.
6. Xóa log ⇒ `miss`.
7. Sửa `log` (sha khác) ⇒ `miss`.
8. Manifest thiếu trường ⇒ `miss` (không exception).

## 5. Ghi chú hiện thực (khi làm)

- Công cụ đề xuất: `v2/tools/check-evidence.py` (đọc manifest, in `reuse|miss` + lý do).
- **Không** tự động skip trong `gate --fast` ở đợt đầu: chỉ **báo** "có thể tái dùng" (A0 §4 nhóm B:
  "Trước hết lập manifest và báo 'có thể tái dùng'").
- Chỉ bật tự động skip sau khi có test invalidation xanh và có review.
