use crate::config;
use crate::ported::mem_sub_partition::{was_writeback_sent, SECTOR_SIZE};
use crate::ported::{
    address, cache, cache_block, interconn as ic, mem_fetch, mshr,
    stats::{CacheStats, Stats},
    tag_array,
};
use console::style;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// Metadata for port bandwidth management
#[derive(Clone)]
pub struct BandwidthManager {
    config: Arc<config::CacheConfig>,

    /// number of cycle that the data port remains used
    data_port_occupied_cycles: usize,
    /// number of cycle that the fill port remains used
    fill_port_occupied_cycles: usize,
}

impl std::fmt::Debug for BandwidthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("BandwidthManager")
            .field("data_port_occupied_cycles", &self.data_port_occupied_cycles)
            .field("fill_port_occupied_cycles", &self.fill_port_occupied_cycles)
            .field("has_free_data_port", &self.has_free_data_port())
            .field("has_free_fill_port", &self.has_free_fill_port())
            .finish()
    }
}

impl BandwidthManager {
    /// Create a new bandwidth manager from config
    pub fn new(config: Arc<config::CacheConfig>) -> Self {
        Self {
            config,
            data_port_occupied_cycles: 0,
            fill_port_occupied_cycles: 0,
        }
    }

    /// Use the data port based on the outcome and
    /// events generated by the mem_fetch request
    pub fn use_data_port(
        &mut self,
        // fetch: &mem_fetch::MemFetch,
        data_size: u32,
        access_status: cache::RequestStatus,
        events: &mut Vec<cache::Event>,
    ) {
        // let data_size = fetch.data_size;
        let port_width = self.config.data_port_width() as u32;
        match access_status {
            cache::RequestStatus::HIT => {
                let mut data_cycles = data_size / port_width;
                data_cycles += if data_size % port_width > 0 { 1 } else { 0 };
                self.data_port_occupied_cycles += data_cycles as usize;
            }
            cache::RequestStatus::HIT_RESERVED | cache::RequestStatus::MISS => {
                // the data array is accessed to read out the entire line for write-back
                // in case of sector cache we need to write bank only the modified sectors
                // todo!("need mem fetch events");
                // if let Some(evicted_block) = writeback_evicted_block {
                //     let data_cycles = evicted_block.modified_size / port_width;
                //     self.data_port_occupied_cycles += data_cycles as usize;
                // }
                // cache_event ev(WRITE_BACK_REQUEST_SENT);
                if let Some(writeback) = was_writeback_sent(events) {
                    let evicted = writeback.evicted_block.as_ref().unwrap();
                    let data_cycles = evicted.modified_size / port_width;
                    self.data_port_occupied_cycles += data_cycles as usize;
                }
            }
            cache::RequestStatus::SECTOR_MISS | cache::RequestStatus::RESERVATION_FAIL => {
                // Does not consume any port bandwidth
            }
            other => panic!("bandwidth manager got unexpected access status {other:?}"),
        }
    }

    /// Use the fill port
    pub fn use_fill_port(&mut self, fetch: &mem_fetch::MemFetch) {
        // assume filling the entire line with the returned request
        println!("atom size: {}", self.config.atom_size());
        println!("line size: {}", self.config.line_size);
        println!("data port width: {}", self.config.data_port_width());
        let fill_cycles = self.config.atom_size() as usize / self.config.data_port_width();
        println!(
            "bandwidth: {} using fill port for {} cycles",
            fetch, fill_cycles
        );
        self.fill_port_occupied_cycles += fill_cycles;
    }

    /// Free up used ports.
    ///
    /// This is called every cache cycle.
    pub fn replenish_port_bandwidth(&mut self) {
        if self.data_port_occupied_cycles > 0 {
            self.data_port_occupied_cycles -= 1;
        }
        debug_assert!(self.data_port_occupied_cycles >= 0);

        if self.fill_port_occupied_cycles > 0 {
            self.fill_port_occupied_cycles -= 1;
        }
        debug_assert!(self.fill_port_occupied_cycles >= 0);
        // todo!("bandwidth: replenish port bandwidth");
    }

