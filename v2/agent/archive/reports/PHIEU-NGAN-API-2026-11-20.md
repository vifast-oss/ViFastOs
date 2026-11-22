# PHIẾU NGẮN — chữ ký API và phạm vi sửa (D2-192)

**Từ**: Agent 1. **Gửi**: Agent 0. **Ngày**: 2026-11-20.
**Căn cứ**: D2-187..D2-193. **Trạng thái**: 4 luồng A/B/C/D đã bàn giao, gate PASS trên commit cuối.

---

## 1. Trạng thái một khối

```text
HEAD          c7469b9f   (main, đã push, chưa push: 0)
build-inputs  233 mục, dấu vân tay 43df126a4a6702ec, dirty=0
gate --fast   PASS trên a2e18cdf (17 m 12 s, 52 bước OK) — gate trên COMMIT CUỐI (D2-191 §3)
task          DONE=211  READY=0  BLOCKED=0  REVIEW=2 (KT-184, KT-194)
checker       lớp tĩnh 331 ca; lớp dep-info 52 ca; cả hai errors=0 trên cây thật
K6            6/6 ĐẠT trên ảnh 639ca6d1… — CHƯA đo lại (chưa đổi API core)
```

---

## 2. Kết quả bốn luồng

| Luồng | Task | Kết quả | Điểm chính |
|---|---|---|---|
| **A** | KT-237 | **DONE** | **216** call-site (62 sản phẩm + 154 test), phân loại **216/216**; đề xuất `NonReadyTarget`; **WB-16 6/6 + WB-17 7/7 chặn ở biên dịch**; primitive thô `E0624` |
| **B** | KT-238 | **DONE** | Phạm vi từ `cargo metadata`: **22 crate bảo vệ / 1 từ chối rõ**; **WB-18 ĐÓNG**; fail-open **0**; selftest lớp 2 **37 → 52** |
| **C** | KT-239 | **DONE** | **UFCS đã vá** (đọc `args[2]`); **36 ca repro bền**; test âm **4/4 kiểm đúng nguyên nhân**; selftest lớp 1 **295 → 331** |
| **D** | KT-240 | **DONE** | **10 thành phần dùng chung** (5 thiếu selftest chéo); hợp đồng **63/68 khớp**; **ma trận 4 nhóm/20 ô** |

---

## 3. Chữ ký API đề xuất (luồng A) — **cần ngài duyệt trước khi sửa mã**

```rust
pub enum NonReadyTarget {
    Inactive, Running { hart: HartId },
    BlockedOnSend { .. }, BlockedOnRecv { .. }, BlockedOnReply { .. },
    BlockedOnNotification { .. }, BlockedOnAddress { .. },
    Sleeping { reason: SleepReason }, Stopping, Dead,
}
impl Machine {
    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError>;
}
```

**Vì sao chặn được**: `Ready` **không phải** giá trị của `NonReadyTarget`; **không** có `From`/`TryFrom` công khai từ `TcbState`; `into_state()` **private**. Đo thật: WB-16 6/6 dạng `E0308`; WB-17 7/7 dạng `E0308`/`E0631`.

**5 cửa `Ready` có ngữ nghĩa** (khớp 15 mục `NON_WAKE_READY_SITES` + `wake_step`):
wake (`wake_step`/`wake_from`/`wake_from_unlink`) · preemption (`preempt_current`) · yield (`yield_to_ready` + `pub tcb_yield`) · khởi tạo (`pub start_tcb`) · suspend/abort (`suspend_to_ready`).

**Ranh giới primitive thô**: module `state_write` — core `src/state_write.rs` (sibling, `pub(crate)` sẵn có), sim `src/kobj/state_write.rs` (module con của `kobj`, đọc field private của cha). `fn write_state` **private**. Đo: module sản phẩm gọi ⇒ **`E0624`** trên **cả core và sim**.

**Chi phí**: **229 call-site**, **55 tệp**, **2 module mới**. **Có đổi `pub` API** (model `[TK]`, không phải ABI): `transition_tcb`, `block_current` đổi kiểu; thêm `start_tcb`, `tcb_yield`. `v2/lib` **không** ảnh hưởng. **Không** đổi `spec/`.

