// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-24 — **IPC core 1**: `MessageInfo` 35 bit, `Message` 64 MR, vùng nhận trong TCB
//! (4 MR "thanh ghi" + 60 MR "IPC buffer"), và **một** hàm [`transfer`] copy message.
//!
//! **Model-only `[TK]`, không phải spec/ABI** — nhưng layout/number **đọc đúng**
//! `spec/abi.toml:31-49` (nguồn sự thật thứ 2, AGENTS.md §1) và thiết kế §4.6
//! (`docs/v2/ViFastOs-v2-thiet-ke.md:316-324`), D2-53 Q7/Q11, K1C-PLAN §5 KT-24.
//!
//! # Hợp đồng số
//!
//! | Hạng mục | Giá trị | Nguồn |
//! |---|---|---|
//! | `MessageInfo` | **35 bit**: `label:0..19`, `caps:20..22`, `no_reply:23`, `extra_caps_unwrapped:24..27`, `length:28..34` | `spec/abi.toml:40-49` |
//! | MR | **64**: `words_in_registers = 4` (MR0..MR3), `words_in_ipc_buffer = 60` | `spec/abi.toml:31-38` |
//! | cap | `max_caps = 4` (chuyển cap thuộc KT-25) | `spec/abi.toml:34` |
//!
//! # Ngữ nghĩa (tóm tắt; chi tiết ở `v2/docs/k1c-ipc-semantics.md`)
//!
//! - [`MessageInfo::new`] **kiểm biên** ngay khi tạo: `label < 2^20`, `caps <= 4`,
//!   `extra_caps_unwrapped <= 0xF`, `length <= 64`; vi phạm ⇒ `InvalidArgument`
//!   (không cắt im lặng). [`MessageInfo::unpack`] cũng kiểm lại các biên này nên
//!   `unpack(pack(x)) == x` với mọi `x` hợp lệ và bit ngoài 35 bit là lỗi.
//! - [`Message`] mang **byte-for-byte** `mr: [u64; 64]` + `info` + `caps` + `badge` + `msg_id`.
//!   `msg_id` do **kernel** cấp khi message được nhận vào đường IPC (0 = chưa cấp) — dùng cho
//!   bất biến bảo toàn message (`Machine::check_ipc`).
//! - [`IpcArea`] là vùng nhận **của một TCB**: `regs[0..4]` (MR0..MR3, "thanh ghi") và
//!   `buffer[0..60]` (MR4..MR63, "IPC buffer") là **hai field riêng**, không gộp — đúng
//!   `spec/abi.toml` (`words_in_registers` + `words_in_ipc_buffer`) và KT-24 DoD 2.
//! - [`transfer`] là **đường duy nhất** copy nội dung message vào vùng nhận: `send`/`call`/
//!   `recv`/`reply` không copy MR tại chỗ (đếm `Machine::transfer_calls`). Hàm **xoá sạch**
//!   vùng nhận trước khi chép nên message ngắn không lộ dữ liệu message trước.
//! - Cap: KT-24 **chưa** chuyển cap (thiết kế `:319`; KT-25). [`Message::new`] chỉ nhận
//!   message `caps = 0`; message có cap phải dựng qua [`Message::with_caps`] hoặc
//!   [`Message::with_cap_ids`].
//! - **KT-25** — [`plan_cap_transfer`] là **quyết định mô hình** chuyển ≤ 4 cap vào receive
//!   window của thread nhận: cap trỏ **chính Endpoint đang gọi** + có badge ⇒ **unwrap**
//!   (OR badge vào message, không chiếm window); thiếu `RIGHT_GRANT` / ô đích đã có cap ⇒
//!   **thất bại mềm** (D2-53 Q6: message **vẫn** giao, `MessageInfo.caps` = số cap cài được,
//!   `Machine::cap_transfer_failed` tăng, **không** thêm bit MessageInfo). Hàm này **không**
//!   đổi trạng thái — `Machine` cài cap theo kế hoạch qua `derive` (bỏ `RIGHT_GRANT`).