    /// Query for data port availability
    pub fn has_free_data_port(&self) -> bool {
        println!(
            "has_free_data_port? data_port_occupied_cycles: {}",
            &self.data_port_occupied_cycles
        );
        self.data_port_occupied_cycles == 0
    }

    /// Query for fill port availability
    pub fn has_free_fill_port(&self) -> bool {
        println!(
            "has_free_fill_port? fill_port_occupied_cycles: {}",
            &self.fill_port_occupied_cycles
        );
        self.fill_port_occupied_cycles == 0
    }
}

#[derive(Debug)]
struct PendingRequest {
    valid: bool,
    block_addr: address,
    addr: address,
    cache_index: usize,
    data_size: u32,
    // this variable is used when a load request generates multiple load
    // transactions For example, a read request from non-sector L1 request sends
    // a request to sector L2
    pending_reads: usize,
}

impl PendingRequest {}

/// Base cache
///
/// Implements common functions for read_only_cache and data_cache
/// Each subclass implements its own 'access' function
#[derive()]
pub struct Base<I>
// where
//     I: ic::MemPort,
{
    pub name: String,
    pub core_id: usize,
    pub cluster_id: usize,

    // pub stats: Arc<Mutex<Stats>>,
    pub stats: Arc<Mutex<CacheStats>>,
    pub config: Arc<config::GPUConfig>,
    pub cache_config: Arc<config::CacheConfig>,

    pub miss_queue: VecDeque<mem_fetch::MemFetch>,
    pub miss_queue_status: mem_fetch::Status,
    pub mshrs: mshr::MshrTable,
    pub tag_array: tag_array::TagArray<()>,

    pending: HashMap<mem_fetch::MemFetch, PendingRequest>,
    mem_port: Arc<I>,

    // /// Specifies type of write allocate request
    // ///
    // /// (e.g., L1 or L2)
    // write_alloc_type: mem_fetch::AccessKind,
    //
    // /// Specifies type of writeback request
    // ///
    // /// (e.g., L1 or L2)
    // write_back_type: mem_fetch::AccessKind,
    pub bandwidth: BandwidthManager,
}

impl<I> std::fmt::Debug for Base<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Base")
            .field("name", &self.name)
            .field("core_id", &self.core_id)
            .field("cluster_id", &self.cluster_id)
            .field("miss_queue", &self.miss_queue)
            .finish()
    }
}

impl<I> Base<I> {
    pub fn new(
        name: String,
        core_id: usize,
        cluster_id: usize,
        // tag_array: tag_array::TagArray<()>,
        mem_port: Arc<I>,
        stats: Arc<Mutex<CacheStats>>,
        config: Arc<config::GPUConfig>,
        cache_config: Arc<config::CacheConfig>,
    ) -> Self {
        // for now we initialize the tag array and mshr

        // m_tag_array(new tag_array(config, core_id, type_id)),
        let tag_array = tag_array::TagArray::new(core_id, 0, cache_config.clone());

        // m_mshrs(config.m_mshr_entries, config.m_mshr_max_merge),
        debug_assert!(matches!(
            cache_config.mshr_kind,
            mshr::Kind::ASSOC | mshr::Kind::SECTOR_ASSOC
        ));
        let mshrs = mshr::MshrTable::new(cache_config.mshr_entries, cache_config.mshr_max_merge);

        let bandwidth = BandwidthManager::new(cache_config.clone());
        Self {
            name,
            core_id,
            cluster_id,
            tag_array,
            mshrs,
            mem_port,
            stats,
            config,
            cache_config,
            bandwidth,
            pending: HashMap::new(),
            miss_queue: VecDeque::new(),
            miss_queue_status: mem_fetch::Status::INITIALIZED,
            // write_alloc_type: mem_fetch::AccessKind::L1_WR_ALLOC_R,
            // write_back_type: mem_fetch::AccessKind::L1_WRBK_ACC,
        }
    }

