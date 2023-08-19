use super::{kepler, ArchOp, Op, Opcode, OpcodeMap};

pub mod op {
    /// Unique trace instruction opcodes for pascal.
    #[derive(strum::FromRepr, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Op {
        RRO,
        DMNMX,
        DSET,
        BFE,
        BFI,
        ICMP,
        IMADSP,
        SHL,
        XMAD,
        CSET,
        CSETP,
        TEXS,
        TLD4S,
        TLDS,
        CAL,
        JCAL,
        PRET,
        BRK,
        PBK,
        CONT,
        PCNT,
        PEXIT,
        SSY,
        SYNC,
        PSET,
        VMNMX,
        ISET,
    }
}

pub static OPCODES: OpcodeMap = phf::phf_map! {
    // memory ops
    "LD" => Opcode { op: Op::LD, category: ArchOp::LOAD_OP },
    // for now, we ignore constant loads, consider it as ALU_OP, TO DO
    "LDC" => Opcode { op: Op::LDC, category: ArchOp::ALU_OP},
    "LDG" => Opcode { op: Op::LDG, category: ArchOp::LOAD_OP },
    "LDL" => Opcode { op: Op::LDL, category: ArchOp::LOAD_OP },
    "LDS" => Opcode { op: Op::LDS, category: ArchOp::LOAD_OP },
    "LDSM" => Opcode { op: Op::LDSM, category: ArchOp::LOAD_OP },
    "ST" => Opcode { op: Op::ST, category: ArchOp::STORE_OP },
    "STG" => Opcode { op: Op::STG, category: ArchOp::STORE_OP },
    "STL" => Opcode { op: Op::STL, category: ArchOp::STORE_OP },
    "STS" => Opcode { op: Op::STS, category: ArchOp::STORE_OP },
    "ATOM" => Opcode { op: Op::ATOM, category: ArchOp::STORE_OP },
    "ATOMS" => Opcode { op: Op::ATOMS, category: ArchOp::STORE_OP },
    "ATOMG" => Opcode { op: Op::ATOMG, category: ArchOp::STORE_OP },
    "RED" => Opcode { op: Op::RED, category: ArchOp::STORE_OP },
    "MEMBAR" => Opcode { op: Op::MEMBAR, category: ArchOp::MEMORY_BARRIER_OP },
    "LDGSTS" => Opcode { op: Op::LDGSTS, category: ArchOp::LOAD_OP },

    // floating point 32 instructions
    "FADD" => Opcode { op: Op::FADD, category: ArchOp::SP_OP },
    "FADD32I" => Opcode { op: Op::FADD32I, category: ArchOp::SP_OP },
    "FCHK" => Opcode { op: Op::FCHK, category: ArchOp::SP_OP},
    "FFMA32I" => Opcode { op: Op::FFMA32I, category: ArchOp::SP_OP},
    "FFMA" => Opcode { op: Op::FFMA, category: ArchOp::SP_OP},
    "FMNMX" => Opcode { op: Op::FMNMX, category: ArchOp::SP_OP},
    "FMUL" => Opcode { op: Op::FMUL, category: ArchOp::SP_OP},
    "FMUL32I" => Opcode { op: Op::FMUL32I, category: ArchOp::SP_OP},
    "FSEL" => Opcode { op: Op::FSEL, category: ArchOp::SP_OP},
    "FSET" => Opcode { op: Op::FSET, category: ArchOp::SP_OP},
    "FSETP" => Opcode { op: Op::FSETP, category: ArchOp::SP_OP},
    "FSWZADD" => Opcode { op: Op::FSWZADD, category: ArchOp::SP_OP},
    "RRO" => Opcode { op: Op::Pascal(op::Op::RRO), category: ArchOp::SP_OP},
    "FCMP" => Opcode { op: Op::Kepler(kepler::op::Op::FCMP), category: ArchOp::SP_OP},

    // SFU
    "MUFU" => Opcode { op: Op::MUFU, category: ArchOp::SFU_OP},

    // floating point 16 instructions
    "HADD2" => Opcode { op: Op::HADD2, category: ArchOp::SP_OP},
    "HFMA2" => Opcode { op: Op::HFMA2, category: ArchOp::SP_OP},
    "HMUL2" => Opcode { op: Op::HMUL2, category: ArchOp::SP_OP},
    "HSET2" => Opcode { op: Op::HSET2, category: ArchOp::SP_OP},
    "HSETP2" => Opcode { op: Op::HSETP2, category: ArchOp::SP_OP},

    // double precision instructions
    "DADD" => Opcode { op: Op::DADD, category: ArchOp::DP_OP},
    "DFMA" => Opcode { op: Op::DFMA, category: ArchOp::DP_OP},
    "DMUL" => Opcode { op: Op::DMUL, category: ArchOp::DP_OP},
    "DSETP" => Opcode { op: Op::DSETP, category: ArchOp::DP_OP},
    "DMNMX" => Opcode { op: Op::Pascal(op::Op::DMNMX), category: ArchOp::DP_OP},
    "DSET" => Opcode { op: Op::Pascal(op::Op::DSET), category: ArchOp::DP_OP},

    // integer instructions
    "BMSK" => Opcode { op: Op::BMSK, category: ArchOp::INT_OP},
    "BREV" => Opcode { op: Op::BREV, category: ArchOp::INT_OP},
    "FLO" => Opcode { op: Op::FLO, category: ArchOp::INT_OP},
    "IABS" => Opcode { op: Op::IABS, category: ArchOp::INT_OP},
    "IADD" => Opcode { op: Op::IADD, category: ArchOp::INT_OP},
    "IADD3" => Opcode { op: Op::IADD3, category: ArchOp::INT_OP},
    "IADD32I" => Opcode { op: Op::IADD32I, category: ArchOp::INT_OP},
    "IDP" => Opcode { op: Op::IDP, category: ArchOp::INT_OP},
    "IDP4A" => Opcode { op: Op::IDP4A, category: ArchOp::INT_OP},
    "IMAD" => Opcode { op: Op::IMAD, category: ArchOp::INT_OP},
    "IMMA" => Opcode { op: Op::IMMA, category: ArchOp::INT_OP},
    "IMNMX" => Opcode { op: Op::IMNMX, category: ArchOp::INT_OP},
    "IMUL" => Opcode { op: Op::IMUL, category: ArchOp::INT_OP},
    "IMUL32I" => Opcode { op: Op::IMUL32I, category: ArchOp::INT_OP},
    "ISCADD" => Opcode { op: Op::ISCADD, category: ArchOp::INT_OP},
    "ISCADD32I" => Opcode { op: Op::ISCADD32I, category: ArchOp::INT_OP},
    "ISETP" => Opcode { op: Op::ISETP, category: ArchOp::INT_OP},
    "ISET" => Opcode { op: Op::Pascal(op::Op::ISET), category: ArchOp::INT_OP},
    "LEA" => Opcode { op: Op::LEA, category: ArchOp::INT_OP},
    "LOP" => Opcode { op: Op::LOP, category: ArchOp::INT_OP},
    "LOP3" => Opcode { op: Op::LOP3, category: ArchOp::INT_OP},
    "LOP32I" => Opcode { op: Op::LOP32I, category: ArchOp::INT_OP},
    "POPC" => Opcode { op: Op::POPC, category: ArchOp::INT_OP},
    "SHF" => Opcode { op: Op::SHF, category: ArchOp::INT_OP},
    "SHL" => Opcode { op: Op::Pascal(op::Op::SHL), category: ArchOp::INT_OP},
    "SHR" => Opcode { op: Op::SHR, category: ArchOp::INT_OP},
    "VABSDIFF" => Opcode { op: Op::VABSDIFF, category: ArchOp::INT_OP},
    "VABSDIFF4" => Opcode { op: Op::VABSDIFF4, category: ArchOp::INT_OP},
    "VADD" => Opcode { op: Op::VADD, category: ArchOp::INT_OP},
    "BFE" => Opcode { op: Op::Pascal(op::Op::BFE), category: ArchOp::INT_OP},
    "BFI" => Opcode { op: Op::Pascal(op::Op::BFI), category: ArchOp::INT_OP},
    "ICMP" => Opcode { op: Op::Pascal(op::Op::ICMP), category: ArchOp::INT_OP},
    "IMADSP" => Opcode { op: Op::Pascal(op::Op::IMADSP), category: ArchOp::INT_OP},
    "XMAD" => Opcode { op: Op::Pascal(op::Op::XMAD), category: ArchOp::INT_OP},
    "VMNMX" => Opcode { op: Op::Pascal(op::Op::VMNMX), category: ArchOp::INT_OP},

    // conversion instructions
    "F2F" => Opcode { op: Op::F2F, category: ArchOp::ALU_OP},
    "F2I" => Opcode { op: Op::F2I, category: ArchOp::ALU_OP},
    "I2F" => Opcode { op: Op::I2F, category: ArchOp::ALU_OP},
    "I2I" => Opcode { op: Op::I2I, category: ArchOp::ALU_OP},
    "I2IP" => Opcode { op: Op::I2IP, category: ArchOp::ALU_OP},
    "FRND" => Opcode { op: Op::FRND, category: ArchOp::ALU_OP},

    // movement instructions
    "MOV" => Opcode { op: Op::MOV, category: ArchOp::ALU_OP},
    "MOV32I" => Opcode { op: Op::MOV32I, category: ArchOp::ALU_OP},
    "PRMT" => Opcode { op: Op::PRMT, category: ArchOp::ALU_OP},
    "SEL" => Opcode { op: Op::SEL, category: ArchOp::ALU_OP},
    "SGXT" => Opcode { op: Op::SGXT, category: ArchOp::ALU_OP},
    "SHFL" => Opcode { op: Op::SHFL, category: ArchOp::ALU_OP},

    // Predicate Instructions
    "PLOP3" => Opcode { op: Op::PLOP3, category: ArchOp::ALU_OP},
    "PSETP" => Opcode { op: Op::PSETP, category: ArchOp::ALU_OP},
    "P2R" => Opcode { op: Op::P2R, category: ArchOp::ALU_OP},
    "R2P" => Opcode { op: Op::R2P, category: ArchOp::ALU_OP},
    "CSET" => Opcode { op: Op::Pascal(op::Op::CSET), category: ArchOp::ALU_OP},
    "CSETP" => Opcode { op: Op::Pascal(op::Op::CSETP), category: ArchOp::ALU_OP},
    "PSET" => Opcode { op: Op::Pascal(op::Op::PSET), category: ArchOp::ALU_OP},

    "MATCH" => Opcode { op: Op::MATCH, category: ArchOp::ALU_OP},
    "QSPC" => Opcode { op: Op::QSPC, category: ArchOp::ALU_OP},
    "CCTL" => Opcode { op: Op::CCTL, category: ArchOp::ALU_OP},
    "CCTLL" => Opcode { op: Op::CCTLL, category: ArchOp::ALU_OP},
    "ERRBAR" => Opcode { op: Op::ERRBAR, category: ArchOp::ALU_OP},
    "CCTLT" => Opcode { op: Op::CCTLT, category: ArchOp::ALU_OP},

    // texture instructions (for now, ignore texture loads, consider it as ALU_OP)
    "TEX" => Opcode { op: Op::TEX, category: ArchOp::ALU_OP},
    "TLD" => Opcode { op: Op::TLD, category: ArchOp::ALU_OP},
    "TLD4" => Opcode { op: Op::TLD4, category: ArchOp::ALU_OP},
    "TMML" => Opcode { op: Op::TMML, category: ArchOp::ALU_OP},
    "TXD" => Opcode { op: Op::TXD, category: ArchOp::ALU_OP},
    "TXQ" => Opcode { op: Op::TXQ, category: ArchOp::ALU_OP},
    "TEXS" => Opcode { op: Op::Pascal(op::Op::TEXS), category: ArchOp::ALU_OP},
    "TLD4S" => Opcode { op: Op::Pascal(op::Op::TLD4S), category: ArchOp::ALU_OP},
    "TLDS" => Opcode { op: Op::Pascal(op::Op::TLDS), category: ArchOp::ALU_OP},

    // control instructions:
    // execute branch insts on a dedicated branch unit (SPECIALIZED_UNIT_1)
    "EXIT" => Opcode { op: Op::EXIT, category: ArchOp::EXIT_OPS},
    "BMOV" => Opcode { op: Op::BMOV, category: ArchOp::BRANCH_OP},
    "BPT" => Opcode { op: Op::BPT, category: ArchOp::BRANCH_OP},
    "BRA" => Opcode { op: Op::BRA, category: ArchOp::BRANCH_OP},
    "BREAK" => Opcode { op: Op::BREAK, category: ArchOp::BRANCH_OP},
    "BRX" => Opcode { op: Op::BRX, category: ArchOp::BRANCH_OP},
    // "BRXU" => Opcode { op: Op::Turing(turing::op::Op::BRXU), category: ArchOp::SPECIALIZED_UNIT_1_OP},
    "BSSY" => Opcode { op: Op::BSSY, category: ArchOp::BRANCH_OP},
    "BSYNC" => Opcode { op: Op::BSYNC, category: ArchOp::BRANCH_OP},
    "CALL" => Opcode { op: Op::CALL, category: ArchOp::CALL_OPS},
    "JMP" => Opcode { op: Op::JMP, category: ArchOp::BRANCH_OP},
    "SSY" => Opcode { op: Op::Pascal(op::Op::SSY), category: ArchOp::BRANCH_OP},
    "SYNC" => Opcode { op: Op::Pascal(op::Op::SYNC), category: ArchOp::BRANCH_OP},
    "JMX" => Opcode { op: Op::JMX, category: ArchOp::BRANCH_OP},
    // "JMXU" => Opcode { op: Op::Turing(turing::op::Op::JMXU), category: ArchOp::SPECIALIZED_UNIT_1_OP},
    "NANOSLEEP" => Opcode { op: Op::NANOSLEEP, category: ArchOp::BRANCH_OP},
    "RET" => Opcode { op: Op::RET, category: ArchOp::RET_OPS},
    "RPCMOV" => Opcode { op: Op::RPCMOV, category: ArchOp::BRANCH_OP},
    "RTT" => Opcode { op: Op::RTT, category: ArchOp::BRANCH_OP},
    "WARPSYNC" => Opcode { op: Op::WARPSYNC, category: ArchOp::BRANCH_OP},
    "YIELD" => Opcode { op: Op::YIELD, category: ArchOp::BRANCH_OP},
    "KILL" => Opcode { op: Op::KILL, category: ArchOp::BRANCH_OP},

    // call ops
    "CAL" => Opcode { op: Op::Pascal(op::Op::CAL), category: ArchOp::CALL_OPS},
    "JCAL" => Opcode { op: Op::Pascal(op::Op::JCAL), category: ArchOp::CALL_OPS},
    "PRET" => Opcode { op: Op::Pascal(op::Op::PRET), category: ArchOp::CALL_OPS},
    "BRK" => Opcode { op: Op::Pascal(op::Op::BRK), category: ArchOp::CALL_OPS},
    "PBK" => Opcode { op: Op::Pascal(op::Op::PBK), category: ArchOp::CALL_OPS},
    "CONT" => Opcode { op: Op::Pascal(op::Op::CONT), category: ArchOp::CALL_OPS},
    "PCNT" => Opcode { op: Op::Pascal(op::Op::PCNT), category: ArchOp::CALL_OPS},
    "PEXIT" => Opcode { op: Op::Pascal(op::Op::PEXIT), category: ArchOp::CALL_OPS},

    // Miscellaneous Instructions
    "B2R" => Opcode { op: Op::B2R, category: ArchOp::ALU_OP},
    "BAR" => Opcode { op: Op::BAR, category: ArchOp::BARRIER_OP},
    "CS2R" => Opcode { op: Op::CS2R, category: ArchOp::ALU_OP},
    "CSMTEST" => Opcode { op: Op::CSMTEST, category: ArchOp::ALU_OP},
    "DEPBAR" => Opcode { op: Op::DEPBAR, category: ArchOp::ALU_OP},
    "GETLMEMBASE" => Opcode { op: Op::GETLMEMBASE, category: ArchOp::ALU_OP},
    "LEPC" => Opcode { op: Op::LEPC, category: ArchOp::ALU_OP},
    "NOP" => Opcode { op: Op::NOP, category: ArchOp::ALU_OP},
    "PMTRIG" => Opcode { op: Op::PMTRIG, category: ArchOp::ALU_OP},
    "R2B" => Opcode { op: Op::R2B, category: ArchOp::ALU_OP},
    "S2R" => Opcode { op: Op::S2R, category: ArchOp::ALU_OP},
    "SETCTAID" => Opcode { op: Op::SETCTAID, category: ArchOp::ALU_OP},
    "SETLMEMBASE" => Opcode { op: Op::SETLMEMBASE, category: ArchOp::ALU_OP},
    "VOTE" => Opcode { op: Op::VOTE, category: ArchOp::ALU_OP},
    "VOTE_VTG" => Opcode { op: Op::VOTE_VTG, category: ArchOp::ALU_OP},
};
