<p align="center">
  <img src="resources/brand/vifastos-lockup-cream.png" alt="ViFastOs · microkernel kiểu seL4 viết bằng Rust" width="620">
</p>

# ViFastOs

**Một hệ điều hành microkernel viết bằng Rust.** v1 đã đóng băng và vẫn chạy được trên QEMU; **v2 đang được xây lại từ đầu** để sạch hơn, nhỏ hơn, kiểm chứng được.

> **Đọc 30 giây.** ViFastOs là "lõi" quản lý CPU, bộ nhớ và quyền truy cập. Lõi cố tình giữ **thật ít việc**; file system, driver, shell… chạy như chương trình bình thường bên ngoài. v1 đã làm được điều đó nhưng lõi còn "phình" và có 6 lỗi thiết kế gốc. v2 giữ nguyên triết lý, làm lại cho đúng: **15 syscall thay vì 58**, object cấp từ Untyped, lập lịch có ngân sách (MCS), kiểm thử bằng mô phỏng tất định.
>
> **Hiện tại:** v2 mới xong K0 (bản vẽ + xưởng), **chưa có kernel để boot**. Việc tiếp theo là KT-03 (làm cổng kiểm) rồi KT-07 (boot riscv64 — lúc đó `v2/kernel/` mới xuất hiện).

---

## 1. Nếu bạn mới học năm nhất

Hãy tưởng tượng máy tính là một **toà nhà**, còn hệ điều hành là **ban quản lý**:

- **Kernel** là lễ tân + bảo vệ: giữ chìa khoá, thang máy, camera. Mọi người muốn vào phòng phải xin phép.
- **Ứng dụng** là công ty thuê văn phòng. Công ty muốn dùng phòng họp, kho, điện… phải gửi yêu cầu xuống lễ tân.
- **Microkernel** là kiểu ban quản lý **rất mỏng**: chỉ giữ việc thiết yếu (mở cửa, cấp chìa, báo cháy). Dọn vệ sinh, sửa điều hoà, trông xe… thuê ngoài. Công ty thuê ngoài làm hỏng việc thì toà nhà vẫn đứng, chỉ dịch vụ đó ngừng.
- **Capability** là **vé**. Không có vé thì không vào được phòng, dù biết đường.
- **Syscall** là **mẫu đơn** gửi xuống lễ tân. Càng ít loại đơn, càng dễ kiểm tra và ít lỗi.

Vì sao phải làm microkernel? Vì nếu mọi thứ nằm chung trong lõi, một driver viết sai có thể làm sập cả hệ thống. Tách ra ngoài thì lỗi bị nhốt trong một tiến trình.

v2 có thêm vài ý tưởng quan trọng, giải thích bằng ví dụ:

| Khái niệm | Ví dụ đời thường |
|---|---|
| **Untyped** | Tấm bìa carton chưa cắt |
| **Retype** | Cắt tấm bìa thành hộp, tấm lót… |
| **CNode** | Cái ví đựng vé |
| **TCB** | Hồ sơ một nhân viên: đang làm, đang nghỉ, hay đang chờ |
| **Endpoint** | Hộp thư của toà nhà |
| **Reply** | Phong bì trả lời có sẵn tem |
| **SchedContext** | Thẻ ngân sách thời gian: mỗi người được 10 ms |
| **MCS** | Cơ chế nạp lại ngân sách khi hết |

---

## 2. v1 có phải microkernel không?

**Đúng — v1 đã là microkernel** kiểu seL4: capability-based, object sinh từ Untyped, IPC đồng bộ, driver/VFS/pager/shell chạy ở user space, ba kiến trúc RISC-V64 / AArch64 / x86_64.

Nhưng "microkernel" mới là **triết lý**; v1 làm chưa sạch:

- Lõi vẫn giữ **58 syscall**, trong đó có ramfs, file descriptor, block, console, tham số tiến trình.
- Kernel còn **tự dựng VSpace** cho server, còn `COPY_WINDOWS`, `read_user`/`write_user` — tức còn làm việc của user.
- `lib.rs` của kernel dài **17.715 dòng**.
- **Untyped chỉ là quota**: object thật vẫn nằm trong pool tĩnh, thành hai trần chồng nhau.
- Trạng thái static bị **reset hai lần**; đồng hồ nằm trong vùng reset được → lỗi thời gian lùi (T33a).
- SMP chưa có giao thức thống nhất (kênh wake lệch, shootdown AP→BSP sai).

Nói ngắn: **v1 đúng hướng nhưng ranh giới kernel/user chưa sạch, và trạng thái không tất định.** Đó là lý do v2 không "thêm tính năng", mà **làm lại lõi cho đúng triết lý microkernel**.

### Sáu lỗi gốc của v1 (bản dễ hiểu)

