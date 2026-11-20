# KẾ HOẠCH CUỐI TUẦN 2026-11-21 → 2026-11-22 (KT-243..KT-251)

Người lập: **Agent 1, được trao toàn quyền Agent 0** (uỷ quyền trực tiếp của người dùng,
2026-11-21 00:05 JST). Hiệu lực tới **hết Chủ nhật 2026-11-22 23:59 JST**.
Nguồn sự thật: `AGENTS.md`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `spec/`.

> Mục tiêu một câu: **mở hai năng lực mới cho ViFastOs** — (1) **kiến trúc thứ hai** (aarch64
> boot thật trên QEMU), (2) **mạng** (virtio-net + DHCP + ICMP echo) — đồng thời **đóng ba nợ
> quyết định** đang chặn (B4/B6/B7) và **chốt K6** bằng bằng chứng.

---

## 0. Vì sao hai năng lực này

`docs/v2/ViFastOs-v2-thiet-ke.md` §10.1 xếp **K4' (plat-aarch64)** ngay sau K6; §10.3 xếp
**D1 (virtio-net/gpu/input)** phụ thuộc **K6** — K6 đã xong phần đo được (KT-76..KT-90,
KT-77b/77c). Cả hai **không** đổi ABI, **không** đổi `spec/`, **không** nới ngưỡng:

| Năng lực | Mốc | Vì sao chọn cuối tuần này | Đo bằng |
|---|---|---|---|
| aarch64 boot | K4' bước 3 (B3.1–B3.3, B3.8) | `aarch64-unknown-none` **đã cài**, `qemu-system-aarch64` **có**, bước 1+2 đã DONE (KT-160/164/167) | ảnh in marker + halt sạch trên QEMU; TAP; MMU bật |
| Mạng | D1 (virtio-net) | QEMU `virt` có virtio-net sẵn; `vifast-virtio` đã có khuôn blk/console | host test codec; DHCP lấy 10.0.2.15; `ping 10.0.2.2` |

**Không** làm trong cuối tuần: K4' B3.4 (nửa cao/physmap — cần quyết định Q4 về trait), B3.5
(`VSpace` thật), B3.6 (TLBI shootdown), B3.7 (U-mode — cần mở `spec/abi.toml [calling.aarch64]`);
K5'; v2.0; đổi ABI; nới ngưỡng.

---

## 1. Quyết định A0 mở đường (ghi vào `DECISIONS.md`)

| Mã | Nội dung |
|---|---|
| **D2-195** | Mở **K4' bước 3** phạm vi **B3.1–B3.3 + B3.8**; trả lời Q1 (`-kernel` trực tiếp), Q3 (`spec/kernel.toml` **không** thuộc bước 3), Q5 (`MAX_HARTS=8`); **hoãn** Q2 (mở `[calling.aarch64]`), Q4 (trait nửa cao), Q6 (U-mode) — ghi rõ là hoãn, không phải đạt |
| **D2-196** | Mở **D1** phạm vi **virtio-net + codec mạng + DHCP + ICMP echo**; **không** TCP, **không** socket API, **không** đổi `spec/` |
| **D2-197** | Chốt **B4** (`cd`/`pwd` **thuộc** "tương đương v1" — bản đóng băng `v1-archive` có), **B6** (build-inputs = bao đóng cargo metadata ∪ tệp ảnh phụ thuộc, **trừ** `v2/agent`, `v2/docs`, `target*`, `artifacts`), **B7** (sửa **test + chú thích** `smp.rs` theo `HART_STRIDE`, **và** thêm L0 workspace kernel vào `gate --fast`) |
| **D2-198** | Uỷ quyền vòng này: tối đa **4 sub-agent**, mỗi sub-agent **một worktree riêng**; sub-agent **được commit trên nhánh task của mình** với `VIFAST_ROLE=lead` (lead uỷ quyền có thời hạn), **không** push, **không** chạm `main`; **chỉ lead** merge + push; T0/T1/T2 theo `AGENTS.md` §6.1; tái dùng bằng chứng theo §6.3 |

---

## 2. Task (mỗi task ≤ 3 h, có tiêu chí đo được)

