#pragma once

#include "../trace_gpgpu_sim.hpp"
#include "playground/src/bridge/main.rs.h"

class trace_gpgpu_sim_bridge : public trace_gpgpu_sim {
 public:
  using trace_gpgpu_sim::trace_gpgpu_sim;

  void transfer_stats(Stats &stats);
  void transfer_dram_stats(Stats &stats);
  void transfer_general_stats(Stats &stats);
  void transfer_core_cache_stats(Stats &stats);
  void transfer_l2d_stats(Stats &stats);
};
