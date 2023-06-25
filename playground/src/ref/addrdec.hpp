#pragma once

#include <memory>
#include <unordered_map>

#include "hal.hpp"
#include "option_parser.hpp"

// start from gpu-misc
unsigned int LOGB2(unsigned int v);

#define gs_min2(a, b) (((a) < (b)) ? (a) : (b))
#define min3(x, y, z) (((x) < (y) && (x) < (z)) ? (x) : (gs_min2((y), (z))))
// end

extern std::unordered_map<new_addr_type, unsigned> address_random_interleaving;

int64_t powli(int64_t x, int64_t y);
uint32_t LOGB2_32(uint32_t v);
uint32_t next_powerOf2(uint32_t n);

new_addr_type addrdec_packbits(new_addr_type mask, new_addr_type val,
                               unsigned char high, unsigned char low);
void addrdec_getmasklimit(new_addr_type mask, unsigned char *high,
                          unsigned char *low);

enum partition_index_function {
  CONSECUTIVE = 0,
  BITWISE_PERMUTATION,
  IPOLY,
  PAE,
  RANDOM,
  CUSTOM
};

struct addrdec_t {
  // void print_hex(FILE *fp) const;
  // void print_dec(FILE *fp) const;

  unsigned chip;
  unsigned bk;
  unsigned row;
  unsigned col;
  unsigned burst;

  unsigned sub_partition;
};

std::ostream &operator<<(std::ostream &os, const addrdec_t &addr);

typedef struct {
  bool run_test;
  int gpgpu_mem_address_mask;
  partition_index_function memory_partition_indexing;
} linear_to_raw_address_translation_params;

class linear_to_raw_address_translation {
public:
  linear_to_raw_address_translation(
      linear_to_raw_address_translation_params params)
      : linear_to_raw_address_translation() {
    addrdec_option =
        "dramid@8;00000000.00000000.00000000.00000000.0000RRRR.RRRRRRRR."
        "RBBBCCCC.BCCSSSSS"; // todo: pass this via params, must live until init
                             // finishes
    run_test = params.run_test;
    gpgpu_mem_address_mask = params.gpgpu_mem_address_mask;
    memory_partition_indexing = params.memory_partition_indexing;
  }
  linear_to_raw_address_translation();
  void configure() {
    addrdec_option = 0;
    run_test = 0;
    gpgpu_mem_address_mask = 0;
    memory_partition_indexing = (partition_index_function)0;
  };
  void addrdec_setoption(option_parser_t opp);
  void init(unsigned int n_channel, unsigned int n_sub_partition_in_channel);

  // accessors
  void addrdec_tlx(new_addr_type addr, addrdec_t *tlx) const;
  new_addr_type partition_address(new_addr_type addr) const;

  void print() const;

private:
  void addrdec_parseoption(const char *option);
  void sweep_test() const; // sanity check to ensure no overlapping

  enum { CHIP = 0, BK = 1, ROW = 2, COL = 3, BURST = 4, N_ADDRDEC };

  const char *addrdec_option;
  int gpgpu_mem_address_mask;
  partition_index_function memory_partition_indexing;
  bool run_test;

  int ADDR_CHIP_S;
  unsigned char addrdec_mklow[N_ADDRDEC];
  unsigned char addrdec_mkhigh[N_ADDRDEC];
  new_addr_type addrdec_mask[N_ADDRDEC];
  new_addr_type sub_partition_id_mask;

  unsigned int gap;
  unsigned m_n_channel;
  int m_n_sub_partition_in_channel;
  int m_n_sub_partition_total;
  unsigned log2channel;
  unsigned log2sub_partition;
  unsigned nextPowerOf2_m_n_channel;
};

std::unique_ptr<linear_to_raw_address_translation>
new_address_translation(linear_to_raw_address_translation_params params);
