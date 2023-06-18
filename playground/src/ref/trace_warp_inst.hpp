#pragma once

#include <string>
#include <unordered_map>

#include "opcode_char.hpp"
#include "warp_instr.hpp"

class inst_trace_t;

class trace_warp_inst_t : public warp_inst_t {
public:
  trace_warp_inst_t() {
    m_opcode = 0;
    should_do_atomic = false;
  }

  trace_warp_inst_t(const class core_config *config) : warp_inst_t(config) {
    m_opcode = 0;
    should_do_atomic = false;
  }

  bool parse_from_trace_struct(
      const inst_trace_t &trace,
      const std::unordered_map<std::string, OpcodeChar> *OpcodeMap,
      const class trace_config *tconfig,
      const class kernel_trace_t *kernel_trace_info);

  unsigned opcode() const { return m_opcode; }
  const char *opcode_str() const;

private:
  unsigned m_opcode;
};

void move_warp(trace_warp_inst_t *&dst, trace_warp_inst_t *&src);
void move_warp(warp_inst_t *&dst, warp_inst_t *&src);