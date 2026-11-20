# KT-252 — vòng **tấn công mới** cho luật `wake` (KT-184/KT-194)

**Ngày**: 2026-11-21 · **Task**: KT-252 · **Nhánh**: `task/kt243-infra` · **Worktree**:
`/tmp/kt243-s3` · **Base**: `main` @ `43a054e3` · **Điều kiện đóng**: D2-190 (7 mục).
**Tiền đề**: `JOURNAL/kt242-wake-review.md` (6 đường lọt N1–N6), `kt242-wake-patch.md` (vá),
`kt242-wake-verify.md` (lead xác minh), `BYPASS-REGISTRY.md` (WB-01..WB-19).

**Ràng buộc đã giữ**: **không** sửa mã sản phẩm (`v2/kernel/**`, `v2/sim/**`, `v2/lib/**`),
**không** sửa `spec/`, **không** sửa `STATE/QUEUE/DECISIONS.md`, **không** nới ngưỡng, **không**
tắt ca. Mọi biến thể chèn vào cây đều **khôi phục** bằng `trap` (chỉ `v2/kernel`/`v2/sim`/`v2/lib`).

---

## 1. Cách tấn công: **theo lớp**, không replay danh sách

Theo D2-190 §5, vòng này **không** chọn mẫu từ N1–N6/WB-01..19. Lập **ma trận 6 nhóm**, mỗi nhóm
≥ 3 biến thể; mỗi biến thể phải **`cargo check --target riscv64gc-unknown-none-elf` RC=0** mới
được tính là đường lọt, và phải chạy **cả ba** lượt kiểm (`check-wake-paths.py`,
`--strict`, `check-wake-depinfo.py`).

| Nhóm | Bề mặt | Số biến thể |
|---|---|---|
| **G1** | alias/UFCS/fn-pointer của `transition_tcb`, `write_state`, `wake` | 4 |
| **G2** | helper liên hàm / closure / `const` / `static` / hàm trả `TcbState::Ready` | 3 |
| **G3** | macro: `macro_rules!` lồng, `include!` trong lời gọi macro, `#[path]` trong `cfg_attr`, macro che tên an toàn | 3 |
| **G4** | `#[cfg]`/feature: module chỉ bật dưới feature, `cfg(test)` trên item/nhánh `match`, `cfg_attr` | 3 |
| **G5** | nguồn sinh: `build.rs` + `$OUT_DIR`, crate ngoài ba gốc, đuôi lạ, tên có dấu cách, symlink | 3 |
| **G6** | đường dẫn: `src/target/`, `#[path]` non-mod-rs, `include!` ra ngoài cây | 3 |
| | **Tổng** | **19** |

Harness: `v2/agent/JOURNAL/kt252-attack/run-attack.sh <nhóm> <tên> <ca.sh>` — chèn, đo, khôi phục,
ghi một dòng TSV. Bảng đầy đủ: `v2/agent/JOURNAL/kt252-attack/kt252-attack.tsv`; log từng ca ở
`kt252-attack/logs/<tên>/` (`cargo.log`, `cargo-rc.txt`, `lop1-thuong.log`, `lop1-strict.log`,
`lop2.log`, `git-truoc.txt`, `git-sau.txt`).

---

## 2. Kết quả

