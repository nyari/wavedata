pub mod dec;
pub mod enc;

pub mod nrzi {
    #[derive(Debug, PartialEq)]
    pub enum Value {
        StartOfFrame(u8),
        Bit(bool),
        EndOfFrame(u8),
        StuffBit,
        Complete,
    }
}
