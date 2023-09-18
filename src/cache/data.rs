use crate::sync::{Arc, Mutex};
use crate::{address, cache, config, interconn as ic, mcu, mem_fetch, mshr::MSHR, tag_array};

use cache::block::Block;
use mcu::MemoryController;
use mem_fetch::access::Kind as AccessKind;
use tag_array::Access;

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Builder<MC, CC> {
    pub name: String,
    pub core_id: usize,
    pub cluster_id: usize,
    pub stats: Arc<Mutex<stats::cache::PerKernel>>,
    pub mem_controller: MC,
    pub cache_controller: CC,
    pub config: Arc<config::GPU>,
    pub cache_config: Arc<config::Cache>,
    pub write_alloc_type: AccessKind,
    pub write_back_type: AccessKind,
}

/// First level data cache in Fermi.
///
/// The cache uses a write-evict (global) or write-back (local) policy
/// at the granularity of individual blocks.
/// (the policy used in fermi according to the CUDA manual)
pub struct Data<MC, CC> {
    pub inner: cache::base::Base<CC>,

    /// Memory controller
    pub mem_controller: MC,

    /// Specifies type of write allocate request (e.g., L1 or L2)
    write_alloc_type: AccessKind,
    /// Specifies type of writeback request (e.g., L1 or L2)
    write_back_type: AccessKind,
}

impl<MC, CC> Builder<MC, CC>
where
    CC: Clone,
{
    pub fn build(self) -> Data<MC, CC> {
        let inner = super::base::Builder {
            name: self.name,
            core_id: self.core_id,
            cluster_id: self.cluster_id,
            stats: self.stats,
            cache_controller: self.cache_controller,
            cache_config: self.cache_config,
        }
        .build();
        Data {
            inner,
            mem_controller: self.mem_controller,
            write_alloc_type: self.write_alloc_type,
            write_back_type: self.write_back_type,
        }
    }
}

impl<MC, CC> Data<MC, CC> {
    #[inline]
    pub fn set_top_port(&mut self, port: ic::Port<mem_fetch::MemFetch>) {
        self.inner.set_top_port(port);
    }
}

