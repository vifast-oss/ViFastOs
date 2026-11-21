// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-60 — `MessageInfo` 35 bit + `Message` 64 MR + vùng nhận [`IpcArea`] + **một** hàm
//! [`transfer`] copy message — port `no_std` từ `v2/sim/vifast-sim/src/ipc.rs` (oracle).
//!
//! Model-only `[TK]`, **không** phải ABI; layout/number đọc đúng `spec/abi.toml`:
//! `MessageInfo` 35 bit (`label:0..19 | caps:20..22 | no_reply:23 | extra:24..27 |
//! length:28..34`), 64 MR (`4` thanh ghi + `60` IPC buffer), ≤ 4 cap.
//!
//! **KT-63:** `Message` mang **`CapId` nguồn** (≤ 4, `caps: [CapId; 4]`) như `Vec<CapId>`
//! của sim + **ảnh chụp** (`CapSnapshot`) chụp lúc message vào đường IPC; kernel port
//! [`plan_cap_transfer`]/[`CapPlan`] nguyên ngữ nghĩa sim (unwrap badge, thiếu `RIGHT_GRANT`
//! hoặc ô đích bận ⇒ thất bại **mềm**; window chưa bind ⇒ `InvalidArgument` **trước** khi
//! giao). Message **không** copy object — chỉ cap tham chiếu (con của cap nguồn).
//! `badge`/`msg_id` **có** (Notification badge OR + bất biến bảo toàn message).

use crate::error::{CapId, KError, ObjId, ObjType, SlotRef};

/// Số MR tối đa của một message — `spec/abi.toml` `[message].max_words = 64`.
pub const MAX_WORDS: usize = 64;
/// Số cap tối đa của một message — `spec/abi.toml` `[message].max_caps = 4`.
pub const MAX_CAPS: usize = 4;
/// MR0..MR3 đi trong **thanh ghi** (`spec/abi.toml` `words_in_registers = 4`).
pub const WORDS_IN_REGISTERS: usize = 4;
/// MR4..MR63 đi trong **IPC buffer model** (`spec/abi.toml` `words_in_ipc_buffer = 60`).
pub const WORDS_IN_IPC_BUFFER: usize = 60;

// ---------------------------------------------------------------------------------------------
// KT-282 — IPC buffer **thật** (frame 4 KiB bind qua `Tcb.Configure`) chở MR4..MR63
// ---------------------------------------------------------------------------------------------

/// Cỡ IPC buffer — `spec/abi.toml` `[message].ipc_buffer_bytes = 4096`.
pub const IPC_BUFFER_BYTES: usize = 4096;
/// Căn của IPC buffer — `spec/abi.toml` `[message].ipc_buffer_align = 16`.
pub const IPC_BUFFER_ALIGN: usize = 16;
/// Byte offset của **vùng MR4..MR63** trong IPC buffer: ngay sau header 10 word
/// (`IPC_HEADER_WORDS * 8 = 80`, đã căn 16 theo `ipc_buffer_align`).
///
/// `[TK]` **KT-282** — `spec/abi.toml` `[calling.*].ipc_buffer` chỉ chốt *"header chứa receive
/// window (CPTR, ≤4 cap)"*, **không** chốt offset vùng MR. Định nghĩa ở đây (một nguồn cho
/// core + arch + shim U-mode) và ghi rõ trong `JOURNAL/kt282-mrbuf.md` §2:
///
/// ```text
/// byte   0.. 80 : header 10 word (magic, info, receive window 4, send caps 4) — KT-67b
/// byte  80..560 : MR4..MR63 (60 word = 480 B) — KT-282
/// byte 560..568 : **extra_caps count n** (0..=4) — KT-309
/// byte 568..600 : **n CPTR nguồn** (4 × 8 B, CPTR trong CSpace **caller**) — KT-309
/// byte 600..608 : **extra_caps dst** (slot đích trong CSpace **receiver**; 0 = slot trống đầu) — KT-309
/// byte 608..4096: dự phòng (zero)
/// ```
pub const IPC_BUFFER_MR_OFFSET: usize = IPC_HEADER_WORDS * 8;
/// Số word của vùng MR4..MR63 — `[message].words_in_ipc_buffer = 60`.
pub const IPC_BUFFER_MR_WORDS: usize = WORDS_IN_IPC_BUFFER;
/// Byte cuối (không tính) của vùng MR4..MR63 — `80 + 480 = 560 ≤ 4096`.
pub const IPC_BUFFER_MR_END: usize = IPC_BUFFER_MR_OFFSET + IPC_BUFFER_MR_WORDS * 8;

