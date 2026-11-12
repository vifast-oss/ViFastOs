# DECISIONS — ViFastOs v2

Người ghi: **Agent 0**. Agent 2 **không** sửa tệp này; cần đổi thì ghi `BLOCKED.md` + đề xuất.
Nguồn: `docs/QUYET-DINH-sau-review.md` (QD) §2.3, §4, §8 và `review/src/ViFastOs-dossier.md` §19.1.
Không gian tên đã chốt (QD §2.1): quyết định v1 giữ `D-01..D-08` (**chỉ áp v1**); quyết định v2 là **`D2-01..D2-26`**. Số cũ `D-09..D-27` **hết hiệu lực**, không dùng lại.

---

## Phần I — D-01..D-08: quyết định v1 (**chỉ áp v1**)

Chép từ dossier §19.1 ("Tám quyết định chốt cho đợt chống flake"). Trạng thái trong v2 theo QD §2.3.

| Mã | Nội dung | Lý do (v1) | Trạng thái trong v2 |
|---|---|---|---|
| **D-01** | Giữ gate **mềm**; `--strict` chỉ dùng khi **đo** | Máy đo chưa ổn định; bật sẽ đỏ vì flake hạ tầng | Thay bằng **D2-07** (không retry) cho v2; D-01 chỉ còn hiệu lực cho v1 |
| **D-02** | **Đóng băng tính năng**, chỉ sửa lỗi + đo | Tránh thêm biến khi đang săn lỗi hiếm | **Hết hiệu lực** từ khi v1 lưu trữ (v2 thay thế v1, không mở tính năng trên v1) |
| **D-03** | T26: đánh **"ai/cái gì chen giữa"**, **không** đánh nhánh `fsck` (đã đóng) | fsck không phải nguyên nhân (đã chứng minh) | **Chuyển sang v2** dưới dạng **KT-T26** (QD §6) |
| **D-04** | Đo ở `MAX_IO=256` (**chỉ nhánh đo**); không bật ở gate/phát hành | 256 làm lộ race; 128 giữ cổng xanh | Giữ cho **KT-T26** (cấu hình tái hiện) |
| **D-05** | Teardown tối đa **2 chỗ**, mỗi chỗ **một vòng riêng**, hồi quy ⇒ **revert** | Gộp nhiều chỗ đã làm hỏng 0/3 cả ba arch | Giữ **tinh thần** trong v2: finalise cài theo nhóm, mỗi task tối đa 2 loại object, mỗi loại có test riêng |
| **D-06** | Soak nền ~600–1000 boot x86_64 trên máy `ps` | Cần mẫu lớn; máy rảnh | Giữ: mọi soak v2 chạy trên máy **ps**, N theo QD §5.3 |
| **D-07** | **Không** đụng nợ kiến trúc (không tách `lib.rs`) | Ưu tiên đóng lỗi trước | **Hết đối tượng** (v2 viết mới) — ghi rõ để không ai trích lại |
| **D-08** | **Không** đụng đồ hoạ / ViFastKey | Ngoài phạm vi đợt | **Còn hiệu lực tới K6**; G-line và ViFastKey chỉ mở sau K6 (= D2-23) |

Ghi chú: D11 (không sửa mò) → **D2-02**; D18 (hồi quy thì revert) → **D2-03**. Quy tắc 2.5 giữ nguyên văn: hai phiên kẹt cùng một ý thì **đổi nhánh**, không thử lần ba.

---

## Phần II — D2-01..D2-26: quyết định v2 (chép nguyên văn QD §8)