use crate::kobj::{CapId, KError, ObjId, ObjType, SlotRef};

/// Số MR tối đa của một message — `spec/abi.toml` `[message].max_words = 64`.
pub const MAX_WORDS: usize = 64;
/// Số cap tối đa của một message — `spec/abi.toml` `[message].max_caps = 4`.
pub const MAX_CAPS: usize = 4;
/// MR0..MR3 đi trong **thanh ghi** (`spec/abi.toml` `words_in_registers = 4`).
pub const WORDS_IN_REGISTERS: usize = 4;
/// MR4..MR63 đi trong **IPC buffer model** (`spec/abi.toml` `words_in_ipc_buffer = 60`).
pub const WORDS_IN_IPC_BUFFER: usize = 60;

/// Số bit của `MessageInfo` — `spec/abi.toml` `[message_info].bits = 35`.
pub const INFO_BITS: u32 = 35;
/// Mặt nạ 35 bit hợp lệ của `MessageInfo`.
pub const INFO_MASK: u64 = (1u64 << INFO_BITS) - 1;
/// Số bit của từng trường trong `MessageInfo` (theo thứ tự bit thấp trước).
pub const LABEL_BITS: u32 = 20;
pub const CAPS_BITS: u32 = 3;
pub const NO_REPLY_BITS: u32 = 1;
pub const EXTRA_CAPS_UNWRAPPED_BITS: u32 = 4;
pub const LENGTH_BITS: u32 = 7;

/// `MessageInfo` — **một thanh ghi 35 bit** (`spec/abi.toml:40-49`, thiết kế §4.6 `:316`).
///
/// Thứ tự bit trên thanh ghi (bit thấp trước):
/// `label:0..19 | caps:20..22 | no_reply:23 | extra_caps_unwrapped:24..27 | length:28..34`.
///
/// Các trường **private** để không thể tạo giá trị ngoài biên mà bỏ qua kiểm tra; đọc bằng
/// getter, tạo bằng [`MessageInfo::new`]/[`MessageInfo::unpack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageInfo {
    label: u32,
    caps: u8,
    no_reply: bool,
    extra_caps_unwrapped: u8,
    length: u8,
}

impl MessageInfo {
    /// `MessageInfo { label: 0, caps: 0, no_reply: false, extra: 0, length: 0 }`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Tạo `MessageInfo` đã **kiểm biên** (biên là hằng số ABI, không phải ngưỡng mô hình):
    ///
    /// - `label < 2^20` (20 bit);
    /// - `caps <= 4` (`spec/abi.toml` `max_caps = 4`, dù trường rộng 3 bit);
    /// - `extra_caps_unwrapped <= 0xF` (4 bit);
    /// - `length <= 64` (`spec/abi.toml` `max_words = 64`, dù trường rộng 7 bit).
    ///
    /// Vi phạm ⇒ `InvalidArgument`.
    pub fn new(
        label: u32,
        caps: u8,
        no_reply: bool,
        extra_caps_unwrapped: u8,
        length: u8,
    ) -> Result<Self, KError> {
        if label >= (1 << LABEL_BITS) {
            return Err(KError::InvalidArgument);
        }
        if caps as usize > MAX_CAPS {
            return Err(KError::InvalidArgument);
        }
        if extra_caps_unwrapped >= (1 << EXTRA_CAPS_UNWRAPPED_BITS) {
            return Err(KError::InvalidArgument);
        }
        if length as usize > MAX_WORDS {
            return Err(KError::InvalidArgument);
        }
        Ok(Self {
            label,
            caps,
            no_reply,
            extra_caps_unwrapped,
            length,
        })
    }

    /// `label` (20 bit) — `spec/abi.toml` `label_bits = 20`.
    pub fn label(self) -> u32 {
        self.label
    }

