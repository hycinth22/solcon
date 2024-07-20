use super::ToBytes;

const RECORD_SIZE: usize = std::mem::size_of::<u128>() 
+ std::mem::size_of::<usize>()
+ std::mem::size_of::<usize>()
+ std::mem::size_of::<u8>();

#[derive(Debug)]
pub struct MemAllocRecord {
    pub timestamp: u128,
    pub addr: usize,
    pub len: usize,
    pub operation_type: u8, // 0 alloc 1 dealloc
}

impl std::fmt::Display for MemAllocRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Record {{ timestamp: {}, addr: {}, len: {}, operation_type: {} }}",
            self.timestamp, self.addr, self.len, self.operation_type
        )
    }
}
impl ToBytes for MemAllocRecord {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = [0; RECORD_SIZE];
        let mut offset = 0;

        bytes[offset..offset + std::mem::size_of::<u128>()].copy_from_slice(&self.timestamp.to_le_bytes());
        offset += std::mem::size_of::<u128>();

        bytes[offset..offset + std::mem::size_of::<usize>()].copy_from_slice(&self.addr.to_le_bytes());
        offset += std::mem::size_of::<usize>();

        bytes[offset..offset + std::mem::size_of::<usize>()].copy_from_slice(&self.len.to_le_bytes());
        offset += std::mem::size_of::<usize>();

        bytes[offset..offset + std::mem::size_of::<u8>()].copy_from_slice(&self.operation_type.to_le_bytes());
        offset += std::mem::size_of::<usize>();
        
        assert_eq!(offset, RECORD_SIZE);
    
        bytes.to_vec()
    }
    
}