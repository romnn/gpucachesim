use std::collections::VecDeque;
use std::sync::Arc;

use trace_model::MemAccessTraceEntry;

use super::instruction::WarpInstruction;
use crate::config::GPUConfig;
use bitvec::{array::BitArray, BitArr};

pub type ThreadActiveMask = BitArr!(for 32);

#[derive(Clone, Debug)]
pub struct SchedulerWarp {
    pub block_id: usize,
    pub dynamic_warp_id: usize,
    pub warp_id: usize,
    pub kernel_id: usize,
    // pub done: bool,
    // todo: what is next and trace pc??
    pub trace_pc: usize,
    pub next_pc: Option<usize>,
    pub active_mask: ThreadActiveMask,
    pub trace_instructions: VecDeque<WarpInstruction>,
    // pub trace_instructions: Vec<MemAccessTraceEntry>,
    // pub warp_traces: Vec<Mem>,
    // pub instructions: Vec<>,
    // pub kernel: Arc<super::KernelInfo>,
    // pub config: Arc<GPUConfig>,
}

impl PartialEq for SchedulerWarp {
    fn eq(&self, other: &Self) -> bool {
        self.kernel_id == other.kernel_id
            && self.block_id == other.block_id
            && self.warp_id == other.warp_id
            && self.dynamic_warp_id == other.dynamic_warp_id
    }
}

impl SchedulerWarp {
    // pub fn new(kernel: Arc<super::KernelInfo>, config: Arc<GPUConfig>) -> Self {
    pub fn new() -> Self {
        Self {
            block_id: 0,
            dynamic_warp_id: 0,
            warp_id: 0,
            kernel_id: 0,
            // done: false,
            // todo: what is next and trace pc??
            trace_pc: 0,
            next_pc: None,
            trace_instructions: VecDeque::new(),
            active_mask: BitArray::ZERO,
            // pub trace_instructions: Vec<MemAccessTraceEntry>,
            // pub warp_traces: Vec<Mem>,
            // pub instructions: Vec<>,
            // kernel: Arc<super::KernelInfo>,
        }
    }

    // todo: just use fields direclty now?
    #[deprecated]
    pub fn init(
        &mut self,
        start_pc: Option<usize>,
        block_id: usize,
        warp_id: usize,
        dynamic_warp_id: usize,
        active_mask: ThreadActiveMask,
    ) {
        self.block_id = block_id;
        self.warp_id = warp_id;
        self.dynamic_warp_id = dynamic_warp_id;
        self.next_pc = start_pc;
        // assert(self.num_completed >= active.count());
        // assert(n_completed <= m_warp_size);
        self.active_mask = active_mask;
    }

    pub fn num_completed(&self) -> usize {
        self.active_mask.count_zeros()
    }

    // todo: might do the conversion using `from_trace` during initialization
    // so this is not a special case and we support execution driven later on?
    pub fn next_trace_inst(&mut self) -> Option<WarpInstruction> {
        let Some(trace_instr) = self.trace_instructions.get(self.trace_pc) else {
            return None;
        };
        // let warp_instr = WarpInstruction::from_trace(&*self.kernel, trace_instr.clone());
        // new_inst->parse_from_trace_struct(
        //     warp_traces[trace_pc], m_kernel_info->OpcodeMap,
        //     m_kernel_info->m_tconfig, m_kernel_info->m_kernel_trace_info);
        self.trace_pc += 1;
        // Some(warp_instr)
        None
    }

    pub fn trace_start_pc(&self) -> Option<usize> {
        // debug_assert!(!self.trace_instructions.is_empty());
        self.trace_instructions.front().map(|instr| instr.pc)
    }

    pub fn pc(&self) -> usize {
        debug_assert!(!self.trace_instructions.is_empty());
        debug_assert!(self.trace_pc < self.trace_instructions.len());
        self.trace_instructions[self.trace_pc].pc
    }

    pub fn done(&self) -> bool {
        self.trace_pc == self.trace_instructions.len()
    }

    pub fn clear(&mut self) {
        // todo: should we actually clear schedule warps or just swap
        self.trace_pc = 0;
        self.trace_instructions.clear();
    }

    pub fn inc_instr_in_pipeline(&self) {}

    pub fn ibuffer_fill(&self, i: usize, instr: WarpInstruction) {}

    pub fn ibuffer_empty(&self) -> bool {
        // self.done
        false
    }

    pub fn done_exit(&self) -> bool {
        // self.done
        false
    }

