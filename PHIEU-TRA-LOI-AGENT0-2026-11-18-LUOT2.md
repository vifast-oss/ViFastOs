# PHIẾU TRẢ LỜI AGENT 0 — lượt 2 (Q4, C1, C2, C7, selftest)

**Ngày**: 2026-11-18 · **HEAD**: `f50c6dea`, đã push, cây sạch, `v2 check` và `gate --fast` PASS.
**Ảnh hiện tại**: `464fe583…`, `inputs=097d1de4`, build từ HEAD `4ed158b4`.

> Tệp tự chứa. Mỗi mục dưới đây là một việc anh giao, kèm bằng chứng và **những chỗ tôi làm sai
> hoặc bỏ sót** — tôi ghi hết, không để anh phải đi tìm.

---

# 1. Q4 — bảng chín lệnh

Bảng đầy đủ ở [`v2/agent/reports/Q4-9-COMMANDS.md`](v2/agent/reports/Q4-9-COMMANDS.md). Tóm tắt:

| lệnh | ca **đúng** | ca **biên** |
|---|---|---|
| `ls` | `new-cmds-ok` | `new-cmds-edge` |
| `cat` | `write-cat-bytes`, `write-de-co-lai` | `cat-khong-tham-so`, `cat-khong-co` |
| `write` | `write-cat-bytes`, `write-de-co-lai` | `write-khong-tham-so` |
| `time` | `new-cmds-ok` | `new-cmds-edge` |
| `uptime` | `new-cmds-ok` | **`new-cmds-edge2`** (mới) |
| `sync` | `new-cmds-ok` | `new-cmds-edge` |
| `stat` | `new-cmds-ok`, `truncate-jobs` | `new-cmds-edge` |
| `truncate` | `truncate-jobs` | `new-cmds-edge` |
| `jobs` | `truncate-jobs` | **`new-cmds-edge2`** (mới) |

**Anh chỉ đúng: `uptime` và `jobs` thiếu ca biên.** Đã thêm ca `new-cmds-edge2` gửi `uptime` hai
lần, `uptime 5`, `jobs` hai lần, `jobs 5`, và khẳng định: `uptime` đơn điệu, `jobs` **idempotent**
(hai dòng giống nhau từng ký tự), cả hai lệnh có tham số thì bị coi là lệnh lạ.

**Đã chạy thật trên ảnh sản phẩm**, không chỉ viết ra:

```text
# new-cmds-edge2: PASS (# shell: jobs ready=3 running=0 blocked=0 other=0)
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 PASS
```

**Ba điều tôi phải nói thẳng:**

- **Bốn ca biên còn mỏng**: `write` mới có "thiếu đường dẫn"; `sync` mới có "gọi hai lần"; `time`
  mới có "gọi hai lần" (chưa có ca `time` kèm tham số); `ls` mới có "đường không có".
- **Phán quyết của `new-cmds-ok` là gộp**: khẳng định riêng từng lệnh, nhưng **một** dòng PASS cho
  **năm** lệnh, và log chỉ in lý do của lệnh hỏng **đầu tiên**.
- **Con số tôi khai ở phiếu trước là sai**: tôi viết "hai mươi bảy phép kiểm, mười chín ca âm".
  Số thật lúc đó là **mười lăm âm + mười hai dương**. Tôi chép nguyên từ dòng tổng kết **bị lỗi**
  của chính script. Đã sửa script; nay là **mười tám âm + mười ba dương** trên ba mươi mốt phép kiểm.

---

# 2. C1 — chốt `VBREV_ALGO`, và **một chỗ tôi bỏ sót chính nguyên tắc của anh**

## 2.1 Đã làm đúng yêu cầu

Tệp ghim `v2/test/boot/build-rev.algo.pin` giữ **cặp** (sha256 của `build-rev.sh`, giá trị
`VBREV_ALGO`). Script `v2/tools/check-algo-pin.sh` kiểm:

- sha đổi **và** algo không đổi ⇒ **ĐỎ** `E_ALGO_NOT_BUMPED`.
- sha đổi **và** algo đã đổi ⇒ xanh, kèm nhắc cập nhật ghim.
- **`--strict`**: ghim cũ (đã tăng algo nhưng quên `--update`) cũng **ĐỎ** `E_PIN_STALE`.

Chỗ `--strict` là tôi thêm ngoài yêu cầu, vì nếu không thì cơ chế chỉ bắt được **lần vi phạm đầu
tiên** rồi xanh vĩnh viễn. Đã nối `--strict` vào `v2 check` và `gate --fast`.