    /// Checks whether this request can be handled in this cycle.
    ///
    /// `n` equals the number of misses to be handled on
    /// this cycle.
    pub fn miss_queue_can_fit(&self, n: usize) -> bool {
        self.miss_queue.len() + n < self.cache_config.miss_queue_size
    }

    /// Checks whether the miss queue is full.
    ///
    /// This leads to misses not being handled in this cycle.
    pub fn miss_queue_full(&self) -> bool {
        self.miss_queue.len() >= self.cache_config.miss_queue_size
    }

    /// Checks if fetch is waiting to be filled by lower memory level
    pub fn waiting_for_fill(&self, fetch: &mem_fetch::MemFetch) -> bool {
        self.pending.contains_key(&fetch)
        // extra_mf_fields_lookup::iterator e = m_extra_mf_fields.find(mf);
        // return e != m_extra_mf_fields.end();
        // todo!("base cache: waiting for fill");
    }

    /// Are any (accepted) accesses that had to wait for memory now ready?
    ///
    /// Note: does not include accesses that "HIT"
    pub fn has_ready_accesses(&self) -> bool {
        self.mshrs.has_ready_accesses()
    }

    pub fn ready_accesses(&self) -> Option<&VecDeque<mem_fetch::MemFetch>> {
        self.mshrs.ready_accesses()
    }

    /// Pop next ready access
    ///
    /// Note: does not include accesses that "HIT"
    pub fn next_access(&mut self) -> Option<mem_fetch::MemFetch> {
        self.mshrs.next_access()
    }

    /// Flush all entries in cache
    fn flush(&mut self) {
        self.tag_array.flush();
    }

    /// Invalidate all entries in cache
    fn invalidate(&mut self) {
        self.tag_array.invalidate();
    }

