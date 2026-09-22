#[derive(Debug, Default)]
pub struct Cp15 {
    pub c0_midr: u32,
    pub c1_sctlr: u32,
    pub c2_ttbr0: u32,
    pub c3_dacr: u32,
    pub c5_dfsr: u32,
    pub c6_dfar: u32,
}

impl Cp15 {
    pub fn new() -> Self {
        Self {
            // ARM926EJ-S MIDR base value (variant/revision-neutral for procinfo matching)
            c0_midr: 0x4106_9265,
            // ARM926 reset-style control register value used by Linux early boot checks.
            c1_sctlr: 0x0000_0C12,
            c2_ttbr0: 0,
            c3_dacr: 0,
            c5_dfsr: 0,
            c6_dfar: 0,
        }
    }

    pub fn read_register(&self, crn: usize, crm: usize, opc1: usize, opc2: usize) -> u32 {
        match (crn, crm, opc1, opc2) {
            (0, 0, 0, 0) => self.c0_midr,
            (1, 0, 0, 0) => self.c1_sctlr,
            (2, 0, 0, 0) => self.c2_ttbr0,
            (3, 0, 0, 0) => self.c3_dacr,
            (5, 0, 0, 0) => self.c5_dfsr,
            (6, 0, 0, 0) => self.c6_dfar,
            // CP15 c7/c8 maintenance/status reads are modeled as benign zero.
            (7, _, _, _) | (8, _, _, _) => 0,
            _ => {
                crate::log(&format!(
                    "⚠️ Unimplemented CP15 read: CRn={}, CRm={}, opc1={}, opc2={}",
                    crn, crm, opc1, opc2
                ));
                0
            }
        }
    }

    pub fn write_register(&mut self, crn: usize, crm: usize, opc1: usize, opc2: usize, val: u32) {
        match (crn, crm, opc1, opc2) {
            (1, 0, 0, 0) => self.c1_sctlr = val,
            (2, 0, 0, 0) => self.c2_ttbr0 = val,
            (3, 0, 0, 0) => self.c3_dacr = val,
            (5, 0, 0, 0) => self.c5_dfsr = val,
            (6, 0, 0, 0) => self.c6_dfar = val,
            // CP15 c7/c8 maintenance operations (cache/TLB/BTB) are no-ops in this model.
            // Linux uses these during MMU enable/transition; treat them as supported.
            (7, _, _, _) | (8, _, _, _) => {}
            _ => crate::log(&format!(
                "⚠️ Unimplemented CP15 write: CRn={}, CRm={}, opc1={}, opc2={}, val={:#010X}",
                crn, crm, opc1, opc2, val
            )),
        }
    }
}