**Mười hai ca tự kiểm**, mỗi ca so **đúng mã lý do**. Đã chứng minh đỏ thật trên repo: thêm một
dòng chú thích vào `build-rev.sh` mà không đổi `VBREV_ALGO` ⇒

```text
ALGO-PIN: RED reason=E_ALGO_NOT_BUMPED script=…/v2/test/boot/build-rev.sh sha=a6bbc035… pin_sha=1b25f329… algo=kt134-v1
```

## 2.2 Chỗ tôi bỏ sót — và đây là phần quan trọng nhất của phiếu này

Sau khi commit xong đợt đầu, tôi chạy `v2 status` và thấy dấu vân tay **đổi** dù tôi chỉ sửa
**script cổng**. Truy ra: `VBREV_INPUT_ROOTS` có `v2/bin` — mà `v2/bin` chứa `v2` (script CLI),
`vifast-gen`, `vifast-idl`, **không tệp nào vào ảnh**.

Tức là **đúng lỗi C1 anh vừa bắt, ở một chỗ khác, và tôi đã bỏ sót khi làm C1 lần đầu.** Tôi chỉ
loại ba tệp anh chỉ ra mà không quét lại toàn bộ phạm vi.

**Đã sửa**: loại `v2/bin` khỏi phạm vi. Kiểm trước khi loại:

- `v2/kernel/*/build.rs` **không** gọi `v2/bin`.
- `vifast-gen` sinh ra `v2/lib/vifast-idl/src/generated*.rs`, và các tệp đó **đã tracked trong
  `v2/lib`** — mà `v2/lib` vẫn nằm trong phạm vi. Nên đổi generator rồi sinh lại thì dấu vân tay
  **vẫn** đổi, qua `v2/lib`. Không có khe.

**Thêm ca (i)**: sửa script cổng ⇒ dấu vân tay **KHÔNG** đổi. Đã chứng minh đỏ thật: thêm lại
`v2/bin` vào phạm vi ⇒ ca (i) đỏ với `sửa script cổng mà dấu vân tay ĐỔI (446baa34… -> 184e8fbe…)`.

**Bump `VBREV_ALGO`** `kt134-v1` → `kt134-v2` (đổi phạm vi là đổi **định nghĩa** dấu vân tay, nên
bump là đúng), cập nhật ghim, rebuild, soak lại.

**Kết quả**: phạm vi nay chỉ còn `v2/kernel`, `v2/lib`, `v2/Cargo.toml`, `v2/Cargo.lock`,
`v2/rust-toolchain.toml`, `v2/tools/repro-build.sh`, `v2/tools/rustc-canon.rs`. Sửa `v2/bin`,
`v2/test`, `v2/tools` (trừ hai tệp build) **không** còn đổi dấu vân tay — cái bẫy chạy đuổi đã hết.

## 2.3 Một lỗ hổng còn lại, tôi không tự sửa vì nó đổi bề mặt log

Dòng bằng chứng hiện là `# build: inputs=<hex> dirty=<0|1>` — **không có** tên thuật toán. Nên log
cũ và log mới không phân biệt được định nghĩa nào sinh ra chúng; tệp ghim chỉ cho biết phiên bản
**hiện tại**.

Cách sửa đúng là nhúng `VBREV_ALGO` vào dòng log/ảnh (`# build: algo=… inputs=… dirty=…`). Việc đó
phải sửa chỗ kernel in, `vbrev_parse` (đang chặt: "không có gì thêm"), và `expected.txt` ⇒ cần
rebuild + đo lại toàn bộ. **Đề nghị tách thành task riêng**; tệp ghim là bản vá rẻ, nhúng algo là
bản vá đúng.

---

# 3. C2 — soak một nghìn vòng, harness chính thức, ảnh phát hành

**Đã chạy đủ một nghìn vòng, không phải smoke.** Lệnh:

```
bash v2/test/boot/soak.sh --features product-shell,product-fs 1000 3
```

Kết quả:

```text
# soak: xong wall=290s (3 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=301720ec292c4f89 sha256=464fe583caf8d95e084e51d5c7f075a7660a7b4fc27fc9dd71befba7c65eeab8 git=4ed158b4ce047116b82925e88cee3cca4c0597f8
# soak: 0/1000 lỗi (PASS)
# soak.sh: PASS
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=7.99 loadavg_5m=5.29 loadavg_15m=4.39 ncpu=12 boot_done=1 verdict=OK
```

