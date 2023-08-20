#![allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]

pub use crate::instruction::MemorySpace;
use bitvec::field::BitField;
use color_eyre::eyre;
use itertools::Itertools;
use nvbit_model;
use std::sync::{atomic, Arc, Mutex};
use trace_model as model;

const DEV_GLOBAL_HEAP_START: u64 = 0xC000_0000;
const WARP_SIZE: u32 = 32;

#[derive()]
pub struct DevicePtr<'s, 'a, T> {
    inner: &'a mut T,
    sim: &'s Simulation,
    mem_space: MemorySpace,
    offset: u64,
}

impl<'s, 'a, T> std::fmt::Debug for DevicePtr<'s, 'a, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl<'s, 'a, T> std::fmt::Display for DevicePtr<'s, 'a, T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

/// Convert multi-dimensional index into flat linear index.
pub trait ToFlatIndex {
    fn flatten(&self) -> usize;
}

impl ToFlatIndex for usize {
    fn flatten(&self) -> usize {
        *self
    }
}

impl<T, O, I> std::ops::Index<I> for DevicePtr<'_, '_, T>
where
    T: std::ops::Index<I, Output = O> + std::fmt::Debug,
    I: ToFlatIndex + std::fmt::Debug,
{
    type Output = O;

    fn index(&self, idx: I) -> &Self::Output {
        let elem_size = std::mem::size_of::<O>();
        let flat_idx = idx.flatten();
        let addr = self.offset + elem_size as u64 * flat_idx as u64;
        self.sim.load(addr, elem_size as u32, self.mem_space);
        log::trace!("{:?}[{:?}] => {}", &self.inner, &idx, &addr);
        &self.inner[idx]
    }
}

impl<T, O, I> std::ops::IndexMut<I> for DevicePtr<'_, '_, T>
where
    T: std::ops::IndexMut<I, Output = O> + std::fmt::Debug,
    I: ToFlatIndex + std::fmt::Debug,
{
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        let elem_size = std::mem::size_of::<O>();
        let flat_idx = idx.flatten();
        let addr = self.offset + elem_size as u64 * flat_idx as u64;
        self.sim.store(addr, elem_size as u32, self.mem_space);
        log::trace!("{:?}[{:?}] => {}", &self.inner, &idx, &addr);
        &mut self.inner[idx]
    }
}

/// Thread index.
#[derive(Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThreadIndex {
    pub block_idx: model::Dim,
    pub block_dim: model::Dim,
    pub thread_idx: model::Dim,
}

/// A kernel implementation.
pub trait Kernel {
    type Error: std::error::Error;

