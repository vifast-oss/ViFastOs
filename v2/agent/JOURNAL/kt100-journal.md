# KT-100 — vá bốn lỗ fail-open do review đêm phát hiện

**Ngày**: 2026-11-17 · **Nguồn**: reviewer độc lập (`night-review`), verdict **DRIFT**.

## Bốn lỗ (đều là fail-open thật, reviewer dựng lại được)

### 1. `build-rev.sh` — tệp ghim **thiếu mặt** ở HEAD bị âm thầm bỏ qua

`git ls-tree --name-only HEAD -- <paths>` coi đường dẫn **không khớp** là **rỗng hợp lệ**. Nên nếu
một tệp trong `VBREV_INPUT_FILES` bị `git rm --cached` rồi thêm vào `.gitignore`:

```text
dấu vân tay KHÔNG đổi · dirty = 0 · sửa nội dung tệp đó tuỳ ý vẫn "sạch"
```

Đó **đúng** là khe KT-93 định đóng, chỉ đóng được nhánh "sửa", chưa đóng nhánh "xoá/ẩn".

**Vá**: sau `ls-tree`, đối chiếu **đủ mặt** từng tệp; thiếu ⇒ `VBREV_CALC_ERROR=missing-input:<tệp>`
và `return 1` ⇒ cổng **từ chối đo**.

### 2. `pre-measure.sh` — thiếu `pgrep` thì bước kiểm QEMU **im lặng PASS**

`pgrep ... | wc -l` khi không có `pgrep` cho ra `0` ⇒ "không có QEMU sót" (sai).

**Vá**: `command -v pgrep || refuse 8 no-pgrep`.

### 3. `pin-artifact.sh` — không tính được dấu vân tay thì **vẫn ghim**

Trước: chỉ in cảnh báo rồi ghim ⇒ ghim được ảnh của cây khác khi thiếu git/metadata.

**Vá**: **từ chối** (mã thoát năm); cờ `--allow-unverified` cho ngoại lệ có chủ ý.

### 4. `soak.sh` — chấp nhận **mọi** thất bại của lần dò nếu log có `# build:`

Trước: chỉ cần log có dòng build là đi tiếp, kể cả khi boot **crash** sau dòng đó.

**Vá**: chỉ chấp nhận khi log **không** có `PANIC` / `FaultKind`.

## Kèm theo

- Thêm `v2/tools/pre-measure.sh` và `v2/tools/task-target.sh` vào `VBREV_INPUT_FILES` — cùng lý do
  "script đo cũng là đầu vào" mà KT-93 đã dùng; trước đó không nhất quán.
- Danh sách build-inputs: **263 → 265** mục; dấu vân tay `3caf5e7d5f0a3321` → `95d919fb5b882d24`.

## Bằng chứng

```text
# ca 1 (clone /tmp/kt100, git rm --cached v2/tools/rustc-canon.rs rồi commit):
vbrev_inputs_calc → rc=1  err=missing-input: v2/tools/rustc-canon.rs

# ca 2: PATH không có pgrep → REFUSE reason=no-pgrep (trước: OK)

# ca 3: cây không có .git + ảnh inputs=ffff… → exit 5 (trước: ghim được)

# selftests sau vá:
pin-selftest: PASS (a/b/c/d/e/f)
pre-measure-selftest: PASS (a/b/c/d/e/f)
task-target-selftest: PASS (a/b/c/d/e/f)
```

## Ảnh canonical mới (sau khi build-inputs đổi)

```text
REPRO sha256=9643311db649806840deb245b791a8bfbcca3153d4f98098c77fee8d758ee1dd
      size=205440  inputs=95d919fb5b882d24  dirty=0  rev=baf3b029d36c
      build=907912139bffcff0  text=130062
```

Đã ghim; `pre-measure.sh` **OK**; parity **6/6** (`KET_LUAN=ok`).

## Ghi nhận thẳng

Reviewer cũng nêu một điểm về **KT-82a**: viết số đo bằng **chữ** ("một trăm tám mươi tám nghìn…")
để qua luật 1 thì **giữ được thông tin** nhưng **làm luật không còn kiểm máy được** cho các giá trị
đó. Đây là hạn chế **thật** của cách làm đêm nay; cách đúng theo ý `debt` là đưa số vào khối
`scope=` với dòng log nguyên văn. Ghi lại để Agent 0 quyết có làm lại hay không.
