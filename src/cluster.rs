use super::{config, interconn as ic, kernel::Kernel, mem_fetch, Core, MockSimulator, Packet};
use console::style;

use std::collections::VecDeque;

use std::sync::{atomic, Arc, Mutex, RwLock};

#[derive(Debug)]
pub struct Cluster<I> {
    pub cluster_id: usize,
    pub cycle: super::Cycle,
    pub warp_instruction_unique_uid: Arc<atomic::AtomicU64>,
    pub cores: Vec<Arc<RwLock<Core<I>>>>,
    pub config: Arc<config::GPU>,
    pub stats: Arc<Mutex<stats::Stats>>,

    pub interconn: Arc<I>,

    pub core_sim_order: VecDeque<usize>,
    pub block_issue_next_core: Mutex<usize>,
    pub response_fifo: VecDeque<mem_fetch::MemFetch>,
}

impl<I> Cluster<I>
where
    I: ic::Interconnect<Packet> + 'static,
{
    pub fn new(
        cluster_id: usize,
        cycle: &super::Cycle,
        warp_instruction_unique_uid: &Arc<atomic::AtomicU64>,
        allocations: &super::allocation::Ref,
        interconn: &Arc<I>,
        stats: &Arc<Mutex<stats::Stats>>,
        config: &Arc<config::GPU>,
    ) -> Self {
        let num_cores = config.num_cores_per_simt_cluster;
        let block_issue_next_core = Mutex::new(num_cores - 1);
        let mut cluster = Self {
            cluster_id,
            cycle: cycle.clone(),
            warp_instruction_unique_uid: Arc::clone(warp_instruction_unique_uid),
            config: config.clone(),
            stats: stats.clone(),
            interconn: interconn.clone(),
            cores: Vec::new(),
            core_sim_order: VecDeque::new(),
            block_issue_next_core,
            response_fifo: VecDeque::new(),
        };
        let cores = (0..num_cores)
            .map(|core_id| {
                cluster.core_sim_order.push_back(core_id);
                let id = config.global_core_id(cluster_id, core_id);
                Arc::new(RwLock::new(Core::new(
                    id,
                    cluster_id,
                    Arc::clone(allocations),
                    cycle.clone(),
                    Arc::clone(warp_instruction_unique_uid),
                    Arc::clone(interconn),
                    Arc::clone(stats),
                    Arc::clone(config),
                )))
            })
            .collect();
        cluster.cores = cores;
        cluster.reinit();
        cluster
    }

    fn reinit(&mut self) {
        for core in &self.cores {
            core.write()
                .unwrap()
                .reinit(0, self.config.max_threads_per_core, true);
        }
    }

    pub fn num_active_sms(&self) -> usize {
        self.cores
            .iter()
            .filter(|core| core.read().unwrap().active())
            .count()
    }

    pub fn not_completed(&self) -> usize {
        self.cores
            .iter()
            .map(|core| core.read().unwrap().not_completed())
            .sum()
    }

    pub fn interconn_cycle(&mut self) {
        use mem_fetch::AccessKind;

        log::debug!(
            "{}",
            style(format!(
                "cycle {:02} cluster {}: interconn cycle (response fifo={:?})",
                self.cycle.get(),
                self.cluster_id,
                self.response_fifo
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>(),
            ))
            .cyan()
        );

        if let Some(fetch) = self.response_fifo.front() {
            let core_id = self.config.global_core_id_to_core_id(fetch.core_id);

            let mut core = self.cores[core_id].write().unwrap();

            match *fetch.access_kind() {
                AccessKind::INST_ACC_R => {
                    // this could be the reason
                    if core.fetch_unit_response_buffer_full() {
                        log::debug!("instr access fetch {} NOT YET ACCEPTED", fetch);
                    } else {
                        let fetch = self.response_fifo.pop_front().unwrap();
                        log::debug!("accepted instr access fetch {}", fetch);
                        core.accept_fetch_response(fetch);
                    }
                }
                _ if !core.ldst_unit_response_buffer_full() => {
                    let fetch = self.response_fifo.pop_front().unwrap();
                    log::debug!("accepted ldst unit fetch {}", fetch);
                    // m_memory_stats->memlatstat_read_done(mf);
                    core.accept_ldst_unit_response(fetch);
                }
                _ => {
                    log::debug!("ldst unit fetch {} NOT YET ACCEPTED", fetch);
                }
            }
        }

        // this could be the reason?
        let eject_buffer_size = self.config.num_cluster_ejection_buffer_size;
        if self.response_fifo.len() >= eject_buffer_size {
            log::debug!(
                "skip: ejection buffer full ({}/{})",
                self.response_fifo.len(),
                eject_buffer_size
            );
            return;
        }

        let Some(Packet::Fetch(mut fetch)) = self.interconn.pop(self.cluster_id) else {
            return;
        };
        log::debug!(
            "{}",
            style(format!(
                "cycle {:02} cluster {}: got fetch from interconn: {}",
                self.cycle.get(),
                self.cluster_id,
                fetch,
            ))
            .cyan()
        );

        debug_assert_eq!(fetch.cluster_id, self.cluster_id);
        // debug_assert!(matches!(
        //     fetch.kind,
        //     mem_fetch::Kind::READ_REPLY | mem_fetch::Kind::WRITE_ACK
        // ));

        // The packet size varies depending on the type of request:
        // - For read request and atomic request, the packet contains the data
        // - For write-ack, the packet only has control metadata
        // let _packet_size = if fetch.is_write() {
        //     fetch.control_size()
        // } else {
        //     fetch.data_size()
        // };
        // m_stats->m_incoming_traffic_stats->record_traffic(mf, packet_size);
        fetch.status = mem_fetch::Status::IN_CLUSTER_TO_SHADER_QUEUE;
        self.response_fifo.push_back(fetch);

        // m_stats->n_mem_to_simt[m_cluster_id] += mf->get_num_flits(false);
    }

    pub fn cache_flush(&mut self) {
        for core in &self.cores {
            core.write().unwrap().cache_flush();
        }
    }

    pub fn cache_invalidate(&mut self) {
        for core in &self.cores {
            core.write().unwrap().cache_invalidate();
        }
    }

    // pub fn cycle(&mut self) {
    //     log::debug!("cluster {} cycle {}", self.cluster_id, self.cycle.get());
    //     for core_id in &self.core_sim_order {
    //         self.cores[*core_id].lock().unwrap().cycle();
    //     }
    //
    //     if let config::SchedulingOrder::RoundRobin = self.config.simt_core_sim_order {
    //         self.core_sim_order.rotate_left(1);
    //     }
    // }

    pub fn issue_block_to_core(&self, sim: &MockSimulator<I>) -> usize {
        let num_cores = self.cores.len();

        log::debug!(
            "cluster {}: issue block to core for {} cores",
            self.cluster_id,
            num_cores
        );
        let mut num_blocks_issued = 0;

        let mut block_issue_next_core = self.block_issue_next_core.lock().unwrap();

        for core_id in 0..num_cores {
            let core_id = (core_id + *block_issue_next_core + 1) % num_cores;
            // let core = &mut cores[core_id];
            let mut core = self.cores[core_id].write().unwrap();
            let kernel: Option<Arc<Kernel>> = if self.config.concurrent_kernel_sm {
                // always select latest issued kernel
                // kernel = sim.select_kernel()
                // sim.select_kernel().map(Arc::clone);
                unimplemented!("concurrent kernel sm");
            } else {
                let mut current_kernel = core.current_kernel.clone();
                let should_select_new_kernel = if let Some(ref current) = current_kernel {
                    // if no more blocks left, get new kernel once current block completes
                    current.no_more_blocks_to_run() && core.not_completed() == 0
                } else {
                    // core was not assigned a kernel yet
                    true
                };

                if let Some(ref current) = current_kernel {
                    log::debug!(
                        "core {}-{}: current kernel {}, more blocks={}, completed={}",
                        self.cluster_id,
                        core_id,
                        current,
                        !current.no_more_blocks_to_run(),
                        core.not_completed() == 0,
                    );
                }

                if should_select_new_kernel {
                    current_kernel = sim.select_kernel();
                    if let Some(ref k) = current_kernel {
                        core.set_kernel(Arc::clone(k));
                    }
                }

                current_kernel
            };
            if let Some(kernel) = kernel {
                log::debug!(
                    "core {}-{}: selected kernel {} more blocks={} can issue={}",
                    self.cluster_id,
                    core_id,
                    kernel,
                    !kernel.no_more_blocks_to_run(),
                    core.can_issue_block(&kernel),
                );

                if !kernel.no_more_blocks_to_run() && core.can_issue_block(&kernel) {
                    core.issue_block(&kernel);
                    num_blocks_issued += 1;
                    *block_issue_next_core = core_id;
                    break;
                }
            } else {
                log::debug!(
                    "core {}-{}: selected kernel NULL",
                    self.cluster_id,
                    core.core_id,
                );
            }
        }
        num_blocks_issued
    }
}