# KT-224 — Review độc lập KT-214: `check-wake-depinfo.py` (lớp 2 kiểm `wake` theo `.d` của rustc)

**Người làm**: L53 (sub-agent của Agent 1) · **Nhánh**: `night3/L53-kt224` · **Gốc**: `83275d94`
**Đối tượng review**: [`v2/tools/check-wake-depinfo.py`](../../tools/check-wake-depinfo.py) (969 dòng)
+ journal [`kt214-depinfo-prototype.md`](kt214-depinfo-prototype.md) + việc nối cổng KT-217.
**Log**: [`kt224-logs/`](kt224-logs/) — 25 ca tấn công + 4 đối chứng dương + log `.d` + bảng JSON.
**Không sửa** `check-wake-depinfo.py` / `check-wake-paths.py`; mọi ca đều **hoàn nguyên** sau khi đo.

---

## 0. KẾT LUẬN NGẮN

**KHÔNG ĐẠT** — ở trạng thái hiện tại, với tư cách **một lớp cổng**.

Giả thuyết khoa học của KT-214 **đúng và đã kiểm lại được**: hỏi rustc danh sách tệp thật rồi quét
`wake` trên đúng danh sách đó bắt **11/11** đường lọt đã biết, stable 1.99.1, không `-Z`, rustc
**vẫn ghi `.d` khi biên dịch lỗi**. Nhưng bản thân công cụ có **năm đường lọt cả hai lớp**, **tám
đường lọt riêng lớp 2**, **một lỗi parse `.d`**, và **một chỗ fail-open** đúng vào yêu cầu
"phải fail-closed, không im lặng xanh". Ngoài ra **luận cứ trung tâm của KT-214/KT-217 đã cũ**:
"checker tĩnh chỉ bắt 5/11" **không còn đúng trên HEAD** — lớp 1 nay bắt **11/11** (6 đường bắt
bằng fail-closed sau KT-211/KT-213).

| Hạng mục | Kết quả |
|---|---|
| `--selftest` | **20/20** (0,30 s) — khớp |
| Chạy thật trên cây sạch | `errors=0`, **145** tệp nạp, **15,63 s** lạnh / **6,75 s** ấm — khớp (claim 14,7 / 6,3) |
| `.d` thật khớp danh sách công cụ báo | **Đúng** — 20 `.d` host + 17 `.d` kernel; parse độc lập ra đúng 145 tệp |
| Đường lọt **cả hai lớp** | **5** — B3b, B12, B13, B14, B15 |
| Đường lọt **riêng lớp 2** (lớp 1 bắt) | **8** — B4b, B7, B8, B9, B10, B11, B17, B18 |
| Báo oan | **0** — 4/4 đối chứng dương XANH |
| Parse `.d` | **SAI** — escape `\ ` bị `split()` phá; `.d` rỗng/thiếu ⇒ **im lặng XANH** (fail-open) |
| Luận cứ "tĩnh 5/11" | **CŨ** — trên HEAD tĩnh bắt **11/11** (đã tái lập p02–p07) |
| Giới hạn #6 của journal | **SAI** — lớp 1 **cũng không** bắt `#[cfg(feature = "ktest")] mod` (miễn test-only) |

---

## 1. KIỂM LẠI TUYÊN BỐ (mục A)

### A1. `--selftest` — 20/20

```
$ python3 v2/tools/check-wake-depinfo.py --selftest
...
# selftest: 20/20 ok
real 0m0.299s
```

Khớp. Nhưng xem §4.2: **ca (4) của selftest kiểm `_unescape_dep` trực tiếp**, không qua
`parse_depinfo` — nên nó **pass trong khi parser thật sai**.

### A2. Chạy thật — `errors=0`, 145 tệp, 15,63 s lạnh / 6,75 s ấm

Lần đầu trên worktree **chưa có `target/`** (lạnh hoàn toàn):

```
# đơn vị host: … → RC=0 (4.39 s), 20 artifact, 20 tệp .d, 74 tệp .rs
# đơn vị kernel: … --target riscv64gc-unknown-none-elf → RC=0 (5.04 s), 18 artifact, 17 tệp .d, 112 tệp .rs
# rustc nạp: 145 tệp .rs trong cây (gộp 2 đơn vị)
# quét: 143 tệp (miễn 2 tệp cửa wake + 0 tệp test-only)
# check-wake-depinfo: errors=0 warnings=0 (15.63 s)
```

