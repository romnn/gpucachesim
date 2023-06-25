use super::base;
use crate::config;
use crate::ported::{address, cache, interconn as ic, mem_fetch, stats::Stats, tag_array};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct ReadOnly<I> {
    inner: base::Base<I>,
}

impl<I> ReadOnly<I> {
    pub fn new(
        name: String,
        core_id: usize,
        cluster_id: usize,
        // tag_array: tag_array::TagArray<()>,
        mem_port: Arc<I>,
        stats: Arc<Mutex<Stats>>,
        config: Arc<config::GPUConfig>,
        cache_config: Arc<config::CacheConfig>,
    ) -> Self {
        let inner = base::Base::new(
            name,
            core_id,
            cluster_id,
            mem_port,
            stats,
            config,
            cache_config,
        );
        Self { inner }
    }

    // pub fn access_ready(&self) -> bool {
    //     todo!("readonly: access_ready");
    // }
}

impl<I> cache::Component for ReadOnly<I>
where
    // I: ic::MemPort,
    I: ic::MemFetchInterface,
    // I: ic::Interconnect<crate::ported::core::Packet> + 'static,
{
    fn cycle(&mut self) {
        self.inner.cycle()
    }
}

impl<I> cache::CacheBandwidth for ReadOnly<I>
where
    I: ic::MemFetchInterface,
{
    fn has_free_data_port(&self) -> bool {
        self.inner.has_free_data_port()
    }

    fn has_free_fill_port(&self) -> bool {
        self.inner.has_free_data_port()
    }
}

impl<I> cache::Cache for ReadOnly<I>
where
    // I: ic::MemPort,
    I: ic::MemFetchInterface,
    // I: ic::Interconnect<crate::ported::core::Packet> + 'static,
{
    fn has_ready_accesses(&self) -> bool {
        self.inner.has_ready_accesses()
    }

    fn next_access(&mut self) -> Option<mem_fetch::MemFetch> {
        self.inner.next_access()
    }

    fn ready_accesses(&self) -> Option<&VecDeque<mem_fetch::MemFetch>> {
        // fn ready_access_iter(&self) -> () {
        self.inner.ready_accesses()
    }

    /// Access read only cache.
    ///
    /// returns `RequestStatus::RESERVATION_FAIL` if
    /// request could not be accepted (for any reason)
    fn access(
        &mut self,
        addr: address,
        fetch: mem_fetch::MemFetch,
        events: &mut Vec<cache::Event>,
    ) -> cache::RequestStatus {
        use cache::RequestStatus as Status;

        let base::Base {
            ref cache_config,
            ref config,
            ref mut tag_array,
            ..
        } = self.inner;
        debug_assert!(fetch.data_size <= cache_config.atom_size());
        debug_assert_eq!(
            cache_config.write_policy,
            config::CacheWritePolicy::READ_ONLY
        );
        debug_assert!(!fetch.is_write());
        let block_addr = cache_config.block_addr(addr);

        println!(
            "{}::readonly_cache::access({addr}, write = {}, data size = {}, control size = {}, block = {block_addr})",
            self.inner.name,
            fetch.is_write(),
            fetch.data_size,
            fetch.control_size,
        );

        // let cache_index = None;
        let is_probe = false;
        let (cache_index, probe_status) =
            tag_array.probe(block_addr, &fetch, fetch.is_write(), is_probe);
        dbg!(&probe_status);
        let mut status = Status::RESERVATION_FAIL;
        let time = 0;

        if probe_status == Status::HIT {
            // update LRU state
            tag_array::AccessStatus { status, .. } = tag_array.access(
                block_addr, time, // cache_index,
                &fetch,
            );
        } else if probe_status != Status::RESERVATION_FAIL {
            dbg!(&self.inner.miss_queue_full());
            if !self.inner.miss_queue_full() {
                // let do_miss = false;
                let (should_miss, writeback, evicted) = self.inner.send_read_request(
                    addr,
                    block_addr,
                    cache_index.unwrap(),
                    fetch.clone(),
                    time,
                    events,
                    true,
                    false,
                );
                if should_miss {
                    status = Status::MISS;
                } else {
                    status = Status::RESERVATION_FAIL;
                }
            } else {
                status = Status::RESERVATION_FAIL;
                let mut stats = self.inner.stats.lock().unwrap();
                stats.inc_access(
                    *fetch.access_kind(),
                    cache::AccessStat::ReservationFailure(
                        cache::ReservationFailure::MISS_QUEUE_FULL,
                    ),
                );
            }
        } else {
            let mut stats = self.inner.stats.lock().unwrap();
            stats.inc_access(
                *fetch.access_kind(),
                cache::AccessStat::ReservationFailure(cache::ReservationFailure::LINE_ALLOC_FAIL),
            );
        }
        let mut stats = self.inner.stats.lock().unwrap();
        stats.inc_access(
            *fetch.access_kind(),
            cache::AccessStat::Status(Stats::select_status(probe_status, status)),
        );
        status
    }

    fn fill(&mut self, fetch: &mut mem_fetch::MemFetch) {
        self.inner.fill(fetch);
    }
}

#[cfg(test)]
mod tests {
    use super::ReadOnly;
    use crate::config::GPUConfig;
    use playground::bridge::readonly_cache as accelsim;

    #[test]
    fn test_read_only_cache() {
        // todo: compare accelsim::read_only_cache and readonly
        let config = GPUConfig::default().data_cache_l1.unwrap();
        assert!(false);
    }
}