// ---------------------------------------------------------------------------------------------
// KT-309 — vùng **extra caps** trong IPC buffer (cap transfer **qua buffer**, > 4 cap)
// ---------------------------------------------------------------------------------------------

/// Số **extra cap** tối đa của một message — `spec/abi.toml` `[message].max_caps = 4` (biên
/// **không nới**; `n > 4` ⇒ `InvalidArgument`).
pub const MAX_EXTRA_CAPS: usize = MAX_CAPS;

/// Byte offset của vùng **extra caps** trong IPC buffer — `[TK]` **KT-309**, ngay sau vùng
/// MR4..MR63 (`IPC_BUFFER_MR_END` = 560, đã căn 16 theo `[message].ipc_buffer_align`).
///
/// `spec/abi.toml` **không** chốt layout vùng dự phòng của IPC buffer (chỉ chốt *"header chứa
/// receive window (CPTR, ≤4 cap)"* + `ipc_buffer_bytes = 4096`), nên đây là **model `[TK]`**,
/// **không** phải ABI — cùng khuôn KT-282 đã dùng cho vùng MR4..MR63. **Không** đổi `spec/`.
///
/// ```text
/// byte 560..568 : n (số extra cap, 0..=4)          — caller ghi trước Call/Send
/// byte 568..600 : n CPTR nguồn (CSpace caller)     — caller ghi
/// byte 600..608 : dst slot (CSpace receiver, 0=auto) — receiver ghi trước Recv
/// ```
pub const IPC_BUFFER_EXTRA_CAPS_OFFSET: usize = IPC_BUFFER_MR_END;
/// Số word của vùng **CPTR nguồn** (không tính word `n` và word `dst`).
pub const IPC_BUFFER_EXTRA_CAPS_WORDS: usize = MAX_EXTRA_CAPS;
/// Byte offset của word **`dst`** (slot đích trong CSpace receiver) — `560 + 5*8 = 600`.
pub const IPC_BUFFER_EXTRA_CAPS_DST_OFFSET: usize =
    IPC_BUFFER_EXTRA_CAPS_OFFSET + (1 + IPC_BUFFER_EXTRA_CAPS_WORDS) * 8;
/// Tổng số word arch phải đọc cho vùng extra caps: `n` + 4 CPTR + `dst` = **6**.
pub const IPC_BUFFER_EXTRA_CAPS_READ_WORDS: usize = 1 + IPC_BUFFER_EXTRA_CAPS_WORDS + 1;
/// Byte cuối (không tính) của vùng extra caps — `560 + 48 = 608 ≤ 4096`.
pub const IPC_BUFFER_EXTRA_CAPS_END: usize = IPC_BUFFER_EXTRA_CAPS_DST_OFFSET + 8;