Ấm (hai lần liên tiếp): **8,47 s** rồi **6,75 s** (quét 6,56 s). Claim 6,3 s ấm / 14,7 s lạnh —
khớp trong sai số máy. Log: `a2-chay-that-lan1.json`, `a2-chay-that-am-{1,2}.log`.

### A3. `.d` có thật và khớp danh sách công cụ báo

Parse độc lập (script riêng, không dùng code công cụ) 20 `.d` host + 17 `.d` kernel:

```
host   d_files=20 loaded=74
kernel d_files=17 loaded=112
union=145          ← đúng bằng `loaded_files` công cụ báo
non-.rs deps trong .d: 0
```

Nội dung `.d` đúng dạng make: dòng đầu `<target tuyệt đối>: <phụ thuộc>`, phụ thuộc của thành
viên workspace là **tương đối** (`lib/vifast-model/src/lib.rs`), path-dependency ngoài workspace
là **tuyệt đối**. Không có dòng nối `\` trong cây thật (mọi `.d` một dòng) — nhưng parser xử lý
nối dòng đúng (kiểm bằng `.d` dựng tay, §4.1).

---

## 2. TẤN CÔNG LỚP 2 (mục B) — 25 ca

Harness: [`kt224-logs/harness-tan-cong.py`](kt224-logs/harness-tan-cong.py); bảng máy đọc:
[`kt224-logs/bang-tan-cong.json`](kt224-logs/bang-tan-cong.json). Mỗi ca: dựng mã **biên dịch
được** (log lớp 2 ghi `RC=0` của `cargo check`), chạy lớp 2 + lớp 1 (thường, `--strict` khi lớp 1
XANH), rồi **hoàn nguyên**.

### 2.1 Năm đường lọt **CẢ HAI LỚP** (XANH/XANH)

| # | Cơ chế | Vì sao lớp 2 mù | Vì sao lớp 1 mù |
|---|---|---|---|
| **B3b** | `#[cfg(feature = "ktest")] mod kt224_cfg_feature;` + tệp riêng chứa `m.wake(t)` | feature tắt ⇒ rustc không nạp tệp ⇒ `.d` không có | `test_only_modules` coi module dưới `#[cfg(feature = "ktest")]` là **test-only** ⇒ miễn |
| **B12** | `#[path = "kt224_evil.txt"] mod …;` trong `v2/lib/vifast-model` | `p.suffix != ".rs"` ⇒ bỏ | luật `wake` **không quét `v2/lib`**; nhánh lib của `resolve_include_targets` phân giải nhưng **không** đưa vào `extra` |
| **B13** | symlink `v2/lib/…/kt224_link.rs` → `/tmp/kt224_evil_lib.rs` | `p.resolve()` → `/tmp/…` → `relative_to(root)` ValueError ⇒ bỏ **im lặng** | như B12 (không quét `v2/lib`) |
| **B14** | `#[path = "target/kt224_evil.rs"]` trong `v2/lib` | `"target" in relp.parts` ⇒ bỏ | như B12 |
| **B15** | `#[path = "kt224 evil.rs"]` (dấu cách) trong `v2/lib` | `parse_depinfo` `split()` phá escape `\ ` ⇒ không resolve được | như B12 |

**Bằng chứng `.d` có thật** (dựng cả 4 tệp cùng lúc, `cargo check` RC=0, đọc `.d` của
`vifast_model`):

```
lib/vifast-model/src/kt224_evil.txt
lib/vifast-model/src/kt224\ evil.rs          ← escape dấu cách
lib/vifast-model/src/target/kt224_evil.rs
lib/vifast-model/src/kt224_link.rs
```

⇒ `.d` **có** cả bốn; công cụ **bỏ** cả bốn. Đây là **đường lọt #14** theo nghĩa "lớp 2 bỏ tệp
rustc nạp"; với B12–B15 nó lọt **cả hai lớp** vì lớp 1 không có luật `wake` trên `v2/lib`.

**B3b là đường lọt cả hai lớp nằm trong `v2/kernel`** — cây bảo vệ chính. Nó chỉ là "test-only"
theo thiết kế (`ktest` = ảnh test), nhưng **journal KT-214 §6 #6 nói sai**: "checker tĩnh quét
**mọi** nhánh" — không đúng với module dưới `#[cfg(feature = "ktest")]`, vì đồ thị module miễn nó.
Đã kiểm trực tiếp: `test_only_modules` trả `True` cho tệp đó.

