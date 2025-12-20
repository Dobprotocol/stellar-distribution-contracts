use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractLocked = 4,
    LowShareCount = 5,
    InvalidShareTotal = 6,
    // Token transfer errors
    ZeroTransferAmount = 7,
    TransferAmountAboveBalance = 8,
    TransferAmountAboveUnusedBalance = 9,
    // Token withdrawal errors
    ZeroWithdrawalAmount = 10,
    WithdrawalAmountAboveAllocation = 11,
    // Share marketplace errors
    NoSharesToSell = 12,
    NoActiveListing = 13,
    InsufficientSharesInListing = 14,
    InvalidPrice = 15,
    InvalidShareAmount = 16,
    CannotBuyOwnShares = 17,
    // Share transfer errors
    NoSharesToTransfer = 18,
    InsufficientSharesToTransfer = 19,
    CannotTransferToSelf = 20,
    // Arithmetic errors
    Overflow = 21,
    // Share validation errors
    NegativeShareAmount = 22,
    DuplicateShareholder = 23,
    // Commission errors
    InvalidCommissionRate = 24,
}
