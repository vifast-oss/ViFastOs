# KT-242 — repro cho review độc lập luật `wake` (KT-184 / KT-194)

**Reviewer**: sub-agent `wake-review`. **HEAD review**: `c6127fa0`. **Worktree**:
`/tmp/kt242-s2` (nhánh `task/wake-review`).

Thư mục này **chỉ** chứa repro + log. **Không** sửa mã sản phẩm, **không** sửa checker,
**không** nới ngưỡng, **không** tắt ca.

Báo cáo đầy đủ: [`../kt242-wake-review.md`](../kt242-wake-review.md).

## Tệp

| Tệp | Việc |
|---|---|
| `probe.py` | harness: dựng cây fixture tối thiểu của **chính** checker (`_fixture_v2`) rồi chèn repro, chạy `scan_v2` hai chế độ |
| `fixture-probes.py` | **toàn bộ** ca fixture (N1–N6, L2, N3, DEBT-KT227-1, include/path) + **đối chứng âm/dương** |
| `run-repro.sh` | chèn repro vào **cây thật**, chạy `cargo check` + lớp 1 (thường, `--strict`) + lớp 2, rồi **khôi phục** bằng `trap` |
| `r3a.sh` | **N1** — `impl` lồng trong `wake_step` = "cửa thứ bảy" gọi thẳng `write_state`; mã sản phẩm gọi nó |
| `rs-case.sh` | **N4** — tệp `.RS` (hoa) được `mod` nạp trên FS không phân biệt hoa/thường |
| `binpath.sh` | **N3** — `[[bin]] path` **ngoài `src/`** chứa `m.wake(0)` |
| `a1.sh` | **kết quả ÂM** — `let f = self.wake;` (method-reference) **không** phải Rust hợp lệ (`E0615`), nên đây **không** phải đường lọt; giữ log để vòng sau không thử lại |
| `logs/<tên>/` | `cargo.log`, `cargo-rc.txt`, `lop1-thuong.log`, `lop1-strict.log`, `lop2.log`, `git-truoc.txt`, `git-sau.txt` |

## Chạy

```bash
cd /tmp/kt242-s2
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

python3 v2/agent/JOURNAL/kt242-wake-repro/fixture-probes.py

bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh r3a     v2/agent/JOURNAL/kt242-wake-repro/r3a.sh
bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh rs-case v2/agent/JOURNAL/kt242-wake-repro/rs-case.sh
bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh binpath v2/agent/JOURNAL/kt242-wake-repro/binpath.sh
```

`run-repro.sh` **luôn** khôi phục cây kể cả khi lỗi, nhưng **chỉ** `v2/kernel`/`v2/sim`/`v2/lib`
(`git checkout -- v2/kernel v2/sim v2/lib` + `git clean -fdq v2/kernel v2/sim v2/lib`).
**KT-242 (bản vá)** — bản cũ dùng `git checkout -- .` **toàn repo**, tức là **xoá luôn** bản vá
checker đang phát triển (đã xảy ra một lần: mất 34 sửa đổi trong `check-wake-paths.py`). Nay chỉ
khôi phục **cây sản phẩm**; `logs/<tên>/git-sau.txt` phải **không** còn tệp nào dưới
`v2/kernel`/`v2/sim`/`v2/lib`.

## Kết quả tóm tắt

### Trước bản vá (review `5546baa0`)

| Ca | Lớp 1 thường | Lớp 1 `--strict` | Lớp 2 | `cargo` |
|---|---|---|---|---|
| `r3a` (N1) | XANH | XANH | XANH | RC=0 |
| `rs-case` (N4) | XANH | XANH | **ĐỎ** `E_DEPINFO_WAKE` | RC=0 |
| `binpath` (N3) | XANH | XANH | **ĐỎ** `E_DEPINFO_WAKE` | RC=0 |
| fixture N2 | XANH | XANH | (không áp) | — |
| fixture N5a/N5b/N5c | XANH | XANH | (lớp 2 không có luật 1–3) | — |
| fixture N5a'/N5b' (đối chứng âm) | **ĐỎ** | **ĐỎ** | — | — |
| fixture N6 (`include!("\0")`) | **crash** `ValueError` | **crash** | — | — |
| `a1` (kết quả âm) | XANH | XANH | — | **RC=101** (`E0615`: `self.wake` không hợp lệ) |

### Sau bản vá (KT-242 nhóm C — xem [`../kt242-wake-patch.md`](../kt242-wake-patch.md))

| Ca | Lớp 1 thường | Lớp 1 `--strict` | Lớp 2 | `cargo` |
|---|---|---|---|---|
| `r3a` (N1) | **ĐỎ** `E_STRICT_STATE_WRITE` | **ĐỎ** | **ĐỎ** `E_DEPINFO_STATE_WRITE` | RC=0 |
| `rs-case` (N4) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` | **ĐỎ** | **ĐỎ** `E_DEPINFO_WAKE` | RC=0 |
| `binpath` (N3) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` | **ĐỎ** | **ĐỎ** `E_DEPINFO_WAKE` | RC=0 |
| fixture N2 | **ĐỎ** `E_STRICT_STATE_WRITE` | **ĐỎ** | (không áp) | — |
| fixture N5a/N5b/N5c | **ĐỎ** `E_ARCH_DOORBELL`/`E_CHANNEL_PRODUCER`/`E_CONSUMER` | **ĐỎ** | — | — |
| fixture N5a'/N5b' (đối chứng âm) | **ĐỎ** | **ĐỎ** | — | — |
| fixture N6 (`include!("\0")`) | **ĐỎ** `E_STRICT_WAKE_INCLUDE` | **ĐỎ** | — | — |
| `a1` (kết quả âm) | XANH | XANH | — | **RC=101** (`E0615`: `self.wake` không hợp lệ) |

Đối chứng dương **giữ nguyên**: R1 cùng dòng `#[cfg(test)]` vẫn **ĐỎ**; R1 trong
`#[cfg(test)] mod t` vẫn **XANH**; `cfg_attr` lồng trong `v2/lib` trỏ ra ngoài vẫn **ĐỎ**; crate
mới dưới `v2/kernel` vẫn **ĐỎ**; DEBT-KT227-1 vẫn **XANH** (ngoài phạm vi bản vá).