impl<MC, CC> Data<MC, CC>
where
    MC: MemoryController,
    CC: cache::CacheController,
{
    /// Write-back hit: mark block as modified.
    fn write_hit_write_back(
        &mut self,
        addr: address,
        cache_index: usize,
        fetch: &mem_fetch::MemFetch,
        time: u64,
        _events: &mut [cache::Event],
        _probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        debug_assert_eq!(addr, fetch.addr());

        let block_addr = self.inner.cache_controller.block_addr(addr);
        log::debug!(
            "handling WRITE HIT WRITE BACK for {} (block_addr={}, cache_idx={:?})",
            fetch,
            block_addr,
            cache_index,
        );

        // update LRU state
        let tag_array::AccessStatus { cache_index, .. } =
            self.inner.tag_array.access(block_addr, fetch, time);
        let cache_index = cache_index.expect("write hit write back");

        let block = self.inner.tag_array.get_block_mut(cache_index);
        let was_modified_before = block.is_modified();
        block.set_status(cache::block::Status::MODIFIED, &fetch.access.sector_mask);
        block.set_byte_mask(&fetch.access.byte_mask);
        if !was_modified_before {
            self.inner.tag_array.num_dirty += 1;
        }
        self.update_readable(fetch, cache_index);

        cache::RequestStatus::HIT
    }

    fn update_readable(&mut self, fetch: &mem_fetch::MemFetch, cache_index: usize) {
        use crate::mem_sub_partition::{SECTOR_CHUNCK_SIZE, SECTOR_SIZE};
        let block = self.inner.tag_array.get_block_mut(cache_index);
        for i in 0..SECTOR_CHUNCK_SIZE as usize {
            let sector_mask = fetch.access.sector_mask;
            if sector_mask[i] {
                let mut all_set = true;
                for k in (i * SECTOR_SIZE as usize)..((i + 1) * SECTOR_SIZE as usize) {
                    // If any bit in the byte mask (within the sector) is not set,
                    // the sector is unreadble
                    if !block.dirty_byte_mask()[k] {
                        all_set = false;
                        break;
                    }
                }
                if all_set {
                    block.set_readable(true, &fetch.access.sector_mask);
                }
            }
        }
    }

    fn read_hit(
        &mut self,
        addr: address,
        fetch: &mem_fetch::MemFetch,
        time: u64,
        _events: &mut [cache::event::Event],
    ) -> cache::RequestStatus {
        let super::base::Base {
            ref mut tag_array,
            ref cache_controller,
            ..
        } = self.inner;
        let block_addr = cache_controller.block_addr(addr);
        let access_status = tag_array.access(block_addr, fetch, time);
        let cache_index = access_status.cache_index.expect("read hit has cache index");

        // Atomics treated as global read/write requests:
        // Perform read, mark line as MODIFIED
        if fetch.is_atomic() {
            debug_assert_eq!(fetch.access_kind(), AccessKind::GLOBAL_ACC_R);
            let block = tag_array.get_block_mut(cache_index);
            let was_modified_before = block.is_modified();
            block.set_status(cache::block::Status::MODIFIED, &fetch.access.sector_mask);
            block.set_byte_mask(&fetch.access.byte_mask);
            if !was_modified_before {
                tag_array.num_dirty += 1;
            }
        }
        cache::RequestStatus::HIT
    }

    /// Sends write request to lower level memory (write or writeback)
    pub fn send_write_request(
        &mut self,
        mut fetch: mem_fetch::MemFetch,
        request: cache::Event,
        time: u64,
        events: &mut Vec<cache::Event>,
    ) {
        log::debug!("data_cache::send_write_request({})", fetch);
        events.push(request);
        fetch.set_status(self.inner.miss_queue_status, time);
        self.inner.miss_queue.push_back(fetch);
    }

    /// Baseline read miss
    ///
    /// Send read request to lower level memory and perform
    /// write-back as necessary.
    fn read_miss(
        &mut self,
        addr: address,
        cache_index: usize,
        fetch: &mem_fetch::MemFetch,
        time: u64,
        events: &mut Vec<cache::Event>,
        _probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        if !self.inner.miss_queue_can_fit(1) {
            // cannot handle request this cycle, might need to generate two requests
            let mut stats = self.inner.stats.lock();
            let kernel_stats = stats.get_mut(0);
            kernel_stats.inc(
                fetch.allocation_id(),
                fetch.access_kind(),
                cache::AccessStat::ReservationFailure(cache::ReservationFailure::MISS_QUEUE_FULL),
                1,
            );
            return cache::RequestStatus::RESERVATION_FAIL;
        }

        let block_addr = self.inner.cache_controller.block_addr(addr);
        let (should_miss, writeback, evicted) = self.inner.send_read_request(
            addr,
            block_addr,
            cache_index,
            fetch.clone(),
            time,
            events,
            false,
            false,
        );

        let writeback_policy = self.inner.cache_config.write_policy;
        log::debug!(
            "handling READ MISS for {} (should miss={}, writeback={}, writeback policy={:?})",
            fetch,
            should_miss,
            writeback,
            writeback_policy,
        );

        if should_miss {
            // If evicted block is modified and not a write-through
            // (already modified lower level)
            if writeback && writeback_policy != cache::config::WritePolicy::WRITE_THROUGH {
                if let Some(evicted) = evicted {
                    let is_write = true;
                    let writeback_access = mem_fetch::access::Builder {
                        kind: self.write_back_type,
                        addr: evicted.block_addr,
                        allocation: evicted.allocation.clone(),
                        req_size_bytes: evicted.modified_size,
                        is_write,
                        warp_active_mask: fetch.access.warp_active_mask,
                        byte_mask: evicted.byte_mask,
                        sector_mask: evicted.sector_mask,
                    }
                    .build();

                    let mut physical_addr = self
                        .mem_controller
                        .to_physical_address(writeback_access.addr);

                    // the evicted block may have wrong chip id when
                    // advanced L2 hashing is used, so set the right chip
                    // address from the original mf
                    physical_addr.chip = fetch.physical_addr.chip;
                    physical_addr.sub_partition = fetch.physical_addr.sub_partition;

                    let partition_addr = self
                        .mem_controller
                        .memory_partition_address(writeback_access.addr);

                    let writeback_fetch = mem_fetch::Builder {
                        instr: fetch.instr.clone(),
                        access: writeback_access,
                        warp_id: 0,
                        core_id: 0,
                        cluster_id: 0,
                        physical_addr,
                        partition_addr,
                    }
                    .build();

                    let event = cache::Event::WriteBackRequestSent {
                        evicted_block: None,
                    };

                    log::trace!(
                        "handling READ MISS for {}: => sending writeback {}",
                        fetch,
                        writeback_fetch
                    );

                    self.send_write_request(writeback_fetch, event, time, events);
                }
            }
            return cache::RequestStatus::MISS;
        }

        cache::RequestStatus::RESERVATION_FAIL
    }

    fn write_miss_no_write_allocate(
        &mut self,
        addr: address,
        _cache_index: Option<usize>,
        fetch: mem_fetch::MemFetch,
        time: u64,
        events: &mut Vec<cache::Event>,
        _probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        debug_assert_eq!(addr, fetch.addr());
        log::debug!(
            "handling WRITE MISS NO WRITE ALLOCATE for {} (miss_queue_full={})",
            fetch,
            self.inner.miss_queue_full()
        );

        if self.inner.miss_queue_full() {
            let mut stats = self.inner.stats.lock();
            let kernel_stats = stats.get_mut(0);
            kernel_stats.inc(
                fetch.allocation_id(),
                fetch.access_kind(),
                cache::AccessStat::ReservationFailure(cache::ReservationFailure::MISS_QUEUE_FULL),
                1,
            );
            // cannot handle request this cycle
            return cache::RequestStatus::RESERVATION_FAIL;
        }

        // on miss, generate write through
        let event = cache::Event::WriteRequestSent;
        self.send_write_request(fetch, event, time, events);
        cache::RequestStatus::MISS
    }

    #[allow(clippy::needless_pass_by_value)]
    fn write_miss_write_allocate_naive(
        &mut self,
        addr: address,
        cache_index: Option<usize>,
        fetch: mem_fetch::MemFetch,
        time: u64,
        events: &mut Vec<cache::Event>,
        probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        // what exactly is the difference between the addr and the fetch addr?
        debug_assert_eq!(addr, fetch.addr());

        let block_addr = self.inner.cache_controller.block_addr(addr);
        let mshr_addr = self.inner.cache_controller.mshr_addr(fetch.addr());

        // Write allocate, maximum 3 requests:
        //  (write miss, read request, write back request)
        //
        //  Conservatively ensure the worst-case request can be handled this cycle
        let mshr_hit = self.inner.mshrs.get(mshr_addr).is_some();
        let mshr_free = !self.inner.mshrs.full(mshr_addr);
        let mshr_full = !self.inner.miss_queue_can_fit(2);
        let mshr_miss_but_free = !mshr_hit && mshr_free && !self.inner.miss_queue_full();

        log::debug!("handling write miss for {} (block addr={}, mshr addr={}, mshr hit={} mshr avail={}, miss queue full={})", &fetch, block_addr, mshr_addr, mshr_hit, mshr_free, self.inner.miss_queue_can_fit(2));

        if mshr_full || !(mshr_miss_but_free || mshr_hit && mshr_free) {
            // check what is the exact failure reason
            let failure = if mshr_full {
                cache::ReservationFailure::MISS_QUEUE_FULL
            } else if mshr_hit && !mshr_free {
                cache::ReservationFailure::MSHR_MERGE_ENTRY_FAIL
            } else if !mshr_hit && !mshr_free {
                cache::ReservationFailure::MSHR_ENTRY_FAIL
            } else {
                panic!("write_miss_write_allocate_naive bad reason");
            };
            let mut stats = self.inner.stats.lock();
            let kernel_stats = stats.get_mut(0);
            kernel_stats.inc(
                fetch.allocation_id(),
                fetch.access_kind(),
                cache::AccessStat::ReservationFailure(failure),
                1,
            );
            log::debug!("handling write miss for {}: RESERVATION FAIL", &fetch);
            return cache::RequestStatus::RESERVATION_FAIL;
        }

        let event = cache::Event::WriteRequestSent;
        self.send_write_request(fetch.clone(), event, time, events);

        let is_write = false;
        let new_access = mem_fetch::access::Builder {
            kind: self.write_alloc_type,
            addr: fetch.addr(),
            allocation: fetch.access.allocation.clone(),
            req_size_bytes: self.inner.cache_config.atom_size,
            is_write, // Now performing a read
            warp_active_mask: fetch.access.warp_active_mask,
            byte_mask: fetch.access.byte_mask,
            sector_mask: fetch.access.sector_mask,
        }
        .build();

        let physical_addr = self.mem_controller.to_physical_address(new_access.addr);
        let partition_addr = self
            .mem_controller
            .memory_partition_address(new_access.addr);

        let new_fetch = mem_fetch::Builder {
            instr: None,
            access: new_access,
            warp_id: fetch.warp_id,
            core_id: fetch.core_id,
            cluster_id: fetch.cluster_id,
            physical_addr,
            partition_addr,
        }
        .build();

        let Some(cache_index) = cache_index else {
            return cache::RequestStatus::RESERVATION_FAIL;
        };

        // Send read request resulting from write miss
        let is_read_only = false;
        let is_write_allocate = true;
        let (should_miss, writeback, evicted) = self.inner.send_read_request(
            addr,
            block_addr,
            cache_index,
            new_fetch,
            time,
            events,
            is_read_only,
            is_write_allocate,
        );

        events.push(cache::Event::WriteAllocateSent);

        if should_miss {
            // If evicted block is modified and not a write-through
            // (already modified lower level)
            // log::debug!(
            //     "evicted block: {:?}",
            //     evicted.as_ref().map(|e| e.block_addr)
            // );
            let not_write_through =
                self.inner.cache_config.write_policy != cache::config::WritePolicy::WRITE_THROUGH;

            if writeback && not_write_through {
                if let Some(evicted) = evicted {
                    log::debug!("evicted block: {:?}", evicted.block_addr);

                    // SECTOR_MISS and HIT_RESERVED should not send write back
                    debug_assert_eq!(probe_status, cache::RequestStatus::MISS);

                    let is_write = true;
                    let writeback_access = mem_fetch::access::Builder {
                        kind: self.write_back_type,
                        addr: evicted.block_addr,
                        allocation: evicted.allocation.clone(),
                        req_size_bytes: evicted.modified_size,
                        is_write,
                        warp_active_mask: fetch.access.warp_active_mask,
                        byte_mask: evicted.byte_mask,
                        sector_mask: evicted.sector_mask,
                    }
                    .build();

                    // the evicted block may have wrong chip id when advanced L2 hashing
                    // is used, so set the right chip address from the original mf
                    let mut tlx_addr = self
                        // .inner
                        .mem_controller
                        .to_physical_address(writeback_access.addr);
                    tlx_addr.chip = fetch.physical_addr.chip;
                    tlx_addr.sub_partition = fetch.physical_addr.sub_partition;

                    let partition_addr = self
                        .mem_controller
                        .memory_partition_address(writeback_access.addr);

                    let writeback_fetch = mem_fetch::Builder {
                        instr: None,
                        access: writeback_access,
                        warp_id: 0,
                        core_id: 0,
                        cluster_id: 0,
                        physical_addr,
                        partition_addr,
                    }
                    .build();

                    let event = cache::Event::WriteBackRequestSent {
                        evicted_block: Some(evicted),
                    };

                    self.send_write_request(writeback_fetch, event, time, events);
                }
            }
            return cache::RequestStatus::MISS;
        }

        cache::RequestStatus::RESERVATION_FAIL
    }

    fn write_miss(
        &mut self,
        addr: address,
        cache_index: Option<usize>,
        fetch: mem_fetch::MemFetch,
        time: u64,
        events: &mut Vec<cache::Event>,
        probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        use cache::config::WriteAllocatePolicy;
        let func = match self.inner.cache_config.write_allocate_policy {
            WriteAllocatePolicy::NO_WRITE_ALLOCATE => Self::write_miss_no_write_allocate,
            WriteAllocatePolicy::WRITE_ALLOCATE => Self::write_miss_write_allocate_naive,
            WriteAllocatePolicy::FETCH_ON_WRITE => {
                // Self::write_miss_write_allocate_fetch_on_write
                unimplemented!("fetch on write")
            }
            WriteAllocatePolicy::LAZY_FETCH_ON_READ => {
                // Self::write_miss_write_allocate_lazy_fetch_on_read
                unimplemented!("fetch on read")
            }
        };
        (func)(self, addr, cache_index, fetch, time, events, probe_status)
    }

    fn write_hit(
        &mut self,
        addr: address,
        cache_index: usize,
        fetch: &mem_fetch::MemFetch,
        time: u64,
        events: &mut [cache::Event],
        probe_status: cache::RequestStatus,
    ) -> cache::RequestStatus {
        use cache::config::WritePolicy;
        let func = match self.inner.cache_config.write_policy {
            // TODO: make read only policy deprecated
            // READ_ONLY is now a separate cache class, config is deprecated
            WritePolicy::READ_ONLY => unimplemented!("todo: remove the read only cache write policy / writable data cache set as READ_ONLY"),
            WritePolicy::WRITE_BACK => Self::write_hit_write_back,
            WritePolicy::WRITE_THROUGH => unimplemented!("Self::wr_hit_wt"),
            WritePolicy::WRITE_EVICT => unimplemented!("Self::wr_hit_we"),
            WritePolicy::LOCAL_WB_GLOBAL_WT => unimplemented!("Self::wr_hit_global_we_local_wb"),
        };
        (func)(self, addr, cache_index, fetch, time, events, probe_status)
    }

    // A general function that takes the result of a tag_array probe.
    //
    // It performs the correspding functions based on the
    // cache configuration.
    fn process_tag_probe(
        &mut self,
        is_write: bool,
        probe: Option<(usize, cache::RequestStatus)>,
        addr: address,
        fetch: mem_fetch::MemFetch,
        events: &mut Vec<cache::Event>,
        time: u64,
    ) -> cache::RequestStatus {
        // dbg!(cache_index, probe_status);
        // Each function pointer ( m_[rd/wr]_[hit/miss] ) is set in the
        // data_cache constructor to reflect the corresponding cache
        // configuration options.
        //
        // Function pointers were used to avoid many long conditional
        // branches resulting from many cache configuration options.
        let probe_status = probe.map_or(cache::RequestStatus::RESERVATION_FAIL, |(_, s)| s);

        let mut access_status = probe_status;
        let data_size = fetch.data_size();

        assert!(
            !matches!(probe, Some((_, cache::RequestStatus::RESERVATION_FAIL))),
            "reservation fail should not be returned as a status"
        );

        if is_write {
            let no_allocate_on_write = self.inner.cache_config.write_allocate_policy
                == cache::config::WriteAllocatePolicy::NO_WRITE_ALLOCATE;
            match probe {
                Some((cache_index, cache::RequestStatus::HIT)) => {
                    access_status = self.write_hit(
                        addr,
                        cache_index,
                        &fetch,
                        time,
                        events,
                        cache::RequestStatus::RESERVATION_FAIL,
                    );
                }
                None if no_allocate_on_write => {
                    access_status = self.write_miss(
                        addr,
                        None,
                        fetch,
                        time,
                        events,
                        cache::RequestStatus::RESERVATION_FAIL,
                    );
                }
                None => {
                    // the only reason for reservation fail here is LINE_ALLOC_FAIL
                    // (i.e all lines are reserved)
                    let mut stats = self.inner.stats.lock();
                    let kernel_stats = stats.get_mut(0);
                    kernel_stats.inc(
                        fetch.allocation_id(),
                        fetch.access_kind(),
                        cache::AccessStat::ReservationFailure(
                            cache::ReservationFailure::LINE_ALLOC_FAIL,
                        ),
                        1,
                    );
                }
                Some((cache_index, probe_status)) => {
                    access_status =
                        self.write_miss(addr, Some(cache_index), fetch, time, events, probe_status);
                }
            }
        } else {
            match probe {
                None => {
                    // the only reason for reservation fail here is LINE_ALLOC_FAIL
                    // (i.e all lines are reserved)
                    let mut stats = self.inner.stats.lock();
                    let kernel_stats = stats.get_mut(0);
                    kernel_stats.inc(
                        fetch.allocation_id(),
                        fetch.access_kind(),
                        cache::AccessStat::ReservationFailure(
                            cache::ReservationFailure::LINE_ALLOC_FAIL,
                        ),
                        1,
                    );
                }
                Some((_cache_index, cache::RequestStatus::HIT)) => {
                    access_status = self.read_hit(addr, &fetch, time, events);
                }
                Some((cache_index, probe_status)) => {
                    access_status =
                        self.read_miss(addr, cache_index, &fetch, time, events, probe_status);
                }
            }
        }

        self.inner
            .bandwidth
            .use_data_port(data_size, access_status, events);

        access_status
    }
}

