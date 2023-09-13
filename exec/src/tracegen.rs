use super::kernel::{Kernel, ThreadIndex};
use super::model;
use super::nop::ArithmeticNop;
use super::DevicePtr;
use bitvec::field::BitField;
use itertools::Itertools;
use std::sync::{atomic, Arc, Mutex};

const DEV_GLOBAL_HEAP_START: u64 = 0xC000_0000;
const WARP_SIZE: u32 = 32;

#[derive(thiserror::Error, Debug)]
pub enum Error<K, T> {
    #[error(transparent)]
    Kernel(K),
    #[error(transparent)]
    Tracer(T),
}

pub trait TraceGenerator {
    type Error;

    /// Trace kernel.
    fn trace_kernel<G, B, K>(
        // &mut self,
        &self,
        grid: G,
        block_size: B,
        kernel: K,
    ) -> Result<(), Error<K::Error, Self::Error>>
    where
        G: Into<trace_model::Dim>,
        B: Into<trace_model::Dim>,
        K: Kernel;

    /// Allocate a variable.
    // fn allocate<'s, 'a, T>(
    // fn allocate<'s, T, O>(
    fn allocate<'s, C, T>(
        &'s self,
        var: C,
        // var: &'a mut T,
        size: u64,
        mem_space: model::MemorySpace,
        // ) -> DevicePtr<'s, 'a, T>;
        // ) -> DevicePtr<'s, T, O>
    ) -> DevicePtr<'s, C, T>;
    // where
    //     T: Container,
    //     <T as Container>::Elem: Zero;
}

pub trait MemoryAccess {
    /// Load address.
    fn load(&self, addr: u64, size: u32, mem_space: model::MemorySpace, active: bool);

    /// Store address.
    fn store(&self, addr: u64, size: u32, mem_space: model::MemorySpace, active: bool);
}

#[derive(Debug, Default)]
pub struct Tracer {
    offset: Mutex<u64>,
    // pub inner: Mutex<
    //     crate::MockSimulator<crate::interconn::ToyInterconnect<ic::Packet<mem_fetch::MemFetch>>>,
    // >,
    // commands: Mutex<Vec<Command>>,
    thread_instructions: Mutex<Vec<model::ThreadInstruction>>,
    kernel_launch_id: atomic::AtomicU64,
}

impl MemoryAccess for Tracer {
    fn load(&self, addr: u64, size: u32, mem_space: model::MemorySpace, active: bool) {
        let inst = if active {
            model::ThreadInstruction::Access(model::MemInstruction {
                kind: model::MemAccessKind::Load,
                addr,
                mem_space,
                size,
            })
        } else {
            model::ThreadInstruction::Inactive
        };
        self.thread_instructions.lock().unwrap().push(inst);
    }