/// **KT-309** — vùng **extra caps** đọc từ IPC buffer của TCB đang chạy (`[TK]`, không phải ABI).
///
/// Arch đọc **bộ nhớ user** (D2-83: core không đọc/ghi bộ nhớ user) rồi trả qua
/// [`crate::syscall::SyscallEnv::ipc_buffer_extra_caps`]; `None` = trang chưa map / nền tảng
/// không có IPC buffer ⇒ **không** có extra cap (fail-closed, không đoán).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IpcExtraCaps {
    /// Số extra cap caller khai (`0..=4`; `> 4` ⇒ `InvalidArgument` khi dựng message).
    pub count: u64,
    /// CPTR nguồn trong CSpace **caller** — `count` phần tử đầu có nghĩa.
    pub cptrs: [u64; MAX_EXTRA_CAPS],
    /// Slot đích trong CSpace **receiver** (`0` = "slot trống đầu tiên").
    pub dst: u64,
}

impl IpcExtraCaps {
    /// Vùng rỗng (không extra cap, `dst` tự động).
    pub const EMPTY: Self = Self {
        count: 0,
        cptrs: [0; MAX_EXTRA_CAPS],
        dst: 0,
    };

    /// Dựng từ 6 word arch đọc được (`n`, 4 CPTR, `dst`).
    pub fn from_words(words: &[u64; IPC_BUFFER_EXTRA_CAPS_READ_WORDS]) -> Self {
        let mut cptrs = [0u64; MAX_EXTRA_CAPS];
        cptrs.copy_from_slice(&words[1..1 + MAX_EXTRA_CAPS]);
        Self {
            count: words[0],
            cptrs,
            dst: words[1 + MAX_EXTRA_CAPS],
        }
    }

    /// Không có extra cap nào.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Byte offset của MR thứ `index` trong IPC buffer: `Some(80 + (index-4)*8)` cho `4..=63`,
/// `None` cho `index < 4` (MR thanh ghi) hoặc `index >= 64` (ngoài `max_words`).
pub const fn mr_buffer_offset(index: usize) -> Option<usize> {
    if index < WORDS_IN_REGISTERS || index >= MAX_WORDS {
        return None;
    }
    Some(IPC_BUFFER_MR_OFFSET + (index - WORDS_IN_REGISTERS) * 8)
}

/// **KT-282** — liên kết **IPC buffer** của một TCB: tham số `ipc_buffer?` của `Tcb.Configure`
/// (`spec/abi.toml` `[method_signature."Tcb.Configure"]`).
///
/// Model-only `[TK]`: core **không** sở hữu Frame/PTE (D2-83) — nó giữ **cap Frame** (danh
/// tính, để `Tcb.Configure` kiểm đúng loại + `check_caps` thấy cap sống) và **VA** của buffer;
/// arch đọc/ghi bộ nhớ user tại `va` (trang 4 KiB đã map, `R|W|U|NX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcBufferBinding {
    /// Object `Frame` 4 KiB đã bind (đại diện trong core, `ObjType::Frame`).
    pub frame: ObjId,
    /// VA của buffer trong VSpace của TCB — phải căn `IPC_BUFFER_BYTES`.
    pub va: u64,
}

/// Số bit của `MessageInfo` — `spec/abi.toml` `[message_info].bits = 35`.
pub const INFO_BITS: u32 = 35;
/// Mặt nạ 35 bit hợp lệ của `MessageInfo`.
pub const INFO_MASK: u64 = (1u64 << INFO_BITS) - 1;
/// Số bit `label`.
pub const LABEL_BITS: u32 = 20;
/// Số bit `caps`.
pub const CAPS_BITS: u32 = 3;
/// Số bit cờ `no_reply`.
pub const NO_REPLY_BITS: u32 = 1;
/// Số bit `extra_caps_unwrapped`.
pub const EXTRA_CAPS_UNWRAPPED_BITS: u32 = 4;
/// Số bit `length`.
pub const LENGTH_BITS: u32 = 7;

/// `MessageInfo` — **một thanh ghi 35 bit** (`spec/abi.toml`, thiết kế §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageInfo {
    label: u32,
    caps: u8,
    no_reply: bool,
    extra_caps_unwrapped: u8,
    length: u8,
}

