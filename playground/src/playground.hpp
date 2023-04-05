#pragma once

#include <stdio.h>

typedef struct CacheConfig {
  char ct;
  unsigned m_nset;
  unsigned m_line_sz;
  unsigned m_assoc;
  //
  char rp;
  char wp;
  char ap;
  char wap;
  char sif;
  //
  char mshr_type;
  unsigned m_mshr_entries;
  unsigned m_mshr_max_merge;
  unsigned m_miss_queue_size;
  unsigned m_result_fifo_entries;
  unsigned m_data_port_width;
} cache_config;

extern "C" void parse_cache_config(char *config, cache_config *dest);
// void parse_cache_config(char *config, cache_config *dest) {
//   sscanf(config, "%c:%u:%u:%u,%c:%c:%c:%c:%c,%c:%u:%u,%u:%u,%u", &dest->ct,
//          &dest->m_nset, &dest->m_line_sz, &dest->m_assoc, &dest->rp, &dest->wp,
//          &dest->ap, &dest->wap, &dest->sif, &dest->mshr_type,
//          &dest->m_mshr_entries, &dest->m_mshr_max_merge,
//          &dest->m_miss_queue_size, &dest->m_result_fifo_entries,
//          &dest->m_data_port_width);
//   printf("test");
// }