| Mã | Lỗi | Hậu quả |
|---|---|---|
| R1 | Lõi ôm quá nhiều dịch vụ | 58 syscall, khó kiểm, dễ lỗi |
| R2 | Trạng thái reset hai lần, đồng hồ nằm trong đó | Thời gian lùi, treo ngẫu nhiên |
| R3 | Trạng thái luồng tách khỏi hàng chờ | Mất tin nhắn |
| R4 | Kernel đọc/ghi bộ nhớ user | Lỗi exec, khó bảo vệ |
| R5 | Identity map + pool tĩnh | Trần 566 KB, 24 TCB |
| R6 | Test kiểu "boot rồi grep" | 11 vòng mới tìm ra một lỗi |

Chi tiết đầy đủ ở [`docs/v2/ViFastOs-v2-thiet-ke.md`](docs/v2/ViFastOs-v2-thiet-ke.md) §1 và sổ lỗi [`docs/v1/KNOWN-ISSUES.md`](docs/v1/KNOWN-ISSUES.md).

---

## 3. v1 và v2 khác nhau ở đâu?

| | **v1** | **v2** |
|---|---|---|
| Vai trò | Đã đóng băng, dùng làm tham chiếu | Đang phát triển |
| Vị trí | `crates/` | `v2/` |
| Triết lý | Microkernel | Microkernel + capability |
| Syscall | 58 | **15** (0–13 + `Exit`) |
| Mã lỗi | 22 | **19** |
| Dịch vụ | Một phần còn trong kernel | Ra user hết |
| Bộ nhớ | Pool tĩnh | Object trong **Untyped** của người tạo |
| Lập lịch | Chưa MCS đầy đủ | **MCS** (SchedContext đi theo IPC) |
| Kiểm thử | Boot + grep | **Sim tất định + mô hình tham chiếu + QEMU** |
| Trạng thái | Chạy 3 kiến trúc trên QEMU | **Chưa boot** (mới xong K0) |

Cùng một lõi, ba cổng: RISC-V64 (OpenSBI), AArch64 (QEMU virt), x86_64 (UEFI/OVMF). v2 làm riscv64 trước, hai cổng còn lại sau.

---

## 4. v2 đang ở đâu?

v2 chia lộ trình thành các mốc, **không đặt deadline ngày** — mỗi mốc có cổng kiểm riêng:

| Mốc | Việc chính | Sản phẩm nhìn thấy được |
|---|---|---|
| **K0** | Bản vẽ, xưởng, thước đo | `spec/abi.toml`, workspace `v2/`, lệnh `v2/bin/v2` |
| **K0.5** | Boot riscv64 trên QEMU | `v2/kernel/`, `v2/boot/` |
| **K1a** | Sim + object cơ bản | CNode/MDB, Untyped |
| **K1b** | Luồng, lập lịch, ngân sách | TCB, scheduler, MCS |
| **K1c** | Gửi tin, trả lời, exit | IPC, Reply, Notification, Exit |
| **K2** | Bộ nhớ ảo | page table, map/unmap |
| **K3** | riscv64 đủ dùng | kernel chạy QEMU |
| **K3.5** | Chương trình người dùng | procsrv, shell |
| **K6** | Ngang v1 | đủ tính năng v1 |

**Hôm nay:** KT-01 (đóng băng v1) và KT-02 (khung `v2/`, port `vifast-fs`, `vifast-fsutil`, `vifast-elf`) đã DONE. KT-03 (cổng kiểm) là việc tiếp theo; sau đó KT-07 mới tạo file kernel. Vì vậy nhìn vào `v2/` **chưa thấy `kernel/` là bình thường**, không phải dự án đứng im.