impl MessageInfo {
    /// `MessageInfo` rỗng.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Tạo `MessageInfo` **đã kiểm biên** (biên ABI): `label < 2^20`, `caps <= 4`,
    /// `extra_caps_unwrapped < 16`, `length <= 64`. Vi phạm ⇒ `InvalidArgument`.
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

    /// `label` (20 bit).
    pub fn label(self) -> u32 {
        self.label
    }

    /// Số cap trong message (0..=4).
    pub fn caps(self) -> u8 {
        self.caps
    }

    /// Cờ `no_reply` (Call = Send khi caller đặt; kernel bật khi thiếu Reply).
    pub fn no_reply(self) -> bool {
        self.no_reply
    }

    /// Số extra cap đã unwrap thành badge.
    pub fn extra_caps_unwrapped(self) -> u8 {
        self.extra_caps_unwrapped
    }

    /// Số MR thật (0..=64).
    pub fn length(self) -> u8 {
        self.length
    }

    /// Bản sao có `no_reply = on`.
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

    /// Bản sao có `caps` mới (biên ABI `max_caps = 4`).
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

    /// Đóng gói thành thanh ghi 35 bit.
    pub fn pack(self) -> u64 {
        (self.label as u64)
            | ((self.caps as u64) << 20)
            | ((self.no_reply as u64) << 23)
            | ((self.extra_caps_unwrapped as u64) << 24)
            | ((self.length as u64) << 28)
    }

    /// Bí danh của [`MessageInfo::pack`].
    pub fn bits(self) -> u64 {
        self.pack()
    }

    /// Giải mã thanh ghi 35 bit; **kiểm lại** mọi biên. Bit ngoài 35 hoặc trường ngoài biên
    /// ⇒ `InvalidArgument` (không cắt im lặng).
    pub fn unpack(raw: u64) -> Result<Self, KError> {
        if raw & !INFO_MASK != 0 {
            return Err(KError::InvalidArgument);
        }
        Self::new(
            (raw & 0xF_FFFF) as u32,
            ((raw >> 20) & 0x7) as u8,
            (raw >> 23) & 1 != 0,
            ((raw >> 24) & 0xF) as u8,
            ((raw >> 28) & 0x7F) as u8,
        )
    }
}

/// Một cap **nguồn** của message, chụp ảnh ở thời điểm message vào đường IPC (KT-63, port
/// `ipc::CapSnapshot` của sim): bên gửi có thể xoá cap ngay sau khi gửi, ảnh chụp đủ để cài
/// một cap con vào receive window của bên nhận.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapSnapshot {
    /// `CapId` nguồn (kiểm còn sống lúc giao).
    pub cap: CapId,
    /// Object cap trỏ tới.
    pub obj: ObjId,
    /// Loại object.
    pub ty: ObjType,
    /// Quyền của cap nguồn (quyền **đã lọc** khi cài = `rights & !RIGHT_GRANT`).
    pub rights: u8,
    /// Badge nguồn (giữ nguyên khi cài; OR vào message khi unwrap).
    pub badge: Option<u64>,
}

impl CapSnapshot {
    /// Ảnh chụp rỗng (mốc const).
    pub const EMPTY: Self = Self {
        cap: 0,
        obj: 0,
        ty: ObjType::Untyped,
        rights: 0,
        badge: None,
    };
}

/// Kế hoạch chuyển cap cho **một** lần giao message (port `ipc::CapPlan` của sim, D2-53 Q6).
///
/// - `install`: `(chỉ số cap trong message, ô receive window đích)`;
/// - `unwrap`: badge của các cap **unwrap** (OR vào `Message.badge`);
/// - `skipped`: số cap **thất bại mềm** (thiếu `RIGHT_GRANT` hoặc ô đích đã có cap) — message
///   **vẫn** giao, bộ đếm `cap_transfer_failed` tăng (không thêm bit `MessageInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapPlan {
    install: [(u8, SlotRef); MAX_CAPS],
    n_install: u8,
    unwrap: [u64; MAX_CAPS],
    n_unwrap: u8,
    skipped: u8,
}

