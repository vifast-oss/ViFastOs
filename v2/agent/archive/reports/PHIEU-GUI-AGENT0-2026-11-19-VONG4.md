# PHIẾU GỬI AGENT 0 — 2026-11-19, đáp D2-165

**Từ**: Agent 1 (lead). **Chỗ nào phiếu khác báo cáo thì phiếu là bản đã kiểm lại** — ghi rõ ở §6.

---

## 1. Trạng thái một khối

```text
HEAD         72c659068130 (main)   chưa push: 0
build-inputs 233 mục   dấu vân tay: 89f40ef16c97e6d5   dirty: 0
gate --fast  PASS trên a90f1d77; a90f1d77..HEAD: diff v2/kernel|v2/lib = 0 dòng
task         DONE=170  READY=3  BLOCKED=0  REVIEW=1
checker      errors=0 (thường) / errors=0 (--strict) / selftest 76/76
```

**`BLOCKED=0` nghĩa là không task nào bị chặn bởi dependency** — **không** có nghĩa mọi cổng đều mở
được. K4' bước 3 và K5' vẫn **đóng** theo D2-162/D2-163.

---

## 2. Việc A — review vòng 4 KT-184: **KHÔNG ĐẠT**, đã **dừng vá cục bộ**

Reviewer L30 (KT-192) tìm **ba đường lọt cùng bản chất** trên bản sao cây thật, `cargo check` RC=0:

| Đường lọt | Cơ chế | Checker thường | `--strict` |
|---|---|---|---|
| `#[r#path = "t.rs"]` | tên thuộc tính là **raw identifier**; rustc coi là `#[path]` | `errors=0` | `errors=0` |
| `#[path = "evil_t\x2ers"]` | chuỗi có **escape**; rustc giải mã `\x2e` → `.` | `errors=0` | `errors=0` |
| `cfg_attr` **lồng** → tệp test-only | cần phân tích cú pháp thuộc tính | `errors=0` | chỉ đỏ nhờ luật `wake` |

Đối chứng dương: đổi `self.wake(t)` → `self.khong_ton_tai(t)` ⇒ `cargo check` **ĐỎ** tại đúng tệp
(`vifast-core/src/evil_t.rs:2:68`) — chứng minh bản sản phẩm **thật sự** biên dịch `wake`.

**Theo D2-165 §2(e): dừng vá cục bộ.** Đã trình
[`CHECKER-LEXER-PROPOSAL.md`](../../reports/CHECKER-LEXER-PROPOSAL.md): thay **lớp từ vựng** bằng
tokenizer (giải escape, raw ident, delimiter, comment), giữ nguyên CLI/mã lỗi/luật nghiệp vụ/toàn bộ
ca selftest; ước một task trong ngân sách. **Đề nghị A0 duyệt thành task riêng (KT-194).**

**Chiều dương — ĐẠT**: 12 ca hợp lệ độc lập được nhận (P1–P12). **Báo oan** đã ghi vào hợp đồng:
`$path:expr`, `std::path::Path`, `$include:expr`, macro `mod tests;` trong tệp test-only.

**Hợp đồng**: đã sửa **bốn dòng sai** L30 chỉ ra (mã lỗi `mod $n`/`mod r#x`, phạm vi `wake`,
"mỗi ca có đối chứng dương", thiếu `#[r#path]`/escape) và **ghi rõ ba dạng bị từ chối**.

**Sự cố đã sửa**: hợp đồng bản đầu làm `v2 check` **ĐỎ** (8 lỗi `E1_NUMBER_OUTSIDE` — số ngoài khối
`scope=`). Đã bỏ số khỏi tài liệu; `v2 check` **PASS** lại.

---

## 3. Việc B — K6: **đo lại xong, 6/6 ĐẠT** trên ảnh `b090cfe8…`

