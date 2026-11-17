# KT-92 — đóng A1-184: `build.rs` gọi `$RUSTC` trực tiếp nên không nhận `--remap-path-prefix`

**Ngày**: 2026-11-17 · **Ngân sách**: 1 h · **Dùng**: ~25 phút.

## Lỗi (A1-184)

`v2/kernel/vifast-kbin-riscv64/build.rs` gọi `$RUSTC` **trực tiếp** để dựng shim U-mode, rlib và ELF
user. Các lần gọi đó **không** nhận `RUSTFLAGS` của cargo ⇒ đường dẫn nguồn **nằm trong byte ảnh**
⇒ cùng một commit build ở hai đường dẫn khác nhau cho ra **sha khác nhau** (`c65ee0f6` vs `07c34fc6`).

Đây là lỗ hổng của tính tái lập: `repro-build.sh` remap đường dẫn cho crate chính, nhưng phần do
`build.rs` tự dựng thì không.

## Vá

Thêm `remap_flags()`: đọc `CARGO_ENCODED_RUSTFLAGS` (cargo cấp cho build script, phân tách `0x1f`),
lấy mọi mục bắt đầu bằng `--remap-path-prefix`; dự phòng `RUSTFLAGS` dạng chuỗi. Truyền các cờ đó
cho **cả ba** chỗ gọi `$RUSTC` (shim `vifast-plat-user`, `build_rlib`, ELF user).

## Bằng chứng (đo, không suy luận)

Hai bản clone của **cùng commit**, đường dẫn khác độ dài: `/tmp/kt92-pA` và `/tmp/kt92-pB-longer-name`.

**Có vá** (cùng `build.rs` mới ở cả hai):

```text
/tmp/kt92-pA              sha256=449df10b46acbc10400cb428170502265b8ad4113befca98d2d26e4d2d679d85 build=907912139bffcff0
/tmp/kt92-pB-longer-name  sha256=449df10b46acbc10400cb428170502265b8ad4113befca98d2d26e4d2d679d85 build=907912139bffcff0
⇒ CÙNG sha, CÙNG build digest
```

**Đối chứng âm** (cùng hai clone, `build.rs` của HEAD — **không** remap):

```text
/tmp/kt92-pA              sha256=fc8ff047357ef812766ae2362b1b85dbe8c7fd29ab25781d0b957d0c2e39bba5 build=34836d77ceaef458
/tmp/kt92-pB-longer-name  sha256=5d81f434a0befe0d6b6e17831a43711eeb21ffe47ec5f7a8f804f0120b52bfce build=af169aca3eedb729
⇒ KHÁC sha, KHÁC build digest
```

Đối chứng âm chứng minh bản vá **là nguyên nhân** của việc cùng sha, không phải trùng hợp.

## Hệ quả

- **A1-184 đóng.** Từ nay "cùng commit ⇒ cùng sha" đúng **cả** khi cây nguồn nằm ở đường dẫn khác
  (điều kiện của `repro-check.sh` và của bất kỳ ai clone repo về máy mình).
- `build=` digest đổi (`af169aca…` → `907912139bffcff0`) vì byte ảnh đổi — **đúng**, không phải hồi quy.
- Ảnh phát hành `291502d2…` (KT-88) build **trước** bản vá này; nó vẫn hợp lệ cho phép đo đã làm,
  nhưng ảnh build lại từ HEAD sau KT-92 sẽ có sha khác. Mọi phép đo mới phải dùng ảnh mới.
