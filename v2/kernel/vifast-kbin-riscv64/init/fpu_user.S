# SPDX-License-Identifier: Apache-2.0 OR MIT
# **KT-418** — chương trình U-mode **dấu phẩy động**: nạp `f0..f31` bằng mẫu **riêng của tác vụ**
# (`(task << 32) | (0x100 + reg)`), `Yield` 8 lần; sau **mỗi** lần được resume: **kiểm lại** toàn
# bộ 32 thanh ghi rồi **ghi lại** mẫu (ghi ⇒ phần cứng đặt `sstatus.FS = Dirty` ⇒ kernel **buộc**
# phải lưu ở lượt ra, nên đường "lưu" cũng được đo chứ không chỉ đường "khôi phục").
#
# Đây là nửa U-mode của bất biến "2 tác vụ dấu phẩy động không lẫn thanh ghi": nếu kernel không
# lưu/khôi phục ngữ cảnh FPU theo TCB thì tác vụ kia ghi đè và `bne` nhảy `3f` ⇒ `Exit(1)`.
#
# ABI `spec/abi.toml` `[calling.riscv64]`: `a7` = số hiệu, `a0` = tham số; trả `a0` = error.
# Chỉ dùng thanh ghi **caller-saved** (`a`/`t`) — trapframe 288 B của kernel lưu đủ 32 GPR nên
# `t3..t6` sống qua `ecall`, và đường về S-mode của kernel không phụ thuộc `s0..s11`.
#
# `a0` lúc vào = **task id** (1 hoặc 2) do kernel đặt trong trapframe; 0 ⇒ mặc định 1.
    .section .text.entry,"ax"
    .globl _start
    .balign 4
_start:
    bnez a0, 1f
    li a0, 1
1:
    mv t3, a0              # t3 = task id
    li t4, 0               # t4 = số vòng đã yield
    li t5, 8               # 8 vòng

    # --- nạp f0..f31 bằng mẫu của **tác vụ này**
    slli t6, t3, 32
    li a3, 256
    or t6, t6, a3
    fmv.d.x f0, t6
    slli t6, t3, 32
    li a3, 257
    or t6, t6, a3
    fmv.d.x f1, t6
    slli t6, t3, 32
    li a3, 258
    or t6, t6, a3
    fmv.d.x f2, t6
    slli t6, t3, 32
    li a3, 259
    or t6, t6, a3
    fmv.d.x f3, t6
    slli t6, t3, 32
    li a3, 260
    or t6, t6, a3
    fmv.d.x f4, t6
    slli t6, t3, 32
    li a3, 261
    or t6, t6, a3
    fmv.d.x f5, t6
    slli t6, t3, 32
    li a3, 262
    or t6, t6, a3
    fmv.d.x f6, t6
    slli t6, t3, 32
    li a3, 263
    or t6, t6, a3
    fmv.d.x f7, t6
    slli t6, t3, 32
    li a3, 264
    or t6, t6, a3
    fmv.d.x f8, t6
    slli t6, t3, 32
    li a3, 265
    or t6, t6, a3
    fmv.d.x f9, t6
    slli t6, t3, 32
    li a3, 266
    or t6, t6, a3
    fmv.d.x f10, t6
    slli t6, t3, 32
    li a3, 267
    or t6, t6, a3
    fmv.d.x f11, t6
    slli t6, t3, 32
    li a3, 268
    or t6, t6, a3
    fmv.d.x f12, t6
    slli t6, t3, 32
    li a3, 269
    or t6, t6, a3
    fmv.d.x f13, t6
    slli t6, t3, 32
    li a3, 270
    or t6, t6, a3
    fmv.d.x f14, t6
    slli t6, t3, 32
    li a3, 271
    or t6, t6, a3
    fmv.d.x f15, t6
    slli t6, t3, 32
    li a3, 272
    or t6, t6, a3
    fmv.d.x f16, t6
    slli t6, t3, 32
    li a3, 273
    or t6, t6, a3
    fmv.d.x f17, t6
    slli t6, t3, 32
    li a3, 274
    or t6, t6, a3
    fmv.d.x f18, t6
    slli t6, t3, 32
    li a3, 275
    or t6, t6, a3
    fmv.d.x f19, t6
    slli t6, t3, 32
    li a3, 276
    or t6, t6, a3
    fmv.d.x f20, t6
    slli t6, t3, 32
    li a3, 277
    or t6, t6, a3
    fmv.d.x f21, t6
    slli t6, t3, 32
    li a3, 278
    or t6, t6, a3
    fmv.d.x f22, t6
    slli t6, t3, 32
    li a3, 279
    or t6, t6, a3
    fmv.d.x f23, t6
    slli t6, t3, 32
    li a3, 280
    or t6, t6, a3
    fmv.d.x f24, t6
    slli t6, t3, 32
    li a3, 281
    or t6, t6, a3
    fmv.d.x f25, t6
    slli t6, t3, 32
    li a3, 282
    or t6, t6, a3
    fmv.d.x f26, t6
    slli t6, t3, 32
    li a3, 283
    or t6, t6, a3
    fmv.d.x f27, t6
    slli t6, t3, 32
    li a3, 284
    or t6, t6, a3
    fmv.d.x f28, t6
    slli t6, t3, 32
    li a3, 285
    or t6, t6, a3
    fmv.d.x f29, t6
    slli t6, t3, 32
    li a3, 286
    or t6, t6, a3
    fmv.d.x f30, t6
    slli t6, t3, 32
    li a3, 287
    or t6, t6, a3
    fmv.d.x f31, t6