    /// Số cap trong message (0..=4).
    pub fn caps(self) -> u8 {
        self.caps
    }

    /// Cờ `no_reply` — caller đặt ⇒ `Call = Send` (D2-53 Q7); kernel cũng bật khi bên nhận
    /// `Recv` không kèm Reply (thiết kế `:322`).
    pub fn no_reply(self) -> bool {
        self.no_reply
    }

    /// Số **extra** cap được unwrap thành badge (KT-25).
    pub fn extra_caps_unwrapped(self) -> u8 {
        self.extra_caps_unwrapped
    }

    /// Số MR thực của message (0..=64) — `spec/abi.toml` `length_bits = 7`.
    pub fn length(self) -> u8 {
        self.length
    }

    /// Bản sao có `no_reply` = `on` (kernel bật cờ khi thiếu Reply — D2-53 Q7 nhánh (b)).
    pub fn with_no_reply(mut self, on: bool) -> Self {
        self.no_reply = on;
        self
    }

    /// Bản sao có `extra_caps_unwrapped` mới (kiểm biên 4 bit).
    pub fn with_extra_caps_unwrapped(mut self, extra: u8) -> Result<Self, KError> {
        if extra >= (1 << EXTRA_CAPS_UNWRAPPED_BITS) {
            return Err(KError::InvalidArgument);
        }
        self.extra_caps_unwrapped = extra;
        Ok(self)
    }

    /// Bản sao có `caps` mới — **biên ABI** `max_caps = 4`, không nới (D2-53 Q6).
    pub fn with_caps(mut self, caps: u8) -> Result<Self, KError> {
        if caps as usize > MAX_CAPS {
            return Err(KError::InvalidArgument);
        }
        self.caps = caps;
        Ok(self)
    }

    /// Bản sao có `length` mới (kiểm biên như [`MessageInfo::new`]).
    pub fn with_length(mut self, length: u8) -> Result<Self, KError> {
        if length as usize > MAX_WORDS {
            return Err(KError::InvalidArgument);
        }
        self.length = length;
        Ok(self)
    }

    /// Đóng gói thành thanh ghi 35 bit (`label:0..19, caps:20..22, no_reply:23,
    /// extra_caps_unwrapped:24..27, length:28..34`).
    pub fn pack(self) -> u64 {
        (self.label as u64)
            | ((self.caps as u64) << 20)
            | ((self.no_reply as u64) << 23)
            | ((self.extra_caps_unwrapped as u64) << 24)
            | ((self.length as u64) << 28)
    }

    /// Bí danh của [`MessageInfo::pack`] — "35 bit" của message info.
    pub fn bits(self) -> u64 {
        self.pack()
    }

    /// Giải mã thanh ghi 35 bit; **kiểm lại** mọi biên (`caps <= 4`, `length <= 64`,
    /// `extra <= 0xF`, `label < 2^20`) — bit ngoài 35 hoặc trường ngoài biên ⇒ `InvalidArgument`
    /// (không cắt im lặng).
    pub fn unpack(raw: u64) -> Result<Self, KError> {
        if raw & !INFO_MASK != 0 {
            return Err(KError::InvalidArgument);
        }
        Self::new(
            (raw & ((1 << LABEL_BITS) - 1)) as u32,
            ((raw >> 20) & ((1 << CAPS_BITS) - 1)) as u8,
            ((raw >> 23) & 1) == 1,
            ((raw >> 24) & ((1 << EXTRA_CAPS_UNWRAPPED_BITS) - 1)) as u8,
            ((raw >> 28) & ((1 << LENGTH_BITS) - 1)) as u8,
        )
    }
}

