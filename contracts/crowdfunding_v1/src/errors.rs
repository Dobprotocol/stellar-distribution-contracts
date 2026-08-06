use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // Initialization
    NotInitialized = 1,
    AlreadyInitialized = 2,
    // Authorization
    Unauthorized = 3,
    // Campaign state
    CampaignNotActive = 4,
    CampaignAlreadyFinalized = 5,
    DeadlineNotReached = 6,
    CampaignNotSucceeded = 7,
    CampaignNotFailed = 8,
    CampaignAlreadyActivated = 9,
    // Contribution
    InvalidSharesAmount = 10,
    HardCapReached = 11,
    // Refund
    NothingToRefund = 12,
    // Pricing / config
    InvalidPrice = 13,
    InvalidCap = 14,
    InvalidDeadline = 15,
    // Arithmetic
    Overflow = 16,
    // Activation (AUDIT 2026-08 / C-1, C-2)
    NoPendingActivation = 17,
    ActivationTimelockPending = 18,
    SplitterMismatch = 19,
    InvalidSplitter = 20,
    ActivationWindowExpired = 21,
    ActivationWindowStillOpen = 22,
    /// Kept declared so the discriminants below never shift, but no longer
    /// returned: `opt_out` is gated on a proposal being pending rather than on
    /// the (zero) activation timelock.
    NoticePeriodOver = 23,
    /// Payout mode. The wasm live on mainnet numbers this 17; here 17…23 were
    /// already taken by the audit errors above, and renumbering them would
    /// invalidate the codes quoted in the audit report and in the tests.
    /// Nothing off-chain matches on the numeric code — the back-end validates
    /// the payout mode itself before it ever reaches the contract — so the two
    /// builds are free to disagree on this one discriminant.
    InvalidPayoutMode = 24,
}
