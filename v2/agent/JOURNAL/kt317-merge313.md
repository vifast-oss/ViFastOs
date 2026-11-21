# KT-317 — hợp nhất KT-313 (bất biến `diff >= 1`) vào KT-315 (**bốn** hart)

Trạng thái: **DONE** — nhánh `task/kt317-merge313`, base `main` @ `bce5cc14` (KT-315), 2026-11-22.
Sub-agent `merge313`. Quyết định: **D2-222** (mở KT-313/KT-315), **D2-221** (KT-310/KT-315),
**D2-216**, **D2-198** (uỷ quyền commit nhánh task), **D2-05** (test đỏ trước), **D2-07** (không
retry).

**Không** tuyên bố K5' đạt.

## 1. Bối cảnh

Hai task làm **cùng vùng mã** và xung đột **6 tệp** khi cherry-pick:

| Tệp | KT-315 (`bce5cc14`) | KT-313 (`0244e0bf`) |
|---|---|---|
| `v2/docs/k5-x86_64-boot.md` | §19 (bốn hart) | §19 (KT-313) |
| `v2/kernel/vifast-kbin-x86_64/src/sched.rs` | `MH_BSP_TICKS`, cổng giữ, `harts=` số đo | `MH_DIFF`, `diff=` |
| `v2/kernel/vifast-plat-x86_64/src/sched.rs` | `MH_HARTS=4`, `mh_hold`, `mh_ok` đòi `first[0] != first[i]` | `mh_ok(…, diff)`, bỏ `first` |
| `v2/kernel/vifast-plat-x86_64/tests/runqueue.rs` | 9 test, 4 hart | 7 test, 2 hart |
| `v2/test/boot/run-x86_64.sh` | mẫu 4 hart, 21 ca âm | `diff`, 22 ca âm + ca dương |
| `v2/test/boot/soak-x86_64.sh` | mẫu 4 hart, 38 ca | `diff`, 39 ca |

**Nguyên tắc hợp nhất**: lấy **KT-315 làm nền** (bốn hart, tác vụ `C`, cổng khởi động/giữ,
`harts=` là số đo) rồi **áp tinh thần KT-313**: bất biến của phép kiểm là `diff >= 1`, **không**
phải `first[0] != first[i]`.

## 2. Cách hợp nhất từng tệp

| Tệp | Cách hợp nhất |
|---|---|
| `vifast-plat-x86_64/src/sched.rs` | `mh_ok(rounds, picks, double, diff)` — **bỏ** tham số `first`, điều kiện 4 = `diff >= 1`; `mh_end(rounds, picks, diff)` — kết thúc bình thường **đòi** `diff >= 1` (trần cứng `MH_HARD_ROUNDS = 48` **giữ nguyên**); giữ `MH_HARTS=4`, `MH_TASKS=[A,B,U,C]`, `MH_MAX_ROUNDS=8`, `MH_HOLD_TICKS=4`, `mh_hold`, `mh_harts_picked`; doc đầu tệp thêm mục "KT-313" và cập nhật lý do cổng giữ (`diff` tất định thay vì `first`) |
| `vifast-kbin-x86_64/src/sched.rs` | thêm `MH_DIFF` (đếm cho **cả ba** hart phụ 1/2/3 — **KT-317**, không chỉ hart 1); đếm **trong** vùng tới hạn sau khi chọn `next`, đọc `MH_CURRENT[0]`; reset trong `mh_start`; `mh_end(…, MH_DIFF.load(…))` ở **cả hai** chỗ (đường giữ và đường thường); in `diff=<n>` trong `# sched: multi-hart`; **giữ** `first=[…]` để chẩn đoán; TAP 25 dùng `mh_ok(mh_rounds, &mh_picks, mh_double, mh_diff)` |
| `vifast-plat-x86_64/tests/runqueue.rs` | mô hình `Queue` thêm `diff`; `Hart::tick_locked(q, bsp_current)` đếm `diff` như ảnh; **10** test (giữ 9 của KT-315 + `kt313_first_collision_is_not_a_failure` **tái hiện** chuỗi R1..R8 với **bốn** hart: `picks=[1, 5, 1, 1] first=[a, a, u, c] diff=3` ⇒ `mh_ok` **XANH**, `diff=0` ⇒ **ĐỎ**) |
| `v2/test/boot/run-x86_64.sh` | mẫu `harts=4 … first=[4] diff=<n> double=<n>`; **bỏ** so `first`; thêm `diff >= 1` + `double == 0`; **giữ** 21 ca âm của KT-315 (cập nhật (t)/(u) có `diff=1`), **thêm** ca âm (v) `diff=0` ⇒ **22** ca âm; **thêm** ca dương hồi quy `picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1` ⇒ XANH |
| `v2/test/boot/soak-x86_64.sh` | như runner; log mẫu `diff=3`; ca âm `aj` đổi từ "first trùng" sang `diff=0`; **thêm** ca âm `al` `double=1`; **thêm** ca dương hồi quy; selftest **40** ca (2 dương + 38 âm) |
| `v2/docs/k5-x86_64-boot.md` | **gộp** KT-313 vào mục KT-315 (§19): §19.3 mới "KT-313 — soak 600 bắt bất biến sai" (đo được, cơ chế, sửa, bài học); §19.4 đo được (log hợp nhất); §19.5 đột biến (thêm (iii) `diff=0`); §19.6 rủi ro (cổng giữ không còn làm TAP đỏ); §18.4/§18.5 cập nhật mẫu log có `diff=` |