/// Một message IPC — `spec/abi.toml:31-38` + thiết kế §4.6 `:316-319`.
///
/// `mr` giữ **đủ 64 word** (byte-for-byte, không cắt); `info.length` cho biết bao nhiêu word
/// là thật. `msg_id` do kernel cấp khi nhận vào đường IPC (`0` = chưa cấp) — định danh dùng
/// cho bất biến bảo toàn message. `caps`/`badge` là chỗ của KT-25; KT-24 chỉ nhận `caps = 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub info: MessageInfo,
    pub mr: [u64; MAX_WORDS],
    pub caps: Vec<CapId>,
    pub badge: u64,
    pub msg_id: u64,
}

impl Message {
    /// Message **không** cap (KT-24): `caps` của `info` phải bằng 0, `words.len()` phải bằng
    /// `info.length()`. Vi phạm ⇒ `InvalidArgument` (không tự sửa `info`).
    pub fn new(info: MessageInfo, words: &[u64]) -> Result<Self, KError> {
        if info.caps() != 0 {
            return Err(KError::InvalidArgument);
        }
        Self::build(info, words, Vec::new(), 0)
    }

    /// Message **có** cap/badge — `caps.len()` phải bằng `info.caps()` (≤ 4) và mỗi phần tử
    /// là `CapId` **nguồn** (kernel chụp ảnh ở `admit`, cài vào receive window khi giao).
    pub fn with_caps(
        info: MessageInfo,
        words: &[u64],
        caps: Vec<CapId>,
        badge: u64,
    ) -> Result<Self, KError> {
        if caps.len() > MAX_CAPS || caps.len() != info.caps() as usize {
            return Err(KError::InvalidArgument);
        }
        Self::build(info, words, caps, badge)
    }

    /// Message từ danh sách `CapId` + độ dài tự suy — dạng dùng cho test/runner `.scn`
    /// (`caps=c0,c1,...`). **5 cap ⇒ `InvalidArgument`** ngay khi dựng message
    /// (`spec/abi.toml` `max_caps = 4`), trước khi vào đường IPC (`.scn` 15, KT-25 DoD 3).
    pub fn with_cap_ids(label: u32, words: &[u64], caps: &[CapId]) -> Result<Self, KError> {
        let info = MessageInfo::new(label, caps.len() as u8, false, 0, words.len() as u8)?;
        Self::with_caps(info, words, caps.to_vec(), 0)
    }

    fn build(
        info: MessageInfo,
        words: &[u64],
        caps: Vec<CapId>,
        badge: u64,
    ) -> Result<Self, KError> {
        if words.len() != info.length() as usize || words.len() > MAX_WORDS {
            return Err(KError::InvalidArgument);
        }
        let mut mr = [0u64; MAX_WORDS];
        mr[..words.len()].copy_from_slice(words);
        Ok(Self {
            info,
            mr,
            caps,
            badge,
            msg_id: 0,
        })
    }

    /// Các MR **thật** của message (`mr[..length]`).
    pub fn words(&self) -> &[u64] {
        &self.mr[..self.info.length() as usize]
    }

    /// `CapId` **nguồn** của các cap mang theo (≤ 4). Sau khi kernel giao message, cap đã
    /// được cài vào receive window nên danh sách này **rỗng** — số cap đã cài nằm ở
    /// [`MessageInfo::caps`] của message nhận.
    pub fn caps(&self) -> &[CapId] {
        &self.caps
    }

    /// Số cap nguồn đi cùng message (0..=4).
    pub fn cap_count(&self) -> usize {
        self.caps.len()
    }

    /// `length` của message (0..=64).
    pub fn len(&self) -> usize {
        self.info.length() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Gán `msg_id` (kernel cấp) — dùng khi đưa message vào đường IPC.
    pub fn with_id(mut self, msg_id: u64) -> Self {
        self.msg_id = msg_id;
        self
    }
}

/// Một cap **nguồn** của message, chụp ảnh ở thời điểm message vào đường IPC (KT-25).
///
/// Kernel **không** giữ `CapId` của bên gửi trong đường IPC (bên gửi có thể xoá cap ngay sau
/// khi gửi); ảnh chụp đủ để cài một cap con vào receive window của bên nhận.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapSnapshot {
    pub cap: CapId,
    pub obj: ObjId,
    pub ty: ObjType,
    pub rights: u8,
    pub badge: Option<u64>,
}

