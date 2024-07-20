use super::ToBytes;

const RECORD_SIZE: usize = std::mem::size_of::<u128>() 
+ std::mem::size_of::<u64>()
+ std::mem::size_of::<u64>()
+ std::mem::size_of::<u32>();

#[derive(Debug)]
pub struct SyncRecord {
    pub timestamp: u128,
    pub thread_id: u64,
    pub memory_address: u64,
    pub operation_type: u32,
}

impl std::fmt::Display for SyncRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Record {{ timestamp: {}, thread_id: {}, memory_address: {}, operation_type: {} }}",
            self.timestamp, self.thread_id, self.memory_address, self.operation_type
        )
    }
}

impl ToBytes for SyncRecord {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = [0; RECORD_SIZE];
        let mut offset = 0;

        bytes[offset..offset + std::mem::size_of::<u128>()].copy_from_slice(&self.timestamp.to_le_bytes());
        offset += std::mem::size_of::<u128>();

        bytes[offset..offset + std::mem::size_of::<u64>()].copy_from_slice(&self.thread_id.to_le_bytes());
        offset += std::mem::size_of::<u64>();

        bytes[offset..offset + std::mem::size_of::<u64>()].copy_from_slice(&self.memory_address.to_le_bytes());
        offset += std::mem::size_of::<u64>();

        bytes[offset..offset + std::mem::size_of::<u32>()].copy_from_slice(&self.operation_type.to_le_bytes());
        offset += std::mem::size_of::<u32>();
        
        assert_eq!(offset, RECORD_SIZE);
        bytes.to_vec()
    }
    
}