    /// Read miss handler.
    ///
    /// Check MSHR hit or MSHR available
    pub fn send_read_request(
        &mut self,
        addr: address,
        block_addr: u64,
        cache_index: usize,
        // cache_index: Option<usize>,
        mut fetch: mem_fetch::MemFetch,
        time: usize,
        events: &mut Vec<cache::Event>,
        read_only: bool,
        write_allocate: bool,
    ) -> (bool, bool, Option<tag_array::EvictedBlockInfo>) {
        let mut should_miss = false;
        let mut writeback = false;
        let mut evicted = None;

        let mshr_addr = self.cache_config.mshr_addr(fetch.addr());
        let mshr_hit = self.mshrs.probe(mshr_addr);
        let mshr_full = self.mshrs.full(mshr_addr);
        // let mut cache_index = cache_index.expect("cache index");

        println!(
            "{}::baseline_cache::send_read_request (addr={}, block={}, mshr_addr={}, mshr_hit={}, mshr_full={}, miss_queue_full={})",
            &self.name, addr, block_addr, &mshr_addr, mshr_hit, mshr_full, self.miss_queue_full(),
        );

        if mshr_hit && !mshr_full {
            if read_only {
                self.tag_array.access(block_addr, time, &fetch);
            } else {
                tag_array::AccessStatus {
                    writeback,
                    evicted,
                    ..
                } = self.tag_array.access(block_addr, time, &fetch);
            }

            self.mshrs.add(mshr_addr, fetch.clone());
            let mut stats = self.stats.lock().unwrap();
            stats.inc(
                *fetch.access_kind(),
                cache::AccessStat::Status(cache::RequestStatus::MSHR_HIT),
                1,
            );

            should_miss = true;
        } else if !mshr_hit && !mshr_full && !self.miss_queue_full() {
            if read_only {
                self.tag_array.access(block_addr, time, &fetch);
            } else {
                tag_array::AccessStatus {
                    writeback,
                    evicted,
                    ..
                } = self.tag_array.access(block_addr, time, &fetch);
            }

            let is_sector_cache = self.cache_config.mshr_kind == mshr::Kind::SECTOR_ASSOC;
            self.pending.insert(
                fetch.clone(),
                PendingRequest {
                    valid: true,
                    block_addr: mshr_addr,
                    addr: fetch.addr(),
                    cache_index,
                    data_size: fetch.data_size,
                    pending_reads: if is_sector_cache {
                        self.cache_config.line_size / SECTOR_SIZE
                    } else {
                        0
                    } as usize,
                },
            );
            // = extra_mf_fields(m_extra_mf_fields[mf] = extra_mf_fields(
            //     mshr_addr, mf->get_addr(), cache_index, mf->get_data_size(), m_config);

            // change address to mshr block address
            fetch.data_size = self.cache_config.atom_size() as u32;
            fetch.access.addr = mshr_addr;

            self.mshrs.add(mshr_addr, fetch.clone());
            self.miss_queue.push_back(fetch.clone());
            fetch.set_status(self.miss_queue_status, time);
            if !write_allocate {
                let event = cache::Event::new(cache::EventKind::READ_REQUEST_SENT);
                events.push(event);
            }

            should_miss = true;
        } else if mshr_hit && mshr_full {
            self.stats.lock().unwrap().inc(
                *fetch.access_kind(),
                cache::AccessStat::ReservationFailure(
                    cache::ReservationFailure::MSHR_MERGE_ENTRY_FAIL,
                ),
                1,
            );
        } else if !mshr_hit && mshr_full {
            self.stats.lock().unwrap().inc(
                *fetch.access_kind(),
                cache::AccessStat::ReservationFailure(cache::ReservationFailure::MSHR_ENTRY_FAIL),
                1,
            );
        } else {
            panic!(
                "mshr_hit={} mshr_full={} miss_queue_full={}",
                mshr_hit,
                mshr_full,
                self.miss_queue_full()
            );
        }
        (should_miss, write_allocate, evicted)
    }

