# KT-210 — đo lại sáu tiêu chí K6 trên ảnh `639ca6d1…` (2026-11-20)

**Vì sao đo lại**: KT-183 đổi `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` (seed `/docs/sub` +
`/docs/sub/deep.txt`) ⇒ dấu vân tay build-inputs `89f40ef16c97e6d5` → `43df126a4a6702ec`; ảnh K6
cũ `b090cfe8…` **không còn** là ảnh nguồn. Theo D2-166 §6, kết quả K6 phải ứng với **ảnh nguồn
cuối cùng được nghiệm thu**.

## Ảnh nguồn (A0 D2-165 §3: mỗi kết quả ghi commit + vân tay + toolchain + lệnh + log)

```text
commit       b85cfa8076bc   (HEAD lúc build; rev nhúng trong dòng REPRO của log build)
image        639ca6d16eb7c5effc7e56ffe93303d3dcdf0a405e598ad25ab7a2d23966dc81
size         213632 B
inputs       43df126a4a6702ec   dirty=0   build=b08c5c57cdbdb156
toolchain    1.99.1-aarch64-apple-darwin (rustc 1.99.1 58cf8f9c3, cargo 1.99.1 5f94df478)
lệnh        VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" bash v2/tools/repro-build.sh
            bash v2/tools/pin-artifact.sh <image>
            KT192_SHA=639ca6d1… bash v2/agent/JOURNAL/kt192-closure-measure.sh
log         v2/agent/JOURNAL/kt210-logs/kt210-measure.log
```

**Tái lập**: build lại từ cùng HEAD cho **đúng** sha `639ca6d1…` (đã kiểm hai lần độc lập: lead
và reviewer L42). Ảnh **không** nhúng `rev`; `rev=` chỉ là dòng chẩn đoán của `repro-build.sh`.

## Sáu tiêu chí — kết quả

| # | Tiêu chí | Kết quả | Dòng log |
|---|---|---|---|
| F1 | Shell in `shell>` ở smp 1/2/4 | `shell_prompt_lines=2` cả ba | §F1 |
| F2 | Parity 4 lượt + ghi/đọc lại sau reboot + persist | `parity smp=1 exit=0`, `smp=4 exit=0`; `f2_write_exit=0`, `f2_reboot_read_exit=0`, `f2_reboot_read_cat_ok=2`, đĩa **không đổi** giữa hai boot (`f2_dia_khong_doi_giua_hai_boot=1`), `persist_exit=0` | §F2 |
| F3 | Ảnh **tắt shell** phải đỏ | `f3_noshell_shell_prompt_lines=0`, `f3_khong_co_dong_product_shell=1`, `f3_parity_noshell_exit=5` | §F3 |
| F4 | Ma trận KTEST + soak 1000 boot | `matrix_exit=0`; `soak: ok=1000 fail=0 wall=297s` | §F4 |
| F5 | `.data + .bss ≤ 262144` | `file=213632 data+bss=224288 budget=262144` | §F5 |
| F6 | Ảnh sản phẩm không chứa mã harness | `harness_scan_exit=0`, product `hits=0`, ảnh âm `hits=25` | §F6 |

**Kết luận**: cả **6/6** tiêu chí **ĐẠT** trên ảnh `639ca6d1…`.

## Khác biệt so với lần đo trước (KT-192, ảnh `b090cfe8…`)

| Thành phần | KT-192 | KT-210 |
|---|---|---|
| ảnh | `b090cfe8…` | `639ca6d1…` |
| inputs | `89f40ef16c97e6d5` | `43df126a4a6702ec` |
| build id | `d6b1bf144b141cf5` | `b08c5c57cdbdb156` |
| soak | 1000/1000, 302 s | 1000/1000, 297 s |
| data+bss | 224288 | 224288 (**không đổi**) |

`data+bss` **không đổi** dù seed thêm hai mục: `/docs/sub` + `deep.txt` là **dữ liệu đĩa**, không
phải cấu trúc tĩnh trong `.data`/`.bss`.

## Quy tắc tái sử dụng bằng chứng (A0 D2-166 §6)

- **Được** tái sử dụng kết quả chạy ảnh khi **đầu vào tạo ảnh** và **môi trường liên quan** không đổi.
- **Phải chạy lại** các kiểm tra bị ảnh hưởng nếu **checker/harness đổi**, dù mã kernel không đổi.
- **Không** phải chạy lại soak chỉ vì sửa tài liệu không liên quan.
- Phân biệt ba thứ: (a) **ảnh sản phẩm** có đổi không; (b) **bộ công cụ đánh giá ảnh** có đổi không;
  (c) **gate trên HEAD hiện tại** đã chạy chưa.

## Ghi chú

- `soak` ghi `git=15365b5522e987c5e0feb08eb58f9fead68c33cf` (HEAD lúc soak bắt đầu — commit merge
  KT-209); ảnh vẫn là `639ca6d1…` build từ `b85cfa80`. Hai commit sau chỉ thêm tài liệu/checker,
  **không** chạm `v2/kernel`+`v2/lib` ⇒ ảnh không đổi.
- `infra: invalid=0 isolation_bad=0` — không có QEMU lạ, không vi phạm giới hạn 3 QEMU.
- Không retry (D2-07): soak chạy **một** lượt, 1000/1000.

## Tệp

| tệp | nội dung |
|---|---|
| `v2/agent/JOURNAL/kt210-logs/kt210-measure.log` | log nguyên văn 1521 dòng, 81 dòng `ok` |
| `v2/agent/JOURNAL/kt210-k6-remeasure.md` | tệp này |
