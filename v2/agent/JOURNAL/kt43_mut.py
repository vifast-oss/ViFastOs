# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-43 — harness mutation tay (17 ca) — bản lưu trong repo để tái lập; không phải mã sản phẩm.
# Chạy: python3 v2/agent/JOURNAL/kt43_mut.py (từ gốc repo; cần CARGO trong PATH ~/.cargo/bin).
# Harness đột biến tay KT-43 — KHÔNG commit; chạy sau khi `nightly --l1` xong (§6).
# Mỗi ca: xanh trước (đúng test) → sửa đúng một chỗ → đỏ đúng test + assert → khôi phục → hash.
import hashlib
import os
import subprocess
import sys
import time

ROOT = os.environ.get("VIFAST_ROOT", "/Users/mps/dvchd/ViFastOs")
LOG = f"{ROOT}/v2/agent/JOURNAL/kt43-mutation.log"
CARGO = "/Users/mps/.cargo/bin/cargo"
MAN = f"{ROOT}/v2/Cargo.toml"

ipc = "v2/sim/vifast-sim/src/ipc.rs"
kobj = "v2/sim/vifast-sim/src/kobj.rs"
pt = "v2/sim/vifast-sim/src/pagetable.rs"
mbox = "v2/lib/vifast-sync/src/mailbox.rs"


def T(pkg, test_bin, fn):
    return ("vifast-sim" if pkg == "sim" else "vifast-sync", test_bin, fn)