impl<MC, CC> crate::engine::cycle::Component for Data<MC, CC> {
    fn cycle(&mut self, cycle: u64) {
        self.inner.cycle(cycle);
    }
}

impl<MC, CC> cache::Cache for Data<MC, CC>
where
    MC: MemoryController,
    CC: cache::CacheController,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn per_kernel_stats(&self) -> &Arc<Mutex<stats::cache::PerKernel>> {
        &self.inner.stats
    }

    fn access(
        &mut self,
        addr: address,
        fetch: mem_fetch::MemFetch,
        events: &mut Vec<cache::Event>,
        time: u64,
    ) -> cache::RequestStatus {
        let super::base::Base {
            ref cache_controller,
            ref cache_config,
            ..
        } = self.inner;

        debug_assert_eq!(&fetch.access.addr, &addr);
        debug_assert!(fetch.data_size() <= cache_config.atom_size);

        let is_write = fetch.is_write();
        let access_kind = fetch.access_kind();
        let allocation_id = fetch.allocation_id();
        let block_addr = cache_controller.block_addr(addr);

        log::debug!(
            "{}::data_cache::access({fetch}, write = {is_write}, size = {}, block = {block_addr}, time = {})",
            self.inner.name,
            fetch.data_size(), time,
        );

        let dbg_fetch = fetch.clone();

        let probe = self
            .inner
            .tag_array
            .probe(block_addr, &fetch, is_write, true);
        let probe_status = probe.map_or(cache::RequestStatus::RESERVATION_FAIL, |(_, s)| s);

        let access_status = self.process_tag_probe(is_write, probe, addr, fetch, events, time);

        log::debug!(
            "{}::access({}) => probe status={:?} access status={:?}",
            self.inner.name,
            &dbg_fetch,
            probe_status,
            access_status
        );

        let stat_cache_request_status = match probe_status {
            cache::RequestStatus::HIT_RESERVED
                if access_status != cache::RequestStatus::RESERVATION_FAIL =>
            {
                probe_status
            }
            cache::RequestStatus::SECTOR_MISS if access_status != cache::RequestStatus::MISS => {
                probe_status
            }
            _ => access_status,
        };
        let mut stats = self.inner.stats.lock();
        let kernel_stats = stats.get_mut(0);
        kernel_stats.inc(
            allocation_id,
            access_kind,
            cache::AccessStat::Status(stat_cache_request_status),
            1,
        );
        access_status
    }

    fn write_allocate_policy(&self) -> cache::config::WriteAllocatePolicy {
        self.inner.cache_config.write_allocate_policy
    }

    fn next_access(&mut self) -> Option<mem_fetch::MemFetch> {
        self.inner.next_access()
    }

    fn ready_accesses(&self) -> Option<&VecDeque<mem_fetch::MemFetch>> {
        self.inner.ready_accesses()
    }

    fn has_ready_accesses(&self) -> bool {
        self.inner.has_ready_accesses()
    }

    fn fill(&mut self, fetch: mem_fetch::MemFetch, time: u64) {
        self.inner.fill(fetch, time);
    }

    fn waiting_for_fill(&self, fetch: &mem_fetch::MemFetch) -> bool {
        self.inner.waiting_for_fill(fetch)
    }

    fn invalidate(&mut self) {
        self.inner.invalidate();
    }

    fn flush(&mut self) -> usize {
        self.inner.flush()
    }
}

impl<MC, CC> cache::Bandwidth for Data<MC, CC> {
    fn has_free_data_port(&self) -> bool {
        self.inner.has_free_data_port()
    }

    fn has_free_fill_port(&self) -> bool {
        self.inner.has_free_fill_port()
    }
}
