# KT-242 — Ba mức L1: `smoke` / `medium` / `full`

**Ngày**: 2026-11-20 · **Agent 2 (luồng L1/kế hoạch)** · nhánh `task/kt242-l1` · HEAD trước task: `21cc75cd`
**Quyết định**: A0 duyệt ba mức L1 (KT-242) · **Công cụ**: `v2/tools/l1-tier.sh`
**Hồ sơ đo**: [`kt242-l1-profile.md`](kt242-l1-profile.md)

---

## 1. Ba mức là gì

| Mức | Seed | Cấu hình smp | Dùng khi nào | Wall [ĐO] |
|---|---:|---|---|---:|
| **`smoke`** | 1.000 | `1` | sau khi sửa driver / mỗi vòng lặp | **7 s** (11 worker) · 28 s (1 tiến trình) |
| **`medium`** | 10.000 | `1,2,4,8` | trước khi báo cáo / trước khi mở PR | **248 s** (11 worker, máy bận) |
| **`full`** | 100.000 | `1,2,4,8` | **hợp đồng K6**: 4 × 100k = **400k seed** | **[ƯL] 12–15 phút** (11 worker, máy rảnh) · 6.989 s tổng CPU (1 tiến trình/cấu hình) |

**Bất biến của cả ba mức** (không mức nào được nới):

- `SO_OP = 64` op/seed — **không** đổi.
- Mọi bất biến kiểm **sau mỗi op** (`check_sched`, `check_harts`, `check_ipc`, `check_waitaddr`,
  `check_mailboxes`, `check_caps`, `check_reply_links`, `tlb_violations`) — **không** bỏ.
- Kiểm tất định "cùng seed ⇒ cùng trace" cho `doi = min(2.000, seeds)` seed đầu — **không** bỏ.
- Ngưỡng `ipc_sent > 5 × seeds`, `ipc_delivered > 0`, phủ đủ 5 bộ đếm VM — **không** nới.
- `check_vm()` cuối **mỗi** lượt — **không** bỏ.

Ba mức chỉ khác **số seed** và **số cấu hình smp**. Đó là **ngân sách**, không phải **tiêu chí**.

---

## 2. Dùng thế nào

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.100.0

# mức smoke (mặc định)
bash v2/tools/l1-tier.sh --tier smoke

# mức medium, chọn số worker
bash v2/tools/l1-tier.sh --tier medium --jobs 8

# mức full (hợp đồng K6) — chạy MỘT MÌNH, không song song với cổng khác
bash v2/tools/l1-tier.sh --tier full --logdir /tmp/l1-full

# xem kế hoạch mà không chạy
bash v2/tools/l1-tier.sh --tier full --dry-run

# tự kiểm công cụ (27 ca, không chạy sim)
bash v2/tools/l1-tier.sh --selftest
```

Tuỳ chọn khác: `--smp 1,2` (đổi cấu hình), `--seeds N` (đổi số seed), `--no-digest`,
`--logdir DIR`.

**Đầu ra**: một bảng gộp theo smp (`seed`, `ipc_min`, `ipc_sent`, `delivered`, `dropped`,
`map/walk/unmap/fault/shoot`, `digest`), wall tổng, và **PASS/FAIL**. Script **đỏ** nếu:

- bất kỳ lô nào không có `test result: ok` (một lô đỏ ⇒ cả mức đỏ),
- tổng seed của các lô **khác** `seeds` (sót hoặc trùng lô),
- không tìm thấy binary test `l1_k1c` (fail-closed, không lấy nhầm bin `vifast-sim`).

---

## 3. `gate --fast` **không** đổi

- `gate --fast` chạy `cargo test … -p vifast-sim l1_ --` — bộ lọc `l1_` khớp
  `l1_1000_seed_tat_dinh` của `v2/sim/vifast-sim/tests/l1.rs` (**L1 smoke cũ**, 1.000 seed,
  ~1 s). Nó **không** khớp `driver_smp_cung_seed_cung_trace` (tên không bắt đầu bằng `l1_`).
- `v2 check` **chỉ** gọi `v2/tools/l1-tier.sh --selftest` (27 ca, < 1 s, **không** chạy sim) —
  đúng luật KT-134 "script tự kiểm phải có cổng gọi" (`check-selftest-wiring.py --standalone-red`
  đỏ nếu thiếu). **Chạy** một mức thì **không** nằm trong cổng nào: nó chạy hàng chục tiến trình,
  phải chạy **một mình** (AGENTS §6).
- **Mặc định của `gate --fast` vẫn là 1.000 seed.** Task này **không** đổi số seed, không đổi
  bộ lọc `l1_`, không đổi bất kỳ ngưỡng nào của cổng.

---

## 4. Chia lô (`VIFAST_SHARD=i/n`)

Driver `v2/sim/vifast-sim/tests/l1_k1c.rs` nhận thêm:

| Env | Mặc định | Nghĩa |
|---|---|---|
| `VIFAST_SEEDS` | `1000` | tổng số seed (không đổi) |
| `VIFAST_SMP` | `1` | danh sách hart (không đổi) |
| `VIFAST_DOI` | — | số seed chạy 2 lần (không đổi) |
| **`VIFAST_SHARD`** | **`0/1`** | **mới**: tiến trình này chạy `seed % n == i` |
| **`VIFAST_L1_DIGEST`** | **tắt** | **mới**: in `# L1 DIGEST: …` |

