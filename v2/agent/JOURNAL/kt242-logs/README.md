# KT-242 — hồ sơ bằng chứng (lưu bền, không phụ thuộc /tmp)

Người ghi: lead (Agent 1) · Ngày: 2026-11-20 · Task: **KT-242**
Quyết định liên quan: A0 2026-11-20 (phiếu `PHIEU-GUI-AGENT0-2026-11-20-PERF.md`).

## 1. Danh tính

```text
commit perf       ffe77527  (nhánh task/kt242-perf, trên 47ee7b92)
commit toolchain  9d0b03fd  (trên ffe77527)
commit chọn rlib  00b82391  (trên 9d0b03fd)
merge perf        417e2a18  (main, đã push)
merge toolchain   50958dd1  (main, đã push)
fmt               45534b78  (main, đã push)  ← HEAD khi đóng hồ sơ
toolchain         Rust 1.100.0 (0ef1bc386 2026-11-09), ghim ở v2/rust-toolchain.toml
build-inputs      234 mục, sha256 54f3b59fc08f32215f97c6da44dbb86009383243b17ec3903cda746ec270431a
                  short 54f3b59fc08f3221, algo kt134-v4, dirty=0
ảnh sản phẩm      sha256 d915b977edbdae4661a3bb31e4aa7ec33f8e9d703bbba2c7f88069f200e541dc
                  191 288 B, `# build: algo=kt134-v4 inputs=54f3b59fc08f3221 dirty=0`
                  (build trong /tmp/kt242-tc; xem mục 4 về P20)
```

## 2. Lệnh đã chạy (tái lập)

```bash
# cổng trên nhánh perf
cd /tmp/kt242-perf && ./v2/bin/v2 gate --fast

# cổng trên nhánh toolchain
cd /tmp/kt242-tc && ./v2/bin/v2 gate --fast

# cổng trên cây chung sau merge
cd /Users/mps/dvchd/ViFastOs && ./v2/bin/v2 gate --fast

# K6 trên ảnh ứng viên (chạy trong /tmp/kt242-tc)
bash v2/test/boot/ktest.sh 1 matrix
bash v2/test/boot/persist.sh
bash v2/test/conformance/run.sh --boot --smp 2
bash v2/test/boot/soak.sh 1000 3
# L1 400k: bốn tiến trình song song, mỗi cấu hình một tiến trình
BIN=v2/target/debug/build/vifast-sim/cd7eafe15d99fe09/out/l1_k1c-cd7eafe15d99fe09
for smp in 1 2 4 8; do VIFAST_SMP=$smp VIFAST_SEEDS=100000 "$BIN" driver_smp --nocapture & done; wait

# đo cấu hình L0 (D)
/tmp/l0-bench.sh   # bản lưu: l0-bench.sh trong thư mục này
```

## 3. Kết quả

| Tệp | Nội dung | Kết quả |
|---|---|---|
| `kt242-gate-perf.log` | gate 53/53 trên `ffe77527` | PASS, 702,07 s |
| `kt242-gate-tc.log` | gate 53/53 trên `9d0b03fd` | PASS, 669,07 s |
| `kt242-gate-main.log` | gate 53/53 trên cây chung sau merge | PASS, 684,64 s |
| `ktest-matrix.log` | `ktest.sh 1 matrix` (7 ảnh × smp 1/2/4) | 21/21 boot sạch |
| `persist.log` | `persist.sh` | PASS, 7/7 pha |
| `conformance.log` | `conformance --boot --smp 2` | 61/61 dòng khớp |
| `soak.log` | `soak.sh 1000 3` | 1000/1000, 0 lỗi |
| `l1-smp{1,2,4,8}.log` | L1 400k (đo lại trên ảnh `d915b977…`) | 4/4 PASS (1506/1633/1782/2088 s) |
| `kt242-soak.log` | soak lần đo đầu (ảnh `d915b977…`) | 1000/1000, 0 lỗi |
| `kt242-l1-smp{1,2,4,8}.log` | L1 400k lần đo đầu | 4/4 PASS (1501/1626/1774/2088 s) |
| `l0-bench.sh` | script đo 6 cấu hình jobs/threads | 6/2 nhanh nhất (55,3 s) |

## 4. Nợ P20 — sha256 phụ thuộc thư mục build

Cùng `build-inputs=54f3b59fc08f3221`, cùng dòng `# build:`, năm lần build cho bốn sha:

| Thư mục | Đường build | sha256 |
|---|---|---|
| `/tmp/kt242-tc` | `ktest.sh` rev-gate | `d915b977…` |
| `/tmp/kt242-tc` | `cargo build` tay | `b77fff0d…` |
| `/tmp/kt242-check` | `cargo build` tay | `d0eeacab…` |
| `/tmp/kt242-tc` | `ktest.sh` rev-gate (đo lại) | `d915b977…` |
| `/Users/mps/dvchd/ViFastOs` | `ktest.sh` rev-gate | `746acb05…` |

Trong **cùng một thư mục** thì tái lập được. Danh tính bền là dòng `# build:` (giống nhau ở cả
năm lần), **không** phải sha256. Bằng chứng K6 neo vào `inputs=54f3b59fc08f3221` + `algo=kt134-v4`
+ `dirty=0`, đo trên ảnh `d915b977…`.

## 5. Checksum

Xem `SHA256SUMS` trong cùng thư mục (`shasum -a 256 -c SHA256SUMS`).