    /// Run an instance of the kernel on a thread identified by its index
    fn run(&mut self, idx: &ThreadIndex) -> Result<(), Self::Error>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum MemAccessKind {
    Load,
    Store,
}

/// Warp instruction
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct MemInstruction {
    mem_space: MemorySpace,
    kind: MemAccessKind,
    addr: u64,
    size: u32,
}

/// Warp instruction
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum WarpInstruction {
    Access(MemInstruction),
}

/// Simulation
#[derive()]
pub struct Simulation {
    offset: Mutex<u64>,
    pub inner: Mutex<crate::MockSimulator<crate::interconn::ToyInterconnect<crate::Packet>>>,
    thread_instructions: Mutex<Vec<WarpInstruction>>,
    kernel_id: atomic::AtomicU64,
}

impl Default for Simulation {
    fn default() -> Self {
        let config = Arc::new(crate::config::GPU::default());
        let interconn = Arc::new(crate::interconn::ToyInterconnect::new(
            config.num_simt_clusters,
            config.num_memory_controllers * config.num_sub_partition_per_memory_channel,
        ));
        let inner = Mutex::new(crate::MockSimulator::new(interconn, Arc::clone(&config)));

        Self {
            offset: Mutex::new(DEV_GLOBAL_HEAP_START),
            thread_instructions: Mutex::new(Vec::new()),
            kernel_id: atomic::AtomicU64::new(0),
            inner,
        }
    }
}

impl Simulation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&self, addr: u64, size: u32, mem_space: MemorySpace) {
        self.thread_instructions
            .lock()
            .unwrap()
            .push(WarpInstruction::Access(MemInstruction {
                kind: MemAccessKind::Load,
                addr,
                mem_space,
                size,
            }));
    }

    pub fn store(&self, addr: u64, size: u32, mem_space: MemorySpace) {
        self.thread_instructions
            .lock()
            .unwrap()
            .push(WarpInstruction::Access(MemInstruction {
                kind: MemAccessKind::Store,
                addr,
                mem_space,
                size,
            }));
    }

    /// Allocate a variable.
    pub fn allocate<'s, 'a, T>(
        &'s self,
        var: &'a mut T,
        size: u64,
        mem_space: MemorySpace,
    ) -> DevicePtr<'s, 'a, T> {
        let mut offset_lock = self.offset.lock().unwrap();
        let offset = *offset_lock;
        *offset_lock += size;

        self.inner.lock().unwrap().gpu_mem_alloc(offset, size, None);
        self.inner
            .lock()
            .unwrap()
            .memcopy_to_gpu(offset, size, None);

        DevicePtr {
            inner: var,
            mem_space,
            sim: self,
            offset,
        }
    }

    pub fn run_to_completion(&self) -> eyre::Result<stats::Stats> {
        let mut inner = self.inner.lock().unwrap();
        inner.run_to_completion()?;
        Ok(inner.stats())
    }

    /// Launches a kernel.
    ///
    /// # Errors
    /// When the kernel fails.
    pub fn launch_kernel<G, B, K>(
        &self,
        grid: G,
        block_size: B,
        mut kernel: K,
    ) -> Result<(), K::Error>
    where
        G: Into<model::Dim>,
        B: Into<model::Dim>,
        K: Kernel,
    {
        let grid: model::Dim = grid.into();
        let block_size: model::Dim = block_size.into();

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
            for (warp_id_in_block, warp) in thread_ids
                .chunks(WARP_SIZE as usize)
                .into_iter()
                .enumerate()
            {
                // log::info!("START WARP #{} ({:?})", &warp_id_in_block, &thread_id);
                let mut warp_instructions = [(); WARP_SIZE as usize].map(|_| Vec::new());

                for (thread_idx, warp_thread_idx) in warp.enumerate() {
                    // log::debug!(
                    //     "warp #{} thread {:?}",
                    //     &warp_num,
                    //     model::Dim::from(warp_thread_idx)
                    // );
                    thread_id.thread_idx = warp_thread_idx.into();
                    kernel.run(&thread_id)?;
                    warp_instructions[thread_idx]
                        .extend(self.thread_instructions.lock().unwrap().drain(..));
                }

                let warp_instruction = model::MemAccessTraceEntry {
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
                    instr_predicate: nvbit_model::Predicate::default(),
                    instr_mem_space: nvbit_model::MemorySpace::None,
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

                // check that all instructions match
                let longest = warp_instructions.iter().map(Vec::len).max().unwrap_or(0);

                let mut instr_idx = 0;
                for i in 0..longest {
                    let instructions: Vec<_> =
                        warp_instructions.iter().map(|inst| inst.get(i)).collect();
                    // assert!(instructions.map(|i| (i.kind, i.size)).all_equal());
                    // assert!(
                    //     instructions.windows(2).all(|w| match (w[0], w[1]) {
                    //         (
                    //             Some(WarpInstruction::Access(a)),
                    //             Some(WarpInstruction::Access(b)),
                    //         ) => a.kind == b.kind && a.size == b.size,
                    //         _ => false,
                    //     }),
                    //     "all threads in a warp need to have equal instructions"
                    // );

                    assert_eq!(instructions.len(), WARP_SIZE as usize);
                    let first_valid = instructions.iter().find_map(std::option::Option::as_ref);

                    if let Some(WarpInstruction::Access(access)) = first_valid {
                        let accesses: Vec<_> = instructions
                            .iter()
                            .map(|i| match i {
                                Some(WarpInstruction::Access(access)) => Some(access),
                                _ => None,
                            })
                            .collect();

                        let mut active_mask = crate::warp::ActiveMask::ZERO;
                        let mut addrs = [0; WARP_SIZE as usize];

                        for (thread_idx, acc) in accesses.iter().enumerate() {
                            if let Some(acc) = acc {
                                active_mask.set(thread_idx, true);
                                addrs[thread_idx] = acc.addr;
                            }
                        }

                        let is_load = access.kind == MemAccessKind::Load;
                        let is_store = access.kind == MemAccessKind::Store;
                        let instr_opcode = match access.mem_space {
                            MemorySpace::Local if is_load => "LDL".to_string(),
                            MemorySpace::Global if is_load => "LDG".to_string(),
                            MemorySpace::Shared if is_load => "LDS".to_string(),
                            // MemorySpace::Texture if is_load => "LDG".to_string(),
                            MemorySpace::Constant if is_load => "LDC".to_string(),
                            MemorySpace::Local if is_store => "STL".to_string(),
                            MemorySpace::Global if is_store => "STG".to_string(),
                            MemorySpace::Shared if is_store => "STS".to_string(),
                            // MemorySpace::Texture if is_store => "LDG".to_string(),
                            MemorySpace::Constant if is_store => panic!("constant store"),
                            other => panic!("unknown memory space {other:?}"),
                        };

                        trace.push(model::MemAccessTraceEntry {
                            instr_opcode: instr_opcode.to_string(),
                            instr_is_mem: true,
                            instr_is_store: is_store,
                            instr_is_load: is_load,
                            instr_idx,
                            active_mask: active_mask.load(),
                            addrs,
                            ..warp_instruction.clone()
                        });
                        instr_idx += 1;
                    };
                }

                trace.push(model::MemAccessTraceEntry {
                    instr_opcode: "EXIT".to_string(),
                    instr_idx,
                    active_mask: (!crate::warp::ActiveMask::ZERO).load(),
                    ..warp_instruction.clone()
                });

                // log::info!("END WARP #{} ({:?})", &warp_id_in_block, &thread_id);
            }
        }

        let trace = model::MemAccessTrace(trace);
        // dbg!(&trace);

        let warp_traces = trace.clone().to_warp_traces();
        dbg!(&warp_traces[&(model::Dim::ZERO, 0)]
            .iter()
            .map(|entry| (&entry.instr_opcode, &entry.active_mask))
            .collect::<Vec<_>>());

        let launch_config = model::KernelLaunch {
            name: String::new(),
            trace_file: String::new(),
            id: self.kernel_id.fetch_add(1, atomic::Ordering::SeqCst),
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
        let kernel = Arc::new(crate::kernel::Kernel::new(launch_config, trace));
        let mut inner = self.inner.lock().unwrap();
        inner.kernels.push_back(Arc::clone(&kernel));
        inner.launch(kernel).unwrap();

        Ok(())
    }
}