// ── nekodroid: ARMv7 Memory Management Unit ────────────────────────────
//
// Flat byte-addressable RAM with little-endian read/write operations.
// This serves as the memory bus for the emulated CPU.

/// Default RAM size: 16 MB (enough for basic ARM programs)
const DEFAULT_RAM_SIZE: usize = 16 * 1024 * 1024;

/// The Memory Management Unit — a flat byte-addressable memory bus.
///
/// In a real ARM system, the MMU handles virtual → physical address
/// translation, caching, and memory-mapped I/O. For now, we implement
/// a simple flat memory model that can be extended later.
pub struct Mmu {
    ram: Vec<u8>,
}

impl Mmu {
    /// Creates a new MMU with the given RAM size in bytes.
    pub fn new(size: usize) -> Self {
        Mmu {
            ram: vec![0u8; size],
        }
    }

    /// Creates a new MMU with the default 16 MB of RAM.
    pub fn default() -> Self {
        Self::new(DEFAULT_RAM_SIZE)
    }

    /// Returns the total RAM size in bytes.
    pub fn ram_size(&self) -> usize {
        self.ram.len()
    }

    // ── Read operations (little-endian) ───────────────────────────────

    /// Reads a single byte from the given address.
    pub fn read_u8(&self, addr: u32) -> u8 {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a]
        } else {
            // Out-of-bounds reads return 0 (unmapped memory)
            0
        }
    }

    /// Reads a 16-bit value (little-endian) from the given address.
    pub fn read_u16(&self, addr: u32) -> u16 {
        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Reads a 32-bit value (little-endian) from the given address.
    pub fn read_u32(&self, addr: u32) -> u32 {
        let b0 = self.read_u8(addr) as u32;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u32;
        let b2 = self.read_u8(addr.wrapping_add(2)) as u32;
        let b3 = self.read_u8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    // ── Write operations (little-endian) ──────────────────────────────

    /// Writes a single byte to the given address.
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        let a = addr as usize;
        if a < self.ram.len() {
            self.ram[a] = val;
        }
        // Out-of-bounds writes are silently ignored (unmapped memory)
    }

    /// Writes a 16-bit value (little-endian) to the given address.
    pub fn write_u16(&mut self, addr: u32, val: u16) {
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
    }

    /// Writes a 32-bit value (little-endian) to the given address.
    pub fn write_u32(&mut self, addr: u32, val: u32) {
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((val >> 8) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(2), ((val >> 16) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(3), ((val >> 24) & 0xFF) as u8);
    }

    // ── Bulk operations ───────────────────────────────────────────────

    /// Loads a byte slice into memory starting at the given address.
    /// Used for loading binary images (kernel, programs) into RAM.
    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) {
        let start = addr as usize;
        let end = start + data.len();
        if end <= self.ram.len() {
            self.ram[start..end].copy_from_slice(data);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_u8() {
        let mut mmu = Mmu::new(1024);
        mmu.write_u8(0x10, 0xAB);
        assert_eq!(mmu.read_u8(0x10), 0xAB);
    }

    #[test]
    fn test_read_write_u16_little_endian() {
        let mut mmu = Mmu::new(1024);
        mmu.write_u16(0x20, 0xBEEF);
        // Little-endian: low byte first
        assert_eq!(mmu.read_u8(0x20), 0xEF); // low byte
        assert_eq!(mmu.read_u8(0x21), 0xBE); // high byte
        assert_eq!(mmu.read_u16(0x20), 0xBEEF);
    }

    #[test]
    fn test_read_write_u32_little_endian() {
        let mut mmu = Mmu::new(1024);
        mmu.write_u32(0x30, 0xDEADBEEF);
        assert_eq!(mmu.read_u8(0x30), 0xEF);
        assert_eq!(mmu.read_u8(0x31), 0xBE);
        assert_eq!(mmu.read_u8(0x32), 0xAD);
        assert_eq!(mmu.read_u8(0x33), 0xDE);
        assert_eq!(mmu.read_u32(0x30), 0xDEADBEEF);
    }

    #[test]
    fn test_out_of_bounds_reads_zero() {
        let mmu = Mmu::new(64);
        assert_eq!(mmu.read_u8(100), 0);
        assert_eq!(mmu.read_u32(100), 0);
    }

    #[test]
    fn test_load_bytes() {
        let mut mmu = Mmu::new(1024);
        let program = [0x01, 0x02, 0x03, 0x04];
        mmu.load_bytes(0x100, &program);
        assert_eq!(mmu.read_u32(0x100), 0x04030201); // little-endian
    }
}