/// Kế hoạch chuyển cap cho **một** lần giao message (model-only `[TK]`, D2-53 Q6).
///
/// - `install`: `(chỉ số cap trong message, ô receive window đích)`;
/// - `unwrap_badges`: badge của các cap **unwrap** (OR vào `Message.badge`);
/// - `skipped`: số cap **thất bại mềm** (thiếu `RIGHT_GRANT` hoặc ô đích đã có cap) — message
///   **vẫn** giao, bộ đếm `cap_transfer_failed` tăng (không thêm bit MessageInfo).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapPlan {
    pub install: Vec<(usize, SlotRef)>,
    pub unwrap_badges: Vec<u64>,
    pub skipped: usize,
}

impl CapPlan {
    /// Số cap sẽ cài vào receive window.
    pub fn installed(&self) -> usize {
        self.install.len()
    }

    /// Số cap được unwrap thành badge.
    pub fn unwrapped(&self) -> usize {
        self.unwrap_badges.len()
    }

    /// OR các badge unwrap (0 nếu không có).
    pub fn badge(&self) -> u64 {
        self.unwrap_badges.iter().fold(0u64, |a, b| a | *b)
    }

    /// Không có cap nào (message không mang cap).
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.unwrap_badges.is_empty() && self.skipped == 0
    }
}

/// **Quyết định** chuyển ≤ 4 cap vào receive window của thread nhận — **không** đổi trạng thái
/// (thiết kế §4.6 `:319`, D2-53 Q6, K1C-PLAN §5 KT-25 DoD 3/4/5).
///
/// Quy tắc (theo **vị trí**: cap thứ `i` ứng với ô window thứ `i`):
/// 1. cap trỏ **chính Endpoint đang gọi** (`endpoint == Some(obj)`) **và** có badge ⇒
///    **unwrap**: badge OR vào message, `extra_caps_unwrapped` tăng, **không** chiếm window;
/// 2. thiếu `RIGHT_GRANT` ⇒ **thất bại mềm** (`skipped += 1`), message vẫn giao;
/// 3. ô window đích **đã có cap** ⇒ **thất bại mềm**;
/// 4. còn lại ⇒ cài vào `window[i]` (bên gọi `Machine` tạo cap con **bỏ** `RIGHT_GRANT`, giữ
///    badge).
///
/// `caps.len() > 4` ⇒ `InvalidArgument` (biên ABI, **không** nới). `window = None` (TCB nhận
/// **chưa bind**) ⇒ `InvalidArgument` khi có cap **cần cài**; message chỉ mang cap **unwrap**
/// vẫn giao được (không cần window).
pub fn plan_cap_transfer(
    caps: &[CapSnapshot],
    window: Option<&[SlotRef; MAX_CAPS]>,
    occupied: &[bool; MAX_CAPS],
    endpoint: Option<ObjId>,
) -> Result<CapPlan, KError> {
    if caps.len() > MAX_CAPS {
        return Err(KError::InvalidArgument);
    }
    let mut plan = CapPlan::default();
    for (i, c) in caps.iter().enumerate() {
        if let (Some(ep), Some(badge)) = (endpoint, c.badge) {
            if ep == c.obj {
                // Unwrap badge: không chuyển cap, badge OR vào message (thiết kế `:319`).
                plan.unwrap_badges.push(badge);
                continue;
            }
        }
        // Cap **cần cài** mà TCB nhận **chưa bind** receive window ⇒ lỗi cấu hình (khác "các
        // cap đều unwrap": không cần window nên vẫn giao được).
        let Some(window) = window else {
            return Err(KError::InvalidArgument);
        };
        if c.rights & crate::kobj::RIGHT_GRANT == 0 || occupied[i] {
            plan.skipped += 1;
            continue;
        }
        plan.install.push((i, window[i]));
    }
    Ok(plan)
}

