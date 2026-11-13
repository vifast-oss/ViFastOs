<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R3-viscript — báo cáo vòng 3, dòng ViScript

Agent 2 · 2026-11-13 · `head` = `c427d64` là **head lúc thu số liệu** (QUEUE có đủ **27 hàng** 6 cột; mọi task
trước ST-24 đã `DONE`); commit chứa báo cáo là commit `[ST-24]` kế tiếp. Bản máy đọc:
[`R3-viscript.json`](R3-viscript.json) — **đúng 11 khoá** Khối E (`project = "viscript"`, `round = 3`).

## 1. Tóm tắt (5 dòng)

1. **VS1-khung chạy được đầu–cuối**: `vis run` (interp, mặc định) + **`vis run --vm`** trên bytecode **VBC1**
   (53 tag, version 1); hai đường **trùng ngữ nghĩa** — differential gốc **272/272** và **100.000 chương
   trình sinh tự động, 0 lệch**.
2. **An toàn bytecode**: `verify(&Module)` kiểm **từ byte stream** (13 biến thể lỗi), `max_stack` **tính lại
   độc lập**; `Vm::run` **verify trước**; fuzz đột biến **2,311,244 vòng / 0 vi phạm** (602.3 s).
3. **318 test workspace** (20 diag + 96 syntax + 18 fmt + 65 types +
   24 interp + 45 bytecode + 8 vm + 42 cli); **323 ca ngữ nghĩa/17 nhóm** (309 `.vis` + 14 `.vish`).
4. **Cache bytecode** theo hash closure (`$XDG_CACHE_HOME/vifast/vis/<hash>.vbc`, nạp **luôn** qua
   `load_verified`); khởi động ấm **8.51 ms** trung vị / **9.14 ms** p95 — **đạt** mục tiêu §19 `< 20 ms [ƯL]`.
5. **`./vis-dev gate --fast` PASS** (fmt, clippy `-D warnings`, R1..R4, self-test case âm, `report-check`,
   fixtures báo cáo, build, doc-examples, L0 test); **`./vis-dev nightly --fuzz-minutes 1` PASS**.

## 2. Bảng task vòng 3 (ST-18 → ST-24, + ST-20b)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-18 | DONE | `57d9e1a` | fast | chế độ lệnh `.vish`: `cmd_lower` hạ xuống HIR + chạy được; **265 test / 323 ca** |
| ST-19 | DONE | `750815b` | fast | định dạng **VBC1** (53 tag, version 1) + sinh mã HIR→bytecode; **284 test** |
| ST-20 | DONE | `65cb63b` | fast | **`vis-vm`**: VM chạy VBC1 + refactor ngữ nghĩa dùng chung (`builtin`/`value`/`interp`/`ops`); **289 test** |
| ST-20b | DONE | `516f668` | fast | differential interp ↔ VM **272/272, 0 lệch**, `KNOWN_LECH` **rỗng** (khoá hai chiều) |
| ST-21 | DONE | `70a5d38` | fast | **verifier** 13 biến thể + `need_stack` dùng chung codegen; fuzz **2,311,244 vòng / 0 vi phạm**; **316 test** |
| ST-22 | DONE | `f071166` | fast | differential **sinh tự động 100.000 chương trình, 0 lệch** (192.6 s, 519/s) |
| ST-23 | DONE | `213e691` | fast | cache bytecode (hash closure, `load_verified`) + đo ấm **8.51–9.92 ms** trung vị |
| ST-24 | READY → **báo cáo này** | — | fast | tệp này + `R3-viscript.json` (task đang viết; QUEUE@head ghi `READY`) |

