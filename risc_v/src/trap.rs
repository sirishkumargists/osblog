// trap.rs
// Trap routines
// Stephen Marz
// 10 October 2019

use crate::{cpu::{TrapFrame, CONTEXT_SWITCH_TIME},
            plic,
            rust_switch_to_user,
            sched::schedule,
            syscall::do_syscall};

#[no_mangle]
/// The m_trap stands for "machine trap". Right now, we are handling
/// all traps at machine mode. In this mode, we can figure out what's
/// going on and send a trap where it needs to be. Remember, in machine
/// mode and in this trap, interrupts are disabled and the MMU is off.
extern "C" fn m_trap(epc: usize,
                     tval: usize,
                     cause: usize,
                     hart: usize,
                     _status: usize,
                     frame: *mut TrapFrame)
                     -> usize
{
	// We're going to handle all traps in machine mode. RISC-V lets
	// us delegate to supervisor mode, but switching out SATP (virtual memory)
	// gets hairy.
	let is_async = {
		if cause >> 63 & 1 == 1 {
			true
		}
		else {
			false
		}
	};
	// The cause contains the type of trap (sync, async) as well as the cause
	// number. So, here we narrow down just the cause number.
	let cause_num = cause & 0xfff;
	let mut return_pc = epc;
	if is_async {
		// Asynchronous trap
		match cause_num {
			3 => {
				// We will use this to awaken our other CPUs so they can process
				// processes.
				println!("Machine software interrupt CPU #{}", hart);
			},
			7 => {
				// This is the context-switch timer.
				// We would typically invoke the scheduler here to pick another
				// process to run.
				// Machine timer
				let frame = schedule();
				// let p = frame as *const TrapFrame;
				// println!(
				// 		 "CTX Startup {}, pc = {:x}",
				// 		 (*p).pid,
				// 		 (*p).pc
				// );
				// print!("   ");
				// for i in 1..32 {
				// 	if i % 4 == 0 {
				// 		println!();
				// 		print!("   ");
				// 	}
				// 	print!("{:2}:{:08x}   ", i, (*p).regs[i]);
				// }
				// println!();
				schedule_next_context_switch(1);
				rust_switch_to_user(frame);
			},
			11 => {
				// Machine external (interrupt from Platform Interrupt Controller (PLIC))
				// println!("Machine external interrupt CPU#{}", hart);
				// We will check the next interrupt. If the interrupt isn't available, this will
				// give us None. However, that would mean we got a spurious interrupt, unless we
				// get an interrupt from a non-PLIC source. This is the main reason that the PLIC
				// hardwires the id 0 to 0, so that we can use it as an error case.
				plic::handle_interrupt();
			},
			_ => {
				panic!("Unhandled async trap CPU#{} -> {}\n", hart, cause_num);
			},
		}
	}
	else {
		// Synchronous trap
		match cause_num {
			2 => {
				// Illegal instruction
				panic!("Illegal instruction CPU#{} -> 0x{:08x}: 0x{:08x}\n", hart, epc, tval);
				// We need while trues here until we have a functioning "delete from scheduler"
				// I use while true because Rust will warn us that it looks stupid.
				// This is what I want so that I remember to remove this and replace
				// them later.
				loop {}
			},
			8 | 9 | 11 => unsafe {
				// Environment (system) call from User, Supervisor, and Machine modes
				// println!("E-call from User mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc = do_syscall(return_pc, frame);
				if return_pc == 0 {
					// We are about to schedule something else here, so we need to store PAST
					// the system call so that when we resume this process, we're after the ecall.
					(*frame).pc += 4;
					let frame = schedule();
					// let p = frame as *const TrapFrame;
					// println!(
					// 	"SYC Startup {}, pc = {:x}",
					// 	(*p).pid,
					// 	(*p).pc,
					// );
					// print!("   ");
					// for i in 1..32 {
					// 	if i % 4 == 0 {
					// 		println!();
					// 		print!("   ");
					// 	}
					// 	print!("{:2}:{:08x}   ", i, (*p).regs[i]);
					// }
					// println!();
					schedule_next_context_switch(1);
					rust_switch_to_user(frame);
				}
			},
			// Page faults
			12 => {
				// Instruction page fault
				println!("Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				// We need while trues here until we have a functioning "delete from scheduler"
				loop {}
				return_pc += 4;
			},
			13 => {
				// Load page fault
				println!("Load page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				// We need while trues here until we have a functioning "delete from scheduler"
				loop {}
				return_pc += 4;
			},
			15 => {
				// Store page fault
				println!("Store page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				// We need while trues here until we have a functioning "delete from scheduler"
				loop {}
				return_pc += 4;
			},
			_ => {
				panic!("Unhandled sync trap {}. CPU#{} -> 0x{:08x}: 0x{:08x}\n", cause_num, hart, epc, tval);
			},
		}
	};
	// Finally, return the updated program counter
	return_pc
}

pub const MMIO_MTIMECMP: *mut u64 = 0x0200_4000usize as *mut u64;
pub const MMIO_MTIME: *const u64 = 0x0200_BFF8 as *const u64;

pub fn schedule_next_context_switch(qm: u16) {
	unsafe {
		MMIO_MTIMECMP.write_volatile(MMIO_MTIME.read_volatile().wrapping_add(CONTEXT_SWITCH_TIME * qm as u64));
	}
}