    // /// Base read miss
    // ///
    // /// Send read request to lower level memory and perform
    // /// write-back as necessary.
    // fn read_miss(
    //     &mut self,
    //     addr: address,
    //     cache_index: Option<usize>,
    //     // cache_index: usize,
    //     fetch: mem_fetch::MemFetch,
    //     time: usize,
    //     // events: Option<&mut Vec<cache::Event>>,
    //     // events: &[cache::Event],
    //     probe_status: cache::RequestStatus,
    // ) -> cache::RequestStatus {
    //     dbg!((&self.miss_queue.len(), &self.cache_config.miss_queue_size));
    //     dbg!(&self.miss_queue_can_fit(1));
    //     if !self.miss_queue_can_fit(1) {
    //         // cannot handle request this cycle
    //         // (might need to generate two requests)
    //         // m_stats.inc_fail_stats(mf->get_access_type(), MISS_QUEUE_FULL);
    //         return cache::RequestStatus::RESERVATION_FAIL;
    //     }
    //
    //     let block_addr = self.cache_config.block_addr(addr);
    //     let (should_miss, writeback, evicted) = self.send_read_request(
    //         addr,
    //         block_addr,
    //         cache_index,
    //         fetch.clone(),
    //         time,
    //         // events.as_mut().cloned(),
    //         false,
    //         false,
    //     );
    //     dbg!((&should_miss, &writeback, &evicted));
    //
    //     if should_miss {
    //         // If evicted block is modified and not a write-through
    //         // (already modified lower level)
    //         if writeback
    //             && self.cache_config.write_policy != config::CacheWritePolicy::WRITE_THROUGH
    //         {
    //             if let Some(evicted) = evicted {
    //                 let wr = true;
    //                 let access = mem_fetch::MemAccess::new(
    //                     self.write_back_type,
    //                     evicted.block_addr,
    //                     evicted.modified_size as u32,
    //                     wr,
    //                     *fetch.access_warp_mask(),
    //                     evicted.byte_mask,
    //                     evicted.sector_mask,
    //                 );
    //
    //                 // (access, NULL, wr ? WRITE_PACKET_SIZE : READ_PACKET_SIZE, -1,
    //                 //   m_core_id, m_cluster_id, m_memory_config, cycle);
    //                 let mut writeback_fetch = mem_fetch::MemFetch::new(
    //                     fetch.instr,
    //                     access,
    //                     &*self.config,
    //                     if wr {
    //                         ported::WRITE_PACKET_SIZE
    //                     } else {
    //                         ported::READ_PACKET_SIZE
    //                     }
    //                     .into(),
    //                     0,
    //                     0,
    //                     0,
    //                 );
    //
    //                 //     None,
    //                 //     access,
    //                 //     // self.write_back_type,
    //                 //     &*self.config.l1_cache.unwrap(),
    //                 //     // evicted.block_addr,
    //                 //     // evicted.modified_size,
    //                 //     // true,
    //                 //     // fetch.access_warp_mask(),
    //                 //     // evicted.byte_mask,
    //                 //     // evicted.sector_mask,
    //                 //     // m_gpu->gpu_tot_sim_cycle + m_gpu->gpu_sim_cycle,
    //                 //     // -1, -1, -1, NULL,
    //                 // );
    //                 // the evicted block may have wrong chip id when
    //                 // advanced L2 hashing is used, so set the right chip
    //                 // address from the original mf
    //                 writeback_fetch.tlx_addr.chip = fetch.tlx_addr.chip;
    //                 writeback_fetch.tlx_addr.sub_partition = fetch.tlx_addr.sub_partition;
    //                 let event = cache::Event {
    //                     kind: cache::EventKind::WRITE_BACK_REQUEST_SENT,
    //                     evicted_block: None,
    //                 };
    //
    //                 self.send_write_request(
    //                     writeback_fetch,
    //                     event,
    //                     time,
    //                     // &events,
    //                 );
    //             }
    //         }
    //         return cache::RequestStatus::MISS;
    //     }
    //
    //     return cache::RequestStatus::RESERVATION_FAIL;
    // }
}

impl<I> cache::Component for Base<I>
where
    // I: ic::MemPort,
    I: ic::MemFetchInterface,
    // I: ic::Interconnect<crate::ported::core::Packet> + 'static,
{
    /// Sends next request to lower level of memory
    fn cycle(&mut self) {
        use cache::CacheBandwidth;
        println!(
            "{}::baseline cache::cycle (fetch interface {:?}) miss queue size={}",
            self.name,
            self.mem_port,
            style(self.miss_queue.len()).blue(),
        );
        if let Some(fetch) = self.miss_queue.front() {
            if !self.mem_port.full(fetch.size(), fetch.is_write()) {
                if let Some(fetch) = self.miss_queue.pop_front() {
                    println!(
                        "{}::baseline cache::memport::push({}, data size={}, control size={})",
                        &self.name,
                        fetch.addr(),
                        fetch.data_size,
                        fetch.control_size,
                    );
                    self.mem_port.push(fetch);
                }
            }
        }
        let data_port_busy = !self.has_free_data_port();
        let fill_port_busy = !self.has_free_fill_port();
        // m_stats.sample_cache_port_utility(data_port_busy, fill_port_busy);
        self.bandwidth.replenish_port_bandwidth();
    }
}