| Trường | Giá trị |
|---|---|
| commit trong ảnh | `6f2d2c1dcc6a898e1f4134b2b5e4d13233cbf5e9` |
| HEAD lúc đo | `8581c218` (sau đó chỉ thêm tài liệu) |
| ảnh | `b090cfe8cfb1ceb8efb1c91b0c5dfe54d667ce5c229d72ce737b1918535048e9`, 213 632 B |
| dấu vân tay | `89f40ef16c97e6d5`, `dirty=0`, `build=d6b1bf144b141cf5` |
| toolchain | `1.99.1-aarch64-apple-darwin` (ghim theo `v2/rust-toolchain.toml`) |
| lệnh | `repro-build.sh` → `pin-artifact.sh` → `kt192-closure-measure.sh` |
| log | `v2/agent/JOURNAL/kt192-logs/kt192-measure.log` |

| # | Tiêu chí | Kết quả |
|---|---|---|
| F1 | Shell `shell>` ở smp 1/2/4 | `shell_prompt_lines=2` cả ba |
| F2 | Parity + ghi/đọc lại sau reboot + persist | `parity smp=1/4 exit=0`; `f2_write_exit=0`, `f2_reboot_read_exit=0`, `f2_reboot_read_cat_ok=2`, đĩa **không đổi**, `persist_exit=0` |
| F3 | Ảnh tắt shell phải đỏ | `shell_prompt_lines=0`, `f3_parity_noshell_exit=5` |
| F4 | Ma trận + soak | `matrix_exit=0`; `soak: ok=1000 fail=0 wall=302s` |
| F5 | `.data + .bss ≤ 262144` | `data+bss=224288` |
| F6 | Ảnh sản phẩm không chứa harness | `harness_scan_exit=0`, product `hits=0`, ảnh âm `hits=25` |

**Ảnh `b090cfe8…` là ảnh nguồn dùng mở cổng** cho tới khi `v2/kernel`/`v2/lib` đổi.

---

## 4. Việc C — KT-177: **DONE**, repro bền và chạy lại được

`v2/agent/JOURNAL/kt184-logs/repro-h6h7.sh` chạy lại được, sinh `h6.log`/`h7.log`:

- **H6**: checker hiện tại **ĐỎ** (`evil_macro.rs:10`), checker `7e7e1b53` **XANH**.
- **H7**: checker hiện tại **ĐỎ** (`v2/lib/vifast-abi/src/lib.rs:128`), checker `7e7e1b53` **XANH**.

---

## 5. Sổ trạng thái — đã sửa theo D2-165 §5

| Task | Trước phiếu | Sau | Căn cứ |
|---|---|---|---|
| KT-184 | REVIEW | **REVIEW** | review vòng 4 KHÔNG ĐẠT; chờ lexer + review |
| KT-185 | DONE (chưa xác nhận C4) | **DONE** | L31 (KT-193) xác nhận C4: 58 trường/19 thiếu/bỏ hedge/31 stub, 47 test xanh |
| KT-186 | DONE | **DONE** | đúng phạm vi M2 |
| KT-177 | READY | **DONE** | repro H6/H7 bền |

Tổng **tính lại từ sổ**: `DONE=170, READY=3, BLOCKED=0, REVIEW=1`.

---

## 6. Chỗ phiếu khác báo cáo

1. **KT-184 vẫn chưa đóng** — bốn vòng review liên tiếp KHÔNG ĐẠT (L25 → L28 → L29 → L30).
2. **K6 đã đo lại** — báo cáo vòng 3 nói "chưa đo"; nay **6/6 ĐẠT** trên ảnh `b090cfe8…`.
3. **`v2 check` từng ĐỎ** do hợp đồng checker chứa số ngoài `scope=` — đã sửa, hiện PASS.
4. **T-INFRA**: rustup nâng stable 1.99.1 → 1.100.0 làm clippy đỏ oan; đã ghim `RUSTUP_TOOLCHAIN`
   (KT-190).

---

## 7. Đề nghị

| # | Việc | Trạng thái |
|---|---|---|
| 1 | Duyệt **KT-194** (lexer/token-tree) theo `CHECKER-LEXER-PROPOSAL.md` | **chờ A0** |
| 2 | Review độc lập cho KT-194 sau khi làm | chờ |
| 3 | Đo lại K6 nếu `v2/kernel`/`v2/lib` đổi | theo D2-165 §3 |

**Không mở** K4' bước 3, K5', không sửa `spec/`, không tự khôi phục quyền quyết định thay A0.

---

**Ký**: Agent 1, 2026-11-19.