impl CapPlan {
    /// Kế hoạch rỗng.
    pub const EMPTY: Self = Self {
        install: [(0, SlotRef { cnode: 0, slot: 0 }); MAX_CAPS],
        n_install: 0,
        unwrap: [0; MAX_CAPS],
        n_unwrap: 0,
        skipped: 0,
    };

    /// Số cap sẽ cài vào receive window.
    pub fn installed(&self) -> usize {
        self.n_install as usize
    }

    /// Số cap được unwrap thành badge.
    pub fn unwrapped(&self) -> usize {
        self.n_unwrap as usize
    }

    /// OR các badge unwrap (0 nếu không có).
    pub fn badge(&self) -> u64 {
        self.unwrap[..self.n_unwrap as usize]
            .iter()
            .fold(0u64, |a, b| a | *b)
    }

    /// Số cap thất bại mềm.
    pub fn skipped(&self) -> usize {
        self.skipped as usize
    }

    /// Các cặp `(chỉ số cap, ô đích)` sẽ cài.
    pub fn install(&self) -> &[(u8, SlotRef)] {
        &self.install[..self.n_install as usize]
    }

    /// Không có cap nào (message không mang cap).
    pub fn is_empty(&self) -> bool {
        self.n_install == 0 && self.n_unwrap == 0 && self.skipped == 0
    }
}

impl Default for CapPlan {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// **Quyết định** chuyển ≤ 4 cap vào receive window của thread nhận — **không** đổi trạng thái
/// (port **nguyên văn** `ipc::plan_cap_transfer` của sim; thiết kế §4.6 `:319`, D2-53 Q6).
///
/// Quy tắc (theo **vị trí**: cap thứ `i` ứng với ô window thứ `i`):
/// 1. cap trỏ **chính Endpoint đang gọi** (`endpoint == Some(obj)`) **và** có badge ⇒
///    **unwrap**: badge OR vào message, `extra_caps_unwrapped` tăng, **không** chiếm window;
/// 2. thiếu `RIGHT_GRANT` ⇒ **thất bại mềm** (`skipped += 1`), message vẫn giao;
/// 3. ô window đích **đã có cap** ⇒ **thất bại mềm**;
/// 4. còn lại ⇒ cài vào `window[i]` (bên gọi tạo cap con **bỏ** `RIGHT_GRANT`, giữ badge).
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
    let mut plan = CapPlan::EMPTY;
    for (i, c) in caps.iter().enumerate() {
        if let (Some(ep), Some(badge)) = (endpoint, c.badge) {
            if ep == c.obj {
                plan.unwrap[plan.n_unwrap as usize] = badge;
                plan.n_unwrap += 1;
                continue;
            }
        }
        let Some(window) = window else {
            return Err(KError::InvalidArgument);
        };
        if c.rights & crate::spec::RIGHT_GRANT == 0 || occupied[i] {
            plan.skipped += 1;
            continue;
        }
        plan.install[plan.n_install as usize] = (i as u8, window[i]);
        plan.n_install += 1;
    }
    Ok(plan)
}