/// Vùng nhận IPC của **một** TCB — hai field **riêng** đúng `spec/abi.toml`:
/// - `regs: [u64; 4]` — MR0..MR3 đi "thanh ghi" (`words_in_registers = 4`);
/// - `buffer: [u64; 60]` — MR4..MR63 đi "IPC buffer model" (`words_in_ipc_buffer = 60`);
/// - `info`/`badge`/`msg_id` — metadata của message **vừa nhận** (`None`/`0` = chưa nhận).
///
/// Không gộp hai vùng: IPC buffer là vùng nhớ riêng trong TCB, không phải phần đuôi của mảng
/// thanh ghi (KT-24 DoD 2 kiểm riêng hai vùng, `mr[4]` chỉ có trong buffer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcArea {
    pub info: Option<MessageInfo>,
    pub regs: [u64; WORDS_IN_REGISTERS],
    pub buffer: [u64; WORDS_IN_IPC_BUFFER],
    pub badge: u64,
    pub msg_id: Option<u64>,
}

impl IpcArea {
    pub fn new() -> Self {
        Self {
            info: None,
            regs: [0; WORDS_IN_REGISTERS],
            buffer: [0; WORDS_IN_IPC_BUFFER],
            badge: 0,
            msg_id: None,
        }
    }

    /// Xoá sạch vùng nhận (message info + hai vùng MR + badge + msg_id).
    pub fn clear(&mut self) {
        self.info = None;
        self.regs = [0; WORDS_IN_REGISTERS];
        self.buffer = [0; WORDS_IN_IPC_BUFFER];
        self.badge = 0;
        self.msg_id = None;
    }

    /// MR thứ `i` (0..64) — ghép hai vùng để tiện đối chiếu; `None` nếu `i >= 64`.
    pub fn mr(&self, i: usize) -> Option<u64> {
        if i < WORDS_IN_REGISTERS {
            self.regs.get(i).copied()
        } else {
            self.buffer.get(i - WORDS_IN_REGISTERS).copied()
        }
    }

    /// Các MR **đang có** của message vừa nhận (`mr(..length)`); rỗng nếu chưa nhận.
    pub fn words(&self) -> Vec<u64> {
        match self.info {
            Some(info) => (0..info.length() as usize)
                .map(|i| self.mr(i).expect("i < length <= 64"))
                .collect(),
            None => Vec::new(),
        }
    }
}

impl Default for IpcArea {
    fn default() -> Self {
        Self::new()
    }
}

/// **Đường duy nhất** copy nội dung một message vào vùng nhận của TCB — thiết kế §4.6 `:324`
/// ("một hàm lõi `ipc::transfer`; fastpath sau này phải gọi cùng hàm").
///
/// Chép `info`, MR (4 word đầu vào `regs`, 60 word sau vào `buffer`), `badge`, `msg_id`.
/// **Xoá sạch** vùng nhận trước khi chép để message ngắn không lộ dữ liệu message trước.
/// Trả số MR thật đã chép (`info.length()`).
///
/// Hàm này **không** cấp `msg_id` và **không** chuyển cap (KT-25): kernel cấp `msg_id` khi
/// nhận message vào đường IPC, cap thuộc đường chuyển cap của KT-25.
pub fn transfer(msg: &Message, area: &mut IpcArea) -> usize {
    area.clear();
    area.info = Some(msg.info);
    let len = msg.info.length() as usize;
    for (i, word) in msg.mr[..len].iter().enumerate() {
        if i < WORDS_IN_REGISTERS {
            area.regs[i] = *word;
        } else {
            area.buffer[i - WORDS_IN_REGISTERS] = *word;
        }
    }
    area.badge = msg.badge;
    area.msg_id = Some(msg.msg_id);
    len
}
