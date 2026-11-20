# BÁO CÁO CUỐI TUẦN — ViFastOs tự chủ (KT-242)

**Người lập**: lead (Agent 1). **Thời điểm**: 2026-11-20 21:15 JST.
**Ủy quyền**: quyết định Agent 0 ngày 2026-11-20 (chạy tự chủ đến 09:00 JST thứ Hai 23/11/2026,
tối đa 4 sub-agent, ba tầng kiểm).
**Kế hoạch**: `KE-HOACH-CUOI-TUAN-2026-11-20.md`.

---

## 0. Tóm tắt 6 dòng

1. **HEAD `e12477fb`**, đã push. `build-inputs` **`54f3b59fc08f3221`** (234 mục, `dirty=0`) —
   **KHÔNG ĐỔI** so với trước cuối tuần ⇒ **bằng chứng K6 trên ảnh `d915b977…` vẫn hiệu lực**,
   không phải đo lại.
2. **KT-241 nghiệm thu xong**: ma trận tấn công **28/28 PASS** (thêm ô D6), `QUEUE` → **DONE**.
3. **Luật wake: tìm thêm 6 đường lọt (N1–N6)**, đã vá cả 6; **KT-184/KT-194 vẫn CHƯA ĐÓNG** —
   cần một vòng review độc lập trên HEAD mới.
4. **Ba tầng kiểm đã có**: `gate --dev` (T1) và `gate --fast` (T2); T1 **không** được gọi là full gate.
5. **Vòng phát triển nhanh hơn 30–64 %**: T1 433 → **272 s**, T2 685 → **477 s**, `v2 check` 200 → **72 s**.
6. **L1**: 92 % thời gian là kiểm bất biến (không phải mô phỏng); chia lô seed cho **3,5–4×**,
   digest chứng minh **không đổi trace** (16/16 ô).

---

## 1. Thay đổi sản phẩm đã hoàn tất

| # | Thay đổi | Commit | Bằng chứng |
|---|---|---|---|
| 1 | **KT-241 nghiệm thu**: ma trận tấn công 28/28, sửa 3 kỳ vọng sai của manifest, thêm ô **D6** (`#[path]` ra ngoài cây quét) | `580e1217` → merge `77d90199` | `JOURNAL/kt241-attack/results/current.tsv` |
| 2 | **Vá 4 lệch hợp đồng luật wake** (P0-1..P0-3, P1-1) — khép **N1–N6** | `c0b48f3b` → merge `eb0546eb` | selftest lớp tĩnh **384 → 406**, dep-info **62 → 65** |
| 3 | **L1 chia lô seed** (`VIFAST_SHARD`) + **digest trace** (`VIFAST_L1_DIGEST`) + **ba mức** `l1-tier.sh` | `970d0a56` → merge `4e4656e2` | digest trùng **16/16** ô; XOR lô = digest một tiến trình |
| 4 | **Làn song song** cho checker tĩnh + bảng wall/CPU/critical-path | `98d429c5` → merge `e12477fb` | T1 −37 %, T2 −30 %, `check` −64 % |
| 5 | **Ba tầng kiểm** `gate --dev` (T1) / `gate --fast` (T2) | `21cc75cd` | T1 in rõ "KHÔNG phải full gate"; 8 bước bỏ đều có dòng |
| 6 | **Thiết kế tái dùng bằng chứng** (khóa đầu vào, fail-closed) | `6303e0b8` | `JOURNAL/kt242-evidence-reuse.md` |
| 7 | **K4' bước 3 design-only** + **K5' kế hoạch/harness** | `970d0a56` | `JOURNAL/kt242-k4-buoc3-design.md`, `kt242-k5-plan.md` |

**Không** thay đổi: `spec/`, ABI, ngưỡng sản phẩm, profile ảnh, `docs/QUYET-DINH-sau-review.md`,
`docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`.

---

## 2. Ba tầng kiểm — số đo

| Tầng | Lệnh | Trước cuối tuần | Sau | Ghi chú |
|---|---|---:|---:|---|
| **T0** | agent tự chạy `cargo test -p …` | — | — | không phải lệnh cổng |
| **T1** | `./v2/bin/v2 gate --dev` | 433,4 s (46 bước) | **272,0 s** (48 bước + 8 bỏ qua) | **−37 %** |
| **T2** | `./v2/bin/v2 gate --fast` | 684,6 s (53 bước) | **477,2 s** (54 bước) | **−30 %** |
| — | `./v2/bin/v2 check` | 199,5 s | **72,5 s** | **−64 %** |