/// Một message IPC: `info` 35 bit + **đủ 64 MR** + `badge` + `msg_id` (kernel cấp) + cap nguồn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message {
    /// `MessageInfo` 35 bit.
    pub info: MessageInfo,
    /// 64 MR — chỉ `mr[..length]` là thật.
    pub mr: [u64; MAX_WORDS],
    /// Badge (Notification unwrap/OR — KT-27).
    pub badge: u64,
    /// `msg_id` do kernel cấp khi message vào đường IPC (`0` = chưa cấp).
    pub msg_id: u64,
    /// `CapId` **nguồn** của các cap mang theo (≤ 4) — `n_caps` phần tử đầu có nghĩa.
    pub caps: [CapId; MAX_CAPS],
    /// Số cap nguồn (0..=4); phải bằng `info.caps()` khi message vào đường IPC.
    pub n_caps: u8,
    /// Ảnh chụp cap nguồn (kernel điền ở `admit`, xoá sau khi giao).
    pub snaps: [CapSnapshot; MAX_CAPS],
    /// Số ảnh chụp có nghĩa.
    pub n_snaps: u8,
    /// **KT-309** — ảnh chụp **extra cap** nguồn (đi qua **IPC buffer**, ≤ 4). Chụp **ngay** ở
    /// `build_message` (caller đang chạy) vì extra cap **không** đi qua `resolve_caps`; xoá sau
    /// khi cài vào CSpace receiver.
    pub extra_snaps: [CapSnapshot; MAX_EXTRA_CAPS],
    /// Số ảnh chụp extra cap có nghĩa (0..=4).
    pub n_extra: u8,
}

impl Message {
    /// `Message` rỗng (mốc const).
    pub const EMPTY: Self = Self {
        info: MessageInfo {
            label: 0,
            caps: 0,
            no_reply: false,
            extra_caps_unwrapped: 0,
            length: 0,
        },
        mr: [0; MAX_WORDS],
        badge: 0,
        msg_id: 0,
        caps: [0; MAX_CAPS],
        n_caps: 0,
        snaps: [CapSnapshot::EMPTY; MAX_CAPS],
        n_snaps: 0,
        extra_snaps: [CapSnapshot::EMPTY; MAX_EXTRA_CAPS],
        n_extra: 0,
    };

    /// Message **không cap**: `info.caps()` phải bằng 0, `words.len()` phải bằng
    /// `info.length()`. Vi phạm ⇒ `InvalidArgument`.
    pub fn new(info: MessageInfo, words: &[u64]) -> Result<Self, KError> {
        if info.caps() != 0 {
            return Err(KError::InvalidArgument);
        }
        Self::build(info, words, &[], 0)
    }

    /// Message **có** cap/badge — `caps.len()` phải bằng `info.caps()` (≤ 4) và mỗi phần tử
    /// là `CapId` **nguồn** (kernel chụp ảnh ở `admit`, cài vào receive window khi giao).
    pub fn with_caps(
        info: MessageInfo,
        words: &[u64],
        caps: &[CapId],
        badge: u64,
    ) -> Result<Self, KError> {
        if caps.len() > MAX_CAPS || caps.len() != info.caps() as usize {
            return Err(KError::InvalidArgument);
        }
        Self::build(info, words, caps, badge)
    }

    /// Message từ danh sách `CapId` + độ dài tự suy — dạng dùng cho test/runner (như sim
    /// `Message::with_cap_ids`). **5 cap ⇒ `InvalidArgument`** ngay khi dựng message
    /// (`spec/abi.toml` `max_caps = 4`), trước khi vào đường IPC.
    pub fn with_cap_ids(label: u32, words: &[u64], caps: &[CapId]) -> Result<Self, KError> {
        let info = MessageInfo::new(label, caps.len() as u8, false, 0, words.len() as u8)?;
        Self::with_caps(info, words, caps, 0)
    }

    fn build(info: MessageInfo, words: &[u64], caps: &[CapId], badge: u64) -> Result<Self, KError> {
        if words.len() != info.length() as usize || words.len() > MAX_WORDS {
            return Err(KError::InvalidArgument);
        }
        let mut mr = [0u64; MAX_WORDS];
        mr[..words.len()].copy_from_slice(words);
        let mut ids = [0u32; MAX_CAPS];
        ids[..caps.len()].copy_from_slice(caps);
        Ok(Self {
            info,
            mr,
            badge,
            msg_id: 0,
            caps: ids,
            n_caps: caps.len() as u8,
            snaps: [CapSnapshot::EMPTY; MAX_CAPS],
            n_snaps: 0,
            extra_snaps: [CapSnapshot::EMPTY; MAX_EXTRA_CAPS],
            n_extra: 0,
        })
    }

