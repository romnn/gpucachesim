#pragma once

#include "ref/addrdec.hpp"
#include "ref/barrier_set.hpp"
#include "ref/box_interconnect.hpp"
#include "ref/cache.hpp"
#include "ref/cache_reservation_fail_reason.hpp"
#include "ref/cache_access_logger_types.hpp"
#include "ref/cache_config.hpp"
#include "ref/cache_sub_stats.hpp"
#include "ref/command_type.hpp"
#include "ref/cu_event.hpp"
#include "ref/cu_stream.hpp"
#include "ref/data_cache.hpp"
#include "ref/dim3.hpp"
#include "ref/gpgpu_context.hpp"
#include "ref/gpgpu_functional_sim_config.hpp"
#include "ref/gpgpu_sim_config.hpp"
#include "ref/hal.hpp"
#include "ref/icnt_wrapper.hpp"
#include "ref/ifetch_buffer.hpp"
#include "ref/inst_memadd_info.hpp"
#include "ref/inst_trace.hpp"
#include "ref/instr.hpp"
#include "ref/intersim2/interconnect_interface.hpp"
#include "ref/intersim2/intersim_config.hpp"
#include "ref/kernel_trace.hpp"
#include "ref/l1_cache.hpp"
#include "ref/l2_cache.hpp"
#include "ref/l2_cache_config.hpp"
#include "ref/l2_interface.hpp"
#include "ref/ldst_unit.hpp"
#include "ref/local_interconnect.hpp"
#include "ref/lrr_scheduler.hpp"
#include "ref/mem_fetch.hpp"
#include "ref/mem_fetch_allocator.hpp"
#include "ref/mem_fetch_interface.hpp"
#include "ref/mem_stage_access_type.hpp"
#include "ref/mem_stage_stall_type.hpp"
#include "ref/memory_config.hpp"
#include "ref/memory_partition_unit.hpp"
#include "ref/memory_sub_partition.hpp"
#include "ref/opcode_char.hpp"
#include "ref/operand_type.hpp"
#include "ref/opndcoll_rfu.hpp"
#include "ref/option_parser.hpp"
#include "ref/partition_mf_allocator.hpp"
#include "ref/pipelined_simd_unit.hpp"
#include "ref/read_only_cache.hpp"
#include "ref/rec_pts.hpp"
#include "ref/register_set.hpp"
#include "ref/scheduler_unit.hpp"
#include "ref/scoreboard.hpp"
#include "ref/shader_core_config.hpp"
#include "ref/shader_core_mem_fetch_allocator.hpp"
#include "ref/shader_trace.hpp"
#include "ref/simd_function_unit.hpp"
#include "ref/stats/histogram.hpp"
#include "ref/stats_wrapper.hpp"
#include "ref/stream_manager.hpp"
#include "ref/stream_operation.hpp"
#include "ref/tag_array.hpp"
#include "ref/tex_cache.hpp"
#include "ref/thread_ctx.hpp"
#include "ref/trace.hpp"
#include "ref/trace_command.hpp"
#include "ref/trace_config.hpp"
#include "ref/trace_function_info.hpp"
#include "ref/trace_gpgpu_sim.hpp"
#include "ref/trace_kernel_info.hpp"
#include "ref/trace_shader_core_ctx.hpp"
#include "ref/trace_shd_warp.hpp"
#include "ref/trace_simt_core_cluster.hpp"
#include "ref/trace_warp_inst.hpp"
#include "ref/warp_set.hpp"

#include "ref/bridge/accelsim_config.hpp"
#include "ref/bridge/accelsim_stats.hpp"
#include "ref/bridge/trace_entry.hpp"
#include "ref/bridge/core.hpp"

#include "tests/parse_cache_config.hpp"