### 2.2 Tám đường lọt **riêng lớp 2** (lớp 2 XANH, lớp 1 ĐỎ)

| # | Cơ chế | Lớp 2 bỏ vì | Lớp 1 bắt vì |
|---|---|---|---|
| **B4b** | `#[cfg(any())] mod` + tệp riêng | không nạp | quét mọi `.rs` |
| **B7** | `#[path = "/tmp/kt224_evil.rs"]` (kernel) | ngoài `root` ⇒ `relative_to` ValueError ⇒ bỏ **im lặng** | `E_STRICT_WAKE_INCLUDE` fail-closed |
| **B8** | symlink `src/kt224_link.rs` → `/tmp/…` (kernel) | `resolve()` ra ngoài root ⇒ bỏ | quét chính symlink |
| **B9** | `#[path = "kt224_evil.txt"]` (kernel) | `suffix != ".rs"` | `extra` được quét |
| **B10** | `#[path = "kt224 evil.rs"]` (kernel) | parse `\ ` sai | `extra` được quét |
| **B11** | `#[path = "target/kt224_evil.rs"]` (kernel) | `"target" in relp.parts` | KT-215 đã vá `_is_build_dir` |
| **B17** | `build.rs` sinh `$OUT_DIR/kt224_gen.rs` + `include!(concat!(…))` | tệp dưới `target/` ⇒ bỏ | `include!` không literal ⇒ fail-closed |
| **B18** | `m.wake(t)` trong `core_sched.rs` (tệp cửa wake) | `is_producer_file` miễn **cả tệp** | chỉ miễn **định nghĩa** `fn wake` |

**B11 là hồi quy của đường lọt #12 trên lớp 2**: KT-215 vá `_is_build_dir` cho lớp 1 nhưng lớp 2
vẫn dùng `"target" in relp.parts` — đúng lỗi cũ. **B18** cho thấy miễn theo tệp quá rộng: chỉ cần
tệp có `fn wake(` là cả tệp thoát luật R1.

### 2.3 Bảy ca **cả hai lớp ĐỎ** (đúng)