## 3. Test đỏ trước (D2-05)

Sửa `tests/runqueue.rs` **trước** khi sửa `plat/src/sched.rs` ⇒ lượt chạy đầu **không biên dịch
được** (19 lỗi `E0061`/`E0308`: `mh_ok`/`mh_end` chưa có tham số `diff`, `mh_ok` còn đòi `first`).
Đó là **đỏ đúng lý do**. Sau khi hiện thực: **10/10** test xanh.

## 4. Đo được (log QEMU nguyên văn, `-smp 4`)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=3 picked=b n=2
# sched: hart=1 picked=u n=3
# sched: hart=2 picked=c n=4
# sched: ticks=10 tasks=4
# sched: runs a=296300 b=228876 c=67113 switches=9 ab=1 ba=2 a_rsp=0x12e198 b_rsp=0x132198 u_rsp=0x13a1a0 c_rsp=0x136198
# sched: umode task ran n=11685558 ring=3 preempted=2 resumed=1
# sched: umode counts first=3665460 last=4989088 cs=0x1b kernel_stack=0x13a250
# sched: umode progress first=3665460 last=4989088 delta=1323628
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=10 picks=[3, 2, 2, 3] first=[a, u, c, b] diff=7 double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

| Phép đo | Kết quả |
|---|---|
| bốn hart `schedule()` | bốn dòng `# sched: hart=0|1|2|3 picked=…` |
| `diff` (bất biến mới) | **7** — cả ba hart phụ đều đóng góp (hart 1 chọn `u`, hart 2 `c`, hart 3 `b` trong khi BSP giữ `a`) |
| `first` (chẩn đoán) | `[a, u, c, b]` — **không** còn là điều kiện |
| mọi hart chọn ≥ 1 | `picks=[3, 2, 2, 3]`, `harts=4` |
| tác vụ thứ tư chạy thật | `c=67113` vòng |
| không double-pick | `double=0` |
| TAP | **27/27** xanh |

## 5. Ca âm + ca dương

```text
$ bash v2/test/boot/run-x86_64.sh
ok 2 - boot 1: TAP 27/27 + 5 dòng '# exc:' + marker 1 lần + halt sạch
ok 22 - ca âm (t): chỉ BSP schedule ⇒ ĐỎ (… hart: 1 2 3 …)
ok 23 - ca âm (u): chỉ 2 hart schedule ⇒ ĐỎ (… hart: 2 3 …)
ok 24 - ca âm (v): diff=0 ⇒ ĐỎ (# multi-hart diff=0 — hart phụ chưa chọn tác vụ **khác** tác vụ BSP đang chạy)
ok 25 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# ca âm: 22/22 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 20/20 kiểm sạch (boot=1, tests=27, exc=5)
# infra: qemu_own=0
```

