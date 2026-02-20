use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // Initialization errors
    NotInitialized = 1,
    AlreadyInitialized = 2,

    // Authorization errors
    Unauthorized = 3,
    ContractLocked = 4,

    // Share validation errors
    LowShareCount = 5,
    InvalidShareTotal = 6,
    NegativeShareAmount = 7,
    DuplicateShareholder = 8,

    // Token transfer errors
    ZeroTransferAmount = 9,
    TransferAmountAboveBalance = 10,
    TransferAmountAboveUnusedBalance = 11,

    // Distribution round errors
    InvalidDistributionRound = 12,
    RoundNotFound = 13,
    AlreadyClaimed = 14,
    NothingToClaim = 15,
    RoundNotFinalized = 16,

    // Token errors
    InsufficientTokenBalance = 17,
    TokenMintFailed = 18,
    TokenBurnFailed = 19,

    // Arithmetic errors
    Overflow = 20,
    DivisionByZero = 21,

    // Commission errors
    InvalidCommissionRate = 22,

    // Withdrawal errors
    ZeroWithdrawalAmount = 23,
    WithdrawalAmountAboveAllocation = 24,

    // Time-gating and claim window errors (NEW)
    DistributionTooSoon = 25,      // Time-gating: not enough time since last distribution
    ClaimsNotOpenYet = 26,         // Claim window: claims haven't opened yet
    RoundExpired = 27,             // Round has expired, no more claims allowed

    // Scheduling errors (NEW)
    ScheduleNotConfigured = 28,       // No schedule has been set up
    ScheduleNotEnabled = 29,          // Schedule exists but is disabled
    ScheduleCompleted = 30,           // All scheduled distributions have been completed
    ScheduledDistributionNotDue = 31, // Next scheduled distribution is not due yet
    InvalidScheduleConfig = 32,       // Invalid schedule configuration

    // Reclaim errors (NEW)
    RoundNotExpired = 33,          // Cannot reclaim from non-expired round
    NothingToReclaim = 34,         // No unclaimed funds to reclaim

    // Marketplace errors (share trading)
    NoSharesToTransfer = 35,
    InsufficientSharesToTransfer = 36,
    CannotTransferToSelf = 37,
    InvalidShareAmount = 38,
    InvalidPrice = 39,
    NoSharesToSell = 40,
    NoActiveListing = 41,
    InsufficientSharesInListing = 42,
    CannotBuyOwnShares = 43,
}