Mỗi vòng đủ **ba** dấu hiệu (trích một vòng bất kỳ): `# build: inputs=097d1de49aef96cd dirty=0`,
`shell>`, `# shell: help cmds=15`, `# shell: exit code=0`, `# product halt:`.

**Đã ghi vào `K6-CLOSURE.md` thành Phụ lục A** — tiêu đề "hạn chế thứ nhất đã gỡ", gắn ảnh
`464fe583…`, và **không** sửa kết luận gốc (vẫn gắn `a0287e4b…`). Phụ lục có bảng so sánh hai ảnh
và nói rõ vì sao khác.

**Một chi tiết tôi phải sửa ngoài yêu cầu**: `soak.sh` build bằng `cargo build` trần, nên sha
**khác** ảnh phát hành (thiếu remap + strip). Nếu để vậy thì "soak ảnh phát hành" hoá ra là soak
một ảnh khác cùng nguồn. Nay khi có `--features`, soak build qua `repro-build.sh` ⇒ sha trùng
pipeline canonical, và in dòng neo `# neo ảnh: sha=… TRÙNG ảnh đã ghim`.

---

# 4. C7 — đếm câu bỏ qua, và **hai lỗi xanh giả trong chính checker của tôi**

## 4.1 Đã làm đúng yêu cầu

Checker in ra số câu bị bỏ qua, vị trí từng câu, và từ khoá nào khớp:

```text
# check-scope-table: bỏ qua 4/19 câu khẳng định (từ khoá loại trừ) — v2/docs/k6-exec.md 1, v2/docs/k6-persist.md 3, v2/agent/reports/K6-DOD.md 0
#   v2/docs/k6-exec.md:154  [đỏ]  `scope-red` dưới), TAP `exec giu pid cspace` **đỏ** (`same` …
#   v2/docs/k6-persist.md:78  [nhánh]  nhánh `durable` bằng không chưa có ca |
# check-scope-table: số câu bỏ qua 4 (mốc 4) — không tăng
```

Số bỏ qua tăng so với mốc ⇒ in **cảnh báo** nổi bật (không làm đỏ, đúng ý anh). Mốc ghi ở
`v2/agent/JOURNAL/scope-table-skip-count.txt`, chỉ ghi khi số **đổi**. `QUALIFIERS` là hằng số
trong code, mỗi từ khoá có lý do ngay cạnh, kèm chú thích "thêm một từ khoá là **nới lỏng** phép
kiểm". **Mười chín ca tự kiểm.**

## 4.2 Hai lỗi xanh giả tìm được trong checker của tôi

**(i) Hex bị cắt thành số thập phân.** `field_value` dùng `(?![0-9])`, nên `observed=0x74310001`
khớp thành `observed=0`. Bảng khai `observed` = không, checker "xác nhận" bằng một giá trị **không
tồn tại**. Đúng lớp lỗi tệp này sinh ra để chặn. Đã sửa.

**(ii) Cắt câu một đầu.** Hàm cắt câu chỉ cắt **đầu**, nên một từ khoá **sau** câu khẳng định trên
cùng dòng nuốt luôn câu đó. Đã sửa: cắt hai đầu theo `\n`, `. `, `; ` (thêm `; ` làm bộ lọc **hẹp**
lại, không phải nới).

## 4.3 Hai chỗ trong tài liệu cần anh biết (tôi **không** tự sửa)

- **`k6-exec.md` dòng 271** — dòng bảng `observed` = "không" là **mập mờ**: khối §2.3 ghi
  `observed=0x74310001`, chỉ khối §7 mới có `observed=0`. Checker nay resolve sang §7 nên vẫn
  xanh, nhưng người đọc tra khối đầu tiên sẽ thấy mâu thuẫn.
- **`k6-persist.md` dòng 76** — câu "…⇒ `match` bằng không và `mismatched` khác không" khẳng định
  hai giá trị bộ đếm, nhưng **không khối nào** trong tài liệu trích chúng: log của ca âm nằm ở
  `/tmp`, không trong repo. Không phải số sai — là **chưa được chứng minh bằng khối**.

---

# 5. Selftest — hai việc, và **một phát hiện nặng hơn cả hai việc**

## 5.1 Đã làm

