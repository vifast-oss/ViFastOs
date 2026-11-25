# KT-416 — RFC Hợp nhất Kiến trúc & ABI Sạch

**Ngày:** 2026-11-26 · **Nhánh:** `task/kt416-rfc-abi` · **Base:** `main` @ `fc998aa8`
**Trạng thái:** `REVIEW` — RFC đã soạn, **chưa được duyệt**; `KT-417` vẫn `BLOCKED`.

## 1. Diễn biến

- Sub-agent `kt416` khảo sát 8 nhóm trong ~3 h, ghi được journal phân tích (6 phát hiện) nhưng
  **hết ngân sách trước khi ghi tệp đầu ra**; worktree bị thu hồi ⇒ journal của agent mất.
- **Lead hoàn tất** phần đầu ra từ 6 phát hiện đã đọc được + hiện trạng mã đã biết trong phiên:
  - `review/agent0/GOI-ABI-KIEN-TRUC.md` — RFC 8 nhóm (mỗi nhóm: hiện trạng đo được, đề xuất,
    rủi ro + cách kiểm, câu hỏi cho Agent 0).
  - `review/agent0/GOI-ABI-KIEN-TRUC.json` — bản tóm tắt máy đọc được.

## 2. Sáu phát hiện đo được (từ khảo sát của agent)

1. **`16 hằng EP_*` trong kế hoạch, đo được `17`** — `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs`
   khai 17 `pub const EP_*` + 8 `SLOT_*`/`LOOP_*` cùng tệp ⇒ `BootCapLayout` phải phủ **17**, không phải 16.
2. **`check_kernel.py:277` đòi `[limits.plat]` tồn tại** ⇒ xoá mục đó khỏi `spec/kernel.toml` ở
   `KT-417` **bắt buộc** sửa cả `required` list của máy kiểm (nếu không `E_SECTION`).
3. **`Tcb.BindNotification` (13) / `UnbindNotification` (14)** có số trong spec nhưng chỉ có model API
   (`v2/kernel/vifast-core/src/core_sync.rs:176,208`), chưa có nhánh dispatch trong `invoke.rs`.
4. **`Frame.MapIo`/`UnmapIo` + `IOSpace` chưa có một dòng mã nào** ngoài enum `ObjType::IOSpace`
   (`v2/kernel/vifast-core/src/error.rs:63`) và ghi chú `vifast-virtio/src/features.rs:22`
   ("**không** đàm phán `VIRTIO_F_ACCESS_PLATFORM` — không có IOMMU: DMA dùng PA thật").
5. **`[profiles]` chưa tồn tại** trong `spec/kernel.toml`; `StaticStore`/`UntypedStore` đã có
   (`v2/kernel/vifast-core/src/store.rs:271,408`) nhưng chưa có tên profile trong spec.
6. **Vùng lưu FPU/SIMD chưa tồn tại**: `v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs` chỉ 62 dòng
   đọc/ghi `CR0`/`CR4`; `TcbCore` (`v2/kernel/vifast-core/src/tcbdata.rs:24`) **không** có trường FPU.

## 3. Kết quả

- 8 nhóm, mỗi nhóm 4 mục; bảng TL;DR §0 ghi rõ nhóm nào đổi `abi_version` 1→2 (nhóm 2, 3, 4, 6, 8)
  và nhóm nào chỉ đổi `spec/kernel.toml` (nhóm 1, 7) hoặc chỉ ghi `planned` (nhóm 5).
- **Không có thay đổi mã nào**; **không có byte nào của `spec/` bị sửa**.
- `KT-417` vẫn `BLOCKED` cho tới `APPROVED` riêng của Agent 0.

## 4. Việc còn lại / rủi ro

- RFC **chưa được duyệt** — cần Agent 0 ký `APPROVED <ngày> Agent 0` trong `QUEUE.md`.
- Câu hỏi lớn nhất: chấp nhận `abi_version = 2` trong một lần (5 nhóm wire) hay tách đợt?
- Nợ công cụ: `check_kernel.py:277` đòi `[limits.plat]` — nhóm 1 phải sửa **cả** máy kiểm.