    /// Các `CapId` nguồn của message (`caps[..n_caps]`). Sau khi kernel giao message, danh
    /// sách này **rỗng** (cap đã cài vào receive window; số cap nằm ở `MessageInfo.caps`).
    pub fn cap_ids(&self) -> &[CapId] {
        &self.caps[..self.n_caps as usize]
    }

    /// Số cap nguồn đi cùng message (0..=4).
    pub fn cap_count(&self) -> usize {
        self.n_caps as usize
    }

    /// Ảnh chụp cap nguồn (`snaps[..n_snaps]`) — kernel điền khi `admit`.
    pub fn snapshots(&self) -> &[CapSnapshot] {
        &self.snaps[..self.n_snaps as usize]
    }

    /// **KT-309** — ảnh chụp **extra cap** nguồn (`extra_snaps[..n_extra]`), chụp ở
    /// `build_message` (caller đang chạy) vì extra cap **không** đi qua `resolve_caps`.
    pub fn extra_snapshots(&self) -> &[CapSnapshot] {
        &self.extra_snaps[..self.n_extra as usize]
    }

    /// **KT-309** — gắn **extra cap** đã chụp ảnh vào message. `snaps.len() > 4` ⇒
    /// `InvalidArgument` (biên `[message].max_caps = 4`, **không** nới).
    pub fn with_extra_caps(mut self, snaps: &[CapSnapshot]) -> Result<Self, KError> {
        if snaps.len() > MAX_EXTRA_CAPS {
            return Err(KError::InvalidArgument);
        }
        self.extra_snaps = [CapSnapshot::EMPTY; MAX_EXTRA_CAPS];
        self.extra_snaps[..snaps.len()].copy_from_slice(snaps);
        self.n_extra = snaps.len() as u8;
        Ok(self)
    }

    /// **KT-309** — xoá ảnh chụp extra cap sau khi đã cài vào CSpace receiver.
    pub fn clear_extra_caps(&mut self) {
        self.extra_snaps = [CapSnapshot::EMPTY; MAX_EXTRA_CAPS];
        self.n_extra = 0;
    }

    /// Xoá danh sách cap nguồn + ảnh chụp sau khi đã cài vào receive window.
    pub fn clear_caps(&mut self) {
        self.caps = [0; MAX_CAPS];
        self.n_caps = 0;
        self.snaps = [CapSnapshot::EMPTY; MAX_CAPS];
        self.n_snaps = 0;
    }

    /// Các MR **thật** của message (`mr[..length]`).
    pub fn words(&self) -> &[u64] {
        &self.mr[..self.info.length() as usize]
    }

    /// `length` của message (0..=64).
    pub fn len(&self) -> usize {
        self.info.length() as usize
    }

    /// Message rỗng?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Gán `msg_id` do kernel cấp.
    pub fn with_id(mut self, msg_id: u64) -> Self {
        self.msg_id = msg_id;
        self
    }
}

/// Vùng nhận IPC của **một** TCB — hai field **riêng** đúng `spec/abi.toml`:
/// `regs: [u64; 4]` (MR0..MR3) + `buffer: [u64; 60]` (MR4..MR63), cùng metadata message vừa
/// nhận (`info`/`badge`/`msg_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcArea {
    /// `MessageInfo` của message **vừa nhận** (`None` = chưa nhận).
    pub info: Option<MessageInfo>,
    /// MR0..MR3.
    pub regs: [u64; WORDS_IN_REGISTERS],
    /// MR4..MR63.
    pub buffer: [u64; WORDS_IN_IPC_BUFFER],
    /// Badge của message vừa nhận.
    pub badge: u64,
    /// `msg_id` của message vừa nhận.
    pub msg_id: Option<u64>,
}