- `persist.sh --red-selftest` vào `gate --fast` (chạy được, xanh).
- `v2/tools/check-selftest-wiring.py` vào `v2 check`: quét repo, script tự kiểm nào không cổng nào
  gọi ⇒ **ĐỎ**. Chín ca tự kiểm, có ca "chỉ nhắc trong chú thích thì không tính" và ca "cổng chỉ
  nhắc trong chú thích thì vẫn mồ côi". Đã chứng minh đỏ thật: gỡ dòng gọi `shell-edge.sh
  --selftest` ⇒ đỏ đúng tên script đó.
- Kết quả hiện tại: **mười một script tự kiểm, mười một có cổng, không mồ côi, không ngoại lệ.**

## 5.2 Phát hiện: một bước cổng mang nhãn "selftest" mà **không kiểm gì**

`v2/test/rules/counter-evidence.sh` **không hề** xử lý `--selftest` — nó chỉ nhận `--root`. Nhưng
`v2/bin/v2` gọi `counter-evidence.sh --selftest` với nhãn "selftest counter-evidence (KT-80)". Cờ bị
bỏ qua **im lặng**, nên bước đó chỉ chạy lại phép kiểm thường. Đúng lớp "xanh giả" mà anh cảnh báo.

Đã vá: thêm `--selftest` thật với bốn ca (dương + ba âm: log xanh không có giá trị không, log âm
chỉ có không, thiếu log âm). Chạy: `# counter-evidence-selftest: PASS (4/4 ca)`.

## 5.3 Danh sách "chín cái" của tôi sai và thiếu

Công cụ mới cho thấy danh sách tôi báo ở phiếu trước **sai một mục và thiếu năm mục**:

- **Sai**: `counter-evidence.sh` không có chế độ tự kiểm (xem §5.2) — nên "chín tìm được, tám nối"
  thực ra là **chín dòng được nối, tám cái là tự kiểm thật**.
- **Thiếu**: `persist.sh` (`--red-selftest`), `check-algo-pin.sh` (mới tạo trong lượt), và **ba
  script tự kiểm độc lập** `v2/test/rules/selfbreak.sh`, `v2/tools/git-guard/neg-test.sh`,
  `v2/tools/git-guard/neg-test-kt85b.sh` — cùng lớp lỗi, **hiện vẫn mồ côi**.
- **Thiếu**: `v2/tools/verify-numbers.py --self-test` chỉ được `selfbreak.sh` gọi, mà `selfbreak.sh`
  không có cổng ⇒ hiện **không cổng nào chạm tới**.

Ba script độc lập kia hiện **chỉ được báo cáo, không bị coi là mồ côi đỏ** (công cụ có cờ
`--standalone-red` để chặn hẳn). **Đề nghị anh quyết**: nối ba cái đó vào cổng, hay thêm ngoại lệ có
lý do, hay bật `--standalone-red` và để chúng đỏ cho tới khi nối.

---

# 6. Trạng thái và việc kế tiếp

**HEAD** `f50c6dea`, đã push, cây sạch. `v2 check` PASS, `gate --fast` PASS.
**Task**: DONE 129 · READY 3 · BLOCKED 0.

| Việc | Trạng thái |
|---|---|
| Q4 bảng chín lệnh + ca thiếu | **XONG**, đã chạy trên ảnh sản phẩm |
| C1 chốt `VBREV_ALGO` | **XONG**, kèm phát hiện và sửa `v2/bin` |
| C2 soak một nghìn vòng | **XONG**, phụ lục A trong văn bản đóng |
| C7 đếm câu bỏ qua | **XONG**, kèm sửa hai lỗi xanh giả |
| Selftest: persist vào gate | **XONG** |
| Selftest: kiểm chống mồ côi | **XONG**, kèm vá `counter-evidence.sh` |
| **KT-136** soak đĩa thật (C5) | **READY** — việc kế tiếp |
| **KT-81** thiết bị giả nghiêm ngặt | READY |
| **K7** (KT-137 `cd`/`pwd` để cuối) | READY |

**Ba câu hỏi tôi cần anh trả lời** (không chặn việc, nhưng nên chốt):

1. **Nhúng `VBREV_ALGO` vào dòng log/ảnh** — có mở task riêng không? (§2.3)
2. **Ba script tự kiểm độc lập còn mồ côi** — nối, ngoại lệ, hay bật `--standalone-red`? (§5.3)
3. **Hai chỗ tài liệu mập mờ** — sửa `k6-exec.md:271` và bổ sung khối chứng minh cho
   `k6-persist.md:76`? (§4.3)
