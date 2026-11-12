# BLOCKED — ViFastOs v2

Người ghi: **Agent 2**. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

## 1. REVIEW — chờ Agent 0 duyệt (không phải lỗi kỹ thuật)

| Task | Trạng thái | Nội dung chờ | Cần gì để mở |
|---|---|---|---|
| **KT-04d** | **REVIEW** | Checklist **5 mục/method** + **21 kịch bản** `.scn` viết chỉ bằng ABI (QD §4). Gửi kèm: `spec/abi.toml` lõi (Untyped, CNode, Tcb, SchedContext, Endpoint, Notification, Reply, MessageInfo, lỗi, quy ước gọi riscv64, layout slot 48 B `[TK]`), bảng ánh xạ v1→v2, danh sách object reserved, 21 `.scn` | Đúng một dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `v2/agent/QUEUE.md`. **APPROVED mở KT-05.** Duyệt không bằng chat, không tự suy ra |

**O1 (Exit) — ĐÃ CHỐT, không còn là câu hỏi mở.** Nguồn: **QD §1 Q-A + D2-25** (câu chữ QD ghi gọn "chuyển `Inactive` rồi gửi fault"; bản chốt đầy đủ là D2-25 + thiết kế §4.4 — `docs/v2/ViFastOs-v2-thiet-ke.md:285` và `:469`): `Exit` là **syscall 14 `Exit(code)`**; kernel lưu `exit_code`, TCB thành **`BlockedOnSend { ep: fault_ep, call: false }`** mang `FaultKind::Exit{code}` — giao qua **hàng gửi của `fault_ep`, không có Reply**; procsrv `Recv` xong TCB **mới** thành `Inactive`; không có `fault_ep` thì `Inactive` ngay. `Resume`/`Configure` xoá `exit_code`. `wait` là RPC của procsrv; `kill` = procsrv `Tcb.Suspend(Abort)` rồi Revoke cap TCB. Dải syscall **15..31 reserved**. Không mở lại O1; **không** chép nguyên văn QD:23 vào `abi.toml` khi làm KT-04a.

## 2. BLOCKED theo bảng `review/PATCH-BRIEF.md` §1

| Task | Chặn bởi | Cần gì để mở |
|---|---|---|
| KT-03 | KT-02 chưa `DONE` (brief xếp BLOCKED) | KT-02 xong (`cargo build` workspace rỗng xanh) ⇒ chuyển READY |
| KT-04b | KT-04a | `abi.toml` lõi parse được + bảng ánh xạ đủ |
| KT-04c | KT-04a | như trên |
| KT-05 | KT-04a + **APPROVED KT-04d** | dòng `APPROVED <ngày> Agent 0`; gen-check chỉ bật sau duyệt |
| KT-07 | KT-02, KT-03 | khung `v2/` build được + `v2 gate --fast` khung chạy |
| KT-08 | KT-07 | 50 boot riscv64 0 lỗi, có log |
| KT-09 | KT-08 | kịch bản end-to-end Retype→Configure→Call/Reply chạy |
| KT-10 | KT-09 | invariant tổng byte + preempt xanh |
| KT-11 | KT-10 | property test cây + Revoke xanh |
| KT-12 | KT-08 | sim harness chạy được |

Không có phụ thuộc vòng. Không có task nào bị chặn vì thiếu spec/thiếu quyết định: QD đã trả lời Q1..Q12 (QD §9).

## 3. Câu hỏi cho Agent 0

### Q-01 — KT-04c là **20** hay **21** kịch bản? — **ĐÃ TRẢ LỜI 2026-11-12 (D2-27)**

- **Chốt**: **21** kịch bản = 20 theo QD §4 + ca **"Exit khi procsrv chưa Recv"** (D2-27, Agent 0). KT-04c/KT-04d dùng 21, không hỏi lại, không tự đổi.
- Bối cảnh mâu thuẫn (giữ lại để tra cứu): **20** ở `docs/QUYET-DINH-sau-review.md:176` (QD §4) và `v2/agent/DECISIONS.md` Phần IV; **21** ở `v2/agent/QUEUE.md:104` + `docs/v2/ViFastOs-v2-thiet-ke.md:1068`; `review/PATCH-BRIEF.md` §1 ghi 20.
- Đóng cùng ngày với **D2-28** (KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`; `cargo build` xanh + mỗi tệp ≤ 1.500 dòng).

### YC-01 — Nhờ Agent 0 lật trạng thái QUEUE (mở 2026-11-12, Agent 2)

- **Hiện trạng sổ**: `QUEUE.md` vẫn ghi `KT-02 … READY` (`:100`), `KT-03 … BLOCKED` (`:101`), `KT-07 … BLOCKED` (`:108`); `v2/AGENTS.md` §3 vẫn viết "`check`/`gate --fast` **chưa có**".
- **Thực tế đã kiểm**: KT-01 `19659ee` (tag `v1-archive`), KT-02 `b4a8693` (build xanh 3 crate, 52/52 test), KT-03 `3222602` (`gate --fast` PASS), KT-07 (50/50 boot riscv64 + `.data+.bss` 0 B ≤ 256 KiB) — **phụ thuộc đã DONE**.
- **Vì sao Agent 2 không tự lật**: `AGENTS.md` §3 — Agent 2 không sửa `QUEUE.md`; bước 7 "`vifast-agentctl done`" chưa dùng được vì KT-06 chưa có.
- **Đề nghị**: Agent 0 ghi `KT-01/02/03/07 = DONE`, `KT-08 = READY` (dep KT-07 xong), và cập nhật `v2/AGENTS.md` §3. Không chặn Agent 2: các task đang chạy theo điều phối miệng.

Không có câu hỏi mở nào khác tại thời điểm này. Khi phát sinh: ghi ở đây kèm dẫn chứng (`tệp:dòng`), đề xuất cụ thể, và task kế sẽ làm thay.

## 4. Dòng ViScript **không** bị chặn bởi mục 1–2

`ST-01`, `ST-02b`, `ST-10` đang `READY` và độc lập với KT-04d/KT-03. Khi `v2/agent/QUEUE.md` hết task `READY` thì lấy task ViScript (D2-20); ngoại lệ duy nhất được chạy song song là `ST-02a` (chỉ là tài liệu).
