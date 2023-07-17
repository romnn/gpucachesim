use std::io::Write;

use crate::bindings;

super::extern_type!(bindings::accelsim_config, "accelsim_config");
super::extern_type!(bindings::pipeline_stage_name_t, "pipeline_stage_name_t");

#[cxx::bridge]
mod default {
    unsafe extern "C++" {
        include!("playground-sys/src/ref/bridge/main.hpp");

        // MEM FETCH
        type mem_fetch_ptr;
        fn get(self: &mem_fetch_ptr) -> *const mem_fetch;

        type mem_fetch = crate::bridge::mem_fetch::mem_fetch;
        type mem_fetch_bridge;
        unsafe fn new_mem_fetch_bridge(ptr: *const mem_fetch) -> SharedPtr<mem_fetch_bridge>;
        #[must_use]
        fn inner(self: &mem_fetch_bridge) -> *const mem_fetch;

        // WARP INST
        type warp_inst_ptr;
        fn get(self: &warp_inst_ptr) -> *const warp_inst_t;

        type warp_inst_t;
        #[must_use]
        fn empty(self: &warp_inst_t) -> bool;
        fn opcode_str(self: &warp_inst_t) -> *const c_char;
        fn get_pc(self: &warp_inst_t) -> u32;
        fn warp_id(self: &warp_inst_t) -> u32;

        type warp_inst_bridge;
        #[must_use]
        unsafe fn new_warp_inst_bridge(ptr: *const warp_inst_t) -> SharedPtr<warp_inst_bridge>;
        #[must_use]
        fn inner(self: &warp_inst_bridge) -> *const warp_inst_t;

        // REGISTER SET
        type register_set_ptr;
        fn get(self: &register_set_ptr) -> *const register_set;

        type register_set;
        fn get_name(self: &register_set) -> *const c_char;

        type register_set_bridge;
        #[must_use]
        unsafe fn new_register_set_bridge(
            ptr: *const register_set,
        ) -> SharedPtr<register_set_bridge>;
        #[must_use]
        fn inner(self: &register_set_bridge) -> *const register_set;
        #[must_use]
        fn get_registers(self: &register_set_bridge) -> UniquePtr<CxxVector<warp_inst_ptr>>;

        // INPUT PORT
        type input_port_t;
        type input_port_bridge;
        unsafe fn new_input_port_bridge(ptr: *const input_port_t) -> SharedPtr<input_port_bridge>;
        fn inner(self: &input_port_bridge) -> *const input_port_t;
        fn get_cu_sets(self: &input_port_bridge) -> &CxxVector<u32>;
        fn get_in_ports(self: &input_port_bridge) -> UniquePtr<CxxVector<register_set_ptr>>;
        fn get_out_ports(self: &input_port_bridge) -> UniquePtr<CxxVector<register_set_ptr>>;

        // OPERAND COLL
        type opndcoll_rfu_t;

        type collector_unit_set;
        fn get_set(self: &collector_unit_set) -> u32;
        fn get_unit(self: &collector_unit_set) -> &collector_unit_t;

        type collector_unit_t;
        fn is_free(self: &collector_unit_t) -> bool;
        fn get_warp_id(self: &collector_unit_t) -> u32;
        fn get_active_count(self: &collector_unit_t) -> u32;
        fn get_reg_id(self: &collector_unit_t) -> u32;
        fn get_output_register(self: &collector_unit_t) -> *mut register_set;

        type dispatch_unit_t;
        fn get_set_id(self: &dispatch_unit_t) -> u32;
        fn get_last_cu(self: &dispatch_unit_t) -> u32;
        fn get_next_cu(self: &dispatch_unit_t) -> u32;

        type operand_collector_bridge;
        fn inner(self: &operand_collector_bridge) -> *const opndcoll_rfu_t;
        fn get_input_ports(self: &operand_collector_bridge) -> &CxxVector<input_port_t>;
        fn get_dispatch_units(self: &operand_collector_bridge) -> &CxxVector<dispatch_unit_t>;
        fn get_collector_units(
            self: &operand_collector_bridge,
        ) -> UniquePtr<CxxVector<collector_unit_set>>;

        // CORE
        type core_bridge;
        #[must_use]
        fn get_register_sets(self: &core_bridge) -> UniquePtr<CxxVector<register_set_ptr>>;
        fn get_operand_collector(self: &core_bridge) -> SharedPtr<operand_collector_bridge>;

        // MEM PARTITION
        type memory_partition_unit_bridge;
        #[must_use]
        fn get_dram_latency_queue(
            self: &memory_partition_unit_bridge,
        ) -> UniquePtr<CxxVector<mem_fetch_ptr>>;

        // MEM SUB PARTITION
        type memory_sub_partition_bridge;
        #[must_use]
        fn get_icnt_L2_queue(
            self: &memory_sub_partition_bridge,
        ) -> UniquePtr<CxxVector<mem_fetch_ptr>>;
        #[must_use]
        fn get_L2_dram_queue(
            self: &memory_sub_partition_bridge,
        ) -> UniquePtr<CxxVector<mem_fetch_ptr>>;
        #[must_use]
        fn get_dram_L2_queue(
            self: &memory_sub_partition_bridge,
        ) -> UniquePtr<CxxVector<mem_fetch_ptr>>;
        #[must_use]
        fn get_L2_icnt_queue(
            self: &memory_sub_partition_bridge,
        ) -> UniquePtr<CxxVector<mem_fetch_ptr>>;

        // ACCELSIM
        type accelsim_bridge;
        type accelsim_config = crate::bindings::accelsim_config;

        #[must_use]
        fn new_accelsim_bridge(
            config: accelsim_config,
            argv: &[&str],
        ) -> UniquePtr<accelsim_bridge>;
        fn run_to_completion(self: Pin<&mut accelsim_bridge>);
        fn process_commands(self: Pin<&mut accelsim_bridge>);
        fn launch_kernels(self: Pin<&mut accelsim_bridge>);
        fn cycle(self: Pin<&mut accelsim_bridge>);
        fn cleanup_finished_kernel(self: Pin<&mut accelsim_bridge>, kernel_uid: u32);
        #[must_use]
        fn get_finished_kernel_uid(self: Pin<&mut accelsim_bridge>) -> u32;

        #[must_use]
        fn get_cycle(self: &accelsim_bridge) -> u64;
        #[must_use]
        fn active(self: &accelsim_bridge) -> bool;
        #[must_use]
        fn limit_reached(self: &accelsim_bridge) -> bool;
        #[must_use]
        fn commands_left(self: &accelsim_bridge) -> bool;
        #[must_use]
        fn active_kernels(self: &accelsim_bridge) -> bool;
        #[must_use]
        fn kernels_left(self: &accelsim_bridge) -> bool;

        // iterate over sub partitions
        #[must_use]
        fn get_sub_partitions(self: &accelsim_bridge) -> &CxxVector<memory_sub_partition_bridge>;

        // iterate over memory partitions
        #[must_use]
        fn get_partition_units(self: &accelsim_bridge) -> &CxxVector<memory_partition_unit_bridge>;

        // iterate over all cores
        #[must_use]
        fn get_cores(self: &accelsim_bridge) -> &CxxVector<core_bridge>;

        // NOTE: stat transfer functions defined in stats.cc bridge
    }
}

pub use default::*;
