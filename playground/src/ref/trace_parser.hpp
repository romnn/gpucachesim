#pragma once

#include <string>
#include <vector>

#include "inst_trace.hpp"
#include "kernel_trace.hpp"
#include "trace_command.hpp"

enum address_format { list_all = 0, base_stride = 1, base_delta = 2 };

class trace_parser {
public:
  trace_parser(const char *kernellist_filepath);

  std::vector<trace_command> parse_commandlist_file();

  kernel_trace_t *parse_kernel_info(const std::string &kerneltraces_filepath);

  void parse_memcpy_info(const std::string &memcpy_command, size_t &add,
                         size_t &count);

  void get_next_threadblock_traces(
      std::vector<std::vector<inst_trace_t> *> threadblock_traces,
      unsigned trace_version, unsigned enable_lineinfo, std::ifstream *ifs);

  void kernel_finalizer(kernel_trace_t *trace_info);

private:
  std::string kernellist_filename;
};