2:  # --- vòng: Yield, kiểm lại **toàn bộ** 32 thanh ghi, rồi ghi lại mẫu (đặt FS = Dirty)
    li a7, 8
    ecall
    slli t6, t3, 32
    li a3, 256
    or t6, t6, a3
    fmv.x.d a3, f0
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 257
    or t6, t6, a3
    fmv.x.d a3, f1
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 258
    or t6, t6, a3
    fmv.x.d a3, f2
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 259
    or t6, t6, a3
    fmv.x.d a3, f3
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 260
    or t6, t6, a3
    fmv.x.d a3, f4
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 261
    or t6, t6, a3
    fmv.x.d a3, f5
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 262
    or t6, t6, a3
    fmv.x.d a3, f6
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 263
    or t6, t6, a3
    fmv.x.d a3, f7
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 264
    or t6, t6, a3
    fmv.x.d a3, f8
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 265
    or t6, t6, a3
    fmv.x.d a3, f9
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 266
    or t6, t6, a3
    fmv.x.d a3, f10
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 267
    or t6, t6, a3
    fmv.x.d a3, f11
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 268
    or t6, t6, a3
    fmv.x.d a3, f12
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 269
    or t6, t6, a3
    fmv.x.d a3, f13
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 270
    or t6, t6, a3
    fmv.x.d a3, f14
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 271
    or t6, t6, a3
    fmv.x.d a3, f15
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 272
    or t6, t6, a3
    fmv.x.d a3, f16
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 273
    or t6, t6, a3
    fmv.x.d a3, f17
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 274
    or t6, t6, a3
    fmv.x.d a3, f18
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 275
    or t6, t6, a3
    fmv.x.d a3, f19
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 276
    or t6, t6, a3
    fmv.x.d a3, f20
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 277
    or t6, t6, a3
    fmv.x.d a3, f21
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 278
    or t6, t6, a3
    fmv.x.d a3, f22
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 279
    or t6, t6, a3
    fmv.x.d a3, f23
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 280
    or t6, t6, a3
    fmv.x.d a3, f24
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 281
    or t6, t6, a3
    fmv.x.d a3, f25
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 282
    or t6, t6, a3
    fmv.x.d a3, f26
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 283
    or t6, t6, a3
    fmv.x.d a3, f27
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 284
    or t6, t6, a3
    fmv.x.d a3, f28
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 285
    or t6, t6, a3
    fmv.x.d a3, f29
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 286
    or t6, t6, a3
    fmv.x.d a3, f30
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 287
    or t6, t6, a3
    fmv.x.d a3, f31
    bne t6, a3, 3f
    slli t6, t3, 32
    li a3, 256
    or t6, t6, a3
    fmv.d.x f0, t6
    slli t6, t3, 32
    li a3, 257
    or t6, t6, a3
    fmv.d.x f1, t6
    slli t6, t3, 32
    li a3, 258
    or t6, t6, a3
    fmv.d.x f2, t6
    slli t6, t3, 32
    li a3, 259
    or t6, t6, a3
    fmv.d.x f3, t6
    slli t6, t3, 32
    li a3, 260
    or t6, t6, a3
    fmv.d.x f4, t6
    slli t6, t3, 32
    li a3, 261
    or t6, t6, a3
    fmv.d.x f5, t6
    slli t6, t3, 32
    li a3, 262
    or t6, t6, a3
    fmv.d.x f6, t6
    slli t6, t3, 32
    li a3, 263
    or t6, t6, a3
    fmv.d.x f7, t6
    slli t6, t3, 32
    li a3, 264
    or t6, t6, a3
    fmv.d.x f8, t6
    slli t6, t3, 32
    li a3, 265
    or t6, t6, a3
    fmv.d.x f9, t6
    slli t6, t3, 32
    li a3, 266
    or t6, t6, a3
    fmv.d.x f10, t6
    slli t6, t3, 32
    li a3, 267
    or t6, t6, a3
    fmv.d.x f11, t6
    slli t6, t3, 32
    li a3, 268
    or t6, t6, a3
    fmv.d.x f12, t6
    slli t6, t3, 32
    li a3, 269
    or t6, t6, a3
    fmv.d.x f13, t6
    slli t6, t3, 32
    li a3, 270
    or t6, t6, a3
    fmv.d.x f14, t6
    slli t6, t3, 32
    li a3, 271
    or t6, t6, a3
    fmv.d.x f15, t6
    slli t6, t3, 32
    li a3, 272
    or t6, t6, a3
    fmv.d.x f16, t6
    slli t6, t3, 32
    li a3, 273
    or t6, t6, a3
    fmv.d.x f17, t6
    slli t6, t3, 32
    li a3, 274
    or t6, t6, a3
    fmv.d.x f18, t6
    slli t6, t3, 32
    li a3, 275
    or t6, t6, a3
    fmv.d.x f19, t6
    slli t6, t3, 32
    li a3, 276
    or t6, t6, a3
    fmv.d.x f20, t6
    slli t6, t3, 32
    li a3, 277
    or t6, t6, a3
    fmv.d.x f21, t6
    slli t6, t3, 32
    li a3, 278
    or t6, t6, a3
    fmv.d.x f22, t6
    slli t6, t3, 32
    li a3, 279
    or t6, t6, a3
    fmv.d.x f23, t6
    slli t6, t3, 32
    li a3, 280
    or t6, t6, a3
    fmv.d.x f24, t6
    slli t6, t3, 32
    li a3, 281
    or t6, t6, a3
    fmv.d.x f25, t6
    slli t6, t3, 32
    li a3, 282
    or t6, t6, a3
    fmv.d.x f26, t6
    slli t6, t3, 32
    li a3, 283
    or t6, t6, a3
    fmv.d.x f27, t6
    slli t6, t3, 32
    li a3, 284
    or t6, t6, a3
    fmv.d.x f28, t6
    slli t6, t3, 32
    li a3, 285
    or t6, t6, a3
    fmv.d.x f29, t6
    slli t6, t3, 32
    li a3, 286
    or t6, t6, a3
    fmv.d.x f30, t6
    slli t6, t3, 32
    li a3, 287
    or t6, t6, a3
    fmv.d.x f31, t6
    addi t4, t4, 1
    blt t4, t5, 2b

    # --- xanh: Exit(0)
    li a0, 0
    li a7, 14
    ecall
    j .

3:  # --- đỏ: một thanh ghi đã bị tác vụ khác ghi đè ⇒ Exit(1)
    li a0, 1
    li a7, 14
    ecall
    j .