    pub fn waiting(&self) -> bool {
        false
        //       if (functional_done()) {
        //   // waiting to be initialized with a kernel
        //   return true;
        // } else if (m_shader->warp_waiting_at_barrier(m_warp_id)) {
        //   // waiting for other warps in CTA to reach barrier
        //   return true;
        // } else if (m_shader->warp_waiting_at_mem_barrier(m_warp_id)) {
        //   // waiting for memory barrier
        //   return true;
        // } else if (m_n_atomic > 0) {
        //   // waiting for atomic operation to complete at memory:
        //   // this stall is not required for accurate timing model, but rather we
        //   // stall here since if a call/return instruction occurs in the meantime
        //   // the functional execution of the atomic when it hits DRAM can cause
        //   // the wrong register to be read.
        //   return true;
        // }
        // return false;
    }

    pub fn dynamic_warp_id(&self) -> usize {
        self.dynamic_warp_id
    }
}

pub trait SchedulerPolicy {
    fn order_warps(&self);
}

fn sort_warps_by_oldest_dynamic_id(lhs: &SchedulerWarp, rhs: &SchedulerWarp) -> std::cmp::Ordering {
    if lhs.done_exit() || lhs.waiting() {
        std::cmp::Ordering::Greater
    } else if rhs.done_exit() || rhs.waiting() {
        std::cmp::Ordering::Less
    } else {
        lhs.dynamic_warp_id().cmp(&rhs.dynamic_warp_id())
    }
}

#[derive(Debug)]
pub struct GTOScheduler {}

impl GTOScheduler {
    pub fn order_warps(
        &self,
        out: &mut VecDeque<SchedulerWarp>,
        warps: &mut Vec<SchedulerWarp>,
        last_issued_warps: &Vec<SchedulerWarp>,
        num_warps_to_add: usize,
    ) {
        // let mut next_cycle_prioritized_warps = Vec::new();
        //
        // let mut supervised_warps = Vec::new(); // input
        // let mut last_issued_from_input = Vec::new(); // last issued
        // let num_warps_to_add = supervised_warps.len();
        debug_assert!(num_warps_to_add <= warps.len());

        // scheduler_unit::sort_warps_by_oldest_dynamic_id

        // ORDERING_GREEDY_THEN_PRIORITY_FUNC
        out.clear();
        let greedy_value = last_issued_warps.first();
        if let Some(greedy_value) = greedy_value {
            out.push_back(greedy_value.clone());
        }

        warps.sort_by(sort_warps_by_oldest_dynamic_id);
        out.extend(
            warps
                .iter()
                .take_while(|w| match greedy_value {
                    None => true,
                    Some(val) => *w != val,
                })
                .take(num_warps_to_add)
                .cloned(),
        );

        //     typename std::vector<T>::iterator iter = temp.begin();
        //     for (unsigned count = 0; count < num_warps_to_add; ++count, ++iter) {
        //       if (*iter != greedy_value) {
        //         result_list.push_back(*iter);
        //       }
        //     }

        //   result_list.clear();
        //   typename std::vector<T> temp = input_list;
        //
        //   if (ORDERING_GREEDY_THEN_PRIORITY_FUNC == ordering) {
        //     T greedy_value = *last_issued_from_input;
        //     result_list.push_back(greedy_value);
        //
        //     std::sort(temp.begin(), temp.end(), priority_func);
        //     typename std::vector<T>::iterator iter = temp.begin();
        //     for (unsigned count = 0; count < num_warps_to_add; ++count, ++iter) {
        //       if (*iter != greedy_value) {
        //         result_list.push_back(*iter);
        //       }
        //     }
        //   } else if (ORDERED_PRIORITY_FUNC_ONLY == ordering) {
        //     std::sort(temp.begin(), temp.end(), priority_func);
        //     typename std::vector<T>::iterator iter = temp.begin();
        //     for (unsigned count = 0; count < num_warps_to_add; ++count, ++iter) {
        //       result_list.push_back(*iter);
        //     }
        //   } else {
        //     fprintf(stderr, "Unknown ordering - %d\n", ordering);
        //     abort();
        //   }

        // order by priority
        // (m_next_cycle_prioritized_warps, m_supervised_warps,
        //                 m_last_supervised_issued, m_supervised_warps.size(),
        //                 ORDERING_GREEDY_THEN_PRIORITY_FUNC,
        //                 scheduler_unit::sort_warps_by_oldest_dynamic_id);
    }
}