**27 hàng QUEUE** khớp đúng `queue_at(c427d64)`. **`ST-20b` chỉ có trong bảng MD**: hàng QUEUE của nó có
**5 cột** (thiếu cột DoD) nên `report-check` không thấy — JSON **không** chứa `ST-20b` (nếu chứa sẽ bị từ chối
"task không có trong QUEUE"). Đề xuất sửa hàng QUEUE ở §6.

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm; chạy 2026-11-13, giờ UTC ghi trong ngoặc)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **318** = 20 diag + 96 syntax + 18 fmt + 65 types + 24 interp + 45 bytecode + 8 vm + 42 cli | `cargo test --workspace`; theo crate `cargo test -p <crate>` |
| ca ngữ nghĩa | **323 ca / 17 nhóm** (309 `.vis` + 14 `.vish`) | `find test/sem -name '*.vis' \| wc -l`; `find test/sem -name '*.vish' \| wc -l`; `ls -d test/sem/*/` (18 thư mục, `lệnh-vish` rỗng) |
| ca lỗi tĩnh `.err` | **51** | `find test/sem -name '*.err' \| wc -l` |
| differential gốc | **272/272 khớp, 0 lệch** (323 − 51 `.err`) | `cargo test -p vis-vm --test vs_interp` |
| differential sinh tự động | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=192.6 toc_do=519/s seed=3523158054` (0 lệch) | `VIS_DIFF_N=100000 cargo test -p vis-vm --test diff_100k -- --nocapture` (03:03:20Z) |
| fuzz verifier 10 phút | **2,311,244 vòng** · decode từ chối 1,497,793 · verify từ chối 66,994 · chạy được 746,457 · **vi phạm 0** · 602.3 s | `VBC_FUZZ_PHUT=10 cargo test -p vis-vm --test fuzz_mutate -- --nocapture` |
| VBC1 | **53 tag (0..52)**, version 1, hash nguồn 32 byte | `grep -c 'Instr::' crates/vis-bytecode/src/format.rs`; `Instr::tag` |
| verifier | **13 biến thể** `VerifyError`, **26** ca | `cargo test -p vis-bytecode --test verify` |
| khởi động ấm (220 mẫu/tệp) | co-ban **8.51** ms (p95 9.14) · fib **9.92** ms (p95 10.33) · or-hai **8.64** ms (p95 10.26); nguội 9.16/10.96/9.31 ms | `vis run --vm <tệp>` (cache ấm) vs `--no-cache`; log `JOURNAL/st23-warm.log` |
| cổng | **PASS** | `./vis-dev gate --fast` |
| nightly | **PASS** | `./vis-dev nightly --fuzz-minutes 1` |

**Số không tái lập được** (không ghi vào báo cáo): số token golden, số snapshot `render`, số ca tyck từng tệp —
chỉ tồn tại trong kỳ vọng của harness.

## 4. Quyết định vòng 3 (VD-52..VD-56, mỗi cái một dòng)

- **VD-52** — bytecode **VBC1** (magic `VBC1`, version 1, hash nguồn 32 byte, const pool + bảng hàm + code +
  span song song); `max_stack` chỉ là **ước lượng ghi ra**, verifier **kiểm lại độc lập** và có quyền từ chối;
  bytecode từ cache **luôn** qua verifier; `match` = test + nhảy.
- **VD-53** — chế độ lệnh `.vish` hạ qua `cmd_lower` xuống cùng HIR; sổ mã thêm **`06xx`**; 3 builtin lệnh.
- **VD-54** — hash nguồn = **FNV-1a×4** (khoá cache, **không** phải SHA-256 — consent vòng sau phải thay);
  `max_stack` là ước lượng; quy ước nhảy **thống nhất** `đích = chỉ_số_lệnh + offset`.
- **VD-55** — VM dùng **chung** ngữ nghĩa với interp (`vis_interp::builtin`/`value`/`ops`), `steps` khác đơn vị
  giữa hai đường (bước ngữ nghĩa vs số lệnh) — **không** so trong differential.
- **VD-56** — giữ `vis run` **mặc định INTERP**; cache bytecode áp cho đường `--vm` (`--no-cache` tắt); đổi
  mặc định sang VM là quyết định vòng sau, **có đo lại**.

## 5. Lỗ hổng đã biết (đầy đủ vòng 3)

1. **Hash nguồn FNV-1a×4, không phải SHA-256** (VD-54): consent §11.3 đòi SHA-256 ⇒ vòng sau phải thay
   (`sha2` đã có trong allowlist; cần bump version định dạng).
2. **`nfree` của closure = cả khung** (thừa, ST-22 **chưa** tối ưu) — verifier chỉ kiểm `used ≤ nfree`.
3. **Cache lợi ích 7–8 %**; chi phối là **khởi động tiến trình (~8–9 ms)**, không phải frontend (§19 vẫn đạt).
4. **`ref`/`await`/`cmd {}`** chưa có mã (VS1b).
5. **Biến thiên `.vish`** (variadic) và **cờ `--x=value`** chưa hỗ trợ (`VS0601`/`VS0603`).
6. **`.vish` không variadic**; `.vish` khi chạy chỉ đi qua `cmd_lower` (chưa phải lệnh đầy đủ).
7. **`steps` khác đơn vị** giữa interp (bước ngữ nghĩa) và VM (số lệnh) — không so trong differential.
8. **`match`/`if` làm đối số** chưa parse (giới hạn văn phạm §A.3).
9. **Nội suy không đệ quy** vào phần chữ: `"{{x}}"` ⇒ `{x}`.
10. **`\u{{…}}`** trong chuỗi chưa hỗ trợ.
11. **`char` escape** (`'\n'`) bị từ chối **VS0105** — **theo VD-30, không phải lỗi**.
12. **`None(1)`** chưa kiểm đối số; **map literal trùng khoá** giữ cả hai.
13. **Closure snapshot** (VD-51): `var` cập nhật sau khi tạo closure thì closure không thấy.
14. **`vis fmt` cho `.vish`** chưa có đặc tả ⇒ `Bail out!`.
15. **Generic/trait/module/`vis-std`/`vis-host-posix`** chưa có (VS1b).

## 6. Đối chiếu điều kiện chốt VS1 (§19) + đề xuất

| Điều kiện §19 (VS1) | Kết quả | Bằng chứng |
|---|---|---|
| differential interp ↔ VM trên **100.000** chương trình, 0 lệch | ✓ **100.000 chương trình / 0 lệch** (192,6 s) | `bin/vis-diff`; `JOURNAL/st22-diff-100k.log` |
| khởi động ấm **< 20 ms [ƯL]** | ✓ **8.51–9.92 ms** trung vị, p95 ≤ 10.33 ms | `JOURNAL/st23-warm.log` |
| bytecode từ cache **luôn** qua verifier | ✓ `load_verified` trên mọi đường nạp | `crates/vis-bytecode/src/verify.rs` |
| fuzz bytecode không panic | ✓ **2,311,244 vòng / 0 vi phạm** | `JOURNAL/st21-fuzz-vbc.log` |
| generic / trait / module / `vis-std` / `vis-host-posix` / `vis run/check/fmt/test` đầy đủ | **chưa** — VS1b | §5 |

**Đề xuất cho Agent 0** (tôi **không** sửa các tệp này):
1. `AGENTS.md` §8: cập nhật **vòng 3** đã chốt (`ST-18..ST-24`, `R3-viscript`, 318 test / 323 ca).
2. `viscript/AGENTS.md` §3/§5: dải quyết định **`VD-01..VD-56`**; thêm `./vis-dev nightly --fuzz-minutes N`,
   `vis cache --path|--clear`, `./bin/vis-diff` vào mục lệnh.
3. `viscript/agent/QUEUE.md`: hàng **`ST-20b`** thiếu **cột DoD** thứ 6 ⇒ `report-check` không thấy; thêm cột
   để báo cáo sau chứa được `ST-20b`; và lật **ST-24 DONE** sau khi push.
4. `docs/` (không thuộc quyền tôi): §19 VS1 nên ghi rõ "khởi động ấm" đo trên **đường `--vm`** (cache), vì
   `vis run` mặc định vẫn interp (VD-56).