---

## 4. Tám câu hỏi cần ngài quyết (từ luồng A)

1. **Enum `NonReadyTarget`** (đề xuất) hay **newtype `NonReady(TcbState)`** field private?
2. Giữ tên **`transition_tcb`** (đề xuất) hay đổi `transition_tcb_nonready`?
3. Chấp nhận thêm **2 cửa `pub`** `start_tcb`/`tcb_yield` cho `core_bridge`/`proc_rt` (crate ngoài)?
4. `block_current` đổi sang `NonReadyTarget` (đề xuất) hay giữ `TcbState` + kiểm runtime?
5. **Task nào sửa checker**, và có cần tách **trước** task API không? (D2-192: gate độc quyền)
6. Ca test `kt19_sched.rs:702` (`block_current(0, Ready)` kỳ vọng `InvalidArgument`) — **xoá** hay chuyển thành ca kiểm tầng checker?
7. `reply_with` xếp **"yield"** (đề xuất) hay "dispatch"?
8. `NonReadyTarget::Running` giữ trong enum (đề xuất — `ALLOWED` chứng minh `Running` chỉ vào từ `Ready`) hay tách cửa riêng?

**Chặn cứng**: khi đổi API, `check-wake-paths.py` + `check-wake-depinfo.py` sẽ **ĐỎ** (`NonReadyTarget::*` bị phân loại `unknown`; `NON_WAKE_READY_SITES`/`WAKE_DOORS` hết khớp) ⇒ **phải sửa checker cùng task hiện thực**, nếu không `v2 check` đỏ toàn cây. Đề xuất của A: mọi `transition_tcb(` là non-Ready theo kiểu ⇒ bỏ `_var_may_be_ready`; thay allowlist bằng 5 cửa `Ready`; thêm luật cấm `write_state` ngoài `state_write.rs`.

---

## 5. Việc đã đóng trong đợt này

| Mã | Trạng thái | Bằng chứng |
|---|---|---|
| **WB-18** | **CLOSED** | KT-238: lớp 2 trước XANH → sau **ĐỎ** `E_DEPINFO_WAKE` tại `out/evil.rs:3`; đối chứng dương XANH |
| **WB-17 (UFCS)** | **CLOSED một phần** | KT-239: đọc `args[2]` khi có `::`; 2 ca UFCS ĐỎ; fn-pointer/generic/trait-object **còn sống** chờ KT-237 |
| **WB-16** | **OPEN** | 12 ca "ghi nhận sống" là **đèn báo**: khi KT-237 vá API, 26 ca này sẽ ĐỎ và buộc đổi kỳ vọng |
| **WB-17 (fn-pointer/generic/trait-object)** | **OPEN** | chờ KT-237 |

---

## 6. Lệch sổ đã sửa (từ audit D)

- STATE HEAD `59bede14` → `c7469b9f`; DONE `206` → `211`.
- Hợp đồng dòng 9 ghi KT-211/213 REVIEW trong khi QUEUE đã DONE — **còn mở**, cần sửa ở đợt sau.
- Phát hiện thêm: tham số `tr` của `_check_wake_text` là **biến chết** (hàm tự tính `_test_char_ranges`, bỏ qua `tr` caller truyền) — mồi lệch tương lai, **chưa sửa**.

---

## 7. Đề nghị bước tiếp theo

1. **Ngài trả lời 8 câu hỏi §4** — đặc biệt câu 1 (enum vs newtype) và câu 5 (task nào sửa checker), vì chúng chặn toàn bộ việc hiện thực.
2. Sau khi có chữ ký chốt: mở **một** task hiện thực API + sửa checker **cùng task** (để `v2 check` không đỏ giữa chừng), ngân sách đề xuất **3 giờ**.
3. **Chưa** đo lại K6 — chỉ phải làm khi API core đổi đầu vào tạo ảnh (D2-191 §4).
4. **Chưa** mở K4' bước 3, K5'; **chưa** phê duyệt phát hành v2.0.