Theo dõi tiến độ thật ở [`v2/agent/STATE.md`](v2/agent/STATE.md) và danh sách việc [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

---

## 5. Cấu trúc repo

```
crates/        v1 — ĐÓNG BĂNG, chỉ đọc, không sửa
v2/            v2 — workspace mới
  lib/         thư viện dùng lại (fs, fsutil, elf…)
  bin/         lệnh v2 (sau KT-03)
  kernel/      kernel v2 (sau KT-07)
  agent/       QUEUE · STATE · BLOCKED · DECISIONS · JOURNAL
viscript/      ViScript/ViShell — ngôn ngữ kiểu tĩnh, chạy đa nền tảng
docs/v1/       tài liệu, báo cáo, sổ lỗi của v1 (đọc-only)
docs/v2/       thiết kế v2 + prompt vận hành Agent 2
docs/viscript/ thiết kế ViScript + SPEC-lex
```

**Quy tắc vàng:** v1 bất động. Mọi thay đổi đi vào `v2/` hoặc `viscript/`. Không sửa `crates/`, `docs/v1/`, `spec/` (trừ task được giao).

---

## 6. Chạy thử

### v1 (đã đóng băng, vẫn boot được)

```bash
rustup target add x86_64-unknown-none aarch64-unknown-none riscv64gc-unknown-none-elf
./scripts/build.sh                       # host test + build 3 kiến trúc
python3 scripts/test-qemu.py --repeat 1  # boot thật trên QEMU
```

Kết quả mong đợi:

```
ViFastOs booting on riscv64
...
SELFTEST PASS
...
SHELL READY
shell>
```

v1 có 389 host test, 677 case âm, 3/3 kiến trúc PASS — nhưng vẫn còn lỗi treo x86_64 ~0,3–1 % (T33) và lỗi FS T26. **Đây là số của v1, không phải v2.**

### v2 (đang xây, chưa boot)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --manifest-path v2/Cargo.toml   # workspace hiện xanh
# ./v2/bin/v2 check                          # sẽ có sau KT-03
```

v2 **chưa có kernel để chạy**. Đừng dùng lệnh v1 để đánh giá v2.

---

## 7. Dự án làm việc như thế nào?

Đây là dự án kỹ thuật có **ba vai**:

| Vai | Việc |
|---|---|
| **Agent 0** | Kiến trúc sư, người duyệt ABI/SPEC |
| **Agent 1** | Soát, vá tài liệu, giữ tính nhất quán |
| **Agent 2** | Thi công: viết code, test, chạy cổng, commit |

Mọi tiến độ nằm trong [`v2/agent/`](v2/agent/):

- `QUEUE.md` — việc cần làm, mỗi task ≤3 giờ, có tiêu chí xong.
- `STATE.md` — đang ở task nào, bước mấy, commit nào.
- `BLOCKED.md` — đang tắc ở đâu, chờ ai.
- `DECISIONS.md` — các quyết định đã chốt (D2-01…).
- `JOURNAL/` — nhật ký: lệnh đã chạy, log, test đỏ trước.

Mỗi task đi theo 8 bước: **đọc → lập kế hoạch → viết test đỏ → code → cổng → commit + push → ghi STATE → task kế**. Không có bước "viết code trước rồi thêm test sau".

Trước mỗi vòng, một **subagent review độc lập** kiểm hướng đi (`ON-TRACK` / `DRIFT` / `UNSAFE`). Review chạy song song với bước đọc/viết test, và chỉ chặn trước khi sửa code sản phẩm hoặc commit.

Quy trình đầy đủ: [`docs/v2/AGENT2-PROMPT.md`](docs/v2/AGENT2-PROMPT.md). Luật chung: [`AGENTS.md`](AGENTS.md).

---

## 8. Đọc tiếp

| Nếu bạn muốn… | Đọc |
|---|---|
| Hiểu thiết kế v2 đầy đủ | [`docs/v2/ViFastOs-v2-thiet-ke.md`](docs/v2/ViFastOs-v2-thiet-ke.md) |
| Biết vì sao viết lại, chốt gì | [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md) |
| Xem lỗi v1 và ca kiểm tương ứng | [`docs/v1/KNOWN-ISSUES.md`](docs/v1/KNOWN-ISSUES.md) |
| Theo dõi tiến độ | [`v2/agent/STATE.md`](v2/agent/STATE.md), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md) |
| Hiểu ViScript | [`docs/viscript/ViScript-thiet-ke.md`](docs/viscript/ViScript-thiet-ke.md) |
| Đọc báo cáo v1 (lịch sử) | [`docs/v1/README.md`](docs/v1/README.md) |

---

## 9. Câu hỏi thường gặp

**v1 có phải microkernel không?**
Có. Nhưng lõi còn ôm dịch vụ và trạng thái chưa tất định, nên chưa "sạch". v2 là bản làm đúng triết lý đó.

**Sao không sửa v1 mà phải viết lại?**
Sáu lỗi gốc nằm ở cấu trúc (trạng thái static, pool tĩnh, ranh giới kernel/user). Sửa từng lỗi trong khi vẫn chạy là được, nhưng sẽ mất nhiều thời gian và khó chứng minh đã sạch. v1 được đóng băng làm tham chiếu.

**389 test, 677 case âm — v2 có chưa?**
Chưa. Đó là số của v1. v2 sẽ có bộ test riêng: sim tất định, mô hình tham chiếu, QEMU TAP.

**Bao giờ xong?**
Không đặt ngày. Mỗi mốc có cổng kiểm; xong cổng mới đi tiếp.

**Tôi muốn đóng góp?**
Đọc tài liệu, chạy v1, phản biện thiết kế v2. Phần thi công do Agent 2 làm theo QUEUE; thay đổi ABI/SPEC phải qua Agent 0.

---

## 10. Trạng thái pháp lý

Mã nguồn: **MIT OR Apache-2.0**. v1 đóng băng ở tag `v1-archive` (commit `ca2eaf5`); không có tag `v1-final`.
