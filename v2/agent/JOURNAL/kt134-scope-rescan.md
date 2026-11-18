# KT-134 — QUÉT LẠI TOÀN BỘ PHẠM VI build-inputs (luật 2 của Agent 0)

**Ngày**: 2026-11-18 · **HEAD khi làm**: `f1077036` · **Người làm**: Agent 2 dòng OS

## Vì sao có tệp này

Agent 0, luật mới số 2:

> Mỗi lần đổi phạm vi/định nghĩa của một cổng (build-inputs, danh sách ghim, từ khoá loại trừ...)
> phải quét lại TOÀN BỘ phạm vi đó, không chỉ sửa đúng chỗ được chỉ ra. Ghi kết quả quét vào JOURNAL.

Lượt trước tôi chỉ loại đúng ba tệp Agent 0 chỉ ra, rồi sau đó mới phát hiện `v2/bin` cũng sai.
Đó là hệ quả của việc sửa đúng chỗ được chỉ mà không quét lại. Tệp này là lần quét đầy đủ.

## Phạm vi hiện tại

```text scope=kt134-logs/kt134-scope-scan.log
### QUET TOAN BO PHAM VI build-inputs (luat 2 cua Agent 0)
### lenh: git ls-files v2/kernel v2/lib | grep -E '/(tests|benches|examples)/'
```

`VBREV_INPUT_ROOTS="v2/kernel v2/lib"` · `VBREV_INPUT_FILES` gồm `v2/Cargo.toml`,
`v2/Cargo.lock`, `v2/rust-toolchain.toml`, `v2/tools/repro-build.sh`, `v2/tools/rustc-canon.rs`.

## Bốn câu hỏi của lần quét, và câu trả lời đo được

### 1. Trong hai gốc có tệp nào **không** phải mã build không?

Có. Ba mươi ba tệp nằm trong `tests/` của mười ba thư mục:

```text scope=kt134-logs/kt134-scope-scan.log
  10 v2/kernel/vifast-plat-common/tests
   4 v2/lib/vifast-objmem/tests
   4 v2/lib/vifast-fs/src/tests
   3 v2/lib/vifast-model/tests
   2 v2/lib/vifast-vfs/tests
   2 v2/lib/vifast-sync/tests
   2 v2/lib/vifast-fs/tests
   1 v2/lib/vifast-virtio/tests
   1 v2/lib/vifast-traceformat/tests
   1 v2/lib/vifast-ring/tests
   1 v2/lib/vifast-pte/tests
   1 v2/lib/vifast-objmem/tests/compile-fail
   1 v2/lib/vifast-alloc/tests
```

Đây là **test tích hợp / bench / example** — cargo **không bao giờ** biên dịch chúng vào binary
phát hành. Sửa một tệp trong đó trước đây **làm dấu vân tay đổi** (cùng lớp lỗi C1).

**Đã sửa**: thêm ba dòng loại `*/tests`, `*/benches`, `*/examples` vào `vbrev_input_allowed`.

### 2. Có tệp nào khác không phải `.rs/.toml/.lock/.ld/.S/.json` không?

Có hai, và **cả hai đều là đầu vào build thật**:

```text scope=kt134-logs/kt134-scope-scan.log
v2/lib/vifast-idl/idl/pager.vidl
v2/lib/vifast-idl/idl/ring.vidl
```

Hai tệp `.vidl` là nguồn sinh mã — `vifast-gen` đọc chúng để sinh `generated*.rs`. **Giữ nguyên.**

### 3. Có thư mục gói cargo nào nằm **ngoài** hai gốc không?

Không. Đo được: **không** thư mục nào.

```text scope=kt134-logs/kt134-scope-scan.log
### thu muc goi cargo nam ngoai hai goc:
       0
```

Điều này quan trọng vì nó dẫn tới một lỗi thứ hai (xem §4).

### 4. Lỗi thứ hai tìm được khi quét: `meta` làm việc loại `tests/` **vô hiệu**