CASES = [
    # ---------------- 12 ca §7.2 cũ (K1C) — chạy lại tại HEAD K2 ----------------
    dict(
        id="M1",
        ten="ipc::transfer bỏ copy 1 trong 64 MR",
        file=ipc,
        old="    for (i, word) in msg.mr[..len].iter().enumerate() {\n        if i < WORDS_IN_REGISTERS {",
        new="    for (i, word) in msg.mr[..len].iter().enumerate() {\n        if i == 32 {\n            continue; // M1: bỏ copy 1/64 MR\n        }\n        if i < WORDS_IN_REGISTERS {",
        pkg="vifast-sim",
        test_bin="kt24_ipc",
        test_fn="message_64_mr_hai_vung_thanh_ghi_va_ipc_buffer",
    ),
    dict(
        id="M2",
        ten="Bỏ kiểm RIGHT_GRANT khi chuyển cap",
        file=ipc,
        old="        if c.rights & crate::kobj::RIGHT_GRANT == 0 || occupied[i] {",
        new="        if occupied[i] { // M2: bỏ kiểm RIGHT_GRANT",
        pkg="vifast-sim",
        test_bin="kt25_ipc2",
        test_fn="thieu_grant_that_bai_mem_message_van_giao",
    ),
    dict(
        id="M3",
        ten="Cap nhận giữ RIGHT_GRANT (không bỏ GRANT)",
        file=kobj,
        old="            let rights = snap.rights & !RIGHT_GRANT;",
        new="            let rights = snap.rights; // M3: giữ GRANT trên cap nhận",
        pkg="vifast-sim",
        test_bin="kt25_ipc2",
        test_fn="bon_cap_cai_du_bo_grant_giu_badge",
    ),
    dict(
        id="M4",
        ten="Bỏ unwrap badge (cap badge vào receive window)",
        file=ipc,
        old="""        if let (Some(ep), Some(badge)) = (endpoint, c.badge) {
            if ep == c.obj {
                // Unwrap badge: không chuyển cap, badge OR vào message (thiết kế `:319`).
                plan.unwrap_badges.push(badge);
                continue;
            }
        }
""",
        new="        // M4: bỏ nhánh unwrap badge\n        let _ = endpoint;\n",
        pkg="vifast-sim",
        test_bin="kt25_ipc2",
        test_fn="unwrap_badge_khong_chiem_receive_window",
    ),
    dict(
        id="M5",
        ten="Signal không xoá word/badge sau khi giao",
        file=kobj,
        old="""        self.unlink_tcb_queues(target);
        self.transition_tcb(target, TcbState::Ready)?;
        self.body_mut(ntfn, |k| match k {
            Kind::Notification(n) => {
                n.word = 0;
                n.badge = 0;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        })?;
""",
        new="        self.unlink_tcb_queues(target);\n        self.transition_tcb(target, TcbState::Ready)?;\n        // M5: KHÔNG xoá word/badge sau khi giao\n",
        pkg="vifast-sim",
        test_bin="kt27_notification",
        test_fn="mot_waiter_duy_nhat_ba_tcb",
    ),
    dict(
        id="M6",
        ten="Bỏ cổng 'một waiter' kiểu v1",
        file=kobj,
        old="        if busy {\n            return Err(KError::Busy);\n        }",
        new="        // M6: bỏ cổng một-waiter\n        let _ = busy;",
        pkg="vifast-sim",
        test_bin="kt27_notification",
        test_fn="mot_waiter_duy_nhat_ba_tcb",
    ),
    dict(
        id="M7",
        ten="WaitAddr xếp hàng trước rồi mới so expected",
        file=kobj,
        old="""        let current = self.addr_word(frame, offset);
        if current != expected {
            return Err(KError::InvalidArgument);
        }
        // Chuyển trạng thái **trước** khi push: `transition_tcb` có thể từ chối cặp (bảng
        // 11×11) và không được để nút mồ côi; push không lỗi ⇒ hàng và trạng thái luôn khớp.
        self.transition_tcb(tcb_obj, TcbState::BlockedOnAddress { frame, offset })?;
        let bucket = waitaddr_bucket(frame, offset);
        self.address_waiters[bucket].push((frame, offset, tcb_obj));
""",
        new="""        let current = self.addr_word(frame, offset);
        // M7: xếp hàng TRƯỚC rồi mới so expected
        self.transition_tcb(tcb_obj, TcbState::BlockedOnAddress { frame, offset })?;
        let bucket = waitaddr_bucket(frame, offset);
        self.address_waiters[bucket].push((frame, offset, tcb_obj));
        if current != expected {
            return Err(KError::InvalidArgument);
        }
""",
        pkg="vifast-sim",
        test_bin="kt28_waitaddr",
        test_fn="expected_lech_tra_ngay_khong_xep_hang",
    ),
    dict(
        id="M8",
        ten="WakeAddr bỏ kiểm khoá (wake mọi waiter trong bucket)",
        file=kobj,
        old="""            for (f, o, t) in self.address_waiters[bucket].iter() {
                if *f == frame && *o == offset {
                    chon.push(*t);
                    if chon.len() == n as usize {
                        break;
                    }
                }
            }
""",
        new="""            for (_f, _o, t) in self.address_waiters[bucket].iter() {
                // M8: bỏ kiểm (frame, offset)
                chon.push(*t);
                if chon.len() == n as usize {
                    break;
                }
            }
""",
        pkg="vifast-sim",
        test_bin="kt28_waitaddr",
        test_fn="va_cham_bucket_khong_anh_huong_tim_kiem",
    ),
    dict(
        id="M9",
        ten="Push migration giữ home cũ (bỏ home = to)",
        file=kobj,
        old="""        let doi = self.body_mut(tcb, |k| match k {
            Kind::Tcb(t) => {
                t.home = to;
                Ok(())
            }
            _ => Err(KError::WrongObjectType),
        });""",
        new="""        let doi = self.body_mut(tcb, |k| match k {
            Kind::Tcb(_t) => Ok(()), // M9: giữ home cũ
            _ => Err(KError::WrongObjectType),
        });""",
        pkg="vifast-sim",
        test_bin="kt29_smp",
        test_fn="push_dung_mot_tcb_cuoi_hang_qua_migrate",
    ),
    dict(
        id="M10",
        ten="Fault-resume không ghi MR trước Ready (đường r319)",
        file=kobj,
        old="""        let snaps = self.resolve_caps(&msg)?;
        let mut m = self.admit_message(msg, snaps.clone())?;
        self.deliver_message(obj, &mut m, &snaps, None)?;
""",
        new="        // M10: KHÔNG ghi MR trước Ready\n",
        pkg="vifast-sim",
        test_bin="kt26_fault",
        test_fn="resume_with_regs_khi_dang_cho_reply_ghi_thanh_ghi_truoc_ready",
    ),
    dict(
        id="M11",
        ten="Hộp thư đầy ghi đè thay vì Err(Busy)",
        file=mbox,
        old="""                // Slot còn thuộc vòng trước ⇒ hộp thư đầy.
                return Err(Busy);
""",
        new="""                // M11: hộp thư đầy ⇒ GHI ĐÈ message cũ (đột biến tạm).
                unsafe {
                    (*slot.value.get()).write(msg);
                }
                slot.seq.store(pos.wrapping_add(1), Release);
                self.head.store(pos.wrapping_add(1), Relaxed);
                self.generation.fetch_add(1, SeqCst);
                return Ok(());
""",
        pkg="vifast-sync",
        test_bin="kt30_sync",
        test_fn="mailbox_day_tra_busy_khong_mat_message_dau",
    ),
    dict(
        id="M12",
        ten="Rời BlockedOnReply không gỡ Reply.caller (cổng transition_tcb; guard hiện hành sau KT-34)",
        file=kobj,
        old="""        } else if let TcbState::BlockedOnReply { reply } = from {
            if self.object_alive(reply)
                && self.object_type(reply) == Some(ObjType::Reply)
                && self.reply_caller(reply)? == Some(tcb)
            {
                self.reply_set_caller(reply, None)?;
            }
        }
""",
        new="""        }
        // M12: không gỡ Reply.caller khi rời BlockedOnReply
""",
        pkg="vifast-sim",
        test_bin="kt25_ipc2",
        test_fn="cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller",
    ),
    # ---------------- 4+1 ca VM mới (K2) ----------------
    dict(
        id="V1",
        ten="Map đổi lá/thứ tự cấp (AP→BSP): level_for_size_bits đảo cấp",
        file=pt,
        old="    Some((size.size_bits() - 12) / 9)",
        new="    Some(2 - (size.size_bits() - 12) / 9) // V1: đảo cấp lá (AP→BSP)",
        pkg="vifast-sim",
        test_bin="kt37_pagetable",
        test_fn="map_walk_unmap_du_ba_co",
    ),
    dict(
        id="V2",
        ten="Bỏ kiểm ack khi unmap (trả Ok khi chưa đủ ack)",
        file=kobj,
        old="""        let mapping = match self.page_table_unmap(info.root, va) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        self.tlb_shootdown_wait(hart)?;
        Ok(mapping)""",
        new="""        let mapping = match self.page_table_unmap(info.root, va) {
            Ok(m) => m,
            Err(e) => {
                self.tlb_pending[hart as usize] = None;
                return Err(e);
            }
        };
        self.tlb_shootdown_post(hart)?;
        // V2: bỏ chờ ack khi unmap
        Ok(mapping)""",
        pkg="vifast-sim",
        test_bin="kt40_tlb",
        test_fn="shootdown_1_2_4_8_hart_ack_du",
    ),
    dict(
        id="V3",
        ten="Bỏ W^X khi map W (cho W sau X)",
        file=kobj,
        old="        if write && (exec || f.mapped_x > 0) {",
        new="        if write && exec { // V3: bỏ nhánh mapped_x > 0",
        pkg="vifast-sim",
        test_bin="kt39_wx",
        test_fn="map_w_sau_x_bi_tu_choi_atomic_khong_ha_ngam",
    ),
    dict(
        id="V4",
        ten="Bỏ flush TLB khi deactivate/teardown",
        file=kobj,
        old="            self.tlb_flush_hart_only(hart, generation, asid, TlbFlushScope::Asid);",
        new="            // V4: bỏ flush TLB khi deactivate\n            let _ = (generation, asid);",
        pkg="vifast-sim",
        test_bin="kt40_tlb",
        test_fn="flush_theo_trang_asid_va_gen",
    ),
    dict(
        id="V5",
        ten="Bỏ rút hộp thư khi chờ ack (AP→BSP deadlock)",
        file=kobj,
        old="        let _ = self.drain_mail_fifo(hart);\n        p.rounds += 1;",
        new="        // V5: bỏ drain hộp thư khi chờ ack (AP→BSP)\n        p.rounds += 1;",
        pkg="vifast-sim",
        test_bin="kt40_tlb",
        test_fn="deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu",
    ),
]


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def write_file(p, noi_dung):
    import os

    with open(p, "w", encoding="utf-8") as fh:
        fh.write(noi_dung)
        fh.flush()
        os.fsync(fh.fileno())