    fn store(&self, addr: u64, size: u32, mem_space: model::MemorySpace, active: bool) {
        let inst = if active {
            model::ThreadInstruction::Access(model::MemInstruction {
                kind: model::MemAccessKind::Store,
                addr,
                mem_space,
                size,
            })
        } else {
            model::ThreadInstruction::Inactive
        };
        self.thread_instructions.lock().unwrap().push(inst);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TraceError {
    #[error("inconsistent number of warp instructions")]
    InconsistentNumberOfWarpInstructions,
}

impl TraceGenerator for Tracer {
    type Error = TraceError;

    // fn allocate<'s, 'a, T>(
    // fn allocate<'s, T, O>(
    fn allocate<'s, C, T>(
        &'s self,
        // var: &'a mut T,
        var: C,
        size: u64,
        mem_space: model::MemorySpace,
        // ) -> DevicePtr<'s, 'a, T> {
    ) -> DevicePtr<'s, C, T>
// where
    //     // T: Container<O>,
    //     T: Container,
    //     <T as Container>::Elem: Zero,
        // O: num_traits::Zero,
    {
        let mut offset_lock = self.offset.lock().unwrap();
        let offset = *offset_lock;
        *offset_lock += size;

        // self.inner.lock().gpu_mem_alloc(offset, size, None, 0);
        // self.inner.lock().memcopy_to_gpu(offset, size, None, 0);

        DevicePtr {
            inner: var,
            // spare: <T as Container>::Elem::zero(),
            mem_space,
            marker: std::marker::PhantomData,
            nop: ArithmeticNop::default(),
            memory: self,
            offset,
        }
    }

    #[allow(irrefutable_let_patterns)]
    fn trace_kernel<G, B, K>(
        // &mut self,
        &self,
        grid: G,
        block_size: B,
        mut kernel: K,
    ) -> Result<(), Error<K::Error, Self::Error>>
    where
        G: Into<trace_model::Dim>,
        B: Into<trace_model::Dim>,
        K: Kernel,
    {
        let grid: trace_model::Dim = grid.into();
        let block_size: trace_model::Dim = block_size.into();

        let mut trace = Vec::new();

        // loop over the grid
        for block_id in grid.clone() {
            log::debug!("block {}", &block_id);

            let mut thread_id = ThreadIndex {
                block_idx: block_id.to_dim(),
                block_dim: block_size.clone(),
                thread_idx: block_size.clone(),
            };

            // loop over the block size and form warps
            let thread_ids = block_size.clone().into_iter();
            for (warp_id_in_block, threads) in thread_ids
                .chunks(WARP_SIZE as usize)
                .into_iter()
                .enumerate()
            {
                // log::info!("START WARP #{} ({:?})", &warp_id_in_block, &thread_id);
                let mut thread_instructions = [(); WARP_SIZE as usize].map(|_| Vec::new());

                for (thread_idx, warp_thread_idx) in threads.enumerate() {
                    // log::debug!(
                    //     "warp #{} thread {:?}",
                    //     &warp_num,
                    //     model::Dim::from(warp_thread_idx)
                    // );
                    thread_id.thread_idx = warp_thread_idx.into();
                    kernel.run(&thread_id).map_err(Error::Kernel)?;
                    thread_instructions[thread_idx]
                        .extend(self.thread_instructions.lock().unwrap().drain(..));
                }

                let warp_instruction = trace_model::MemAccessTraceEntry {
                    cuda_ctx: 0,
                    sm_id: 0,
                    kernel_id: 0,
                    block_id: block_id.clone().into(),
                    warp_id_in_sm: warp_id_in_block as u32,
                    warp_id_in_block: warp_id_in_block as u32,
                    warp_size: WARP_SIZE,
                    line_num: 0,
                    instr_data_width: 0,
                    instr_opcode: String::new(),
                    instr_offset: 0,
                    instr_idx: 0,
                    instr_predicate: trace_model::Predicate::default(),
                    instr_mem_space: trace_model::MemorySpace::None,
                    instr_is_mem: false,
                    instr_is_load: false,
                    instr_is_store: false,
                    instr_is_extended: false,
                    dest_regs: [0; 1],
                    num_dest_regs: 0,
                    src_regs: [0; 5],
                    num_src_regs: 0,
                    active_mask: 0,
                    addrs: [0; 32],
                };

                dbg!(&warp_id_in_block);
                dbg!(&thread_instructions.iter().map(Vec::len).collect::<Vec<_>>());

                // todo: our own partial eq here
                // #[derive(Debug, PartialEq, Eq)]
                // struct Instruction<'a>(&'a WarpInstruction);

                // check that all instructions match
                // if thread_instructions.iter().map(|instructions| instructions.iter().map(|inst| inst_)
                // dbg!(thread_instructions
                //     .iter()
                //     .map(|instructions| instructions) // .iter().map(Instruction).collect::<Vec<_>>())
                //     .collect::<Vec<_>>());

                if !thread_instructions
                    .iter()
                    .map(|instructions| instructions) // .iter().map(Instruction).collect::<Vec<_>>())
                    .all_equal()
                {
                    return Err(Error::Tracer(
                        TraceError::InconsistentNumberOfWarpInstructions,
                    ));
                }

                let num_instructions = thread_instructions[0].len();

                for instr_idx in 0..num_instructions {
                    let mut active_mask = trace_model::ActiveMask::ZERO;
                    let mut addrs = [0; WARP_SIZE as usize];

                    for thread_idx in 0..(WARP_SIZE as usize) {
                        // let current_thread_instructions: Vec<_> =
                        //     warp_instructions.iter().map(|inst| inst.get(i)).collect();

                        let thread_instruction = &thread_instructions[thread_idx][instr_idx];
                        if let model::ThreadInstruction::Access(ref access) = thread_instruction {
                            active_mask.set(thread_idx, true);
                            addrs[thread_idx] = access.addr;

                            // let accesses: Vec<_> = instructions
                            //     .iter()
                            //     .map(|i| match i {
                            //         Some(WarpInstruction::Access(access)) => Some(access),
                            //         _ => None,
                            //     })
                            //     .collect();

                            // let mut active_mask = crate::warp::ActiveMask::ZERO;
                            // let mut addrs = [0; WARP_SIZE as usize];

                            // for (thread_idx, acc) in accesses.iter().enumerate() {
                            //     if let Some(acc) = acc {
                            //         active_mask.set(thread_idx, true);
                            //         addrs[thread_idx] = acc.addr;
                            //     }
                            // }

                            // instr_idx += 1;
                        }
                    }

                    // first_thread_instruction
                    if let model::ThreadInstruction::Access(ref access) =
                        thread_instructions[0][instr_idx]
                    {
                        let is_load = access.kind == model::MemAccessKind::Load;
                        let is_store = access.kind == model::MemAccessKind::Store;
                        let instr_opcode = match access.mem_space {
                            model::MemorySpace::Local if is_load => "LDL".to_string(),
                            model::MemorySpace::Global if is_load => "LDG".to_string(),
                            model::MemorySpace::Shared if is_load => "LDS".to_string(),
                            // MemorySpace::Texture if is_load => "LDG".to_string(),
                            model::MemorySpace::Constant if is_load => "LDC".to_string(),
                            model::MemorySpace::Local if is_store => "STL".to_string(),
                            model::MemorySpace::Global if is_store => "STG".to_string(),
                            model::MemorySpace::Shared if is_store => "STS".to_string(),
                            // MemorySpace::Texture if is_store => "LDG".to_string(),
                            model::MemorySpace::Constant if is_store => panic!("constant store"),
                            other => panic!("unknown memory space {other:?}"),
                        };

                        trace.push(trace_model::MemAccessTraceEntry {
                            instr_opcode: instr_opcode.to_string(),
                            instr_is_mem: true,
                            instr_is_store: is_store,
                            instr_is_load: is_load,
                            instr_idx: instr_idx as u32,
                            active_mask: active_mask.load(),
                            addrs,
                            ..warp_instruction.clone()
                        });
                    }
                }

                // let longest = warp_instructions.iter().map(Vec::len).max().unwrap_or(0);
                //
                // let mut instr_idx = 0;
                // for i in 0..longest {
                //     let instructions: Vec<_> =
                //         warp_instructions.iter().map(|inst| inst.get(i)).collect();
                //     // assert!(instructions.map(|i| (i.kind, i.size)).all_equal());
                //     // assert!(
                //     //     instructions.windows(2).all(|w| match (w[0], w[1]) {
                //     //         (
                //     //             Some(WarpInstruction::Access(a)),
                //     //             Some(WarpInstruction::Access(b)),
                //     //         ) => a.kind == b.kind && a.size == b.size,
                //     //         _ => false,
                //     //     }),
                //     //     "all threads in a warp need to have equal instructions"
                //     // );
                //
                //     assert_eq!(instructions.len(), WARP_SIZE as usize);
                //     let first_valid = instructions.iter().find_map(std::option::Option::as_ref);
                //
                //     if let Some(WarpInstruction::Access(access)) = first_valid {
                //         let accesses: Vec<_> = instructions
                //             .iter()
                //             .map(|i| match i {
                //                 Some(WarpInstruction::Access(access)) => Some(access),
                //                 _ => None,
                //             })
                //             .collect();
                //
                //         let mut active_mask = crate::warp::ActiveMask::ZERO;
                //         let mut addrs = [0; WARP_SIZE as usize];
                //
                //         for (thread_idx, acc) in accesses.iter().enumerate() {
                //             if let Some(acc) = acc {
                //                 active_mask.set(thread_idx, true);
                //                 addrs[thread_idx] = acc.addr;
                //             }
                //         }
                //
                //         let is_load = access.kind == MemAccessKind::Load;
                //         let is_store = access.kind == MemAccessKind::Store;
                //         let instr_opcode = match access.mem_space {
                //             MemorySpace::Local if is_load => "LDL".to_string(),
                //             MemorySpace::Global if is_load => "LDG".to_string(),
                //             MemorySpace::Shared if is_load => "LDS".to_string(),
                //             // MemorySpace::Texture if is_load => "LDG".to_string(),
                //             MemorySpace::Constant if is_load => "LDC".to_string(),
                //             MemorySpace::Local if is_store => "STL".to_string(),
                //             MemorySpace::Global if is_store => "STG".to_string(),
                //             MemorySpace::Shared if is_store => "STS".to_string(),
                //             // MemorySpace::Texture if is_store => "LDG".to_string(),
                //             MemorySpace::Constant if is_store => panic!("constant store"),
                //             other => panic!("unknown memory space {other:?}"),
                //         };
                //
                //         trace.push(model::MemAccessTraceEntry {
                //             instr_opcode: instr_opcode.to_string(),
                //             instr_is_mem: true,
                //             instr_is_store: is_store,
                //             instr_is_load: is_load,
                //             instr_idx,
                //             active_mask: active_mask.load(),
                //             addrs,
                //             ..warp_instruction.clone()
                //         });
                //         instr_idx += 1;
                //     };
                // }

                // EXIT instruction
                trace.push(trace_model::MemAccessTraceEntry {
                    instr_opcode: "EXIT".to_string(),
                    instr_idx: num_instructions as u32,
                    active_mask: (!trace_model::ActiveMask::ZERO).load(),
                    ..warp_instruction.clone()
                });

                // log::info!("END WARP #{} ({:?})", &warp_id_in_block, &thread_id);
            }
        }

        let trace = trace_model::MemAccessTrace(trace);
        // dbg!(&trace);

        let warp_traces = trace.clone().to_warp_traces();
        dbg!(&warp_traces[&(trace_model::Dim::ZERO, 0)]
            .iter()
            .map(|entry| (&entry.instr_opcode, &entry.active_mask))
            .collect::<Vec<_>>());

        let launch_config = trace_model::command::KernelLaunch {
            mangled_name: kernel.name().to_string(),
            unmangled_name: kernel.name().to_string(),
            trace_file: String::new(),
            id: self.kernel_launch_id.fetch_add(1, atomic::Ordering::SeqCst),
            grid,
            block: block_size,
            shared_mem_bytes: 0,
            num_registers: 0,
            binary_version: 61,
            stream_id: 0,
            shared_mem_base_addr: 0,
            local_mem_base_addr: 0,
            nvbit_version: "none".to_string(),
        };
        dbg!(launch_config);
        // let kernel = Arc::new(crate::kernel::Kernel::new(launch_config, trace));
        // let mut inner = self.inner.lock();
        // inner.kernels.push_back(Arc::clone(&kernel));
        // inner.launch(kernel).unwrap();
        Ok(())
    }
}
