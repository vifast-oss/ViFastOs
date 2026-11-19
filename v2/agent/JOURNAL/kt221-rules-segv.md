# KT-221 — `rules` không dùng process substitution (T-INFRA SIGSEGV bash 3.2)

**Ngày**: 2026-11-20 (đêm, Agent 1 giữ quyền A0)
**Loại**: hạ tầng cổng — **không** đổi ngưỡng, **không** đổi ngữ nghĩa phép kiểm.

## 1. Triệu chứng

`gate --fast` trên HEAD `af1ba3c9` **ĐỎ** ở bước `luật cấu trúc`:

```text scope=kt221-gate-segv.log
--- luật cấu trúc R1..R5
./v2/bin/v2: line 255: 80613 Segmentation fault: 11  bash "$V2_ROOT/bin/v2" rules --root "$V2_ROOT"
--- luật cấu trúc: FAIL
v2 check: 1 bước FAIL
```

Đây là **SIGSEGV**, khác SIGTRAP của KT-155/KT-207. Chạy **rời** `v2 rules` 10/10 lượt xanh;
chạy lại `v2 check` ngay sau đó **PASS** ⇒ **flake theo tải**, không theo cây.

## 2. Nguyên nhân nghi ngờ

`rules()` dùng **process substitution** `< <(find …)` cho **năm** vòng lặp, trong đó R4/R5 còn
**lồng** `while read` trong `while read`. Đây đúng là mẫu đã làm hỏng heap bash 3.2 ở KT-155
(SIGTRAP) và KT-207 (subshell thừa hưởng heap hỏng). SIGSEGV là biểu hiện khác của cùng lớp lỗi.

## 3. Bản vá

Tách `rules()` thành hai hàm:
- `rules()` — tạo thư mục tạm, gọi `_rules_impl`, dọn, trả mã thoát.
- `_rules_impl()` — **cùng** năm luật, nhưng `find`/`grep` ghi ra **tệp tạm** rồi đọc từ tệp:
  **không** còn `< <( … )`.

Cùng output, cùng mã thoát, cùng ngữ nghĩa.

## 4. Bằng chứng

```text scope=kt221-rules-before.txt
rules: PASS (/Users/mps/dvchd/ViFastOs/v2)
```

```text scope=kt221-rules-after.txt
rules: PASS (/Users/mps/dvchd/ViFastOs/v2)
```

`diff` hai tệp **rỗng** ⇒ output **giống hệt**.

- `bash v2/bin/v2 rules --root "$PWD/v2"` **10/10** lượt EXIT=0.
- `bash v2/test/rules/run.sh` ⇒ **6/6** case đúng kỳ vọng (R1..R5 + đối chứng dương) — bản vá
  **không** làm mất khả năng bắt vi phạm.
- `./v2/bin/v2 check` ⇒ **PASS** (`kt221-v2check-after.log`).

## 5. Giới hạn

- Đây vẫn là **workaround**: gốc là bash 3.2 của macOS. Cách chữa triệt để là bash ≥ 4.
- Chưa chứng minh được process substitution **là** nguyên nhân (chỉ là nghi phạm mạnh nhất và
  đã loại bỏ). Nếu SIGSEGV tái diễn, bước tiếp theo là chạy từng luật R1..R5 trong **tiến trình
  riêng**.

## 6. Tệp

| tệp | thay đổi |
|---|---|
| `v2/bin/v2` | `rules()` tách `_rules_impl()`; bỏ toàn bộ process substitution |
| `v2/agent/JOURNAL/kt221-logs/` | 4 log |
| `v2/agent/JOURNAL/kt221-rules-segv.md` | tệp này |