**CPU gần như không đổi** (T2: 901 → 648 s user+sys; T1: 650 → 654 s) ⇒ cùng khối lượng việc,
chỉ đổi **đồng thời**. Critical path T2: 680,6 s tuần tự → **430 s** (`max(main, làn nền) + sau`).

**T1 không phải full gate.** Output in đúng câu:
`v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')`.
Mỗi bước bị bỏ in `--- <tên>: BỎ QUA (T1 — chỉ chạy ở T2)` — **không** bỏ âm thầm.

**T2 bắt buộc ngay** (không trì hoãn) khi chạm: SMP/memory ordering/vòng đời đối tượng; page
table/quyền/revoke/allocator; boot/linker/layout ảnh/toolchain; persist/toàn vẹn dữ liệu; bản sửa
lỗi chỉ tái hiện dưới stress.

---

## 3. Số lần kiểm nặng (L1-full / soak) và lý do

| Phép đo | Số lần trong cuối tuần | Lý do |
|---|---:|---|
| **L1-full (400k)** | **0** | `build-inputs` không đổi ⇒ bằng chứng `d915b977…` (4/4 PASS) **còn hiệu lực**; chạy lại chỉ để tăng số lần PASS — A0 cấm |
| **soak 1.000 boot** | **0** | cùng lý do; ảnh không đổi |
| **T2 (full gate)** | **2** | (a) trên cây tích hợp sau merge; (b) đo trước/sau của luồng P3 |
| **L1-medium (10k)** | 2 (đo profile + kiểm digest) | chứng minh chia lô không đổi trace |
| **L1-smoke (1k)** | mỗi lần T1/T2 | mặc định của cổng |

**Bằng chứng tái dùng + khóa đầu vào**:
- K6 (ktest 21/21, persist, conformance 61/61, soak 1000/1000, L1 400k 4/4) — khóa
  `commit 00b82391` + `build-inputs 54f3b59fc08f3221` + `toolchain 1.100.0` + ảnh `d915b977…`.
  **Vẫn hiệu lực** vì `build-inputs` không đổi suốt cuối tuần (đã kiểm lại lúc đóng hồ sơ).
- Gate T2 `c6127fa0` (53/53, 684,6 s) — **thay** bằng gate T2 `e12477fb` (54/54, 477,2 s) vì
  runner đổi.

---

## 4. Phát hiện đáng chú ý

### 4.1 Sáu đường lọt mới của luật wake (N1–N6) — đã vá

| # | Đường lọt | Trước | Sau |
|---|---|---|---|
| **N1** | **"cửa thứ bảy"**: `impl`/`fn` **lồng trong thân** một cửa gọi thẳng `write_state(t, Ready)` | cả 3 lớp **XANH**, `cargo` RC=0 | **ĐỎ** cả 3 lớp |
| **N2** | như N1 nhưng `fn` lồng | XANH | **ĐỎ** |
| **N3** | `[[bin]] path` **ngoài `src/`** | lớp 1 XANH | **ĐỎ** cả 3 lớp |
| **N4** | tệp `.RS` (hoa) | lớp 1 XANH | **ĐỎ** cả 3 lớp |
| **N5** | `#[cfg(test)]` **cùng dòng** với mục sản phẩm | 3 luật bị lách | **ĐỎ** |
| **N6** | `include!("\0")` | checker **crash** | **ĐỎ** `E_STRICT_WAKE_INCLUDE` |

Gốc N1/N2: `_check_state_write` quy `write_state` cho hàm **ngoài** (`find_functions` nhảy qua hàm
lồng). Vá bằng `_fn_body_groups` + `_innermost_fn_tree` (token-tree, khoảng nhỏ nhất thắng).

**KT-184 và KT-194 vẫn CHƯA ĐÓNG** — bản vá khép 6 đường lọt nhưng cần **một vòng review độc lập
trên HEAD `e12477fb`** trước khi đóng. `WB-19` (cửa thứ bảy) đã ghi **OPEN** trong
`BYPASS-REGISTRY.md`.

### 4.2 L1: 92 % thời gian là kiểm bất biến, không phải mô phỏng

