# KT-242 — Hồ sơ L1: thời gian nằm ở đâu, tối ưu gì, bằng chứng tương đương

**Ngày**: 2026-11-20 · **Agent 2 (luồng L1/kế hoạch)** · nhánh `task/kt242-l1` · HEAD trước task: `21cc75cd`
**Máy đo**: macOS 27.0.1 arm64, **12 core**, Rust **1.100.0** (`0ef1bc386 2026-11-09`), profile **debug**
(`cargo test`, **không** `--release` — đúng cấu hình hiện hành).
**Driver**: `v2/sim/vifast-sim/tests/l1_k1c.rs`, test `driver_smp_cung_seed_cung_trace`, `SO_OP = 64`.

> Nhãn số liệu (QD §2.2): mọi con số dưới đây là **[ĐO]** trên máy nêu trên, sinh bằng lệnh ở §7.
> Con số **suy ra** (không đo trực tiếp) mang nhãn **[ƯL]**.

---

## 1. Kết luận trước (đọc nhanh)

1. **~92 % thời gian L1 là kiểm bất biến**, không phải mô phỏng:
   **53 %** là **mô hình tự kiểm** (`sched_after_op` — 11 checker sau **mỗi** public op) và
   **39 %** là **driver kiểm** (`kiem()` — 7 checker + `tlb_violations` sau **mỗi** op).
   Mô phỏng thật chỉ **~6 %**; định dạng trace **~1,6 %**.
2. Vì thế **không có chỗ nào để "tối ưu" mà không nới ngưỡng**. Cách duy nhất giảm thời gian
   **wall** là **chạy song song** — và đó là tối ưu duy nhất đã làm.
3. **Không** đổi `SO_OP`, **không** giảm seed, **không** đổi tập cấu hình smp, **không** bỏ kiểm
   tất định, **không** đổi ngưỡng, **không** đổi profile build.
4. **Bằng chứng tương đương**: digest trace **trùng tuyệt đối** trước/sau ở **1.000 seed × 4
   cấu hình smp** và **10.000 seed × 4 cấu hình smp** (§5). Chia lô cũng **không** đổi digest
   (XOR giao hoán — §5.3).
5. **`doi` không có việc thừa**: biểu thức `if seeds <= 1_000 { seeds } else { 2_000.min(seeds) }`
   **tương đương đúng** `2_000.min(seeds)` với **mọi** `seeds` — nhánh `if` là mã dư, **không**
   phải việc dư. Đã kiểm bằng bảng (§4.2).

---

## 2. Phương pháp đo

Ba lượt, **không** dùng `perf` (macOS không có); `sample(1)` cho call-graph nhưng chỉ ~320 mẫu
trong 10 s nên **không** đủ phân giải — dùng làm định hướng, **không** dùng làm số liệu.

1. **Lượt A — `sample`**: xác nhận nóng nằm trong `check_*` của `vifast_sim::kobj` và trong
   `mot_luot`/`mot_op`, **không** nằm trong `format!`. (Định hướng, không trích số.)
2. **Lượt B — đếm thời gian từng pha trong driver** (chèn tạm `Instant`, **đã gỡ**):
   `Rig::new` / `mot_op` / `kiem` / `format!`+`anh_chup` / cuối lượt.
3. **Lượt C — đếm thêm `sched_after_op` của mô hình** (chèn tạm static trong `kobj.rs`, **đã gỡ**):
   tách "mô hình tự kiểm" khỏi "mô phỏng thật".

Cả hai lần chèn đều **đã gỡ sạch** trước khi commit — kiểm bằng `git diff` trên
`v2/sim/vifast-sim/src/kobj.rs` (rỗng) và bằng việc tệp driver cuối cùng **không** còn
`Instant`/`PROF`/`AtomicU64` nào.

---

## 3. Kết quả — thời gian nằm ở đâu

**1.000 seed × smp 1 = 2.000 lượt** (mỗi seed chạy 2 lần vì `doi = 1.000`), tổng **28,40 s**.

### 3.1 Theo pha của driver (lượt B)

| Pha | Tổng (ms) | % | Mỗi lượt (ms) |
|---|---:|---:|---:|
| `Rig::new` (dựng máy + object + VSpace + 4 TCB) | 2.529,7 | **8,9 %** | 1,265 |
| `mot_op` (thân op + **mô hình tự kiểm**) | 14.233,2 | **50,1 %** | 7,117 |
| `kiem()` (7 checker + `tlb_violations`) | 11.086,9 | **39,0 %** | 5,543 |
| `format!` + `anh_chup()` (dựng dòng trace) | 367,6 | **1,3 %** | 0,184 |
| Cuối lượt (`check_vm` + assert bảo toàn) | 179,4 | **0,6 %** | 0,090 |
| **Tổng** | **28.396,7** | 100 % | 14,198 |

