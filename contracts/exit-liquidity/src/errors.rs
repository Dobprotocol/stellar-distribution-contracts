use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidPrice = 5,
    OfferNotFound = 6,
    OfferNotActive = 7,
    InsufficientLiquidity = 8,
    BelowMinShares = 9,
    InvalidFeeRate = 10,
    CannotSellToSelf = 11,
    Overflow = 12,
    InvalidMinShares = 13,
}
