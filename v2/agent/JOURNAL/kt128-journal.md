# KT-128 — Ca hồi quy **ÂM** cho cổng build-inputs (Q3 bổ sung của Agent 0)

**Ngày**: 2026-11-18 · **Nguồn**: quyết định Agent 0 ngày 18/11, **Q3**:
"thêm ca hồi quy **ÂM** (sửa 1 byte trong mã kernel/crate ⇒ dấu vân tay PHẢI đổi **và harness PHẢI
từ chối đo**). Hiện chỉ có ca dương, chưa đủ."
**Phạm vi ghi**: `v2/test/boot/build-rev.sh`, `v2/bin/v2`, `v2/agent/**`. **Không** sửa mã kernel,
không sửa `spec/`, không đổi ngưỡng.

---

## 1. Phát hiện trước khi làm: selftest đang **ĐỎ** ở HEAD, và **không cổng nào chạy nó**

Chạy `bash v2/test/boot/build-rev.sh --selftest` trên **bản HEAD `f4d1dd5f` chưa sửa**:

```text
# vbrev-selftest (c) FAIL: commit kernel mà dấu vân tay không đổi
# vbrev-selftest (d) FAIL: crate ngoài v2/kernel đổi mà dấu vân tay không đổi
# vbrev-selftest (e) FAIL: rust-toolchain.toml đổi mà dấu vân tay không đổi
# vbrev-selftest: FAIL
```

**Nguyên nhân** (truy bằng `bash -x`): `vbrev_inputs_calc` trả
`missing-input: v2/test/boot/build-rev.sh v2/tools/repro-build.sh v2/tools/rustc-canon.rs
v2/tools/pre-measure.sh v2/tools/task-target.sh`.

Danh sách tệp ghim `VBREV_INPUT_FILES` **lớn dần** (KT-93 thêm 3 tệp, KT-95/98 thêm 2) nhưng repo
mini trong selftest **không** được cập nhật ⇒ `cargo metadata` chạy được mà bước kiểm "đủ mặt tệp
ghim" (KT-100) báo thiếu ⇒ mọi dấu vân tay rỗng ⇒ (c)(d)(e) đỏ.

**Vì sao không ai thấy**: `build-rev.sh --selftest` **không** được `v2 check` / `gate --fast` gọi.
A1-191 đã sửa selftest cho danh sách **lúc đó**, nhưng hai task sau thêm tệp ghim mà không ai chạy
lại. Đây đúng dạng "test có mà không chạy" — không phải xanh giả, mà là **đỏ im lặng**.

## 2. Vá

**(a) Repo mini sinh tệp ghim từ chính biến** — không lệch lần nữa:

```bash
for _f in $VBREV_INPUT_FILES; do
    [ -f "$_f" ] && continue
    mkdir -p "$(dirname "$_f")"
    printf '# pinned\n' >"$_f"
done
```

**(b) Thêm ca (f) — ÂM, đúng yêu cầu Agent 0**: sửa **một byte** trong mã kernel rồi commit ⇒
(i) dấu vân tay PHẢI đổi, **và** (ii) cổng PHẢI `REFUSE reason=inputs-mismatch` khi log/ảnh còn mang
dấu vân tay **cũ**.

**(c) Thêm ca (g) — ĐỐI CHỨNG DƯƠNG của (f)**: cùng cây đó, log/ảnh mang dấu vân tay **mới** ⇒ cổng
PHẢI `ACCEPT`. Thiếu (g) thì một cổng hỏng kiểu "luôn REFUSE" vẫn qua được (f).

**(d) Nối vào cổng**: `v2 check` gọi `build-rev.sh --selftest` mỗi lần chạy.

## 3. Bằng chứng

**Selftest sau khi vá**:

```text
$ bash v2/test/boot/build-rev.sh --selftest
# vbrev-selftest: PASS (a/b/c/d/e/f/g)
```

**Đột biến phía mã — chứng minh (f) bắt được cổng hỏng**: bỏ bước so dấu vân tay trong `vbrev_gate`
(`if [ "$inputs" != "$cur" ]; then` → `if false; then`):

```text
# vbrev-selftest (f) FAIL: cổng KHÔNG từ chối ảnh cũ sau khi kernel đổi —
#   [REV-GATE: ACCEPT inputs=517e69323be3151e current_inputs=8f9f4cbb91db0dd7 head_rev=f2b030f2960a count=17 …]
# vbrev-selftest: FAIL
```

Đột biến đã **revert**; selftest về `PASS (a/b/c/d/e/f/g)`.

**Cổng**:

```text
$ ./v2/bin/v2 check
--- selftest build-inputs (KT-128: a..g)
--- selftest build-inputs (KT-128: a..g): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

## 4. Ghi chú cho Agent 0

- Ca âm Agent 0 yêu cầu **đã có**, và có **đối chứng dương** đi kèm để nó không thể "xanh vì cổng
  luôn từ chối".
- Phát hiện thêm: selftest **đã đỏ từ trước** (không do lần này). Nay nó chạy trong `v2 check` nên
  không thể đỏ im lặng lần nữa.
- Không đổi ngưỡng, không đổi định nghĩa build-inputs; chỉ thêm ca kiểm + nối vào cổng.