### 3.2 Tách "mô hình tự kiểm" khỏi "mô phỏng thật" (lượt C)

`Machine::sched_after_op` (`v2/sim/vifast-sim/src/kobj.rs:6136`) chạy **11 checker** sau **mỗi**
public op: `check_sched`, `check_smp`, `check_mailboxes`, `check_ipc`, `check_reply_links`,
`check_notifications`, `check_waitaddr`, `check_frames`, `check_pages`, `check_wx`, TLB-safety.

| Hạng mục | Tổng (ms) | % tổng |
|---|---:|---:|
| `sched_after_op` (mô hình tự kiểm) | 15.776,7 | **53,0 %** |
| `kiem()` (driver kiểm) | 11.483,7 | **38,6 %** |
| Định dạng trace | 464,4 | 1,6 % |
| Cuối lượt | 167,9 | 0,6 % |
| **Mô phỏng thật** (op + cấp phát, sau khi trừ `sched_after_op`) | **~1.896** | **~6,4 %** |
| **Tổng** | **29.788,8** | 100 % |

- `sched_after_op` được gọi **165.244** lần cho 2.000 lượt ⇒ **82,6 lần/lượt** (65 driver-op +
  ~17 public op trong `Rig::new` + các op phụ như `set_current_hart`/`tlb_translate` của nhánh
  unmap), **0,095 ms** mỗi lần.
- Lượt C có tổng cao hơn lượt B **4,9 %** — do chính `Instant::now()` chèn thêm (4 lần/op ×
  128.000 op) cộng dao động máy. Vì vậy **tỉ lệ** ở §3.2 là số dùng, **không** phải tổng tuyệt đối.

### 3.3 Đọc kết quả này đúng cách

- **Không** phải "driver chậm". `kiem()` là **hợp đồng của L1** ("bất biến sau **mỗi** op") —
  bỏ nó là **nới ngưỡng**, cấm (AGENTS §4.4).
- **Không** phải "mô hình thừa". `sched_after_op` là `debug_assert!` của mô hình, dùng chung với
  **mọi** test K1a..K2; bỏ nó là **nới ngưỡng** ở tầng mô hình, cấm.
- Bốn checker của driver (`check_sched`, `check_ipc`, `check_mailboxes`, `check_reply_links`)
  **trùng** với bốn checker của `sched_after_op`. Nhưng chúng **không** thay thế nhau: checker
  của mô hình là `debug_assert!` (**biến mất** ở build release), còn `kiem()` là **bảo đảm của
  cổng**. Vì vậy **không** gỡ bên nào.
- Hệ quả: **~92 % thời gian là bất biến**, và bất biến **không** được giảm ⇒ chỉ còn **song song**.

---

## 4. Đã kiểm "có đang làm thừa không"

### 4.1 Dựng trace cho seed **không** so sánh

Với `seed >= doi`, `mot_luot` vẫn dựng `Vec<String>` 64 dòng rồi chỉ dùng `a.len()`. Đây là
**việc thừa thật** — nhưng đo được **1,3–1,6 %** tổng (§3.1/§3.2). **Quyết định: KHÔNG sửa.**
Lý do: đổi đường dựng trace là đổi **đường sinh bằng chứng** của L1 để tiết kiệm < 2 %; rủi ro
không tương xứng. Ghi lại đây để lần sau khỏi đo lại.

### 4.2 `doi` — hai lượt cho seed nào

```rust
let mac_dinh = if seeds <= 1_000 { seeds } else { 2_000.min(seeds) };
```

| `seeds` | nhánh `if` cho | `2_000.min(seeds)` | bằng nhau? |
|---:|---:|---:|:--:|
| 500 | 500 | 500 | ✔ |
| 1.000 | 1.000 | 1.000 | ✔ |
| 1.500 | 1.500 | 1.500 | ✔ |
| 2.000 | 2.000 | 2.000 | ✔ |
| 10.000 | 2.000 | 2.000 | ✔ |
| 100.000 | 2.000 | 2.000 | ✔ |

⇒ Biểu thức **tương đương đúng** `2_000.min(seeds)` với **mọi** `seeds`. Nhánh `if` là **mã dư**
(không phải **việc** dư): nó **không** làm thêm lượt nào. **Không** có việc thừa ở `doi`.
**Không** sửa (sửa chỉ để gọn mã, không đổi thời gian — và mọi thay đổi driver phải có bằng
chứng tương đương, xem §5).