- **D2-01** v1 lưu trữ ở `v1-archive`, sổ lỗi mang sang (mục 6).
- **D2-02** không sửa mò (D11 cũ).
- **D2-03** hồi quy thì revert (D18 cũ).
- **D2-04** core không chứa FS/fd/block/console/args/demo; thay đổi ABI qua Agent 0.
- **D2-05** commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic tái hiện trên sim, kèm `.scn` đỏ trước.
- **D2-06** luật cấu trúc là cổng cứng.
- **D2-07** không retry ở mọi cổng; flake là T-INFRA.
- **D2-08** test kernel không nằm trong boot sản phẩm.
- **D2-09** trace luôn bật, không có cờ tắt.
- **D2-10** chỉ riscv64 tới K6.
- **D2-11** một nguồn sự thật trong `spec/`.
- **D2-12** đồng thời theo S1..S4 theo số đo; đọc trường công bố của hart khác được phép.
- **D2-13** kịch bản `.scn` chạy cả sim và QEMU.
- **D2-14** object trong Untyped qua objmem (API closure, Miri từ đầu).
- **D2-15** ABI đóng băng 5 thứ + version + golden (mục 4 QD).
- **D2-16** MCS: donation từ K1b, thuật toán refill sau K6.
- **D2-17** unsafe chỉ trong danh sách crate đóng.
- **D2-18** ứng dụng hệ thống viết bằng Rust std, crate thuần Rust; libc, wasm để sau.
- **D2-19** ViScript là ngôn ngữ script và shell chính; Linux qua máy ảo.
- **D2-20** OS trước, ViScript khi không còn task OS READY.
- **D2-21** giấy phép MIT OR Apache-2.0, kiểm bằng `cargo-deny` (allowlist mục 9.3 QD), `THIRD-PARTY.md` sinh tự động, license nhúng vào ảnh, không tạo NOTICE.
- **D2-22** duyệt spec bằng dòng APPROVED; Agent 2 soạn nháp spec ở trạng thái REVIEW, sau duyệt chỉ Agent 0 sửa.
- **D2-23** D-08 còn hiệu lực tới K6.
- **D2-24** mọi soak trên máy ps, N theo `n ≥ 3/p` (p mục tiêu 0,3 % ⇒ **N = 1.000**, song song 3 luồng).
- **D2-25** Tài liệu `docs/v2/ViFastOs-v2-thiet-ke.md` bản vá 2026-11-12 được Agent 0 duyệt, kèm errata `DUYET-AGENT0-vong-va.md`.
- **D2-26** Thư viện "giữ từ v1" (`vifast-pte`, `vifast-elf`, `vifast-acpi`, `vifast-fs`, `vifast-fsutil`, `vifast-virtio`) được **copy/port vào `v2/lib/`**, **không** path-dependency sang `crates/`; giữ SPDX và ghi `PORTED-FROM: crates/<tên>@ca2eaf5`. v1 vẫn đóng băng.

---

## Phần III — Duyệt

- **T04 cũ = KT-04d; APPROVED mở KT-05.**
- Mẫu: `APPROVED <YYYY-MM-DD> Agent 0`, ghi ngay dưới mục KT-04d trong `v2/agent/QUEUE.md`; nếu quyết định làm đổi nội dung ABI thì thêm một mục D2 mới ở tệp này. Không duyệt bằng chat. Không tự suy ra "coi như đã duyệt".
- Agent 2 soạn nháp ở `REVIEW`, sau duyệt **chỉ Agent 0** sửa `spec/` (D2-22).
- `ST-02a` (dòng ViScript) duyệt theo cùng mẫu trong `viscript/agent/QUEUE.md`; quyết định tương ứng ghi vào `viscript/agent/DECISIONS.md` (VD-).

## Phần IV — Bảng nguồn cho các con số đang có hiệu lực

| Số | Giá trị | Nguồn |
|---|---|---|
| slot cap | **48 B** `[TK]` (sửa 32 B) | QD §3.9 |
| `MAX_HARTS` | mặc định **8**, trần cấu hình **64** `[TK]` (sửa 256) | QD §3.15 |
| `vifast-core` | ≤ **25.000** dòng `[ƯL]` (sửa 15.000); tệp ≤ 1.500 dòng | QD §3.15 |
| `.data + .bss` | ≤ **256 KiB** `[TK]` `[GT]`, đo ở K0.5 | QD §3.5, §9 Q8 |
| mã lỗi kernel | **19** (bỏ `Restart`); syscall **0..13 + 14 Exit**; dải 15..31 reserved | QD §4 |
| soak v2 | **N = 1.000**, máy **ps**, 3 luồng, `n ≥ 3/p`, p = 0,3 % | QD §5.3 |
| L1 trong `--fast` | **1.000** seed (nightly **100.000**) | QD §5.2 |
| số kịch bản ABI | **20** `.scn` | QD §4 |
