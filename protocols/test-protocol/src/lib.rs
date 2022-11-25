#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SYN(pub Vec<u8>);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SYNACK(pub Vec<u8>);