### 4.3 Cấp phát mỗi seed

`Rig::new` = **8,9 %** tổng, và **~89 %** của phần đó là `sched_after_op` của chính các public op
dựng máy — **không** phải cấp phát. Cấp phát thật (arena/`Vec` của `Machine`, CNode 64 ô, 3
`retype` Frame) nằm trong **~1 %** tổng. **Không** sửa: không có gì để giành.

---

## 5. Tối ưu đã làm + bằng chứng tương đương

### 5.1 Tối ưu duy nhất: **chia lô seed** (`VIFAST_SHARD=i/n`)

- Driver nhận `VIFAST_SHARD=i/n` (mặc định `0/1` = **không** chia ⇒ hành vi **y nguyên**): tiến
  trình này chạy các seed `seed % n == i`.
- **Không** đổi: tập seed (hợp các lô = `0..seeds`), `SO_OP = 64`, tập cấu hình smp, `doi`, mọi
  bất biến sau mỗi op, mọi ngưỡng (`ipc_sent > 5 × số_seed_của_lô`, `ipc_delivered > 0`, phủ VM).
- Ngưỡng **tổng** dùng **số seed của lô** ⇒ **cùng** ngưỡng **trên mỗi seed**; chia lô **không**
  nới ngưỡng.
- Lô rỗng (`i >= seeds`) ⇒ **panic rõ**, **không** được coi là xanh.

### 5.2 Thêm **digest trace** (`VIFAST_L1_DIGEST=1`) — phương tiện bằng chứng, không phải ngưỡng

- In `# L1 DIGEST: smp N shard i/n seeds X digest <16 hex>`; mặc định **tắt** (không đặt env ⇒
  không tốn gì, đầu ra **y nguyên**).
- Digest = **XOR 64-bit** của `vifast_sim::trace_hash(trace)` **từng seed** (kèm `seed`). XOR
  **giao hoán + kết hợp** ⇒ `digest(hợp các lô) = XOR digest từng lô`. Nhờ vậy chứng minh được
  "chia lô không đổi kết quả" ở mức **toàn bộ trace**, không phải mẫu.
- **Không** assert trên digest; **không** thay `assert_eq!(a, b)` của kiểm tất định.

### 5.3 Bằng chứng — digest **trùng tuyệt đối**

**1.000 seed** (mỗi seed 2 lượt, `doi = 1.000`):

| smp | digest **trước** | digest **sau** | |
|---|---|---|---|
| 1 | `237f3fa69d2bf1ea` | `237f3fa69d2bf1ea` | **TRÙNG** |
| 2 | `2f4615be1d1ae3de` | `2f4615be1d1ae3de` | **TRÙNG** |
| 4 | `d83c8947b19b19bf` | `d83c8947b19b19bf` | **TRÙNG** |
| 8 | `abaa6f381073088c` | `abaa6f381073088c` | **TRÙNG** |

**10.000 seed** (`doi = 2.000`):

| smp | digest **trước** | digest **sau** | |
|---|---|---|---|
| 1 | `f11e8bf72f8083fb` | `f11e8bf72f8083fb` | **TRÙNG** |
| 2 | `65d347211408784b` | `65d347211408784b` | **TRÙNG** |
| 4 | `ed6726edf194cca9` | `ed6726edf194cca9` | **TRÙNG** |
| 8 | `744ad4c446c308ef` | `744ad4c446c308ef` | **TRÙNG** |

**Chia lô không đổi digest** (1.000 seed, smp 1, 22 lô chạy 11 tiến trình):

```
digest(0/1) = 237f3fa69d2bf1ea
XOR digest(0/22 … 21/22) = 237f3fa69d2bf1ea   ← trùng
```

Bộ đếm tổng cũng trùng **từng số** (1.000 seed, smp 1): `ipc_sent 12245`, `ipc_delivered 2143`,
`ipc_dropped 8042`, `vm map 2614 / walk 700 / unmap 1342 / fault 2349 / shootdown 1342` — **giống
hệt** chạy một tiến trình.

### 5.4 Số đo trước/sau

**Một tiến trình** — đo **xen kẽ** A/B/A/B/A/B (cùng máy, cùng tải) để loại nhiễu:

| Lượt | `before` | `after` |
|---|---:|---:|
| 1 | 28 s | 29 s |
| 2 | 32 s | 31 s |
| 3 | 29 s | 30 s |
| **trung vị** | **29 s** | **30 s** |