| Nhóm | Biến thể | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| G1 | **a5-trung-ten-impl** — `impl Kt252Door { pub fn wake_step { m.write_state(Ready) } }` trong `state_write.rs` + mã sản phẩm gọi | **0** | **0** | **0** | **0** | **ĐƯỜNG LỌT (WB-20)** — trước vá |
| G1 | **a6-trung-ten-mod** — `mod kt252 { pub fn wake_step }` | **0** | **0** | **0** | **0** | **ĐƯỜNG LỌT (WB-20)** |
| G1 | **a7-trung-ten-trait** — `impl Kt252Trait for Kt252Door { fn wake_step }` | **0** | **0** | **0** | **0** | **ĐƯỜNG LỌT (WB-20)** |
| G1 | **a8-trung-ten-free-fn** — free `fn wake_step` mức module | **0** | **0** | **0** | **0** | **ĐƯỜNG LỌT (WB-20)** |
| G1 | a5..a8 **sau vá** | 0 | 1 | 1 | 1 | **bị bắt** (cả bốn) |
| G2 | b1-helper-tra-ve-ready — `fn helper() -> TcbState::Ready` + `transition_tcb(t, helper())` | **101** | – | – | – | không biên dịch (`E0308`: `NonReadyTarget`) ⇒ **kiểu đã đóng** |
| G2 | b2-helper-write-state-muc-module — helper mức module gọi `write_state` | 0 | 1 | 1 | 1 | bị bắt (`E_STRICT_STATE_WRITE`) |
| G2 | b3-closure-trong-cua — closure **trong thân** `wake_step` gọi `write_state` | 0 | 0 | 0 | 0 | **XANH — KHÔNG phải cửa mới** (closure là mã của cửa; nợ đã ghi KT-242 §9.1) |
| G3 | c1-macro-sinh-cua — `macro_rules!` mở rộng thành `fn wake_step` | 0 | 1 | 1 | 1 | bị bắt |
| G3 | c2-macro-item-passthrough — `pass!($i:item)` + `fn wake_step` | 0 | 2 | 2 | 1 | bị bắt (`E_STRICT_MACRO_TT` + `E_STRICT_STATE_WRITE`) |
| G3 | c3-include-trong-loi-goi-macro — `include!` trong đối số macro | **101** | – | – | – | không biên dịch (dựng sai: `include!` là item, không phải `expr`) |
| G4 | d1-cfg-not-test-wake — `#[cfg(not(test))] fn … { m.wake(t) }` | 0 | 1 | 1 | 1 | bị bắt (`E_STRICT_TEST_ONLY_WAKE`) |
| G4 | d2-cfg-feature-ktest-wake — `#[cfg(feature = "ktest")] fn … { m.wake(t) }` | 0 | 1 | 1 | 1 | bị bắt (hợp đồng: `feature="ktest"` là **sản phẩm**, fail-closed) |
| G4 | d3-cfg-attr-path — `#[cfg_attr(not(test), path = "/tmp/…")] mod` | 0 | 1 | 1 | 1 | bị bắt (`E_STRICT_WAKE_INCLUDE`) |
| G5 | e1-buildrs-outdir — `build.rs` ghi `$OUT_DIR` + `include!(concat!(env!("OUT_DIR"), …))` | 0 | 1 | 1 | 1 | bị bắt (`E_STRICT_WAKE_INCLUDE`, không literal) |
| G5 | e2-duoi-la-txt — `include!("…txt")` chứa `wake` | 0 | 1 | 1 | 1 | bị bắt |
| G5 | e3-symlink — `src/kt252_e3.rs` là **symlink** ra `/tmp` chứa `wake` + `mod` | 0 | 1 | 1 | 1 | bị bắt (checker **đi theo** symlink) |
| G6 | f1-path-dot-slash — `#[path = "./kt252_f1.rs"]` (đích trong cây) | 0 | 1 | 1 | 1 | bị bắt |
| G6 | f2-path-dotdot-normalize — `#[path = "src/../src/…"]` | **101** | – | – | – | dựng sai: `#[path]` giải theo **thư mục tệp** ⇒ `src/src/…` không tồn tại |
| G6 | f3-src-target — `src/target/kt252_f3.rs` + `#[path = "target/…"]` | 0 | 1 | 1 | 1 | bị bắt (WB-12 giữ) |

**Tổng**: **19 biến thể**; **4 đường lọt** (a5–a8, **cùng một gốc** = WB-20); **15 bị bắt**;
**3 không biên dịch** (b1/c3/f2 — b1 là bằng chứng **kiểu đã đóng**, c3/f2 là ca dựng sai, ghi rõ);
**1 XANH đúng thiết kế** (b3 — closure trong thân cửa, không phải cửa mới).

---

## 3. Đường lọt WB-20 — "cửa thứ bảy" qua **TRÙNG TÊN**

### 3.1 Gốc

`_check_state_write` (lớp 1; lớp 2 **import lại** đúng hàm này) quy một lời gọi `write_state` cho
**hàm trong cùng theo token-tree** (`_innermost_fn_tree`, KT-242) rồi so **tên hàm** với
`STATE_WRITE_CALLERS`. KT-242 vá đúng ca **hàm lồng** (N1/N2) nhưng **không** vá ca **trùng tên**:
một `fn wake_step` **không phải method của `Machine`** vẫn khớp tên ⇒ được coi là cửa.

Bốn hình dạng đều biên dịch được (`cargo RC=0`) và **cả ba lượt kiểm XANH**:

