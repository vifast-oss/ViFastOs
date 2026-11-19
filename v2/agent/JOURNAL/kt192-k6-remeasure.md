# KT-192 — đo lại sáu tiêu chí K6 trên ảnh `b090cfe8…` (2026-11-19)

**Vì sao đo lại**: `v2/lib/vifast-pte` đổi (chuyển `PageTable` về dùng chung cho Sv39/AArch64/x86_64)
⇒ dấu vân tay build-inputs `40f902a4…` → `89f40ef1…`; ảnh K6 cũ `2af101d1…` không còn khớp cây.

## Ảnh nguồn (A0 D2-165 §3: mỗi kết quả phải ghi commit + vân tay + toolchain + lệnh + log)

```text
commit      6f2d2c1dcc6a898e1f4134b2b5e4d13233cbf5e9   (rev trong ảnh)
HEAD lúc đo 8581c218a091d1ba0b79914e21a69a1ff59404b7   (chỉ thêm tài liệu sau 6f2d2c1d)
image       b090cfe8cfb1ceb8efb1c91b0c5dfe54d667ce5c229d72ce737b1918535048e9
size        213632 B
inputs      89f40ef16c97e6d5   dirty=0   build=d6b1bf144b141cf5
toolchain   1.99.1-aarch64-apple-darwin (RUSTUP_TOOLCHAIN ghim theo v2/rust-toolchain.toml)
lệnh        VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" bash v2/tools/repro-build.sh
            bash v2/tools/pin-artifact.sh <image>
            KT192_SHA=b090cfe8… bash v2/agent/JOURNAL/kt192-closure-measure.sh
log         v2/agent/JOURNAL/kt192-logs/kt192-measure.log
```

## Sáu tiêu chí — kết quả

| # | Tiêu chí | Kết quả | Dòng log |
|---|---|---|---|
| F1 | Shell in `shell>` ở smp 1/2/4 | `shell_prompt_lines=2` cả ba | `kt192-measure.log` §F1 |
| F2 | Parity 4 lượt + ghi/đọc lại sau reboot + persist | `parity smp=1 exit=0`, `smp=4 exit=0`; `f2_write_exit=0`, `f2_reboot_read_exit=0`, `f2_reboot_read_cat_ok=2`, đĩa **không đổi** giữa hai boot, `persist_exit=0` | §F2 |
| F3 | Ảnh **tắt shell** phải đỏ | `f3_noshell_shell_prompt_lines=0`, `f3_khong_co_dong_product_shell=1`, `f3_parity_noshell_exit=5` | §F3 |
| F4 | Ma trận KTEST + soak 1000 boot | `matrix_exit=0`; `soak: ok=1000 fail=0 wall=302s` | §F4 |
| F5 | `.data + .bss ≤ 262144` | `file=213632 data+bss=224288 budget=262144` | §F5 |
| F6 | Ảnh sản phẩm không chứa mã harness | `harness_scan_exit=0`, product `hits=0`, ảnh âm `hits=25` | §F6 |

**Kết luận**: cả **6/6** tiêu chí **ĐẠT** trên ảnh `b090cfe8…`.

## Quy tắc tái sử dụng bằng chứng (A0 D2-166 §6 — thay câu "hiệu lực tới khi kernel/lib đổi")

- **Được** tái sử dụng kết quả chạy ảnh khi **đầu vào tạo ảnh** và **môi trường liên quan** không đổi.
- **Phải chạy lại** các kiểm tra bị ảnh hưởng nếu **checker/harness đổi**, dù mã kernel không đổi.
- **Không** phải chạy lại soak chỉ vì sửa tài liệu không liên quan.
- Phân biệt ba thứ: (a) **ảnh sản phẩm** có đổi không; (b) **bộ công cụ đánh giá ảnh** có đổi không;
  (c) **gate trên HEAD hiện tại** đã chạy chưa.
- Kết thúc KT-194 phải chạy **gate phù hợp trên HEAD tích hợp cuối** — **không** gọi gate của
  `a90f1d77` là "gate đã chạy trên HEAD mới".

## Chuỗi bằng chứng: commit nhúng trong ảnh ≠ HEAD lúc đo

| Thành phần | Giá trị | Ý nghĩa |
|---|---|---|
| commit **nhúng trong ảnh** | `6f2d2c1d` | `rev=` do `repro-build.sh` ghi vào ảnh lúc build |
| **HEAD lúc đo** | `8581c218` | HEAD khi script đo chạy; sau `6f2d2c1d` chỉ có commit **tài liệu** |
| **dấu vân tay build-inputs** | `89f40ef16c97e6d5` | băm của `v2/kernel`+`v2/lib`+tệp build; **không** phụ thuộc tài liệu |
| **gate** | PASS trên `a90f1d77` | commit trước `6f2d2c1d`; `a90f1d77..HEAD` diff `v2/kernel`+`v2/lib` = **0 dòng** |

Cách tái lập: `git diff --stat a90f1d77..HEAD -- v2/kernel v2/lib` phải rỗng; `v2 status` phải in
đúng dấu vân tay `89f40ef16c97e6d5`; `REV-GATE: ACCEPT inputs=89f40ef16c97e6d5` trong log xác nhận
ảnh khớp cây tại thời điểm đo.

## Ghi chú

- `REV-GATE: ACCEPT inputs=89f40ef16c97e6d5 current_inputs=89f40ef16c97e6d5` — ảnh khớp cây tại thời điểm đo.
- Soak ghi `git=8581c218` (HEAD lúc soak bắt đầu); ảnh vẫn là `6f2d2c1d` — hai commit sau chỉ thêm tài liệu.
- **Không** dùng kết quả này cho cây khác: nếu `v2/kernel`/`v2/lib` đổi, phải đo lại phần bị ảnh hưởng (A0 D2-165 §3).