⇒ **Không hồi quy**: chênh lệch nằm trong nhiễu máy (máy **không rảnh** — xem §6.6). Đường chạy
một seed **không** đổi (chỉ thêm một phép chia lô `step_by(1)` và một `env::var` mỗi smp).

**Chia lô** (wall, 11 worker trên 12 core):

| Mức | Một tiến trình | Chia lô | Nhanh hơn |
|---|---:|---:|---:|
| smoke — 1.000 seed × smp 1 | 28 s | **7–8 s** | **3,5–4,0×** |
| medium — 10.000 seed × smp 1,2,4,8 | 784 s tổng CPU (4 tiến trình song song: **364 s** wall) | **248 s** | **1,5×** so với 4 tiến trình; **3,2×** so với tổng CPU |
| full — 100.000 seed × smp 1,2,4,8 | 6.989 s tổng CPU (4 tiến trình song song: **2.088 s** wall) | **[ƯL] 12–15 phút** | **[ƯL] 2,3–2,9×** |

> **L1-full KHÔNG chạy lại trong task này** (theo chỉ đạo: không chạy L1-full nếu không cần).
> Con số 6.989 s là **tổng** của bốn lần đo đã có: 1.501,50 + 1.626,23 + 1.773,54 + 2.087,71 s
> (`v2/agent/JOURNAL/kt242-logs/kt242-l1-smp{1,2,4,8}.log`). Wall chia lô là **[ƯL]** vì chưa đo.
>
> **Vì sao medium chỉ 1,5× mà smoke 4×**: lúc đo medium máy **đang bận** (load average **11,25**,
> một `rustc` của agent khác chiếm 97 % CPU — §6.6). 24 task trên 11 worker chỉ đạt **3,2×**
> song song thật (`user` 794 s / `wall` 248 s). Trên máy **rảnh** tỉ lệ này cao hơn; smoke đo
> lúc tải thấp hơn cho **4×**. Vì vậy con số `full` là **khoảng**, không phải một điểm.

---

## 6. Giới hạn của hồ sơ này

1. **Một máy, một kiến trúc** (macOS arm64, 12 core). Tỉ lệ % có thể lệch trên máy khác; **kết
   luận định tính** ("~92 % là kiểm bất biến") bám vào **cấu trúc mã**, không bám vào máy.
2. **Profile debug**. Build release sẽ **bỏ** `debug_assert!` của mô hình ⇒ tỉ lệ đổi hẳn. Đó
   chính là lý do **không** đổi profile: đổi profile là đổi **thứ được kiểm**, không phải tối ưu.
3. Lượt C có overhead đo **+4,9 %**; dùng **tỉ lệ**, không dùng tổng tuyệt đối.
4. Wall chia lô phụ thuộc **số core** và **tải máy**; số ở §5.4 đo trên máy **rảnh**.
5. **Chưa** đo L1-full chia lô — chỉ **[ƯL]**.
6. **Máy đo KHÔNG rảnh**: trong lúc đo có agent khác chạy `rustc` (97 % CPU) và load average
   dao động **5,6 → 11,3**. Mọi số wall ở §5.4 vì thế là **cận trên** (chậm hơn máy rảnh), và
   phép so A/B ở §5.4 được làm **xen kẽ** để nhiễu tác động đều hai bên. Số **tổng CPU**
   (`user`) thì ổn định hơn và khớp với các lần đo trước (`JOURNAL/kt242-logs/`).

---

## 7. Lệnh tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.100.0
export CARGO_TARGET_DIR=/tmp/kt242-s4-target

# build một lần
cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test l1_k1c --no-run

# 1.000 seed, một tiến trình, kèm digest
VIFAST_L1_DIGEST=1 VIFAST_SEEDS=1000 VIFAST_SMP=1 \
  <bin> --nocapture

# 1.000 seed, chia 22 lô trên 11 tiến trình (dùng công cụ chính thức)
bash v2/tools/l1-tier.sh --tier smoke --jobs 11

# 10.000 seed × 4 cấu hình smp
bash v2/tools/l1-tier.sh --tier medium

# 100.000 seed × 4 cấu hình smp (hợp đồng K6 — chạy MỘT MÌNH)
bash v2/tools/l1-tier.sh --tier full
```

Đo pha (chỉ khi cần đo lại): chèn `Instant` quanh `Rig::new`/`mot_op`/`kiem`/`format!` trong
`mot_luot`, và một static `AtomicU64` bọc `sched_after_op` trong `kobj.rs`; **gỡ cả hai** trước
khi commit (đúng cách tài liệu này được làm).