```rust
// trong v2/kernel/vifast-core/src/state_write.rs
pub struct Kt252Door;
impl Kt252Door {                                  // (a5) impl kiểu KHÁC
    pub fn wake_step(m: &mut Machine, tcb: ObjId) -> Result<(), KError> {
        m.write_state(tcb, TcbState::Ready)       // Ready KHÔNG qua điều kiện nguồn nào
    }
}
pub mod kt252 { pub fn wake_step(…) { … } }       // (a6) module con
impl Kt252Trait for Kt252Door { fn wake_step(…) { … } }  // (a7) trait impl
pub fn wake_step(…) { … }                         // (a8) free fn mức module
```

Mã sản phẩm gọi `crate::state_write::Kt252Door::wake_step(&mut m, t)` ⇒ TCB về `Ready` **không
qua điều kiện nguồn nào** — đúng điều D2-194 §2 **cấm** ("cấm wrapper tổng quát nhận `TcbState`
rồi gọi primitive private"). Log: `kt252-attack/logs/a5-trung-ten-impl/` … `a8-…/`.

### 3.2 Vá (fail-closed, không allowlist rộng)

`v2/tools/check-wake-paths.py`:

1. Thêm `_impl_groups` / `_impl_type_from_header` / `_enclosing_impl_type` / `_fn_def_token` —
   đọc **kiểu của `impl` trong cùng** chứa một token (bỏ generic `<…>` ở mức 0; `impl X for Y` ⇒
   `Y`).
2. `_check_state_write`: tính `owner` = kiểu của `impl` chứa **định nghĩa** `write_state`
   (thực tế `Machine`; fixture selftest là `M` — **không hardcode tên**). Một lời gọi chỉ được
   coi là **cửa** khi `fn_name ∈ STATE_WRITE_CALLERS` **VÀ** `_enclosing_impl_type(call) == owner`.
   Hàm tự do / method kiểu khác / module con / trait impl ⇒ **ĐỎ** `E_STRICT_STATE_WRITE`.
3. Lớp 2 **không** phải sửa: nó `import` `CWP._check_state_write` (một nguồn luật, không chép).

Không nới gì: sáu cửa thật (`impl Machine`) vẫn XANH — có **đối chứng dương** trong selftest.

### 3.3 Ca selftest mới (có răng)

Lớp 1: **406 → 411** (+5): (4c2) `impl` khác ⇒ ĐỎ; (4c3) module con ⇒ ĐỎ; (4c4) trait impl ⇒ ĐỎ;
(4c5) free fn ⇒ ĐỎ; (4c6) **đối chứng dương** sáu cửa thật ⇒ XANH (thiếu vế này thì một luật
"luôn đỏ" cũng qua được 4c2–4c5).

**Răng đo được**: bốn biến thể a5–a8 **trước** vá = XANH cả ba lượt; **sau** vá = **ĐỎ cả ba lượt**
(`errors=1` mỗi lớp) — bảng §2. Đây là đột biến hai chiều, không phải "thêm ca cho có".

---

## 4. Số đo selftest trước/sau

| Phép đo | Trước KT-252 | Sau KT-252 |
|---|---|---|
| `check-wake-paths.py --selftest` | 406/406 ok | **411/411 ok** (39,9 s) |
| `check-wake-depinfo.py --selftest` | 65/65 ok | **65/65 ok** (1,7 s) |
| lớp 1 cây thật (thường) | `errors=0` | `errors=0` |
| lớp 1 cây thật (`--strict`) | `errors=0` | `errors=0` |
| lớp 2 cây thật | `errors=0` | `errors=0` |
| `./v2/bin/v2 check` | PASS | **PASS** |

---

## 5. Những gì **đã** chịu được tấn công (bằng chứng dương)

Ghi rõ để vòng sau không làm lại:

- **Kiểu đóng WB-16/WB-17**: helper trả `TcbState::Ready` đưa vào `transition_tcb` ⇒ `cargo`
  **E0308** (`expected NonReadyTarget, found TcbState`) — không biểu diễn được (b1).
- **`write_state` ngoài cửa**: helper mức module (b2), macro sinh cửa (c1), passthrough `:item`
  (c2) ⇒ ĐỎ cả ba lớp.
- **`cfg`/feature**: `cfg(not(test))` (d1), `cfg(feature="ktest")` (d2 — **sản phẩm**, fail-closed),
  `cfg_attr(path)` ra ngoài cây (d3) ⇒ ĐỎ.
- **Nguồn sinh**: `build.rs` + `$OUT_DIR` (e1), đuôi `.txt` (e2), **symlink** (e3) ⇒ ĐỎ — checker
  **đi theo** symlink và quét đích.
- **Đường dẫn**: `#[path = "./…"]` (f1), `src/target/` (f3) ⇒ ĐỎ.
- **Closure trong thân cửa** (b3) ⇒ XANH **đúng**: closure không phải `fn` nên quy cho cửa; cửa
  trả `Result<(), KError>` nên closure **không thoát ra ngoài** được ⇒ không phải cửa mới. Đây là
  **nợ đã ghi** ở `kt242-wake-patch.md` §9.1, không phải đường lọt mới.

---

## 6. Kết luận — **KHÔNG** tuyên bố đóng (D2-190)

- **Tìm được 1 lớp đường lọt mới** (WB-20, 4 hình dạng) — **đã vá** + có ca selftest có răng +
  đối chứng dương. Đây là **bằng chứng bổ sung**, không phải điều kiện đủ.
- **KT-184/KT-194 vẫn `REVIEW`**. D2-190 đòi 7 mục; vòng này đóng góp mục (5) "reviewer khác tác
  giả tấn công theo nhóm" **cho một vòng nữa**, và mục (7) "không còn đường lọt **đã biết**" —
  nhưng **không** chứng minh được "không còn đường lọt" (D2-190 §7 nói rõ: đóng luật = đạt hợp
  đồng hữu hạn đã công bố).
- **Nợ còn lại, không đổi**:
  - **DEBT-KT227-1** — `v2/lib` ngoài luật token `wake` (latent: không crate `v2/lib` nào phụ thuộc
    `vifast-core`). Vòng này **không** thử lại (đã đo ở KT-242).
  - **L2** — ba mẫu phạm vi (`in_test_only_wake_scope` / `RE_CRATE_SRC` / `MACRO_SCAN_ROOTS`) vênh
    nhau; không có đường lọt sống.
  - **`--cfg test` trong build sản phẩm** — vẫn là **giả định** chưa kiểm máy.
  - **Closure trong thân cửa** — nợ đã ghi (b3), không thoát được với chữ ký cửa hiện tại.
- **Đề nghị cho vòng sau** (không tự làm): (i) kiểm máy "build sản phẩm **không** có `--cfg test`";
  (ii) hợp nhất ba mẫu phạm vi thành **một** định nghĩa sinh từ `cargo metadata`; (iii) nếu mở
  `v2/lib` cho luật token `wake` thì đo lại toàn bộ ma trận này.

---

## 7. Tái lập

```bash
cd /tmp/kt243-s3
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + baseline
python3 v2/tools/check-wake-paths.py --selftest          # 411/411
python3 v2/tools/check-wake-depinfo.py --selftest        # 65/65
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py                   # errors=0

# (b) một biến thể (tự khôi phục bằng trap)
bash v2/agent/JOURNAL/kt252-attack/run-attack.sh "G1-alias/UFCS/fnptr" a5-trung-ten-impl \
     v2/agent/JOURNAL/kt252-attack/cases/a5-trung-ten-impl.sh
```

**Lưu ý cho người chạy lại**: `git clean -fd` **không** xoá tệp bị `.gitignore`; ca f3 tạo
`src/target/…` nên harness nay xoá tường minh `v2/{kernel,sim,lib}/*/src/target` (đã đo: để sót
thì lớp 1 ĐỎ sau khi ca xong).

---

## 8. Tệp thay đổi trong KT-252

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | 4 helper `impl`/`fn` + `_check_state_write` đòi cửa là method **cùng kiểu** với `write_state`; +5 ca selftest |
| `v2/agent/BYPASS-REGISTRY.md` | thêm **WB-20** (CLOSED) |
| `v2/agent/JOURNAL/kt252-wake-attack.md` | nhật ký này |
| `v2/agent/JOURNAL/kt252-attack/**` | harness + 19 ca + log + `kt252-attack.tsv` |

**Không** sửa: `v2/kernel/**`, `v2/sim/**`, `v2/lib/**`, `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/tools/check-wake-depinfo.py` (import lại luật lớp 1 — một nguồn).