impl IpcArea {
    /// Vùng nhận rỗng.
    pub const fn new() -> Self {
        Self {
            info: None,
            regs: [0; WORDS_IN_REGISTERS],
            buffer: [0; WORDS_IN_IPC_BUFFER],
            badge: 0,
            msg_id: None,
        }
    }

    /// Xoá sạch vùng nhận.
    pub fn clear(&mut self) {
        self.info = None;
        self.regs = [0; WORDS_IN_REGISTERS];
        self.buffer = [0; WORDS_IN_IPC_BUFFER];
        self.badge = 0;
        self.msg_id = None;
    }

    /// MR thứ `i` (0..64) — ghép hai vùng.
    pub fn mr(&self, i: usize) -> Option<u64> {
        if i < WORDS_IN_REGISTERS {
            self.regs.get(i).copied()
        } else {
            self.buffer.get(i - WORDS_IN_REGISTERS).copied()
        }
    }
}

impl Default for IpcArea {
    fn default() -> Self {
        Self::new()
    }
}

/// **Đường duy nhất** copy nội dung message vào vùng nhận của TCB (thiết kế §4.6): xoá sạch
/// vùng nhận trước, chép `info` + MR (4 word đầu vào `regs`, 60 sau vào `buffer`) + `badge` +
/// `msg_id`. Trả số MR thật đã chép.
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

// ---------------------------------------------------------------------------------------------
// KT-67b — header **IPC buffer** của U-mode (D2-98; spec dòng 58 "header chứa receive window")
// ---------------------------------------------------------------------------------------------

/// Magic của header IPC buffer — `b"KT67b"` little-endian. Shim U-mode
/// (`v2/kernel/vifast-plat-user/lib.rs`, `HEADER_MAGIC`) ghi **đúng** hằng này; kernel chỉ đọc
/// header khi khớp (vùng zero = chương trình chưa dùng header ⇒ bỏ qua, không phá ảnh cũ).
pub const IPC_HEADER_MAGIC: u64 = 0x0000_0062_3736_544b;
/// Số word qword của header: magic + info + receive window (4) + send caps (4).
pub const IPC_HEADER_WORDS: usize = 10;
/// Word `MessageInfo` (bản ghi của shim; ABI vẫn lấy `a2` — D2-15).
pub const IPC_HEADER_W_INFO: usize = 1;
/// Word đầu của receive window (4 CPTR) — spec `abi.toml` dòng 58.
pub const IPC_HEADER_W_WINDOW: usize = 2;
/// Word đầu của CPTR nguồn cap của message đang gửi (4 CPTR) — `[TK]` KT-67b.
pub const IPC_HEADER_W_SEND_CAPS: usize = 6;

/// Header IPC buffer của **một** TCB (model `[TK]`, không phải ABI): `MessageInfo` (bản ghi) +
/// receive window 4 CPTR + 4 CPTR nguồn cap.
///
/// Arch đọc từ **bộ nhớ user** (D2-83: core không đọc user memory) rồi trả qua
/// [`crate::syscall::SyscallEnv::ipc_header`]; `None` = ảnh không dùng header (magic 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IpcHeader {
    /// `MessageInfo` 35 bit bản ghi của shim (không phải nguồn ABI — nguồn là `a2`).
    pub info: u64,
    /// 4 CPTR đích khi **nhận** cap (receive window).
    pub recv_window: [u64; MAX_CAPS],
    /// 4 CPTR nguồn cap của message đang **gửi**.
    pub send_caps: [u64; MAX_CAPS],
    /// **KT-309** — vùng **extra caps** (offset 560, cùng trang IPC buffer): `n` + 4 CPTR nguồn
    /// + `dst`. Đọc **cùng** lần với header (một trang, một magic) — `[TK]`, không phải ABI.
    pub extra: IpcExtraCaps,
}