- `0/1` (mặc định) ⇒ **hành vi y nguyên**: một tiến trình, toàn bộ seed, thứ tự tăng dần, đầu ra
  **không** thêm dòng nào.
- `i/n` ⇒ mỗi tiến trình vẫn chạy **đủ** `SO_OP` op và kiểm **đủ** bất biến trên **mọi** seed của
  lô mình. Chỉ **việc** được chia.
- Lô rỗng (`i >= seeds`) ⇒ **panic rõ** — không được coi là xanh.
- Ngưỡng **tổng** dùng **số seed của lô** ⇒ **cùng** ngưỡng **trên mỗi seed**.

### 4.1 Digest — dấu vân tay trace

`VIFAST_L1_DIGEST=1` in `# L1 DIGEST: smp N shard i/n seeds X digest <16 hex>`.

Digest = **XOR 64-bit** của `vifast_sim::trace_hash(trace)` **từng seed** (kèm `seed`). XOR
**giao hoán + kết hợp** ⇒ `digest(hợp các lô) = XOR digest từng lô`. Vì vậy digest **không phụ
thuộc cách chia lô** — so hai lần chạy (hai bản mã, hai máy) là **một** phép so hex.

Digest **không** phải ngưỡng: không assert trên nó, không thay `assert_eq!(a, b)` của kiểm tất định.

---

## 5. Số đo

### 5.1 Một tiến trình (không đổi — không hồi quy)

| Mức | Trước task | Sau task |
|---|---:|---:|
| 1.000 seed × smp 1 | 28 s | **28 s** |
| 10.000 seed × smp 1 | 169 s | **169 s** |

### 5.2 Chia lô (11 worker trên 12 core)

| Mức | Một tiến trình | Chia lô | Nhanh hơn |
|---|---:|---:|---:|
| smoke (1.000 × smp 1) | 28 s | **7–8 s** | **3,5–4,0×** |
| medium (10.000 × smp 1,2,4,8) | 784 s tổng CPU (4 tiến trình song song: 364 s wall) | **248 s** | **1,5×** so với 4 tiến trình; **3,2×** so với tổng CPU |
| full (100.000 × smp 1,2,4,8) | 6.989 s tổng CPU (4 tiến trình song song: 2.088 s wall) | **[ƯL] 12–15 phút** | **[ƯL] 2,3–2,9×** |

> **Máy đo không rảnh** (load average 5,6–11,3; một `rustc` của agent khác chiếm 97 % CPU). Số
> wall là **cận trên**; phép so một-tiến-trình trước/sau được làm **xen kẽ** để nhiễu tác động
> đều hai bên (xem `kt242-l1-profile.md` §5.4 và §6.6).

- 6.989 s = 1.501,50 + 1.626,23 + 1.773,54 + 2.087,71 s — bốn lần đo **đã có** ở
  `v2/agent/JOURNAL/kt242-logs/kt242-l1-smp{1,2,4,8}.log`.
- **L1-full KHÔNG chạy lại trong task này** (chỉ đạo: không chạy L1-full nếu không cần). Wall
  chia lô của `full` là **[ƯL]**, chưa đo.
- Trần worker = `số core − 1` (AGENTS §9). Trên máy 12 core ⇒ 11.

---

## 6. Vì sao chia lô là tối ưu **duy nhất**

Hồ sơ đo ([`kt242-l1-profile.md`](kt242-l1-profile.md) §3): **~92 %** thời gian L1 là **kiểm bất
biến** — 53 % mô hình tự kiểm (`sched_after_op`, 11 checker sau mỗi public op) + 39 % driver kiểm
(`kiem()`, 7 checker sau mỗi op). Mô phỏng thật chỉ **~6 %**, định dạng trace **~1,6 %**.

⇒ Không có chỗ nào để tối ưu mà **không** nới ngưỡng. Chia lô **không** đổi một bất biến nào —
chỉ đổi **ai** chạy **seed nào**. Đó là lý do nó là tối ưu duy nhất được làm.

---

## 7. Giới hạn

1. Wall chia lô phụ thuộc **số core** và **tải máy**; số ở §5.2 đo trên máy **rảnh**.
2. `full` wall là **[ƯL]** — chưa đo trong task này.
3. Chia lô **không** làm nhanh hơn trên máy 1 core; khi đó `--jobs 1` cho hành vi ~ một tiến trình.
4. Script cần `python3` (bước gộp) và `cargo` (bước build) — cả hai đã là phụ thuộc của repo.
5. Script **không** thay `gate --fast`; nó là công cụ **đo**, không phải cổng nghiệm thu.
