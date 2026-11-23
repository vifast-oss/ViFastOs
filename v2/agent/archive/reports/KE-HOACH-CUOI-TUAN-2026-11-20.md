# KẾ HOẠCH CUỐI TUẦN — ViFastOs tự chủ đến 09:00 JST thứ Hai 23/11/2026

**Người lập**: lead (Agent 1). **Ngày lập**: 2026-11-20 ~19:30 JST.
**Ủy quyền**: quyết định Agent 0 ngày 2026-11-20 (chạy tự chủ, tối đa 4 sub-agent, ba tầng kiểm).
**HEAD khi lập**: `c6127fa0` (`main`, đã push).

---

## 1. Mục tiêu (theo thứ tự ưu tiên)

1. **Khép tích hợp perf/toolchain + nghiệm thu API `NonReadyTarget`** (KT-241).
2. **Tăng tốc chu kỳ sửa → kiểm → tích hợp**, không giảm sức kiểm nghiệm thu.
3. **Tiến thêm hạng mục kernel đã có hợp đồng** (không dành cả cuối tuần chạy lại seed).

Không đặt mục tiêu "đóng thật nhiều task" hay "chạy máy đủ tải".

---

## 2. Ba tầng kiểm (đã hiện thực trong `v2/bin/v2`)

| Tầng | Lệnh | Nội dung | Khi chạy |
|---|---|---|---|
| **T0** | agent tự chạy | `cargo test -p <crate>`, test đỏ-trước, checker liên quan | trong lúc sửa |
| **T1** | `./v2/bin/v2 gate --dev` | fmt/lint + luật + checker + compile-fail + L0 + L1 1.000 + **smoke QEMU** (7 ảnh × 1 boot). Bỏ 8 bước selftest harness nặng (~350 s) | trước khi tích hợp nhóm thay đổi |
| **T2** | `./v2/bin/v2 gate --fast` | **đầy đủ 53 bước** (mặc định) | trên ứng viên tích hợp / bàn giao |

**Quy tắc gọi tên**: T1 **không** được gọi là "full gate PASS". Output của T1 in rõ
`v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, KHÔNG phải full gate)`. Không bỏ âm thầm bước
nào: mỗi bước bị bỏ in `--- <tên>: BỎ QUA (T1 — chỉ chạy ở T2)`.

**T2 bắt buộc ngay** (không trì hoãn) khi thay đổi chạm: đồng bộ SMP/memory ordering/vòng đời
đối tượng; page table/quyền truy cập/revoke/allocator; boot/linker/layout ảnh/toolchain; persist
và toàn vẹn dữ liệu; bản sửa cho lỗi chỉ tái hiện dưới stress.

---

## 3. Tầng L1 (A0 duyệt tạo)

| Mức | Seed | Dùng khi | Gọi là gì |
|---|---:|---|---|
| **L1-smoke** | 1.000 (mặc định `gate --fast`/`--dev`) | mọi lần T1/T2 | "L1 smoke", **không** tương đương full |
| **L1-medium** | 10.000 | thay đổi chạm sim/scheduler/IPC | "L1 medium" |
| **L1-full** | 100.000 × 4 cấu hình = 400k | ứng viên nghiệm thu | "L1 full (hợp đồng K6)" |

Kích thước smoke/medium chọn theo **đo thời gian** (xem §6), công bố rõ, không gọi tương đương.

---

## 4. Tái dùng bằng chứng (chống soak/L1 lặp vô ích)

Mỗi phép đo lưu **khóa đầu vào**:

```text
commit + fingerprint build-inputs + toolchain + cấu hình/seed + binary kiểm + ảnh (nếu có)
+ lệnh + kết quả + log
```

- Chỉ sửa tài liệu/runner **không** ảnh hưởng phép kiểm ⇒ **giữ bằng chứng cũ**, ghi lý do.
- **Không** chỉ nhìn SHA ảnh để quyết định tái dùng L1: L1 còn phụ thuộc source mô phỏng,
  harness, cấu hình, seed.
- **Nợ P20**: sha256 ảnh phụ thuộc thư mục build (5 lần build → 4 sha). Danh tính bền là dòng
  `# build: algo=… inputs=… dirty=…`. Ghi rõ khi trích bằng chứng.

**Ngân sách T2**: mặc định **≤ 1 đợt T2 theo lịch / 24 giờ**, cộng **1 đợt trên ứng viên bàn
giao cuối** nếu đầu vào đổi. Vượt lịch phải có lý do kỹ thuật (tái hiện lỗi, xác nhận bản sửa,
thay đổi rủi ro cao) và ghi lại.

---

## 5. Bốn luồng và quyền sở hữu

| Luồng | Sub-agent | Worktree | Phạm vi tệp |
|---|---|---|---|
| **1 — Kernel/API** | `kt241-accept` | `/tmp/kt242-s1` | `v2/agent/JOURNAL/kt241-attack/**`, `v2/agent/QUEUE.md`, `v2/agent/BYPASS-REGISTRY.md`, phiếu |
| **2 — Checker/review** | `wake-review` | `/tmp/kt242-s2` | `v2/agent/JOURNAL/kt242-wake-*` (chỉ repro + báo cáo) |
| **3 — Hạ tầng hiệu năng** | (chưa dispatch) | `/tmp/kt242-s3` | `v2/bin/v2`, `v2/tools/check-*.py` |
| **4 — L1 và kế hoạch** | (chưa dispatch) | `/tmp/kt242-s4` | `v2/sim/vifast-sim/tests/l1_k1c.rs`, `v2/agent/JOURNAL/kt242-l1-*` |