| # | Cơ chế | Ghi chú |
|---|---|---|
| B1 | `macro_rules!` thân chứa `m.wake(t)` | cả hai bắt (token nằm trong tệp nạp) |
| B2 | `macro_rules!` nhận `wake` làm đối số | cả hai bắt (`_macro_arg_of` + token) |
| B3a | `#[cfg(feature = "ktest")]` **khối trong `lib.rs`** | cả hai bắt — **giới hạn #6 chỉ đúng khi mã ở tệp riêng** |
| B4a | `#[cfg(any())]` **khối trong `lib.rs`** | cả hai bắt — cùng lý do |
| B5 | `#[cfg(target_os = "none")] mod` tệp riêng | lớp 2 **kernel** nạp ⇒ bắt; thiết kế hai đơn vị đúng |
| B6 | `include!` tệp không tồn tại | build RC=101, **rustc vẫn ghi `.d`** (17 `.d`, 112 tệp) ⇒ `E_DEPINFO_BUILD` + `E_STRICT_WAKE_INCLUDE` |
| B19 | `#[cfg(test)]` trên nhánh `match` + `wake` nhánh sản phẩm (đường lọt #13) | cả hai bắt — bản vá KT-219 có tác dụng ở **cả hai** lớp |

**B6 xác nhận tuyên bố "rustc vẫn ghi `.d` khi biên dịch lỗi"**: build RC=101 nhưng vẫn 17 `.d`
và 112 tệp `.rs` — danh sách tệp không phụ thuộc build xanh.

### 2.4 Một ca lớp 2 ĐỎ / lớp 1 XANH

**B16** — `#[path = "kt224_évil.rs"]` (Unicode) trong `v2/lib`: lớp 2 **bắt** (parse UTF-8 đúng),
lớp 1 XANH (không quét `v2/lib`). Đây là **đối chứng dương cho parse Unicode** — không phải lỗi.

---

## 3. ĐỐI CHỨNG DƯƠNG (mục C) — 0 báo oan

| Ca | Nội dung | Lớp 2 | Lớp 1 | Lớp 1 `--strict` |
|---|---|---|---|---|
| C1a | `wake_from`, `wake_step`, biến cục bộ `let mut wake` | XANH | XANH | XANH |
| C1b | `MailboxMsg::Wake` trong **pattern** (`matches!`) | XANH | XANH | XANH |
| C1c | định nghĩa `fn wake` trong tệp mới | XANH | XANH | XANH |
| C1d | `m.wake(t)` trong chú thích + literal chuỗi | XANH | XANH | XANH |

**Không báo oan.** Ghi chú: ca đầu tiên tôi dựng có `MailboxMsg::Wake(t)` **constructor** trong
hàm thường ⇒ lớp 1 ĐỎ `E_CHANNEL_PRODUCER` — đó là **luật kênh** (chỉ cửa wake được phát
`MailboxMsg::Wake`), hành vi **đúng**, không phải báo oan của luật `wake`. Đã tách thành C1b
(pattern) để kiểm đúng luật.

---

## 4. TÍNH ĐÚNG CỦA VIỆC ĐỌC `.d` (mục D)

### 4.1 Nối dòng, tuyệt đối/tương đối, escape — đúng một phần

| Dạng | Kết quả |
|---|---|
| Nối dòng `\` + newline | **Đúng** — `parse_depinfo` thay `\\\n` bằng dấu cách trước khi tách dòng |
| Nhiều dòng đích trong một `.d` | **Đúng** — gom đích + phụ thuộc mọi dòng |
| Tuyệt đối vs tương đối | **Đúng** — `resolve_dep` dùng `ws_root` = thư mục `Cargo.toml` |
| Escape `\\` (backslash), `\#` | **Đúng** — `_unescape_dep` xử lý |
| **Escape `\ ` (dấu cách)** | **SAI** — xem §4.2 |

### 4.2 LỖI: `parse_depinfo` tách theo khoảng trắng **trước** khi giải escape

```python
for tok in rhs.split():          # ← tách trước
    deps.append(_unescape_dep(tok))
```

`src/a\ b.rs` (một tệp tên `a b.rs`) bị `split()` thành `['src/a\\', 'b.rs']`; không token nào
resolve tới tệp thật ⇒ **tệp im lặng không được quét**. Kiểm trực tiếp:

```
parse_depinfo("libx.rmeta: src/a\\ b.rs src/c.rs") → ['src/a\\', 'b.rs', 'src/c.rs']
```

Selftest ca (4) **không phát hiện** vì nó gọi `_unescape_dep("a\\ b.rs")` trực tiếp, bỏ qua
`split()`. Đây là lỗi thật, đã tái lập bằng build thật ở **B10/B15**.

### 4.3 LỖI: `.d` rỗng / không tồn tại ⇒ **im lặng XANH** (fail-open)

Yêu cầu của phiếu: "phải fail-closed, không im lặng xanh". Thực đo:

```
$ python3 v2/tools/check-wake-depinfo.py --root <root có .d RỖNG> --no-build
# đơn vị host: (no-build) → RC=0 (0.00 s), 0 artifact, 1 tệp .d, 0 tệp .rs
# đơn vị kernel: … 1 tệp .d, 0 tệp .rs
# rustc nạp: 0 tệp .rs …
# check-wake-depinfo: errors=0 warnings=0 (0.00 s)
RC=0

$ python3 v2/tools/check-wake-depinfo.py --root <root KHÔNG có target/> --no-build
# … 0 tệp .d, 0 tệp .rs [không có …/debug/deps]
# check-wake-depinfo: errors=0 warnings=0 (0.00 s)
RC=0
```

Cùng lớp lỗi với `--no-build` trên cây chưa build: **0 tệp quét, XANH**. Trong chế độ build,
nếu `cargo check` RC=0 nhưng không có `.d` (hoặc bộ lọc artifact loại hết `.d`), công cụ cũng
XANH với 0 tệp. **Không có ngưỡng tối thiểu số tệp nạp** ⇒ xanh-giả.

### 4.4 Bộ lọc artifact — đúng nhưng có thể bỏ hết `.d`

`use_all = (build_ok is False) or (not artifacts)`. Khi build xanh, chỉ `.d` có đích giao với
`filenames` của cargo được dùng. Đúng cho cây thật (20/20 host, 17/17 kernel khớp). Nhưng nếu
`.d` còn lại là **cũ** (target dir đổi profile/layout) thì `depinfo_used=0` ⇒ 0 tệp ⇒ XANH
(fail-open, cùng §4.3).

---

## 5. LUẬN CỨ "TĨNH 5/11" ĐÃ CŨ — LỚP 1 NAY BẮT 11/11

Bảng 11 đường của KT-214 đo ở gốc `dc6e01ae` (**trước** khi KT-211/KT-213/KT-215 merge). Tôi
tái lập trên HEAD `83275d94`:

| Đường | KT-214 (gốc `dc6e01ae`) | **HEAD `83275d94`** (đo lại) |
|---|---|---|
| p02 `include!` trong `( … )` | tĩnh XANH | **ĐỎ** `E_STRICT_WAKE_INCLUDE` |
| p03 `include!` trong `[ … ]` | tĩnh XANH | **ĐỎ** (cùng lý do) |
| p04 `(0u8, include!{…})` | tĩnh XANH | **ĐỎ** |
| p05 `(|| include!{…})` | tĩnh XANH | **ĐỎ** |
| p06 `Some(include!{…})` | tĩnh XANH | **ĐỎ** |
| p07 `#[path]` inline module non-mod-rs | tĩnh XANH | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |

⇒ Trên HEAD, lớp 1 bắt **11/11** (5 đường cũ + 6 đường nay bắt bằng fail-closed). Luận cứ
"lớp 2 bắt 6 đường mà lớp 1 lọt" **không còn đúng**; giá trị của lớp 2 bây giờ là **cơ chế độc
lập** + **phủ `v2/lib`** (nơi lớp 1 không có luật `wake`), không phải "lấp 6/11".

Đã tái lập p02–p07 nguyên văn repro của harness KT-214; log:
`p02-p07-tai-lap-head.log`. Bảng 11 đường gốc vẫn **nhất quán nội bộ** (11/11 cargo RC=0,
tĩnh 5/11, depinfo 11/11, hidden=1) — chỉ là **cũ** so với HEAD.

---

## 6. DANH SÁCH PHÁT HIỆN

### P1 — Nghiêm trọng (phải sửa trước khi dùng làm cổng)

1. **Fail-open khi `.d` rỗng/thiếu/0 tệp** — `errors=0`, RC=0, 0 tệp quét. Vi phạm trực tiếp yêu
   cầu "fail-closed". Cần: nếu `loaded` rỗng hoặc `depinfo_used==0` ⇒ `E_DEPINFO_EMPTY` ĐỎ.
2. **`parse_depinfo` phá escape `\ `** — tên tệp có dấu cách không bao giờ được quét. Cần: tách
   token tôn trọng `\ ` (hoặc dùng `shlex`-style).
3. **`"target" in relp.parts`** — hồi quy đường lọt #12 trên lớp 2. Cần dùng logic
   `_is_build_dir` của lớp 1 (chỉ bỏ khi `target` đứng **trước** `src`).
4. **Bỏ im lặng tệp ngoài `root` / symlink ra ngoài / đuôi không `.rs`** — ba lớp "im lặng xanh".
   Cần: đếm và **báo** (tối thiểu `W_DEPINFO_SKIPPED`, tốt hơn là ĐỎ fail-closed) mọi phụ thuộc
   `.rs`-hoặc-không mà công cụ không quét được.
5. **`is_producer_file` miễn cả tệp** — chỉ nên miễn **định nghĩa** `fn wake`, không miễn mọi
   tham chiếu khác trong cùng tệp (B18).

### P2 — Trung bình

6. **Không có "sản phẩm thắng" cho test-only** — tệp trong `test_only_modules` được miễn kể cả khi
   rustc nạp nó trong bản build sản phẩm (lớp 1 đã có bản vá KT-174; lớp 2 chưa).
7. **Luận cứ "tĩnh 5/11" cũ** — phải đo lại trên HEAD (nay 11/11) trước khi dùng để biện minh
   chi phí +8 s trong `v2 check`.
8. **Giới hạn #6 của journal sai** — lớp 1 **cũng không** bắt `#[cfg(feature = "ktest")] mod`
   (miễn test-only). Cần sửa journal/hợp đồng, không được nói "tĩnh quét mọi nhánh".

### P3 — Thấp / ghi nhận

9. `--no-build` không có cơ chế phát hiện `.d` cũ (không so artifact) — chấp nhận được nếu P1.1 vá.
10. Hard-code `debug` profile — đã ghi trong journal, không phải lỗi mới.
11. `hidden_files` không báo tệp `.txt`/`target`/dấu cách vì chúng nằm trong `src/` — chỉ số cốt
    lõi bỏ sót đúng các tệp này.

---

## 7. ĐIỀU KIỆN ĐỂ THÀNH "ĐẠT CÓ ĐIỀU KIỆN"

1. Vá P1.1 (fail-closed khi 0 tệp) — **bắt buộc**, đây là yêu cầu của phiếu.
2. Vá P1.2 (escape `\ `) + thêm ca selftest đi **qua `parse_depinfo`**, không chỉ `_unescape_dep`.
3. Vá P1.3 (`target`) + P1.4 (báo tệp bị bỏ) + P1.5 (miễn theo định nghĩa, không theo tệp).
4. Đo lại bảng 11 đường trên HEAD và sửa luận cứ "5/11" trong journal KT-214 + KT-217 + QUEUE.
5. Sửa giới hạn #6 (hoặc vá để lớp 1 bắt `#[cfg(feature = "ktest")]` khi feature bật ở ảnh test).

Sau 5 điều kiện trên, lớp 2 xứng đáng là **lớp bổ sung** (không thay lớp 1) — đúng như khuyến
nghị §7.2 của KT-214.

---

## 8. VIỆC ĐÃ LÀM / CHƯA LÀM

**Đã làm**: kiểm 20/20 selftest; chạy thật lạnh/ấm; parse độc lập `.d`; 25 ca tấn công + 4 đối
chứng dương, mỗi ca hoàn nguyên; tái lập p02–p07 trên HEAD; kiểm `test_only_modules` cho B3b;
kiểm fail-open `.d` rỗng/thiếu; kiểm escape `\ `.

**Chưa làm**: không chạy lại **toàn bộ** harness 11 đường của KT-214 (≈13 phút; đã tái lập 6/11
đường quan trọng nhất p02–p07 + đọc bảng JSON gốc); không chạy `gate --fast` (bị cấm); không
sửa công cụ.

---

## Phụ lục A — Tệp đã chạm

| Tệp | Loại |
|---|---|
| `v2/agent/JOURNAL/kt224-review214.md` | **mới** (tệp này) |
| `v2/agent/JOURNAL/kt224-logs/**` | **mới** (harness + log + bảng JSON) |

**Không** chạm: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`,
`v2/tools/check-wake-depinfo.py`, `v2/tools/check-wake-paths.py`, journal của agent khác.

## Phụ lục A2 — Chỉ mục log

| Log | Nội dung |
|---|---|
| `a1-baseline-{thuong,strict}.log` | lớp 1 trên cây sạch: `errors=0` cả hai chế độ (27,8 / 28,0 s) |
| `a2-chay-that-lan1.json` | lớp 2 lạnh hoàn toàn: `errors=0`, 145 tệp, 15,63 s |
| `a2-chay-that-am-{1,2}.log` | lớp 2 ấm: 8,47 s / 6,75 s |
| `harness-tan-cong.py` | harness 25 ca tấn công + 4 đối chứng dương |
| `bang-tan-cong.json` | bảng máy đọc: RC lớp 1 / lớp 1 `--strict` / lớp 2 từng ca |
| `b-<ca>-layer{1,2}[-strict].log` | log thô từng ca |
| `d-dot-d-co-tep-bi-bo.log` | `.d` **có** cả bốn tệp B12–B15 mà công cụ bỏ |
| `d-fail-open-d-rong-thieu.log` | `.d` rỗng/thiếu ⇒ `errors=0`, RC=0 (fail-open) |
| `d-parse-d-escape-dau-cach.log` | `parse_depinfo` phá escape `\ ` |
| `p02-p07-tai-lap-head.log` | tái lập 6 đường lọt trên HEAD: lớp 1 bắt 6/6, lớp 2 bắt 2 |

## Phụ lục B — Cách tái lập

```bash
cd /private/tmp/night3-L53-kt224
export PATH="$HOME/.cargo/bin:$PATH"

python3 v2/tools/check-wake-depinfo.py --selftest          # 20/20
python3 v2/tools/check-wake-depinfo.py --root "$PWD"       # errors=0, 145 tệp
python3 v2/agent/JOURNAL/kt224-logs/harness-tan-cong.py    # 25 ca (≈15 phút)
```