`vbrev_inputs_calc` gộp ba nguồn: `tracked` (từng tệp), `pins` (năm tệp), và `meta` (thư mục gói
cargo). `meta` được băm bằng `git rev-parse HEAD:<thư-mục>` — trả **tree oid**, mà tree oid gộp
**mọi** tệp bên trong, kể cả `tests/`.

Nên chỉ cần `meta` có mặt là việc loại `*/tests/*` **không có tác dụng**: sửa một tệp test vẫn đổi
dấu vân tay. Ca tự kiểm (j) đỏ đúng vì lý do này trước khi sửa:

```text scope=kt134-logs/kt134-scope-scan.log
# vbrev-selftest (j) FAIL: sửa tệp tests/ mà dấu vân tay ĐỔI (9ea4df40781645e1 -> a9c52950ada6d4ce) — test tích hợp còn trong phạm vi
```

**Đã sửa**: chỉ giữ `meta` cho thư mục **nằm ngoài** các gốc — đó mới là mục đích ban đầu của nó
(bắt gói cargo ở nơi khác). Thư mục nằm trong gốc đã được `tracked` phủ **theo từng tệp**, nên
`meta` cho chúng là thừa và có hại.

## Cố ý **không** loại `src/`

Trong `src/` có ba tệp trông như test: `v2/kernel/vifast-core/src/capalloc_tests.rs`,
`v2/kernel/vifast-core/src/tls_tests.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest.rs`.

**Giữ nguyên**, vì:

- Một tệp trong `src/` **có thể** được `mod` không-test biên dịch — không có cách nào phân biệt
  bằng đường dẫn.
- `ktest.rs` là nguồn của **binary ảnh test**, mà ảnh test cũng được đo bằng cùng dấu vân tay này.
  Loại nó đi thì dấu vân tay mù với thay đổi của ảnh test.

Nguyên tắc: **thừa còn hơn thiếu**. Thừa ⇒ phải build lại (phiền). Thiếu ⇒ hai ảnh khác nhau cùng
một dấu vân tay (nguy hiểm).

## Kết quả sau khi sửa

```text scope=kt134-logs/kt134-scope-scan.log
# vbrev-selftest: PASS (a/b/c/d/e/f/g/h/i/j)
```

Đột biến chứng minh ca (j) không phải xanh rỗng — bỏ dòng loại `*/tests/*`:

```text scope=kt134-logs/kt134-scope-scan.log
# vbrev-selftest (j) FAIL: sửa tệp tests/ mà dấu vân tay ĐỔI (26191c6838c097a5 -> 55d8b3811e76e352) — test tích hợp còn trong phạm vi
```

## Ảnh và dấu vân tay

Đổi định nghĩa ⇒ **bump `VBREV_ALGO`** `kt134-v2` → `kt134-v3` (đúng quy trình: đổi định nghĩa thì
phải đổi nhãn phiên bản), cập nhật tệp ghim, rebuild.

| | Trước lượt này | Sau lượt này |
|---|---|---|
| `VBREV_ALGO` | `kt134-v2` | `kt134-v3` |
| Số mục trong dấu vân tay | 258 | 210 |
| Dấu vân tay | `097d1de49aef96cd` | `1324edbf33f315b8` |
| Ảnh | `464fe583…` | `af01e670…` |

## Việc còn lại của luật 2

Luật 2 áp cho **mọi** cổng có "phạm vi/định nghĩa", không chỉ build-inputs. Hai cổng còn lại:

- **Danh sách ghim** (`VBREV_INPUT_FILES`) — đã quét: năm tệp, cả năm đều là đầu vào thật
  (`Cargo.toml`/`Cargo.lock`/`rust-toolchain.toml` quyết định bao đóng; `repro-build.sh` đặt
  `RUSTFLAGS` remap+strip; `rustc-canon.rs` là `RUSTC_WRAPPER`). Không có tệp nào để loại.
- **Từ khoá loại trừ** (`QUALIFIERS` trong `check-scope-table.py`) — **chưa** quét lại toàn bộ.
  Việc này thuộc lượt sau (Agent 0 câu 3), và tôi ghi ở đây để không quên.
