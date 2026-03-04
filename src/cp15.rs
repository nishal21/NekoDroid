#[derive(Debug, Default)]
pub struct Cp15 {
    pub c0_midr: u32,
    pub c1_sctlr: u32,
    pub c2_ttbr0: u32,
    pub c3_dacr: u32,
}

impl Cp15 {
    pub fn new() -> Self {
        Self {
            c0_midr: 0x410F_C080,
            c1_sctlr: 0x0000_0000,
            c2_ttbr0: 0,
            c3_dacr: 0,
        }
    }

    pub fn read_register(&self, crn: usize, crm: usize, opc1: usize, opc2: usize) -> u32 {
        match (crn, crm, opc1, opc2) {
            (0, 0, 0, 0) => self.c0_midr,
            (1, 0, 0, 0) => self.c1_sctlr,
            (2, 0, 0, 0) => self.c2_ttbr0,
            (3, 0, 0, 0) => self.c3_dacr,
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
            _ => crate::log(&format!(
                "⚠️ Unimplemented CP15 write: CRn={}, CRm={}, opc1={}, opc2={}, val={:#010X}",
                crn, crm, opc1, opc2, val
            )),
        }
    }
}