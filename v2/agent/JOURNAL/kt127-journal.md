# KT-127 — `fault_seen=1` là gì, và vì sao "1" là giá trị mong đợi

**Ngày**: 2026-11-18 · **Nguồn**: quyết định Agent 0 ngày 18/11, **Q1 điều kiện 1** —
"Giải thích `fault_seen=1` trong dòng `# product halt` của mọi vòng soak".
**Phạm vi ghi**: `v2/agent/JOURNAL/kt90-soak-product-image.sh` (checker), `v2/agent/**` (sổ).
**Không** sửa mã kernel, không sửa `spec/`, không đổi ngưỡng.

---

## 1. Câu trả lời ngắn

`fault_seen=1` **không phải lỗi**. Nó là **`Exit` của chính shell** đi qua **kênh fault** — đúng
thiết kế **D2-25** ("Exit là fault, không phải Reply"). Mỗi boot có **đúng một** tiến trình thoát
(shell), nên giá trị mong đợi là **đúng 1**.

Ba thứ nó **không** phải:

- **không** phải page fault — page fault là **fatal** trong runtime này (`Stop::Fault` → `fail(...)`),
  không đi qua đường đếm này;
- **không** phải crash — boot vẫn in `# shell: exit code=0` rồi `# product halt:` và QEMU tự thoát 0;
- **không** phải "ít nhất một" — 0 nghĩa là giao thức Exit đã đứt, 2 nghĩa là có fault thứ hai bị che.

## 2. Chuỗi sinh ra con số (đọc từ mã, không suy đoán)

```text
shell gọi syscall 14 `Exit(0)`
  → core ghi `Tcb.last_fault = FaultKind::Exit{code}` + TCB `Inactive`        (D2-25)
  → kernel `service()` đọc `take_last_fault` rồi gửi `FAULT_EXIT` vào fault_ep
  → procsrv `recv(EP_FAULT)` in `# proc: recv exit pid=… code=…` rồi gửi `EV_FAULT`
  → kernel `service()` nhận `EV_FAULT` ⇒ `svc.faults += 1` ⇒ in `fault_seen=1`
```

Dẫn chứng mã (đọc, **không** sửa):

| Mắt xích | Tệp:dòng |
|---|---|
| `EV_FAULT => svc.faults += 1` | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:219` |
| in `fault_seen={}` = `svc.faults` | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:573,575` |
| procsrv in `# proc: recv exit` + gửi `EV_FAULT` | `v2/kernel/vifast-kbin-riscv64/init/procsrv_user.rs:142` |
| kernel chuyển tiếp `FAULT_EXIT` khi thấy `FaultKind::Exit` | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:230-238` |

**Vì sao đúng 1, không phải nhiều hơn**: mỗi boot chỉ có **một** tiến trình gọi `Exit` (shell).
`kill` dùng `Tcb.Suspend(Abort)` — suspend **không** sinh fault. Victim trong soak không bị kill
(`kill_suspend=0`). Vậy tổng số fault = 1.

## 3. Bằng chứng đo được (1.000 boot)

```text
$ d=v2/agent/JOURNAL/kt90-logs/run-soak-20261118-102508-72680
$ grep -h -o "fault_seen=[0-9]*" $d/boot-*.log | sort | uniq -c
   1000 fault_seen=1
$ grep -h -o "shell_exit=[0-9]*" $d/boot-*.log | sort | uniq -c
   1000 shell_exit=0
$ grep -h "proc: recv exit" $d/boot-1.log
# proc: recv exit pid=15 code=0
```

Tức: **1.000/1.000** vòng có đúng một fault, và fault đó là `Exit` của shell (pid 15, code 0).

## 4. Điều kiện mới trong checker (theo yêu cầu Agent 0)

Thêm vào `v2/agent/JOURNAL/kt90-soak-product-image.sh`:

| Kiểm | Điều kiện | Mã lỗi khi sai |
|---|---|---|
| `fault_seen` | **đúng bằng** `WANT_FAULT_SEEN=1` | `fault-seen=<n>(want=1)` |
| `# product halt:` | xuất hiện **đúng một** lần | `halt-line=<n>` |
| `# proc: recv exit pid=… code=0` | xuất hiện **đúng một** lần | `exit-recv=<n>` |

Hai vế sau **độc lập** với vế đầu: chúng chứng minh fault đếm được **là** `Exit` của shell, không
phải một fault lạ tình cờ cũng cho ra số 1.

**Sửa kèm một báo cáo sai sự thật** (phát hiện khi chạy ca đột biến): trước đây dòng `fail` **không**
mang `help=`, nên một vòng đỏ vì lý do khác vẫn bị tổng kết là `help-response=0/N` dù shell đã trả
lời. Nay dòng `fail` mang `help=<n>` và tổng kết đếm cả hai loại dòng.

## 5. Đột biến phía mã — chứng minh checker bắt được

**Đột biến**: `proc_rt.rs:219` `EV_FAULT => svc.faults += 1` → `+= 2` (đếm đôi).

```text
$ VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
    bash v2/tools/repro-build.sh --target-dir /tmp/kt127-mut
REPRO … sha256=03fb176b25306282b1c75aceb1d9795c8fc764abddd8920becfa55635ef859f7 size=205440
      inputs=8b5d76c4fdeaf7bb dirty=1 rev=f4d1dd5f6904 build=35067589b4b3dea5 text=130072

$ bash v2/agent/JOURNAL/kt90-soak-product-image.sh --dry --image=/tmp/kt127-mut/…/vifast-kbin-riscv64
#   fail 1 lane=1 why=fault-seen=2(want=1) help=15 shell_polls=2 help_polls=0 halt_polls=1 …
# kt90: FAIL (1/1 vòng đỏ)
```

**Đối chứng dương** (ảnh thật, sau khi revert đột biến):

```text
$ bash v2/agent/JOURNAL/kt90-soak-product-image.sh --dry --image=v2/artifacts/a0287e4b….bin
# timeout-shell=0 panic=0 help-response=1/1 rx_bytes_min=10
# kt90: 1/1 vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch
# kt90: PASS
```

Đột biến đã **revert**; `git diff` chỉ còn tệp checker.

## 6. Kết luận trình Agent 0

`fault_seen=1` **giải thích được** và **là giá trị mong đợi**; checker nay **fail-closed** trên nó
và đã có **đột biến phía mã** chứng minh nó đỏ khi sai. Điều kiện 1 của Q1 **đã xong**.