| Pha | % tổng |
|---|---:|
| Mô hình tự kiểm (`sched_after_op`, 11 checker × 165.244 lần) | **53,0 %** |
| Driver kiểm (`kiem()`, 7 checker) | **38,6 %** |
| Định dạng trace | 1,6 % |
| **Mô phỏng thật** | **~6,4 %** |

⇒ Tối ưu đúng là **chia lô** (không phải viết lại sim). Bốn checker trùng nhau giữa mô hình và
driver nhưng **không** thay thế nhau (mô hình dùng `debug_assert!` — biến mất ở release) ⇒ **không
gỡ bên nào**.

### 4.3 Nợ P20 (đã báo trước, vẫn mở)

sha256 ảnh **phụ thuộc thư mục build** (5 lần build → 4 sha). Danh tính bền là dòng
`# build: algo=… inputs=… dirty=…`. Đã ghi trong `JOURNAL/kt242-logs/README.md` §4.

---

## 5. Trạng thái cuối

```text
HEAD          e12477fb  (main, đã push)
BUILD-INPUTS  234 mục, sha256 54f3b59fc08f3221, dirty=0   ← KHÔNG ĐỔI
TOOLCHAIN     Rust 1.100.0
ẢNH SẢN PHẨM  d915b977… (191 288 B, inputs=54f3b59fc08f3221) — vẫn hiệu lực
K6            đo lại đủ 5 hạng mục trên ảnh đó; C1..C8 phủ; N1..N7 + S1 giữ mở
CỔNG          T2 PASS 54/54 (477,2 s) trên e12477fb; T1 PASS (272,0 s)
TASK          KT-241 DONE; KT-184/KT-194 REVIEW; WB-19 OPEN
```

**Task mở / blocker cho A0**:

| # | Việc | Cần gì |
|---|---|---|
| 1 | **KT-184/KT-194** | một vòng review độc lập trên `e12477fb` (bản vá đã khép N1–N6) |
| 2 | **WB-19** | thêm ô ma trận riêng cho "cửa thứ bảy" khi chốt |
| 3 | **DEBT-KT227-1** (`v2/lib` ngoài luật token `wake`) | quyết định có mở rộng phạm vi không (hiện **latent**) |
| 4 | **L2** (ba mẫu phạm vi vênh) | không có đường lọt sống; bản vá **giảm** vênh, chưa gộp |
| 5 | **K4' bước 3** | 6 câu hỏi trong `kt242-k4-buoc3-design.md` §7 (đường boot aarch64, `[calling.aarch64]`, trait nửa cao/physmap, phạm vi) |
| 6 | **K5'** | mở kèm ngân sách/phạm vi; `spec/kernel.toml` chưa tồn tại |
| 7 | **P20** | nếu muốn dùng sha ảnh làm danh tính |

---

## 6. Kết quả nào là T1, kết quả nào là full gate/K6

| Kết quả | Loại | Ghi chú |
|---|---|---|
| `gate --dev` PASS 272,0 s trên `e12477fb` | **T1** | **không** phải full gate |
| `gate --fast` PASS 54/54, 477,2 s trên `e12477fb` | **T2 / full gate** | |
| Ma trận tấn công 28/28 | kiểm tác vụ (T0) | chạy trên worktree riêng |
| K6 (ktest/persist/conformance/soak/L1 400k) | **K6** | đo trên ảnh `d915b977…`, **tái dùng** (inputs không đổi) |
| `v2 check` 72,5 s | T1 (thành phần) | |

---

## 7. Thời gian phân bổ (ước lượng theo nhật ký phiên)

| Hoạt động | Ước lượng |
|---|---|
| Phát triển (KT-241, vá checker, L1, làn, tài liệu) | ~55 % |
| Kiểm nhanh (T1, `v2 check`, selftest từng bước) | ~30 % |
| Kiểm nặng (T2 ×2, L1-medium ×2) | ~15 % |

Không có đợt soak/L1-full nào chạy chỉ để "tăng số lần PASS".

---

## 8. Việc **không** làm (đúng ủy quyền)

- Không mở ABI/spec mới, không đổi luật kernel chưa có quyết định.
- Không nới ngưỡng, không `#[ignore]`, không tắt ca, không sửa bài kiểm để lấy xanh.
- Không mở K4'/K5' triển khai (chỉ design/plan).
- Không E, không nextest, không phát hành v2.0.
- Không force-push, không bỏ branch protection.
