# K6 — chỉ mục bằng chứng (bàn giao cho Agent 0)

**Mục đích**: một chỗ để Agent 0 đối chiếu từng tiêu chí K6 với **log thật + dòng thật**, không phải
đọc lại hàng nghìn dòng journal. Tài liệu này **không** tuyên bố K6 đạt — việc đó thuộc Agent 0.

Quy ước: mọi số đo nằm trong log, không chép vào đây (luật một). Tham chiếu dạng `tệp:dòng` trỏ tới
dòng **in ra số** trong `v2/agent/JOURNAL/`.

## 1. Ảnh được đo

| Trường | Giá trị | Ở đâu |
|---|---|---|
| Ảnh phát hành | `291502d2…0353` | `kt88-journal.md` §0; `kt90-logs/kt90-soak-1000.log:8` |
| HEAD lúc đo | `cf96eb9b589d`, cây sạch | `kt88-22-freeze-run2.log:41` |
| Dòng danh tính trong ảnh | `# build: rev=cf96eb9b589d` (cây sạch) | `kt90-logs/kt90-soak-1000.log:9` |

**Cảnh báo thời điểm**: ảnh trên build **trước** KT-89 (đổi danh tính sang dấu vân tay build-inputs),
KT-92 (remap đường dẫn) và KT-93 (mở rộng build-inputs). Build lại từ HEAD hôm nay sẽ ra **sha khác**
— đó là hệ quả có chủ ý, **không** phải hồi quy. Mọi phép đo mới phải dùng ảnh mới.

### Ảnh canonical **mới nhất** (khớp HEAD)

| Trường | Giá trị | Ở đâu |
|---|---|---|
| Ảnh | `9643311d…` | `kt100-journal.md` |
| Dấu vân tay build-inputs | `95d919fb5b882d24`, cây sạch | `kt100-journal.md` |
| Parity sáu lệnh | sáu trên sáu, `not_ok` bằng không | `kt100-journal.md` |
| Soak một nghìn vòng | một nghìn trên một nghìn, ảnh không đổi | `kt99-journal.md` |
| Ma trận + persist | hai mươi mốt trên hai mươi mốt, và PASS | `kt97-journal.md` |
| Conformance | `trace-compare` sáu mươi mốt trên sáu mươi mốt dòng khớp | `kt101-journal.md` |

⇒ Nếu Agent 0 chọn phương án (b) ở `K6-CLOSURE-DRAFT.md` §2, **bằng chứng đã sẵn trên ảnh khớp
HEAD** — không phải đo lại.

## 2. Sáu tiêu chí

| # | Tiêu chí | Bằng chứng (dòng in số) | Kết luận của người đo |
|---|---|---|---|
| TC một | Boot ảnh phát hành ở ba cấu hình hart, tới `shell>` | `boot-final-smp1.log:68-89`; `boot-final-smp2.log:68`; `boot-final-smp4.log:68`; cổng rev `kt88-22-freeze-run2.log:41-55` | ĐẠT |
| TC hai | Parity sáu lệnh `active` ở hai cấu hình hart | `kt88-22-freeze-run2.log:67-118` | ĐẠT |
| TC ba | Ca âm: không shell; không FS | `kt88-10b-negatives-frozen.log:29-93` | ĐẠT |
| TC bốn (a) | Ma trận `ktest.sh` bảy ảnh × ba cấu hình | `kt88-22-freeze-run2.log:256-280` | ĐẠT |
| TC bốn (b) | Cộng ba boot ảnh phát hành vào ma trận | `kt88-22-freeze-run2.log:41-55` | ĐẠT |
| TC bốn (c) | Soak `soak.sh` | `kt88-22-freeze-run2.log:1304-1308` | ĐẠT — **xem §3** |
| TC năm | Ba ngưỡng | ảnh `kt88-22-freeze-run2.log:130-131`; `Machine` `ktest-matrix/ktest-smp1-1.log:253` | ĐẠT |
| TC sáu | Harness trên ảnh phát hành | `kt88-22-freeze-run2.log:139-153` | ĐẠT |

## 3. Ba hạn chế phải nói trước khi kết luận

**(a) Soak của KT-88 chạy trên ảnh KHÔNG feature.** `v2/test/boot/soak.sh:172` build **không**
`--features`, nên "không lỗi trên một nghìn vòng" là ảnh sản phẩm **không shell, không FS** — khác ảnh phát hành.
Bằng chứng bổ sung: **KT-90** soak **một nghìn trên một nghìn** trên **đúng** ảnh phát hành, mỗi vòng có `shell>` +
`help` + `exit` sạch — `kt90-logs/kt90-soak-1000.log:1016-1020`; journal
`kt90-journal.md`. **Agent 0 quyết định** tiêu chí bốn dùng số nào.

**(b) Soak không gắn `-drive`.** Volume FS trong soak là RAM (`medium=ram`, cờ `volatile` bật). Đường **đĩa
thật** mới có hai boot + persist (`kt88-journal.md` §2b), không phải một nghìn vòng.

**(c) Ma trận là ảnh test, không phải ảnh phát hành.** Đúng thiết kế: ma trận đo bảy ảnh test; ảnh
phát hành được đo riêng ở tiêu chí một, hai, năm, sáu và KT-90.

## 4. Cách tái lập (không cần đọc journal)

```bash
./v2/bin/v2 check                 # fmt + clippy + luật + L0
./v2/bin/v2 check-rules           # năm luật máy
bash v2/test/boot/ktest.sh        # ma trận
bash v2/test/parity/run.sh --mode serial --image product
bash v2/agent/JOURNAL/kt90-soak-product-image.sh --pilot <n>   # pilot nhanh
```

## 5. Việc còn lại trước khi K6 có thể được tuyên bố

| Việc | Chờ gì |
|---|---|
| KT-82 — văn bản đóng K6 | Agent 0 chọn số cho tiêu chí bốn (§3a) |
| KT-83 — bảng lỗi v1 | KT-82 |
| KT-91 — ba lệnh shell `time`/`uptime`/`jobs` | Agent 0 duyệt đổi `# shell: help cmds=` (assert trong `proc_demo.rs:761`, `ktest.sh:709`) |
| KT-81 — thiết bị giả nghiêm ngặt | theo D2-119: làm **sau** khi K6 đóng |