// stop: we do not want to implement cache for base as
// it should not actually implement an access function
// impl<I> cache::Cache for Base<I>
impl<I> Base<I>
where
    // I: ic::MemPort,
    I: ic::MemFetchInterface,
    // I: ic::Interconnect<crate::ported::core::Packet> + 'static,
{
    /// Interface for response from lower memory level.
    ///
    /// bandwidth restictions should be modeled in the caller.
    /// TODO: fill could also accept the fetch by value, otherwise we drop the fetch!!
    // pub fn fill(&mut self, fetch: &mut mem_fetch::MemFetch) {
    pub fn fill(&mut self, mut fetch: mem_fetch::MemFetch) {
        if self.cache_config.mshr_kind == mshr::Kind::SECTOR_ASSOC {
            todo!("sector assoc cache");
            let original_fetch = fetch.original_fetch.as_ref().unwrap();
            let pending = self.pending.get_mut(original_fetch).unwrap();
            pending.pending_reads -= 1;
            if pending.pending_reads > 0 {
                // wait for the other requests to come back
                // delete mf;
                return;
            } else {
                // mem_fetch *temp = mf;
                // todo: consume the fetch here?
                let original_fetch = fetch.original_fetch.as_ref().unwrap().as_ref().clone();
                // *fetch = original_fetch;
                // delete temp;
            }
        }

        let pending = self.pending.remove(&fetch).unwrap();
        self.bandwidth.use_fill_port(&fetch);

        debug_assert!(pending.valid);
        fetch.data_size = pending.data_size;
        fetch.access.addr = pending.addr;

        match self.cache_config.allocate_policy {
            config::CacheAllocatePolicy::ON_MISS => {
                self.tag_array.fill_on_miss(pending.cache_index, &fetch);
            }
            config::CacheAllocatePolicy::ON_FILL => {
                self.tag_array.fill_on_fill(pending.block_addr, &fetch);
            }
            other => unimplemented!("cache allocate policy {:?} is not implemented", other),
        }

        let access_sector_mask = fetch.access_sector_mask().clone();
        let access_byte_mask = fetch.access_byte_mask().clone();

        let has_atomic = self
            .mshrs
            .mark_ready(pending.block_addr, fetch)
            .unwrap_or(false);

        if has_atomic {
            debug_assert!(
                self.cache_config.allocate_policy == config::CacheAllocatePolicy::ON_MISS
            );
            let block = self.tag_array.get_block_mut(pending.cache_index);
            // mark line as dirty for atomic operation
            block.set_status(cache_block::Status::MODIFIED, &access_sector_mask);
            block.set_byte_mask(&access_byte_mask);
            if !block.is_modified() {
                self.tag_array.num_dirty += 1;
            }
        }
    }
}

impl<I> cache::CacheBandwidth for Base<I> {
    fn has_free_data_port(&self) -> bool {
        self.bandwidth.has_free_data_port()
    }

    fn has_free_fill_port(&self) -> bool {
        self.bandwidth.has_free_fill_port()
    }
}

#[cfg(test)]
mod tests {
    use super::Base;
    use crate::config;
    use crate::ported::{
        interconn as ic, mem_fetch,
        stats::{CacheStats, Stats},
        Packet,
    };
    use std::sync::{Arc, Mutex};

    #[ignore = "todo"]
    #[test]
    fn base_cache_init() {
        let core_id = 0;
        let cluster_id = 0;
        let config = Arc::new(config::GPUConfig::default());
        let cache_stats = Arc::new(Mutex::new(CacheStats::default()));
        let cache_config = config.data_cache_l1.clone().unwrap();

        let stats = Arc::new(Mutex::new(Stats::new(&*config)));
        let interconn: Arc<ic::ToyInterconnect<Packet>> =
            Arc::new(ic::ToyInterconnect::new(0, 0, None));
        let port = Arc::new(ic::CoreMemoryInterface {
            interconn,
            cluster_id: 0,
            stats,
            config: config.clone(),
        });

        let base = Base::new(
            "base cache".to_string(),
            core_id,
            cluster_id,
            port,
            cache_stats,
            config,
            Arc::clone(&cache_config.inner),
        );
        dbg!(&base);
        assert!(false);
    }
}