def run_test(pkg, test_bin, fn, log, nhan, extra_env=None):
    import os

    env = os.environ.copy()
    env["PATH"] = "/Users/mps/.cargo/bin:" + env.get("PATH", "")
    if extra_env:
        env.update(extra_env)
    cmd = [CARGO, "test", "--manifest-path", MAN, "-p", pkg, "--test", test_bin, "--", "--exact", fn]
    t0 = time.time()
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    dt = time.time() - t0
    out = r.stdout + r.stderr
    log.write(f"--- [{nhan}] {' '.join(cmd[1:])}\n")
    log.write(f"    exit={r.returncode} wall={dt:.2f}s\n")
    for line in out.splitlines():
        if (
            "test result:" in line
            or " FAILED" in line
            or " panicked at " in line
            or "assertion" in line
            or "left:" in line
            or "right:" in line
            or "error[E" in line
            or "could not compile" in line
        ):
            log.write(f"    {line.strip()}\n")
    log.flush()
    return r.returncode, out


def main():
    log = open(LOG, "a")
    log.write("\n" + "=" * 78 + "\n")
    log.write("# KT-43 — mutation tay 12 ca §7.2 cũ (K1C) + 5 ca VM K2 (gồm AP→BSP); log tự sinh\n")
    log.write(f"# HEAD={subprocess.check_output(['git','rev-parse','--short','HEAD'], cwd=ROOT, text=True).strip()} ")
    log.write(f"time={time.strftime('%Y-%m-%d %H:%M:%S')}\n")
    log.flush()
    ket = []
    for c in CASES:
        f = f"{ROOT}/{c['file']}"
        goc = open(f, "r", encoding="utf-8").read()
        h0 = sha(f)
        n = goc.count(c["old"])
        log.write("\n" + "=" * 78 + "\n")
        log.write(f"## {c['id']} — {c['ten']}\n")
        log.write(f"File : {c['file']}\n")
        log.write(f"Test : {c['pkg']}::{c['test_bin']}::{c['test_fn']}\n")
        log.write(f"hash HEAD:{c['file']} = {h0}\n")
        if n != 1:
            log.write(f">>> BỎ QUA: mẫu khớp {n} lần (phải đúng 1)\n")
            ket.append((c["id"], "MẪU-KHÔNG-KHỚP", False))
            log.flush()
            continue
        # 1) xanh trước
        rc, out = run_test(c["pkg"], c["test_bin"], c["test_fn"], log, "xanh trước")
        xanh = rc == 0 and "test result: ok" in out and "1 passed" in out
        log.write(f"    xanh truoc: {'OK' if xanh else 'KHÔNG'}\n")
        if not xanh:
            ket.append((c["id"], "XANH-TRƯỚC-FAIL", False))
            log.flush()
            continue
        # 2) áp đột biến (ghi + fsync; kiểm cây sạch trước khi áp)
        truoc = subprocess.run(
            ["git", "status", "--porcelain", "--", c["file"]], cwd=ROOT, capture_output=True, text=True
        ).stdout.strip()
        if truoc != "":
            log.write(f">>> BỎ QUA: tệp đang bẩn trước khi áp: {truoc}\n")
            ket.append((c["id"], "TỆP-BẨN-TRƯỚC", False))
            log.flush()
            continue
        write_file(f, goc.replace(c["old"], c["new"]))
        # 3) chạy đỏ
        rc, out = run_test(c["pkg"], c["test_bin"], c["test_fn"], log, "đỏ")
        do = (
            rc != 0
            and "test result: FAILED" in out
            and f"{c['test_fn']} ... FAILED" in out
            and "could not compile" not in out
        )
        log.write(f"    do dung test: {'OK' if do else 'KHÔNG'}\n")
        # 4) khôi phục + hash
        write_file(f, goc)
        h1 = sha(f)
        sach = subprocess.run(
            ["git", "status", "--porcelain", "--", c["file"]], cwd=ROOT, capture_output=True, text=True
        ).stdout.strip()
        log.write(f"    khôi phục: hash {'KHỚP' if h1 == h0 else 'LỆCH'}; git sạch: {'OK' if sach == '' else sach}\n")
        log.flush()
        ket.append((c["id"], "ĐỎ-ĐÚNG-TEST" if do else "SỐNG", do and h1 == h0 and sach == ""))
    log.write("\n" + "=" * 78 + "\nTỔNG KẾT {} CA:\n".format(len(ket)))
    for i, (cid, kq, ok) in enumerate(ket, 1):
        log.write(f"  {cid}: {kq} | restore/clean={ok}\n")
    nk = sum(1 for _, _, ok in ket if ok)
    log.write(f"  → {nk}/{len(ket)} ca đỏ đúng test + khôi phục hash == HEAD\n")
    log.close()
    print(f"{nk}/{len(ket)} killed")
    return 0 if nk == len(ket) else 1


if __name__ == "__main__":
    sys.exit(main())