| Mã | Task | Phụ thuộc | Ngân sách | Tiêu chí xong |
|---|---|---|---|---|
| **KT-243** | **B4/B6/B7**: `cd`/`pwd` vào `v1-parity.toml` + runner; build-inputs theo D2-197; sửa `smp.rs` + thêm L0 kernel vào `gate --fast` | — | 3 h | parity `cd`/`pwd` chạy thật trên ảnh sản phẩm; build-inputs **không** đổi khi chỉ sửa `v2/agent`/`v2/docs` (ca dương); `smp` test xanh **và** có bước gate gọi nó; 3 ca âm |
| **KT-244** | `vifast-net` (thuần, host-testable): Ethernet/ARP/IPv4/ICMP/UDP/DHCP encode+decode, checksum, địa chỉ | — | 3 h | crate mới `no_std` + `deny(unsafe_code)`; ≥ 40 test host; mỗi codec có ca âm (sai checksum/độ dài/phiên bản); `v2 check` xanh |
| **KT-245** | **virtio-net driver**: `vifast-virtio::net` (config/feature/header thuần) + `plat-riscv64::virtio_net` (MMIO, 2 queue, MAC) + quét PCI | KT-244 (kiểu) | 3 h | QEMU `virt` nhận thiết bị, đọc MAC thật, gửi/nhận được **một** frame thô; host test cho phần thuần; ảnh `ktest-net` |
| **KT-246** | **Nối mạng**: DHCP client lấy địa chỉ từ QEMU slirp + ARP + ICMP echo tới gateway | KT-244, KT-245 | 3 h | log QEMU: `net: dhcp ip=10.0.2.15 gw=10.0.2.2` + `net: ping 10.0.2.2 ok rtt=…`; ca âm (không có `-netdev` ⇒ fail-closed) |
| **KT-247** | **K4' B3.1**: crate `vifast-kbin-aarch64` + `linker.ld` + `_start` + PL011 | — | 3 h | `cargo build --target aarch64-unknown-none` xanh; `qemu-system-aarch64 -M virt -kernel` in **một** dòng marker rồi halt sạch; objdump đúng `0x4008_0000` |
| **KT-248** | **K4' B3.2**: bảng vector 16 ô + `VBAR_EL1` + handler đồng bộ | KT-247 | 2,5 h | test host khẳng định offset từng ô; `brk #0` trên QEMU in đúng `EC`/`ELR`; bảng vector **không** nằm trong `.bss` |
| **KT-249** | **K4' B3.3**: bật MMU (TCR→MAIR→TTBR0→SCTLR→ISB) + identity map | KT-248 | 3 h | sau khi bật, marker qua VA không đổi; 3 đột biến bắt buộc **ĐỎ** (bỏ ISB, sai T0SZ, sai MAIR) |
| **KT-250** | **K4' B3.8 (phần đầu)**: `v2/test/boot/run-aarch64.sh` + TAP + ca âm | KT-249 | 3 h | runner chạy QEMU aarch64, TAP **N/N**, ca âm (marker sai ⇒ đỏ), `v2 check` xanh |
| **KT-251** | **Chốt K6**: xác minh **S1** trên ảnh sản phẩm hiện tại + cập nhật `K6-DOD.md` trung thực | — | 2 h | log serial ảnh sản phẩm có `shell>` + `help`; `K6-DOD.md` ghi S1 **ĐẠT/KHÔNG** kèm `log:dòng`; không sửa tiêu chí |

**Thứ tự tích hợp**: KT-243 (infra) → KT-244/245/246 (mạng) → KT-247..250 (aarch64) → KT-251.
Mỗi task một commit `[KT-##]`, push sau khi T1 xanh; T2 trên ứng viên tích hợp cuối.

---

## 3. Cách chia việc (4 sub-agent, 4 worktree)

| Sub-agent | Worktree | Nhánh | Task |
|---|---|---|---|
| `a64` | `/tmp/kt243-s1` | `task/kt243-a64` | KT-247 → KT-248 → KT-249 → KT-250 |
| `net` | `/tmp/kt243-s2` | `task/kt243-net` | KT-244 → KT-245 → KT-246 |
| `infra` | `/tmp/kt243-s3` | `task/kt243-infra` | KT-243 → KT-251 |
| `lead` | cây chính | `main` | review, tích hợp, T1/T2, push, STATE/QUEUE/DECISIONS |

**Ranh giới tệp** (tránh xung đột merge):

- `a64`: `v2/kernel/vifast-kbin-aarch64/**` (mới), `v2/kernel/vifast-plat-aarch64/**`,
  `v2/kernel/Cargo.toml` (một dòng member), `v2/test/boot/run-aarch64.sh` (mới),
  `v2/rust-toolchain.toml` (**không** sửa — target đã có).
- `net`: `v2/lib/vifast-net/**` (mới), `v2/lib/vifast-virtio/src/net.rs` (mới),
  `v2/kernel/vifast-plat-riscv64/src/virtio_net.rs` (mới), `pci.rs`/`lib.rs` (thêm dòng),
  `v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs` (mới), `v2/Cargo.toml` (một dòng member).
- `infra`: `v2/bin/v2`, `v2/tools/build-inputs*`, `v2/kernel/vifast-plat-common/src/smp.rs`,
  `spec/v1-parity.toml`, `v2/test/parity/**`, `v2/agent/reports/K6-DOD.md`.
- **Không ai** sửa `v2/agent/STATE.md`, `QUEUE.md`, `DECISIONS.md` (lead giữ).

---

## 4. Luật chơi trong cuối tuần (không đổi)

1. **Test đỏ trước** (D2-05): mỗi task có log đỏ trước khi xanh.
2. **Không** nới ngưỡng, **không** `#[ignore]`, **không** retry để lấy xanh, **không** sửa
   `accept()`/kỳ vọng harness.
3. **Không** sửa `spec/` (trừ khi có dòng D2 riêng — cuối tuần này **không** mở).
4. **Không** `cfg(target_arch)` trong `vifast-core`/runtime; `unsafe` chỉ trong crate arch.
5. **Commit**: `[KT-##] <tóm tắt>`; `cargo fmt` trước; T1 xanh trước khi push.
6. **Bằng chứng**: mỗi phép đo có khóa đầu vào (commit + build-inputs + toolchain + lệnh +
   kết quả + log) theo `AGENTS.md` §6.3.
7. **QEMU**: tối đa 3 song song; chỉ `kill <pid>` do mình tạo.
8. Hết ngân sách task ⇒ dừng task đó, ghi `STATE.md`, chuyển task kế.

---

## 5. Định nghĩa "cuối tuần xong"

- [ ] `qemu-system-aarch64` boot ảnh ViFastOs, in marker, MMU bật, TAP xanh, runner tự động.
- [ ] Ảnh riscv64 lấy được địa chỉ qua DHCP và `ping` được gateway trong QEMU.
- [ ] B4/B6/B7 đóng bằng mã + test, không chỉ bằng văn bản.
- [ ] `K6-DOD.md` có kết luận S1 kèm bằng chứng.
- [ ] HEAD cuối: T1 xanh, T2 xanh, cây sạch, đã push, `STATE.md` khớp HEAD.
- [ ] Báo cáo cuối tuần `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md`.
