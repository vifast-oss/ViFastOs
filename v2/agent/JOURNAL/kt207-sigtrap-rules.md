# KT-207 — `rules` chạy trong tiến trình bash **mới** (T-INFRA SIGTRAP)

**Ngày**: 2026-11-20 (đêm, Agent 1 giữ quyền A0)
**Loại**: hạ tầng cổng — **không** đổi ngưỡng, **không** đổi ngữ nghĩa phép kiểm.

## 1. Triệu chứng

Trong đêm, **hai reviewer độc lập** báo `./v2/bin/v2 check` đỏ ở bước `luật cấu trúc`:

```text scope=kt207-before-sigtrap-L42.log
./v2/bin/v2: line 255: 49547 Trace/BPT trap: 5       ( rules "$V2_ROOT" )
--- luật cấu trúc: FAIL
v2 check: 1 bước FAIL
```

- Reviewer L42 (KT-204): chạy 2 lần, **cả hai** chết cùng chỗ; 12/13 bước còn lại OK.
- Reviewer L41 (KT-203): cùng bước, cùng mã thoát.
- Lead: `./v2/bin/v2 rules` **5/5** lượt EXIT=0; `./v2/bin/v2 check` trong worktree **EXIT=0**.

⇒ **Flake theo tải**, không theo cây. Đây là `T-INFRA`, không phải lỗi mã của task nào.

## 2. Nguyên nhân

KT-155 đã chẩn đoán: bash 3.2 (macOS) hỏng heap (`BUG IN CLIENT OF LIBMALLOC: memory
corruption of free block`, SIGTRAP trong `malloc_type_realloc`). Workaround KT-155 bọc
`rules` trong **subshell**:

```bash
if ( rules "$V2_ROOT" ); then …
```

**Subshell `( … )` là `fork`** — tiến trình con **thừa hưởng nguyên trạng heap** của cha.
Khi `clippy`/`cargo` chạy trước đó đã làm hỏng heap, subshell vẫn mang heap hỏng ⇒ crash
**bên trong** `rules`. Vì vậy workaround KT-155 chỉ dời chỗ đỏ (từ `self-test luật` sang
`rules`), không chữa gốc.

## 3. Bản vá

Chạy `rules` trong **tiến trình bash mới** bằng chính lệnh con có sẵn `v2 rules --root`:

```bash
if bash "$V2_ROOT/bin/v2" rules --root "$V2_ROOT"; then …
```

Tiến trình mới **không** thừa hưởng heap hỏng. Cùng output, cùng mã thoát, cùng ngữ nghĩa
(`v2 rules --root` gọi đúng hàm `rules` với đúng `$V2_ROOT`, nên nhánh `EXCL` cho
`v2/test/fixtures` vẫn bật).

## 4. Bằng chứng

Trước (reviewer L42, dưới tải): `kt207-before-sigtrap-L42.log` — SIGTRAP ở `rules`.

Sau (lead, **dưới tải 3 sub-agent đang chạy**):

```text scope=kt207-after-check-1.log
--- luật cấu trúc: OK
v2 check: PASS (L1 chạy trong gate --fast)
```

```text scope=kt207-after-check-2.log
--- luật cấu trúc: OK
v2 check: PASS (L1 chạy trong gate --fast)
```

```text scope=kt207-after-check-3.log
--- luật cấu trúc: OK
v2 check: PASS (L1 chạy trong gate --fast)
```

```text scope=kt207-after-rules-fresh.log
rules: PASS (/Users/mps/dvchd/ViFastOs/v2)
```

Ba lượt `v2 check` liên tiếp **3/3 PASS**, bước `luật cấu trúc` **3/3 OK**.

## 5. Giới hạn

- Đây là **workaround hạ tầng**, không phải chữa gốc. Gốc là bash 3.2 của macOS; cách chữa
  triệt để là dùng bash ≥ 4 cho `v2/bin/v2` (đổi shebang hoặc yêu cầu `bash` mới) — **ngoài
  phạm vi** đêm nay, ghi thành nợ.
- Nếu `rules` **tự nó** làm hỏng heap (không phải thừa hưởng), bản vá này không cứu được.
  Chưa quan sát được trường hợp đó: 5/5 lượt `v2 rules` độc lập và 3/3 lượt qua tiến trình
  mới đều xanh.
- Không sửa `v2/test/rules/run.sh` (bước `self-test luật`) — bước đó vẫn có thể flake theo
  cùng cơ chế; nó chạy **sau** `rules` trong cùng tiến trình cha. Nếu flake lại ở đó, áp dụng
  cùng cách: chạy trong tiến trình mới.

## 6. Tệp đã sửa

| tệp | thay đổi |
|---|---|
| `v2/bin/v2` | `do_check`: `( rules "$V2_ROOT" )` → `bash "$V2_ROOT/bin/v2" rules --root "$V2_ROOT"`; cập nhật chú thích KT-155 → KT-207 |
| `v2/agent/JOURNAL/kt207-logs/` | 5 log (1 trước, 4 sau) |
| `v2/agent/JOURNAL/kt207-sigtrap-rules.md` | tệp này |