```text
$ bash v2/test/boot/soak-x86_64.sh --selftest
ok 36 - ca âm (hart phụ chưa chọn tác vụ khác tác vụ BSP (diff=0)) ⇒ ĐỎ
ok 37 - ca âm (có double-pick (double=1)) ⇒ ĐỎ
ok 40 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# soak-x86_64 --selftest: 40/40 ca đúng như phải (2 dương + 38 âm)
# soak-x86_64 --selftest: PASS
```

## 6. Đột biến (đều ĐỎ)

| Đột biến | Kết quả |
|---|---|
| `--features two-hart-sched` | `# sched: multi-hart harts=2 rounds=50 picks=[22, 28, 0, 0] first=[a, b, ?, ?] diff=28 double=0` ⇒ `not ok 25` + `# TAP 25 FAIL — dừng (không in marker)` (ĐỎ, **không** treo) |
| `--features no-sched-lock` | `# sched: BAD double-pick hart=1 task=b n=8` + `# sched: BAD double-pick hart=3 task=b n=8` + `# sched: multi-hart harts=3 rounds=48 picks=[42, 3, 0, 3] first=[a, u, ?, b] diff=6 double=3` ⇒ `not ok 25`, không marker (ĐỎ) |
| ca âm `diff=0` (runner + soak) | `# multi-hart diff=0 — hart phụ chưa chọn tác vụ **khác** tác vụ BSP đang chạy` ⇒ ĐỎ |

Đột biến `two-hart-sched` cho thấy `diff` **không** thay thế `harts=4`: hai điều kiện là **hai
lớp lỗi khác nhau** (số hart tham gia vs. bằng chứng chọn khác tác vụ).

## 7. Cổng

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test runqueue
test result: ok. 10 passed; 0 failed

$ ./v2/bin/v2 check
exit=0
# step-time-total   118780 ms  TỔNG check
```

## 8. Còn gì chưa xong / rủi ro

1. **Chưa soak 600** trên ảnh hợp nhất: soak là **T2** và cần máy sạch (D2-116). Bằng chứng soak
   hiện có là của KT-313 (ảnh **hai** hart) — **không** dùng lại được cho ảnh **bốn** hart
   (AGENTS §6.3: đổi ảnh ⇒ đổi khoá đầu vào). Lead phải chạy `soak-x86_64.sh 600 3` trên máy sạch
   trước khi coi là bằng chứng cổng K5'.
2. `diff >= 1` chứng minh "**tồn tại** một lượt hart phụ chọn khác tác vụ BSP", **không** chứng
   minh "bốn hart chạy song song **lâu dài**" — đúng mức bằng chứng của pha (8–15 vòng).
3. **Cổng giữ** vẫn dựa trên tick BSP (PIT 100 Hz): host deschedule hart phụ > 40 ms thì BSP trả
   tác vụ trước. Từ KT-313 điều này **không** còn làm TAP 25 đỏ, nhưng làm `diff` chậm đạt hơn.
4. Lớp (B) của KT-313 (COM1 không khoá) là **KT-316** — đã ở `main` @ `9e58d163`; ảnh hợp nhất
   **kế thừa** bản sửa ấy (base `bce5cc14` có nó).
5. **Không** tuyên bố K5' đạt.

## 9. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng (`MH_HARD_ROUNDS = 48`, N=600, 20 s/boot, 3 luồng giữ nguyên), **không**
  `#[ignore]`/retry, **không** `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`.
- Commit bằng `VIFAST_ROLE=lead git commit` (uỷ quyền D2-198); `git add` **từng đường dẫn cụ
  thể** (không `git add -A`); cây sạch sau commit.