**Tệp chung do lead giữ**: `v2/bin/v2`, `v2/agent/STATE.md`, `v2/agent/QUEUE.md` (chỉ lead ghi
trạng thái cuối), `AGENTS.md`, `spec/`.

**Người viết không tự nghiệm thu phần mình** — mọi thay đổi phải qua review của lead hoặc
sub-agent khác.

---

## 6. Việc theo nhóm (thứ tự thực hiện)

### Nhóm A — khép việc đã duyệt
- [x] Perf merge + push (`417e2a18`).
- [x] Toolchain merge + push (`50958dd1`, `45534b78`).
- [x] Chọn rlib theo artifact Cargo (`00b82391`).
- [x] Review thời điểm panic của bản vá lint (ghi đúng, không "giống hệt").
- [x] Lưu bằng chứng khỏi `/tmp` (`v2/agent/JOURNAL/kt242-logs/` + SHA256SUMS).
- [x] Đối chiếu hợp đồng K6 (C1..C8 phủ; N1..N7 + S1 giữ mở).
- [ ] **KT-241**: 8 ô ma trận còn lại (D5, E1–E7) + QUEUE/BYPASS + phiếu → `kt241-accept`.
- [ ] **KT-184/KT-194**: review độc lập → `wake-review`.

### Nhóm B — giảm thời gian vòng phát triển
- [x] Tách T1/T2 trong runner (`gate --dev`).
- [ ] **C/P3**: cô lập tài nguyên + chạy song song các làn checker.
- [ ] **Cache kết quả kiểm theo đầu vào**: manifest + "có thể tái dùng"; chỉ skip khi khóa đủ rõ
      và có test invalidation; thiếu dữ liệu ⇒ chạy, không PASS mặc định.
- [ ] **Tối ưu L1**: profile (mô phỏng / so trace / cấp phát / khởi tạo), shard seed, giữ nguyên
      seed/config/op + kiểm tất định; so trace/digest cũ-mới.
- [ ] **Bảng thời gian cổng**: wall-time, CPU-time, critical path, tài nguyên.

### Nhóm C — gia cố có phạm vi
- [ ] Khép lệch hợp đồng ảnh hưởng luật wake (theo kết quả `wake-review`).
- [ ] Selftest chéo cho thành phần checker dùng chung còn thiếu.
- [ ] Fail-closed khi artifact/stamp/nguồn sinh thiếu hoặc cũ.
- [ ] Ma trận review + registry repro ổn định.
- [ ] Triage nợ chặn milestone gần nhất.

### Nhóm D — mở đường (design-only)
- [ ] **K4' bước 3**: khảo sát/design-only — phần việc, điều kiện vào, bất biến, test.
- [ ] **K5'**: kế hoạch + harness độc lập + tài liệu phụ thuộc + repro. **Chưa** triển khai kernel.

---

## 7. Nhịp chạy

| Mốc | Việc |
|---|---|
| 20/11 tối | Dispatch luồng 1–2; hiện thực T1/T2; đo T1 vs T2 |
| 21/11 | Luồng 3 (C/P3, cache, L1); review kết quả luồng 1–2; tích hợp |
| 22/11 | Luồng 4 (L1 + K4'/K5' design); T2 trên ứng viên; gia cố nhóm C |
| 23/11 08:00 | **Không mở task lớn mới**; khép tích hợp + hồ sơ |
| 23/11 09:00 | **Bàn giao** |

Không bắt đầu L1-full sát giờ bàn giao nếu biết không kịp. Không kill ngang tác vụ đang ghi dữ
liệu; ghi trạng thái rõ nếu còn phép đo đang chạy.

---

## 8. Điểm phải dừng (không tự quyết)

- Đổi ABI/spec hoặc luật kernel chưa có quyết định.
- Thêm `unsafe`/dependency cần ngoại lệ.
- Nới ngưỡng sản phẩm hoặc giảm phủ nghiệm thu.
- Đổi profile ảnh test thay ảnh sản phẩm.
- Mở K4'/K5' ngoài phạm vi §6 nhóm D.
- Thao tác có nguy cơ mất dữ liệu hoặc ảnh hưởng công việc khác.
- **E, nextest, phát hành v2.0**: chưa duyệt.

---

## 9. Báo cáo cuối (09:00 thứ Hai) phải có

1. Thay đổi sản phẩm nào đã hoàn tất.
2. Thời gian dành cho phát triển / kiểm nhanh / kiểm nặng.
3. Bao nhiêu lần L1-full/soak, mỗi lần vì lý do gì.
4. Bằng chứng nào tái dùng + khóa đầu vào tương ứng.
5. HEAD, ảnh, build-inputs, task mở, blocker.
6. Kết quả nào là **T1**, kết quả nào là **full gate/K6**.
